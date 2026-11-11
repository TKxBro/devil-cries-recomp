# DMC3 native PC recompilation: staged roadmap

**Status:** proposed plan, not implemented. This is a local working document; do not distribute generated executables, profiles, saves, game assets, keys, screenshots or dumps. Keep original NSPs untouched, separate base and update, and preserve the first working executable.

**Goal:** a reliable, finishable Linux PC build from the owner's Switch copy, followed by a user-friendly PC edition and an independently validated Windows x64 build. The recompilation translates original code but retains a host runtime that supplies Switch services; PC enhancements are separate work, not automatic consequences of recompilation.

**Stack:** Kirikae static recompilation, Rust host runtime, Vulkan presentation, H.264 decoder. Modify only the isolated experimental checkout until a specific change is tested and promoted deliberately. Do not import GPL yuzu/suyu code into Kirikae; use compatible clean-room references.

## 0. Stabilize the experiment and define evidence

- Freeze the current audio7 correction and working FFmpeg A/B build; do not declare Slice or Frame the winner from one subjective comparison. Preserve both launchers/modes and the stable original build.
- Split experimental changes into reviewable patches: audio buffer, H.264 backend, decoder threading, instrumentation, and any unrelated rendering changes. Identify which are necessary before promoting any into a clean candidate.
- Establish a repeatable route and save for (a) title/menu, (b) one cutscene, (c) one combat encounter, (d) Guns/Equip/Devil Arms, (e) pause/save/reload/exit. Keep the same profile, display, route, game version, cache condition and audio settings in A/B tests. Clean runs must not trace or dump per-frame data.
- Record for each run: executable/commit, flags, route, observed correctness, audio, FPS/frame-time tail and longest hitches, Vulkan/presentation stats and interpreted instructions. Tool measurements and player reports are different evidence.
- **Gate:** reproducible checklist and known-good rollback exist. No further decoder work without a reproducible visible or audible defect.

## 1. Finish a faithful, completable base game (first release candidate)

1. Walk through gameplay and cutscenes in sections, including menus, combat, equipment, mission changes, loading, saves and a clean exit. Record blockers with reproducible steps and severity; do not make the player redo the whole game for each test.
2. Repair correctness before throughput: visible equipment/artifact defects, wrong surfaces or synchronization, broken input and saves, audio drift, cutscene transitions and crashes. Separate each fix and its regression test.
3. For the chosen decoder, verify packet/frame association, delayed-frame drain, reset, surface reuse and ordered NVDEC→VIC/fence behavior. Only rework async media if a remaining reproducible problem justifies it; do not revive the rejected partial async prototype.
4. Keep the 60 Hz guest behavior intact. A default 60 FPS output is acceptable for a faithful port; `KIRIKAE_NO_FRAME_LIMIT=1` is not a high-FPS feature because it can speed up frame-driven gameplay.
5. **Gate:** the base game can be completed on Linux with no progression blockers, repeatable menu/gameplay/cutscene/audio/save/input checks, and no high-severity visual or stability defects. Performance claims require paired clean runs; no fixed numerical threshold is claimed before a measured baseline.

## 2. Build the PC usability layer

- Windowed/fullscreen/borderless, selectable output resolution, aspect-safe scaling and correct presentation on resize and high-DPI displays. Start at `crates/rt/src/display.rs`, `services/vi.rs` and `gpu/vulkan/present.rs`; distinguish the 1280×720 guest surface from the host window. Do not claim higher internal rendering resolution from a larger swapchain alone.
- Rebindable keyboard/gamepad controls, sensible defaults, controller hot-plug and accessibility of menus. Start at `crates/rt/src/input.rs`; test gamepad and keyboard paths separately. Mouse support is valuable where the game's controls meaningfully use it, not a cosmetic checkbox.
- A small persistent settings UI: video, audio, controls and a safe reset-to-default button. Keep platform and game-facing settings distinct, reject invalid config, and never let an enhancement block the original mode.
- **Gate:** fresh install can be configured and played without environment variables or shell scripts, with settings saved and restored; default settings preserve the faithful 60 Hz path.

## 3. Add optional enhancements one at a time

- **Higher internal resolution:** trace the render targets, viewports, post effects and UI coordinates; raise resolution behind a toggle only after gameplay and cutscene output remain correct. Output upscaling and true internal resolution are distinct.
- **Widescreen/ultrawide:** extend the 3D view with correct projection and culling; anchor HUD, menus and subtitles deliberately. Pre-rendered video may remain at its original aspect ratio with bars. Test 16:9, 21:9 and the original presentation; no stretching or unintentional reveal.
- **High refresh:** instrument whether simulation, animation, input, physics, particles and timing are frame-bound. Decouple or interpolate rendering without changing game speed; validate 60 first, then 120, then higher modes only if they pass gameplay and cutscene regression. Avoid equating repeated/interpolated frames with true simulation at 120/240 Hz.
- **Upscaling:** first ship reliable native/output scaling and frame pacing. Consider FSR or another optional spatial/temporal method only when measured to help; temporal methods need motion/depth/jitter/history integration and artifact testing. DLSS is not a prerequisite for a PC port and should not be a launch gate.
- **Gate per option:** reversible toggle, matched before/after captures, no progression change, acceptable frame-time distribution, correct UI/video, and a tested fallback to original behavior.

## 4. Base update, Windows, packaging and public quality

- Handle the Switch 1.0.1 update separately after the base is stable. Verify title/build IDs, patch layering, new profiles and regressions against the base; do not overwrite the base dump or claim update support from a successful build alone.
- Produce and play-test a distinct Windows x64 build; check Vulkan, input, audio, paths, saves and all acceptance routes. Linux success is not Windows validation.
- Create reproducible build instructions, versioned local patches, issue templates and a smoke/regression suite with synthetic test data. Continuous integration can test the runtime and legal synthetic fixtures, not distribute generated game executables or game data.
- Release only legal tooling/patches/instructions with owner-provided game data kept local, subject to legal review; the local Kirikae `LEGAL.md` explicitly forbids distributing its generated executable, profiles, saves and captures.
- **Final gate:** finishable and stable on each claimed platform, PC usability defaults work without flags, enhancement modes are honestly labeled, and no proprietary data leaves the owner's machine.

## Immediate next action

Do **one bounded acceptance pass of the base build** rather than another decoder experiment: menu → short combat → Guns/Equip/Devil Arms → save/reload → one cutscene → exit. Record each defect and a short route. Fix the highest-severity reproducible blocker before adding PC enhancements. Slice/Frame remains an A/B candidate, not a final selection.

## References

- Local `README.md`, `LEGAL.md`, `crates/rt/src/services/vi.rs`, `crates/rt/src/display.rs`, `crates/rt/src/gpu/vulkan/present.rs`, `crates/rt/src/input.rs` (checked 2026-10-29).
- [Zelda 64: Recompiled](https://github.com/Zelda64Recomp/Zelda64Recomp): original-data-on-user-machine, high framerate, widescreen, settings and controls as separately implemented features, not automatic recompilation benefits.
- [2 Ship 2 Harkinian](https://github.com/HarbourMasters/2ship2harkinian): supported owner-provided dump, platform releases, controls/settings and documentation.
- [Kirikae legal rules](https://github.com/Cor3Duo/kirikae/blob/main/LEGAL.md): no generated game binary or game data in the public repository.
