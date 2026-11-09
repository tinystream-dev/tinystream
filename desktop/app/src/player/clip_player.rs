// SPDX-License-Identifier: AGPL-3.0-or-later

//! A rendered clip, played through its own mpv with the player's controls, cut down to what a short
//! file needs (`ClipPlayer.tsx`): no tracks, quality, chapters or episodes.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use iced_core::Length::Fill;
use iced_core::keyboard::{Key, Modifiers, key};
use iced_core::{Alignment, Color, ContentFit, Padding, mouse};
use iced_widget::image::Handle;
use iced_widget::{Element, Row, Stack, Widget, column, container, image, mouse_area, row, space};

use super::engine::{Engine, Update};
use super::tracks::Prefs;
use super::view::{chrome_button, clock, downwards, hud_view, media_spinner};
use super::{Hud, HudKind, IDLE, timeline};
use crate::api::Api;
use crate::app::{Effect, Message as AppMessage};
use crate::ui::Look;
use crate::ui::icon::Icon;
use crate::ui::text::{self as type_, text};
use crate::ui::video::{Screen, video};

/// How long the controls stay after the pointer stops, here (`2000` in `ClipPlayer.tsx`).
const CLIP_IDLE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub enum Message {
    Mpv(Update),
    Tick,
    Toggle,
    Seek(f64),
    Drag(f64),
    SeekBy(f64),
    Volume(f64),
    Mute,
    VolumeHover(bool),
    Poke,
    Fullscreen,
    PictureInPicture,
}

/// What the clip player asks of whatever holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    Fullscreen,
    PictureInPicture,
}

pub struct ClipPlayer {
    engine: Option<Engine>,
    screen: Option<Arc<Screen>>,
    /// Where the volume is remembered; a preview that starts muted leaves it alone.
    prefs: Option<Prefs>,
    pub pip: bool,
    pub playing: bool,
    pub time: f64,
    pub duration: f64,
    cached: f64,
    pub waiting: bool,
    pub started: bool,
    pub error: Option<String>,
    pub volume: f64,
    pub muted: bool,
    aspect: Option<f32>,
    drag: Option<f64>,
    volume_open: bool,
    hud: Option<Hud>,
    idle_since: Instant,
}

