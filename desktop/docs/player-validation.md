# Player acceptance checks (#86)

## NVIDIA test build

In GitHub Actions, open a successful **CI** run for the player commit, then download
`desktop-linux-x86_64-test-<commit>`. Extract the ZIP and its `.tar.gz`. `BUILD.txt`
identifies the exact commit. This is an unoptimized development build, not a release.
The workflow can also be run manually against the branch.

Use Linux x86_64 with a working NVIDIA Vulkan/OpenGL driver and a GPU supporting
10-bit AV1 decode. The host needs EGL, Vulkan, xkbcommon and Wayland or X11 libraries,
plus PipeWire/ALSA for audio. No separate mpv or FFmpeg installation is needed.
On Ubuntu 24.04 the runtime packages include `libegl1 libvulkan1 libxkbcommon0
libwayland-client0 libwayland-cursor0 libwayland-egl1 libx11-6 libpipewire-0.3-0t64
libasound2t64 hwdata`. Use the NVIDIA driver packages appropriate to the machine.

From the extracted directory:

```sh
RUST_LOG=tinystream_desktop=debug,wgpu=warn,naga=warn ./tinystream-desktop 2>player.log
```

Connect and sign in to the test server normally. Open a known **10-bit AV1** video
using a `/watch/<id>` route (the app also accepts this as its first argument).
Keep Quality at **Auto**, which reads the original file. Use an SDR display for
this milestone. Do not add an `mpv.conf` for the first run.

Record the following results:

1. Picture and sound play correctly for at least a minute. `player.log` identifies
   the NVIDIA GPU and reports `decoding with nvdec`; `no`, software decode, or
   `nvdec-copy` does not establish the required shared hardware path. Confirm a
   first video frame is drawn and there are no GL/Vulkan import or mapping errors.
2. Seek forward and backward; change audio tracks when the file has multiple tracks.
   Both should respond promptly without corrupt frames.
3. Pause, select 720p, seek inside the buffered interval and outside it, then return
   to Auto. Position stays in absolute file time and the pause state is preserved.
   Resume and confirm direct playback reports NVDEC again.
4. Resize, enter/leave fullscreen with F and double-click, and toggle PiP with P.
   PiP keeps the same playback position; closing it returns to the main player.
   On Wayland, floating/above placement is compositor policy; niri and sway have
   explicit integration, while X11 uses the native always-on-top window hint.
5. Pause, leave, reopen the video and confirm resume/progress. Try a screenshot
   and X for the clip draft; closing the draft restores the original watch position.

Return `BUILD.txt`, GPU model, driver version (`nvidia-smi`), distribution,
Wayland/X11 and compositor, the video's codec/bit depth/resolution, each result,
and `player.log`. Redact private server paths if needed. A black window or a
software fallback is a failure to investigate, not a passing result.

## AMD and performance acceptance

The implementation has been exercised on an RX 7700 XT under niri, with generated
1080p 10-bit AV1 and 4K 10-bit HEVC MKVs. Hardware tests cover Vulkan pixel readback,
consecutive/concurrent players, resize, direct seeking/audio selection, 720p cached
and uncached seeks, return to direct play, and real HTTP-backed PiP lifecycles.

The additional HEVC workload uses [Jellyfin's test video](https://repo.jellyfin.org/test-videos/SDR/HEVC%2010bit/Test%20Jellyfin%204K%20HEVC%2010bit%2060M.mp4),
by Gnattu under CC BY-SA 4.0: 3840×2160, 60 fps, 10-bit SDR, 59 Mbps.
The source SHA-256 matches the published checksum:
`29464b4ccea8e34cc5af116b9dd1c193bbdef17db6eb036f7f2fd57d9163ed71`.
Its video bitstream was copied unchanged into a 119.8-second Matroska file (four
repetitions) with two Opus audio tracks from the generated fixture. This is a
high-bitrate sample remux, not a commercial disc remux.

The HTTP-backed test passed direct play with VA-API, audio selection, pause-preserving
720p, cached and uncached transcode seeks, return to direct play, clip frame stepping
and restoration against this file. Its real native PiP lifecycle also passed.

A 10-second comparison after a 3-second warmup used that same Matroska file at
2560×1440, VA-API, debanding, no audio output and hidden controls. The debug app
read through local HTTP; the installed optimized mpv read the local file.

| Workload | App CPU (% of one core) | mpv CPU | App GPU busy | mpv GPU busy |
| --- | ---: | ---: | ---: | ---: |
| Generated 4K24 HEVC, preliminary | 9.38% | 3.69% | 43.50% | 42.34% |
| Jellyfin 4K60 HEVC remux, final retained rendering | 6.89% | 8.78% | 50.76% | 53.82% |

GPU measurements are whole-device busy averages, sampled every 200 ms; CPU is
`/proc/<pid>/stat` user+system deltas divided by elapsed wall time. Both windows
were fullscreen at the same size. Both used mpv 0.41.0, but different FFmpeg and
libplacebo builds. The preliminary generated comparison preceded the final
separation of widget and decoder wakeups. No release build was made.
These short measurements show comparable load on the high-bitrate sample; they
are not a claim about all remuxes, drivers or frame pacing. Native tests separately
check that new video frames continue to be drawn after chrome fades.

David's library is primarily 1080p 10-bit AV1; no commercial 4K HEVC remux is
available. The public sample supplies the high-bitrate HEVC acceptance workload.
NVIDIA/NVDEC remains pending the friend's CI-build result. Keep #86 open until
that result is recorded.
