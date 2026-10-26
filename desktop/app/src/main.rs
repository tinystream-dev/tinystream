// SPDX-License-Identifier: AGPL-3.0-or-later

//! # tinystream desktop
//!
//! The native tinystream client: the web UI's desktop layout in iced, with video through libmpv and
//! subtitles through rustyass.

#![recursion_limit = "256"]

mod api;
mod app;
mod chrome;
mod images;
mod keys;
mod paths;
mod platform;
mod route;
mod runner;
mod secrets;
mod servers;
mod session;
mod shell;
#[cfg(debug_assertions)]
mod snapshot;
#[cfg(test)]
mod test_support;
mod ui;
mod updates;
mod window_state;

use std::process::ExitCode;

use iced_core::window;
use iced_winit::winit::event_loop::EventLoop;
use tracing_subscriber::EnvFilter;

use crate::app::{App, Message};
use crate::platform::Desktop;
use crate::route::Route;
use crate::runner::{Runner, UserEvent};
use crate::window_state::{MIN_SIZE, WindowState};

/// The Wayland app id and X11 class, matching the desktop entry (#96).
const APP_ID: &str = "dev.tinystream.Tinystream";

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,wgpu=warn,naga=warn")),
        )
        .init();

    tracing::debug!(config = ?paths::config_dir(), state = ?paths::state_dir(), cache = ?paths::cache_dir(), "paths");

    let mut fonts = iced_wgpu::graphics::text::font_system().write().expect("font system");
    fonts.load_font(std::borrow::Cow::Borrowed(ui::text::GEIST));
    fonts.raw().db_mut().set_sans_serif_family("Geist");
    drop(fonts);

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!("can't start the async runtime: {error}");
            return ExitCode::FAILURE;
        },
    };
    let flags = Flags::parse();
    // A path like `/title/12` opens that screen, as web links will (#92).
    let argument = flags.argument.clone();
    let route = argument.as_deref().and_then(Route::parse).unwrap_or(Route::Home);
    let state = WindowState::load();
    let settings = window::Settings {
        size: state.size(),
        maximized: state.maximized,
        min_size: Some(MIN_SIZE),
        decorations: false,
        platform_specific: window::settings::PlatformSpecific {
            application_id: APP_ID.to_owned(),
            ..Default::default()
        },
        ..window::Settings::default()
    };

    let mut app = App::new(route, state);
    if let Some(link) = argument.filter(|arg| arg.contains("://")) {
        app.session.initial_link(link);
    }
    if cfg!(debug_assertions) && flags.gallery {
        let mut gallery = ui::gallery::Gallery::default();
        gallery.style = flags.style;
        gallery.custom = flags.custom;
        gallery.still = flags.still;
        gallery.scroll = flags.scroll;
        match flags.scene.as_deref() {
            Some("dialog") => {
                gallery.update(ui::gallery::Message::Dialog(true), false);
            },
            Some("sheet") => {
                gallery.update(ui::gallery::Message::Sheet(true), false);
            },
            Some("toast") => {
                gallery.update(ui::gallery::Message::Toast, false);
            },
            _ => {},
        }
        app.gallery = Some(gallery);
    }
    #[cfg(debug_assertions)]
    if let Some(path) = &flags.snapshot {
        app.session.system_dark = !flags.light;
        if app.gallery.is_none()
            && let Some(scene) = &flags.scene
        {
            if let Err(error) = runtime.block_on(app.session.preview(scene)) {
                tracing::error!("can't prepare the snapshot: {error:#}");
                return ExitCode::FAILURE;
            }
            if matches!(scene.as_str(), "settings" | "credits") {
                app.history = route::History::new(Route::Settings);
            }
            if scene == "credits" {
                app.overlay = Some(app::Overlay::Credits);
            }
            if scene == "account" {
                app.overlay = Some(app::Overlay::Account);
            }
        }
        return match snapshot::snapshot(&mut app, flags.size, flags.scale, flags.pointer, path, &flags.input()) {
            Ok(messages) => {
                tracing::debug!(?messages, "snapshot input");
                ExitCode::SUCCESS
            },
            Err(error) => {
                tracing::error!("can't take the snapshot: {error:#}");
                ExitCode::FAILURE
            },
        };
    }
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            tracing::error!("can't connect to the display: {error}");
            return ExitCode::FAILURE;
        },
    };

    let proxy = event_loop.create_proxy();
    let sender = proxy.clone();
    runtime.spawn(platform::Current::watch_button_layout(Box::new(move |layout| {
        let _ = sender.send_event(UserEvent::Message(Message::ButtonLayout(layout)));
    })));
    let sender = proxy.clone();
    runtime.spawn(platform::Current::watch_reduced_motion(Box::new(move |reduce| {
        let _ = sender.send_event(UserEvent::Message(Message::ReducedMotion(reduce)));
    })));

    let mut runner = Runner::new(app, settings, proxy, runtime.handle().clone());
    match event_loop.run_app(&mut runner) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        },
    }
}

