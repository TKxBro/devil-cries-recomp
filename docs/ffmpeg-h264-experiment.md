# Experimental FFmpeg H.264 backend

This candidate is isolated to the no-flush experiment. The default backend remains OpenH264.

Build the runtime and the `kirikae` builder with the optional backend:

```bash
cargo build --release -p kirikae --features 'kirikae-rt/ffmpeg-h264'
```

When generating a new recompiled executable, pass `KIRIKAE_FFMPEG_H264=1` so the generated `base-build` also enables the runtime feature:

```bash
KIRIKAE_FFMPEG_H264=1 ./target/release/kirikae build \
  '/mnt/SSD_Jogos/Port DMC3?/work/dump/base' \
  -o out-h264-ffmpeg-audio7 --name base \
  --profile out-base-no-flush-audio5/base.profile
```

Select it only for an experimental run:

```bash
KIRIKAE_H264_BACKEND=ffmpeg \
KIRIKAE_FFMPEG_THREADS=4 \
KIRIKAE_FFMPEG_QUEUE=4 \
./run-h264-ffmpeg-audio7.sh
```

`KIRIKAE_H264_BACKEND` must be `ffmpeg`; otherwise OpenH264 remains active. The FFmpeg decoder receives until `EAGAIN`, keeps a bounded pending-frame queue, and requires YUV420P. `KIRIKAE_FFMPEG_DIAGNOSTICS=1` prints the requested and active thread configuration.

For a controlled threading experiment, `KIRIKAE_FFMPEG_THREAD_TYPE=slice` selects slice threading; the default is still `frame`. Both modes use the same executable and `KIRIKAE_FFMPEG_THREADS=4`. After building it into the separate `out-h264-ffmpeg-slice-audio7` directory, run `./run-h264-thread-ab-audio7.sh slice` or `./run-h264-thread-ab-audio7.sh frame` separately with the game closed between runs. The launcher enables audio and disables tracing/dumps; diagnostic output records the active FFmpeg thread mode. This does not change the NVDEC→VIC surface/queue policy or the audio7 fix. FFmpeg frame threading increases decode latency per thread; slice threading may remove that extra delay but may have different CPU cost. Verify output count/order, delayed frames, surfaces, audio, and the same cutscene visually before treating it as a fix.

This is not yet a verified game fix. Do not enable it in the stable build. Compare one clean audible cutscene at a time against `run-audio7-audible.sh`, with the game closed between runs.
