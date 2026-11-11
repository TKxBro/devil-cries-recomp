pub mod h264;
pub mod nvdec;
pub mod vic;
pub mod vp9;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

use crate::gpu::{block_linear_offset, Gpu, NvMap};
use crate::mem::GuestMemory;
use crate::process::Process;

pub const CLASS_HOST1X: u32 = 0x01;
pub const CLASS_VIC: u32 = 0x5d;
pub const CLASS_NVDEC: u32 = 0xf0;

const THI_INCR_SYNCPT: u32 = 0x00;
const THI_METHOD0: u32 = 0x10;
const THI_METHOD1: u32 = 0x11;

const IOVA_BASE: u64 = 0x10_0000;
const IOVA_PAGE: u64 = 0x1000;

pub(crate) fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("KIRIKAE_TRACE_VIDEO").is_some())
}

pub(crate) fn media_timing() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("KIRIKAE_TRACE_MEDIA_TIMING").is_some())
}

#[derive(Default)]
struct Iova {
    handles: HashMap<u32, u64>,
    maps: BTreeMap<u64, (u64, u64)>,
    next: u64,
}

impl Iova {
    fn map(&mut self, handle: u32, m: NvMap) -> Option<u32> {
        if let Some(&iova) = self.handles.get(&handle) {
            let size = m.size.div_ceil(IOVA_PAGE) * IOVA_PAGE;
            if self.maps.get(&iova).is_some_and(|&(_, s)| s >= size) {
                self.maps.insert(iova, (m.addr, size));
                return Some(iova as u32);
            }
        }
        let size = m.size.div_ceil(IOVA_PAGE).max(1) * IOVA_PAGE;
        let iova = self.next.max(IOVA_BASE);
        if iova + size > u32::MAX as u64 {
            return None;
        }
        self.next = iova + size;
        self.handles.insert(handle, iova);
        self.maps.insert(iova, (m.addr, size));
        Some(iova as u32)
    }

    fn translate(&self, iova: u64, len: u64) -> Option<u64> {
        let (&start, &(cpu, size)) = self.maps.range(..=iova).next_back()?;
        (cpu != 0 && iova + len <= start + size).then(|| cpu + (iova - start))
    }
}

pub struct Dma<'a> {
    iova: &'a Iova,
    mem: &'a GuestMemory,
    gpu: &'a Gpu,
    later: Option<Later<'a>>,
    h264: Option<Arc<H264Worker>>,
}

type Later<'a> = (&'static Process, &'a Arc<Mutex<Iova>>, &'a Arc<Worker>);

impl Dma<'_> {
    pub fn defer(&self, job: impl FnOnce(&Dma) -> Result<(), String> + Send + 'static) -> Result<(), String> {
        static ASYNC: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let deferred = *ASYNC.get_or_init(|| std::env::var_os("KIRIKAE_NVDEC_ASYNC").is_some());
        match self.later {
            Some((p, iova, worker)) if deferred => {
                let iova = iova.clone();
                worker.push(p, Box::new(move || {
                    let iova = iova.lock().unwrap();
                    job(&Dma { iova: &iova, mem: &p.memory, gpu: &p.gpu, later: None, h264: None })
                }));
                Ok(())
            }
            _ => job(self),
        }
    }

    pub fn read(&self, offset: u32, len: usize) -> Option<Vec<u8>> {
        let cpu = self.iova.translate((offset as u64) << 8, len as u64)?;
        self.mem.read(cpu, len as u64)
    }

    pub fn write(&self, offset: u32, bytes: &[u8]) -> bool {
        let Some(cpu) = self.iova.translate((offset as u64) << 8, bytes.len() as u64) else { return false };
        let ok = self.mem.write(cpu, bytes);
        self.gpu.guest_memory_written(cpu, bytes.len() as u64);
        ok
    }
}

#[derive(Default)]
struct Engines {
    nvdec: nvdec::Nvdec,
    vic: vic::Vic,
    nvdec_method: u32,
    vic_method: u32,
}

type Job = Box<dyn FnOnce() -> Result<(), String> + Send>;

