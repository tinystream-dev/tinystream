// SPDX-License-Identifier: AGPL-3.0-or-later

//! The video player (#86). The web's `Player.tsx` is the spec: it does what that does, the same
//! way, with mpv playing the file as it is unless a quality is picked.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use iced_core::keyboard::{Key, Modifiers, key};
use iced_widget::image::Handle;

use crate::api::{self, Api, playback, player_overview, player_schedule, save_progress, take_screenshot};
use crate::app::{Effect, Message as AppMessage};
use crate::ui::video::Screen;

mod clip;
mod engine;
pub mod stream;
mod timeline;
mod tracks;
mod view;

pub use engine::config_file;
use engine::{Engine, Source, Update};
use tracks::Prefs;
pub use view::{pip_view, view};

/// How long the controls stay after the pointer stops.
const IDLE: Duration = Duration::from_millis(2600);
/// Paused and left alone this long, the frame recedes.
const REST: Duration = Duration::from_millis(2400);
/// How long a skip button shows on its own as its chapter starts.
const SKIP_FRESH: Duration = Duration::from_secs(6);
/// How long the HUD shows what a key did.
const HUD: Duration = Duration::from_millis(700);
/// Seconds before the next episode starts by itself.
const COUNTDOWN: u64 = 5;
/// How often progress is saved while playing.
const SAVE: Duration = Duration::from_secs(10);
/// Tries at playing on from where it was after the stream breaks, waiting 1, 2, 4… seconds.
const RETRIES: u32 = 5;
/// The server's preview frames come in steps this long (`PREVIEW_STEP`).
const PREVIEW_STEP: u32 = 10;
/// An end of stream this far before the stated duration still counts as the end (#68).
const END_SLACK: f64 = 15.0;

/// The picture's quality: the file as it is, or converted to a height (the web's menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Auto,
    Height(u32),
}

pub const QUALITIES: [Quality; 4] = [Quality::Auto, Quality::Height(1080), Quality::Height(720), Quality::Height(480)];
pub const RATES: [f64; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];

/// What a key or gesture just did, shown briefly (`Hud`).
#[derive(Debug, Clone, PartialEq)]
pub enum HudKind {
    Seek(f64),
    Volume(f64),
    Play,
    Pause,
    Speed(String),
    Notice(String),
}

#[derive(Debug, Clone)]
pub struct Hud {
    pub kind: HudKind,
    pub at: Instant,
}

/// A screenshot being taken: being taken, saved, or why it wasn't (`Shot`).
#[derive(Debug, Clone)]
pub struct Shot {
    pub key: u64,
    pub at: Instant,
    pub clip: Option<(i64, Option<String>)>,
    pub error: Option<String>,
    pub done: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Tracks,
    Quality,
}

/// Where the pointer is over the timeline: the time there, and how far along it is, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    pub at: f64,
    pub x: f32,
    pub width: f32,
}

#[derive(Clone)]
pub enum Message {
    Info(Result<Arc<playback::ResponseData>, api::Error>),
    Mpv(Update),
    Tick,
    Picture(String, Option<Handle>),
    Preview(u32, Option<Handle>),
    Overview(Option<String>),
    Series(Option<Arc<player_schedule::PlayerScheduleTitleSeries>>),
    Shot(u64, Result<(i64, Option<String>), String>),
    Saved,
    Poke,
    Toggle,
    Seek(f64),
    SeekBy(f64),
    Volume(f64),
    Mute,
    Audio(i64),
    Subtitle(Option<String>),
    Quality(Quality),
    Rate(f64),
    Menu(Option<Menu>),
    Hover(Option<Hover>),
    Drag(f64),
    VolumeHover(bool),
    Fullscreen,
    PictureInPicture,
    PipWindow(crate::chrome::Action),
    PipFailed(String),
    Back,
    Go(i64),
    Skip,
    Screenshot,
    OpenShot,
    MakeClip,
    Editor(clip::Message),
    Retry,
    Convert,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PlayerMessage")
    }
}

/// What the player asks of the rest of the app.
#[derive(Debug, Clone)]
pub enum Out {
    /// Leave, back to where it was opened from, or the title.
    Back {
        title: Option<i64>,
    },
    /// Play this video instead, in place of this one in history.
    Go(i64),
    Fullscreen,
    PictureInPicture(bool),
    PipWindow(crate::chrome::Action),
    /// Open a clip in Clips.
    Clip(i64),
}

pub struct Player {
    /// Tells this player's messages from an earlier one's.
    pub serial: u64,
    pub id: i64,
    api: Api,
    engine: Option<Engine>,
    pub screen: Option<Arc<Screen>>,
    pub pb: Option<playback::Playback>,
    pub info_error: Option<String>,
    vaapi: bool,
    pub can_clip: bool,
    /// Whether the window is full screen, for the button.
    pub fullscreen: bool,
    pub pip: bool,
    prefs: Prefs,
    start_at: Option<f64>,
    base: String,

    pub source: Source,
    pub quality: Quality,
    pub audio: Option<i64>,
    pub subtitle: Option<String>,
    /// mpv's audio tracks as (its id, FFmpeg's stream index).
    tracks: Vec<(i64, i64)>,
    pub playing: bool,
    pub time: f64,
    pub cached: f64,
    pub waiting: bool,
    pub started: bool,
    pub ended: bool,
    pub error: Option<String>,
    pub volume: f64,
    pub muted: bool,
    pub rate: f64,
    pub aspect: Option<f32>,
    /// Where playback was meant to be when the stream was last (re)built.
    resume: (f64, bool),
    retries: u32,
    retry_at: Option<Instant>,

