// SPDX-License-Identifier: AGPL-3.0-or-later

//! Making or changing a clip (`ClipEditor.tsx`): pick the range on a filmstrip, the tracks and the
//! quality, watch it loop in the player's own mpv as it'll come out (rustyass drawing the subtitles
//! it burns in), then send it off. Rendering happens on the server; its progress comes through the
//! `events` subscription.

use std::path::PathBuf;
use std::sync::Arc;

use iced_core::Length::Fill;
use iced_core::{Alignment, ContentFit, Padding};
use iced_widget::{Column, Element, Row, Stack, Widget, column, container, image, row, space};
use serde_json::json;

use super::clip_player::{self, ClipPlayer};
use super::strip::{self, Edge, Strip};
use super::{Player, Quality};
use crate::api;
use crate::app::Effect;
use crate::clips::{
    Allowance, BITMAP_SUBTITLES, CJK, Clip, Out, Person, Preset, bytes, call, clip_of, length, presets, space as gb,
    stamp, worst_size,
};
use crate::route::Route;
use crate::ui::Look;
use crate::ui::controls::{button, checkbox, icon_button, owned_input, panel, progress, spinner, toggle};
use crate::ui::icon::{Icon, icon};
use crate::ui::pressable::{Appearance, Status, pressable};
use crate::ui::select::select;
use crate::ui::squircle::{Paint, squircle};
use crate::ui::text::{self as type_, MEDIUM, SEMIBOLD, text, truncate};
use crate::ui::video::video;

/// How long a new clip is when nothing says otherwise.
const DEFAULT_LENGTH: f64 = 30.0;
const MIN_LENGTH: f64 = 0.5;
/// Frames across the filmstrip.
const TILES: usize = 8;

#[derive(Clone)]
pub enum Message {
    Close,
    Allowance(u64, Result<Allowance, String>),
    Editing(u64, Result<Option<Arc<Clip>>, String>),
    People(u64, Vec<Person>),
    Strip(Strip),
    /// Moves the edge last touched a frame, or a second, earlier (`-1`) or later (`1`).
    Nudge(f64, bool),
    Play,
    In,
    Out,
    Step(bool),
    Title(String),
    Audio(Option<i64>),
    Subtitle(Option<String>),
    Preset(usize),
    Public(bool),
    Pick(Person, bool),
    Picker(bool),
    Save,
    Saved(u64, Result<Arc<Clip>, String>),
    Status(i64, Result<Option<Arc<Clip>>, String>),
    CopyLink,
    Download,
    Downloaded(Result<Option<PathBuf>, String>),
    YourClips,
    Reel(i64, clip_player::Message),
}

/// The clip once it's been sent off, following its render (`Rendering`).
pub struct Made {
    pub id: i64,
    pub clip: Option<Arc<Clip>>,
    pub player: Option<ClipPlayer>,
    played: Option<Option<i64>>,
    pub copied: bool,
}

pub struct Editor {
    pub(super) key: u64,
    /// Where playback was when the editor opened, to go back to.
    pub at: f64,
    quality: Quality,
    audio_before: Option<i64>,
    subtitle_before: Option<String>,
    /// The clip being changed, once it's here (`None` while it's asked for, with `changing` set).
    pub editing: Option<Arc<Clip>>,
    changing: Option<i64>,
    pub you: Option<Allowance>,
    pub range: (f64, f64),
    /// The stretch of the video the filmstrip shows.
    pub view: (f64, f64),
    pub drag: Option<Edge>,
    edge: Edge,
    pub title: String,
    pub audio: Option<i64>,
    pub subtitle: Option<String>,
    pub presets: Vec<Preset>,
    choices: Vec<usize>,
    pub preset: usize,
    audio_options: Vec<Option<i64>>,
    subtitle_options: Vec<Option<String>>,
    pub public: bool,
    pub recipients: Vec<Person>,
    pub picking: bool,
    pub people: Vec<Person>,
    pub saving: bool,
    pub made: Option<Made>,
}

/// Whether a subtitle track can be burned in: text the server draws, or images (`burnable`).
pub fn burnable(track: &api::playback::PlaybackMediaSubtitles) -> bool {
    track.supported || BITMAP_SUBTITLES.contains(&track.codec.as_str())
}

impl Editor {
    fn new(player: &Player, key: u64, changing: Option<i64>) -> Self {
        let pb = player.pb.as_ref().expect("clip metadata");
        let video = pb.media.video.as_ref();
        let presets = presets(video.map(|v| v.width), video.map(|v| v.height), video.map(|v| v.fps));
        let audio_options =
            if pb.media.audio.is_empty() { vec![None] } else { pb.media.audio.iter().map(|a| Some(a.index)).collect() };
        let subtitle_options = std::iter::once(None)
            .chain(pb.media.subtitles.iter().filter(|s| burnable(s)).map(|s| Some(s.id.clone())))
            .collect();
        let subtitle =
            player.subtitle.clone().filter(|id| pb.media.subtitles.iter().any(|s| &s.id == id && burnable(s)));
        Self {
            key,
            at: player.time,
            quality: player.quality,
            audio_before: player.audio,
            subtitle_before: player.subtitle.clone(),
            editing: None,
            changing,
            you: None,
            range: (0.0, 0.0),
            view: (0.0, 0.0),
            drag: None,
            edge: Edge::End,
            title: String::new(),
            audio: player.audio,
            subtitle,
            choices: (0..presets.len()).collect(),
            presets,
            preset: 0,
            audio_options,
            subtitle_options,
            public: false,
            recipients: Vec::new(),
            picking: false,
            people: Vec::new(),
            saving: false,
            made: None,
        }
    }

    /// Whether everything the editor needs is here.
    pub fn ready(&self) -> bool {
        self.you.is_some() && self.changing.is_none()
    }

    /// The longest the clip may be.
    fn max(&self, duration: f64) -> f64 {
        let limit =
            self.you.as_ref().filter(|you| you.max_length > 0).map_or(f64::INFINITY, |you| you.max_length as f64);
        limit.min(if duration > 0.0 { duration } else { f64::INFINITY })
    }

    pub fn length(&self) -> f64 {
        self.range.1 - self.range.0
    }

    pub fn chosen(&self) -> Option<&Preset> {
        self.presets.get(self.preset).or(self.presets.first())
    }

    /// The stretch around `range` the filmstrip shows: four times it, a minute at least.
    fn around(range: (f64, f64), duration: f64) -> (f64, f64) {
        let span = duration.min(60f64.max((range.1 - range.0) * 4.0));
        let from = ((range.0 + range.1) / 2.0 - span / 2.0).min(duration - span).max(0.0);
        (from, from + span)
    }

