// SPDX-License-Identifier: AGPL-3.0-or-later
use iced_core::Alignment;
use iced_core::Length::Fill;
use iced_widget::{Column, Element, Row, Widget, column, container, image, row, scrollable};

use super::model::{Gain, Track};
use super::{Message, Music, Tab};
use crate::route::Route;
use crate::ui::Look;
use crate::ui::controls::{button, icon_button, input};
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
        body = body.push(track_row(track, Message::Play(music.songs.clone(), index), look));
        if matches!(music.route, Some(Route::Playlist(_))) && music.detail.as_ref().is_some_and(|d| d.mine) {
            body = body.push(
                row![
                    button("Move up", look).small().on_press(Message::PlaylistMove(index, -1)).boxed(),
                    button("Move down", look).small().on_press(Message::PlaylistMove(index, 1)).boxed(),
                    button("Remove", look).small().on_press(Message::PlaylistRemove(index)).boxed()
                ]
                .spacing(6)
                .boxed(),
            );
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
fn track_row<'a>(track: &'a Track, play: Message, look: &Look) -> Element<'a, Message> {
    row![
        icon_button(Icon::Play, "Play", Some(play), look),
        column![
            text(&track.title, ty::BASE, look.ink),
            text(format!("{} · {}", track.artist, track.album), ty::SM, look.ink_3)
        ]
        .width(Fill),
        text(time(track.duration), ty::SM, look.ink_3),
        button(format!("{} ★", track.rating.unwrap_or(0)), look)
            .small()
            .plain()
            .on_press(Message::Rating(track.id, (track.rating.unwrap_or(0) + 1) % 6))
            .boxed(),
        button(if track.starred { "Unstar" } else { "Star" }, look)
            .small()
            .plain()
            .on_press(Message::Star(track.id, !track.starred))
            .boxed(),
        button("Next", look).small().plain().on_press(Message::Add(std::sync::Arc::new(track.clone()), true)).boxed(),
        button("Queue", look).small().plain().on_press(Message::Add(std::sync::Arc::new(track.clone()), false)).boxed(),
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
    let controls = row![
        icon_button(Icon::SkipBack, "Previous", Some(Message::Previous), look),
        icon_button(if music.playing { Icon::Pause } else { Icon::Play }, "Play / pause", Some(Message::Toggle), look),
        icon_button(Icon::SkipForward, "Next", Some(Message::Next), look),
        button(track.title.clone(), look).plain().on_press(Message::Expand).boxed(),
        text(time(music.position), ty::SM, look.ink_3),
        icon_button(
            if music.fullscreen { Icon::Minimize } else { Icon::Maximize },
            "Full screen",
            Some(Message::Fullscreen),
            look
        ),
        icon_button(Icon::X, "Dismiss", (!music.playing).then_some(Message::Dismiss), look),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let mut body = column![controls].spacing(12).padding(16);
    if music.expanded {
        body = body.push(art(music, track.cover.as_deref(), 120.0, look));
        body = body.push(text(format!("{} · {}", track.artist, track.album), ty::BASE, look.ink_2).boxed());
        body = body.push(
            crate::player::timeline::volume(music.position / track.duration.max(1.0), look.ink, {
                let duration = track.duration;
                move |p| Message::Seek(p * duration)
            })
            .boxed(),
        );
        body = body.push(
            row![
                button(if music.shuffled { "Unshuffle" } else { "Shuffle" }, look).on_press(Message::Shuffle).boxed(),
                button(format!("Repeat {:?}", music.repeat), look).on_press(Message::Repeat).boxed(),
                button("Listen together", look).on_press(Message::StartRoom).boxed()
            ]
            .spacing(8)
            .boxed(),
        );
        body = body.push(
            row![
                button("Lyrics", look).on_press(Message::Panel(0)).boxed(),
                button("Queue", look).on_press(Message::Panel(1)).boxed(),
                button("Info", look).on_press(Message::Panel(2)).boxed()
            ]
            .spacing(8)
            .boxed(),
        );
        let mut panel = Column::new().spacing(8);
        match music.panel {
            0 => {
                for (at, line) in &music.lyrics {
                    panel = panel.push(
                        button(line.clone(), look)
                            .plain()
                            .on_press_maybe(at.map(|n| Message::Seek(n as f64 / 1000.0)))
                            .boxed(),
                    );
                }
                if music.lyrics.is_empty() {
                    panel = panel.push(text("No lyrics available", ty::SM, look.ink_3).boxed());
                }
            },
            1 => {
                for (i, entry) in music.queue.iter().enumerate() {
                    panel = panel.push(
                        row![
                            button(entry.track.title.clone(), look).plain().on_press(Message::Jump(i)).fill().boxed(),
                            button("↑", look).small().on_press(Message::Move(i, -1)).boxed(),
                            button("↓", look).small().on_press(Message::Move(i, 1)).boxed(),
                            icon_button(Icon::X, "Remove", Some(Message::Remove(i)), look)
                        ]
                        .spacing(4)
                        .boxed(),
                    );
                }
            },
            _ => {
                panel = panel.push(
                    text(
                        format!(
                            "{} · {} Hz · {:?} bit · {}",
                            track.codec.to_uppercase(),
                            track.sample_rate.unwrap_or(music.rate),
                            track.bit_depth,
                            if current.fallback { "FLAC" } else { "Original file" }
                        ),
                        ty::SM,
                        look.ink_2,
                    )
                    .boxed(),
                );
                let mut gains = Row::new().spacing(6);
                for (gain, label) in
                    [(Gain::Off, "Gain off"), (Gain::Track, "Track"), (Gain::Album, "Album"), (Gain::Auto, "Auto")]
                {
                    gains = gains.push(button(label, look).small().on_press(Message::Gain(gain)).boxed());
                }
                panel = panel.push(gains.boxed());
                panel = panel.push(
                    row![
                        button("No crossfade", look).small().on_press(Message::Fade(0.0)).boxed(),
                        button("Crossfade 3s", look).small().on_press(Message::Fade(3.0)).boxed(),
                        button("Crossfade 5s", look).small().on_press(Message::Fade(5.0)).boxed()
                    ]
                    .spacing(6)
                    .boxed(),
                );
                panel = panel.push(
                    crate::player::timeline::volume(music.settings.volume as f64, look.ink, |v| {
                        Message::Volume(v as f32)
                    })
                    .boxed(),
                );
            },
        }
        body = body.push(scrollable(panel).height(180).boxed());
    }
    if let Some(error) = &music.error {
        body = body.push(text(error, ty::SM, look.danger).boxed());
    }
    container(body)
        .width(if music.fullscreen { Fill } else { iced_core::Length::Fixed(640.0) })
        .style({
            let fill = look.panel;
            move |_| container::Style {
                background: Some(fill.into()),
                border: iced_core::Border { radius: 20.0.into(), ..Default::default() },
                ..Default::default()
            }
        })
        .boxed()
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
