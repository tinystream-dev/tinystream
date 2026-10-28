// SPDX-License-Identifier: AGPL-3.0-or-later

//! The player's handoff to clip editing: preserve the playback context, preview with the same
//! mpv instance, and send the recipe to the server. The Clips library and sharing UI live in #93.

use std::time::{Duration, Instant};

use iced_core::{Alignment, Length};
use iced_widget::{Element, Widget, column, container, row, space};

use super::{Player, Quality};
use crate::api::{self, player_clip_allowance, player_clip_status, player_create_clip};
use crate::app::Effect;
use crate::ui::Look;
use crate::ui::controls::{button, field, input, toggle};
use crate::ui::select::select;
use crate::ui::text::{self as type_, SEMIBOLD, text};
use crate::ui::video::video;

#[derive(Clone)]
pub enum Message {
    Close,
    Start(String),
    End(String),
    Name(String),
    Height(i64),
    HalfRate(bool),
    Audio(Option<i64>),
    Subtitle(Option<String>),
    Public(bool),
    Step(bool),
    In,
    Out,
    Save,
    Allowance(u64, Result<player_clip_allowance::ResponseData, String>),
    Created(u64, Result<i64, String>),
    Status(i64, Result<player_clip_status::ResponseData, String>),
}

pub struct Editor {
    key: u64,
    pub at: f64,
    quality: Quality,
    audio_before: Option<i64>,
    subtitle_before: Option<String>,
    pub start: String,
    pub end: String,
    pub name: String,
    pub audio: Option<i64>,
    pub subtitle: Option<String>,
    pub height: i64,
    pub half_rate: bool,
    pub public: bool,
    pub saving: bool,
    pub made: Option<i64>,
    pub state: String,
    pub error: Option<String>,
    audio_options: Vec<Option<i64>>,
    subtitle_options: Vec<Option<String>>,
    heights: Vec<i64>,
    allowance: Option<player_clip_allowance::PlayerClipAllowanceClipAllowance>,
    polled: Instant,
}

impl Editor {
    pub fn new(player: &Player, key: u64) -> Self {
        let end = player.time.max(30.0).min(player.duration());
        let pb = player.pb.as_ref().expect("clip metadata");
        let source_height = pb.media.video.as_ref().map_or(720, |v| v.height);
        let mut heights: Vec<i64> = [1080, 720, 480, 360].into_iter().filter(|h| *h <= source_height).collect();
        if heights.is_empty() {
            heights.push(source_height);
        }
        let height = heights[0];
        let audio_options =
            if pb.media.audio.is_empty() { vec![None] } else { pb.media.audio.iter().map(|a| Some(a.index)).collect() };
        let subtitle_options = std::iter::once(None)
            .chain(pb.media.subtitles.iter().filter(|s| super::tracks::usable(s)).map(|s| Some(s.id.clone())))
            .collect();
        Self {
            key,
            at: player.time,
            quality: player.quality,
            audio_before: player.audio,
            subtitle_before: player.subtitle.clone(),
            start: format!("{:.3}", (end - 30.0).max(0.0)),
            end: format!("{end:.3}"),
            name: String::new(),
            audio: player.audio,
            subtitle: player.subtitle.clone(),
            height,
            half_rate: false,
            public: false,
            audio_options,
            subtitle_options,
            heights,
            saving: false,
            made: None,
            state: String::new(),
            error: None,
            allowance: None,
            polled: Instant::now(),
        }
    }

    pub fn range(&self, duration: f64) -> Option<(f64, f64)> {
        let (start, end) = (self.start.parse::<f64>().ok()?, self.end.parse::<f64>().ok()?);
        let max = self
            .allowance
            .as_ref()
            .map_or(f64::INFINITY, |a| if a.max_length > 0 { a.max_length as f64 } else { f64::INFINITY });
        (start.is_finite()
            && end.is_finite()
            && start >= 0.0
            && end <= duration
            && end - start >= 0.5
            && end - start <= max)
            .then_some((start, end))
    }
}