    pub idle_since: Option<Instant>,
    pub hud: Option<Hud>,
    pub shot: Option<Shot>,
    pub clipping: Option<clip::Editor>,
    shots: u64,
    pub menu: Option<Menu>,
    pub hover: Option<Hover>,
    pub drag: Option<f64>,
    pub volume_open: bool,
    pub countdown: Option<Instant>,
    pub skip_fresh: Option<(f64, Instant)>,
    pub resting_since: Option<Instant>,
    /// When the frame started receding or coming back, and which.
    pub rested: Option<(bool, Instant)>,
    pub overview: Option<String>,
    overview_asked: bool,
    pub series: Option<Arc<player_schedule::PlayerScheduleTitleSeries>>,
    series_asked: bool,
    pub pictures: HashMap<String, Handle>,
    pub previews: HashMap<u32, Handle>,
    preview_wanted: Option<(u32, Instant)>,
    previews_asked: Vec<u32>,
    pub preview_shown: Option<u32>,
    previews_failed: bool,
    last_save: Instant,
}

impl Player {
    /// Opens video `id` (from `start_at` seconds, or where it was left), for user `user` of the
    /// saved server `server`.
    pub fn new(
        serial: u64,
        id: i64,
        start_at: Option<f64>,
        api: Api,
        (server, user): (u64, i64),
        runtime: Option<tokio::runtime::Handle>,
        effects: &mut Vec<Effect>,
    ) -> Self {
        let prefs = Prefs::load(server, user);
        Self::create(serial, id, start_at, api, runtime, prefs, effects)
    }

