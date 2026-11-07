// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub(super) fn library<'a>(b: &'a Browsing, route: &Route, name: &'a str, look: &Look) -> View<'a> {
    if b.libraries().iter().any(|l| l.name == name && l.kind == "BOOKS") {
        return books(b, name, None, look);
    }
    let key = Key::Library(name.into());
    let Some(d) = b.data(&key) else { return pending(b, &key, look) };
    let cards = d.library.as_ref().map_or(&[][..], |l| l.titles.as_slice());
    let filter = b.filter.get(route).map_or("all", String::as_str);
    let mut list: Vec<_> = cards
        .iter()
        .filter(|c| {
            let done = c.video_count > 0 && c.watched_count >= c.video_count;
            let started = c.watched_count > 0 || c.progress.is_some();
            match filter {
                "watching" => started && !done,
                "unwatched" => !started,
                "watched" => done,
                _ => true,
            }
        })
        .collect();
    let newest = b.newest.contains(route);
    if newest {
        list.sort_by_key(|c| std::cmp::Reverse(c.year.unwrap_or(0)));
    } else {
        list.sort_by_key(|c| c.name.to_lowercase());
    }
    let count = list.len();
    let filters = Row::with_children(
        [("all", "All"), ("watching", "Watching"), ("unwatched", "Not started"), ("watched", "Watched")]
            .into_iter()
            .map(|(v, l)| chip(l, filter == v, msg(b, Message::Filter(v.into())), look))
            .chain([
                space().width(Fill).boxed(),
                chip("A–Z", !newest, msg(b, Message::Sort(false)), look),
                chip("Newest", newest, msg(b, Message::Sort(true)), look),
            ]),
    )
    .spacing(4)
    .align_y(Alignment::Center);
    column![
        page_title(
            name,
            Some(
                container(text(
                    format!("{} title{}", cards.len(), if cards.len() == 1 { "" } else { "s" }),
                    ty::SM,
                    look.ink_3
                ))
                .padding(Padding { bottom: 4.0, ..Padding::ZERO })
                .boxed()
            ),
            look
        ),
        container(filters).padding(Padding { bottom: 28.0, ..Padding::ZERO }),
        if count == 0 {
            layout::empty(
                if cards.is_empty() { "This library is empty" } else { "Nothing matches this filter" },
                None,
                look,
            )
        } else {
            let l = *look;
            layout::virtual_poster_grid(count, ty::S13.line + ty::XS.line + 8.0, move |i| poster(b, list[i], None, &l))
        }
    ]
    .boxed()
}

