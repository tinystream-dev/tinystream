// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::Length::Fill;
use iced_core::{Alignment, Padding};
use iced_widget::{Column, Element, Row, Stack, Widget, column, container, image, row, scrollable};

use super::article::Article;
use super::{Books, Message, SCROLL, model};
use crate::route::Route;
use crate::ui::Look;
use crate::ui::controls::{button, icon_button, input};
use crate::ui::icon::{Icon, icon};
use crate::ui::text::{self as ty, text};

pub fn view<'a>(books: &'a Books, reading: bool, look: &Look) -> Element<'a, Message> {
    let Some(book) = &books.detail else {
        return container(
            column![
                text(books.error.as_deref().filter(|s| !s.is_empty()).unwrap_or("Loading book…"), ty::BASE, look.ink_2),
                button("Try again", look).on_press(Message::Retry).boxed()
            ]
            .spacing(12),
        )
        .center(Fill)
        .boxed();
    };
    if !reading {
        let cover = if let Some(handle) = books.images.get("cover") {
            image(handle.clone()).width(180).content_fit(iced_core::ContentFit::Contain).boxed()
        } else {
            container(icon(Icon::BookOpen, 48.0).color(look.ink_3)).width(180).height(270).center_y(270).boxed()
        };
        let finished = book.card.reading.as_ref().is_some_and(|r| r.finished);
        let progress = book.card.reading.as_ref().map_or(0, |r| (r.progress * 100.0).floor() as usize);
        let mut actions = row![
            button(if progress > 0 && !finished { "Continue reading" } else { "Read" }, look)
                .primary()
                .icon(Icon::BookOpen)
                .on_press(Message::Go(Route::Read(book.card.id as u64)))
                .boxed(),
            button(if finished { "Mark not started" } else { "Mark read" }, look)
                .on_press_maybe((!books.busy).then_some(Message::Finish(!finished)))
                .boxed()
        ]
        .spacing(8);
        if books.can_edit {
            actions = actions.push(
                button("Refresh details", look)
                    .plain()
                    .on_press_maybe((!books.busy).then_some(Message::Refresh))
                    .boxed(),
            );
        }
        let mut info = column![
            text(&book.card.title, ty::TITLE, look.ink).font(ty::SEMIBOLD),
            text(book.card.authors.join(" & "), ty::BASE, look.ink_2),
            actions
        ]
        .spacing(16);
        if let Some(series) = &book.card.series {
            info = info.push(
                text(
                    format!("{series}{}", book.card.series_index.map_or(String::new(), |n| format!(" · Book {n}"))),
                    ty::SM,
                    look.ink_2,
                )
                .boxed(),
            );
        }
        if let Some(description) = &book.description {
            info = info.push(
                text(description, ty::BASE, look.ink).line_height(iced_core::text::LineHeight::Relative(1.625)).boxed(),
            );
        }
        let facts = [
            book.publisher.clone(),
            book.published.clone().or_else(|| book.card.year.map(|n| n.to_string())),
            book.language.clone(),
            Some(book.format.clone()),
            Some(format!("{} chapters", book.chapter_count)),
            Some(format!("{:.1} MB", book.size as f64 / 1_000_000.0)),
            book.isbn.as_ref().map(|n| format!("ISBN {n}")),
            book.pages.map(|n| format!("{n:.0} pages")),
            book.rating.map(|n| format!("Rating {n}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        info = info
            .push(text(facts, ty::SM, look.ink_3).boxed())
            .push(text(book.genres.join(", "), ty::SM, look.ink_2).boxed());
        if let Some(url) = &book.provider_url {
            info = info.push(button("View source", look).plain().on_press(Message::Link(url.clone())).boxed());
        }
        if progress > 0 {
            info = info.push(text(format!("{progress}% read"), ty::SM, look.ink_2).boxed());
        }
        let body = column![
            button("Back to library", look)
                .plain()
                .on_press(Message::Go(Route::Library(book.card.library.clone())))
                .boxed(),
            row![cover, info.width(Fill)].spacing(32),
            quotes(books, look)
        ]
        .spacing(28)
        .padding(32)
        .width(Fill.max(1120));
        return scrollable(container(body).center_x(Fill)).height(Fill).boxed();
    }
    let pct = books.document.as_ref().map_or(0, |doc| model::percent(books.position, doc.length));
    let mut main = Column::new();
    if !books.focused {
        let chapter = books
            .document
            .as_ref()
            .and_then(|doc| doc.chapter_at(books.position))
            .map(|c| c.title.as_str())
            .unwrap_or("");
        let header = row![
            icon_button(Icon::ChevronLeft, "Back to book", Some(Message::Back), look),
            column![text(&book.card.title, ty::SM, look.ink).font(ty::MEDIUM), text(chapter, ty::XS, look.ink_3)]
                .width(Fill),
            text(format!("{pct}%"), ty::XS, look.ink_3),
            icon_button(Icon::Quote, "Quotes", Some(Message::Quotes), look),
            icon_button(Icon::BookOpen, "Contents", Some(Message::Contents), look),
            icon_button(Icon::Minus, "Smaller text (-)", Some(Message::TextSize(-1)), look),
            text(format!("{:.0}", books.size()), ty::SM, look.ink_2),
            icon_button(Icon::Plus, "Larger text (+)", Some(Message::TextSize(1)), look),
            icon_button(Icon::Maximize, "Focus (F)", Some(Message::Focus), look)
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .padding(Padding { top: 8.0, right: 118.0, bottom: 8.0, left: 12.0 });
        main = main.push(header.boxed());
    }
    if let Some(notice) = &books.notice {
        main = main.push(container(text(notice, ty::SM, look.danger)).padding([8, 24]).boxed());
    }
    if let Some(range) = books.selection {
        let count = range.end.saturating_sub(range.start);
        let mut selected = row![
            text(format!("{count} characters"), ty::XS, look.ink_2),
            button("Copy", look).small().on_press(Message::CopySelection).boxed()
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if books.can_quote {
            selected = selected.push(input("quote-note", &books.note, "Add a note", Message::Note, look).boxed()).push(
                button("Save quote", look)
                    .small()
                    .primary()
                    .on_press_maybe(
                        (!books.busy && count > 0 && count <= model::QUOTE_LENGTH).then_some(Message::SaveQuote),
                    )
                    .boxed(),
            );
        }
        main = main.push(
            container(
                selected.push(icon_button(Icon::X, "Clear selection", Some(Message::ClearSelection), look).boxed()),
            )
            .padding([8, 24])
            .boxed(),
        );
    }
    let article: Element<'a, Message> = if books.document.is_some() {
        Article { books, ink: look.ink, secondary: look.ink_2, highlight: look.highlight }.boxed()
    } else {
        let message = books.error.as_deref().filter(|s| !s.is_empty()).unwrap_or("Opening book…");
        container(
            column![text(message, ty::BASE, look.ink_2), button("Try again", look).on_press(Message::Retry).boxed()]
                .spacing(12),
        )
        .padding(48)
        .boxed()
    };
    let reader = scrollable(article)
        .id(SCROLL)
        .height(Fill)
        .width(Fill)
        .on_scroll(|s| Message::Scrolled(s.viewport.absolute_offset().y, s.viewport.bounds.height));
    let mut body = Row::new().push(reader.boxed()).height(Fill);
    if books.contents {
        let mut list = column![text("Contents", ty::LG, look.ink).font(ty::SEMIBOLD)].spacing(8).padding(20);
        if let Some(doc) = &books.document {
            for chapter in &doc.chapters {
                list = list.push(
                    container(
                        button(chapter.title.clone(), look)
                            .plain()
                            .fill()
                            .on_press(Message::Jump(
                                books
                                    .document
                                    .as_ref()
                                    .and_then(|doc| doc.blocks.get(chapter.block))
                                    .map_or(chapter.at, |b| b.at),
                            ))
                            .boxed(),
                    )
                    .padding(Padding { left: chapter.depth.min(8) as f32 * 12.0, ..Padding::ZERO })
                    .boxed(),
                );
            }
        }
        body = body.push(scrollable(list).width(300).height(Fill).boxed());
    } else if books.show_quotes {
        body = body.push(scrollable(quotes(books, look)).width(320).height(Fill).boxed());
    }
    main = main.push(body.boxed()).height(Fill);
    let main: Element<'a, Message> = main.boxed();
    if books.focused && !books.idle() {
        Stack::with_children([
            main,
            container(icon_button(Icon::Minimize, "Leave focus (F / Esc)", Some(Message::Focus), look))
                .align_right(Fill)
                .padding(Padding { top: 56.0, right: 16.0, bottom: 16.0, left: 16.0 })
                .boxed(),
        ])
        .height(Fill)
        .width(Fill)
        .boxed()
    } else {
        main
    }
}

fn quotes<'a>(books: &'a Books, look: &Look) -> Element<'a, Message> {
    let mut list = column![text("Quotes", ty::LG, look.ink).font(ty::SEMIBOLD)].spacing(16).padding(20);
    if books.quotes().is_empty() {
        list = list.push(text("Select text in the reader to save a quote.", ty::SM, look.ink_3).boxed());
    }
    for quote in books.quotes() {
        let Some(q) = &quote.quote else { continue };
        let mut card =
            column![text(&q.text, ty::SM, look.ink).line_height(iced_core::text::LineHeight::Relative(1.6))].spacing(8);
        if !quote.name.is_empty() {
            card = card.push(text(&quote.name, ty::SM, look.ink_2).boxed());
        }
        card = card.push(
            text(if quote.mine { "You".to_string() } else { quote.owner.username.clone() }, ty::XS, look.ink_3).boxed(),
        );
        let mut actions = row![
            button("Copy", look).small().plain().on_press(Message::CopyQuote(quote.id)).boxed(),
            button("Manage", look).small().plain().on_press(Message::ManageQuote(quote.id)).boxed()
        ]
        .spacing(4);
        if let Some(range) = q.range {
            actions = actions.push(button("Go to", look).small().on_press(Message::Jump(range.start)).boxed());
        }
        if quote.can_manage {
            actions = actions.push(
                icon_button(
                    Icon::Trash2,
                    "Delete quote",
                    (!books.busy).then_some(Message::DeleteQuote(quote.id)),
                    look,
                )
                .boxed(),
            );
        }
        if let Some(link) = &quote.link {
            actions =
                actions.push(button("Open link", look).small().plain().on_press(Message::Link(link.clone())).boxed());
        }
        card = card.push(actions.boxed());
        list = list.push(
            container(card)
                .padding(12)
                .width(Fill)
                .style({
                    let line = look.line;
                    move |_| container::Style {
                        border: iced_core::Border { width: 1.0, color: line, radius: 10.0.into() },
                        ..Default::default()
                    }
                })
                .boxed(),
        );
    }
    list.width(Fill).boxed()
}
