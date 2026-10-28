// SPDX-License-Identifier: AGPL-3.0-or-later

//! A local-file playback diagnostic using the app's mpv configuration and shared GPU surface.

use std::path::Path;
use std::sync::Arc;

use iced_core::Length::Fill;
use iced_core::keyboard;
use iced_widget::{Element, Stack, Widget, column, container, mouse_area, row, space};

use super::engine::{Engine, Update};
use crate::app::{Effect, Message as AppMessage};
use crate::ui::Look;
use crate::ui::controls::button;
use crate::ui::text::{self as type_, text};
use crate::ui::video::{Screen, video};

#[derive(Debug, Clone)]
pub enum Message {
    Mpv(Update),
    Toggle,
    Seek(f64),
    Volume(f64),
    Mute,
    Fullscreen,
    Close,
}

pub struct Viewer {
    engine: Engine,
    screen: Arc<Screen>,
    name: String,
    aspect: Option<f32>,
    time: f64,
    duration: f64,
    paused: bool,
    volume: f64,
    error: Option<String>,
}

impl Viewer {
    pub fn open(path: &Path, effects: &mut Vec<Effect>) -> Result<Self, String> {
        let engine = Engine::local()?;
        let screen = Screen::new(engine.mpv.clone());
        let listener = engine.listener();
        effects.push(Effect::Listen(Box::new(move |send| {
            listener(Box::new(move |update| send(AppMessage::LocalVideo(Message::Mpv(update)))));
        })));
        let file = url::Url::from_file_path(path).map_err(|_| "invalid local video path")?;
        engine.command(&["loadfile", file.as_str()]);
        let volume = engine.mpv.get_property::<f64>("volume").unwrap_or(100.0) / 100.0;
        Ok(Self {
            engine,
            screen,
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            aspect: None,
            time: 0.0,
            duration: 0.0,
            paused: false,
            volume,
            error: None,
        })
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Mpv(Update::Size(w, h)) if w > 0 && h > 0 => self.aspect = Some(w as f32 / h as f32),
            Message::Mpv(Update::Time(at)) => self.time = at,
            Message::Mpv(Update::Paused(paused)) => self.paused = paused,
            Message::Mpv(Update::Loaded(_)) => {
                self.duration = self.engine.mpv.get_property::<f64>("duration").unwrap_or(0.0);
            },
            Message::Mpv(Update::Failed(error)) => self.error = Some(error),
            Message::Toggle => self.engine.set("pause", !self.paused),
            Message::Seek(by) => self.engine.seek((self.time + by).clamp(0.0, self.duration.max(self.time))),
            Message::Volume(by) => {
                self.volume = (self.volume + by).clamp(0.0, 1.0);
                self.engine.set("volume", self.volume * 100.0);
            },
            Message::Mute => self.engine.command(&["cycle", "mute"]),
            _ => {},
        }
    }

    pub fn key(&self, key: &keyboard::Key) -> Option<Message> {
        use keyboard::Key;
        use keyboard::key::Named;
        Some(match key.as_ref() {
            Key::Named(Named::Space) | Key::Character("k" | "K") => Message::Toggle,
            Key::Named(Named::ArrowLeft) | Key::Character("j" | "J") => Message::Seek(-10.0),
            Key::Named(Named::ArrowRight) | Key::Character("l" | "L") => Message::Seek(10.0),
            Key::Named(Named::ArrowUp) => Message::Volume(0.05),
            Key::Named(Named::ArrowDown) => Message::Volume(-0.05),
            Key::Character("m" | "M") => Message::Mute,
            Key::Character("f" | "F") => Message::Fullscreen,
            Key::Named(Named::Escape) => Message::Close,
            _ => return None,
        })
    }

    pub fn view<'a>(&'a self, look: &Look) -> Element<'a, Message> {
        let picture = mouse_area(video(&self.screen, self.aspect))
            .on_press(Message::Toggle)
            .on_double_click(Message::Fullscreen)
            .boxed();
        let mut layers = vec![picture];
        if let Some(error) = self.error.clone().or_else(|| self.screen.failure()) {
            layers.push(container(text(error, type_::S15, look.media_ink)).center(Fill).boxed());
        }
        let controls = row![
            button(if self.paused { "Play" } else { "Pause" }, look).on_press(Message::Toggle).boxed(),
            button("−10 s", look).on_press(Message::Seek(-10.0)).boxed(),
            button("+10 s", look).on_press(Message::Seek(10.0)).boxed(),
            text(format!("{:.0} / {:.0} s", self.time, self.duration), type_::S13, look.media_ink),
            space().width(Fill),
            button("Fullscreen (F)", look).on_press(Message::Fullscreen).boxed(),
        ]
        .spacing(12)
        .align_y(iced_core::Alignment::Center);
        layers.push(
            column![
                container(text(&self.name, type_::S15, look.media_ink)).padding(20),
                space().height(Fill),
                container(controls).padding(20),
            ]
            .boxed(),
        );
        Stack::with_children(layers).width(Fill).height(Fill).boxed()
    }
}
