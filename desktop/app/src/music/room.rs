// SPDX-License-Identifier: AGPL-3.0-or-later

//! A listening room (`listen.$code.tsx`): joining it, what's on with everyone's controls, and
//! what's coming up.

use iced_core::Length::Fill;
use iced_core::{Alignment, Padding};
use iced_widget::{Column, Element, Row, Stack, Widget, column, container, responsive, row, scrollable, space, stack};

use super::listen::State;
use super::model::Entry;
use super::{Message, Music, RoomAction};
use crate::player::sync::Status;
use crate::route::Route;
use crate::ui::controls::{button, input, spinner};
use crate::ui::effects::{self, Ambient};
use crate::ui::icon::{Icon, icon};
use crate::ui::pressable::{Appearance, Status as Press, pressable};
use crate::ui::squircle::Paint;
use crate::ui::text::{self as ty, MEDIUM, SEMIBOLD, text, truncate};
use crate::ui::{Look, media};

type View<'a> = Element<'a, Message>;

const HEADLINE: ty::Size = ty::Size { px: 22.0, line: 28.6 };

pub fn view<'a>(music: &'a Music, signed_in: bool, look: &Look) -> View<'a> {
    if let Some(gone) = &music.room_gone {
        return self::gone(gone, signed_in, look);
    }
    if !music.room_joined {
        if music.loading && music.queue.is_empty() {
            return center(spinner(36.0, look), look);
        }
        return join(music, signed_in, look);
    }
    let (Some(room), Some(entry)) = (&music.snapshot.room, music.current()) else {
        return center(spinner(36.0, look), look);
    };
    let l = *look;
    let tint = entry.track.cover_tint.as_deref().and_then(media::parse_tint);
    let mut layers = Stack::new().width(Fill).height(Fill);
    if let Some(color) = tint {
        layers = layers.push(effects::ambient(Ambient {
            color: color.scale_alpha(0.3),
            size: (1.1, 1.1),
            center: (0.25, 0.45),
            reach: 0.8,
        }));
    }
    layers.push(responsive(move |size| inside(music, room, entry, size, &l)).boxed()).boxed()
}

fn inside<'a>(music: &'a Music, room: &'a State, entry: &'a Entry, size: iced_core::Size, look: &Look) -> View<'a> {
    let host = if music.is_host { "you".to_owned() } else { room.host.name.clone() };
    let mut header = row![
        icon(Icon::Headphones, 18.0).color(look.ink_2),
        text(format!("Listening with {host}"), ty::SM, look.ink).font(MEDIUM)
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .height(56);
    if !music.snapshot.connected {
        header = header.push(text("Reconnecting…", ty::XS, look.warn).boxed());
    }
    if let Some(error) = music.snapshot.error.as_ref().or(music.error.as_ref()) {
        header = header.push(text(error, ty::XS, look.danger).boxed());
    }
    header = header.push(space().width(Fill).boxed()).push(members(room, look)).push(invite(music, look));

    let width = (size.height * 0.48).min(448.0).min(size.width * 0.42).max(160.0);
    let track = &entry.track;
    let about = row![
        column![
            truncate(&track.title, ty::XL2, look.ink).font(SEMIBOLD),
            truncate(format!("{} · {}", track.artist, track.album), ty::S15, look.ink_2)
        ]
        .width(Fill),
        super::pages::quality(track, look)
    ]
    .spacing(12);
    let now = column![
        super::view::art(music, track.cover.as_deref(), width, look),
        container(about).padding(Padding { top: 24.0, ..Padding::ZERO }),
        container(super::view::scrubber(music, look)).padding(Padding { top: 16.0, ..Padding::ZERO }),
        container(super::view::transport(music, look)).center_x(Fill).padding(Padding { top: 8.0, ..Padding::ZERO }),
        container(super::view::volume(music, look)).center_x(Fill).padding(Padding { top: 16.0, ..Padding::ZERO }),
    ]
    .width(width);
    let mut up_next = Column::new().push(
        container(text("Up next", ty::XS, look.ink_3))
            .padding(Padding { left: 8.0, bottom: 8.0, ..Padding::ZERO })
            .boxed(),
    );
    for (i, entry) in music.queue.iter().enumerate() {
        up_next = up_next.push(queued(music, entry, i, look));
    }
    let body = row![
        container(now).center_y(Fill),
        scrollable(up_next.width(Fill).padding(Padding { right: 4.0, ..Padding::ZERO })).width(Fill).height(Fill)
    ]
    .spacing(if size.width < 1000.0 { 40 } else { 64 })
    .height(Fill);
    container(
        column![
            container(header).padding(Padding { right: 112.0, ..Padding::ZERO }),
            container(body).padding(Padding { top: 24.0, ..Padding::ZERO })
        ]
        .width(Fill.max(1300)),
    )
    .center_x(Fill)
    .padding(Padding { top: 16.0, right: 48.0, bottom: 24.0, left: 48.0 })
    .boxed()
}

/// Who's in the room, with a pulsing dot on whoever isn't ready yet.
fn members<'a>(room: &'a State, look: &Look) -> View<'a> {
    let mut row = Row::new().spacing(4).align_y(Alignment::Center);
    for member in room.members.iter().take(8) {
        let fallback = member.user_id.is_none().then(|| {
            text(member.name.replace("Guest ", "").chars().take(2).collect::<String>(), ty::XS2, look.ink_2)
                .font(MEDIUM)
                .boxed()
        });
        let mut face = Stack::new().push(media::avatar(&member.name, None, 28.0, fallback, look));
        if member.status != Status::Ready {
            face = face
                .push(container(effects::pulse_dot(10.0, look.warn, look)).align_right(28).align_bottom(28).boxed());
        }
        row = row.push(crate::ui::controls::tip(face, &member.name, look));
    }
    if room.members.len() > 8 {
        row = row.push(text(format!("+{}", room.members.len() - 8), ty::XS, look.ink_3).boxed());
    }
    row.boxed()
}

