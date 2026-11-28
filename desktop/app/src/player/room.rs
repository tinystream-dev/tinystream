// SPDX-License-Identifier: AGPL-3.0-or-later

//! The watch-together parts of the player (`Room.tsx`): who's here, the link, invites, the room's
//! settings, and quiet notices when someone else does something.

use std::time::{Duration, Instant};

use iced_core::Length::Fill;
use iced_core::{Alignment, Padding};
use iced_widget::{Column, Element, Row, Widget, column, container, row, scrollable, space};

use super::sync::{Action, Control, Member, Snapshot, Status, SyncClient, can_control};
use crate::api;
use crate::ui::Look;
use crate::ui::controls::{button, divider, input, panel, segmented, toggle};
use crate::ui::icon::{Icon, icon};
use crate::ui::media::avatar;
use crate::ui::pressable::{Appearance, Status as Press, pressable};
use crate::ui::squircle::{Paint, squircle};
use crate::ui::text::{self as type_, MEDIUM, text, truncate};

/// How long a notice stays.
const NOTICE: Duration = Duration::from_millis(2600);
pub const GUEST_NAME: &str = "guest-name";

#[derive(Debug, Clone)]
pub enum Message {
    Panel(bool),
    Copy,
    Name(String),
    SaveName,
    Public(bool),
    Control(Control),
    WaitForAll(bool),
    Invite(i64),
    End,
}

/// A room, as the player in it sees it.
pub struct Room {
    pub client: SyncClient,
    pub code: String,
    /// The link to share, on the server's own address.
    pub link: String,
    pub signed_in: bool,
    pub is_host: bool,
    pub can_share: bool,
    pub can_invite: bool,
    /// Everyone on the server, to invite.
    pub people: Vec<api::people::PeopleUsers>,
    pub snap: Snapshot,
    pub panel: bool,
    ending: bool,
    pub name: String,
    pub notice: Option<(String, Instant)>,
    seen: Option<u64>,
    copied: Option<Instant>,
    /// Whether we picked our own tracks rather than the room's.
    pub own_audio: bool,
    pub own_subtitle: bool,
}

impl Room {
    pub fn new(
        client: SyncClient,
        code: String,
        link: String,
        signed_in: bool,
        is_host: bool,
        can_share: bool,
        can_invite: bool,
    ) -> Self {
        Self {
            snap: client.snapshot(),
            client,
            code,
            link,
            signed_in,
            is_host,
            can_share,
            can_invite,
            people: Vec::new(),
            // Whoever started the room sees the link straight away.
            panel: is_host,
            ending: false,
            name: String::new(),
            notice: None,
            seen: None,
            copied: None,
            own_audio: false,
            own_subtitle: false,
        }
    }

    pub fn can_control(&self) -> bool {
        self.snap.room.as_ref().is_some_and(|room| can_control(room, self.snap.you))
    }

    /// Who controls playback here, when it isn't us.
    pub fn controlled_by(&self) -> Option<&str> {
        self.snap.room.as_ref().filter(|room| !can_control(room, self.snap.you)).map(|room| room.host.name.as_str())
    }

    pub fn me(&self) -> Option<&Member> {
        self.snap.room.as_ref()?.members.iter().find(|m| Some(m.id) == self.snap.you)
    }

    /// Takes in a new snapshot, with a notice for what someone else did.
    pub fn changed(&mut self, snap: Snapshot) {
        if let Some(action) = snap.room.as_ref().and_then(|room| room.action.as_ref()) {
            // Whatever happened before we arrived isn't news.
            if self.seen.is_none_or(|seen| action.seq <= seen) {
                self.seen = Some(self.seen.unwrap_or(0).max(action.seq));
            } else {
                self.seen = Some(action.seq);
                if Some(action.member) != snap.you
                    && let Some(words) = describe(action)
                {
                    self.notice = Some((words, Instant::now()));
                }
            }
        }
        if let Some((error, at)) = &snap.error
            && self.snap.error.as_ref().is_none_or(|(_, before)| before != at)
        {
            self.notice = Some((error.clone(), Instant::now()));
        }
        if self.name.is_empty()
            && let Some(me) = snap.room.as_ref().and_then(|room| room.members.iter().find(|m| Some(m.id) == snap.you))
        {
            self.name = me.name.clone();
        }
        self.snap = snap;
    }

