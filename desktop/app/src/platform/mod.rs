// SPDX-License-Identifier: AGPL-3.0-or-later

//! What differs between desktops. Each platform implements [`Desktop`]; only Linux exists so far,
//! and every other platform gets the defaults.

use std::sync::Arc;

use futures::future::BoxFuture;
use iced_wgpu::wgpu;

use crate::chrome::ButtonLayout;

#[cfg(target_os = "linux")]
mod linux;

pub trait Desktop {
    fn secrets() -> crate::secrets::Store;

    fn picture_in_picture_attributes() -> iced_winit::winit::window::WindowAttributes {
        pip_attributes()
    }

    fn picture_in_picture_opened() {}

    fn open_url(_url: String) -> BoxFuture<'static, Result<(), String>> {
        Box::pin(async { Err("Opening links is not supported on this platform.".into()) })
    }

    /// Opens the GPU device, or leaves it to wgpu's defaults with `None`.
    fn request_device(
        _adapter: &wgpu::Adapter,
        _descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Option<(wgpu::Device, wgpu::Queue)> {
        None
    }

    /// How far local time is ahead of UTC at Unix time `at`, in seconds.
    fn utc_offset(_at: i64) -> i64 {
        0
    }

    /// Draws `mpv`'s video on `device`, calling `wake` from another thread whenever a frame is
    /// ready.
    fn video(
        _device: &wgpu::Device,
        _mpv: &mpv::Mpv,
        _wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Box<dyn VideoSurface>, String> {
        Err("Video can't be shown on this platform yet.".into())
    }

    /// Follows the desktop's window button layout, calling `send` now and whenever it changes.
    fn watch_button_layout(_send: Box<dyn Fn(ButtonLayout) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(async {})
    }

    /// Follows whether the desktop asks for less motion, calling `send` now and whenever it
    /// changes.
    fn watch_reduced_motion(_send: Box<dyn Fn(bool) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(async {})
    }
}

/// mpv's video, drawn into textures wgpu samples without copying them (Windows would share D3D11
/// handles, macOS IOSurfaces).
pub trait VideoSurface: Send + Sync {
    /// Asks for frames `width` by `height` physical pixels from now on.
    fn resize(&self, width: u32, height: u32);

    /// The newest finished frame. It's not drawn into again until a newer one has been taken.
    fn frame(&self) -> Option<VideoFrame>;

    /// Says the frame taken last has reached the screen, for mpv's timing.
    fn presented(&self);

    /// Why video can't be drawn, once that's known.
    fn failure(&self) -> Option<String>;
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub texture: wgpu::Texture,
    /// Counts up with each frame mpv renders.
    pub id: u64,
}

#[cfg(target_os = "linux")]
pub type Current = linux::Linux;

#[cfg(not(target_os = "linux"))]
pub type Current = Fallback;

#[cfg(not(target_os = "linux"))]
pub struct Fallback;

#[cfg(not(target_os = "linux"))]
impl Desktop for Fallback {
    fn secrets() -> crate::secrets::Store {
        std::sync::Arc::new(UnavailableSecrets)
    }
}

#[cfg(not(target_os = "linux"))]
struct UnavailableSecrets;

#[cfg(not(target_os = "linux"))]
impl crate::secrets::Secrets for UnavailableSecrets {
    fn get(&self, _: u64) -> BoxFuture<'_, anyhow::Result<crate::secrets::Stored>> {
        Box::pin(async { anyhow::bail!("Credential storage is not supported on this platform.") })
    }

    fn set(&self, _: u64, _: Option<crate::api::Token>) -> BoxFuture<'_, anyhow::Result<bool>> {
        Box::pin(async { anyhow::bail!("Credential storage is not supported on this platform.") })
    }
}

fn pip_attributes() -> iced_winit::winit::window::WindowAttributes {
    use iced_winit::winit::dpi::LogicalSize;
    use iced_winit::winit::window::{Window, WindowLevel};
    Window::default_attributes()
        .with_title("Picture in picture — tinystream")
        .with_inner_size(LogicalSize::new(480.0, 270.0))
        .with_min_inner_size(LogicalSize::new(240.0, 135.0))
        .with_decorations(false)
        .with_window_level(WindowLevel::AlwaysOnTop)
}