impl Player {
    pub fn start_clip(&mut self, effects: &mut Vec<Effect>) {
        if !self.can_clip || self.pb.is_none() || self.clipping.is_some() {
            return;
        }
        self.set_playing(Some(false));
        self.save(effects);
        self.shots += 1;
        let editor = Editor::new(self, self.shots);
        let key = editor.key;
        let start = editor.start.parse().unwrap_or(0.0);
        self.clipping = Some(editor);
        self.menu = None;
        self.rested = None;
        self.resting_since = None;
        self.quality = Quality::Auto;
        self.play_from(start, true);
        let api = self.api.clone();
        self.request(effects, async move {
            let result = api
                .request::<api::PlayerClipAllowance>(player_clip_allowance::Variables {})
                .await
                .map_err(|e| e.message);
            super::Message::Editor(Message::Allowance(key, result))
        });
    }

    pub fn edit_clip(&mut self, message: Message, effects: &mut Vec<Effect>) {
        if matches!(message, Message::Close) {
            if let Some(editor) = self.clipping.take() {
                self.audio = editor.audio_before;
                self.subtitle = editor.subtitle_before;
                self.quality = editor.quality;
                self.play_from(editor.at, true);
                self.poke();
            }
            return;
        }
        let duration = self.duration();
        let room = self.room().map(str::to_owned);
        let Some(editor) = &mut self.clipping else { return };
        match message {
            Message::Start(value) => editor.start = value,
            Message::End(value) => editor.end = value,
            Message::Name(value) => editor.name = value.chars().take(120).collect(),
            Message::Height(height) if editor.heights.contains(&height) => editor.height = height,
            Message::HalfRate(on) => editor.half_rate = on,
            Message::Public(on) => editor.public = on && editor.allowance.as_ref().is_some_and(|a| a.can_link),
            Message::Audio(index) => {
                editor.audio = index;
                self.audio = index;
                if let (Some(engine), Some(id)) = (&self.engine, index.and_then(|index| self.mpv_audio(index))) {
                    engine.set("aid", id);
                }
            },
            Message::Subtitle(id) => {
                editor.subtitle = id.clone();
                self.subtitle = id;
                self.apply_subtitle();
            },
            Message::In => editor.start = format!("{:.3}", self.time),
            Message::Out => editor.end = format!("{:.3}", self.time),
            Message::Step(back) => {
                self.set_playing(Some(false));
                if let Some(engine) = &self.engine {
                    engine.frame_step(back);
                }
            },
            Message::Allowance(key, result) if key == editor.key => match result {
                Ok(data) => {
                    let allowance = data.clip_allowance;
                    if allowance.max_length > 0
                        && let (Ok(start), Ok(end)) = (editor.start.parse::<f64>(), editor.end.parse::<f64>())
                    {
                        editor.start = format!("{:.3}", start.max(end - allowance.max_length as f64));
                    }
                    editor.allowance = Some(allowance);
                },
                Err(error) => editor.error = Some(error),
            },
            Message::Created(key, result) if key == editor.key => {
                editor.saving = false;
                match result {
                    Ok(id) => {
                        editor.made = Some(id);
                        editor.state = "Queued".into();
                        editor.polled = Instant::now() - Duration::from_secs(2);
                    },
                    Err(error) => editor.error = Some(error),
                }
            },
            Message::Status(id, result) if editor.made == Some(id) => match result {
                Ok(data) => match data.clip {
                    Some(clip) => {
                        editor.state = match clip.state {
                            player_clip_status::ClipState::READY => "Saved to Clips".into(),
                            player_clip_status::ClipState::FAILED => {
                                editor.error = clip.error;
                                "Failed".into()
                            },
                            player_clip_status::ClipState::EVICTED => {
                                editor.error = Some("This clip is no longer stored.".into());
                                "No longer stored".into()
                            },
                            _ => format!("Rendering · {}%", (clip.progress.unwrap_or(0.0) * 100.0).round()),
                        };
                    },
                    None => editor.error = Some("This clip isn’t here anymore.".into()),
                },
                Err(error) => editor.error = Some(error),
            },
            Message::Save
                if !editor.saving && editor.made.is_none() && editor.allowance.as_ref().is_some_and(|a| a.can_clip) =>
            {
                let Some((start, end)) = editor.range(duration) else {
                    editor.error = Some("Choose a valid clip range within your length limit.".into());
                    return;
                };
                let input = player_create_clip::NewClip {
                    video_id: self.id,
                    recipe: player_create_clip::RecipeInput {
                        start,
                        end,
                        audio: editor.audio,
                        subtitles: editor.subtitle.clone(),
                        height: editor.height,
                        half_rate: editor.half_rate,
                    },
                    name: editor.name.clone(),
                    public: editor.public,
                    recipients: Vec::new(),
                    room,
                };
                editor.saving = true;
                editor.error = None;
                let key = editor.key;
                let api = self.api.clone();
                self.request(effects, async move {
                    let result = api
                        .request::<api::PlayerCreateClip>(player_create_clip::Variables { input })
                        .await
                        .map(|d| d.create_clip.id)
                        .map_err(|e| e.message);
                    super::Message::Editor(Message::Created(key, result))
                });
            },
            _ => {},
        }
    }

