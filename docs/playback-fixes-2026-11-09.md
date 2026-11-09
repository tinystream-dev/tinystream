# Playback fixes — 2026-11-09

Implemented on `feat/desktop`, starting with the urgent episode-switching behavior, then following the screenshot from top to bottom. All changes are committed locally. No release builds, deployment, GitHub issue closure, or pushes were performed.

| Item | Change | Verification |
| --- | --- | --- |
| Urgent desktop Watch Together episode switching | Changing the room's episode atomically requests playback at position zero, even if the previous episode was paused. The room still waits for loading members. This is a server fix shared by all clients; use the updated server. Solo playback already starts new players unpaused. | Server test failed before the change and passes afterward for forward/backward transitions and stale requests. A native mpv test replaces players for episodes 1 → 2 → 1, verifies readiness while held, then verifies unpausing when the room releases them. |
| #101 | Starts the player idle timer on mount, so controls hide without requiring a fresh pointer event after episode navigation. Existing commit `70be9a94`. | Isolated timer harness verified mount, timeout and cleanup in the preceding implementation. Interactive browser playback was not exercised. |
| #77 | Failed music initialization closes the audio context, disconnects the node, terminates the worker, and clears the cached rejection. Worker errors, unreadable messages and a 15-second readiness deadline reject startup. | Worklet failure/retry, worker error/retry, and silent worker timeout/retry tests. |
| #76 | Requires a separator after the exact video stem. External subtitle IDs are SHA-256 identities of filenames rather than directory-list indexes. Missing files fail instead of silently selecting another file. | Episode1/Episode10/Episode1Extra matching; insertion before an existing track; deletion of the selected track. |
| #74 | Caches embedded video metadata separately and rediscovers external subtitles on every metadata request. | Cached probe test adds, renames and removes sidecars without changing the video. |
| #72 | Builds VP9 codec strings from actual profile, known level and pixel-format depth. Unknown parameters return no codec string. | Profile 2 / 10-bit source and unknown level/format cases. |
| #71 | Live HDR transcoding uses the clip pipeline's software tone mapping, downloads GPU frames when needed, and uploads converted SDR frames for hardware encoding. Encoder color metadata follows the filtered frames. | PQ and HLG filter tests; complete generated PQ and HLG fixtures transcoded to H.264 and inspected for BT.709 transfer, primaries and matrix. |
| #70 | Stream setup exceptions notify the player and release its video attachment. The caller handles rejected setup promises. | SourceBuffer setup failure emits the expected player error. |
| #68 | A response ending more than five seconds before the known duration reports an early-EOF error without ending the MediaSource or advancing the episode. | Early clean EOF fails safely; complete response finishes normally. |
| #66 | Pending reads have a 15-second inactivity check while actively playing with no useful buffer. Three retries use 1/2/4-second backoff; final failure is actionable. Pauses and buffer backpressure are exempt. Cancellation clears the deadline. | Open response stalls, retries four total requests, cancels them and reports failure. Paused response does not retry. |
| #65 | Renames and undo move every user's progress, merging path collisions by newest timestamp. Catalog media/item/series paths move in the same database transaction, retaining IDs. A failed commit attempts to restore the filesystem path. | Multiple users, resume/watched states, directory rename and undo, literal wildcard characters, neighboring prefixes, stable media and item IDs; existing filesystem-operation tests. |
| #98 | Adds NVENC selection, CUDA device index, startup encoder probing, capability/error fields, client settings and player labels. FFmpeg builds NVENC/NVDEC/CUDA filters with the existing pinned headers and clang, without non-free NPP or nvcc. Live streams and clips use CUDA scaling, with download/upload around tone mapping and subtitle overlays. Failed hardware decoding retries software decoding with NVENC before falling back fully to software. Per-stream failures do not disable hardware globally. Docker override and README instructions are included. | Native build succeeded with NVENC and CUDA filters enabled and nvcc/NPP disabled. Missing CUDA driver falls back safely. Schema/codegen and all three clients compile. Actual NVIDIA encoding/decoding and session-limit exhaustion require an NVIDIA host and remain unverified here. |

## Checks

- Server: `cargo test --locked` — 154 passed, 2 opt-in tests ignored. The new HDR stream test was also run explicitly against both generated PQ and HLG fixtures and passed. Media tests passed again after the final fallback change.
- Desktop: `cargo check --locked` passed. Full tests: 119 app tests plus 5 mpv tests passed; existing opt-in platform tests remained ignored. The added replacement-player test subsequently passed separately.
- Web: TypeScript check and 8 playback tests passed. Playback tests are now run by the web CI job.
- Mobile: typecheck and 30 tests passed.
- Shared package: typecheck and 27 tests passed.
- Schema export and code generation completed for shared, web and mobile.
- Rust formatting and `git diff --check` passed.

## Verification limits

The desktop replacement test uses actual mpv with headless output and room snapshots; it is not an interactive multi-device room test. Browser streaming tests exercise the real engine with controlled platform doubles. No NVIDIA driver/device is available here, so NVIDIA-specific hardware execution, visual quality, and concurrent-session exhaustion are not claimed as tested. No release or cross-platform builds were requested or run.

The implementation follows the [VP codec string specification](https://www.webmproject.org/vp9/mp4/), [FFmpeg filter documentation](https://www.ffmpeg.org/ffmpeg-filters.html), and [NVIDIA's FFmpeg guidance](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/ffmpeg-with-nvidia-gpu/index.html).