    pub fn tick(&mut self) {
        if self.notice.as_ref().is_some_and(|(_, at)| at.elapsed() >= NOTICE) {
            self.notice = None;
        }
        if self.copied.is_some_and(|at| at.elapsed() >= Duration::from_millis(1600)) {
            self.copied = None;
        }
    }

    pub fn animating(&self) -> bool {
        self.notice.is_some() || self.copied.is_some()
    }

    /// Handles `message`; returns the link when it's to be copied.
    pub fn update(&mut self, message: Message) -> Option<String> {
        match message {
            Message::Panel(open) => {
                self.panel = open;
                self.ending = false;
            },
            Message::Copy => {
                self.copied = Some(Instant::now());
                return Some(self.link.clone());
            },
            Message::Name(name) => self.name = name.chars().take(40).collect(),
            Message::SaveName => {
                let name = self.name.trim().to_owned();
                if !name.is_empty() && self.me().is_some_and(|me| me.name != name) {
                    if let Some(file) = crate::paths::config_dir().map(|dir| dir.join(GUEST_NAME)) {
                        let _ = std::fs::create_dir_all(file.parent().unwrap_or(&file));
                        let _ = std::fs::write(file, &name);
                    }
                    self.client.rename(name);
                }
            },
            Message::Public(public) => self.client.settings(None, None, Some(public)),
            Message::Control(control) => self.client.settings(Some(control), None, None),
            Message::WaitForAll(wait) => self.client.settings(None, Some(wait), None),
            Message::Invite(user) => self.client.invite(user),
            Message::End => {
                if self.ending {
                    self.client.end();
                } else {
                    self.ending = true;
                }
            },
        }
        None
    }
}

/// The guest name used last, if any.
pub fn guest_name() -> Option<String> {
    let file = crate::paths::config_dir()?.join(GUEST_NAME);
    std::fs::read_to_string(file).ok().map(|name| name.trim().to_owned()).filter(|name| !name.is_empty())
}

fn describe(a: &Action) -> Option<String> {
    Some(match a.kind.as_str() {
        "play" => format!("{} pressed play", a.by),
        "pause" => format!("{} paused", a.by),
        "seek" => format!("{} jumped to {}", a.by, super::view::clock(a.value.unwrap_or(0.0))),
        "rate" => format!("{} set the speed to {}×", a.by, super::number(a.value.unwrap_or(1.0))),
        "media" => format!("{} changed the episode", a.by),
        "tracks" => format!("{} changed the audio or subtitles", a.by),
        "join" => format!("{} joined", a.by),
        "leave" => format!("{} left", a.by),
        _ => return None,
    })
}

fn status_text(status: Status) -> Option<&'static str> {
    match status {
        Status::Ready => None,
        Status::Joining => Some("Joining"),
        Status::Loading => Some("Loading"),
        Status::Buffering => Some("Catching up"),
        Status::Blocked => Some("Needs a tap"),
    }
}

/// Where a member's picture is, if they have one.
pub fn avatar_path(code: &str, m: &Member) -> Option<String> {
    match (m.user_id, m.avatar) {
        (Some(user), Some(version)) => Some(format!("api/together/{code}/avatars/{user}?v={version}")),
        _ => None,
    }
}

fn face<'a, M: 'a>(room: &Room, m: &Member, pictures: &super::Pictures, size: f32, look: &Look) -> Element<'a, M> {
    let picture = avatar_path(&room.code, m).and_then(|path| pictures.get(&path));
    avatar(&m.name, picture, size, None, look)
}

