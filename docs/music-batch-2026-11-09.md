# Music batch: Android #46 and desktop #91

Android now connects the shared decoder and queue policy to AudioTrack and a Media3 music service, with music browsing, the mini-player, Now Playing, lyrics, queue editing and playback settings. The rebuilt transport validates ranges, bounds responses, cancels pending network work and refuses redirects carrying credentials. Native events follow frames heard rather than tracks merely decoded. Android queue reports identify the client, and subscription events reconcile other clients' changes.

Desktop now has CPAL output through the Desktop platform interface, a bounded atomic PCM ring, the shared Rust sequencer, authenticated Range reads, queue persistence, original/FLAC selection, ReplayGain, crossfade, repeat, shuffle, lyrics, artwork, ratings and playlist editing. Music pauses for video and clips. The player has a pill, expanded card and full-screen view; MPRIS/media keys and tray behavior connect to music. Default-device changes reopen playback at the current position.

Listen Together uses the existing authenticated WebSocket protocol, clock-offset samples, drift correction, reconnects, room queues, participant status, host settings and invitations. Unknown room links can join as guests without adding or switching a saved server. Room links copied from the UI use the server's web URL.

The prior Android implementation was consulted for API/UI reference rather than restored wholesale. The current shared music API is retained. OpenSubtitles remains excluded.

## Verification

| Check | Result |
| --- | --- |
| Shared decoder with Opus | 15 passed: gapless, crossfade, gain, seeking, trimming, resampling and Opus pre-skip |
| Android Rust bridge | 2 passed: replacement/stop and Opus seek |
| Android native module | Debug Kotlin/Rust compilation passed for x86_64 |
| Android Kotlin tests | 6 passed: playback clock, wraparound, exact authenticated ranges, wrong-range/redirect rejection and active cancellation |
| Mobile | TypeScript check and 30 tests passed |
| Shared TypeScript package | TypeScript check and 27 tests passed |
| Desktop | cargo check --locked passed; full suite 131 app tests plus 5 mpv tests passed |
| Focused desktop music tests | 6 passed; guest link also covered by the app suite |
| CPAL output integration | Explicit ignored test passed, playing two decoded fixtures silently through local ALSA/PipeWire |
| Native UI | Explicit Vulkan render test passed; expanded queue/info renders inspected and placeholder sizing corrected |
| Clippy | Completed; one pre-existing collapsible-if warning in books/mod.rs |
| Formatting | cargo fmt and git diff --check passed |

The desktop audio dependencies were audited and explicitly approved by David before compilation; details are in desktop/docs/music-audit.md. Existing desktop lockfile pins were preserved.

## Remaining validation limits

No Android device was attached. Audible quality, screen-off/recents playback, Bluetooth/headset commands and OEM lock-screen surfaces were not exercised on a physical phone. The desktop output check was silent, so it does not establish perceptual click-free transitions or compare loudness by ear. Real mixed web/desktop listening sessions, guest permissions on a live deployment, device hotplug and long-session CPU use remain manual checks. Protocol tests use a local WebSocket peer; renders use synthetic library data.

No release builds, pushes, issue closures or publishing were performed.
