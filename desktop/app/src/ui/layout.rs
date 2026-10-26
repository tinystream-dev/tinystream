// SPDX-License-Identifier: AGPL-3.0-or-later

//! How pages are laid out: `Page.tsx`, `PosterGrid.tsx` and `Row.tsx`.

use iced_core::{Alignment, Color, Length, Padding, Theme, Widget as CoreWidget, gradient, widget};
use iced_widget::scrollable::{Direction, Scrollbar};
use iced_widget::{
    Column, Element, Renderer, Row, Widget, column, container, responsive, row, scrollable, space, stack,
};

use super::controls::icon_button;
use super::icon::Icon;
use super::text::{self as type_, MEDIUM, SEMIBOLD, text};
use super::theme::Look;

/// `<Page>`: the page's column, centred, at most 1400 wide (1600 for `wide` ones).
pub fn page<'a, Message: 'a>(
    content: impl CoreWidget<Message, Theme, Renderer> + 'a,
    wide: bool,
) -> Element<'a, Message> {
    container(container(content).width(Length::Fill.max(if wide { 1600 } else { 1400 })).padding([40, 40]))
        .center_x(Length::Fill)
        .boxed()
}

/// `<PageTitle>`: the page's name, with what goes beside it.
pub fn page_title<'a, Message: 'a>(
    title: &'a str,
    aside: Option<Element<'a, Message>>,
    look: &Look,
) -> Element<'a, Message> {
    let mut heading = Row::new()
        .push(type_::truncate(title, type_::TITLE, look.ink).font(SEMIBOLD).width(Length::Fill).boxed())
        .align_y(Alignment::End)
        .spacing(16);
    if let Some(aside) = aside {
        heading = heading.push(aside);
    }
    container(heading).padding(Padding { bottom: 32.0, ..Padding::ZERO }).boxed()
}

/// `<Section>`: a titled part of a page.
pub fn section<'a, Message: 'a>(
    title: &'a str,
    aside: Option<Element<'a, Message>>,
    content: impl CoreWidget<Message, Theme, Renderer> + 'a,
    look: &Look,
) -> Element<'a, Message> {
    let mut heading =
        Row::new().push(text(title, type_::S15, look.ink).font(SEMIBOLD).width(Length::Fill).boxed()).spacing(12);
    if let Some(aside) = aside {
        heading = heading.push(aside);
    }
    column![container(heading).padding(Padding { bottom: 14.0, ..Padding::ZERO }), content]
        .padding(Padding { bottom: 44.0, ..Padding::ZERO })
        .width(Length::Fill)
        .boxed()
}

/// `<Empty>`: nothing to show, said kindly.
pub fn empty<'a, Message: 'a>(title: &'a str, body: Option<&'a str>, look: &Look) -> Element<'a, Message> {
    let mut content =
        Column::new().push(text(title, type_::S15, look.ink).font(MEDIUM).boxed()).align_x(Alignment::Center);
    if let Some(body) = body {
        content = content.push(
            container(
                text(body, type_::SM, look.ink_2).line_height(iced_core::text::LineHeight::Relative(1.625)).center(),
            )
            .width(Length::Fill.max(384))
            .padding(Padding { top: 8.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    container(content).center_x(Length::Fill).padding([96, 0]).boxed()
}

/// `<PosterGrid>`: as many 2:3 cards across as fit at 152 or more, 20 apart, rows 28 apart.
pub fn poster_grid<'a, Message: 'a>(
    count: usize,
    card: impl Fn(usize) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
    responsive(move |size| {
        let columns = (((size.width + 20.0) / (152.0 + 20.0)).floor() as usize).max(1);
        let mut rows = Column::new().spacing(28).width(Length::Fill);
        let mut cells = (0..count).map(&card).peekable();
        while cells.peek().is_some() {
            let mut line = Row::new().spacing(20).width(Length::Fill);
            for _ in 0..columns {
                line = line.push(match cells.next() {
                    Some(cell) => container(cell).width(Length::Fill).boxed(),
                    None => space().width(Length::Fill).boxed(),
                });
            }
            rows = rows.push(line.boxed());
        }
        rows.boxed()
    })
    .height(Length::Shrink)
    .boxed()
}

/// `<Row>`: a shelf scrolling sideways, 20 apart, its edges fading into the page, with arrows
/// for paging through it once the pointer's over it.
pub fn shelf<'a, Message: Clone + 'a>(
    id: &'static str,
    items: Vec<Element<'a, Message>>,
    viewport: Option<iced_widget::scrollable::Viewport>,
    scrolled: impl Fn(iced_widget::scrollable::Scroll) -> Message + 'a,
    page: impl Fn(f32) -> Message + 'a,
    look: &Look,
) -> Element<'a, Message> {
    let strip = scrollable(container(Row::with_children(items).spacing(20)).padding(Padding {
        top: 4.0,
        bottom: 12.0,
        left: 40.0,
        right: 40.0,
    }))
    .id(widget::Id::new(id))
    .direction(Direction::Horizontal(Scrollbar::hidden()))
    .on_scroll(scrolled)
    .width(Length::Fill);
    let start = viewport.is_none_or(|v| v.absolute_offset().x < 8.0);
    let end = viewport.is_some_and(|v| v.distance_to_end().x < 8.0);
    let fade = |towards_end: bool| {
        let canvas = look.canvas;
        container(space().width(48).height(Length::Fill)).style(move |_| {
            let hidden = if towards_end { end } else { start };
            let canvas = if hidden { Color::TRANSPARENT } else { canvas };
            let (from, to) = if towards_end { (Color::TRANSPARENT, canvas) } else { (canvas, Color::TRANSPARENT) };
            iced_widget::container::Style {
                background: Some(
                    iced_core::Gradient::Linear(
                        gradient::Linear::new(iced_core::Radians(std::f32::consts::FRAC_PI_2))
                            .add_stop(0.0, from)
                            .add_stop(1.0, to),
                    )
                    .into(),
                ),
                ..iced_widget::container::Style::default()
            }
        })
    };
    let arrows = row![
        container(if start {
            space().width(32).height(32).boxed()
        } else {
            icon_button(Icon::ChevronLeft, "Scroll left", Some(page(-1.0)), look)
        })
        .center_y(Length::Fill),
        space().width(Length::Fill),
        container(if end {
            space().width(32).height(32).boxed()
        } else {
            icon_button(Icon::ChevronRight, "Scroll right", Some(page(1.0)), look)
        })
        .center_y(Length::Fill),
    ];
    let edges = row![fade(false), space().width(Length::Fill), fade(true)];
    iced_widget::hover(stack![strip, edges], arrows).boxed()
}