/// Everyone's faces in the corner; opens the room panel.
pub fn button_view<'a, M: Clone + 'a>(
    room: &'a Room,
    pictures: &'a super::Pictures,
    alpha: f32,
    to: impl Fn(Message) -> M + 'a,
    look: &Look,
) -> Element<'a, M> {
    let l = *look;
    let ink = l.media_ink;
    let Some(state) = &room.snap.room else { return space().boxed() };
    let mut faces = Row::new().spacing(-8.0);
    for m in state.members.iter().take(3) {
        faces = faces.push(face(room, m, pictures, 26.0, look));
    }
    let waiting = state.members.iter().any(|m| matches!(m.status, Status::Buffering | Status::Loading));
    let mut content = row![faces, text(state.members.len().to_string(), type_::S13, ink.scale_alpha(alpha))]
        .spacing(8)
        .align_y(Alignment::Center);
    if waiting || !room.snap.connected {
        content = content.push(squircle(space().width(6).height(6), 3.0, look).round().fill(l.warn).boxed());
    }
    let open = room.panel;
    pressable(
        container(content).center_y(40).padding(Padding { left: 6.0, right: 12.0, ..Padding::ZERO }),
        12.0,
        look,
        move |status| {
            let hovered = matches!(status, Press::Hovered | Press::Pressed);
            Appearance {
                paint: Paint { fill: (hovered || open).then(|| ink.scale_alpha(0.1 * alpha)), ..Paint::NONE },
                ink: ink.scale_alpha(alpha),
            }
        },
    )
    .on_press(to(Message::Panel(!open)))
    .boxed()
}

