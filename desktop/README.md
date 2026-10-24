# tinystream desktop

The Linux desktop client lives in its own Cargo workspace. It currently provides the window,
navigation shell, keyboard shortcuts and placeholder pages from [foundation #81](https://github.com/tinystream-dev/tinystream/issues/81).
Server connections and the media players arrive in later parts of [#80](https://github.com/tinystream-dev/tinystream/issues/80).

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
