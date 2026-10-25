# tinystream desktop

The Linux desktop client lives in its own Cargo workspace. It currently provides the window,
navigation shell, server connections, sign-in and live API events. Browsing pages and media
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
placeholder page, for example `cargo run --locked -- /watch/12`.

## Rendering and dependencies

iced is pinned to master commit `cd04b74ca7073766f654535d6e72c452ef749d07` (2026-10-21).
The app uses `iced_core`, `iced_widget`, `iced_winit` and `iced_wgpu` directly, following iced's
integration example. `runner.rs` owns the winit event loop and the wgpu device. There are no forks
or local dependency patches.

On Linux, `platform/linux/gpu.rs` opens the Vulkan device through wgpu-hal, enabling
`VK_KHR_external_memory_fd` and `VK_KHR_external_semaphore_fd` for the future mpv renderer. If the
driver lacks these extensions, the shell uses a standard wgpu device and logs the limitation.
The raw device remains accessible through `wgpu::Device::as_hal`.

The [dependency audit](https://github.com/tinystream-dev/tinystream/issues/81#issuecomment-6389685662)
was approved in the implementation conversation, including the recent iced, cosmic-text and
cryoglyph git pins and zbus 5.19.0. The audit comment describes an earlier fork proposal; the final
implementation uses the integration APIs instead, as requested. zbus stays pinned to 5.19.0.
All future additions and pin changes require the same audit before building. Use `--locked` and
never `cargo update`.

## Window behavior

The shell draws its own controls and resize edges. Drag the logo row or the top 12 logical pixels
of the content; double-click either area to maximize, or right-click for the compositor's window
menu where supported. Controls follow the settings portal's button layout and update live.
Fullscreen hides them. Tiled-state detection and hiding controls while tiled are deferred, as
agreed, to keep winit unpatched.

Window size and maximized state are saved in the XDG state directory as `tinystream/window.json`.
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
