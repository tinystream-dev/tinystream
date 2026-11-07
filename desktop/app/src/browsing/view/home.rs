// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub(super) fn home<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let Some(h) = b.data(&Key::Home).and_then(|d| d.home.as_ref()) else { return pending(b, &Key::Home, look) };
    let mut page = Column::<View<'a>>::new().width(Fill);
    let featured = h.continue_watching.iter().find(|e| e.new_episode).or_else(|| h.continue_watching.first());
    if let Some(e) = featured {
        let v = &e.video;
        let detail = [v.label.clone(), v.name.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let resuming = !e.up_next;
        let left = remaining(e.position, v.duration);
        let mut metric = column![if resuming && !left.is_empty() {
            crate::ui::numbers::styled_ticker(&left, BIG, SEMIBOLD, None, look)
        } else {
            text(v.label.clone().unwrap_or("Play".into()), BIG, look.ink).font(SEMIBOLD).boxed()
        }];
        if let Some(value) = v.duration.filter(|d| resuming && *d > 0.0 && e.position > 0.0) {
            metric = metric.push(
                container(container(progress(Some((e.position / value) as f32), look.media_ink, look)).width(224))
                    .padding(Padding { top: 12.0, ..Padding::ZERO })
                    .boxed(),
            );
        }
        let route = Route::Watch(v.id as u64, None);
        let play = button(if resuming { "Resume" } else { "Play" }, look)
            .primary()
            .large()
            .icon(Icon::Play)
            .on_press(AppMessage::Navigate(route.clone()));
        page = page.push(
            (container(hero(
                b,
                Hero {
                    name: &v.title.name,
                    label: if e.new_episode {
                        "New episode"
                    } else if resuming {
                        "Continue watching"
                    } else {
                        "Up next"
                    },
                    mark: if e.new_episode {
                        crate::ui::icon::icon(Icon::Radio, 14.0).color(look.warn)
                    } else {
                        crate::ui::icon::icon(Icon::RotateCcw, 14.0).color(look.ink_2)
                    },
                    detail,
                    image: v.title.backdrop.as_deref().or(v.still.as_deref()),
                    tint: v.title.backdrop_tint.as_deref().or(v.title.poster_tint.as_deref()),
                    route: Some(route),
                    height: 304.0,
                    name_size: 38.0,
                    padding: 36.0,
                    body: row![metric, play.boxed()].spacing(24).align_y(Alignment::End).wrap().boxed(),
                    aside: None,
                    bar: None,
                },
                look,
            ))
            .padding(Padding { bottom: 44.0, ..Padding::ZERO }))
            .boxed(),
        );
    } else {
        page = page.push((heading("Home", look)).boxed());
    }
    let rest: Vec<_> = h
        .continue_watching
        .iter()
        .filter(|e| featured.is_none_or(|f| f.video.id != e.video.id))
        .map(|e| {
            let v = &e.video;
            wide_card(
                b,
                WideCard {
                    name: &v.title.name,
                    subtitle: format!(
                        "{} · {}",
                        v.label.as_deref().unwrap_or(""),
                        if e.up_next { "Up next".into() } else { remaining(e.position, v.duration) }
                    ),
                    image: v.still.as_deref().or(v.title.backdrop.as_deref()),
                    route: Some(Route::Watch(v.id as u64, None)),
                    label: if e.new_episode { Some("New episode".into()) } else { None },
                    value: v
                        .duration
                        .filter(|d| *d > 0.0 && !e.up_next && e.position > 0.0)
                        .map(|d| (e.position / d) as f32),
                    width: 320.0,
                },
                look,
            )
        })
        .collect();
    if !rest.is_empty() {
        page = page.push((section("Continue watching", shelf(b, "home/continue", rest, look), look)).boxed());
    }
    if let Some(d) = b.data(&Key::Coming) {
        let entries: Vec<_> = d
            .calendar
            .iter()
            .filter(|e| e.state != "DONE")
            .take(12)
            .map(|e| {
                wide_card(
                    b,
                    WideCard {
                        name: &e.show,
                        subtitle: format!("{} · {}", code(e.season, e.episode), e.name.as_deref().unwrap_or("")),
                        image: e.backdrop.as_deref().or(e.poster.as_deref()),
                        route: e.title.as_ref().map(|t| Route::Title(t.id as u64)),
                        label: Some(
                            e.download
                                .as_ref()
                                .map(|d| format!("Downloading {}%", (d.progress * 100.0) as i64))
                                .unwrap_or_else(|| {
                                    if e.air_at <= now() {
                                        if e.state == "WANTED" { "Searching…" } else { "Out now" }.into()
                                    } else if e.air_at - now() < 86400 {
                                        countdown(e.air_at)
                                    } else {
                                        airs(e.air_at)
                                    }
                                }),
                        ),
                        value: e.download.as_ref().map(|d| d.progress as f32),
                        width: 288.0,
                    },
                    look,
                )
            })
            .collect();
        if !entries.is_empty() {
            page = page.push((section("Coming up", shelf(b, "home/coming", entries, look), look)).boxed());
        }
    }
    if !h.popular_here.is_empty() {
        page = page.push(
            (section(
                "Popular with people here",
                shelf(
                    b,
                    "home/popular",
                    h.popular_here
                        .iter()
                        .map(|p| {
                            container(poster(
                                b,
                                &p.title,
                                Some(if p.people > 1 {
                                    format!("{} people watching", p.people)
                                } else {
                                    "Someone’s watching".into()
                                }),
                                look,
                            ))
                            .width(152)
                            .boxed()
                        })
                        .collect(),
                    look,
                ),
                look,
            ))
            .boxed(),
        );
    }
    for s in &h.recently_added {
        if !s.titles.is_empty() {
            let header = row![
                text(format!("New in {}", s.library), ty::S15, look.ink).font(SEMIBOLD).width(Fill),
                button("See all", look)
                    .plain()
                    .small()
                    .on_press(AppMessage::Navigate(Route::Library(s.library.clone())))
                    .boxed()
            ]
            .align_y(Alignment::Center);
            page = page.push(
                (column![
                    header,
                    shelf(
                        b,
                        format!("home/new/{}", s.library),
                        s.titles.iter().map(|c| container(poster(b, c, None, look)).width(152).boxed()).collect(),
                        look
                    )
                ]
                .spacing(14)
                .padding(Padding { bottom: 44.0, ..Padding::ZERO }))
                .boxed(),
            );
        }
    }
    if let Some(d) = b.data(&Key::BookHome) {
        if !d.reading_now.is_empty() {
            page = page.push(section(
                "Continue reading",
                shelf(
                    b,
                    "home/reading",
                    d.reading_now
                        .iter()
                        .map(|book| container(book_tile(b, book, None, None, None, look)).width(136).boxed())
                        .collect(),
                    look,
                ),
                look,
            ));
        }
        for s in &d.new_books {
            if !s.books.is_empty() {
                page = page.push(section(
                    format!("New in {}", s.library),
                    shelf(
                        b,
                        format!("home/books/{}", s.library),
                        s.books
                            .iter()
                            .map(|book| container(book_tile(b, book, None, None, None, look)).width(136).boxed())
                            .collect(),
                        look,
                    ),
                    look,
                ));
            }
        }
    }
    if let Some(h) = b.data(&Key::Music).and_then(|d| d.music_home.as_ref()) {
        for (name, albums) in [("Jump back in", &h.recently_played), ("New music", &h.recently_added)] {
            if !albums.is_empty() {
                page = page.push(
                    (section(
                        name,
                        shelf(
                            b,
                            format!("home/music/{name}"),
                            albums
                                .iter()
                                .map(|a| {
                                    container(
                                        column![
                                            card_button(look)
                                                .content(picture(b, a.cover.as_deref(), 136.0, 136.0, 12.0, look))
                                                .on_press(AppMessage::Navigate(Route::Album(a.id as u64)))
                                                .boxed(),
                                            truncate(&a.name, ty::S13, look.ink),
                                            truncate(&a.artist, ty::XS, look.ink_3)
                                        ]
                                        .spacing(6),
                                    )
                                    .width(136)
                                    .boxed()
                                })
                                .collect(),
                            look,
                        ),
                        look,
                    ))
                    .boxed(),
                );
            }
        }
    }
    if h.recently_added.iter().all(|r| r.titles.is_empty())
        && featured.is_none()
        && !b
            .libraries()
            .iter()
            .any(|l| (l.kind == "MUSIC" && l.album_count > 0) || (l.kind == "BOOKS" && l.book_count > 0))
    {
        page = page.push(
            (layout::empty(
                if b.libraries().is_empty() { "No libraries yet" } else { "Your libraries are empty so far" },
                if b.viewer().is_some_and(|v| v.is_admin) { None } else { Some("Ask an admin for access.") },
                look,
            ))
            .boxed(),
        );
        if b.viewer().is_some_and(|v| v.is_admin) {
            page = page.push(
                (button("Add a library", look).primary().on_press(AppMessage::Navigate(Route::Settings)).boxed())
                    .boxed(),
            );
        }
    }
    page.boxed()
}
