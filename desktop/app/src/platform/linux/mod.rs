// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use futures::future::BoxFuture;

use super::{Desktop, VideoSurface};
use crate::chrome::ButtonLayout;

mod gpu;
mod pip;
mod portal;
mod secrets;
mod video;

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

    fn picture_in_picture_attributes() -> iced_winit::winit::window::WindowAttributes {
        use iced_winit::winit::platform::wayland::WindowAttributesExtWayland;
        super::pip_attributes().with_name(pip::APP_ID, "tinystream-pip")
    }

    fn picture_in_picture_opened() {
        pip::opened();
    }

    fn request_device(
        adapter: &wgpu::Adapter,
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Option<(wgpu::Device, wgpu::Queue)> {
        gpu::request_device(adapter, descriptor)
    }

    fn utc_offset(at: i64) -> i64 {
        /// glibc's `struct tm`.
        #[repr(C)]
        struct Tm {
            fields: [std::ffi::c_int; 9],
            gmtoff: std::ffi::c_long,
            zone: *const std::ffi::c_char,
        }
        unsafe extern "C" {
            fn localtime_r(time: *const i64, tm: *mut Tm) -> *mut Tm;
        }
        let mut tm = Tm { fields: [0; 9], gmtoff: 0, zone: std::ptr::null() };
        // SAFETY: localtime_r only writes the struct it's given.
        if unsafe { localtime_r(&at, &mut tm) }.is_null() { 0 } else { tm.gmtoff as i64 }
    }

    fn video(
        device: &wgpu::Device,
        mpv: &mpv::Mpv,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Box<dyn VideoSurface>, String> {
        video::Surface::start(device, mpv, wake).map(|surface| Box::new(surface) as Box<dyn VideoSurface>)
    }

    fn watch_button_layout(send: Box<dyn Fn(ButtonLayout) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(portal::watch_button_layout(send))
    }

    fn watch_reduced_motion(send: Box<dyn Fn(bool) + Send + Sync>) -> BoxFuture<'static, ()> {
        Box::pin(portal::watch_reduced_motion(send))
    }
}
