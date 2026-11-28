// SPDX-License-Identifier: AGPL-3.0-or-later

//! The Downloaded page, and the download buttons and settings around the app.

use iced_core::Length::Fill;
use iced_core::{Alignment, Padding};
use iced_widget::{Element, Widget, column, container, row};

use super::Message;
use crate::app::{App, Message as AppMessage};
use crate::ui::Look;
use crate::ui::controls::button;
use crate::ui::text::{self as ty, text};

type View<'a> = Element<'a, AppMessage>;

fn send(message: Message) -> AppMessage {
    AppMessage::Offline(message)
}

/// "1.4 GB", as the web writes sizes.
pub fn size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", units[unit]) }
}

/// What's in Settings → Desktop's Offline downloads card: the switch, and what each saved
/// server's downloads take up.
pub fn settings<'a>(app: &'a App, look: &Look) -> View<'a> {
    let d = &app.downloads;
    let words = column![
        text("Keep downloads", ty::SM, look.ink),
        container(text(
            "Download episodes, films, albums and playlists to play without the server, and fetch the next few songs as you listen. Your server has to allow it too.",
            ty::XS,
            look.ink_3
        ))
        .padding(Padding { top: 2.0, ..Padding::ZERO })
    ]
    .width(Fill);
    let switch = crate::ui::controls::toggle(
        app.session.config.offline_downloads,
        |on| AppMessage::Session(crate::session::Message::OfflineDownloads(on)),
        look,
    );
    let mut content = column![row![words, switch].spacing(16).align_y(Alignment::Center)].spacing(16);
    for server in &app.session.config.servers {
        let Some(origin) = super::origin(&server.url) else { continue };
        let Some(bytes) = d.usage.get(&origin).copied().filter(|b| *b > 0) else { continue };
        content = content.push(
            row![
                column![text(server.name.as_str(), ty::SM, look.ink), text(size(bytes), ty::XS, look.ink_3)]
                    .spacing(2)
                    .width(Fill),
                button("Remove downloads", look).small().on_press(send(Message::Forget(origin))).boxed()
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    content.boxed()
}