#[derive(Default)]
pub struct Worker {
    state: Mutex<(VecDeque<Job>, bool, bool)>,
    changed: Condvar,
}

impl Worker {
    fn push(self: &Arc<Self>, p: &'static Process, job: Job) {
        let mut st = self.state.lock().unwrap();
        st.0.push_back(job);
        if !st.2 {
            st.2 = true;
            let w = self.clone();
            let _ = std::thread::Builder::new().name("kirikae-host1x".into()).spawn(move || w.run(p));
        }
        self.changed.notify_all();
    }

    fn run(&self, p: &'static Process) {
        loop {
            let mut st = self.state.lock().unwrap();
            while st.0.is_empty() {
                st = self.changed.wait(st).unwrap();
            }
            let job = st.0.pop_front().unwrap();
            st.1 = true;
            drop(st);
            if let Err(e) = job() {
                p.warn_once(e);
            }
            self.state.lock().unwrap().1 = false;
            self.changed.notify_all();
        }
    }

    fn drain(&self) {
        let mut st = self.state.lock().unwrap();
        while !st.0.is_empty() || st.1 {
            st = self.changed.wait(st).unwrap();
        }
    }
}

struct H264Queue<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T> H264Queue<T> {
    fn new(capacity: usize) -> H264Queue<T> {
        assert!(capacity > 0);
        H264Queue { items: VecDeque::with_capacity(capacity), capacity }
    }

    fn push(&mut self, item: T) -> Result<(), T> {
        if self.items.len() == self.capacity {
            Err(item)
        } else {
            self.items.push_back(item);
            Ok(())
        }
    }

    fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn is_full(&self) -> bool {
        self.items.len() == self.capacity
    }
}

impl<T> Default for H264Queue<T> {
    fn default() -> Self {
        H264Queue::new(8)
    }
}

struct H264Job {
    process: &'static Process,
    iova: Arc<Mutex<Iova>>,
    context: u32,
    info: h264::PictureInfo,
    slices: Vec<u8>,
    output_surface: usize,
    luma: u32,
    chroma: u32,
}

enum H264Work {
    Decode(H264Job),
    Callback(Box<dyn FnOnce() -> Result<(), String> + Send>),
}

struct H264Item {
    work: H264Work,
    done: Arc<(Mutex<Option<Result<(), String>>>, Condvar)>,
}

pub(crate) struct H264Completion {
    done: Arc<(Mutex<Option<Result<(), String>>>, Condvar)>,
}

impl H264Completion {
    pub(crate) fn wait(self) -> Result<(), String> {
        let (done, changed) = &*self.done;
        let mut result = done.lock().unwrap();
        while result.is_none() {
            result = changed.wait(result).unwrap();
        }
        result.take().unwrap()
    }
}

impl H264Item {
    fn completion(&self) -> H264Completion {
        H264Completion { done: self.done.clone() }
    }
}

#[derive(Default)]
pub(crate) struct H264Worker {
    state: Mutex<(H264Queue<H264Item>, bool, bool)>,
    changed: Condvar,
}

impl H264Worker {
    fn enqueue(self: &Arc<Self>, work: H264Work) -> Result<H264Completion, String> {
        let item = H264Item { work, done: Arc::new((Mutex::new(None), Condvar::new())) };
        let completion = item.completion();
        let mut state = self.state.lock().unwrap();
        while state.0.is_full() {
            state = self.changed.wait(state).unwrap();
        }
        if !state.2 {
            state.2 = true;
            let worker = self.clone();
            if let Err(e) = std::thread::Builder::new().name("kirikae-h264-worker".into()).spawn(move || worker.run()) {
                state.2 = false;
                return Err(format!("NVDEC: cannot start H.264 worker: {e}"));
            }
        }
        let _ = state.0.push(item);
        self.changed.notify_all();
        Ok(completion)
    }

    fn callback(self: &Arc<Self>, callback: impl FnOnce() -> Result<(), String> + Send + 'static) -> Result<H264Completion, String> {
        self.enqueue(H264Work::Callback(Box::new(callback)))
    }

