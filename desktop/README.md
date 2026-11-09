# tinystream desktop

The Linux desktop client lives in its own Cargo workspace. It provides the window, navigation
shell, server connections, sign-in, live API events, browsing, settings, admin screens and the video player. Other media
players arrive in later parts of [#80](https://github.com/tinystream-dev/tinystream/issues/80).

From `desktop/`:

```sh
cargo run --locked
cargo check --locked
cargo clippy --locked --all-targets
cargo test --locked
cargo --locked fmt --check
```

The root `rust-toolchain.toml` and `rustfmt.toml` apply here too. On Ubuntu, install `pkg-config`,
`libwayland-dev`, `libxkbcommon-dev`, `libx11-dev` and `libvulkan-dev`, plus a working Vulkan driver.
Wayland is preferred when available; winit falls back to X11. An optional path argument opens a
page, for example `cargo run --locked -- '/watch/12?t=90'` to play from 90 seconds.

## Browsing

The browsing routes in `app/src/browsing/` follow the desktop web routes: Home, the search
palette, Discover, video and book libraries and book series, titles and their seasons,
Calendar, Requests, Wanted and server Downloads. Music shelves on Home lead into #91's
routes; Clips are below, and book readers remain #94. Account switching and the shortcuts sheet
are part of the shell. The current web title route and shared schema have no cast selection;
the desktop uses the same available title metadata.

Generated operations come from `app/graphql/browsing.graphql` and the shared fragments and
schema. Shared view models keep the repeated selections consistent across screens. Cached
pages keep their filters and scroll position on back/forward navigation. Server/profile
changes discard those caches and reject late replies. Reconnect acknowledgements invalidate
all cached pages; download events invalidate their related screens without refetching an
unrelated library. Progress saves also invalidate watched state after the player has closed.
Download, calendar and request polling fills the gaps between events.

Artwork is requested on entering the scroll viewport, with at most six fetch/decode jobs at
once, keeps at most 256 decoded pictures, and uses the existing disk cache. Artwork uploads use the FileChooser desktop portal
behind `Desktop::choose_image`, then the authenticated GraphQL multipart upload protocol.
Recent title IDs are kept locally per server and account and checked against the server before
appearing in search. No download sources are supplied by the client.

The existing headless renderer accepts browsing scenes as well as sign-in scenes:

```sh
cargo run --locked -- --scene home --snapshot /tmp/home.png --size 1440x1000
cargo run --locked -- --scene title --scroll 650 --snapshot /tmp/episodes.png
cargo run --locked -- --scene library --pointer 320,240 --right-click --snapshot /tmp/menu.png
cargo run --locked -- --scene search --key ArrowDown --snapshot /tmp/search.png
cargo run --locked -- --scene downloads --style glass --custom --reduce-motion --snapshot /tmp/downloads.png
```

Browsing scenes include `home`, `library`, `title`, `discover`, `calendar`, `requests`,
`wanted`, `downloads`, `notifications`, `search`, `books`, `series`, `add`, `releases` and
`series-settings`. They reset application zoom in memory, so the requested logical size is
reproducible. `--style flat|glass`, `--custom`, `--reduce-motion` and `--scroll` also apply.
The fixtures are local; they do not connect to a server or perform the actions shown.

## Rendering and dependencies

iced is pinned to master commit `cd04b74ca7073766f654535d6e72c452ef749d07` (2026-10-21).
The app uses `iced_core`, `iced_widget`, `iced_winit` and `iced_wgpu` directly, following iced's
integration example. `runner.rs` owns the winit event loop and the wgpu device. There are no forks
or local dependency patches.

On Linux, `platform/linux/gpu.rs` opens the Vulkan device through wgpu-hal, enabling
`VK_KHR_external_memory_fd` and `VK_KHR_external_semaphore_fd` for the mpv renderer. If the
driver lacks these extensions, the shell uses a standard wgpu device and logs the limitation.
The raw device remains accessible through `wgpu::Device::as_hal`.

The [dependency audit](https://github.com/tinystream-dev/tinystream/issues/81#issuecomment-6389685662)
was approved in the implementation conversation, including the recent iced, cosmic-text and
cryoglyph git pins and zbus 5.19.0. The audit comment describes an earlier fork proposal; the final
implementation uses the integration APIs instead, as requested. zbus stays pinned to 5.19.0.
All future additions and pin changes require the same audit before building. Use `--locked` and
never `cargo update`.

## Video

Playback goes through libmpv (`mpv/`, over the hand-written bindings in `mpv-sys/`), linked
statically from `native/mpv`: mpv 0.41.0 built with its OpenGL render API on EGL, VA-API through a
DRM render node the app passes in, NVDEC, the PipeWire and ALSA outputs, and nothing else. It's
patched to build without libass (`native/mpv/patches/no-libass.patch`), since rustyass renders text
subtitles; mpv still draws bitmap ones like PGS. FFmpeg comes from `native/ffmpeg` with its `nvdec`
feature, and libplacebo (OpenGL only), libdrm and libdisplay-info from `native/` too. The first build
compiles all of them into `../.native`, which takes a while.

They need `meson`, `ninja`, `python3`, `git`, `curl`, the libdrm headers, `libegl-dev`,
`libpipewire-0.3-dev`, `libasound2-dev` and `hwdata` (see `.github/actions/native-deps` and the
desktop CI job). `cargo test --locked -p mpv` plays a short clip through the render API into an
offscreen framebuffer on Mesa's surfaceless EGL, with VA-API too when `/dev/dri/renderD128` exists.

The player reads the original Range-capable `/file` endpoint through the app's rustls HTTP
client. Its quality menu switches to a server transcode; audio switches use mpv directly for
the original file and rebuild a transcode at the current file time otherwise. Stream reads
have an inactivity timeout and bounded retries. An early EOF cannot advance the episode while
most of its known duration remains. Progress is saved every ten seconds, on pause and on exit.

`platform::VideoSurface` keeps GPU sharing behind the platform boundary. Linux matches EGL and
Vulkan device UUIDs, prefers Mesa's surfaceless display for VA-API imports, and falls back to
device displays. Hardware interop follows the matched GPU (VA-API on AMD/Intel, CUDA on
NVIDIA), avoiding unrelated driver probes. mpv renders into three Vulkan-allocated images imported into OpenGL, which wgpu
samples directly. The render thread finishes OpenGL before handing over a frame and waits for
the preceding Vulkan submission before reusing its memory. The player also draws the ambient
light, chapter timeline and previews, skip buttons, episode countdown, paused and finale
overlays, track/quality/speed menus, screenshot feedback and fullscreen controls.

An optional `mpv.conf` in the app's config directory can tune picture quality; Settings →
Advanced shows its path. Playback, input, timing and text-subtitle options remain controlled by
the app. Text subtitles use the shared `subtitles/` rustyass renderer with the same fonts as
web, timed to the rendered video frame. Bitmap subtitles use mpv during direct play and are
disabled on transcodes. Track choices are remembered; switching never reloads the video.
P opens a native picture-in-picture window using the same decoder and
GPU device; closing it returns to the player. X opens the clip editor on that player (see
Clips). Active video inhibits screen blanking through the platform idle-inhibition guard, including PiP.

## Clips

`/clips` (#93) is the web's Clips page: yours, shared with you and shared by you, filtered by
kind or by a book's quotes, with your clip space. `/clips?clip=<id>` opens a clip's dialog over it,
in place as on web, with its public link, who it was sent to, sending, renaming, deleting and
downloading (saved where the FileChooser portal says, named as the server names it). Rendered
clips play through their own mpv and the same stream callback as the player
(`player/clip_player.rs`), with the web clip player's controls; F fills the window with one.

The editor (`player/clip.rs`, `player/strip.rs`) works in the player's own mpv, on direct play:
the range on a filmstrip of the server's preview frames, frame stepping (`,` `.`), I and O, the
arrow keys nudging the edge last touched by a frame (a second with Shift), and the range looping
exactly through mpv's A-B loop while it plays. Text subtitles show through the same rustyass
overlay as the player, so the preview has what the server burns in. Its presets, size estimate
and every other option are the web's, sent with the same mutations; changing a clip (the dialog's
pencil) opens the editor on its recipe. Render progress, in the editor and on the page, comes
through the `events` subscription.

## Watch Together

`/together/<code>` opens a room (#88). `player/sync.rs` is the web's `sync.ts`, with the same
constants: the room clock, the NTP-style offset from the quickest round trip, and the loop that
nudges mpv's `speed` for small drift, seeks on big drift and only says it's ready again after
refilling five seconds. It talks to `/api/together/{code}/ws` and plays from the room's own media
routes, direct by default or a transcode from the quality menu, so guests need no account. Room
links (`https://<server>/together/<code>` or `tinystream://<host[:port]>/together/<code>`) given
on the command line go through `App::join_room`, signed in when they're for the server in use and
as a guest otherwise. The player's Users button starts a room, and its panel invites people on
the server. Closing a room hides it to the tray when a tray host is available and the setting is enabled.
Picture in picture and clips inside rooms aren't there yet.

The steering loop can be checked against real mpv, and the protocol against an isolated server
(with a second user there, it checks invites too):

```sh
TINYSTREAM_TEST_VIDEO='/absolute/path/to/a-minute-or-more.mp4' cargo test --locked steers_mpv_onto_the_room_clock -- --ignored
cargo test --locked hosts_and_guests_share_a_room_on_the_server -- --ignored
```

To visibly play a local file through the same mpv/GPU path, without a server:

```sh
cargo run --locked -- --video '/absolute/path/to/video.mkv'
```

Space pauses, Left/Right seek ten seconds, Up/Down change volume, M mutes, and F
or double-click toggles fullscreen. Escape leaves fullscreen, then closes the viewer.
This diagnostic viewer does not save server progress or offer server transcoding.
Its opt-in hardware check opens the real window, requires hardware decoding and a drawn
frame, and rejects unrelated CUDA-load diagnostics:

```sh
TINYSTREAM_TEST_VIDEO='/absolute/path/to/video.mkv' cargo test --locked --test local_video -- --ignored
```

The hardware checks are opt-in. From `desktop/`, the shared-texture test reads actual pixels
back through Vulkan, opens consecutive players, reloads, and resizes their textures:

```sh
cargo test --locked shares_decoded_frames_with_vulkan_across_players -- --ignored
cargo test --locked player_chrome_and_menus_render_and_accept_input -- --ignored
cargo test --locked picture_in_picture_keeps_one_player_and_survives_navigation -- --ignored
```

`TINYSTREAM_TEST_VIDEO` can replace the bundled H.264 test pattern with a generated 10-bit AV1
or HEVC fixture. `TINYSTREAM_TEST_ARTIFACT_DIR` retains the player UI test's PNGs. For an isolated
test server with a video at least 60 seconds long and two audio tracks, set
`TINYSTREAM_TEST_SERVER`, `TINYSTREAM_TEST_TOKEN`, and optionally `TINYSTREAM_TEST_VIDEO_ID`
(defaults to 1), then run:

```sh
cargo test --locked direct_play_seeks_tracks_and_transcodes -- --ignored
```

That check exercises the HTTP protocol, hardware direct play, audio selection, seeking,
pause-preserving 720p transcode, cached and uncached seeks, return to direct play and clip
preview restoration. It makes no progress mutations. The same server variables enable real
playback in the native PiP test. Local validation used generated 1080p 10-bit AV1 and 4K 10-bit
HEVC patterns on AMD with VA-API. The [acceptance checklist](docs/player-validation.md) records
measurements and the confirmed NVIDIA tester result; the [requirement audit](docs/player-completion.md)
maps the issue to implementation and evidence. CI produces a debug test tarball with licenses
and that checklist for the NVIDIA tester.

## Window behavior

The shell draws its own controls and resize edges. Drag the logo row or the top 12 logical pixels
of the content; double-click either area to maximize, or right-click for the compositor's window
menu where supported. Controls follow the settings portal's button layout and update live.
Fullscreen hides them. Tiled-state detection and hiding controls while tiled are deferred, as
agreed, to keep winit unpatched.

Window size, maximized state and UI scale are saved on close in the XDG state directory as
`tinystream/window.json`. `Ctrl++` (or `Ctrl+=`) and `Ctrl+-` change UI scale in browser-style
steps from 50% to 200%; `Ctrl+0` resets it to 100%. These work in the gallery, fields and dialogs,
and multiply the display’s DPI scale without changing the window size.
Configuration and cache directories also use XDG paths through `paths.rs`.

## Connections and credentials

The first run opens Connect to a server. Bare hosts and host:port addresses try HTTPS before
HTTP; explicit HTTP(S) URLs keep their scheme. HTTP connections show an unencrypted connection
warning and an HTTP marker in the server list. The API currently exposes no server display name,
so the address labels each saved server.

The account menu switches servers, adds/removes them and signs out. Switching cancels requests
and subscriptions and clears navigation and session data. The client uses bearer authentication
for GraphQL and media requests, and sends the token in graphql-transport-ws connection_init.
Each successful connection, including reconnections, invalidates session data and refetches it;
revision and last_event are the hooks for future browsing/player caches.

Server metadata, the active server and update preferences live in the XDG config directory's
`tinystream/servers.json`. Tokens never go in this file. Linux uses Secret Service through zbus;
if no service is available, credentials go in `tinystream/tokens/` in the state directory, with
0700 directory and 0600 file permissions. Settings explains this fallback once. Locked keyrings
prompt for unlock rather than silently falling back. Local fallback credentials migrate into the
keyring when it returns; sign-out tombstones prevent old keyring credentials from resurfacing.
Server identifiers are random so resetting the config cannot attach old credentials to a new server.

Profile and username/password sign-in, first-server setup, invite acceptance and password reset
use generated GraphQL types from `app/graphql/session.graphql`, `packages/shared/schema.graphql`
and the shared Person/NotificationFields fragments. The app build script generates them in OUT_DIR.
Changes to these documents are checked by `cargo check --locked`; no separate Rust codegen command
is needed. The sign-in screen accepts invite/reset links, and a URL argument preserves its fragment,
for example `cargo run --locked -- 'https://server.example/invite#token'`. `tinystream://host/reset#token`
uses the matching saved server, or prompts to connect first. Desktop link registration and forwarding
to an existing instance belong to #92.

A dismissible banner reports app/server major/minor differences. Settings enables the GitHub
release check by default; attempts are persisted and limited to once per day, including failures.
New releases link to their release page; the app does not self-update.

The [connect/auth audit](docs/connect-auth-audit.md) records twelve approved dependency versions;
eleven remain in the final build after removing the derive macro. Focused tests cover transport/authentication, reconnect invalidation,
server isolation, sign-in variants and credential-file permissions. The ignored Secret Service
round-trip test must run with isolated XDG directories and either a private, unlocked keyring
under `dbus-run-session`, or `TINYSTREAM_TEST_FILE_SECRETS=1` with an unavailable session bus.

The auth screens use the web's Geist font, shared logo, 384 px form frame, 36 px inputs,
44 px primary action, continuous panel corners and layered surfaces. Colours follow the
server's public appearance (and the signed-in user's appearance afterward); light/dark
changes also reach the shell and window controls. Passkeys remain #95.

The UI repair was verified in a native 1280×800 Linux window against two isolated real
0.38.0 servers: setup, password/profile sign-in with a served avatar, switching/removal,
invite acceptance, reset-link redemption, live appearance events and server restart/reconnect.
App restart restored both file-fallback sessions and sessions migrated to an isolated GNOME
Secret Service. The installed server lacks SMTP, so the reset token was seeded in its temporary
database; mail delivery was not verified. Version mismatch, updates and transport error paths
are covered by the focused tests. No release build was run.

## Design system

`app/src/ui/` follows the web's `styles.css` and components: all 28 colour tokens,
Layered/Flat/Glass, Geist Variable, bundled Lucide vectors, squircle surfaces and focus rings.
`theme/` contains the server's unchanged theme derivation and frozen `ts1` format;
`squircle/` ports the shared TypeScript corner geometry. Neither adds a registry dependency.
Shared crate changes trigger the desktop CI job too.

The app draws into a readable frame texture. Glass samples only the layer already drawn
beneath it, then downsamples, blurs horizontally/vertically and composites through the
squircle outline. A surface without COPY_DST support uses solid materials. Posters use a
separate perspective pipeline for tilt/glare. Route changes blur and fade the page while the
chrome stays in place. Reduced motion follows the settings portal (GNOME animations as a
fallback), with a persisted System/On/Off override in Settings → Appearance.

The reusable pieces include buttons, fields, switches, checkboxes, keyboard-operated selects,
menus, dialogs/sheets, segments, badges, progress, spinners, posters/grids/shelves, avatars,
images, skeletons, tickers, sparklines and toasts. Modal layers contain keyboard focus and
close on Escape or an outside click. The image cache shows disk entries immediately and
revalidates them from the connected server; decoding runs off the UI thread with allocation
limits. A late disk read cannot overwrite a newer download.

The development gallery and headless PNG snapshots need no display connection:

```sh
cargo run --locked -- --gallery
cargo run --locked -- --gallery --style glass --custom --snapshot /tmp/gallery.png
cargo run --locked -- --gallery --scroll 2100 --pointer 100,100 --snapshot /tmp/posters.png
cargo run --locked -- --gallery --scene dialog --key Escape --snapshot /tmp/dismissed.png
cargo run --locked -- --scene servers --snapshot /tmp/servers.png
cargo run --locked -- --scene http://localhost:3000 --light --snapshot /tmp/sign-in.png
cargo test --locked snapshot::tests::scrolled_artwork_and_modal_keyboard_input -- --ignored
cargo test --locked --manifest-path ../Cargo.toml -p tinystream-theme -p tinystream-squircle
```

`--style flat|layered|glass`, `--custom`, and `--reduce-motion` select gallery variants.
`--size 1920x1200 --scale 1.5` checks fractional scaling at a logical 1280×800.
`--scroll`, `--pointer`, `--click`, and `--key Tab|Enter|Space|Escape` exercise the headless UI.
Snapshot scenes include connect, login, profiles, unlock, servers, settings, invite and reset;
a URL scene reads only the server's public connection query. Scenes never perform sign-in,
credential or configuration writes. The gallery is available only in debug builds.

Geist and Lucide notices are embedded in Settings → About → Licenses and collected in
`THIRD_PARTY_LICENSES`. Native and Rust dependency notices are collected by release packaging
(#96) alongside these asset notices.

## Desktop services

Linux provides MPRIS media controls, native Wayland idle inhibition (portal/X11 fallbacks),
notifications, single-instance link routing and a StatusNotifierItem tray. Settings → Show tray
icon controls close-to-tray. Ctrl+Q always quits. See [OS integration](docs/os-integration.md)
for interfaces, packaging resources and focused validation commands.

## Settings and administration

Settings has the same personal, library, downloads, server and credits sections as the web.
Admin sections follow the signed-in user's role, and download settings appear when downloads
are enabled. Editors use the shared GraphQL schema and the web's server mutations; server
validation errors keep the draft open. Live config, user and appearance events refresh loaded
settings without replacing an open draft. Destructive actions ask for confirmation.

Appearance supports server defaults, personal overrides, system light/dark schemes, component
styles, scheme creation/forking/publishing and `ts1` import/export. The scheme editor derives its
live preview and contrast warnings with the shared theme crate. Passkeys are listed read-only
until #95. Profile pictures use the desktop image picker.

The Desktop tab stores default playback quality, hardware decoding, motion, tray and update
preferences on this machine. Auto quality means direct play; changes apply to newly opened
videos. It also shows the mpv configuration path and token storage backend. Credits reads the
generated `THIRD_PARTY_LICENSES` beside the packaged executable, with embedded asset notices
as the development fallback.

Focused settings tests run with `cargo test --locked settings::`. The optional native rendering
and input test needs Vulkan: `cargo test --locked settings_editor_renders_and_saves -- --ignored`.
Set `TINYSTREAM_TEST_ARTIFACT_DIR` to retain its account, music and scheme-editor PNGs.