    /// The filmstrip's frames' times, on the server's preview steps.
    pub fn tiles(&self) -> [u32; TILES] {
        let span = self.view.1 - self.view.0;
        std::array::from_fn(|i| {
            let at = self.view.0 + (i as f64 + 0.5) * span / TILES as f64;
            (at / f64::from(super::PREVIEW_STEP)).floor().max(0.0) as u32 * super::PREVIEW_STEP
        })
    }
}

impl Player {
    pub fn start_clip(&mut self, effects: &mut Vec<Effect>) {
        self.open_editor(None, effects);
    }

    /// Opens the editor on clip `id` of this video, to change it.
    pub fn edit_existing(&mut self, id: i64, effects: &mut Vec<Effect>) {
        self.open_editor(Some(id), effects);
    }

    fn open_editor(&mut self, changing: Option<i64>, effects: &mut Vec<Effect>) {
        if !self.can_clip || self.pb.is_none() || self.clipping.is_some() {
            return;
        }
        self.set_playing(Some(false));
        self.save(effects);
        self.shots += 1;
        let key = self.shots;
        let editor = Editor::new(self, key, changing);
        let (audio, subtitle) = (editor.audio, editor.subtitle.clone());
        self.clipping = Some(editor);
        self.preview_tracks(audio, subtitle, effects);
        self.menu = None;
        self.rested = None;
        self.resting_since = None;
        self.quality = Quality::Auto;
        let api = self.api.clone();
        self.request(effects, async move {
            let result = call::<api::ClipsAllowance, AllowanceData>(&api, json!({})).await;
            super::Message::Editor(Message::Allowance(key, result.map(|d| d.clip_allowance).map_err(|e| e.message)))
        });
        let api = self.api.clone();
        self.request(effects, async move {
            #[derive(serde::Deserialize)]
            struct Users {
                users: Vec<Person>,
            }
            let people = call::<api::ClipsPeople, Users>(&api, json!({})).await.map(|d| d.users).unwrap_or_default();
            super::Message::Editor(Message::People(key, people))
        });
        if let Some(id) = changing {
            let api = self.api.clone();
            self.request(effects, async move {
                let result = clip_of::<api::ClipsClip>(&api, json!({"id":id})).await.map_err(|e| e.message);
                super::Message::Editor(Message::Editing(key, result))
            });
        }
    }

    /// Once the editor has what it needs: the range, tracks and quality to start from, and the
    /// first frame of it.
    fn set_up_clip(&mut self, effects: &mut Vec<Effect>) {
        let duration = self.duration();
        let fps = self.fps();
        let Some(editor) = &mut self.clipping else { return };
        let max = editor.max(duration);
        let snap = |t: f64| (t * fps).round() / fps;
        editor.range = match &editor.editing {
            Some(clip) => (clip.start, clip.end.min(clip.start + max)),
            None => {
                let len = DEFAULT_LENGTH.min(max);
                // The moment just watched, usually; from the start when near it.
                let end = editor.at.max(len).min(if duration > 0.0 { duration } else { f64::INFINITY });
                (snap((end - len).max(0.0)), snap(end))
            },
        };
        editor.view = Editor::around(editor.range, duration);
        let start = editor.range.0;
        self.play_from(start, true);
        self.want_tiles(effects);
    }

    fn fps(&self) -> f64 {
        self.pb.as_ref().and_then(|pb| pb.media.video.as_ref()).map(|v| v.fps).filter(|fps| *fps > 0.0).unwrap_or(24.0)
    }

