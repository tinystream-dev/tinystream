// SPDX-License-Identifier: AGPL-3.0-or-later
use iced_core::Alignment;
use iced_core::Length::Fill;
use iced_widget::{Column, Element, Row, Widget, column, container, image, row, scrollable};

use super::model::{Gain, Track};
use super::{Message, Music, Tab};
use crate::route::Route;
use crate::ui::Look;
use crate::ui::controls::{button, input};
use crate::ui::icon::Icon;
use crate::ui::text::{self as ty, text};

pub fn page<'a>(music: &'a Music, look: &Look) -> Element<'a, Message> {
    let mut body = Column::new().spacing(20).padding(28).width(Fill);
    if let Some(code) = &music.room_code {
        body = body.push(text("Listening together", ty::TITLE, look.ink).font(ty::SEMIBOLD).boxed());
        body = body.push(text(format!("Room {code}"), ty::SM, look.ink_3).boxed());
        body = body.push(button("Copy room link", look).on_press(Message::CopyRoom).boxed());
        if !music.room_joined {
            body = body.push(
                row![
                    input("guest-name", &music.guest_name, "Your name", Message::Name, look).boxed(),
                    button("Join", look).on_press(Message::JoinRoom).boxed()
                ]
                .spacing(8)
                .boxed(),
            );
        }
        if let Some(room) = &music.snapshot.room {
            body = body.push(text(format!("Hosted by {}", room.host.name), ty::SM, look.ink_2).boxed());
            for member in &room.members {
                body = body.push(text(format!("{} · {:?}", member.name, member.status), ty::SM, look.ink_2).boxed());
            }
            if music.can_invite {
                for (id, name) in &music.people {
                    body = body.push(
                        button(format!("Invite {name}"), look)
                            .small()
                            .on_press(Message::RoomCommand(serde_json::json!({"type":"invite","userId":id})))
                            .boxed(),
                    );
                }
            }
            if music.snapshot.you == Some(room.host.id) {
                body = body.push(
                    row![
                        button(if room.public { "Make private" } else { "Make public" }, look)
                            .on_press(Message::RoomCommand(
                                serde_json::json!({"type":"settings","public":!room.public})
                            ))
                            .boxed(),
                        button(if room.settings.wait_for_all { "Don't wait" } else { "Wait for all" }, look)
                            .on_press(Message::RoomCommand(
                                serde_json::json!({"type":"settings","waitForAll":!room.settings.wait_for_all})
                            ))
                            .boxed(),
                        button(if room.settings.control==crate::player::sync::Control::Everyone {"Host controls"}else{"Everyone controls"},look).on_press(Message::RoomCommand(serde_json::json!({"type":"settings","control":if room.settings.control==crate::player::sync::Control::Everyone{"host"}else{"everyone"}}))).boxed(),
                        button("End room", look)
                            .on_press(Message::RoomCommand(serde_json::json!({"type":"end"})))
                            .boxed()
                    ]
                    .spacing(8)
                    .boxed(),
                );
            }
        }
        if let Some(error) = &music.snapshot.error {
            body = body.push(text(error, ty::SM, look.danger).boxed());
        }
    } else if let Some(detail) = &music.detail {
        body = body.push(text(&detail.name, ty::TITLE, look.ink).font(ty::SEMIBOLD).boxed());
        body = body.push(text(&detail.artist, ty::BASE, look.ink_2).boxed());
        body = body.push(art(
            music,
            detail.cover.as_deref().or_else(|| detail.covers.first().map(String::as_str)),
            200.0,
            look,
        ));
        body = body.push(
            row![
                button("Play", look).primary().on_press(Message::Play(music.songs.clone(), 0)).boxed(),
                button("Listen together", look).on_press(Message::StartRoom).boxed()
            ]
            .spacing(8)
            .boxed(),
        );
        if matches!(music.route, Some(Route::Playlist(_))) && detail.mine {
            body = body.push(
                button(if detail.public { "Public playlist" } else { "Private playlist" }, look)
                    .on_press(Message::PlaylistPublic)
                    .boxed(),
            );
            body = body.push(
                button(
                    if music.delete == Some(detail.id) { "Confirm delete playlist" } else { "Delete playlist" },
                    look,
                )
                .on_press(if music.delete == Some(detail.id) {
                    Message::DeletePlaylist(detail.id)
                } else {
                    Message::AskDelete(detail.id)
                })
                .boxed(),
            );
            body = body.push(
                row![
                    input("playlist-name", &music.name, "Playlist name", Message::Name, look).boxed(),
                    button("Save", look).on_press(Message::SavePlaylist(detail.id)).boxed()
                ]
                .spacing(8)
                .boxed(),
            );
        }
    } else {
        body = body.push(text("Music", ty::TITLE, look.ink).font(ty::SEMIBOLD).boxed());
        let mut tabs = Row::new().spacing(8);
        for (tab, name) in
            [(Tab::Albums, "Albums"), (Tab::Artists, "Artists"), (Tab::Songs, "Songs"), (Tab::Playlists, "Playlists")]
        {
            tabs = tabs.push(button(name, look).on_press(Message::Tab(tab)).boxed());
        }
        body = body.push(tabs.boxed());
        if music.tab == Tab::Songs {
            body = body.push(input("song-search", &music.query, "Find a song", Message::Query, look).boxed());
        }
        if music.tab == Tab::Playlists {
            body = body.push(
                row![
                    input("new-playlist", &music.name, "Playlist name", Message::Name, look).boxed(),
                    button("Create playlist", look).on_press(Message::CreatePlaylist).boxed()
                ]
                .spacing(8)
                .boxed(),
            );
        }
    }
    if let Some(error) = &music.error {
        body = body.push(text(error, ty::SM, look.danger).boxed());
    }
    for cards in music.cards.chunks(4) {
        let mut line = Row::new().spacing(16);
        for card in cards {
            let route = if music.tab == Tab::Artists && music.detail.is_none() {
                Route::Artist(card.id as u64)
            } else if music.tab == Tab::Playlists && music.detail.is_none() {
                Route::Playlist(card.id as u64)
            } else {
                Route::Album(card.id as u64)
            };
            let content = column![
                art(music, card.cover.as_deref().or_else(|| card.covers.first().map(String::as_str)), 140.0, look),
                text(&card.name, ty::BASE, look.ink),
                text(&card.artist, ty::SM, look.ink_3)
            ]
            .spacing(8);
            line = line.push(button("", look).content(content).fill().on_press(Message::Go(route)).boxed());
        }
        body = body.push(line.boxed());
    }
    if music.room_code.is_some() {
        for (index, entry) in music.queue.iter().enumerate() {
            body = body.push(track_row(&entry.track, Message::Jump(index), look));
        }
    }
    for (index, track) in music.songs.iter().enumerate() {
        let track = track_row(track, Message::Play(music.songs.clone(), index), look);
        if matches!(music.route, Some(Route::Playlist(_))) && music.detail.as_ref().is_some_and(|d| d.mine) {
            body = body.push(
                row![
                    container(track).width(Fill),
                    control(
                        Icon::ArrowUp,
                        "Move up",
                        (index > 0).then_some(Message::PlaylistMove(index, -1)),
                        false,
                        false,
                        28.0,
                        look
                    ),
                    control(
                        Icon::ArrowDown,
                        "Move down",
                        (index + 1 < music.songs.len()).then_some(Message::PlaylistMove(index, 1)),
                        false,
                        false,
                        28.0,
                        look
                    ),
                    control(
                        Icon::Trash2,
                        "Remove from playlist",
                        Some(Message::PlaylistRemove(index)),
                        false,
                        false,
                        28.0,
                        look
                    )
                ]
                .spacing(4)
                .align_y(Alignment::Center)
                .boxed(),
            );
        } else {
            body = body.push(track);
        }
    }
    if music.loading {
        body = body.push(text("Loading music…", ty::SM, look.ink_3).boxed());
    } else if music.cards.is_empty() && music.songs.is_empty() {
        body = body.push(text("No music here yet", ty::BASE, look.ink_3).boxed());
    }
    if music.more {
        body = body.push(button("Load more", look).on_press(Message::More).boxed());
    }
    scrollable(body).height(Fill).boxed()
}
#[derive(Clone, Copy, PartialEq)]
enum TrackAction {
    Next,
    Queue,
    Rate(i64),
}
fn track_row<'a>(track: &'a Track, play: Message, look: &Look) -> Element<'a, Message> {
    const ACTIONS: [TrackAction; 8] = [
        TrackAction::Next,
        TrackAction::Queue,
        TrackAction::Rate(0),
        TrackAction::Rate(1),
        TrackAction::Rate(2),
        TrackAction::Rate(3),
        TrackAction::Rate(4),
        TrackAction::Rate(5),
    ];
    let more = crate::ui::select::icon_menu(
        Icon::Ellipsis,
        &ACTIONS,
        Some(TrackAction::Rate(track.rating.unwrap_or(0))),
        |a| match a {
            TrackAction::Next => "Play next".into(),
            TrackAction::Queue => "Add to queue".into(),
            TrackAction::Rate(0) => "Clear rating".into(),
            TrackAction::Rate(n) => format!("Rate {n} {}", if *n == 1 { "star" } else { "stars" }),
        },
        move |a| match a {
            TrackAction::Next => Message::Add(std::sync::Arc::new(track.clone()), true),
            TrackAction::Queue => Message::Add(std::sync::Arc::new(track.clone()), false),
            TrackAction::Rate(n) => Message::Rating(track.id, n),
        },
        look,
    );
    row![
        control(Icon::Play, "Play", Some(play), false, false, 32.0, look),
        column![
            crate::ui::text::truncate(&track.title, ty::SM, look.ink),
            crate::ui::text::truncate(format!("{} · {}", track.artist, track.album), ty::XS, look.ink_3)
        ]
        .spacing(2)
        .width(Fill),
        text(time(track.duration), ty::XS, look.ink_3),
        control(
            Icon::Star,
            if track.starred { "Unstar" } else { "Star" },
            Some(Message::Star(track.id, !track.starred)),
            track.starred,
            false,
            32.0,
            look
        ),
        crate::ui::controls::tip(more, "More", look)
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([8, 0])
    .boxed()
}
pub fn pill<'a>(music: &'a Music, look: &Look) -> Element<'a, Message> {
    let Some(current) = music.current().filter(|_| !music.dismissed && !music.suspended) else {
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
    if music.room_code.is_none() {
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
    music.index + 1 < music.queue.len() || music.repeat == super::model::Repeat::All
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
        if music.playing { Icon::Pause } else { Icon::Play },
        if music.playing { "Pause" } else { "Play" },
        Some(Message::Toggle),
        false,
        true,
        size,
        look,
    )
}
fn transport<'a>(music: &Music, look: &Look) -> Element<'a, Message> {
    row![
        control(Icon::Shuffle, "Shuffle", Some(Message::Shuffle), music.shuffled, false, 36.0, look),
        control(Icon::SkipBack, "Previous", Some(Message::Previous), false, false, 40.0, look),
        play(music, 48.0, look),
        control(Icon::SkipForward, "Next", has_next(music).then_some(Message::Next), false, false, 40.0, look),
        control(
            if music.repeat == super::model::Repeat::One { Icon::Repeat1 } else { Icon::Repeat },
            "Repeat",
            Some(Message::Repeat),
            music.repeat != super::model::Repeat::Off,
            false,
            36.0,
            look
        ),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .boxed()
}
fn scrubber<'a>(music: &Music, look: &Look) -> Element<'a, Message> {
    let duration = music.current().map_or(0.0, |e| e.track.duration).max(0.0);
    column![
        crate::player::timeline::volume((music.position / duration.max(1.0)).clamp(0.0, 1.0), look.ink, move |p| {
            Message::Seek(p * duration)
        }),
        row![
            text(time(music.position), ty::XS, look.ink_3),
            iced_widget::space().width(Fill),
            text(format!("-{}", time((duration - music.position).max(0.0))), ty::XS, look.ink_3)
        ],
    ]
    .spacing(2)
    .width(Fill)
    .boxed()
}
fn volume<'a>(music: &Music, look: &Look) -> Element<'a, Message> {
    let value = if music.settings.muted { 0.0 } else { music.settings.volume as f64 };
    row![
        control(
            if value == 0.0 {
                Icon::VolumeX
            } else if value < 0.5 {
                Icon::Volume1
            } else {
                Icon::Volume2
            },
            if music.settings.muted { "Unmute" } else { "Mute" },
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
    if music.room_code.is_none() {
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
    let mut body = column![
        header,
        cover,
        metadata(music, look, false),
        scrubber(music, look),
        container(transport(music, look)).center_x(Fill),
        footer
    ]
    .spacing(12)
    .padding(16)
    .width(352);
    if let Some(next) = music.queue.get(music.index + 1) {
        body = body.push(
            crate::ui::text::truncate(
                format!("Next: {} · {}", next.track.title, next.track.artist),
                ty::XS,
                look.ink_3,
            )
            .boxed(),
        );
    }
    if let Some(error) = &music.error {
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
    let left = column![
        container(art(music, entry.track.cover.as_deref(), (size.height - 430.0).clamp(150.0, 320.0), look))
            .center_x(Fill),
        metadata(music, look, true),
        scrubber(music, look),
        container(transport(music, look)).center_x(Fill),
        container(volume(music, look)).center_x(Fill)
    ]
    .spacing(20)
    .width((size.width * 0.38).clamp(260.0, 360.0));
    let body = row![container(left).center_y(Fill), panel(music, look)]
        .spacing(if size.width < 1000.0 { 32 } else { 56 })
        .height(Fill)
        .width(Fill);
    container(column![header, body].spacing(24).padding(32)).width(Fill).height(Fill).boxed()
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
                .rposition(|(at, _)| at.is_some_and(|t| t as f64 <= music.position * 1000.0 + 150.0));
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
                        "{} tracks · {} left",
                        music.queue.len(),
                        time(music.queue.iter().skip(music.index).map(|e| e.track.duration).sum())
                    ),
                    ty::XS,
                    look.ink_3,
                )
                .boxed(),
            );
            for (i, e) in music.queue.iter().enumerate() {
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
                        (i + 1 < music.queue.len()).then_some(Message::Move(i, 1)),
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
                let selected = i == music.index;
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
            for (label, value) in [
                ("Format", track.codec.to_uppercase()),
                ("Sample rate", track.sample_rate.map_or("—".into(), |n| format!("{} kHz", n as f64 / 1000.0))),
                ("Bit depth", track.bit_depth.map_or("—".into(), |n| format!("{n} bit"))),
                ("Bitrate", track.bitrate.map_or("—".into(), |n| format!("{n} kbit/s"))),
                ("Size", format!("{:.1} MB", track.size as f64 / 1048576.0)),
                (
                    "Getting here",
                    if current.fallback { "Converted to FLAC, losslessly" } else { "The original file, untouched" }
                        .into(),
                ),
                ("Output", format!("{} kHz", music.rate as f64 / 1000.0)),
            ] {
                body = body.push(
                    row![text(label, ty::SM, look.ink_3).width(120), text(value, ty::SM, look.ink)].spacing(16).boxed(),
                );
            }
            body = body.push(text("Volume levelling", ty::SM, look.ink_3).boxed());
            body = body.push(crate::ui::controls::segmented(
                music.settings.gain,
                &[(Gain::Auto, "Auto"), (Gain::Track, "Track"), (Gain::Album, "Album"), (Gain::Off, "Off")],
                Message::Gain,
                look,
            ));
            body = body.push(text("Crossfade", ty::SM, look.ink_3).boxed());
            body = body.push(
                row![
                    container(crate::player::timeline::volume(music.settings.crossfade / 12.0, look.ink, |p| {
                        Message::Fade((p * 12.0).round())
                    }))
                    .width(160),
                    text(
                        if music.settings.crossfade > 0.0 {
                            format!("{} s, never inside an album", music.settings.crossfade)
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

fn art<'a>(music: &'a Music, path: Option<&str>, size: f32, look: &Look) -> Element<'a, Message> {
    if let Some(handle) = path.and_then(|p| music.images.get(p)) {
        image(handle.clone()).width(size).height(size).content_fit(iced_core::ContentFit::Cover).boxed()
    } else {
        container(crate::ui::icon::icon(Icon::Music, 48.0).color(look.ink_3))
            .width(size)
            .height(size)
            .center_x(size)
            .center_y(size)
            .boxed()
    }
}
