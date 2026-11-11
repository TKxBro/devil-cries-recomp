<div align="center">

# Devil Cries Recomp

**An unofficial static recompilation and native PC port of *Devil May Cry 3: Special Edition* (Switch build), powered by [Kirikae](https://github.com/Cor3Duo/kirikae).**

<br>

[![Platform: Linux x64 | Windows x64](https://img.shields.io/badge/platform-Linux%20x64%20%7C%20Windows%20x64-0078D6)](#system-requirements)
[![Graphics: Vulkan 1.3](https://img.shields.io/badge/Vulkan-1.3-AC162C?logo=vulkan)](#system-requirements)
[![Language: Rust 2024](https://img.shields.io/badge/rust-2024%20edition-orange?logo=rust)](#building-from-source)
[![Codegen: Cranelift](https://img.shields.io/badge/codegen-Cranelift-4b32c3)](#how-it-works)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

</div>

<br>

> [!CAUTION]
> **Legal & Copyright Notice:**
> **Devil Cries Recomp does NOT contain, distribute, download, or decrypt any copyrighted game files, ROMs, NSPs, encryption keys, or proprietary assets from Capcom or Nintendo.**
>
> This project consists solely of original static recompilation tooling, a clean-room Horizon HLE/Vulkan runtime, and the user-facing launcher. You **must provide your own legally dumped, decrypted copy** of *Devil May Cry 3 Special Edition* from your own Nintendo Switch console in order to compile the native PC executable locally on your machine. Compiled executables produced on your machine must not be redistributed. See [`LEGAL.md`](LEGAL.md).

---

## Table of Contents

- [About Devil Cries Recomp](#about-devil-cries-recomp)
- [Features & PC Enhancements](#features--pc-enhancements)
- [Roadmap & Current Status](#roadmap--current-status)
- [How Static Recompilation Works](#how-static-recompilation-works)
- [The Launcher (`devil-cries-recomp`)](#the-launcher-devil-cries-recomp)
- [System Requirements](#system-requirements)
- [Building from Source](#building-from-source)
- [Architecture & Repository Layout](#architecture--repository-layout)
- [Credits & Acknowledgments](#credits--acknowledgments)
- [License](#license)

---

## About Devil Cries Recomp

**Devil Cries Recomp** brings *Devil May Cry 3: Special Edition* to modern PCs as a true **native executable**—without dynamic CPU emulation overhead. By statically lifting AArch64 instructions into native x86_64 machine code via Cranelift and translating NVIDIA Maxwell GPU shaders directly into Vulkan SPIR-V, the game executes directly on host hardware with minimal latency and deep room for modern PC features.

### Why Static Recompilation Instead of Emulation?
Unlike traditional emulators that interpret or JIT-compile guest instructions dynamically at runtime alongside full virtualized system layers, static recompilation:
- **AOT-compiles the entire executable offline** into optimized native machine code objects.
- **Replaces Switch OS/Horizon kernel calls & GPU driver calls** with lightweight native Rust HLE shims (`kirikae-rt`) running on Vulkan, CPAL (PipeWire/ALSA/WASAPI), and Gilrs.
- **Enables authentic PC enhancements**: custom internal rendering resolutions, true 21:9 ultrawide FOV/aspect adjustments, high frame rate decoupling, and native filesystem save/config management.

---

## Features & PC Enhancements

### Implemented & Working
- **Custom Native Launcher (`devil-cries-recomp`)**:
  - Clean Rust (`egui`/`eframe`) GUI with DMC-inspired crimson/dark aesthetic.
  - One-click dump selection, verification, offline compilation progress tracker, persistent settings, and direct game launch.
  - Automatic management of isolated save directories and configuration files (`~/.local/share/DevilCriesRecomp/`).
- **High-Performance H.264 Cutscene Decoder (`FFmpeg` / `host1x`)**:
  - Multi-threaded slice-based FFmpeg NVDEC H.264 decoding replacing blocking synchronous frames, dramatically reducing stutter/hitches and restoring audio/video synchronization in FMVs and transitions.
- **Horizon Audio Synchronization (`audio/out.rs`)**:
  - Corrected audio buffer ring timing and queueing behavior for distortion-free, crackle-free playback.
- **Vulkan Graphics & Maxwell Shader Translator Fixes**:
  - **Strict Format Class Tracking**: Prevents incompatible compressed texture reuse (e.g., BC3/RGBA mismatches) fixing corrupted loading screens and equipment menus.
  - **Constant Buffer Jump Table Bounds (`ldc` / `imnmx`)**: Properly tracks jump-table constant buffer sizes and performs safe partial mapped reads, fixing broken indirect shader branches (`brx`).

### Actively In Progress / Roadmap
- [ ] **Render Resolution Scaling**: True internal 3D resolution multipliers (**1080p**, **2K / 1440p**, **3K / 1800p**, **4K / 2160p**) across framebuffers, viewports, and scissors.
- [ ] **Aspect Ratio Support (16:9 & Native 21:9 Ultrawide)**:
  - Unstretched **Native 21:9 Ultrawide** 3D camera expansion.
  - Toggleable HUD anchoring: keep gauges/UI pinned to **Outer Screen Edges (21:9)** or **Centered inside 16:9 Safe Area**.
- [ ] **High Frame Rate Modes (60 / 120 / 240 FPS)**:
  - Instrumentation and decoupling of frame-driven physics, combat timings, i-frames, and animation speed from VI display pacing.
- [ ] **Input & Customization**: Full keyboard/mouse bindings, controller hot-plugging, and automatic Xbox/PlayStation button prompts.
- [ ] **Windows x64 Release Parity**: Dedicated cross-tested Windows builds and releases.

---

## Roadmap & Current Status

Detailed engineering milestones and test verification criteria are tracked in:
- [`docs/plans/2026-10-29-dmc3-pc-recomp-roadmap.md`](docs/plans/2026-10-29-dmc3-pc-recomp-roadmap.md)
- [`docs/acceptance-first-pass.md`](docs/acceptance-first-pass.md)
- [`docs/ffmpeg-h264-experiment.md`](docs/ffmpeg-h264-experiment.md)

| Area | Status | Notes |
|---|---|---|
| Boot, Title Screen & Menus | Working | Clean rendering, textures, and menu navigation |
| Story Cutscenes (FMV / H.264) | Working | FFmpeg slice-threading active; clean audio sync |
| Gameplay & Combat (Mission 1+) | Working | Playable at native 60 FPS; lighting/post-fx polish ongoing |
| Save System & Persistence | Working | Persistent saves via `KIRIKAE_SAVES` |
| Dedicated PC Launcher | Working | Native setup, settings, and launch dashboard |
| Resolution Scaling (2K / 3K / 4K) | Next Milestone | In active development |
| Native 21:9 Ultrawide + HUD Toggle | Next Milestone | In active development |
| 120 / 240 FPS Decoupling | Planned | Requires timing/delta decoupling verification |

---

## How Static Recompilation Works

```
  [Your Legal Dump (.nsp / ExeFS + RomFS)]
                    │
                    ▼
           [ kirikae-loader ] ── Parses NPDM, NSOs, dynamic symbols, RomFS offsets
                    │
                    ▼
           [ kirikae-a64 ]    ── Lifts AArch64 machine code into clean IR (crates/a64/src/ir.rs)
                    │
                    ▼
           [ kirikae-recomp ] ── Discovers functions, emits parallel object files via Cranelift
                    │
                    ▼
       [ cargo / Linker ] ───── Links native x86_64 objects + kirikae-rt (Vulkan + FFmpeg + Audio + Input)
                    │
                    ▼
        [ Native PC Executable ]
```

* **Shader Pipeline (`kirikae-maxwell`)**: Translates NVIDIA Tegra X1 / Maxwell shader bytecode on the fly into optimized SPIR-V shaders cached locally for Vulkan 1.3 pipelines.
* **Memory Safety**: Reserves a protected virtual address window up front so every guest pointer is safely masked—guest faults cannot corrupt host application memory.
* **Interpreter Fallback + Profiling**: Any indirect jump/vtable unreachable during static analysis executes seamlessly through the integrated interpreter (`kirikae-a64`) and logs `.profile` entries so subsequent rebuilds compile those functions natively as well.

---

## The Launcher (`devil-cries-recomp`)

The included **Devil Cries Recomp Launcher** provides the complete end-user workflow:
1. **First-Time Setup**: Select your decrypted *Devil May Cry 3 Special Edition* game file or extracted ExeFS/RomFS directory. Click **Compilar Jogo para PC Nativo** to generate the optimized native binary locally.
2. **Play Dashboard**: Once built, launch directly with **Iniciar Devil May Cry 3**, open your persistent saves directory, or rebuild when updating the codebase.
3. **Settings**: Toggle fullscreen, FPS statistics, and FFmpeg video decoder threading parameters without touching CLI environment variables.

---

## System Requirements

* **OS**: Linux x86_64 (Wayland / X11) *(Windows x64 support via upstream toolchain, DMC3 validation pending)*
* **GPU**: Vulkan 1.3 compatible graphics card (NVIDIA, AMD, or Intel)
* **Dependencies (for building from source)**:
  * Rust Toolchain (2024 edition, `cargo` / `rustc`)
  * Vulkan loader & headers
  * FFmpeg development libraries (`libavcodec`, `libavutil`, `libswscale`) for enhanced H.264 cutscene decoding
  * GTK/XDG portal utilities (for native file picker dialogs on Linux)

---

## Building from Source

### 1. Clone the repository
```bash
git clone <repository-url>
cd devil-cries-recomp
```

### 2. Build the Launcher and Recompiler CLI
> **Note:** On machines with 16 GB RAM or less, pass `-j 2` or `-j 4` to Cargo so parallel Cranelift/LLVM codegen does not exhaust system memory.

```bash
# Build the native Launcher
cargo build --release -p devil-cries-recomp -j 2

# Build the Kirikae CLI builder with FFmpeg H.264 support
cargo build --release -p kirikae --features 'kirikae-rt/ffmpeg-h264' -j 2
```

### 3. Run the Launcher
```bash
./target/release/devil-cries-recomp
```
Point the Launcher to your dumped copy of *Devil May Cry 3 Special Edition* and follow the on-screen compilation setup!

*(Alternatively, compile manually via CLI):*
```bash
KIRIKAE_FFMPEG_H264=1 ./target/release/kirikae build /path/to/dmc3-dump -o ./build --name base -j 4
```

---

## Architecture & Repository Layout

```
crates/
├── a64/       # AArch64 decoder, single-source IR, semantics, and interpreter fallback
├── loader/    # NRO, NSO, ExeFS, RomFS, and package parsing
├── maxwell/   # NVIDIA Maxwell GPU bytecode decoder, CFG analysis, and SPIR-V translator
├── recomp/    # Function discovery, Cranelift/Rust object codegen, and project builder
├── rt/        # Host runtime: Horizon HLE services, guest memory, Vulkan renderer, FFmpeg NVDEC, CPAL audio, input
├── kirikae/   # CLI recompiler frontend
├── gui/       # Upstream minimal GUI frontend
└── launcher/  # Devil Cries Recomp custom DMC3 PC launcher (egui / eframe)
```

---

## Credits & Acknowledgments

- **[Cor3Duo / Kirikae](https://github.com/Cor3Duo/kirikae)**: Core static recompiler, AArch64 lifter, Horizon HLE runtime, and Maxwell-to-SPIR-V translation architecture. (Original upstream documentation preserved in [`KIRIKAE_UPSTREAM.md`](KIRIKAE_UPSTREAM.md)).
- Inspired by the pioneering work of **[Zelda 64: Recompiled](https://github.com/Zelda64Recomp/Zelda64Recomp)**, **N64Recomp**, and **HarbourMasters (Ship of Harkinian / 2Ship2Harkinian)** in establishing legal, user-supplied asset recompilation workflows.
- Clean-room technical references: Switchbrew, Ryujinx (MIT), envytools (MIT), NVIDIA Open GPU documentation, and the ARM Architecture Reference Manual.

---

## License

Code in this workspace is dual-licensed under **MIT** or **Apache-2.0** (see [`LEGAL.md`](LEGAL.md)). No GPL code or copyrighted game assets are included.