    fn create(
        serial: u64,
        id: i64,
        start_at: Option<f64>,
        api: Api,
        runtime: Option<tokio::runtime::Handle>,
        prefs: Prefs,
        effects: &mut Vec<Effect>,
    ) -> Self {
        let engine = match runtime {
            Some(runtime) => {
                Engine::new(api.clone(), runtime).inspect_err(|error| tracing::error!("can't start mpv: {error}")).ok()
            },
            None => None,
        };
        let mut player = Self {
            serial,
            id,
            base: format!("api/media/{id}"),
            screen: engine.as_ref().map(|engine| Screen::new(engine.mpv.clone())),
            error: engine.is_none().then(|| "the video player couldn't start".to_owned()),
            engine,
            api: api.clone(),
            pb: None,
            info_error: None,
            vaapi: false,
            can_clip: false,
            fullscreen: false,
            pip: false,
            volume: prefs.f64("volume").unwrap_or(1.0).clamp(0.0, 1.0),
            rate: prefs.f64("rate").unwrap_or(1.0).clamp(0.25, 3.0),
            prefs,
            start_at,
            source: Source::Direct,
            quality: Quality::Auto,
            audio: None,
            subtitle: None,
            tracks: Vec::new(),
            playing: false,
            time: 0.0,
            cached: 0.0,
            waiting: true,
            started: false,
            ended: false,
            muted: false,
            aspect: None,
            resume: (0.0, false),
            retries: 0,
            retry_at: None,
            idle_since: Some(Instant::now()),
            hud: None,
            shot: None,
            clipping: None,
            shots: 0,
            menu: None,
            hover: None,
            drag: None,
            volume_open: false,
            countdown: None,
            skip_fresh: None,
            resting_since: None,
            rested: None,
            overview: None,
            overview_asked: false,
            series: None,
            series_asked: false,
            pictures: HashMap::new(),
            previews: HashMap::new(),
            preview_wanted: None,
            previews_asked: Vec::new(),
            preview_shown: None,
            previews_failed: false,
            last_save: Instant::now(),
        };

        player.apply_volume();
        if let Some(engine) = &player.engine {
            engine.set("speed", player.rate);
            let listener = engine.listener();
            let alive = engine.alive();
            effects.push(Effect::Listen(Box::new(move |send| {
                let updates = send.clone();
                listener(Box::new(move |update| updates(AppMessage::Player(serial, Message::Mpv(update)))));
                let _ = std::thread::Builder::new().name("player clock".into()).spawn(move || {
                    while alive.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(250));
                        send(AppMessage::Player(serial, Message::Tick));
                    }
                });
            })));
        }
        player.request(effects, async move {
            Message::Info(api.request::<api::Playback>(playback::Variables { id }).await.map(Arc::new))
        });
        player
    }

    fn request(&self, effects: &mut Vec<Effect>, future: impl Future<Output = Message> + Send + 'static) {
        let serial = self.serial;
        effects.push(Effect::Request(Box::pin(async move { AppMessage::Player(serial, future.await) })));
    }

    /// Whether video playback should keep the desktop awake, including picture in picture.
    pub fn playback_active(&self) -> bool {
        self.playing && self.started && !self.ended && self.failure().is_none()
    }

    pub fn duration(&self) -> f64 {
        self.pb.as_ref().and_then(|pb| pb.media.duration).unwrap_or(0.0)
    }

    pub fn idle(&self) -> bool {
        self.idle_since.is_some_and(|since| since.elapsed() >= IDLE)
    }

    /// Whether the frame has receded behind what's playing (`rested`).
    pub fn is_rested(&self) -> bool {
        self.rested.is_some_and(|(rested, _)| rested)
    }

    pub fn show_chrome(&self) -> bool {
        self.menu.is_some()
            || self.drag.is_some()
            || self.volume_open
            || (!self.playing && !self.is_rested())
            || !self.idle()
            || self.failure().is_some()
    }

    /// Why the video can't play, if it can't.
    pub fn failure(&self) -> Option<String> {
        self.error.clone().or_else(|| self.info_error.clone()).or_else(|| {
            self.screen
                .as_ref()
                .and_then(|screen| screen.failure())
                .map(|why| format!("the video can't be shown here: {why}"))
        })
    }

    /// How much of the controls shows: they fade out over 300 ms as they go.
    pub fn chrome_alpha(&self) -> f32 {
        if self.show_chrome() {
            return 1.0;
        }
        let hidden = match (self.playing, self.idle_since, self.rested) {
            (true, Some(since), _) => since + IDLE,
            (false, _, Some((true, since))) => since,
            _ => return 0.0,
        };
        1.0 - crate::ui::motion::progress(hidden, Duration::from_millis(300), Instant::now())
    }

    /// How far the frame has receded behind what's playing, 0 to 1 (`.recede`, over 1.4 s).
    pub fn recede(&self) -> f32 {
        let Some((rested, since)) = self.rested else { return 0.0 };
        let t = crate::ui::motion::SETTLE.at(crate::ui::motion::progress(
            since,
            Duration::from_millis(1400),
            Instant::now(),
        ));
        if rested { t } else { 1.0 - t }
    }

    /// The chapter playing now.
    pub fn chapter(&self) -> Option<&playback::PlaybackMediaChapters> {
        let pb = self.pb.as_ref()?;
        pb.media.chapters.iter().find(|c| self.time >= c.start && self.time < c.end)
    }

    /// The chapter playing now, if it's one to skip and there's more than a moment of it left.
    pub fn skippable(&self) -> Option<&playback::PlaybackMediaChapters> {
        self.chapter().filter(|c| c.title.as_deref().is_some_and(skippable) && c.end - self.time > 3.0)
    }

    pub fn near_end(&self) -> bool {
        let duration = self.duration();
        self.pb.as_ref().is_some_and(|pb| pb.next.is_some())
            && duration > 0.0
            && (self.time > duration - 25.0
                || self
                    .chapter()
                    .and_then(|c| c.title.as_deref())
                    .is_some_and(|title| starts_with(title, &["ed", "ending", "credits"])))
    }

    pub fn show_skip(&self) -> bool {
        let start = self.skippable().map(|c| c.start);
        self.skip_fresh.is_some_and(|(at, since)| Some(at) == start && since.elapsed() < SKIP_FRESH)
            || self.show_chrome()
    }

    /// Seconds left before the next episode plays.
    pub fn countdown_left(&self) -> Option<u64> {
        self.countdown.map(|since| COUNTDOWN.saturating_sub(since.elapsed().as_secs()))
    }

    pub fn describe(&self) -> String {
        match self.quality {
            Quality::Auto => "Direct play".into(),
            Quality::Height(height) => {
                format!("Converting to {height}p ({})", if self.vaapi { "VA-API" } else { "software" })
            },
        }
    }

    /// Whether something on screen moves by itself, so frames have to keep coming.
    pub fn animating(&self) -> bool {
        self.hud.is_some()
            || self.shot.is_some()
            || self.countdown.is_some()
            || self.rested.is_some_and(|(_, since)| since.elapsed() < Duration::from_millis(1400))
            || (self.chrome_alpha() > 0.0 && self.chrome_alpha() < 1.0)
            || self.idle_since.is_some_and(|since| since.elapsed() < IDLE + Duration::from_millis(300))
            || (self.waiting && self.error.is_none())
            || !self.started
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) -> Option<Out> {
        match message {
            Message::Info(Ok(data)) => {
                let Some(pb) = data.video.clone() else {
                    self.info_error = Some("This video isn’t here anymore.".into());
                    return None;
                };
                self.vaapi = data.server.transcoding.vaapi.is_some();
                self.can_clip = data.server.clips && data.viewer.as_ref().is_some_and(|v| v.permissions.clip);
                self.audio = tracks::default_audio(&pb, &self.prefs);
                self.subtitle = tracks::default_subtitle(&pb, &self.prefs);
                let start = self.start_at.unwrap_or(match (pb.finished, pb.position) {
                    (Some(true), _) => 0.0,
                    (_, Some(position)) if position > 5.0 => position,
                    _ => 0.0,
                });
                let wanted: Vec<String> = [Some(pb.still.clone()), pb.next.as_ref().map(|next| next.still.clone())]
                    .into_iter()
                    .flatten()
                    .collect();
                self.pb = Some(pb);
                for path in wanted {
                    self.picture(path, effects);
                }
                self.play_from(start, false);
            },
            Message::Info(Err(error)) => self.info_error = Some(error.message),
            Message::Mpv(update) => return self.mpv(update, effects),
            Message::Tick => return self.tick(effects),
            Message::Picture(path, handle) => {
                if let Some(handle) = handle {
                    self.pictures.insert(path, handle);
                }
            },
            Message::Preview(at, handle) => match handle {
                Some(handle) => {
                    self.previews.insert(at, handle);
                    self.preview_shown = Some(at);
                },
                None => self.previews_failed = true,
            },
            Message::Overview(overview) => self.overview = overview,
            Message::Series(series) => self.series = series,
            Message::Shot(key, result) => {
                let shot = self.shot.as_mut().filter(|shot| shot.key == key)?;
                shot.done = Some(Instant::now());
                match result {
                    Ok(clip) => {
                        let poster = clip.1.clone();
                        shot.clip = Some(clip);
                        if let Some(poster) = poster {
                            self.picture(poster, effects);
                        }
                    },
                    Err(error) => shot.error = Some(error),
                }
            },
            Message::Saved => {},
            Message::Poke => self.poke(),
            Message::Toggle => {
                let playing = self.set_playing(None);
                if !playing {
                    self.save(effects);
                }
                self.flash(if playing { HudKind::Play } else { HudKind::Pause });
            },
            Message::Seek(to) => {
                self.drag = None;
                self.seek(to);
            },
            Message::SeekBy(by) => self.seek_by(by),
            Message::Volume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                self.muted = false;
                self.apply_volume();
            },
            Message::Mute => {
                self.muted = !self.muted;
                self.apply_volume();
            },
            Message::Audio(index) => {
                self.menu = None;
                self.choose_audio(index);
            },
            Message::Subtitle(id) => {
                self.menu = None;
                self.choose_subtitle(id);
            },
            Message::Quality(quality) => {
                self.menu = None;
                if quality != self.quality {
                    self.quality = quality;
                    self.rebuild();
                }
            },
            Message::Rate(rate) => self.change_rate(rate),
            Message::Menu(menu) => self.menu = menu,
            Message::Hover(hover) => {
                self.hover = hover;
                if let Some(hover) = hover {
                    if self.drag.is_some() {
                        self.drag = Some(hover.at);
                    }
                    let step = (hover.at / PREVIEW_STEP as f64).floor() as u32 * PREVIEW_STEP;
                    if self.preview_wanted.is_none_or(|(wanted, _)| wanted != step) {
                        self.preview_wanted = Some((step, Instant::now()));
                    }
                }
            },
            Message::Drag(at) => self.drag = Some(at),
            Message::VolumeHover(open) => self.volume_open = open,
            Message::PictureInPicture => {
                self.pip = !self.pip;
                return Some(Out::PictureInPicture(self.pip));
            },
            Message::PipWindow(action) => return Some(Out::PipWindow(action)),
            Message::PipFailed(error) => {
                self.pip = false;
                self.flash(HudKind::Notice(error));
            },
            Message::Fullscreen => return Some(Out::Fullscreen),
            Message::Back => {
                self.save(effects);
                return Some(Out::Back { title: self.pb.as_ref().map(|pb| pb.title.id) });
            },
            Message::Go(id) => {
                self.save(effects);
                return Some(Out::Go(id));
            },
            Message::Skip => {
                if let Some(end) = self.skippable().map(|c| c.end) {
                    self.seek(end);
                }
            },
            Message::MakeClip => {
                let pip = self.pip;
                self.pip = false;
                self.start_clip(effects);
                if pip {
                    return Some(Out::PictureInPicture(false));
                }
            },
            Message::Editor(message) => self.edit_clip(message, effects),
            Message::Screenshot => self.screenshot(effects),
            Message::OpenShot => {
                if let Some(id) = self.shot.take().and_then(|shot| shot.clip).map(|(id, _)| id) {
                    return Some(Out::Clip(id));
                }
            },
            Message::Retry => {
                if self.info_error.take().is_some() {
                    let (api, id) = (self.api.clone(), self.id);
                    self.request(effects, async move {
                        Message::Info(api.request::<api::Playback>(playback::Variables { id }).await.map(Arc::new))
                    });
                    return None;
                }
                self.error = None;
                self.retries = 0;
                self.rebuild();
            },
            Message::Convert => {
                self.error = None;
                self.retries = 0;
                self.quality = Quality::Height(720);
                self.rebuild();
            },
        }
        None
    }

    fn mpv(&mut self, update: Update, effects: &mut Vec<Effect>) -> Option<Out> {
        match update {
            Update::Time(at) => {
                if at > self.resume.0 + 10.0 {
                    self.retries = 0;
                }
                if self.drag.is_none() {
                    self.time = at;
                }
                if self.ended && at < self.duration() - END_SLACK {
                    self.ended = false;
                    self.countdown = None;
                }
                let start = self.skippable().map(|c| c.start);
                if let Some(start) = start
                    && self.skip_fresh.is_none_or(|(at, _)| at != start)
                {
                    self.skip_fresh = Some((start, Instant::now()));
                }
            },
            Update::Paused(paused) => {
                let was = self.playing;
                self.playing = !paused && !self.ended;
                if self.playing {
                    self.ended = false;
                    self.countdown = None;
                }
                if was && paused {
                    self.save(effects);
                }
            },
            Update::Waiting(waiting) => self.waiting = waiting,
            Update::Eof(true) if self.clipping.is_some() => {
                if self.playing
                    && let Some((start, _)) = self.clipping.as_ref().and_then(|editor| editor.range(self.duration()))
                {
                    self.seek(start);
                }
            },
            Update::Eof(true) => self.reached_end(effects),
            Update::Eof(false) => {},
            Update::Cached(at) => self.cached = at,
            Update::Size(width, height) => self.aspect = Some(width as f32 / height as f32),
            Update::Loaded(tracks) => {
                self.tracks = tracks;
                self.apply_subtitle();
                if self.source == Source::Direct
                    && let Some(id) = self.audio.and_then(|index| self.mpv_audio(index))
                    && let Some(engine) = &self.engine
                {
                    engine.set("aid", id);
                }
            },
            Update::Restarted => {
                self.waiting = false;
                self.started = true;
            },
            Update::Failed(why) => {
                if self.started && self.retries < RETRIES {
                    // Picked up again from where it got to, after a wait that grows.
                    self.retry_at = Some(Instant::now() + Duration::from_secs(1 << self.retries));
                    self.retries += 1;
                    self.waiting = true;
                    tracing::info!("playback broke off ({why}), trying again");
                } else {
                    self.error = Some(why);
                    self.waiting = false;
                }
            },
        }
        None
    }

    /// The stream ran out: the end, or (well before the stated duration) a break to recover from.
    fn reached_end(&mut self, effects: &mut Vec<Effect>) {
        let duration = self.duration();
        if duration <= 0.0 || self.time >= duration - END_SLACK {
            self.ended = true;
            self.playing = false;
            self.save(effects);
            if self.pb.as_ref().is_some_and(|pb| pb.next.is_some()) {
                self.countdown = Some(Instant::now());
            }
        } else if self.retries < RETRIES {
            tracing::info!("the stream ended at {:.1}s of {duration:.1}s, picking it up again", self.time);
            self.retries += 1;
            self.retry_at = Some(Instant::now() + Duration::from_secs(1 << (self.retries - 1)));
        } else {
            self.error = Some("the video stopped before its end".into());
        }
    }

    fn tick(&mut self, effects: &mut Vec<Effect>) -> Option<Out> {
        self.tick_clip(effects);
        let now = Instant::now();
        if self.hud.as_ref().is_some_and(|hud| now.duration_since(hud.at) >= HUD) {
            self.hud = None;
        }
        if let Some(shot) = &self.shot
            && let Some(done) = shot.done
            && now.duration_since(done) >= Duration::from_secs(if shot.error.is_some() { 6 } else { 4 })
        {
            self.shot = None;
        }
        if self.playing && now.duration_since(self.last_save) >= SAVE {
            self.save(effects);
        }
        if let Some(at) = self.retry_at
            && now >= at
        {
            self.retry_at = None;
            self.rebuild();
        }

        let resting = !self.playing
            && self.idle()
            && self.started
            && !self.ended
            && !self.waiting
            && self.error.is_none()
            && self.pb.is_some()
            && self.clipping.is_none();
        match (resting, self.resting_since) {
            (true, None) => self.resting_since = Some(now),
            (false, Some(_)) => self.resting_since = None,
            _ => {},
        }
        let rested = self.resting_since.is_some_and(|since| now.duration_since(since) >= REST);
        if rested != self.is_rested() && (rested || self.rested.is_some()) {
            self.rested = Some((rested, now));
        }
        if resting
            && !self.overview_asked
            && let Some(id) = self.pb.as_ref().map(|pb| pb.title.id)
        {
            self.overview_asked = true;
            let (api, video) = (self.api.clone(), self.id);
            self.request(effects, async move {
                let data =
                    api.request::<api::PlayerOverview>(player_overview::Variables { id, video_id: video }).await.ok();
                Message::Overview(
                    data.and_then(|d| d.video.and_then(|v| v.overview).or(d.title.and_then(|t| t.overview))),
                )
            });
        }

        if self.ended
            && !self.series_asked
            && let Some(id) = self
                .pb
                .as_ref()
                .filter(|pb| pb.next.is_none() && matches!(pb.title.kind, playback::TitleKind::SHOW))
                .map(|pb| pb.title.id)
        {
            self.series_asked = true;
            let api = self.api.clone();
            self.request(effects, async move {
                let data = api.request::<api::PlayerSchedule>(player_schedule::Variables { id }).await.ok();
                Message::Series(data.and_then(|d| d.title?.series).map(Arc::new))
            });
        }

        if let Some((step, since)) = self.preview_wanted
            && self.hover.is_some()
            && !self.previews_failed
            && now.duration_since(since) >= Duration::from_millis(60)
            && !self.previews_asked.contains(&step)
        {
            if self.previews.contains_key(&step) {
                self.preview_shown = Some(step);
            } else {
                self.previews_asked.push(step);
                let (api, path) = (self.api.clone(), format!("{}/preview/{step}", self.base));
                self.request(effects, async move {
                    let bytes = crate::images::fetch(&api, &path).await;
                    let handle = match bytes {
                        Some(bytes) => crate::images::decode(bytes).await,
                        None => None,
                    };
                    Message::Preview(step, handle)
                });
            }
        }

        if self.countdown_left() == Some(0)
            && let Some(next) = self.pb.as_ref().and_then(|pb| pb.next.as_ref()).map(|next| next.id)
        {
            self.countdown = None;
            self.save(effects);
            return Some(Out::Go(next));
        }
        None
    }

    /// mpv's id for the audio track the server calls `index`.
    fn mpv_audio(&self, index: i64) -> Option<i64> {
        self.tracks.iter().find(|(_, ff)| *ff == index).map(|(id, _)| *id)
    }

    fn audio_codec(&self) -> Option<&str> {
        let pb = self.pb.as_ref()?;
        let audio = pb.media.audio.iter().find(|a| Some(a.index) == self.audio)?;
        Some(audio.codec.as_str())
    }

    /// (Re)builds the stream for the quality and tracks picked, from `start`.
    fn play_from(&mut self, start: f64, paused: bool) {
        self.source = match self.quality {
            Quality::Auto => Source::Direct,
            Quality::Height(height) => Source::Transcode { height, start },
        };
        self.retry_at = None;
        self.resume = (start, paused);
        self.time = start;
        self.waiting = true;
        let path = self.source.path(&self.base, self.audio, self.audio_codec());
        if let Some(engine) = &self.engine {
            engine.load(&path, start, paused);
        }
    }

    /// Builds the stream again where playback is, keeping it paused or playing.
    fn rebuild(&mut self) {
        let (start, paused) = if self.started { (self.time, !self.playing && !self.ended) } else { self.resume };
        self.play_from(start, paused);
    }

    fn apply_volume(&mut self) {
        // mpv's volume is cubic; the web's (and this one's) is the plain amplitude.
        let volume = if self.muted { 0.0 } else { self.volume.cbrt() * 100.0 };
        if let Some(engine) = &self.engine {
            engine.set("volume", volume);
        }
        self.prefs.set("volume", self.volume.to_string());
    }

    pub fn poke(&mut self) {
        self.idle_since = Some(Instant::now());
    }

    fn flash(&mut self, kind: HudKind) {
        // Seeks the same way add up.
        let kind = match (&self.hud, kind) {
            (Some(Hud { kind: HudKind::Seek(before), .. }), HudKind::Seek(by)) if before.signum() == by.signum() => {
                HudKind::Seek(before + by)
            },
            (_, kind) => kind,
        };
        self.hud = Some(Hud { kind, at: Instant::now() });
    }

    /// Plays or pauses (toggles with `None`); returns whether it's now playing.
    fn set_playing(&mut self, play: Option<bool>) -> bool {
        let want = play.unwrap_or(!self.playing);
        if want && self.ended {
            self.seek(0.0);
        }
        if let Some(engine) = &self.engine {
            engine.set("pause", !want);
        }
        self.playing = want;
        want
    }

    fn seek(&mut self, to: f64) {
        let duration = self.duration();
        let to = to.max(0.0).min(if duration > 0.0 { duration } else { f64::INFINITY });
        self.time = to;
        self.poke();
        let Some(engine) = &self.engine else { return };
        match self.source {
            Source::Direct => engine.seek(to),
            // A transcode can't seek outside what's cached; a new one starts there instead.
            Source::Transcode { .. } => {
                if engine.seekable().iter().any(|&(start, end)| to >= start && to <= end) {
                    engine.seek(to);
                } else {
                    self.play_from(to, !self.playing);
                }
            },
        }
    }

    fn seek_by(&mut self, by: f64) {
        self.seek(self.time + by);
        self.flash(HudKind::Seek(by));
    }

    fn change_rate(&mut self, rate: f64) {
        let rate = (rate.clamp(0.25, 3.0) * 100.0).round() / 100.0;
        self.rate = rate;
        if let Some(engine) = &self.engine {
            engine.set("speed", rate);
        }
        self.prefs.set("rate", rate.to_string());
        self.flash(HudKind::Speed(format!("{}×", number(rate))));
    }

    fn choose_audio(&mut self, index: i64) {
        let Some(pb) = &self.pb else { return };
        let Some(track) = pb.media.audio.iter().find(|a| a.index == index) else { return };
        tracks::remember_audio(pb, track, &mut self.prefs);
        self.audio = Some(index);
        match self.source {
            Source::Direct => {
                if let (Some(engine), Some(id)) = (&self.engine, self.mpv_audio(index)) {
                    engine.set("aid", id);
                }
            },
            // The transcode carries only the track it was made with.
            Source::Transcode { .. } => self.rebuild(),
        }
    }

    fn choose_subtitle(&mut self, id: Option<String>) {
        let Some(pb) = &self.pb else { return };
        let track = id.as_ref().and_then(|id| pb.media.subtitles.iter().find(|s| &s.id == id));
        if id.is_some() && !track.is_some_and(tracks::usable) {
            return;
        }
        tracks::remember_subtitle(pb, track, &mut self.prefs);
        self.subtitle = id;
        self.apply_subtitle();
    }

    fn apply_subtitle(&self) {
        let index = self
            .pb
            .as_ref()
            .and_then(|pb| {
                pb.media.subtitles.iter().find(|s| Some(&s.id) == self.subtitle.as_ref() && tracks::bitmap(s))
            })
            .filter(|_| self.source == Source::Direct)
            .and_then(|s| s.id.strip_prefix('s')?.parse().ok());
        if let Some(engine) = &self.engine {
            engine.bitmap_subtitle(index);
        }
    }

    /// The next usable subtitle track, then off (`c`).
    fn cycle_subtitles(&mut self) {
        let Some(pb) = &self.pb else { return };
        let usable: Vec<&String> = pb.media.subtitles.iter().filter(|s| tracks::usable(s)).map(|s| &s.id).collect();
        let next = match usable.iter().position(|id| Some(*id) == self.subtitle.as_ref()) {
            Some(i) => usable.get(i + 1).map(|id| (*id).clone()),
            None => usable.first().map(|id| (*id).clone()),
        };
        self.choose_subtitle(next);
    }

    /// Saves where playback is, so pages behind the player and other devices pick it up.
    pub fn save(&mut self, effects: &mut Vec<Effect>) {
        self.last_save = Instant::now();
        let duration = self.duration();
        if self.clipping.is_some() || duration <= 0.0 || self.time < 1.0 || !self.started {
            return;
        }
        let (api, video_id, position) = (self.api.clone(), self.id, self.time);
        self.request(effects, async move {
            let result =
                api.request::<api::SaveProgress>(save_progress::Variables { video_id, position, duration }).await;
            if let Err(error) = result {
                tracing::debug!("can't save progress: {error}");
            }
            Message::Saved
        });
    }

    fn screenshot(&mut self, effects: &mut Vec<Effect>) {
        if !self.can_clip || self.pb.is_none() || self.shot.as_ref().is_some_and(|shot| shot.done.is_none()) {
            return;
        }
        self.shots += 1;
        let key = self.shots;
        self.shot = Some(Shot { key, at: Instant::now(), clip: None, error: None, done: None });
        let input = take_screenshot::NewScreenshot {
            video_id: self.id,
            at: self.time,
            subtitles: self.subtitle.clone(),
            room: self.room().map(str::to_owned),
        };
        let api = self.api.clone();
        self.request(effects, async move {
            let result = api
                .request::<api::TakeScreenshot>(take_screenshot::Variables { input })
                .await
                .map(|data| (data.take_screenshot.id, data.take_screenshot.poster))
                .map_err(|error| error.message);
            Message::Shot(key, result)
        });
    }

    fn picture(&mut self, path: String, effects: &mut Vec<Effect>) {
        if path.is_empty() || self.pictures.contains_key(&path) {
            return;
        }
        let api = self.api.clone();
        self.request(effects, async move {
            let handle = match crate::images::cached(&api, &path).await {
                Some(bytes) => crate::images::decode(bytes).await,
                None => match crate::images::fetch(&api, &path).await {
                    Some(bytes) => crate::images::decode(bytes).await,
                    None => None,
                },
            };
            Message::Picture(path, handle)
        });
    }

    fn room(&self) -> Option<&str> {
        self.base.strip_prefix("api/together/")?.split('/').next()
    }

    /// What a key press does here, as the web player's keys go; `None` leaves it to the shell.
    pub fn key(&mut self, key: &Key, modified: &Key, modifiers: Modifiers, fullscreen: bool) -> Option<Message> {
        if modifiers.control() || modifiers.alt() || modifiers.logo() {
            return None;
        }
        if self.clipping.is_some() {
            return match key.as_ref() {
                Key::Named(key::Named::Escape) => Some(Message::Editor(clip::Message::Close)),
                Key::Named(key::Named::Space) | Key::Character("k" | "K") => Some(Message::Toggle),
                Key::Character("i" | "I") => Some(Message::Editor(clip::Message::In)),
                Key::Character("o" | "O") => Some(Message::Editor(clip::Message::Out)),
                Key::Character(",") => Some(Message::Editor(clip::Message::Step(true))),
                Key::Character(".") => Some(Message::Editor(clip::Message::Step(false))),
                _ => None,
            };
        }
        if self.menu.is_some() {
            return matches!(key, Key::Named(key::Named::Escape)).then_some(Message::Menu(None));
        }
        let pb = self.pb.as_ref();
        let message = match (key.as_ref(), modified.as_ref()) {
            (Key::Named(key::Named::Space), _) | (Key::Character("k" | "K"), _) => Message::Toggle,
            (Key::Named(key::Named::ArrowLeft), _) => Message::SeekBy(-5.0),
            (Key::Named(key::Named::ArrowRight), _) => Message::SeekBy(5.0),
            (Key::Character("j" | "J"), _) => Message::SeekBy(-10.0),
            (Key::Character("l" | "L"), _) => Message::SeekBy(10.0),
            (Key::Named(key::Named::ArrowUp), _) => {
                let volume = (self.volume + 0.05).min(1.0);
                self.flash(HudKind::Volume(volume));
                Message::Volume(volume)
            },
            (Key::Named(key::Named::ArrowDown), _) => {
                let volume = (self.volume - 0.05).max(0.0);
                self.flash(HudKind::Volume(volume));
                let muted = self.muted;
                self.volume = volume;
                self.muted = muted;
                self.apply_volume();
                return Some(Message::Poke);
            },
            (Key::Character("m" | "M"), _) => {
                self.flash(HudKind::Volume(if self.muted { self.volume } else { 0.0 }));
                Message::Mute
            },
            (_, Key::Character("?")) => {
                self.poke();
                return None;
            },
            (_, Key::Character(">")) => Message::Rate(self.rate + 0.25),
            (_, Key::Character("<")) => Message::Rate(self.rate - 0.25),
            (_, Key::Character("S")) if self.can_clip => Message::Screenshot,
            (Key::Character("x" | "X"), _) if self.can_clip => Message::MakeClip,
            (Key::Character("p" | "P"), _) => Message::PictureInPicture,
            (Key::Character("f" | "F"), _) => Message::Fullscreen,
            (Key::Character("n" | "N"), _) if pb.is_some_and(|pb| pb.next.is_some()) => {
                Message::Go(pb.and_then(|pb| pb.next.as_ref()).map(|next| next.id)?)
            },
            (Key::Character("s"), _) if self.skippable().is_some() => Message::Skip,
            (Key::Character("c" | "C"), _) if pb.is_some() => {
                self.cycle_subtitles();
                return Some(Message::Poke);
            },
            (Key::Named(key::Named::Escape), _) if self.pip => Message::PictureInPicture,
            (Key::Named(key::Named::Escape), _) if fullscreen => Message::Fullscreen,
            (Key::Named(key::Named::Escape), _) => return Some(Message::Back),
            (Key::Character(digit), _) if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() => {
                let duration = self.duration();
                if duration <= 0.0 {
                    return None;
                }
                Message::Seek(duration * f64::from(digit.as_bytes()[0] - b'0') / 10.0)
            },
            _ => return None,
        };
        self.poke();
        Some(message)
    }
}

