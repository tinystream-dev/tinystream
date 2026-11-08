// SPDX-License-Identifier: AGPL-3.0-or-later

//! Notices for the type and vectors embedded in the app.

use iced_core::{Alignment, Length};
use iced_widget::{Element, Widget as _, column, row, scrollable};

use super::controls::{icon_button, panel};
use super::icon::Icon;
use super::text::{self as type_, SEMIBOLD, text};
use super::theme::Look;

pub fn credits<'a, Message: Clone + 'a>(close: Message, look: &Look) -> Element<'a, Message> {
    static LICENSES: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let bundled = LICENSES.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("THIRD_PARTY_LICENSES")))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_else(|| include_str!("../../../THIRD_PARTY_LICENSES").to_owned())
    });
    let notices = text(bundled.as_str(), type_::XS, look.ink_2);
    panel(
        column![
            row![
                text("Licenses", type_::LG, look.ink).font(SEMIBOLD).width(Length::Fill),
                icon_button(Icon::X, "Close", Some(close), look)
            ]
            .align_y(Alignment::Center),
            scrollable(notices).height(Length::Fill),
        ]
        .spacing(16),
        20.0,
        look,
    )
    .width(Length::Fill.max(640))
    .height(Length::Fill.max(560))
    .padding(20)
    .boxed()
}