impl ClipPlayer {
    /// Starts playing the file at `path` on the server (a clip's `file`), muted for a preview.
    /// `wrap` makes the player's messages the app's, from mpv's own threads.
    pub fn new(
        api: Api,
        runtime: Option<tokio::runtime::Handle>,
        path: &str,
        muted: bool,
        (server, user): (u64, i64),
        wrap: Arc<dyn Fn(Message) -> AppMessage + Send + Sync>,
        effects: &mut Vec<Effect>,
    ) -> Self {
        let engine = runtime.and_then(|runtime| {
            Engine::new(api, runtime).inspect_err(|error| tracing::error!("can't start mpv: {error}")).ok()
        });
        let prefs = Prefs::load(server, user);
        let volume = prefs.f64("volume").unwrap_or(1.0).clamp(0.0, 1.0);
        let mut player = Self {
            screen: engine.as_ref().map(|engine| Screen::new(engine.mpv.clone())),
            error: engine.is_none().then(|| "the video player couldn't start".to_owned()),
            engine,
            prefs: (!muted).then_some(prefs),
            pip: false,
            playing: true,
            time: 0.0,
            duration: 0.0,
            cached: 0.0,
            waiting: true,
            started: false,
            volume,
            muted,
            aspect: None,
            drag: None,
            volume_open: false,
            hud: None,
            idle_since: Instant::now(),
        };
        player.apply_volume();
        if let Some(engine) = &player.engine {
            // A clip plays once, and stays on its last frame.
            engine.load(path.trim_start_matches('/'), 0.0, false);
            let listener = engine.listener();
            let alive = engine.alive();
            effects.push(Effect::Listen(Box::new(move |send| {
                let updates = (send.clone(), wrap.clone());
                listener(Box::new(move |update| (updates.0)((updates.1)(Message::Mpv(update)))));
                let _ = std::thread::Builder::new().name("clip clock".into()).spawn(move || {
                    while alive.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(250));
                        send(wrap(Message::Tick));
                    }
                });
            })));
        }
        player
    }

    pub fn idle(&self) -> bool {
        self.idle_since.elapsed() >= CLIP_IDLE
    }

    pub fn show_chrome(&self) -> bool {
        !self.playing || !self.idle() || self.error.is_some() || self.drag.is_some() || self.volume_open
    }

    fn failure(&self) -> Option<String> {
        self.error.clone().or_else(|| self.screen.as_ref().and_then(|screen| screen.failure()))
    }

    /// Whether something on screen moves by itself, so frames have to keep coming.
    pub fn animating(&self) -> bool {
        self.failure().is_none()
            && (self.hud.is_some() || self.waiting || self.idle_since.elapsed() < IDLE + Duration::from_millis(300))
    }

    pub fn update(&mut self, message: Message) -> Option<Out> {
        match message {
            Message::Mpv(update) => match update {
                Update::Time(at) if self.drag.is_none() => self.time = at,
                Update::Paused(paused) => self.playing = !paused,
                Update::Waiting(waiting) => self.waiting = waiting,
                Update::Eof(true) => self.playing = false,
                Update::Cached(at) => self.cached = at,
                Update::Size(width, height) if width > 0 && height > 0 => {
                    self.aspect = Some(width as f32 / height as f32)
                },
                Update::Duration(length) => self.duration = length,
                Update::Restarted => {
                    self.waiting = false;
                    self.started = true;
                },
                Update::Failed(why) => {
                    self.error = Some(why);
                    self.waiting = false;
                },
                _ => {},
            },
            Message::Tick => {
                if self.hud.as_ref().is_some_and(|hud| hud.at.elapsed() >= super::HUD) {
                    self.hud = None;
                }
            },
            Message::Toggle => {
                let playing = self.toggle();
                self.flash(if playing { HudKind::Play } else { HudKind::Pause });
            },
            Message::Drag(at) => self.drag = Some(at),
            Message::Seek(to) => {
                self.drag = None;
                self.seek(to);
            },
            Message::SeekBy(by) => {
                self.seek(self.time + by);
                self.flash(HudKind::Seek(by));
            },
            Message::Volume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                self.muted = false;
                self.apply_volume();
            },
            Message::Mute => {
                self.muted = !self.muted;
                self.apply_volume();
            },
            Message::VolumeHover(open) => self.volume_open = open,
            Message::Poke => self.poke(),
            Message::Fullscreen => return Some(Out::Fullscreen),
            Message::PictureInPicture => {
                self.pip = !self.pip;
                return Some(Out::PictureInPicture);
            },
        }
        None
    }

    fn poke(&mut self) {
        self.idle_since = Instant::now();
    }

    fn flash(&mut self, kind: HudKind) {
        let kind = match (&self.hud, kind) {
            (Some(Hud { kind: HudKind::Seek(before), .. }), HudKind::Seek(by)) if before.signum() == by.signum() => {
                HudKind::Seek(before + by)
            },
            (_, kind) => kind,
        };
        self.hud = Some(Hud { kind, at: Instant::now() });
    }

    /// Plays or pauses; returns whether it's now playing. Played again from its end, it starts
    /// over.
    fn toggle(&mut self) -> bool {
        let want = !self.playing;
        if want && self.duration > 0.0 && self.time >= self.duration - 0.1 {
            self.seek(0.0);
        }
        if let Some(engine) = &self.engine {
            engine.set("pause", !want);
        }
        self.playing = want;
        want
    }

    fn seek(&mut self, to: f64) {
        let to = to.max(0.0).min(if self.duration > 0.0 { self.duration } else { f64::INFINITY });
        self.time = to;
        self.poke();
        if let Some(engine) = &self.engine {
            engine.seek(to);
        }
    }

    fn apply_volume(&mut self) {
        let volume = if self.muted { 0.0 } else { self.volume.cbrt() * 100.0 };
        if let Some(engine) = &self.engine {
            engine.set("volume", volume);
        }
        if let Some(prefs) = &mut self.prefs {
            prefs.set("volume", self.volume.to_string());
        }
    }

    /// What a key does here (`ClipPlayer.tsx`'s keys).
    pub fn key(&mut self, key: &Key, modifiers: Modifiers) -> Option<Message> {
        if modifiers.control() || modifiers.alt() || modifiers.logo() {
            return None;
        }
        let message = match key.as_ref() {
            Key::Named(key::Named::Space) | Key::Character("k" | "K") => Message::Toggle,
            Key::Named(key::Named::ArrowLeft) => Message::SeekBy(-5.0),
            Key::Named(key::Named::ArrowRight) => Message::SeekBy(5.0),
            Key::Character("m" | "M") => {
                self.flash(HudKind::Volume(if self.muted { self.volume } else { 0.0 }));
                Message::Mute
            },
            Key::Character("f" | "F") => Message::Fullscreen,
            Key::Character("p" | "P") if self.prefs.is_some() => Message::PictureInPicture,
            _ => return None,
        };
        self.poke();
        Some(message)
    }

    /// The clip filling what it's given, with `poster` until its first frame, and the controls
    /// along the bottom while they're wanted.
    pub fn view<'a>(&'a self, poster: Option<&Handle>, fullscreen: bool, look: &Look) -> Element<'a, Message> {
        let l = *look;
        let mut layers: Vec<Element<'a, Message>> = Vec::new();
        if let Some(screen) = &self.screen {
            layers.push(video(screen, self.aspect).radius(12.0).boxed());
        }
        if !self.started
            && let Some(poster) = poster
        {
            layers.push(image(poster.clone()).width(Fill).height(Fill).content_fit(ContentFit::Contain).boxed());
        }
        let failure = self.failure();
        if self.waiting && failure.is_none() {
            layers.push(container(media_spinner(look)).center(Fill).boxed());
        }
        if failure.is_some() {
            layers.push(
                container(text("This clip can't play here", type_::SM, l.media_ink.scale_alpha(0.7)))
                    .center(Fill)
                    .padding(24)
                    .boxed(),
            );
        }
        if let Some(hud) = &self.hud {
            layers.push(hud_view(&hud.kind, hud.at, look));
        }
        let chrome = self.show_chrome();
        if chrome {
            layers.push(self.controls(fullscreen, look));
        }
        let shade = l.media_shade;
        let stage = container(Stack::with_children(layers).width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(move |_| container::Style { background: Some(shade.into()), ..container::Style::default() });
        let stage =
            mouse_area(stage).on_press(Message::Toggle).on_double_click(Message::Fullscreen).on_move(|_| Message::Poke);
        if chrome { stage.boxed() } else { stage.interaction(mouse::Interaction::Hidden).boxed() }
    }

    fn controls<'a>(&'a self, fullscreen: bool, look: &Look) -> Element<'a, Message> {
        let ink = look.media_ink;
        let shown = self.drag.unwrap_or(self.time);
        let bar = timeline::timeline(
            shown,
            self.duration,
            (self.time.min(self.cached), self.cached),
            &[],
            ink,
            (|_| Message::Poke, Message::Drag, Message::Seek),
        );
        let glyph = if self.muted || self.volume == 0.0 {
            Icon::VolumeX
        } else if self.volume < 0.5 {
            Icon::Volume1
        } else {
            Icon::Volume2
        };
        let mut volume = Row::new().align_y(Alignment::Center);
        volume = volume.push(chrome_button((glyph, 20.0, false), "Mute (m)", Some(Message::Mute), false, 1.0, look));
        if self.volume_open {
            let value = if self.muted { 0.0 } else { self.volume };
            volume = volume.push(
                container(timeline::volume(value, ink, Message::Volume).boxed())
                    .width(80)
                    .padding(Padding { right: 4.0, ..Padding::ZERO })
                    .boxed(),
            );
        }
        let volume = mouse_area(volume).on_enter(Message::VolumeHover(true)).on_exit(Message::VolumeHover(false));
        let buttons = row![
            chrome_button(
                (if self.playing { Icon::Pause } else { Icon::Play }, 20.0, true),
                if self.playing { "Pause (k)" } else { "Play (k)" },
                Some(Message::Toggle),
                false,
                1.0,
                look,
            ),
            volume,
            container(
                row![
                    text(clock(self.time), type_::S13, ink.scale_alpha(0.8)),
                    text(format!("/ {}", clock(self.duration)), type_::S13, ink.scale_alpha(0.4)),
                ]
                .spacing(4)
            )
            .padding(Padding { left: 8.0, ..Padding::ZERO }),
            space().width(Fill),
            chrome_button(
                (Icon::Minimize, 20.0, false),
                "Picture in picture (p)",
                self.prefs.as_ref().map(|_| Message::PictureInPicture),
                false,
                1.0,
                look
            ),
            chrome_button(
                (if fullscreen { Icon::Minimize } else { Icon::Maximize }, 20.0, false),
                if fullscreen { "Exit full screen (f)" } else { "Full screen (f)" },
                Some(Message::Fullscreen),
                false,
                1.0,
                look,
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        let shade = look.media_shade;
        container(
            container(column![bar.boxed(), buttons].spacing(4))
                .width(Fill)
                .padding(Padding { top: 56.0, right: 12.0, bottom: 8.0, left: 12.0 })
                .style(move |_| container::Style {
                    background: Some(downwards(&[
                        (0.0, Color::TRANSPARENT),
                        (0.5, shade.scale_alpha(0.4)),
                        (1.0, shade.scale_alpha(0.8)),
                    ])),
                    ..container::Style::default()
                }),
        )
        .align_bottom(Fill)
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip_player() -> ClipPlayer {
        let api = Api::new("http://127.0.0.1/".parse().unwrap(), None).unwrap();
        let mut player = ClipPlayer::new(
            api,
            None,
            "/api/clips/1/video",
            true,
            (0, 0),
            Arc::new(|_| AppMessage::Quit),
            &mut Vec::new(),
        );
        player.error = None;
        player
    }

    #[test]
    fn picture_in_picture_keeps_playback_position_and_returns() {
        let mut player = clip_player();
        player.time = 4.25;
        assert_eq!(player.update(Message::PictureInPicture), Some(Out::PictureInPicture));
        assert!(player.pip);
        assert_eq!(player.time, 4.25);
        assert_eq!(player.update(Message::PictureInPicture), Some(Out::PictureInPicture));
        assert!(!player.pip);
        assert!(player.playing);
    }

    #[test]
    fn a_finished_clip_plays_again_from_the_start() {
        let mut player = clip_player();
        player.update(Message::Mpv(Update::Duration(12.0)));
        player.update(Message::Mpv(Update::Time(12.0)));
        player.update(Message::Mpv(Update::Eof(true)));
        assert!(!player.playing);
        player.update(Message::Toggle);
        assert!(player.playing);
        assert_eq!(player.time, 0.0);
    }

    #[test]
    fn a_muted_preview_never_changes_the_remembered_volume() {
        let mut player = clip_player();
        assert!(player.prefs.is_none());
        player.update(Message::Volume(0.3));
        assert!(!player.muted);
        assert_eq!(player.volume, 0.3);
        assert!(matches!(player.key(&Key::Character("f".into()), Modifiers::empty()), Some(Message::Fullscreen)));
        assert!(player.key(&Key::Character("f".into()), Modifiers::CTRL).is_none());
    }
}
