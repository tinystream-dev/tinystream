// SPDX-License-Identifier: AGPL-3.0-or-later

//! # tinystream desktop
//!
//! The native tinystream client: the web UI's desktop layout in iced, with video through libmpv and
//! subtitles through rustyass.

mod app;
mod chrome;
mod keys;
mod paths;
mod platform;
mod route;
mod runner;
mod shell;
mod style;
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

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!("can't start the async runtime: {error}");
            return ExitCode::FAILURE;
        },
    };
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

    // A path like `/title/12` opens that screen, as web links will (#92).
    let route = std::env::args().nth(1).and_then(|path| Route::parse(&path)).unwrap_or(Route::Home);
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

    let mut runner = Runner::new(App::new(route, state), settings, proxy);
    match event_loop.run_app(&mut runner) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        },
    }
}