    pub fn edit_clip(&mut self, message: Message, effects: &mut Vec<Effect>) -> Option<Out> {
        if matches!(message, Message::Close) {
            self.close_clip(effects);
            return None;
        }
        let duration = self.duration();
        let fps = self.fps();
        let snap = |t: f64| (t * fps).round() / fps;
        let room = self.room().map(str::to_owned);
        let editor = self.clipping.as_mut()?;
        match message {
            Message::Allowance(key, result) if key == editor.key => match result {
                Ok(you) => {
                    editor.you = Some(you);
                    if editor.ready() {
                        self.set_up_clip(effects);
                    }
                },
                Err(error) => {
                    self.close_clip(effects);
                    return Some(Out::Toast(error, None, crate::ui::overlay::Tone::Danger));
                },
            },
            Message::Editing(key, result) if key == editor.key => match result {
                Ok(Some(clip)) => {
                    editor.changing = None;
                    editor.title = clip.name.clone();
                    editor.public = clip.public;
                    editor.audio = clip.audio;
                    let pb = self.pb.as_ref()?;
                    editor.subtitle = clip
                        .subtitles
                        .clone()
                        .filter(|id| pb.media.subtitles.iter().any(|s| &s.id == id && burnable(s)));
                    editor.preset = editor
                        .presets
                        .iter()
                        .position(|p| p.height == clip.quality.height && p.half_rate == clip.quality.half_rate)
                        .unwrap_or(0);
                    editor.editing = Some(clip);
                    let (audio, subtitle) = (editor.audio, editor.subtitle.clone());
                    let ready = editor.ready();
                    self.preview_tracks(audio, subtitle, effects);
                    if ready {
                        self.set_up_clip(effects);
                    }
                },
                Ok(None) => {
                    self.close_clip(effects);
                    return Some(Out::Toast(
                        "That clip isn’t here anymore.".into(),
                        None,
                        crate::ui::overlay::Tone::Danger,
                    ));
                },
                Err(error) => {
                    self.close_clip(effects);
                    return Some(Out::Toast(error, None, crate::ui::overlay::Tone::Danger));
                },
            },
            Message::People(key, people) if key == editor.key => {
                for path in people.iter().filter_map(|p| p.avatar.clone()) {
                    self.picture(path, effects);
                }
                let editor = self.clipping.as_mut()?;
                editor.people = people;
                editor.people.sort_by_key(|p| p.username.to_lowercase());
            },
            Message::Strip(event) if editor.ready() => {
                let max = editor.max(duration);
                let (start, end) = editor.range;
                let len = end - start;
                match event {
                    Strip::Seek(at) => {
                        self.seek(at);
                        return None;
                    },
                    Strip::Grab(edge) => {
                        editor.drag = Some(edge);
                        if edge != Edge::Both {
                            editor.edge = edge;
                        }
                        return None;
                    },
                    Strip::Release => editor.drag = None,
                    Strip::Move(Edge::Start, at) => {
                        let s = snap(at.min(end - MIN_LENGTH).max(end - max).max(0.0));
                        self.change_range((s, end), Edge::Start);
                    },
                    Strip::Move(Edge::End, at) => {
                        let e = snap(at.max(start + MIN_LENGTH).min(start + max).min(duration));
                        self.change_range((start, e), Edge::End);
                    },
                    Strip::Move(Edge::Both, at) => {
                        let s = snap(at.min(duration - len).max(0.0));
                        self.change_range((s, s + len), Edge::Both);
                    },
                    Strip::Center(at) => {
                        let s = snap((at - len / 2.0).min(duration - len).max(0.0));
                        self.change_range((s, s + len), Edge::Both);
                    },
                }
                self.follow_range(effects);
            },
            Message::Nudge(direction, second) if editor.ready() => {
                let step = direction * if second { 1.0 } else { 1.0 / fps };
                let edge = editor.edge;
                let at = if edge == Edge::Start { editor.range.0 } else { editor.range.1 } + step;
                editor.drag = None;
                return self.edit_clip(Message::Strip(Strip::Move(edge, at)), effects);
            },
            Message::Play => {
                let (start, end) = editor.range;
                if !self.playing && (self.time < start || self.time >= end - 0.05) {
                    self.seek(start);
                }
                let playing = self.set_playing(None);
                self.flash(if playing { super::HudKind::Play } else { super::HudKind::Pause });
            },
            Message::In if editor.ready() => {
                let t = snap(self.time);
                let max = editor.max(duration);
                let e = editor.range.1.max(t + MIN_LENGTH).min(t + max).min(duration);
                editor.edge = Edge::Start;
                self.change_range((t, e), Edge::Start);
                self.follow_range(effects);
            },
            Message::Out if editor.ready() => {
                let t = snap(self.time);
                let max = editor.max(duration);
                let s = editor.range.0.min(t - MIN_LENGTH).max(t - max).max(0.0);
                editor.edge = Edge::End;
                self.change_range((s, t), Edge::End);
                self.follow_range(effects);
            },
            Message::Step(back) => {
                self.set_playing(Some(false));
                if let Some(engine) = &self.engine {
                    engine.frame_step(back);
                }
            },
            Message::Title(title) => editor.title = title.chars().take(120).collect(),
            Message::Audio(index) => {
                editor.audio = index;
                let subtitle = editor.subtitle.clone();
                self.preview_tracks(index, subtitle, effects);
            },
            Message::Subtitle(id) => {
                editor.subtitle = id.clone();
                let audio = editor.audio;
                self.preview_tracks(audio, id, effects);
            },
            Message::Preset(index) if index < editor.presets.len() => editor.preset = index,
            Message::Public(on) => editor.public = on && editor.you.as_ref().is_some_and(|you| you.can_link),
            Message::Pick(person, on) => {
                editor.recipients.retain(|p| p.id != person.id);
                if on {
                    editor.recipients.push(person);
                }
            },
            Message::Picker(open) => editor.picking = open,
            Message::Save if !editor.saving && editor.made.is_none() && editor.ready() => {
                if editor.length() < MIN_LENGTH {
                    return None;
                }
                let preset = *editor.chosen()?;
                let recipe = json!({
                    "start": editor.range.0,
                    "end": editor.range.1,
                    "audio": editor.audio,
                    "subtitles": editor.subtitle,
                    "height": preset.height,
                    "halfRate": preset.half_rate,
                });
                let (name, public, key) = (editor.title.clone(), editor.public, editor.key);
                let users: Vec<i64> = editor.recipients.iter().map(|p| p.id).collect();
                let editing = editor.editing.as_ref().map(|clip| clip.id);
                editor.saving = true;
                editor.picking = false;
                let (api, video) = (self.api.clone(), self.id);
                self.request(effects, async move {
                    let result = async {
                        let clip = match editing {
                            Some(id) => {
                                let input = json!({"name":name,"public":public,"recipe":recipe});
                                let clip = clip_of::<api::ClipsUpdate>(&api, json!({"id":id,"input":input})).await?;
                                if users.is_empty() {
                                    clip
                                } else {
                                    clip_of::<api::ClipsShare>(&api, json!({"id":id,"users":users})).await?
                                }
                            },
                            None => {
                                let input = json!({"videoId":video,"recipe":recipe,"name":name,"public":public,"recipients":users,"room":room});
                                clip_of::<api::ClipsCreate>(&api, json!({"input":input})).await?
                            },
                        };
                        clip.ok_or_else(|| api::Error::message("That clip isn’t here anymore."))
                    }
                    .await;
                    super::Message::Editor(Message::Saved(key, result.map_err(|e| e.message)))
                });
            },
            Message::Saved(key, result) if key == editor.key => {
                editor.saving = false;
                match result {
                    Ok(clip) => {
                        editor.made =
                            Some(Made { id: clip.id, clip: Some(clip), player: None, played: None, copied: false });
                        self.set_playing(Some(false));
                        self.apply_loop();
                        self.follow_made(effects);
                    },
                    Err(error) => return Some(Out::Toast(error, None, crate::ui::overlay::Tone::Danger)),
                }
            },
            Message::Status(id, result) => {
                let made = editor.made.as_mut().filter(|made| made.id == id)?;
                match result {
                    Ok(clip) => made.clip = clip,
                    Err(error) => return Some(Out::Toast(error, None, crate::ui::overlay::Tone::Danger)),
                }
                self.follow_made(effects);
            },
            Message::CopyLink => {
                let made = editor.made.as_mut()?;
                let link = made.clip.as_ref()?.link.clone()?;
                made.copied = true;
                let url = crate::clips::absolute(&link, &self.origin);
                return Some(Out::Copy(url.clone(), "Link copied".into(), Some(url)));
            },
            Message::Download => {
                let file = editor.made.as_ref()?.clip.as_ref()?.file.clone();
                let api = self.api.clone();
                self.request(effects, async move {
                    super::Message::Editor(Message::Downloaded(crate::clips::download(api, file).await))
                });
            },
            Message::Downloaded(result) => return crate::clips::downloaded(result),
            Message::YourClips => {
                let id = editor.made.as_ref()?.id;
                return Some(Out::Navigate(Route::Clip(id as u64)));
            },
            Message::Reel(id, message) => {
                let player = editor.made.as_mut().filter(|made| made.id == id)?.player.as_mut()?;
                player.update(message);
            },
            _ => {},
        }
        None
    }

