// SPDX-License-Identifier: AGPL-3.0-or-later

//! A surface-bound Wayland inhibitor, or an owned D-Bus inhibition request. The guard never
//! outlives the window; dropping it releases inhibition, including during shutdown.

use std::sync::Arc;

use iced_winit::winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use iced_winit::winit::window::Window;
use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1;
use wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1;

pub struct Wayland {
    connection: Connection,
    manager: Option<ZwpIdleInhibitManagerV1>,
    surface: WlSurface,
    inhibitor: Option<ZwpIdleInhibitorV1>,
    queue: wayland_client::EventQueue<Registry>,
}

#[derive(Default)]
struct Registry;
impl Dispatch<wl_registry::WlRegistry, wayland_client::globals::GlobalListContents> for Registry {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let _ = event;
    }
}

impl Dispatch<ZwpIdleInhibitManagerV1, ()> for Registry {
    fn event(
        _: &mut Self,
        _: &ZwpIdleInhibitManagerV1,
        _: wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibit_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ZwpIdleInhibitorV1, ()> for Registry {
    fn event(
        _: &mut Self,
        _: &ZwpIdleInhibitorV1,
        _: wayland_protocols::wp::idle_inhibit::zv1::client::zwp_idle_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Wayland {
    pub fn new(window: &Window) -> Option<Self> {
        let RawDisplayHandle::Wayland(display) = window.display_handle().ok()?.as_raw() else { return None };
        let RawWindowHandle::Wayland(surface) = window.window_handle().ok()?.as_raw() else { return None };
        // SAFETY: winit owns both objects. Runner drops this guard before destroying its window.
        let backend =
            unsafe { wayland_client::backend::Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let surface_id = unsafe {
            wayland_client::backend::ObjectId::from_ptr(WlSurface::interface(), surface.surface.as_ptr().cast())
        }
        .ok()?;
        let surface = WlSurface::from_id(&connection, surface_id).ok()?;
        let (globals, queue) = wayland_client::globals::registry_queue_init::<Registry>(&connection).ok()?;
        let manager = globals.bind::<ZwpIdleInhibitManagerV1, _, _>(&queue.handle(), 1..=1, ()).ok();
        Some(Self { connection, manager, surface, inhibitor: None, queue })
    }

    pub fn set(&mut self, active: bool) -> bool {
        let Some(manager) = &self.manager else { return false };
        if active && self.inhibitor.is_none() {
            self.inhibitor = Some(manager.create_inhibitor(&self.surface, &self.queue.handle(), ()));
        } else if !active && let Some(inhibitor) = self.inhibitor.take() {
            inhibitor.destroy();
        }
        let _ = self.connection.flush();
        true
    }
}
impl Drop for Wayland {
    fn drop(&mut self) {
        self.set(false);
        if let Some(manager) = self.manager.take() {
            manager.destroy();
        }
        let _ = self.connection.flush();
    }
}

pub struct Inhibit {
    wayland: Option<Wayland>,
    fallback: Option<tokio::task::JoinHandle<()>>,
    window: Arc<Window>,
    runtime: tokio::runtime::Handle,
    active: bool,
}
impl Inhibit {
    pub fn new(window: Arc<Window>, runtime: tokio::runtime::Handle) -> Self {
        Self { wayland: Wayland::new(&window), fallback: None, window, runtime, active: false }
    }

    fn update(&mut self, active: bool) {
        if active == self.active {
            return;
        }
        self.active = active;
        if self.wayland.as_mut().is_some_and(|wayland| wayland.set(active)) {
            return;
        }
        if let Some(task) = self.fallback.take() {
            task.abort();
        }
        if active {
            let x11 = matches!(
                self.window.window_handle().map(|handle| handle.as_raw()),
                Ok(RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_))
            );
            self.fallback = Some(self.runtime.spawn(async move {
                if let Err(error) = fallback(x11).await {
                    tracing::warn!("can't inhibit idle: {error}");
                }
            }));
        }
    }
}
impl Drop for Inhibit {
    fn drop(&mut self) {
        if let Some(task) = self.fallback.take() {
            task.abort();
        }
    }
}

enum LeaseKind {
    X11(u32),
    Portal(zbus::zvariant::OwnedObjectPath),
}
struct Lease {
    connection: zbus::Connection,
    kind: Option<LeaseKind>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let Some(kind) = self.kind.take() else { return };
        let connection = self.connection.clone();
        tokio::spawn(async move {
            match kind {
                LeaseKind::X11(cookie) => {
                    if let Ok(proxy) = zbus::Proxy::new(
                        &connection,
                        "org.freedesktop.ScreenSaver",
                        "/org/freedesktop/ScreenSaver",
                        "org.freedesktop.ScreenSaver",
                    )
                    .await
                    {
                        let _ = proxy.call::<_, _, ()>("UnInhibit", &(cookie,)).await;
                    }
                },
                LeaseKind::Portal(path) => {
                    if let Ok(proxy) = zbus::Proxy::new(
                        &connection,
                        "org.freedesktop.portal.Desktop",
                        path,
                        "org.freedesktop.portal.Request",
                    )
                    .await
                    {
                        let _ = proxy.call::<_, _, ()>("Close", &()).await;
                    }
                },
            }
        });
    }
}

async fn fallback(x11: bool) -> zbus::Result<()> {
    let connection = zbus::Connection::session().await?;
    let kind = if x11 {
        let proxy = zbus::Proxy::new(
            &connection,
            "org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
            "org.freedesktop.ScreenSaver",
        )
        .await?;
        LeaseKind::X11(proxy.call("Inhibit", &("tinystream", "Video playback")).await?)
    } else {
        let proxy = zbus::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Inhibit",
        )
        .await?;
        let options = std::collections::HashMap::from([("reason", zbus::zvariant::Value::from("Video playback"))]);
        LeaseKind::Portal(proxy.call("Inhibit", &("", 8u32, options)).await?)
    };
    let _lease = Lease { connection, kind: Some(kind) };
    std::future::pending::<()>().await;
    Ok(())
}

impl crate::platform::IdleInhibit for Inhibit {
    fn set(&mut self, active: bool) {
        self.update(active);
    }
}

#[cfg(test)]
mod tests {
    use iced_winit::winit::application::ApplicationHandler;
    use iced_winit::winit::event::WindowEvent;
    use iced_winit::winit::event_loop::{ActiveEventLoop, EventLoop};
    use iced_winit::winit::platform::wayland::EventLoopBuilderExtWayland;
    use iced_winit::winit::window::WindowId;

    use super::*;

    #[test]
    #[ignore = "requires a Wayland compositor advertising idle-inhibit"]
    fn inhibitor_uses_the_window_surface_and_releases_it() {
        struct Test;
        impl ApplicationHandler for Test {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                let window = event_loop.create_window(Window::default_attributes()).unwrap();
                let mut wayland = Wayland::new(&window).expect("Wayland window");
                assert!(wayland.set(true), "compositor has no idle-inhibit manager");
                assert!(wayland.inhibitor.is_some());
                wayland.connection.roundtrip().unwrap();
                wayland.set(false);
                assert!(wayland.inhibitor.is_none());
                wayland.connection.roundtrip().unwrap();
                drop(wayland);
                event_loop.exit();
            }

            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        }
        let mut builder = EventLoop::builder();
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtWayland::with_wayland(&mut builder);
        builder.build().unwrap().run_app(&mut Test).unwrap();
    }
}
