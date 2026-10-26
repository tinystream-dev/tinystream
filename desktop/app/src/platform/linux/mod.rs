// SPDX-License-Identifier: AGPL-3.0-or-later

use futures::future::BoxFuture;

use super::Desktop;
use crate::chrome::ButtonLayout;

mod gpu;
mod portal;
mod secrets;

pub struct Linux;

impl Desktop for Linux {
    fn secrets() -> crate::secrets::Store {
        std::sync::Arc::new(secrets::LinuxSecrets::default())
    }

    fn open_url(url: String) -> BoxFuture<'static, Result<(), String>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || std::process::Command::new("xdg-open").arg(url).status())
                .await
                .map_err(|_| "Couldn't open the link.".to_string())?
                .map_err(|_| "Couldn't open the link.".to_string())?
                .success()
                .then_some(())
                .ok_or_else(|| "Couldn't open the link.".into())
        })
    }

    fn request_device(
        adapter: &wgpu::Adapter,
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Option<(wgpu::Device, wgpu::Queue)> {
        gpu::request_device(adapter, descriptor)
    }

    fn watch_button_layout(send: Box<dyn Fn(ButtonLayout) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(portal::watch_button_layout(send))
    }

    fn watch_reduced_motion(send: Box<dyn Fn(bool) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(portal::watch_reduced_motion(send))
    }
}
