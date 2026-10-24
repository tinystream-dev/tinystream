// SPDX-License-Identifier: AGPL-3.0-or-later

//! What differs between desktops. Each platform implements [`Desktop`]; only Linux exists so far,
//! and every other platform gets the defaults.

use futures::future::BoxFuture;
use iced_wgpu::wgpu;

use crate::chrome::ButtonLayout;

#[cfg(target_os = "linux")]
mod linux;

pub trait Desktop {
    /// Opens the GPU device, or leaves it to wgpu's defaults with `None`.
    fn request_device(
        _adapter: &wgpu::Adapter,
        _descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Option<(wgpu::Device, wgpu::Queue)> {
        None
    }

    /// Follows the desktop's window button layout, calling `send` now and whenever it changes.
    fn watch_button_layout(_send: Box<dyn Fn(ButtonLayout) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(async {})
    }
}

#[cfg(target_os = "linux")]
pub type Current = linux::Linux;

#[cfg(not(target_os = "linux"))]
pub type Current = Fallback;

#[cfg(not(target_os = "linux"))]
pub struct Fallback;

#[cfg(not(target_os = "linux"))]
impl Desktop for Fallback {}