    fn drain(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.0.is_empty() || state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn run(self: Arc<Self>) {
        let mut decoders = HashMap::new();
        loop {
            let item = {
                let mut state = self.state.lock().unwrap();
                while state.0.is_empty() {
                    state.1 = false;
                    self.changed.notify_all();
                    state = self.changed.wait(state).unwrap();
                }
                state.1 = true;
                let item = state.0.pop().unwrap();
                self.changed.notify_all();
                item
            };
            let H264Item { work, done } = item;
            let result = match work {
                H264Work::Decode(job) => decode_h264_job(&mut decoders, job),
                H264Work::Callback(callback) => callback(),
            };
            let (finished, changed) = &*done;
            *finished.lock().unwrap() = Some(result);
            changed.notify_all();
            let mut state = self.state.lock().unwrap();
            state.1 = false;
            self.changed.notify_all();
        }
    }
}

fn decode_h264_job(decoders: &mut HashMap<u32, h264::Decoder>, job: H264Job) -> Result<(), String> {
    let H264Job { process, iova, context, info, slices, output_surface, luma, chroma } = job;
    if let Some(dir) = std::env::var_os("KIRIKAE_VIDEO_DUMP") {
        let path = std::path::Path::new(&dir).join(format!("nvdec-{context}.h264"));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write;
            let _ = f.write_all(&h264::parameter_sets(&info));
            let _ = f.write_all(&slices);
        }
    }
    let decoder = match decoders.entry(context) {
        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
        std::collections::hash_map::Entry::Vacant(e) => e.insert(h264::Decoder::new()?),
    };
    let frames = decoder.decode_surface(&info, &slices, (output_surface, luma, chroma))?;
    if trace() {
        eprintln!(
            "nvdec: async H.264 channel {context}, {} bytes, surface {output_surface} -> {}",
            info.bitstream_size,
            if !frames.is_empty() { "decoded" } else { "no output" },
        );
    }
    let iova = iova.lock().unwrap();
    let dma = Dma { iova: &iova, mem: &process.memory, gpu: &process.gpu, later: None, h264: None };
    if frames.is_empty() {
        let w = (info.width_mbs.max(1) as usize) * 16;
        let h = (info.height_mbs.max(1) as usize) * 16;
        let uv_w = w.div_ceil(2) * 2;
        let uv_h = h.div_ceil(2);
        let y = to_block_linear(&vec![16u8; w * h], w, w, h, 1);
        let uv = to_block_linear(&vec![128u8; uv_w * uv_h], uv_w, uv_w, uv_h, 1);
        let _ = dma.write(luma, &y) && dma.write(chroma, &uv);
        return Ok(());
    }
    for ((out_surface, out_luma, out_chroma), frame) in frames {
        let y = to_block_linear(&frame.y, frame.width, frame.width, frame.height, 1);
        let uv_width = frame.width.div_ceil(2) * 2;
        let uv = to_block_linear(&frame.uv, uv_width, uv_width, frame.height.div_ceil(2), 1);
        if !dma.write(out_luma, &y) || !dma.write(out_chroma, &uv) {
            return Err(format!("NVDEC: output surface {out_surface} is not mapped"));
        }
    }
    Ok(())
}

pub(crate) fn h264_async() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KIRIKAE_NVDEC_H264_ASYNC").is_ok_and(|v| v == "1"))
}

pub(crate) struct SubmitResult {
    pub(crate) errors: Vec<String>,
    pub(crate) h264: Vec<H264Completion>,
}

#[derive(Default)]
pub struct Host1x {
    iova: Arc<Mutex<Iova>>,
    engines: Mutex<Engines>,
    worker: Arc<Worker>,
    h264: Arc<H264Worker>,
}


impl Host1x {
    pub fn map_buffer(&self, handle: u32, m: NvMap) -> Option<u32> {
        self.iova.lock().unwrap().map(handle, m)
    }

    pub fn drain(&self) {
        self.worker.drain();
        self.h264.drain();
    }

