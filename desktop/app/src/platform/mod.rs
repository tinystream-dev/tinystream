// SPDX-License-Identifier: AGPL-3.0-or-later

//! What differs between desktops. Each platform implements [`Desktop`]; only Linux exists so far,
//! and every other platform gets the defaults.

use futures::future::BoxFuture;
use iced_wgpu::wgpu;

use crate::chrome::ButtonLayout;

#[cfg(target_os = "linux")]
mod linux;

pub trait Desktop {
    fn secrets() -> crate::secrets::Store;

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