/// What the command line asks for: a route or link, and the development switches.
struct Flags {
    argument: Option<String>,
    /// `--gallery`: the design system's gallery instead of the app.
    gallery: bool,
    /// `--style flat|glass` and `--custom`, for the gallery.
    style: ui::Style,
    custom: bool,
    light: bool,
    still: Option<bool>,
    scroll: f32,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    scene: Option<String>,
    /// `--snapshot <file.png>`, with `--size <w>x<h>`, `--scale <factor>` and `--pointer <x>,<y>`.
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    snapshot: Option<std::path::PathBuf>,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    size: iced_core::Size<u32>,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    scale: f32,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pointer: Option<iced_core::Point>,
    click: bool,
    key: Option<String>,
}

impl Flags {
    #[cfg(debug_assertions)]
    fn input(&self) -> Vec<iced_core::Event> {
        use iced_core::{Event, keyboard, mouse};
        let mut events = Vec::new();
        if self.click {
            events.extend([
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ]);
        }
        if let Some(key) = &self.key {
            let named = match key.as_str() {
                "Escape" => keyboard::key::Named::Escape,
                "Tab" => keyboard::key::Named::Tab,
                "Space" => keyboard::key::Named::Space,
                _ => keyboard::key::Named::Enter,
            };
            events.push(Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(named),
                modified_key: keyboard::Key::Named(named),
                physical_key: keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text: None,
                repeat: false,
            }));
        }
        events
    }

    fn parse() -> Self {
        let mut flags = Self {
            argument: None,
            gallery: false,
            style: ui::Style::Layered,
            custom: false,
            light: false,
            still: None,
            scroll: 0.0,
            scene: None,
            snapshot: None,
            size: iced_core::Size::new(1280, 800),
            scale: 1.0,
            pointer: None,
            click: false,
            key: None,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--gallery" => flags.gallery = true,
                "--click" => flags.click = true,
                "--key" => flags.key = args.next(),
                "--custom" => flags.custom = true,
                "--light" => flags.light = true,
                "--reduce-motion" => flags.still = Some(true),
                "--scroll" => flags.scroll = args.next().and_then(|s| s.parse().ok()).unwrap_or(0.0),
                "--scene" => flags.scene = args.next(),
                "--style" => {
                    flags.style = match args.next().as_deref() {
                        Some("flat") => ui::Style::Flat,
                        Some("glass") => ui::Style::Glass,
                        _ => ui::Style::Layered,
                    }
                },
                "--snapshot" => flags.snapshot = args.next().map(Into::into),
                "--size" => {
                    if let Some((w, h)) = args.next().as_deref().and_then(|size| size.split_once('x')) {
                        flags.size = iced_core::Size::new(w.parse().unwrap_or(1280), h.parse().unwrap_or(800));
                    }
                },
                "--scale" => flags.scale = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0),
                "--pointer" => {
                    flags.pointer = args
                        .next()
                        .as_deref()
                        .and_then(|p| p.split_once(','))
                        .and_then(|(x, y)| Some(iced_core::Point::new(x.parse().ok()?, y.parse().ok()?)))
                },
                _ => flags.argument = Some(arg),
            }
        }
        flags
    }
}