    pub(crate) fn submit(&self, p: &'static Process, context: u32, class: u32, words: &[u32]) -> SubmitResult {
        let timing = media_timing();
        let started = timing.then(std::time::Instant::now);
        let submitted_class = class;
        if class != CLASS_NVDEC {
            self.drain();
        }
        let drain_us = started.as_ref().map_or(0, |s| s.elapsed().as_micros());
        let (gpu, mem) = (&p.gpu, &p.memory);
        let iova = self.iova.lock().unwrap();
        let dma = Dma { iova: &iova, mem, gpu, later: Some((p, &self.iova, &self.worker)), h264: Some(self.h264.clone()) };
        let mut engines = self.engines.lock().unwrap();
        let mut errors = Vec::new();
        let mut h264 = Vec::new();
        let (mut class, mut offset, mut count, mut mask, mut incrementing) = (class, 0u32, 0u32, 0u32, false);
        for &w in words {
            let reg = if mask != 0 {
                let bit = mask.trailing_zeros();
                mask &= mask - 1;
                offset + bit
            } else if count != 0 {
                count -= 1;
                let r = offset;
                if incrementing {
                    offset += 1;
                }
                r
            } else {
                match w >> 28 {
                    0 => {
                        offset = (w >> 16) & 0xfff;
                        class = (w >> 6) & 0x3ff;
                        mask = w & 0x3f;
                    }
                    op @ (1 | 2) => {
                        offset = (w >> 16) & 0xfff;
                        count = w & 0xffff;
                        incrementing = op == 1;
                    }
                    3 => {
                        offset = (w >> 16) & 0xfff;
                        mask = w & 0xffff;
                    }
                    4 => {
                        let r = (w >> 16) & 0xfff;
                        engines.write(&dma, context, class, r, w & 0xfff, &mut errors, &mut h264);
                    }
                    op => errors.push(format!("host1x: opcode {op} is not supported")),
                }
                continue;
            };
            engines.write(&dma, context, class, reg, w, &mut errors, &mut h264);
        }
        if let Some(s) = started {
            let engine_us = s.elapsed().as_micros().saturating_sub(drain_us);
            if drain_us >= 1_000 || engine_us >= 1_000 {
                eprintln!(
                    "host1x: media timing t={} class {submitted_class:#x} context {context} words {} drain_us {drain_us} engine_us {engine_us} total_us {}",
                    crate::gpu::gpu_ticks(),
                    words.len(),
                    drain_us + engine_us,
                );
            }
        }
        SubmitResult { errors, h264 }
    }
}

