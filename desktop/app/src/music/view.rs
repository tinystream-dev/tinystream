// SPDX-License-Identifier: AGPL-3.0-or-later
use iced_core::Length::Fill;
use iced_core::{Alignment, Color};
use iced_widget::{Column, Element, Widget, column, container, row, scrollable};

use super::model::Gain;
use super::pages::plural;
use super::{Message, Music};
use crate::ui::Look;
use crate::ui::effects::{self, Ambient};
use crate::ui::icon::Icon;
use crate::ui::text::{self as ty, text};

pub fn pill<'a>(music: &'a Music, look: &Look) -> Element<'a, Message> {
    let Some(current) = music.current().filter(|_| !music.player.dismissed && !music.player.suspended) else {
        return iced_widget::space().boxed();
    };
    let track = &current.track;
    if music.expanded {
        return super::dismiss::dismiss_on_outside_press(card(music, look));
    }
    let title = column![
        crate::ui::text::truncate(&track.title, ty::S13, look.ink).font(ty::MEDIUM),
        crate::ui::text::truncate(&track.artist, ty::XS, look.ink_3),
    ]
    .spacing(2)
    .width(192);
    let mut line = row![
        art(music, track.cover.as_deref(), 38.0, look),
        title,
        play(music, 36.0, look),
        control(Icon::SkipForward, "Next", has_next(music).then_some(Message::Next), false, false, 36.0, look)
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    if music.player.room.is_none() {
        line = line.push(control(Icon::X, "Close the player", Some(Message::Dismiss), false, false, 32.0, look));
    }
    let content = container(line).padding([7, 8]);
    let click = crate::ui::pressable::pressable(content, 26.0, look, {
        let l = *look;
        move |_| crate::ui::pressable::Appearance { paint: crate::ui::squircle::Paint::NONE, ink: l.ink }
    })
    .round()
    .on_press(Message::Expand);
    crate::ui::squircle::squircle(click, 26.0, look).fill(look.float).material(look.material()).edge().lift().boxed()
}
fn has_next(music: &Music) -> bool {
    music.player.index + 1 < music.player.queue.len() || music.player.repeat == super::model::Repeat::All
}
fn control<'a>(
    glyph: Icon,
    label: &'a str,
    message: Option<Message>,
    active: bool,
    primary: bool,
    size: f32,
    look: &Look,
) -> Element<'a, Message> {
    use crate::ui::pressable::{Appearance, Status, pressable};
    use crate::ui::squircle::Paint;
    let l = *look;
    let icon = crate::ui::icon::icon(glyph, if primary { size * 0.42 } else { 18.0 });
    let button = pressable(
        container(if primary { icon.filled() } else { icon }).center(size),
        size / 2.0,
        look,
        move |status| {
            let disabled = status == Status::Disabled;
            Appearance {
                paint: Paint {
                    fill: if primary {
                        Some(l.ink)
                    } else if matches!(status, Status::Hovered | Status::Pressed) {
                        Some(l.hover)
                    } else {
                        None
                    },
                    ..Paint::NONE
                },
                ink: if primary {
                    l.canvas
                } else if disabled {
                    l.ink_3.scale_alpha(0.35)
                } else if active {
                    l.accent
                } else {
                    l.ink_2
                },
            }
        },
    )
    .round()
    .on_press_maybe(message);
    crate::ui::controls::tip(button, label, look)
}
fn play<'a>(music: &Music, size: f32, look: &Look) -> Element<'a, Message> {
    control(
        if music.player.playing { Icon::Pause } else { Icon::Play },
        if music.player.playing { "Pause" } else { "Play" },
        Some(Message::Toggle),
        false,
        true,
        size,
        look,
    )
}
pub(super) fn transport<'a>(music: &Music, look: &Look) -> Element<'a, Message> {
    row![
        control(Icon::Shuffle, "Shuffle", Some(Message::Shuffle), music.player.shuffled, false, 36.0, look),
        control(Icon::SkipBack, "Previous", Some(Message::Previous), false, false, 40.0, look),
        play(music, 48.0, look),
        control(Icon::SkipForward, "Next", has_next(music).then_some(Message::Next), false, false, 40.0, look),
        control(
            if music.player.repeat == super::model::Repeat::One { Icon::Repeat1 } else { Icon::Repeat },
            "Repeat",
            Some(Message::Repeat),
            music.player.repeat != super::model::Repeat::Off,
            false,
            36.0,
            look
        ),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .boxed()
}
/// `<Elsewhere>`: the other app this player is mirroring, if any.
fn elsewhere<'a>(music: &Music, look: &Look) -> Option<Element<'a, Message>> {
    let remote = music.player.remote.as_ref()?;
    let what = format!("{} on {}", if remote.paused { "Paused" } else { "Playing" }, remote.client);
    row![
        crate::ui::icon::icon(Icon::MonitorSpeaker, 14.0).color(look.ink_3),
        crate::ui::text::truncate(what, ty::XS, look.ink_3)
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .boxed()
    .into()
}
pub(super) fn scrubber<'a>(music: &Music, look: &Look) -> Element<'a, Message> {
    let duration = music.current().map_or(0.0, |e| e.track.duration).max(0.0);
    column![
        crate::player::timeline::volume(
            (music.player.shown() / duration.max(1.0)).clamp(0.0, 1.0),
            look.ink,
            move |p| { Message::Seek(p * duration) }
        ),
        row![
            text(time(music.player.shown()), ty::XS, look.ink_3),
            iced_widget::space().width(Fill),
            text(format!("-{}", time((duration - music.player.shown()).max(0.0))), ty::XS, look.ink_3)
        ],
    ]
    .spacing(2)
    .width(Fill)
    .boxed()
}
pub(super) fn volume<'a>(music: &Music, look: &Look) -> Element<'a, Message> {
    let value = if music.player.settings.muted { 0.0 } else { music.player.settings.volume as f64 };
    row![
        control(
            if value == 0.0 {
                Icon::VolumeX
            } else if value < 0.5 {
                Icon::Volume1
            } else {
                Icon::Volume2
            },
            if music.player.settings.muted { "Unmute" } else { "Mute" },
            Some(Message::Mute),
            false,
            false,
            28.0,
            look
        ),
        container(crate::player::timeline::volume(value, look.ink, |v| Message::Volume(v as f32))).width(Fill)
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .width(128)
    .boxed()
}
fn metadata<'a>(music: &'a Music, look: &Look, large: bool) -> Element<'a, Message> {
    let track = &music.current().unwrap().track;
    row![
        column![
            crate::ui::text::truncate(&track.title, if large { ty::XL2 } else { ty::BASE }, look.ink)
                .font(ty::SEMIBOLD),
            crate::ui::text::truncate(format!("{} · {}", track.artist, track.album), ty::SM, look.ink_2)
        ]
        .spacing(4)
        .width(Fill),
        control(
            Icon::Star,
            if track.starred { "Unstar" } else { "Star" },
            Some(Message::Star(track.id, !track.starred)),
            track.starred,
            false,
            32.0,
            look
        )
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .boxed()
}
fn card<'a>(music: &'a Music, look: &Look) -> Element<'a, Message> {
    let l = *look;
    iced_widget::responsive(move |size| card_at(music, &l, (size.height - 395.0).clamp(140.0, 320.0)))
        .width(352)
        .height(iced_core::Length::Shrink)
        .boxed()
}
fn card_at<'a>(music: &'a Music, look: &Look, cover_size: f32) -> Element<'a, Message> {
    let track = &music.current().unwrap().track;
    let cover = crate::ui::pressable::pressable(
        container(art(music, track.cover.as_deref(), cover_size, look)).center_x(Fill),
        12.0,
        look,
        {
            let l = *look;
            move |_| crate::ui::pressable::Appearance { paint: crate::ui::squircle::Paint::NONE, ink: l.ink }
        },
    )
    .on_press(Message::Fullscreen);
    let mut header = row![
        control(Icon::ChevronDown, "Collapse player", Some(Message::Collapse), false, false, 28.0, look),
        iced_widget::space().width(Fill)
    ];
    if music.player.room.is_none() {
        header = header.push(control(Icon::X, "Close the player", Some(Message::Dismiss), false, false, 28.0, look));
    }
    let footer = row![
        volume(music, look),
        iced_widget::space().width(Fill),
        control(Icon::Captions, "Lyrics", Some(Message::Panel(0)), false, false, 28.0, look),
        control(Icon::ListMusic, "Queue", Some(Message::Panel(1)), false, false, 28.0, look),
        control(Icon::Maximize, "Open Now Playing", Some(Message::Fullscreen), false, false, 28.0, look)
    ]
    .align_y(Alignment::Center)
    .spacing(4);
    let mut body = column![header, cover, metadata(music, look, false)];
    if let Some(elsewhere) = elsewhere(music, look) {
        body = body.push(elsewhere);
    }
    let mut body = body
        .push(scrubber(music, look))
        .push(container(transport(music, look)).center_x(Fill).boxed())
        .push(footer.boxed())
        .spacing(12)
        .padding(16)
        .width(352);
    if let Some(next) = music.player.queue.get(music.player.index + 1) {
        body = body.push(
            crate::ui::text::truncate(
                format!("Next: {} · {}", next.track.title, next.track.artist),
                ty::XS,
                look.ink_3,
            )
            .boxed(),
        );
    }
    if let Some(error) = music.player.error.as_ref().or(music.error.as_ref()) {
        body = body.push(text(error, ty::SM, look.danger).boxed());
    }
    crate::ui::squircle::squircle(body, 26.0, look).fill(look.float).material(look.material()).edge().lift().boxed()
}
pub fn now_playing<'a>(music: &'a Music, look: &Look) -> Element<'a, Message> {
    let l = *look;
    iced_widget::responsive(move |size| now_playing_at(music, &l, size)).boxed()
}
fn now_playing_at<'a>(music: &'a Music, look: &Look, size: iced_core::Size) -> Element<'a, Message> {
    let Some(entry) = music.current() else { return iced_widget::space().boxed() };
    let tabs = crate::ui::controls::segmented(
        music.panel,
        &[(0, "Lyrics"), (1, "Queue"), (2, "Details")],
        Message::Panel,
        look,
    );
    let header = row![
        control(Icon::ChevronDown, "Close Now Playing", Some(Message::Collapse), false, false, 40.0, look),
        iced_widget::space().width(Fill),
        tabs
    ]
    .align_y(Alignment::Center)
    .padding([16, 0]);
    let mut left = column![
        container(art(music, entry.track.cover.as_deref(), (size.height - 430.0).clamp(150.0, 320.0), look))
            .center_x(Fill),
        metadata(music, look, true)
    ];
    if let Some(elsewhere) = elsewhere(music, look) {
        left = left.push(elsewhere);
    }
    let left = left
        .push(scrubber(music, look))
        .push(container(transport(music, look)).center_x(Fill).boxed())
        .push(container(volume(music, look)).center_x(Fill).boxed())
        .spacing(20)
        .width((size.width * 0.38).clamp(260.0, 360.0));
    let body = row![container(left).center_y(Fill), panel(music, look)]
        .spacing(if size.width < 1000.0 { 32 } else { 56 })
        .height(Fill)
        .width(Fill);
    let content = container(column![header, body].spacing(24).padding(32)).width(Fill).height(Fill);
    let mut layers = iced_widget::Stack::new().width(Fill).height(Fill);
    if let Some(crate::images::Picture::Ready(cover, _)) =
        entry.track.cover.as_deref().and_then(|p| music.images.picture(p))
    {
        layers = layers.push(effects::smear(cover.clone(), 90.0, 1.5, look.canvas.scale_alpha(0.55)));
    }
    let tint = entry
        .track
        .cover_tint
        .as_deref()
        .and_then(crate::ui::media::parse_tint)
        .unwrap_or(Color::from_rgb8(128, 128, 128));
    let canvas = look.canvas.scale_alpha(0.55);
    layers
        .push(effects::ambient(Ambient {
            color: tint.scale_alpha(0.25),
            size: (0.9, 0.8),
            center: (0.2, 0.3),
            reach: 0.7,
        }))
        .push(
            container(iced_widget::space().width(Fill).height(Fill))
                .style(move |_| container::Style { background: Some(canvas.into()), ..container::Style::default() })
                .boxed(),
        )
        .push(content.boxed())
        .boxed()
}
fn panel<'a>(music: &'a Music, look: &Look) -> Element<'a, Message> {
    let mut body = Column::new().spacing(12).width(Fill);
    let current = music.current().unwrap();
    let track = &current.track;
    match music.panel {
        0 => {
            let active = music
                .lyrics
                .iter()
                .rposition(|(at, _)| at.is_some_and(|t| t as f64 <= music.player.shown() * 1000.0 + 150.0));
            for (i, (at, line)) in music.lyrics.iter().enumerate() {
                let content = text(
                    if line.is_empty() { "♪" } else { line.as_str() },
                    ty::XL2,
                    if active == Some(i) { look.ink } else { look.ink_3 },
                )
                .font(ty::SEMIBOLD);
                body = body.push(
                    crate::ui::pressable::pressable(container(content).padding([8, 0]), 0.0, look, {
                        let l = *look;
                        move |_| crate::ui::pressable::Appearance {
                            paint: crate::ui::squircle::Paint::NONE,
                            ink: l.ink,
                        }
                    })
                    .on_press_maybe(at.map(|n| Message::Seek(n as f64 / 1000.0)))
                    .boxed(),
                );
            }
            if music.lyrics.is_empty() {
                body = body.push(text("No lyrics for this one.", ty::BASE, look.ink_3).boxed());
            }
        },
        1 => {
            body = body.push(
                text(
                    format!(
                        "{} track{} · {} left",
                        music.player.queue.len(),
                        plural(music.player.queue.len() as i64),
                        time(
                            music.player.queue.iter().skip(music.player.index).map(|e| e.track.duration).sum::<f64>()
                                - music.player.shown()
                        )
                    ),
                    ty::XS,
                    look.ink_3,
                )
                .boxed(),
            );
            for (i, e) in music.player.queue.iter().enumerate() {
                let content = row![
                    art(music, e.track.cover.as_deref(), 40.0, look),
                    column![
                        crate::ui::text::truncate(&e.track.title, ty::SM, look.ink),
                        crate::ui::text::truncate(&e.track.artist, ty::XS, look.ink_3)
                    ]
                    .width(Fill),
                    text(time(e.track.duration), ty::XS, look.ink_3),
                    control(
                        Icon::ArrowUp,
                        "Move up",
                        (i > 0).then_some(Message::Move(i, -1)),
                        false,
                        false,
                        28.0,
                        look
                    ),
                    control(
                        Icon::ArrowDown,
                        "Move down",
                        (i + 1 < music.player.queue.len()).then_some(Message::Move(i, 1)),
                        false,
                        false,
                        28.0,
                        look
                    ),
                    control(Icon::X, "Remove", Some(Message::Remove(i)), false, false, 28.0, look)
                ]
                .spacing(10)
                .align_y(Alignment::Center);
                let l = *look;
                let selected = i == music.player.index;
                body = body.push(
                    crate::ui::pressable::pressable(container(content).padding(8), 12.0, look, move |status| {
                        crate::ui::pressable::Appearance {
                            paint: crate::ui::squircle::Paint {
                                fill: if selected {
                                    Some(l.press)
                                } else if matches!(status, crate::ui::pressable::Status::Hovered) {
                                    Some(l.hover)
                                } else {
                                    None
                                },
                                ..crate::ui::squircle::Paint::NONE
                            },
                            ink: l.ink,
                        }
                    })
                    .on_press(Message::Jump(i))
                    .width(Fill)
                    .boxed(),
                );
            }
        },
        _ => {
            let direct = !current.fallback && super::player::DECODES.contains(&track.codec.as_str());
            let khz = |rate: u32| format!("{}", rate as f64 / 1000.0);
            let mut file = super::model::quality(track);
            if super::model::hi_res(track) {
                file.push_str(" · Hi-res");
            }
            if let Some(channels) = track.channels.filter(|&c| c != 2) {
                file.push_str(&format!(" · {channels} channels"));
            }
            for (label, value) in [
                ("File", file),
                ("Bitrate", track.bitrate.map_or("—".into(), |n| format!("{n} kbit/s"))),
                ("Size", format!("{:.1} MB", track.size as f64 / 1048576.0)),
                (
                    "Getting here",
                    if direct {
                        "The original file, untouched".into()
                    } else {
                        format!("Converted to FLAC on the server, losslessly ({} can't be decoded here)", track.codec)
                    },
                ),
                ("Decoded", "On this computer, gaplessly".into()),
                (
                    "Output",
                    match track.sample_rate.filter(|&r| r != music.rate) {
                        Some(rate) => {
                            format!("Resampled from {} kHz to your device's {} kHz", khz(rate), khz(music.rate))
                        },
                        None => format!("{} kHz, as recorded", khz(music.rate)),
                    },
                ),
            ] {
                body = body.push(
                    row![text(label, ty::SM, look.ink_3).width(120), text(value, ty::SM, look.ink)].spacing(16).boxed(),
                );
            }
            body = body.push(text("Volume levelling", ty::SM, look.ink_3).boxed());
            let g = &track.gains;
            let gain = match music.player.settings.gain {
                Gain::Off => None,
                Gain::Album => g.album_gain.or(g.track_gain),
                _ => g.track_gain,
            };
            let mut levelling = row![crate::ui::controls::segmented(
                music.player.settings.gain,
                &[(Gain::Auto, "Auto"), (Gain::Track, "Track"), (Gain::Album, "Album"), (Gain::Off, "Off")],
                Message::Gain,
                look,
            )]
            .spacing(8)
            .align_y(Alignment::Center);
            if let Some(gain) = gain {
                let measuring = if g.pending { " (measuring)" } else { "" };
                let sign = if gain > 0.0 { "+" } else { "" };
                levelling = levelling.push(text(format!("{sign}{gain:.1} dB{measuring}"), ty::XS, look.ink_3).boxed());
            }
            body = body.push(levelling.boxed());
            body = body.push(text("Crossfade", ty::SM, look.ink_3).boxed());
            body = body.push(
                row![
                    container(crate::player::timeline::volume(music.player.settings.crossfade / 12.0, look.ink, |p| {
                        Message::Fade((p * 12.0).round())
                    }))
                    .width(160),
                    text(
                        if music.player.settings.crossfade > 0.0 {
                            format!("{} s, never inside an album", music.player.settings.crossfade)
                        } else {
                            "Off".into()
                        },
                        ty::XS,
                        look.ink_3
                    )
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .boxed(),
            );
        },
    }
    scrollable(body).height(Fill).width(Fill).boxed()
}
fn time(seconds: f64) -> String {
    let n = seconds.max(0.0) as u64;
    format!("{}:{:02}", n / 60, n % 60)
}

pub(super) fn art<'a>(music: &'a Music, path: Option<&str>, size: f32, look: &Look) -> Element<'a, Message> {
    let radius = (size / 14.0).round().max(6.0);
    let content = match path.and_then(|p| music.images.picture(p)) {
        picture @ Some(crate::images::Picture::Ready(..) | crate::images::Picture::Loading) => {
            crate::ui::media::img(picture, radius, look)
        },
        _ => crate::ui::squircle::squircle(
            container(crate::ui::icon::icon(Icon::Music, size / 4.0).color(look.ink_3)).center(Fill),
            radius,
            look,
        )
        .fill(look.raised)
        .width(Fill)
        .height(Fill)
        .boxed(),
    };
    container(content).width(size).height(size).boxed()
}