/// Chapters worth a skip button. Not "intro": releases often call the cold open that, and it's
/// part of the episode.
fn skippable(title: &str) -> bool {
    starts_with(title, &["op", "opening", "ed", "ending", "credits", "preview", "next episode", "recap"])
}

/// Whether `title` starts with one of `words` as a whole word, ignoring case (`/^(…)\b/i`).
fn starts_with(title: &str, words: &[&str]) -> bool {
    let title = title.to_lowercase();
    words.iter().any(|word| {
        title
            .strip_prefix(word)
            .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_'))
    })
}

/// The skip button's words (`skipLabel`).
pub fn skip_label(title: &str) -> &'static str {
    let title = title.to_lowercase();
    if title.starts_with("op") {
        "Skip opening"
    } else if title.starts_with("ed") || title.starts_with("ending") || title.starts_with("credits") {
        "Skip credits"
    } else if title.starts_with("recap") {
        "Skip recap"
    } else {
        "Skip preview"
    }
}

/// When something airs, from a Unix time: "Today at 17:30", "Wednesday at 17:30", "Oct 14 at
/// 17:30" (`airs` in `downloads.ts`), in local time.
pub fn airs(unix: i64) -> String {
    use crate::platform::Desktop;

    const DAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let local = |at: i64| at + crate::platform::Current::utc_offset(at);
    let (day, today) = (local(unix).div_euclid(86_400), local(now).div_euclid(86_400));
    let seconds = local(unix).rem_euclid(86_400);
    let at = format!("{}:{:02}", seconds / 3600, (seconds % 3600) / 60);
    let (year, month, date) = civil(day);
    match day - today {
        0 => format!("Today at {at}"),
        1 => format!("Tomorrow at {at}"),
        -1 => format!("Yesterday at {at}"),
        2..=6 => format!("{} at {at}", DAYS[day.rem_euclid(7) as usize]),
        days if days.abs() > 180 => format!("{} {date}, {year}", MONTHS[month - 1]),
        _ => format!("{} {date} at {at}", MONTHS[month - 1]),
    }
}