/// The room panel: the link, who's here, and the host's settings.
pub fn panel_view<'a, M: Clone + 'a>(
    room: &'a Room,
    pictures: &'a super::Pictures,
    to: impl Fn(Message) -> M + Copy + 'a,
    look: &Look,
) -> Element<'a, M> {
    let l = *look;
    let Some(state) = &room.snap.room else { return space().boxed() };
    let heading = |label: &'a str| {
        container(text(label, type_::XS2, l.ink_3).font(MEDIUM))
            .padding(Padding { top: 16.0, right: 4.0, bottom: 4.0, left: 4.0 })
            .boxed()
    };
    let mut title = row![text("Watching together", type_::S15, l.ink).font(MEDIUM).width(Fill)];
    if !room.snap.connected {
        title = title.push(text("Reconnecting…", type_::XS, l.warn).boxed());
    }
    let shown = room.link.trim_start_matches("https://").trim_start_matches("http://").to_owned();
    let link = squircle(
        row![icon(Icon::Link, 14.0).color(l.ink_3), truncate(shown, type_::S13, l.ink_2)]
            .spacing(8)
            .align_y(Alignment::Center),
        10.0,
        look,
    )
    .fill(l.raised)
    .edge()
    .padding([8, 10])
    .width(Fill);
    let copied = room.copied.is_some();
    let copy = button(if copied { "Copied" } else { "Copy" }, look)
        .primary()
        .icon(if copied { Icon::Check } else { Icon::Copy })
        .on_press(to(Message::Copy))
        .boxed();

    let mut body = Column::new()
        .push(container(title.align_y(Alignment::Center)).padding([0, 4]).boxed())
        .push(
            container(row![link, copy].spacing(8).align_y(Alignment::Center))
                .padding(Padding { top: 12.0, ..Padding::ZERO })
                .boxed(),
        )
        .push(heading("HERE NOW"));
    for m in &state.members {
        let mut line = row![face(room, m, pictures, 28.0, look)].spacing(10).align_y(Alignment::Center);
        let mut name = row![truncate(m.name.clone(), type_::SM, l.ink)].spacing(6);
        if Some(m.id) == room.snap.you {
            name = name.push(text("you", type_::SM, l.ink_3).boxed());
        }
        line = line.push(container(name).width(Fill).boxed());
        if m.user_id == Some(state.host.id) {
            line = line.push(text("host", type_::XS, l.ink_3).boxed());
        }
        if let Some(words) = status_text(m.status) {
            line =
                line.push(text(words, type_::XS, if m.status == Status::Blocked { l.ink_3 } else { l.warn }).boxed());
        }
        body = body.push(container(line).padding([6, 4]).boxed());
    }
    if room.me().is_some_and(|me| me.user_id.is_none()) {
        body = body.push(
            container(
                row![
                    text("Your name", type_::XS, l.ink_3),
                    input("room-guest-name", &room.name, "Your name", move |name| to(Message::Name(name)), look)
                        .on_submit(to(Message::SaveName))
                        .boxed()
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(Padding { top: 8.0, left: 4.0, right: 4.0, bottom: 0.0 })
            .boxed(),
        );
    }

    let me = room.me().and_then(|me| me.user_id);
    let people: Vec<_> = room.people.iter().filter(|p| Some(p.id) != me).collect();
    if room.can_invite && !people.is_empty() {
        body = body.push(heading("INVITE"));
        let shown = people.len();
        let mut list = Column::new();
        for p in people {
            let picture = p.avatar.as_ref().and_then(|path| pictures.get(path));
            let mut line = row![
                avatar(&p.username, picture, 28.0, None, look),
                truncate(p.username.clone(), type_::SM, l.ink).width(Fill)
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            line = if state.members.iter().any(|m| m.user_id == Some(p.id)) {
                line.push(text("Here", type_::XS, l.ink_3).boxed())
            } else {
                let invited = state.invited.contains(&p.id);
                let invite = button(if invited { "Invite again" } else { "Invite" }, look).small();
                line.push((if invited { invite.plain() } else { invite }).on_press(to(Message::Invite(p.id))).boxed())
            };
            list = list.push(container(line).padding([4, 4]).boxed());
        }
        // A long list scrolls rather than pushing the panel off the window.
        body = body.push(if shown > 5 { scrollable(list).height(5.0 * 36.0).boxed() } else { list.boxed() });
    }

    if room.is_host {
        body = body.push(heading("ROOM"));
        let setting = |label: &'a str, control: Element<'a, M>| {
            container(row![text(label, type_::SM, l.ink).width(Fill), control].spacing(12).align_y(Alignment::Center))
                .padding([10, 4])
                .boxed()
        };
        if room.can_share {
            body = body
                .push(setting("Anyone with the link", toggle(state.public, move |on| to(Message::Public(on)), look)));
            body = body.push(divider(look));
        }
        body = body.push(setting(
            "Who controls playback",
            segmented(
                state.settings.control,
                &[(Control::Everyone, "Everyone"), (Control::Host, "Only me")],
                move |control| to(Message::Control(control)),
                look,
            ),
        ));
        body = body.push(divider(look));
        body = body.push(setting(
            "Wait for everyone",
            toggle(state.settings.wait_for_all, move |on| to(Message::WaitForAll(on)), look),
        ));
        body = body.push(
            container(
                button(if room.ending { "Sure? This ends it for everyone" } else { "End for everyone" }, look)
                    .danger()
                    .fill()
                    .on_press(to(Message::End))
                    .boxed(),
            )
            .padding(Padding { top: 8.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    panel(body, 14.0, look).padding(12).width(352).boxed()
}

/// A quiet line near the top when someone else does something, or the room says no.
pub fn notice_view<'a, M: 'a>(room: &'a Room, look: &Look) -> Option<Element<'a, M>> {
    let (words, _) = room.notice.as_ref()?;
    let pill = squircle(text(words.clone(), type_::S13, look.media_ink), 18.0, look)
        .round()
        .fill(look.media_shade.scale_alpha(0.6))
        .padding([8, 16]);
    Some(column![space().height(80), container(pill).center_x(Fill)].width(Fill).height(Fill).boxed())
}

/// The words over the timeline when someone else holds the controls.
pub fn held_by(room: &Room) -> Option<String> {
    room.controlled_by().map(|host| format!("{host} controls playback"))
}
