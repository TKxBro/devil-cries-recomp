use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::output;
use crate::cpu::Cpu;
use crate::process::Process;
use crate::services::stub::unimplemented;
use crate::services::{Reply, Request, Service};

const DEVICE_NAME: &[u8] = b"DeviceOut";
const NAME_SIZE: u64 = 0x100;
const SAMPLE_RATE: u32 = 48_000;
const PCM_INT16: u32 = 2;
const STARTED: u32 = 0;
const STOPPED: u32 = 1;
const BUFFER_INFO: u64 = 0x28;

static NEXT_AUDIO_OUT_ID: AtomicU64 = AtomicU64::new(1);
static AUDIO_DUMP: OnceLock<Mutex<Option<output::WavDump>>> = OnceLock::new();

fn write_name(c: &Cpu, req: &Request) {
    if let Some(out) = req.out_buf(0) {
        let mut name = vec![0u8; out.size.min(NAME_SIZE) as usize];
        let n = DEVICE_NAME.len().min(name.len().saturating_sub(1));
        name[..n].copy_from_slice(&DEVICE_NAME[..n]);
        c.process().memory.write(out.addr, &name);
    }
}

pub struct AudioOutManager;

impl Service for AudioOutManager {
    fn name(&self) -> String {
        "audout:u".into()
    }

    fn call(&self, c: &mut Cpu, req: &Request) -> Reply {
        match req.cmd {
            0 | 2 => {
                write_name(c, req);
                Reply::ok().u32(1)
            }
            1 | 3 => {
                write_name(c, req);
                let channels = if req.u32(4) & 0xffff > 2 { 6 } else { 2 };
                let p = c.process();
                let id = NEXT_AUDIO_OUT_ID.fetch_add(1, Ordering::Relaxed);
                if std::env::var_os("KIRIKAE_TRACE_AUDIO_OUT").is_some() {
                    eprintln!("audio-out: object #{id} opened, {SAMPLE_RATE} Hz, {channels} channels, PCM_INT16");
                }
                let out = Arc::new(AudioOut { id, event: p.create_event(false), channels, state: Mutex::new(State::default()) });
                start_playback(p, out.clone());
                Reply::ok().u32(SAMPLE_RATE).u32(channels).u32(PCM_INT16).u32(STOPPED).object(out)
            }
            _ => unimplemented("audout:u", c, req),
        }
    }
}

#[derive(Default)]
struct State {
    started: bool,
    queued: VecDeque<Buffer>,
    released: VecDeque<u64>,
    played: u64,
    volume: f32,
    trace_appends: u64,
    trace_plays: u64,
}

#[derive(Clone, Copy)]
struct Buffer {
    tag: u64,
    samples: u64,
    size: u64,
}

struct AudioOut {
    id: u64,
    event: u32,
    channels: u32,
    state: Mutex<State>,
}

impl Service for AudioOut {
    fn name(&self) -> String {
        "audout:u/IAudioOut".into()
    }