    /// Plays what the clip will have: its audio, and its subtitles drawn as they'll be burned in.
    fn preview_tracks(&mut self, audio: Option<i64>, subtitle: Option<String>, effects: &mut Vec<Effect>) {
        self.audio = audio;
        if let (Some(engine), Some(id)) = (&self.engine, audio.and_then(|index| self.mpv_audio(index))) {
            engine.set("aid", id);
        }
        if self.subtitle != subtitle {
            self.subtitle = subtitle;
            self.load_subtitle(effects);
            self.apply_subtitle();
        }
    }

    /// Takes the range, showing the edge being moved; moving the whole range starts it over.
    fn change_range(&mut self, range: (f64, f64), moved: Edge) {
        let frame = 1.0 / self.fps();
        let Some(editor) = &mut self.clipping else { return };
        editor.range = range;
        if !self.playing || moved != Edge::End {
            self.seek(if moved == Edge::End { range.0.max(range.1 - frame) } else { range.0 });
        }
        self.apply_loop();
    }

    /// Keeps the filmstrip still while dragging, and follows the range once it wanders off.
    fn follow_range(&mut self, effects: &mut Vec<Effect>) {
        let duration = self.duration();
        let Some(editor) = &mut self.clipping else { return };
        if editor.drag.is_none() && (editor.range.0 < editor.view.0 || editor.range.1 > editor.view.1) {
            editor.view = Editor::around(editor.range, duration);
        }
        self.want_tiles(effects);
    }

    /// Asks for the filmstrip's frames that aren't here yet.
    fn want_tiles(&mut self, effects: &mut Vec<Effect>) {
        let Some(editor) = &self.clipping else { return };
        for step in editor.tiles() {
            self.want_preview(step, effects);
        }
    }

    /// Loops the range while it plays, exactly where it'll be cut (mpv's A-B loop).
    pub(super) fn apply_loop(&self) {
        let Some(engine) = &self.engine else { return };
        match &self.clipping {
            Some(editor) if self.playing && editor.made.is_none() && editor.ready() => {
                engine.set("ab-loop-a", editor.range.0);
                engine.set("ab-loop-b", editor.range.1);
            },
            _ => {
                engine.set("ab-loop-a", "no");
                engine.set("ab-loop-b", "no");
            },
        }
    }

    /// Starts the finished clip's preview, muted, once it's rendered.
    fn follow_made(&mut self, effects: &mut Vec<Effect>) {
        let (api, runtime, who, serial) = (self.api.clone(), self.runtime.clone(), self.who, self.serial);
        let Some(made) = self.clipping.as_mut().and_then(|editor| editor.made.as_mut()) else { return };
        let Some(clip) = made.clip.clone() else { return };
        let poster = clip.poster.clone();
        if clip.ready() && clip.poster.is_some() && made.played != Some(clip.rendered_at) {
            made.played = Some(clip.rendered_at);
            let id = made.id;
            let wrap: Arc<dyn Fn(clip_player::Message) -> crate::app::Message + Send + Sync> =
                Arc::new(move |message| {
                    crate::app::Message::Player(serial, super::Message::Editor(Message::Reel(id, message)))
                });
            made.player = Some(ClipPlayer::new(api, runtime, &clip.file, true, who, wrap, effects));
        }
        if let Some(poster) = poster {
            self.picture(poster, effects);
        }
    }

    /// The server says clip `id` changed: a render's `progress` is patched in, anything else is
    /// asked for again.
    pub fn clip_changed(&mut self, id: i64, progress: Option<f64>, effects: &mut Vec<Effect>) {
        let Some(made) = self.clipping.as_mut().and_then(|editor| editor.made.as_mut()).filter(|made| made.id == id)
        else {
            return;
        };
        if let (Some(progress), Some(clip)) = (progress, &mut made.clip)
            && clip.state == "RENDERING"
        {
            Arc::make_mut(clip).progress = Some(progress);
            return;
        }
        let api = self.api.clone();
        self.request(effects, async move {
            let result = clip_of::<api::ClipsClip>(&api, json!({"id":id})).await.map_err(|e| e.message);
            super::Message::Editor(Message::Status(id, result))
        });
    }

    fn close_clip(&mut self, effects: &mut Vec<Effect>) {
        let Some(editor) = self.clipping.take() else { return };
        self.apply_loop();
        self.audio = editor.audio_before;
        if let (Some(engine), Some(id)) = (&self.engine, self.audio.and_then(|index| self.mpv_audio(index))) {
            engine.set("aid", id);
        }
        let subtitle_changed = self.subtitle != editor.subtitle_before;
        self.subtitle = editor.subtitle_before;
        if subtitle_changed {
            self.load_subtitle(effects);
            self.apply_subtitle();
        }
        self.quality = editor.quality;
        self.play_from(editor.at, true);
        self.poke();
    }

    /// The clip sent off from the editor, while its render is being followed.
    pub fn rendering(&self) -> Option<i64> {
        self.clipping.as_ref()?.made.as_ref().map(|made| made.id)
    }

