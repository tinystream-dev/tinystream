// SPDX-License-Identifier: AGPL-3.0-or-later

//! The security key prompt, where a browser would show its own.

use iced_core::{Alignment, Length, Padding};
use iced_widget::{Column, Element, Widget as _, column, container, row};

use super::{Ceremony, Stage};
use crate::ui::Look;
use crate::ui::controls::{button, input, spinner};
use crate::ui::icon::{Icon, icon};
use crate::ui::overlay::dialog;
use crate::ui::text::{self as type_, MEDIUM, text};

/// The PIN field, focused when the key asks for its PIN.
pub const PIN: &str = "passkey-pin";

pub fn prompt<'a, Message: Clone + 'a>(
    ceremony: &'a Ceremony,
    on_pin: impl Fn(String) -> Message + 'a,
    submit: Message,
    close: Message,
    look: &Look,
) -> Element<'a, Message> {
    let (title, body) = match &ceremony.stage {
        Stage::Waiting => ("Use your security key", "Waiting for your security key…".to_owned()),
        Stage::Insert => ("Insert your security key", "Plug it into a USB port to continue.".to_owned()),
        Stage::Touch => ("Touch your security key", "Touch its button or sensor to continue.".to_owned()),
        Stage::Pin { retries, wrong } => (
            "Enter your security key’s PIN",
            match (wrong, retries) {
                (true, Some(1)) => "That PIN isn’t right. One try left before the key locks.".to_owned(),
                (true, Some(n)) => format!("That PIN isn’t right. {n} tries left."),
                (true, None) => "That PIN isn’t right. Try again.".to_owned(),
                (false, Some(n)) if *n <= 3 => format!("{n} tries left before the key locks."),
                (false, _) => "Your key needs its PIN to continue.".to_owned(),
            },
        ),
    };
    let warn = matches!(ceremony.stage, Stage::Pin { wrong: true, .. });
    let mut content = Column::new().push(
        row![
            icon(Icon::KeyRound, 18.0).color(look.ink_2),
            text(title, type_::S15, look.ink).font(MEDIUM).width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .boxed(),
    );
    let note = text(body, type_::SM, if warn { look.danger } else { look.ink_2 })
        .line_height(iced_core::text::LineHeight::Relative(1.625));
    content = content.push(if matches!(ceremony.stage, Stage::Pin { .. }) {
        container(note).padding(Padding { top: 6.0, ..Padding::ZERO }).boxed()
    } else {
        container(row![spinner(16.0, look), note].spacing(10).align_y(Alignment::Center))
            .padding(Padding { top: 12.0, ..Padding::ZERO })
            .boxed()
    });
    let mut actions = row![button("Cancel", look).plain().on_press(close.clone()).boxed()].spacing(8);
    if matches!(ceremony.stage, Stage::Pin { .. }) {
        content = content.push(
            container(input(PIN, &ceremony.pin, "PIN", on_pin, look).secure().on_submit(submit.clone()).boxed())
                .padding(Padding { top: 16.0, ..Padding::ZERO })
                .boxed(),
        );
        actions = actions.push(
            button("Continue", look).primary().on_press_maybe((!ceremony.pin.is_empty()).then_some(submit)).boxed(),
        );
    }
    content = content
        .push(container(actions).align_right(Length::Fill).padding(Padding { top: 24.0, ..Padding::ZERO }).boxed());
    dialog(column![content], 384.0, close, look)
}
