// SPDX-License-Identifier: AGPL-3.0-or-later

//! A watch-together room (#88), as the web's `/together/$code` route: what the room is, joining
//! it (signed in or as a guest), and the player in it, started again for each episode.

use std::sync::Arc;

use iced_core::Alignment;
use iced_core::Length::Fill;
use iced_widget::{Column, Element, Widget, column, container};
use url::Url;

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};
use crate::player::room::{self, Room};
use crate::player::sync::{Snapshot, SyncClient};
use crate::route::Route;
use crate::ui::Look;
use crate::ui::controls::{button, spinner};
use crate::ui::text::{self as type_, SEMIBOLD, text};

#[derive(Clone)]
pub enum Message {
    Info(Result<Arc<api::room::ResponseData>, api::Error>),
    Changed,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TogetherMessage")
    }
}

pub struct Together {
    pub serial: u64,
    pub api: Api,
    pub code: String,
    pub info: Option<api::room::RoomRoom>,
    /// Why there's nothing to join.
    pub gone: Option<String>,
    pub client: SyncClient,
    pub snap: Snapshot,
    /// Whether the panel has opened by itself already (once is enough).
    opened: bool,
}

impl Together {
    pub fn new(serial: u64, api: Api, code: String, effects: &mut Vec<Effect>) -> Self {
        let client = SyncClient::new(code.clone(), room::guest_name());
        let (request, variables) = (api.clone(), api::room::Variables { code: code.clone() });
        effects.push(Effect::Request(Box::pin(async move {
            AppMessage::Together(serial, Message::Info(request.request::<api::Room>(variables).await.map(Arc::new)))
        })));
        Self { serial, api, code, info: None, gone: None, client, snap: Snapshot::default(), opened: false }
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) {
        match message {
            Message::Info(Ok(data)) => {
                self.info = Some(data.room.clone());
                let (client, api, serial) = (self.client.clone(), self.api.clone(), self.serial);
                effects.push(Effect::Listen(Box::new(move |send| {
                    let notify = Arc::new(move || send(AppMessage::Together(serial, Message::Changed)));
                    client.connect(api, &tokio::runtime::Handle::current(), notify);
                })));
            },
            Message::Info(Err(error)) => {
                self.gone = Some(if error.unauthorized || error.message.contains("sign in") {
                    "Sign in to join this room.".into()
                } else if error.message.contains("not found") {
                    "This room has ended, or the link isn’t right.".into()
                } else {
                    error.message
                });
            },
            Message::Changed => self.snap = self.client.snapshot(),
        }
    }

    pub fn is_host(&self) -> bool {
        self.info.as_ref().is_some_and(|info| info.is_host)
    }

    /// The room as the player in it sees it.
    pub fn room(&mut self) -> Option<Room> {
        let info = self.info.as_ref()?;
        let link = self.api.origin.join(&format!("together/{}", self.code)).ok()?.to_string();
        let mut room = Room::new(
            self.client.clone(),
            self.code.clone(),
            link,
            info.signed_in,
            info.is_host,
            info.can_share,
            info.can_invite,
        );
        room.panel = info.is_host && !self.opened;
        self.opened = true;
        Some(room)
    }
}

impl Drop for Together {
    fn drop(&mut self) {
        self.client.close();
    }
}

/// The server and room code in a link to a room: `https://<server>/together/<code>`, or
/// `tinystream://<host[:port]>/together/<code>`.
pub fn link(link: &str) -> Option<(Url, String)> {
    let url = Url::parse(link).ok()?;
    if !matches!(url.scheme(), "http" | "https" | "tinystream") || url.host_str().is_none() {
        return None;
    }
    let parts: Vec<&str> = url.path_segments()?.filter(|part| !part.is_empty()).collect();
    let ["together", code] = parts[..] else { return None };
    let mut server = url.clone();
    server.set_path("/");
    server.set_query(None);
    server.set_fragment(None);
    Some((server, code.to_owned()))
}

/// The room screen: arriving, the player in it, or why there's nothing to join.
pub fn view<'a>(app: &'a crate::app::App, look: &Look) -> Element<'a, AppMessage> {
    let signed_in = app.session.screen == crate::session::Screen::Ready;
    let Some(together) = &app.together else { return arriving(None, look) };
    if let Some(gone) = &together.gone {
        return self::gone(gone, signed_in, look);
    }
    let title = together.info.as_ref().map(|info| info.title.name.as_str());
    if together.snap.ended && !together.is_host() {
        return gone("Whoever started this room has ended it.", signed_in, look);
    }
    match &app.player {
        Some(player) if player.room.is_some() && together.snap.room.is_some() => {
            let serial = player.serial;
            crate::player::view(player, look).map(move |message| AppMessage::Player(serial, message)).boxed()
        },
        _ => arriving(title, look),
    }
}

fn stage<'a>(content: Element<'a, AppMessage>, look: &Look) -> Element<'a, AppMessage> {
    let shade = look.media_shade;
    container(content)
        .center(Fill)
        .style(move |_| container::Style { background: Some(shade.into()), ..container::Style::default() })
        .boxed()
}

fn arriving<'a>(title: Option<&str>, look: &Look) -> Element<'a, AppMessage> {
    let mut l = *look;
    l.ink = look.media_ink.scale_alpha(0.8);
    l.line = look.media_ink.scale_alpha(0.15);
    let mut content = Column::new().spacing(16).align_x(Alignment::Center).push(spinner(36.0, &l));
    if let Some(title) = title {
        content = content.push(text(format!("Joining {title}…"), type_::S15, look.media_ink.scale_alpha(0.7)).boxed());
    }
    stage(content.boxed(), look)
}

fn gone<'a>(message: &str, signed_in: bool, look: &Look) -> Element<'a, AppMessage> {
    let mut content = column![
        text("Nothing playing here", type_::XL2, look.media_ink).font(SEMIBOLD),
        text(message.to_owned(), type_::S15, look.media_ink.scale_alpha(0.6)),
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    if signed_in {
        content = content.push(
            container(button("Go home", look).primary().large().on_press(AppMessage::Navigate(Route::Home)).boxed())
                .padding(iced_core::Padding { top: 16.0, ..iced_core::Padding::ZERO })
                .boxed(),
        );
    }
    stage(content.boxed(), look)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_room_links() {
        let (server, code) = link("https://media.example.ts.net/together/ab12").unwrap();
        assert_eq!((server.as_str(), code.as_str()), ("https://media.example.ts.net/", "ab12"));
        let (server, code) = link("tinystream://192.168.1.4:8080/together/xyz").unwrap();
        assert_eq!((server.as_str(), code.as_str()), ("tinystream://192.168.1.4:8080/", "xyz"));
        assert!(link("https://media.example.ts.net/watch/12").is_none());
        assert!(link("/together/ab12").is_none());
    }
}