    pub fn tick_clip(&mut self) {
        let Some(editor) = &self.clipping else { return };
        // Played from outside the range, it starts over at its beginning.
        if self.playing && editor.ready() && editor.made.is_none() && self.time < editor.range.0 - 0.3 {
            let start = editor.range.0;
            self.seek(start);
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AllowanceData {
    clip_allowance: Allowance,
}

pub fn view<'a>(player: &'a Player, editor: &'a Editor, look: &Look) -> Element<'a, super::Message> {
    let l = *look;
    let canvas = l.media_canvas;
    let content: Element<'a, super::Message> = if !editor.ready() {
        container(spinner(32.0, look)).center(Fill).boxed()
    } else if let Some(made) = &editor.made {
        rendering(player, made, look)
    } else {
        editing(player, editor, look)
    };
    container(content)
        .width(Fill)
        .height(Fill)
        .style(move |_| iced_widget::container::Style { background: Some(canvas.into()), ..Default::default() })
        .boxed()
}

fn msg(message: Message) -> super::Message {
    super::Message::Editor(message)
}

fn editing<'a>(player: &'a Player, editor: &'a Editor, look: &Look) -> Element<'a, super::Message> {
    let l = *look;
    let pb = player.pb.as_ref().expect("a clip has playback metadata");
    let duration = player.duration();
    let max = editor.max(duration);

    let source = match pb.title.kind {
        api::playback::TitleKind::SHOW => [Some(pb.title.name.as_str()), pb.label.as_deref()]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
        _ => pb.title.name.clone(),
    };
    let mut header = Row::new()
        .spacing(12)
        .align_y(Alignment::Center)
        .push(icon_button(Icon::X, "Close (Esc)", Some(msg(Message::Close)), look))
        .push(
            column![
                row![
                    icon(Icon::Scissors, 16.0).color(l.highlight),
                    text(if editor.editing.is_some() { "Edit clip" } else { "New clip" }, type_::S15, l.ink)
                        .font(MEDIUM),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                truncate(source, type_::S13, l.ink_3),
            ]
            .width(Fill)
            .boxed(),
        );
    if let Some(you) = editor.you.as_ref().filter(|you| you.storage > 0) {
        let used = (you.bytes as f64 / (you.storage as f64 * 1024.0 * 1024.0)) as f32;
        header = header.push(
            column![
                container(text(format!("{} of {} used", bytes(you.bytes), gb(you.storage)), type_::XS2, l.ink_3))
                    .align_right(Fill),
                progress(Some(used), if used > 0.9 { l.info } else { l.ink }, look),
            ]
            .spacing(4)
            .width(176)
            .boxed(),
        );
    }

    let mut stage: Vec<Element<'a, super::Message>> = Vec::new();
    if let Some(screen) = &player.screen {
        stage.push(iced_widget::mouse_area(video(screen, player.aspect)).on_press(msg(Message::Play)).boxed());
    }
    let failure = player.failure();
    if player.waiting && failure.is_none() {
        stage.push(container(super::view::media_spinner(look)).center(Fill).boxed());
    }
    if let Some(error) = failure {
        stage.push(
            container(
                text(
                    format!("The preview can’t play: {error}. The clip itself will still render."),
                    type_::SM,
                    l.ink_2,
                )
                .center(),
            )
            .center(Fill)
            .padding(24)
            .boxed(),
        );
    }
    if let Some(hud) = &player.hud {
        stage.push(super::view::hud_view(&hud.kind, hud.at, look));
    }
    let shade = l.media_shade;
    let radius = l.radius(16.0);
    let stage =
        container(Stack::with_children(stage).width(Fill).height(Fill)).width(Fill).height(Fill).clip(true).style(
            move |_| container::Style {
                background: Some(shade.into()),
                border: iced_core::Border::default().rounded(radius),
                ..container::Style::default()
            },
        );

    let tiles = editor.tiles();
    let corner = l.radius(10.0);
    let frames = Row::with_children(tiles.iter().enumerate().map(|(i, step)| {
        let radius = match i {
            0 => [corner, 0.0, 0.0, corner],
            i if i == TILES - 1 => [0.0, corner, corner, 0.0],
            _ => [0.0; 4],
        };
        match player.previews.get(step) {
            Some(frame) => image(frame.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .opacity(0.6_f32)
                .border_radius(strip::corners(radius))
                .boxed(),
            None => space().width(Fill).height(Fill).boxed(),
        }
    }))
    .width(Fill)
    .height(Fill);
    let range = strip::Range { view: editor.view, range: editor.range, time: player.time };
    let filmstrip = Stack::with_children([
        squircle(space().width(Fill).height(Fill), 10.0, look).fill(l.raised).boxed(),
        frames.boxed(),
        strip::overlay(range, look, |event| msg(Message::Strip(event))).boxed(),
    ])
    .width(Fill)
    .height(64);
    let strip = column![
        container(strip::overview(duration, editor.view, editor.range, look, |event| msg(Message::Strip(event))))
            .padding(Padding { bottom: 8.0, ..Padding::ZERO }),
        filmstrip,
        row![
            text(stamp(editor.view.0), type_::XS2, l.ink_3),
            space().width(Fill),
            text(stamp(editor.view.1), type_::XS2, l.ink_3),
        ]
        .padding(Padding { top: 4.0, ..Padding::ZERO }),
    ];

    let mut summary = row![
        text(format!("{} – {}", stamp(editor.range.0), stamp(editor.range.1)), type_::S13, l.ink_2),
        container(text(length(editor.length()), type_::S13, l.ink)).padding(Padding { left: 8.0, ..Padding::ZERO }),
    ]
    .align_y(Alignment::Center);
    if max.is_finite() {
        summary = summary.push(text(format!(" of {} max", length(max)), type_::S13, l.ink_3).boxed());
    }
    let kbd = |label: &'a str| text(label, type_::XS2, l.ink_3);
    let controls = row![
        button(if player.playing { "Pause" } else { "Play" }, look)
            .icon(if player.playing { Icon::Pause } else { Icon::Play })
            .on_press(msg(Message::Play))
            .boxed(),
        icon_button(Icon::ChevronLeft, "Previous frame (,)", Some(msg(Message::Step(true))), look),
        icon_button(Icon::ChevronRight, "Next frame (.)", Some(msg(Message::Step(false))), look),
        button("", look)
            .plain()
            .small()
            .content(row![text("Start here", type_::S13, l.ink_2), kbd("I")].spacing(6).align_y(Alignment::Center))
            .on_press(msg(Message::In))
            .boxed(),
        button("", look)
            .plain()
            .small()
            .content(row![text("End here", type_::S13, l.ink_2), kbd("O")].spacing(6).align_y(Alignment::Center))
            .on_press(msg(Message::Out))
            .boxed(),
        container(summary).padding(Padding { left: 4.0, ..Padding::ZERO }),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let audio_menu = select(
        Some(editor.audio),
        &editor.audio_options,
        |id| {
            id.and_then(|i| pb.media.audio.iter().find(|a| a.index == i)).map_or("None".into(), |a| {
                let name =
                    super::tracks::track_name(a.title.as_deref(), a.language.as_deref(), &format!("Track {}", a.index));
                if a.channels > 2 { format!("{name} · {} ch → stereo", a.channels) } else { name }
            })
        },
        |a| msg(Message::Audio(a)),
        look,
    );
    let subtitle_menu = select(
        Some(editor.subtitle.clone()),
        &editor.subtitle_options,
        |id| {
            id.as_ref().and_then(|id| pb.media.subtitles.iter().find(|s| &s.id == id)).map_or("None".into(), |s| {
                let fallback = if s.id.starts_with('x') {
                    "External".to_owned()
                } else {
                    format!("Track {}", &s.id[1.min(s.id.len())..])
                };
                let name = super::tracks::track_name(s.title.as_deref(), s.language.as_deref(), &fallback);
                let kind = if !s.supported {
                    " · image"
                } else if s.forced {
                    " · forced"
                } else {
                    ""
                };
                format!("{name}{kind}")
            })
        },
        |s| msg(Message::Subtitle(s)),
        look,
    );
    let chosen = editor.chosen().copied();
    let quality_menu = select(
        Some(editor.preset),
        &editor.choices,
        |i| editor.presets.get(*i).map(Preset::label).unwrap_or_default(),
        |i| msg(Message::Preset(i)),
        look,
    );
    let size_hint = chosen.map(|p| format!("Up to {}", bytes(worst_size(&p, editor.length()) as i64)));
    let options = row![
        container(labeled(
            "Title",
            None,
            owned_input("clip-title", editor.title.clone(), |s| msg(Message::Title(s)), look)
                .placeholder("Optional")
                .boxed(),
            look
        ))
        .width(iced_core::Length::FillPortion(14)),
        container(labeled("Audio", None, audio_menu, look)).width(iced_core::Length::FillPortion(10)),
        container(labeled("Subtitles", Some("Burned into the video".into()), subtitle_menu, look))
            .width(iced_core::Length::FillPortion(10)),
        container(labeled("Quality", size_hint, quality_menu, look)).width(iced_core::Length::FillPortion(10)),
    ]
    .spacing(12);

    let mut body = Column::new().spacing(16).push(strip.boxed()).push(controls.boxed()).push(options.boxed());
    let track = editor.subtitle.as_ref().and_then(|id| pb.media.subtitles.iter().find(|s| &s.id == id));
    if let (Some(track), Some(you)) = (track, &editor.you)
        && track.supported
        && CJK.contains(&track.language.as_deref().unwrap_or_default())
        && !you.custom_default_font
        && (track.codec != "ass" || pb.media.fonts.is_empty())
    {
        let language = super::tracks::language(track.language.as_deref()).unwrap_or_default().into_owned();
        body = body.push(
            row![
                icon(Icon::TriangleAlert, 14.0).color(l.warn_soft),
                text(
                    format!(
                        "{language} text may come out as empty boxes: this server has no font for it set up for clips. \
                         An admin can set [clips] default-font to one that has it."
                    ),
                    type_::XS,
                    l.warn_soft
                )
                .width(Fill),
            ]
            .spacing(8)
            .boxed(),
        );
    }

    let mut footer = Row::new().spacing(20).align_y(Alignment::Center);
    if !editor.people.is_empty() {
        footer = footer.push(send_to(player, editor, look));
    }
    if editor.you.as_ref().is_some_and(|you| you.can_link) {
        footer = footer.push(
            row![
                toggle(editor.public, |on| msg(Message::Public(on)), look),
                icon(Icon::Link2, 14.0).color(l.ink_2),
                text("Anyone with the link", type_::SM, l.ink_2),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    let ready = !editor.saving && editor.length() >= MIN_LENGTH;
    let make = button(if editor.editing.is_some() { "Save and render" } else { "Make clip" }, look).primary().large();
    let make = if editor.saving {
        make.content(
            row![spinner(16.0, look), text("Saving…", type_::S15, l.on_accent).font(MEDIUM)]
                .spacing(10)
                .align_y(Alignment::Center),
        )
    } else {
        make.icon(Icon::Scissors)
    };
    footer = footer
        .push(space().width(Fill).boxed())
        .push(button("Cancel", look).plain().on_press(msg(Message::Close)).boxed())
        .push(make.on_press_maybe(ready.then_some(msg(Message::Save))).boxed());
    let picker = editor.picking.then(|| people_picker(player, editor, look));
    let mut footer = Column::new().spacing(12).push(crate::ui::controls::divider(look)).push(footer.boxed());
    if let Some(picker) = picker {
        footer = footer.push(picker);
    }
    body = body.push(footer.boxed());

    column![
        // Clear of the window's controls in the corner.
        container(header).padding(Padding {
            top: 16.0,
            right: if player.fullscreen { 20.0 } else { 120.0 },
            bottom: 12.0,
            left: 20.0,
        }),
        container(stage).height(Fill).padding([0, 20]),
        container(body).padding(Padding { top: 16.0, right: 20.0, bottom: 20.0, left: 20.0 }),
    ]
    .boxed()
}

/// A label over a control, with a hint at its end (`Labeled`).
fn labeled<'a>(
    label: &'a str,
    hint: Option<String>,
    control: Element<'a, super::Message>,
    look: &Look,
) -> Element<'a, super::Message> {
    let mut top =
        Row::new().spacing(8).align_y(Alignment::End).push(text(label, type_::XS, look.ink_3).width(Fill).boxed());
    if let Some(hint) = hint {
        top = top.push(truncate(hint, type_::XS2, look.ink_3).boxed());
    }
    column![top, control].spacing(6).width(Fill).boxed()
}

/// Who to send the clip to: the button, and the faces picked (`SendTo`).
fn send_to<'a>(player: &'a Player, editor: &'a Editor, look: &Look) -> Element<'a, super::Message> {
    let l = *look;
    let mut line = Row::new()
        .spacing(8)
        .align_y(Alignment::Center)
        .push(button("Send to", look).icon(Icon::Send).on_press(msg(Message::Picker(!editor.picking))).boxed());
    for person in editor.recipients.iter().take(6) {
        let picture = person.avatar.as_deref().and_then(|path| player.pictures.get(path));
        line = line.push(crate::ui::media::avatar(&person.username, picture, 24.0, None, look));
    }
    if !editor.recipients.is_empty() {
        let who = if editor.recipients.len() == 1 {
            editor.recipients[0].username.clone()
        } else {
            format!("{} people", editor.recipients.len())
        };
        line = line.push(truncate(who, type_::XS, l.ink_3).boxed());
    }
    line.boxed()
}

fn people_picker<'a>(player: &'a Player, editor: &'a Editor, look: &Look) -> Element<'a, super::Message> {
    let l = *look;
    let mut list = Column::new().spacing(2);
    for person in &editor.people {
        let on = editor.recipients.iter().any(|p| p.id == person.id);
        let picture = person.avatar.as_deref().and_then(|path| player.pictures.get(path));
        let line = row![
            checkbox(on, Some(msg(Message::Pick(person.clone(), !on))), look),
            crate::ui::media::avatar(&person.username, picture, 22.0, None, look),
            truncate(person.username.as_str(), type_::S13, l.ink).width(Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        list = list.push(
            pressable(line, 8.0, look, move |status| Appearance {
                paint: Paint {
                    fill: matches!(status, Status::Hovered | Status::Pressed).then_some(l.hover),
                    ..Paint::NONE
                },
                ink: l.ink,
            })
            .width(Fill)
            .padding([6, 8])
            .on_press(msg(Message::Pick(person.clone(), !on)))
            .boxed(),
        );
    }
    panel(crate::ui::layout::at_most(iced_widget::scrollable(list), 288.0), 14.0, look).padding(6).width(256).boxed()
}

/// Following the render once it's sent off; there's no need to stay (`Rendering`).
fn rendering<'a>(player: &'a Player, made: &'a Made, look: &Look) -> Element<'a, super::Message> {
    let l = *look;
    let Some(clip) = &made.clip else { return container(spinner(32.0, look)).center(Fill).boxed() };
    let done = clip.ready();
    let failed = clip.state == "FAILED";
    let picture: Element<'a, super::Message> = match &made.player {
        Some(reel) if done => {
            let id = made.id;
            let poster = clip.poster.as_deref().and_then(|p| player.pictures.get(p));
            reel.view(poster, false, look).map(move |message| msg(Message::Reel(id, message))).boxed()
        },
        _ => container(if failed {
            icon(Icon::TriangleAlert, 32.0).color(l.danger)
        } else {
            icon(Icon::Scissors, 32.0).color(l.highlight)
        })
        .center(Fill)
        .boxed(),
    };
    let raised = l.raised;
    let radius = l.radius(18.0);
    let picture = crate::ui::layout::aspect(
        container(picture).width(Fill).height(Fill).clip(true).style(move |_| container::Style {
            background: Some(raised.into()),
            border: iced_core::Border::default().rounded(radius),
            ..container::Style::default()
        }),
        16.0 / 9.0,
    );
    let title = if done {
        "Your clip is ready"
    } else if failed {
        "The clip didn’t render"
    } else if clip.state == "QUEUED" {
        "Waiting its turn"
    } else {
        "Rendering your clip"
    };
    let mut body = column![
        picture,
        container(text(title, type_::LG, l.ink).font(SEMIBOLD)).padding(Padding {
            top: 24.0,
            bottom: 6.0,
            ..Padding::ZERO
        })
    ]
    .align_x(Alignment::Center);
    if failed {
        body = body.push(text(clip.error.clone().unwrap_or_default(), type_::SM, l.ink_2).center().boxed());
    } else if !done {
        body = body
            .push(
                container(progress(
                    (clip.state == "RENDERING").then(|| clip.progress.unwrap_or(0.0) as f32),
                    l.ink,
                    look,
                ))
                .width(256)
                .padding(Padding { top: 10.0, ..Padding::ZERO })
                .boxed(),
            )
            .push(
                container(
                    text("Keep watching if you like; you’ll get a notification when it’s done.", type_::S13, l.ink_3)
                        .center(),
                )
                .padding(Padding { top: 12.0, ..Padding::ZERO })
                .boxed(),
            );
    } else {
        body = body.push(
            text(
                format!(
                    "{}×{} · {} fps · {}",
                    clip.width.unwrap_or_default(),
                    clip.height.unwrap_or_default(),
                    clip.fps.unwrap_or_default().round(),
                    bytes(clip.bytes.unwrap_or_default())
                ),
                type_::SM,
                l.ink_2,
            )
            .boxed(),
        );
    }
    let mut actions = Row::new().spacing(8).align_y(Alignment::Center);
    if done {
        actions = actions.push(
            button("Download", look).primary().large().icon(Icon::Download).on_press(msg(Message::Download)).boxed(),
        );
    }
    if done && clip.link.is_some() && clip.link_live {
        actions = actions.push(
            button("Copy link", look)
                .large()
                .icon(if made.copied { Icon::Check } else { Icon::Copy })
                .on_press(msg(Message::CopyLink))
                .boxed(),
        );
    }
    actions = actions.push(button("Your clips", look).large().on_press(msg(Message::YourClips)).boxed());
    actions = actions.push(
        button(if done || failed { "Back to the video" } else { "Keep watching" }, look)
            .plain()
            .large()
            .on_press(msg(Message::Close))
            .boxed(),
    );
    body = body.push(container(actions.wrap()).padding(Padding { top: 24.0, ..Padding::ZERO }).boxed());
    container(container(body.width(Fill)).width(Fill.max(448))).center(Fill).padding(24).boxed()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::app::Message as AppMessage;
    use crate::player::Message as PlayerMessage;
    use crate::player::tests::player;

    async fn settle(player: &mut Player, effects: &mut Vec<Effect>) -> Vec<Out> {
        let mut outs = Vec::new();
        while !effects.is_empty() {
            for effect in std::mem::take(effects) {
                if let Effect::Request(request) = effect
                    && let AppMessage::Player(_, message) = request.await
                    && let Some(super::super::Out::Clips(out)) = player.update(message, effects)
                {
                    outs.push(out);
                }
            }
        }
        outs
    }

    fn clip(id: i64) -> serde_json::Value {
        json!({"id":id,"state":"QUEUED","name":"","start":49.0,"end":77.0,"quality":{"height":720,"halfRate":false},"file":format!("/api/clips/{id}/video")})
    }

    #[test]
    fn handoff_saves_a_recipe_without_overwriting_watch_progress() {
        crate::test_support::run(async {
            let (origin, requests) = crate::test_support::http(4, |request| match request["operationName"].as_str().unwrap() {
                "SaveProgress" => json!({"data":{"saveProgress":{"id":1}}}),
                "ClipsAllowance" => json!({"data":{"clipAllowance":{"canClip":true,"canLink":true,"maxLength":60,"customDefaultFont":false}}}),
                "ClipsPeople" => json!({"data":{"users":[{"id":3,"username":"Cranky","avatar":null}]}}),
                "ClipsCreate" => json!({"data":{"createClip":clip(42)}}),
                op => panic!("unexpected {op}"),
            })
            .await;
            let mut player = player();
            player.api = api::Api::new(origin, None).unwrap();
            player.can_clip = true;
            // The filmstrip's frames come from elsewhere.
            player.previews_failed = true;
            player.time = 76.25;
            player.audio = Some(2);
            player.subtitle = Some("s3".into());
            player.quality = Quality::Height(720);
            let mut effects = Vec::new();
            player.update(PlayerMessage::MakeClip, &mut effects);
            assert!(!player.playing);
            assert_eq!(player.clipping.as_ref().unwrap().at, 76.25);
            settle(&mut player, &mut effects).await;
            let editor = player.clipping.as_ref().unwrap();
            assert!(editor.ready());
            // The last half minute before where it was, on frame boundaries.
            assert_eq!(editor.range, (46.25, 76.25));
            assert_eq!(editor.people.len(), 1);
            player.update(PlayerMessage::Editor(Message::Strip(Strip::Move(Edge::Start, 49.0))), &mut effects);
            player.update(PlayerMessage::Editor(Message::Strip(Strip::Move(Edge::End, 77.0))), &mut effects);
            player.update(
                PlayerMessage::Editor(Message::Pick(Person { id: 3, ..Default::default() }, true)),
                &mut effects,
            );
            player.update(PlayerMessage::Editor(Message::Save), &mut effects);
            settle(&mut player, &mut effects).await;
            assert_eq!(player.clipping.as_ref().unwrap().made.as_ref().unwrap().id, 42);
            player.time = 300.0;
            player.save(&mut effects);
            assert!(effects.is_empty(), "preview position must not become watch progress");
            player.update(PlayerMessage::Editor(Message::Close), &mut effects);
            assert_eq!(player.time, 76.25);
            assert_eq!(player.quality, Quality::Height(720));
            assert_eq!(player.audio, Some(2));
            assert_eq!(player.subtitle.as_deref(), Some("s3"));
            assert!(!player.playing);
            let requests = requests.await.unwrap();
            assert_eq!(requests[0].1["variables"]["position"], 76.25);
            let created = requests.iter().find(|(_, body)| body["operationName"] == "ClipsCreate").unwrap();
            assert_eq!(
                created.1["variables"]["input"]["recipe"],
                json!({"start":49.0,"end":77.0,"audio":2,"subtitles":null,"height":1080,"halfRate":false})
            );
            assert_eq!(created.1["variables"]["input"]["recipients"], json!([3]));
        });
    }

    #[test]
    fn changing_a_clip_starts_from_its_recipe_and_saves_it_in_place() {
        crate::test_support::run(async {
            let (origin, requests) = crate::test_support::http(4, |request| match request["operationName"].as_str().unwrap() {
                "ClipsAllowance" => json!({"data":{"clipAllowance":{"canClip":true,"canLink":false,"maxLength":0,"customDefaultFont":false}}}),
                "ClipsPeople" => json!({"data":{"users":[]}}),
                "ClipsClip" => json!({"data":{"clip":{"id":7,"name":"Best bit","public":true,"start":100.0,"end":110.0,"audio":2,"quality":{"height":720,"halfRate":false},"state":"READY","file":"/api/clips/7/video"}}}),
                "ClipsUpdate" => json!({"data":{"updateClip":clip(7)}}),
                op => panic!("unexpected {op}"),
            })
            .await;
            let mut player = player();
            player.api = api::Api::new(origin, None).unwrap();
            player.can_clip = true;
            // The filmstrip's frames come from elsewhere.
            player.previews_failed = true;
            let mut effects = Vec::new();
            player.edit_existing(7, &mut effects);
            assert!(!player.clipping.as_ref().unwrap().ready());
            settle(&mut player, &mut effects).await;
            let editor = player.clipping.as_ref().unwrap();
            assert_eq!(editor.range, (100.0, 110.0));
            assert_eq!(editor.title, "Best bit");
            assert!(editor.public);
            assert_eq!(player.audio, Some(2));
            player.update(PlayerMessage::Editor(Message::Save), &mut effects);
            settle(&mut player, &mut effects).await;
            let requests = requests.await.unwrap();
            let update = requests.iter().find(|(_, body)| body["operationName"] == "ClipsUpdate").unwrap();
            assert_eq!(update.1["variables"]["id"], 7);
            assert_eq!(update.1["variables"]["input"]["name"], "Best bit");
            assert_eq!(update.1["variables"]["input"]["recipe"]["start"], 100.0);
            assert_eq!(update.1["variables"]["input"]["public"], true);
        });
    }

    #[test]
    fn ranges_stay_within_the_video_the_limit_and_half_a_second() {
        let mut player = player();
        player.can_clip = true;
        let mut effects = Vec::new();
        player.start_clip(&mut effects);
        let key = player.clipping.as_ref().unwrap().key;
        let you = Allowance { can_clip: true, max_length: 20, ..Default::default() };
        player.update(PlayerMessage::Editor(Message::Allowance(key, Ok(you))), &mut effects);
        let editor = |player: &Player| player.clipping.as_ref().unwrap().range;
        assert_eq!(editor(&player), (0.0, 20.0));
        player.update(PlayerMessage::Editor(Message::Strip(Strip::Move(Edge::End, 100.0))), &mut effects);
        assert_eq!(editor(&player), (0.0, 20.0));
        player.update(PlayerMessage::Editor(Message::Strip(Strip::Move(Edge::Start, 19.9))), &mut effects);
        assert_eq!(editor(&player).0, 19.5);
        player.update(PlayerMessage::Editor(Message::Strip(Strip::Move(Edge::Both, 5000.0))), &mut effects);
        assert_eq!(editor(&player), (1799.5, 1800.0));
        player.update(PlayerMessage::Editor(Message::Strip(Strip::Center(0.0))), &mut effects);
        assert_eq!(editor(&player), (0.0, 0.5));
        // A frame at a time from the edge last touched, a second with Shift.
        player.update(PlayerMessage::Editor(Message::Nudge(1.0, true)), &mut effects);
        assert_eq!(editor(&player), (0.0, 1.5));
        player.update(PlayerMessage::Editor(Message::Strip(Strip::Grab(Edge::Start))), &mut effects);
        player.update(PlayerMessage::Editor(Message::Nudge(1.0, false)), &mut effects);
        assert_eq!(editor(&player), (1.0 / 24.0, 1.5));
        player.update(PlayerMessage::Editor(Message::Saved(key + 1, Err("stale".into()))), &mut effects);
        assert!(player.clipping.as_ref().unwrap().made.is_none());
    }

    #[test]
    fn render_progress_from_events_is_patched_in() {
        let mut player = player();
        player.can_clip = true;
        let mut effects = Vec::new();
        player.start_clip(&mut effects);
        let editor = player.clipping.as_mut().unwrap();
        let mut made: Clip = serde_json::from_value(clip(9)).unwrap();
        made.state = "RENDERING".into();
        editor.made = Some(Made { id: 9, clip: Some(Arc::new(made)), player: None, played: None, copied: false });
        effects.clear();
        player.clip_changed(9, Some(0.5), &mut effects);
        assert!(effects.is_empty());
        assert_eq!(player.clipping.as_ref().unwrap().made.as_ref().unwrap().clip.as_ref().unwrap().progress, Some(0.5));
        player.clip_changed(9, None, &mut effects);
        assert_eq!(effects.len(), 1);
        player.clip_changed(10, None, &mut effects);
        assert_eq!(effects.len(), 1);
    }
}