    fn call(&self, c: &mut Cpu, req: &Request) -> Reply {
        let p = c.process();
        let mut s = self.state.lock().unwrap();
        match req.cmd {
            0 => Reply::ok().u32(if s.started { STARTED } else { STOPPED }),
            1 => {
                s.started = true;
                if std::env::var_os("KIRIKAE_TRACE_AUDIO_OUT").is_some() {
                    eprintln!("audio-out: object #{} Start, queued {}", self.id, s.queued.len());
                }
                Reply::ok()
            }
            2 => {
                s.started = false;
                if std::env::var_os("KIRIKAE_TRACE_AUDIO_OUT").is_some() {
                    eprintln!("audio-out: object #{} Stop, queued {}", self.id, s.queued.len());
                }
                Reply::ok()
            }
            3 | 7 => {
                let Some(info) = req.in_buf(0).and_then(|b| p.memory.read(b.addr, b.size.min(BUFFER_INFO))) else { return Reply::ok() };
                let word = |i: usize| info.get(i * 8..i * 8 + 8).map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()));
                let buffer = Buffer { tag: req.u64(0), samples: word(1), size: word(3) };
                if std::env::var_os("KIRIKAE_TRACE_AUDIO_OUT").is_some() {
                    let n = s.trace_appends;
                    s.trace_appends += 1;
                    if n < 16 || (n + 1).is_multiple_of(500) {
                        eprintln!("audio-out: object #{} append #{} cmd {} tag {:#x} samples {:#x} size {:#x} queued {}", self.id, n + 1, req.cmd, buffer.tag, buffer.samples, buffer.size, s.queued.len());
                    }
                }
                s.queued.push_back(buffer);
                Reply::ok()
            }
            4 => Reply::ok().copy_handle(self.event),
            5 | 8 => {
                let Some(out) = req.out_buf(0) else { return Reply::ok().u32(0) };
                let n = ((out.size / 8) as usize).min(s.released.len());
                let mut tags: Vec<u8> = s.released.drain(..n).flat_map(|t| t.to_le_bytes()).collect();
                tags.resize(out.size as usize, 0);
                p.memory.write(out.addr, &tags);
                Reply::ok().u32(n as u32)
            }
            6 => {
                let tag = req.u64(0);
                Reply::ok().u32(s.queued.iter().any(|b| b.tag == tag) as u32)
            }
            9 => Reply::ok().u32(s.queued.len() as u32),
            10 => Reply::ok().u64(s.played),
            11 => {
                let flushed: Vec<u64> = s.queued.drain(..).map(|b| b.tag).collect();
                let any = !flushed.is_empty();
                if std::env::var_os("KIRIKAE_TRACE_AUDIO_OUT").is_some() {
                    eprintln!("audio-out: object #{} flushed {} buffers", self.id, flushed.len());
                }
                s.released.extend(flushed);
                drop(s);
                if any {
                    p.set_event(self.event, true);
                }
                Reply::ok().u32(any as u32)
            }
            12 => {
                s.volume = f32::from_bits(req.u32(0));
                if std::env::var_os("KIRIKAE_TRACE_AUDIO_OUT").is_some() {
                    eprintln!("audio-out: object #{} volume {}", self.id, s.volume);
                }
                Reply::ok()
            }
            13 => Reply::ok().u32(s.volume.to_bits()),
            _ => unimplemented("audout:u/IAudioOut", c, req),
        }
    }
}