    pub fn tick_clip(&mut self, effects: &mut Vec<Effect>) {
        let Some(editor) = &mut self.clipping else { return };
        if let Some((start, end)) = editor.range(self.pb.as_ref().and_then(|pb| pb.media.duration).unwrap_or(0.0))
            && self.playing
            && self.time >= end
        {
            self.seek(start);
        }
        let Some(editor) = &mut self.clipping else { return };
        if let Some(id) = editor.made
            && editor.state != "Saved to Clips"
            && editor.state != "Failed"
            && editor.error.is_none()
            && editor.polled.elapsed() >= Duration::from_secs(1)
        {
            editor.polled = Instant::now();
            let api = self.api.clone();
            self.request(effects, async move {
                let result = api
                    .request::<api::PlayerClipStatus>(player_clip_status::Variables { id })
                    .await
                    .map_err(|e| e.message);
                super::Message::Editor(Message::Status(id, result))
            });
        }
    }
}

pub fn view<'a>(player: &'a Player, editor: &'a Editor, look: &Look) -> Element<'a, super::Message> {
    let msg = super::Message::Editor;
    let header = row![
        button("Back", look).plain().on_press(msg(Message::Close)).boxed(),
        text("Make a clip", type_::S15, look.ink).font(SEMIBOLD),
        space().width(Length::Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let picture: Element<'a, super::Message> = match &player.screen {
        Some(screen) => video(screen, player.aspect).boxed(),
        None => space().boxed(),
    };
    let controls = row![
        button(if player.playing { "Pause" } else { "Play" }, look).on_press(super::Message::Toggle).boxed(),
        button("Previous frame", look).plain().on_press(msg(Message::Step(true))).boxed(),
        button("Next frame", look).plain().on_press(msg(Message::Step(false))).boxed(),
        button("Start here (i)", look).plain().on_press(msg(Message::In)).boxed(),
        button("End here (o)", look).plain().on_press(msg(Message::Out)).boxed(),
    ]
    .spacing(8);
    let range = row![
        field(
            "Start (seconds)",
            input("clip-start", &editor.start, "0.000", move |s| msg(Message::Start(s)), look).boxed(),
            None,
            look
        ),
        field(
            "End (seconds)",
            input("clip-end", &editor.end, "30.000", move |s| msg(Message::End(s)), look).boxed(),
            None,
            look
        ),
        field(
            "Title",
            input("clip-name", &editor.name, "Optional", move |s| msg(Message::Name(s)), look).boxed(),
            None,
            look
        ),
    ]
    .spacing(16);
    let pb = player.pb.as_ref().expect("a clip has playback metadata");
    let audio_menu = select(
        Some(editor.audio),
        &editor.audio_options,
        |id| {
            id.and_then(|i| pb.media.audio.iter().find(|a| a.index == i)).map_or("None".into(), |a| {
                super::tracks::track_name(a.title.as_deref(), a.language.as_deref(), "Audio")
            })
        },
        move |a| msg(Message::Audio(a)),
        look,
    );
    let subtitle_menu = select(
        Some(editor.subtitle.clone()),
        &editor.subtitle_options,
        |id| {
            id.as_ref().and_then(|id| pb.media.subtitles.iter().find(|s| &s.id == id)).map_or("None".into(), |s| {
                super::tracks::track_name(s.title.as_deref(), s.language.as_deref(), "Subtitles")
            })
        },
        move |s| msg(Message::Subtitle(s)),
        look,
    );
    let quality_menu =
        select(Some(editor.height), &editor.heights, |h| format!("{h}p"), move |h| msg(Message::Height(h)), look);
    let tracks = row![
        field("Audio", audio_menu, None, look),
        field("Subtitles", subtitle_menu, Some("Burned into the clip"), look),
        field("Quality", quality_menu, None, look)
    ]
    .spacing(16);
    let options = row![
        toggle(editor.half_rate, |on| msg(Message::HalfRate(on)), look),
        text("Half frame rate", type_::SM, look.ink_2),
        toggle(editor.public, |on| msg(Message::Public(on)), look),
        text("Public link", type_::SM, look.ink_2)
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let save = button(if editor.saving { "Saving…" } else { "Make clip" }, look)
        .primary()
        .on_press_maybe(
            (!editor.saving
                && editor.made.is_none()
                && editor.allowance.as_ref().is_some_and(|a| a.can_clip)
                && editor.range(player.duration()).is_some())
            .then_some(msg(Message::Save)),
        )
        .boxed();
    let mut footer =
        column![controls, range, tracks, row![options, space().width(Length::Fill), save].align_y(Alignment::Center)]
            .spacing(14);
    if let Some(error) = &editor.error {
        footer = footer.push(text(error.clone(), type_::SM, look.danger).boxed());
    }
    if !editor.state.is_empty() {
        footer = footer.push(text(&editor.state, type_::SM, look.ink_2).boxed());
    }
    let canvas = look.canvas;
    container(column![header, container(picture).height(Length::Fill), footer].spacing(16))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| iced_widget::container::Style { background: Some(canvas.into()), ..Default::default() })
        .boxed()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::app::Message as AppMessage;
    use crate::player::Message as PlayerMessage;
    use crate::player::tests::player;

    #[test]
    fn handoff_saves_a_recipe_without_overwriting_watch_progress() {
        crate::test_support::run(async {
            let (origin, requests) = crate::test_support::http(3, |request| match request["operationName"].as_str().unwrap() {
                "SaveProgress" => json!({"data":{"saveProgress":{"id":1}}}),
                "PlayerClipAllowance" => json!({"data":{"clipAllowance":{"canClip":true,"canLink":true,"maxLength":60,"customDefaultFont":false}}}),
                "PlayerCreateClip" => json!({"data":{"createClip":{"id":42}}}),
                op => panic!("unexpected {op}"),
            }).await;
            let mut player = player();
            player.api = api::Api::new(origin, None).unwrap();
            player.can_clip = true;
            player.time = 76.25;
            player.audio = Some(2);
            player.subtitle = Some("s3".into());
            player.quality = Quality::Height(720);
            let mut effects = Vec::new();
            player.update(PlayerMessage::MakeClip, &mut effects);
            assert!(!player.playing);
            assert_eq!(player.clipping.as_ref().unwrap().at, 76.25);
            for effect in std::mem::take(&mut effects) {
                if let Effect::Request(request) = effect
                    && let AppMessage::Player(_, message) = request.await
                {
                    player.update(message, &mut effects);
                }
            }
            player.update(PlayerMessage::Editor(Message::Start("49.000".into())), &mut effects);
            player.update(PlayerMessage::Editor(Message::End("77.000".into())), &mut effects);
            player.update(PlayerMessage::Editor(Message::Save), &mut effects);
            for effect in std::mem::take(&mut effects) {
                if let Effect::Request(request) = effect
                    && let AppMessage::Player(_, message) = request.await
                {
                    player.update(message, &mut effects);
                }
            }
            assert_eq!(player.clipping.as_ref().unwrap().made, Some(42));
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
            assert_eq!(
                requests[2].1["variables"]["input"]["recipe"],
                json!({"start":49.0,"end":77.0,"audio":2,"subtitles":"s3","height":720,"halfRate":false})
            );
        });
    }

    #[test]
    fn invalid_ranges_and_stale_save_replies_are_rejected() {
        let mut player = player();
        player.can_clip = true;
        let mut effects = Vec::new();
        player.start_clip(&mut effects);
        let key = player.clipping.as_ref().unwrap().key;
        for (start, end) in [("NaN", "30"), ("0", "inf"), ("10", "9"), ("0", "0.1"), ("0", "9999")] {
            let editor = player.clipping.as_mut().unwrap();
            editor.start = start.into();
            editor.end = end.into();
            assert!(editor.range(1800.0).is_none());
        }
        player.edit_clip(Message::Close, &mut effects);
        player.start_clip(&mut effects);
        player.edit_clip(Message::Created(key, Ok(99)), &mut effects);
        assert!(player.clipping.as_ref().unwrap().made.is_none());
    }
}