fn current_book<'a>(books: &[&'a Book]) -> Option<&'a Book> {
    books
        .iter()
        .filter(|b| b.reading.as_ref().is_some_and(|r| !r.finished && r.position > 0.0))
        .max_by_key(|b| b.reading.as_ref().map_or(0, |r| r.updated_at))
        .copied()
        .or_else(|| {
            let last = books.iter().rposition(|b| b.reading.as_ref().is_some_and(|r| r.finished));
            books
                .iter()
                .skip(last.map_or(0, |i| i + 1))
                .find(|b| !b.reading.as_ref().is_some_and(|r| r.finished))
                .copied()
        })
        .or_else(|| books.iter().find(|b| !b.reading.as_ref().is_some_and(|r| r.finished)).copied())
        .or_else(|| books.first().copied())
}
pub(super) fn book_tile<'a>(
    b: &'a Browsing,
    book: &'a Book,
    name: Option<String>,
    caption: Option<String>,
    route: Option<Route>,
    look: &Look,
) -> View<'a> {
    let art = crate::ui::tilt::art(book.cover.as_deref().and_then(|p| b.images.get(p)).cloned(), 14.0, 1.5, look);
    let label = caption.unwrap_or_else(|| {
        if book.reading.as_ref().is_some_and(|r| r.finished) {
            "Read".into()
        } else if let Some(r) = book.reading.as_ref().filter(|r| r.progress > 0.0) {
            format!("{}% read", (r.progress * 100.0).round() as u32)
        } else {
            book.authors.join(", ")
        }
    });
    watch_image(
        b,
        book.cover.as_deref(),
        card_button(look)
            .content(
                column![
                    art,
                    truncate(name.unwrap_or_else(|| book.title.clone()), ty::S13, look.ink).font(MEDIUM),
                    truncate(label, ty::XS, look.ink_3)
                ]
                .spacing(6),
            )
            .on_press(AppMessage::Navigate(route.unwrap_or(Route::Book(book.id as u64))))
            .boxed(),
    )
}
pub(super) fn books<'a>(b: &'a Browsing, name: &str, series: Option<&str>, look: &Look) -> View<'a> {
    let key = Key::Books(name.into());
    let Some(d) = b.data(&key) else { return pending(b, &key, look) };
    let route = Route::Library(name.into());
    let filter = b.filter.get(&route).map_or("all", String::as_str);
    let sort = b.book_sort.get(&route).map_or("title", String::as_str);
    let mut books = d
        .books
        .iter()
        .filter(|book| {
            series.map_or_else(
                || match filter {
                    "reading" => book.reading.as_ref().is_some_and(|r| !r.finished && r.position > 0.0),
                    "read" => book.reading.as_ref().is_some_and(|r| r.finished),
                    "unread" => !book.reading.as_ref().is_some_and(|r| r.finished || r.position > 0.0),
                    _ => true,
                },
                |s| book.series.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(s)),
            )
        })
        .collect::<Vec<_>>();
    if series.is_some() {
        books.sort_by(|a, b| {
            a.series_index.unwrap_or(f64::INFINITY).total_cmp(&b.series_index.unwrap_or(f64::INFINITY))
        });
    } else if sort == "author" {
        books.sort_by_key(|b| b.authors.first().and_then(|a| a.split_whitespace().last()).unwrap_or("").to_lowercase());
    } else if sort == "added" {
        books.sort_by_key(|b| std::cmp::Reverse(b.added_at));
    }
    let mut page = column![heading(series.unwrap_or(name).to_owned(), look)].width(Fill).spacing(16);
    if series.is_none() {
        page = page.push(line(
            [("all", "All"), ("reading", "Reading"), ("unread", "Not started"), ("read", "Read")]
                .into_iter()
                .map(|(v, l)| chip(l, filter == v, msg(b, Message::Filter(v.into())), look))
                .chain(
                    [("title", "A–Z"), ("author", "Author"), ("added", "Newest")]
                        .into_iter()
                        .map(|(v, l)| chip(l, sort == v, msg(b, Message::BookSort(v.into())), look)),
                )
                .collect(),
        ));
    }
    if books.is_empty() {
        return page
            .push(layout::empty(
                if series.is_some() {
                    "This series isn't here anymore"
                } else if d.books.is_empty() {
                    "This library is empty"
                } else {
                    "Nothing matches this filter"
                },
                None,
                look,
            ))
            .boxed();
    }
    if series.is_some()
        && let Some(current) = current_book(&books)
    {
        let read = books.iter().filter(|b| b.reading.as_ref().is_some_and(|r| r.finished)).count();
        let started = current.reading.as_ref().is_some_and(|r| !r.finished && r.position > 0.0);
        let authors = books
            .iter()
            .flat_map(|b| b.authors.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        page = page.push(
            row![
                picture(b, current.cover.as_deref(), 176.0, 264.0, 14.0, look),
                column![
                    text(authors, ty::SM, look.ink_2),
                    text(format!("{} books · {read} read", books.len()), ty::SM, look.ink_3),
                    progress(Some(read as f32 / books.len() as f32), look.ink, look),
                    button(
                        format!(
                            "{}{}",
                            if started {
                                "Continue"
                            } else if read == books.len() {
                                "Read again"
                            } else {
                                "Read"
                            },
                            current.series_index.map(|n| format!(" volume {n}")).unwrap_or_default()
                        ),
                        look
                    )
                    .primary()
                    .large()
                    .icon(Icon::BookOpen)
                    .on_press(AppMessage::Navigate(Route::Read(current.id as u64)))
                    .boxed()
                ]
                .spacing(16)
                .width(Fill)
            ]
            .spacing(32)
            .boxed(),
        );
    }
    let mut groups: Vec<(Option<&str>, Vec<&Book>)> = Vec::new();
    if series.is_some() {
        for book in books {
            groups.push((None, vec![book]));
        }
    } else {
        for book in &books {
            let members = book
                .series
                .as_deref()
                .map(|s| {
                    books
                        .iter()
                        .filter(|b| b.series.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(s)))
                        .copied()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if members.len() < 2 {
                groups.push((None, vec![book]));
            } else if !groups
                .iter()
                .any(|(s, _)| s.is_some_and(|s| book.series.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(s))))
            {
                groups.push((book.series.as_deref(), members));
            }
        }
    }
    let l = *look;
    let library = name.to_owned();
    let volume = series.is_some();
    page.push(layout::virtual_poster_grid(groups.len(), ty::S13.line + ty::XS.line + 12.0, move |i| {
        let (series, members) = &groups[i];
        let book = current_book(members).expect("a nonempty shelf");
        let route = series.map(|series| Route::Series { library: library.clone(), series: series.into() });
        book_tile(
            b,
            book,
            series
                .map(str::to_owned)
                .or_else(|| if volume { book.series_index.map(|n| format!("Volume {n}")) } else { None }),
            series.map(|_| format!("{} books", members.len())),
            route,
            &l,
        )
    }))
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continues_the_most_recently_read_volume_then_moves_past_the_last_finished_one() {
        let mut books = (1..=3).map(|id| Book { id, ..Default::default() }).collect::<Vec<_>>();
        books[0].reading = Some(Reading { position: 100.0, updated_at: 10, ..Default::default() });
        books[1].reading = Some(Reading { position: 20.0, updated_at: 20, ..Default::default() });
        assert_eq!(current_book(&books.iter().collect::<Vec<_>>()).unwrap().id, 2);
        books[0].reading.as_mut().unwrap().finished = true;
        books[1].reading.as_mut().unwrap().finished = true;
        assert_eq!(current_book(&books.iter().collect::<Vec<_>>()).unwrap().id, 3);
    }
}