fn start_playback(p: &'static Process, out: Arc<AudioOut>) {
    let _ = std::thread::Builder::new().name("kirikae-audio-out-service".into()).spawn(move || {
        let id = out.id;
        out.state.lock().unwrap().volume = 1.0;
        let trace = std::env::var_os("KIRIKAE_TRACE_AUDIO_OUT").is_some();
        let mut output: Option<output::Output> = None;
        let dump_path = std::env::var_os("KIRIKAE_AUDIO_DUMP");
        let object_dump_dir = std::env::var_os("KIRIKAE_AUDIO_DUMP_OBJECTS");
        let mut object_dump: Option<output::WavDump> = None;
        let media_timing = crate::host1x::media_timing();
        let mut queue_empty = false;
        let mut last_dry = 0u64;
        let channels = out.channels as usize;
        let mut next = Instant::now();
        let mut frames: Vec<[f32; 2]> = Vec::new();
        loop {
            if p.sched.exiting.load(Ordering::Relaxed) {
                return;
            }
            let started = out.state.lock().unwrap().started;
            if !started {
                if output.take().is_some() && trace {
                    eprintln!("audio-out: object #{} host stream closed after Stop", id);
                }
                std::thread::sleep(Duration::from_millis(2));
                next = Instant::now();
                continue;
            }
            if output.is_none() && !std::env::var_os("KIRIKAE_MUTE").is_some_and(|v| v != "0") {
                output = match output::Output::open(SAMPLE_RATE) {
                    Ok(o) => {
                        if trace { eprintln!("audio-out: object #{} host stream open on {}", id, o.device); }
                        Some(o)
                    }
                    Err(e) => {
                        eprintln!("kirikae: audio output: {e}");
                        None
                    }
                };
            }
            let buffer = out.state.lock().unwrap().queued.front().copied();
            let Some(b) = buffer else {
                if media_timing && !queue_empty {
                    eprintln!(
                        "audio-out: media timing t={} object #{} guest queue empty played {}",
                        crate::gpu::gpu_ticks(),
                        id,
                        out.state.lock().unwrap().played,
                    );
                }
                queue_empty = true;
                std::thread::sleep(Duration::from_millis(2));
                next = Instant::now();
                continue;
            };
            queue_empty = false;
            let pcm = p.memory.read(b.samples, b.size).unwrap_or_default();
            let (volume, play_no) = {
                let mut s = out.state.lock().unwrap();
                let n = s.trace_plays;
                s.trace_plays += 1;
                (s.volume, n + 1)
            };
            frames.clear();
            frames.extend(pcm.chunks_exact(2 * channels).map(|f| {
                let s = |i: usize| i16::from_le_bytes([f[2 * i], f[2 * i + 1]]) as f32 / 32768.0 * volume;
                [s(0), s(1)]
            }));
            if trace && (play_no <= 16 || play_no.is_multiple_of(500)) {
                let nonzero = pcm.iter().filter(|&&v| v != 0).count();
                let peak = pcm.iter().map(|&v| v.abs_diff(0)).max().unwrap_or(0);
                eprintln!("audio-out: object #{} play #{} tag {:#x} bytes {:#x} pcm {} bytes -> {} frames channels {} volume {} nonzero {}/{} peak {:#x}", id, play_no, b.tag, b.size, pcm.len(), frames.len(), channels, volume, nonzero, pcm.len(), peak);
            }
            if let Some(o) = &output {
                o.push(&frames);
                if media_timing {
                    let dry = o.dry_frames();
                    if dry > last_dry {
                        eprintln!(
                            "audio-out: media timing t={} object #{} host dry +{} total {} queued {}",
                            crate::gpu::gpu_ticks(),
                            id,
                            dry - last_dry,
                            dry,
                            o.queued(),
                        );
                        last_dry = dry;
                    }
                }
            }
            if let Some(path) = dump_path.as_ref() {
                let slot = AUDIO_DUMP.get_or_init(|| {
                    let dump = match output::WavDump::create(path.as_ref(), SAMPLE_RATE) {
                        Ok(d) => Some(d),
                        Err(e) => {
                            eprintln!("kirikae: audio: cannot write {}: {e}", path.to_string_lossy());
                            None
                        }
                    };
                    Mutex::new(dump)
                });
                let mut guard = slot.lock().unwrap();
                if let Some(d) = guard.as_mut() {
                    if d.write(&frames).is_err() {
                        *guard = None;
                    }
                }
            }
            if let Some(dir) = object_dump_dir.as_ref() {
                if object_dump.is_none() {
                    let path = std::path::PathBuf::from(dir).join(format!("object-{id}.wav"));
                    object_dump = match output::WavDump::create(&path, SAMPLE_RATE) {
                        Ok(d) => {
                            if trace { eprintln!("audio-out: object #{} dump opened at {}", id, path.display()); }
                            Some(d)
                        }
                        Err(e) => {
                            eprintln!("kirikae: audio: cannot write {}: {e}", path.display());
                            None
                        }
                    };
                }
                if let Some(d) = object_dump.as_mut() {
                    if d.write(&frames).is_err() {
                        eprintln!("kirikae: audio: object #{} dump write failed", id);
                        object_dump = None;
                    }
                }
            }
            next += Duration::from_secs_f64(frames.len() as f64 / f64::from(SAMPLE_RATE));
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
            let mut s = out.state.lock().unwrap();
            if s.queued.front().is_some_and(|q| q.tag == b.tag) {
                s.queued.pop_front();
                s.released.push_back(b.tag);
                s.played += frames.len() as u64;
            }
            drop(s);
            p.set_event(out.event, true);
        }
    });
}
