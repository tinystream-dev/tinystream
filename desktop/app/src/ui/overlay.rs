// SPDX-License-Identifier: AGPL-3.0-or-later

//! What floats over the page: dialogs (`Dialog`), toasts (`feedback.tsx`) and dropdowns (`Select`).

use iced_core::{Alignment, Length, Padding, Theme, Widget as CoreWidget};
use iced_widget::{Column, Element, Renderer, Row, Widget, container, mouse_area, opaque, row, space, stack};

use super::controls::{button, icon_button, panel};
use super::effects::{Entrance, enter};
use super::icon::{Icon, icon};
use super::squircle::squircle;
use super::text::{self as type_, MEDIUM, text, truncate};
use super::theme::Look;

/// A centred floating panel over the dimmed page; a click outside closes it (`Dialog`).
pub fn dialog<'a, Message: Clone + 'a>(
    content: impl CoreWidget<Message, Theme, Renderer> + 'a,
    width: f32,
    on_close: Message,
    look: &Look,
) -> Element<'a, Message> {
    let scrim = look.scrim();
    let backdrop = mouse_area(
        container(space().width(Length::Fill).height(Length::Fill))
            .style(move |_| container::Style { background: Some(scrim.into()), ..container::Style::default() }),
    )
    .on_press(on_close);
    let floating = enter(panel(content, 20.0, look).padding(20).width(Length::Fill), Entrance::pop(look), look);
    stack![
        backdrop,
        container(opaque(container(floating).width(Length::Fill.max(width))))
            .center_x(Length::Fill)
            .padding(Padding { top: 64.0, ..Padding::ZERO })
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// `ask()`'s dialog: a question, and whether to go ahead.
pub fn confirm<'a, Message: Clone + 'a>(
    title: &'a str,
    body: Option<&'a str>,
    action: &'a str,
    danger: bool,
    answer: impl Fn(bool) -> Message,
    look: &Look,
) -> Element<'a, Message> {
    let mut content = Column::new().push(text(title, type_::S15, look.ink).font(MEDIUM).boxed());
    if let Some(body) = body {
        content = content.push(
            container(text(body, type_::SM, look.ink_2).line_height(iced_core::text::LineHeight::Relative(1.625)))
                .padding(Padding { top: 6.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    let go = button(action, look).primary();
    let go = if danger { go.variant(super::controls::Variant::Danger) } else { go };
    content = content.push(
        container(
            row![button("Cancel", look).plain().on_press(answer(false)).boxed(), go.on_press(answer(true)).boxed()]
                .spacing(8),
        )
        .align_right(Length::Fill)
        .padding(Padding { top: 24.0, ..Padding::ZERO })
        .boxed(),
    );
    dialog(content, 384.0, answer(false), look)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Quiet,
    Ok,
    Danger,
}

/// A passing notice, bottom right.
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub title: String,
    pub body: Option<String>,
    pub tone: Tone,
}

/// The toasts on screen, newest at the bottom.
pub fn toasts<'a, Message: Clone + 'a>(
    toasts: &'a [Toast],
    dismiss: impl Fn(u64) -> Message,
    look: &Look,
) -> Element<'a, Message> {
    let cards = toasts.iter().map(|toast| {
        let mut content = Row::new().spacing(12).align_y(Alignment::Center);
        let tone = match toast.tone {
            Tone::Ok => Some((Icon::CircleCheck, look.ok)),
            Tone::Danger => Some((Icon::CircleAlert, look.danger)),
            Tone::Quiet => None,
        };
        if let Some((glyph, color)) = tone {
            content = content.push(icon(glyph, 18.0).color(color).boxed());
        }
        let mut words = Column::new().push(truncate(toast.title.as_str(), type_::SM, look.ink).font(MEDIUM).boxed());
        if let Some(body) = &toast.body {
            words = words.push(
                container(
                    text(body.as_str(), type_::XS, look.ink_2)
                        .line_height(iced_core::text::LineHeight::Relative(1.625)),
                )
                .padding(Padding { top: 2.0, ..Padding::ZERO })
                .boxed(),
            );
        }
        content = content.push(words.width(Length::Fill).boxed());
        let card = squircle(
            row![content, icon_button(Icon::X, "Dismiss", Some(dismiss(toast.id)), look)]
                .spacing(8)
                .align_y(Alignment::Center),
            16.0,
            look,
        )
        .fill(look.float)
        .material(look.material())
        .edge()
        .lift()
        .padding(Padding { top: 10.0, right: 8.0, bottom: 10.0, left: 12.0 })
        .width(Length::Fill);
        enter(container(card).width(Length::Fill.max(384)), Entrance::rise(), look)
    });
    container(Column::with_children(cards).spacing(8).align_x(Alignment::End).width(384))
        .align_right(Length::Fill)
        .align_bottom(Length::Fill)
        .padding(20)
        .boxed()
}

pub use super::select::select;

/// A sheet attached to the right edge, with the same material and dismiss behavior as a dialog.
pub fn sheet<'a, Message: Clone + 'a>(
    content: impl CoreWidget<Message, Theme, Renderer> + 'a,
    on_close: Message,
    look: &Look,
) -> Element<'a, Message> {
    let scrim = look.scrim();
    let backdrop = mouse_area(
        container(space().width(Length::Fill).height(Length::Fill))
            .style(move |_| container::Style { background: Some(scrim.into()), ..container::Style::default() }),
    )
    .on_press(on_close);
    let surface = panel(content, 20.0, look).padding(24).width(Length::Fill).height(Length::Fill);
    stack![
        backdrop,
        container(opaque(container(surface).width(Length::Fill.max(440)).height(Length::Fill)))
            .align_right(Length::Fill)
            .height(Length::Fill)
            .padding(12)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
