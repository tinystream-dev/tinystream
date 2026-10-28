# Desktop player requirement audit (#86)

The issue body and `web/src/player/Player.tsx` define the scope. Implementation and
hardware acceptance are tracked separately. No result from an AMD fixture is evidence
of NVIDIA compatibility. Public sample-remux measurements are qualified below.

| Requirement | Implementation / evidence | Status |
| --- | --- | --- |
| One mpv instance; enforced options; optional mpv.conf; Advanced path | player/engine.rs; configuration override test | Implemented and tested |
| Streaming cache and default debanding | player/engine.rs; enforced-options test | Implemented |
| Rust HTTP protocol, bearer authentication, Range read-ahead, seek/cancel | player/stream.rs; HTTP boundary, interruption and cancellation tests | Implemented and tested |
| Direct file and room endpoint capability | Source::path and stream opener accept server-relative media bases, including api/together/{code}/media/{id}; room synchronization is #88 | Implemented |
| Manual quality; absolute timestamps; cached and uncached transcode seeks | player/mod.rs, engine.rs; HTTP-backed hardware test drives Player operations | Implemented and tested |
| Direct audio selection and transcode rebuild | player/mod.rs, engine.rs; hardware HTTP test | Implemented and tested |
| Web error text; bounded recovery; metadata retry | player/stream.rs, mod.rs, view.rs; interruption and early-EOF tests | Implemented |
| VideoSurface; matching GPU UUIDs; EGL mpv rendering | platform/linux/video.rs | Implemented; AMD tested |
| Three Vulkan images imported into GL, no frame copies | platform/linux/video.rs; Vulkan pixel readback test | Implemented; AMD tested |
| Frame synchronization; physical-size letterboxing; report_swap; resize | platform/linux/video.rs, ui/video.rs; resized pixel readback | Implemented; AMD tested |
| Auto-hiding chrome, top bar and window controls | player/view.rs, chrome.rs, shell.rs; control colors and materials follow player fade | Implemented |
| Chapters, scrub previews, volume, play/pause, +/-10s buttons | player/view.rs, timeline.rs; rendered controls and menu input test | Implemented |
| Track names and per-user remembered choices | player/tracks.rs; naming and preference tests | Implemented and tested |
| Subtitle selection and bitmap playback | player/tracks.rs, mod.rs; text overlay belongs to #87 | Implemented |
| Quality and speed menus | player/view.rs; rendered menu test | Implemented |
| Skip pills; UpNext/autoplay; Finale; Resting; HUD | player/view.rs, mod.rs; rendered overlays and early-EOF guard | Implemented |
| GPU Ambilight | ui/video.rs, video.wgsl | Implemented; rendered |
| Screenshot card and clip editor handoff | player/clip.rs; server recipe submission, stale-result and progress-restoration tests | Implemented and tested |
| Resume/startAt; progress cadence; watched threshold; exit saves | player/mod.rs, runner.rs; pause-save and clip-preview isolation tests | Implemented |
| Fullscreen F/double-click; P PiP; X clip; web shortcuts | player/mod.rs, view.rs, runner.rs; native fullscreen/PiP lifecycle | Implemented; Wayland tested |
| Picture in picture | secondary native window with the same GPU device, mpv and Screen; close/return/navigation tests; niri floating placement verified | Implemented and tested |
| Idle-inhibition hook for #92 | Desktop::video_activity; playback transition test, including PiP navigation | Implemented and tested |
| AMD 1080p 10-bit AV1 and 4K HEVC direct play | generated fixtures, VA-API and Vulkan pixel readback; HTTP seeks/audio/720p round trip | Passed generated and public high-bitrate sample-remux checks |
| CPU/GPU comparable to standalone mpv on the same remux | retained video drawing; 4K60/59-Mbps sample remux: app 6.89% CPU/50.76% GPU, mpv 8.78%/53.82%; methodology in player-validation.md | Comparable on tested sample |
| NVIDIA 10-bit AV1 NVDEC from a CI build | David confirmed successful NVIDIA playback and accepted the tester result on 2026-10-28 | Accepted; tester-reported |

`cargo test --locked`: 71 passing tests (5 mpv integration tests and 66 app tests),
with hardware/native checks opt-in. Strict all-target clippy and formatting pass.
The native PiP test also runs against real HTTP-backed AV1 and verifies decoding,
floating placement on niri, window close/return, navigation and fullscreen.

The neighboring milestones remain #87 text subtitle rendering, #88 room synchronization,
#92 OS inhibition, and #93 the full Clips screen/sharing. The #86 clip handoff and
activity hook are present; these are not deferred #86 requirements.