/// `<RoomMenu>`: the link, who can join and control it, inviting people, and leaving or ending it.
fn invite<'a>(music: &'a Music, look: &Look) -> View<'a> {
    let content = container(
        row![icon(Icon::Users, 16.0), text("Invite", ty::SM, look.ink)].spacing(8).align_y(Alignment::Center),
    )
    .center_y(36)
    .padding([0, 12])
    .boxed();
    crate::ui::select::menu(
        content,
        &music.room_menu,
        |action| match action {
            RoomAction::Copy => "Copy link".into(),
            RoomAction::Public(false) => "Anyone with the link".into(),
            RoomAction::Public(true) => "Only people here can join".into(),
            RoomAction::Everyone(false) => "Everyone controls it".into(),
            RoomAction::Everyone(true) => "Only you control it".into(),
            RoomAction::Invite(_, name) => format!("Invite {name}"),
            RoomAction::End => "End the room".into(),
            RoomAction::Leave => "Leave".into(),
        },
        Message::RoomMenu,
        256.0,
        look,
    )
}

fn queued<'a>(music: &'a Music, entry: &'a Entry, i: usize, look: &Look) -> View<'a> {
    let current = i == music.index;
    let fade = |c: iced_core::Color| if i < music.index { c.scale_alpha(0.45) } else { c };
    let mut cover = Stack::new().push(super::view::art(music, entry.track.cover.as_deref(), 40.0, look));
    if current {
        cover = cover.push(
            crate::ui::squircle::squircle(container(super::pages::bars(look.media_ink)).center(40), 6.0, look)
                .fill(look.media_shade.scale_alpha(0.55))
                .boxed(),
        );
    }
    let content = row![
        cover,
        column![
            truncate(&entry.track.title, ty::SM, fade(look.ink)),
            truncate(&entry.track.artist, ty::XS, fade(look.ink_3))
        ]
        .width(Fill)
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let l = *look;
    pressable(content, 12.0, look, move |status| Appearance {
        paint: Paint {
            fill: if current {
                Some(l.press)
            } else if matches!(status, Press::Hovered | Press::Pressed) {
                Some(l.hover)
            } else {
                None
            },
            ..Paint::NONE
        },
        ink: l.ink,
    })
    .width(Fill)
    .padding([6, 8])
    .on_press_maybe((!current).then_some(Message::Jump(i)))
    .boxed()
}

/// Before joining: whose room it is, and a name for guests.
fn join<'a>(music: &'a Music, signed_in: bool, look: &Look) -> View<'a> {
    let badge = container(icon(Icon::Headphones, 28.0).color(look.ink_2)).center(64).style({
        let panel = look.panel;
        move |_| container::Style {
            background: Some(panel.into()),
            border: iced_core::Border::default().rounded(32),
            ..container::Style::default()
        }
    });
    let title =
        if music.is_host { "Your listening room".to_owned() } else { format!("{} is listening", music.host_name) };
    let mut content = column![
        badge,
        container(text(title, HEADLINE, look.ink).font(SEMIBOLD).center())
            .padding(Padding { top: 20.0, ..Padding::ZERO }),
        container(
            text("Everyone here hears the same thing at the same moment.", ty::S15, look.ink_2)
                .center()
                .line_height(iced_core::text::LineHeight::Relative(1.625))
        )
        .padding(Padding { top: 8.0, ..Padding::ZERO }),
    ]
    .align_x(Alignment::Center)
    .width(Fill.max(384));
    if !signed_in {
        content = content.push(
            container(
                input("guest-name", &music.guest_name, "Your name", Message::Name, look)
                    .on_submit(Message::JoinRoom)
                    .boxed(),
            )
            .padding(Padding { top: 24.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    content = content.push(
        container(
            button(if music.is_host { "Start listening" } else { "Join" }, look)
                .primary()
                .large()
                .icon(Icon::Headphones)
                .on_press(Message::JoinRoom)
                .boxed(),
        )
        .padding(Padding { top: if signed_in { 24.0 } else { 12.0 }, ..Padding::ZERO })
        .boxed(),
    );
    if let Some(error) = &music.error {
        content = content
            .push(container(text(error, ty::SM, look.danger)).padding(Padding { top: 12.0, ..Padding::ZERO }).boxed());
    }
    center(content.boxed(), look)
}

fn gone<'a>(message: &'a str, signed_in: bool, look: &Look) -> View<'a> {
    let mut content = column![
        text("Nothing playing here", HEADLINE, look.ink).font(SEMIBOLD),
        text(message, ty::S15, look.ink_2).center(),
    ]
    .spacing(8)
    .align_x(Alignment::Center)
    .width(Fill.max(384));
    if signed_in {
        content = content.push(
            container(button("Go home", look).large().on_press(Message::Go(Route::Home)).boxed())
                .padding(Padding { top: 16.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    center(content.boxed(), look)
}

fn center<'a>(content: View<'a>, look: &Look) -> View<'a> {
    let canvas = look.canvas;
    container(stack![content])
        .center(Fill)
        .padding(24)
        .style(move |_| container::Style { background: Some(canvas.into()), ..container::Style::default() })
        .boxed()
}
