// SPDX-License-Identifier: AGPL-3.0-or-later

use futures::future::BoxFuture;

use super::Desktop;
use crate::chrome::ButtonLayout;

mod gpu;
mod portal;

pub struct Linux;

impl Desktop for Linux {
    fn request_device(
        adapter: &wgpu::Adapter,
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Option<(wgpu::Device, wgpu::Queue)> {
        gpu::request_device(adapter, descriptor)
    }

    fn watch_button_layout(send: Box<dyn Fn(ButtonLayout) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(portal::watch_button_layout(send))
    }
}