/// The year, month and day of `days` since 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, usize, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let date = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month as usize, date)
}

/// A number as JavaScript writes it: no trailing zeros.
pub fn number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn player() -> Player {
        use playback::*;

        let mut player = Player::create(
            1,
            1,
            None,
            Api::new("http://127.0.0.1/".parse().unwrap(), None).unwrap(),
            None,
            Prefs::default(),
            &mut Vec::new(),
        );
        player.error = None;
        player.started = true;
        player.playing = true;
        player.waiting = false;
        player.pb = Some(Playback {
            id: 1,
            still: String::new(),
            label: Some("S01E01".into()),
            name: Some("First episode".into()),
            position: None,
            finished: None,
            title: PlaybackTitle { id: 7, kind: TitleKind::SHOW, name: "Test show".into(), backdrop: None },
            previous: None,
            next: Some(PlaybackNext {
                id: 2,
                still: String::new(),
                label: Some("S01E02".into()),
                name: Some("Next episode".into()),
            }),
            media: PlaybackMedia {
                duration: Some(1800.0),
                video: None,
                audio: Vec::new(),
                subtitles: Vec::new(),
                fonts: Vec::new(),
                chapters: Vec::new(),
            },
        });
        player
    }

    #[test]
    #[ignore = "requires Vulkan; renders and clicks the player headlessly"]
    fn player_chrome_and_menus_render_and_accept_input() {
        use iced_core::{Event, Point, Size, mouse};

        let dir = crate::test_support::Directory::new("player-render");
        let output = std::env::var_os("TINYSTREAM_TEST_ARTIFACT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| dir.0.clone());
        std::fs::create_dir_all(&output).unwrap();
        let mut app =
            crate::app::App::new(crate::route::Route::Watch(1, None), crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        let mut player = player();
        player.started = false;
        player.time = 420.0;
        player.can_clip = true;
        let bars = image::RgbaImage::from_fn(640, 360, |x, y| {
            let colors = [
                [180, 180, 180],
                [180, 180, 16],
                [16, 180, 180],
                [16, 180, 16],
                [180, 16, 180],
                [180, 16, 16],
                [16, 16, 180],
            ];
            let color = colors[(x as usize * colors.len() / 640).min(colors.len() - 1)];
            image::Rgba([color[0], color[1], color[2], if y < 300 { 255 } else { 180 }])
        });
        player.pictures.insert(String::new(), Handle::from_rgba(640, 360, bars.into_raw()));
        app.player = Some(player);
        let draw = |app: &mut crate::app::App, name: &str, cursor, input: &[Event]| {
            crate::snapshot::snapshot(app, Size::new(1280, 800), 1.0, cursor, &output.join(name), input).unwrap()
        };
        let messages = draw(
            &mut app,
            "controls.png",
            Some(Point::new(40.0, 764.0)),
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
        );
        assert!(messages.iter().any(|message| matches!(message, AppMessage::Player(_, Message::Toggle))));
        assert!(!app.player.as_ref().unwrap().playing);
        app.player.as_mut().unwrap().menu = Some(Menu::Quality);
        draw(&mut app, "quality.png", None, &[]);
        app.player.as_mut().unwrap().menu = Some(Menu::Tracks);
        draw(&mut app, "tracks.png", None, &[]);
        let player = app.player.as_mut().unwrap();
        player.menu = None;
        player.rested = Some((true, Instant::now() - Duration::from_secs(2)));
        draw(&mut app, "resting.png", None, &[]);
        let player = app.player.as_mut().unwrap();
        player.ended = true;
        player.rested = None;
        player.pb.as_mut().unwrap().next = None;
        draw(&mut app, "finale.png", None, &[]);
        app.player.as_mut().unwrap().start_clip(&mut Vec::new());
        draw(&mut app, "editor.png", None, &[]);
    }

    #[test]
    fn early_eof_retries_are_bounded_and_never_advance_the_episode() {
        let mut player = player();
        player.time = 1.0;
        let mut effects = Vec::new();
        for _ in 0..RETRIES {
            player.update(Message::Mpv(Update::Eof(true)), &mut effects);
            assert!(!player.ended);
            assert!(player.countdown.is_none());
            assert!(player.retry_at.is_some());
            player.retry_at = Some(Instant::now());
            assert!(player.update(Message::Tick, &mut effects).is_none());
            player.update(Message::Mpv(Update::Loaded(Vec::new())), &mut effects);
            player.update(Message::Mpv(Update::Restarted), &mut effects);
        }
        player.update(Message::Mpv(Update::Eof(true)), &mut effects);
        assert!(player.error.is_some());
        assert!(!player.ended);
        assert!(player.countdown.is_none());
        player.time = 1795.0;
        player.update(Message::Mpv(Update::Eof(true)), &mut effects);
        assert!(player.ended);
        assert_eq!(player.countdown_left(), Some(COUNTDOWN));
    }

    #[test]
    fn track_menu_stays_visible_and_escape_dismisses_it() {
        let mut player = player();
        player.idle_since = Some(Instant::now() - IDLE * 2);
        assert!(!player.show_chrome());
        player.menu = Some(Menu::Tracks);
        assert!(player.show_chrome());
        assert!(matches!(
            player.key(&Key::Named(key::Named::Escape), &Key::Named(key::Named::Escape), Modifiers::empty(), false),
            Some(Message::Menu(None))
        ));
    }

    #[test]
    fn pausing_saves_the_current_position_and_duration() {
        crate::test_support::run(async {
            let (origin, request) =
                crate::test_support::http(1, |_| serde_json::json!({"data":{"saveProgress":{"id":1}}})).await;
            let mut player = player();
            player.api = Api::new(origin, None).unwrap();
            player.time = 123.0;
            let mut effects = Vec::new();
            player.update(Message::Toggle, &mut effects);
            assert!(effects.iter().any(|effect| matches!(effect, Effect::Request(_))), "pausing did not save progress");
            for effect in effects {
                if let Effect::Request(request) = effect {
                    request.await;
                }
            }
            let requests = request.await.unwrap();
            assert_eq!(requests[0].1["variables"], serde_json::json!({"videoId":1,"position":123.0,"duration":1800.0}));
        });
    }

    #[test]
    fn skips_openings_and_credits_but_not_intros() {
        assert!(skippable("Opening"));
        assert!(skippable("OP"));
        assert!(skippable("ED 2"));
        assert!(skippable("Next Episode Preview"));
        assert!(!skippable("Intro"));
        assert!(!skippable("Opportunity"));
        assert_eq!(skip_label("Opening"), "Skip opening");
        assert_eq!(skip_label("Ending"), "Skip credits");
        assert_eq!(skip_label("Recap"), "Skip recap");
        assert_eq!(skip_label("Preview"), "Skip preview");
    }

    #[test]
    fn dates_from_days() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_744), (2026, 10, 18));
        assert_eq!(civil(-1), (1969, 12, 31));
    }

    #[test]
    fn writes_rates_like_javascript() {
        assert_eq!(number(1.0), "1");
        assert_eq!(number(1.25), "1.25");
        assert_eq!(number(0.5), "0.5");
    }
}