impl Engines {
    fn write(&mut self, dma: &Dma, context: u32, class: u32, reg: u32, value: u32, errors: &mut Vec<String>, h264: &mut Vec<H264Completion>) {
        let method = match class {
            CLASS_NVDEC => &mut self.nvdec_method,
            CLASS_VIC => &mut self.vic_method,
            CLASS_HOST1X => return,
            _ => {
                errors.push(format!("host1x: engine class {class:#x} is not emulated"));
                return;
            }
        };
        match reg {
            THI_INCR_SYNCPT => {
                if h264.is_empty() {
                    dma.gpu.increment(value & 0xff, 1);
                } else if let Some((p, _, _)) = dma.later {
                    let Some(worker) = dma.h264.as_ref() else { return };
                    let id = value & 0xff;
                    let gpu = &p.gpu;
                    match worker.callback(move || {
                        gpu.increment(id, 1);
                        Ok(())
                    }) {
                        Ok(done) => h264.push(done),
                        Err(e) => errors.push(e),
                    }
                }
            }
            THI_METHOD0 => *method = value,
            THI_METHOD1 => {
                let m = *method;
                let r = match class {
                    CLASS_NVDEC => self.nvdec.write(dma, context, m, value),
                    _ => self.vic.write(dma, m, value).map(|()| None),
                };
                match r {
                    Ok(Some(done)) => h264.push(done),
                    Ok(None) => {},
                    Err(e) => errors.push(e),
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn bits(b: &[u8], lsb: usize, len: usize) -> u32 {
    let mut v = 0u64;
    for i in 0..8 {
        v |= (b.get(lsb / 8 + i).copied().unwrap_or(0) as u64) << (8 * i);
    }
    ((v >> (lsb % 8)) & ((1u64 << len) - 1)) as u32
}

pub(crate) fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4).map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

pub fn block_linear_size(width_bytes: usize, height: usize, block_height_log2: u32) -> usize {
    let rows = 8usize << block_height_log2;
    width_bytes.div_ceil(64) * 64 * height.div_ceil(rows) * rows
}

pub fn to_block_linear(src: &[u8], pitch: usize, width_bytes: usize, height: usize, block_height_log2: u32) -> Vec<u8> {
    let mut out = vec![0u8; block_linear_size(width_bytes, height, block_height_log2)];
    for y in 0..height {
        let row = &src[(y * pitch).min(src.len())..];
        for x in (0..width_bytes).step_by(16) {
            let n = 16.min(width_bytes - x).min(row.len().saturating_sub(x));
            if n == 0 {
                break;
            }
            let o = block_linear_offset(x as u64, y as u64, width_bytes as u64, block_height_log2) as usize;
            out[o..o + n].copy_from_slice(&row[x..x + n]);
        }
    }
    out
}

pub fn from_block_linear(raw: &[u8], width_bytes: usize, height: usize, block_height_log2: u32) -> Vec<u8> {
    let mut out = vec![0u8; width_bytes * height];
    for y in 0..height {
        for x in (0..width_bytes).step_by(16) {
            let n = 16.min(width_bytes - x);
            let o = block_linear_offset(x as u64, y as u64, width_bytes as u64, block_height_log2) as usize;
            if let Some(s) = raw.get(o..o + n) {
                out[y * width_bytes + x..y * width_bytes + x + n].copy_from_slice(s);
            }
        }
    }
    out
}

pub fn pitch(width_bytes: usize) -> usize {
    width_bytes.div_ceil(256) * 256
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_linear_round_trip() {
        let (w, h) = (200usize, 37usize);
        let src: Vec<u8> = (0..w * h).map(|i| (i * 7 + i / w) as u8).collect();
        for bh in 0..4 {
            let bl = to_block_linear(&src, w, w, h, bh);
            assert_eq!(bl.len(), block_linear_size(w, h, bh));
            assert_eq!(from_block_linear(&bl, w, h, bh), src);
        }
    }

    #[test]
    fn bit_fields() {
        let b = [0xffu8, 0x0f, 0, 0, 0, 0, 0, 0x80, 0x34, 0x12];
        assert_eq!(bits(&b, 0, 12), 0xfff);
        assert_eq!(bits(&b, 4, 8), 0xff);
        assert_eq!(bits(&b, 63, 1), 1);
        assert_eq!(bits(&b, 64, 16), 0x1234);
    }

    #[test]
    fn iova_follows_nvmap() {
        let mut io = Iova::default();
        let a = io.map(7, NvMap { size: 0x1800, addr: 0x8000_0000, ..NvMap::default() }).unwrap();
        let b = io.map(9, NvMap { size: 0x100, addr: 0x9000_0000, ..NvMap::default() }).unwrap();
        assert_eq!(io.map(7, NvMap { size: 0x1800, addr: 0x8000_0000, ..NvMap::default() }), Some(a));
        assert!(b as u64 >= a as u64 + 0x2000);
        assert_eq!(io.translate(a as u64 + 0x10, 0x10), Some(0x8000_0010));
        assert_eq!(io.translate(a as u64 + 0x1ff0, 0x20), None);
    }

    #[test]
    fn h264_queue_is_bounded_and_fifo() {
        let mut q = H264Queue::new(2);
        assert_eq!(q.push(1), Ok(()));
        assert_eq!(q.push(2), Ok(()));
        assert_eq!(q.push(3), Err(3));
        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.push(3), Ok(()));
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.pop(), Some(3));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn h264_worker_preserves_callback_order() {
        let worker = Arc::new(H264Worker::default());
        let order = Arc::new(Mutex::new(Vec::new()));
        let first_order = order.clone();
        let first = worker.callback(move || {
            first_order.lock().unwrap().push(1);
            Ok(())
        }).unwrap();
        let second_order = order.clone();
        let second = worker.callback(move || {
            second_order.lock().unwrap().push(2);
            Ok(())
        }).unwrap();
        first.wait().unwrap();
        second.wait().unwrap();
        assert_eq!(*order.lock().unwrap(), [1, 2]);
    }
}
