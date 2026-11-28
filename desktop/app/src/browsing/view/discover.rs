// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::column;

use super::*;

pub(super) fn discover<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let can = b.can();
    let name = if can.manage_shows { "Add shows" } else { "Request shows" };
    if !can.manage_shows && !can.request {
        return column![heading(name, look), layout::empty("You don't have permission to request shows", None, look)]
            .boxed();
    }
    let key = Key::Discover(b.library.clone(), b.query.clone());
    let fetching = b.discover_input.trim() != b.query || b.cache.get(&key).is_some_and(|e| e.pending);
    let l = *look;
    let field = row![
        crate::ui::icon::icon(Icon::Search, 18.0).color(look.ink_3),
        iced_widget::text_input("Search AniList or TMDB", &b.discover_input)
            .id("discover-query")
            .on_input(move |q| msg(b, Message::Query(q)))
            .size(15)
            .padding([14, 0])
            .style(move |_, _| iced_widget::text_input::Style {
                background: iced_core::Background::Color(Color::TRANSPARENT),
                border: iced_core::Border::default(),
                placeholder: l.ink_3,
                value: l.ink,
                selection: l.selection,
            })
    ]
    .push(if fetching { spinner(16.0, look) } else { space().boxed() })
    .spacing(12)
    .align_y(Alignment::Center);
    let mut controls =
        row![squircle(field, 14.0, look).fill(look.raised).edge().padding([0, 16]).height(48).width(Fill)]
            .spacing(8)
            .align_y(Alignment::Center);
    let libraries = b.libraries();
    if libraries.len() > 1 {
        let chosen = if b.library.is_empty() {
            b.data(&key).and_then(|d| d.discover.as_ref()).and_then(|d| d.library.clone())
        } else {
            Some(b.library.clone())
        };
        let chosen = libraries.iter().find(|l| chosen.as_ref() == Some(&l.name)).cloned();
        controls = controls.push(
            container(crate::ui::select::select(
                chosen,
                libraries,
                |l| l.name.clone(),
                move |l| msg(b, Message::Library(l.name)),
                look,
            ))
            .width(192)
            .boxed(),
        );
    }
    controls = controls
        .push(container(media_select(&b.category, move |v| msg(b, Message::Category(v)), look)).width(192).boxed());
    let title = page_title(name, None, look);
    let mut page = column![controls];
    if b.query.is_empty()
        && let Some(f) = b.data(&Key::ForYou(b.library.clone())).and_then(|d| d.for_you.as_ref())
    {
        for s in &f.shelves {
            if s.results.iter().any(|r| b.category.is_empty() || r.category == b.category) {
                page = page.push(
                    (section(
                        &s.name,
                        shelf(
                            b,
                            format!("discover/{}", s.key),
                            s.results
                                .iter()
                                .filter(|r| b.category.is_empty() || r.category == b.category)
                                .map(|r| container(discover_card(b, r, look)).width(152).boxed())
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
    if let Some(d) =
        b.data(&key).or_else(|| b.previous_discover.as_ref().and_then(|k| b.data(k))).and_then(|d| d.discover.as_ref())
    {
        let results: Vec<_> = d.results.iter().filter(|r| b.category.is_empty() || r.category == b.category).collect();
        if results.is_empty() {
            let empty = if b.query.is_empty() {
                "No titles of this type".into()
            } else {
                format!("Nothing called “{}”{}", b.query, if b.category.is_empty() { "" } else { " of this type" })
            };
            page = page.push(layout::empty(empty, None, look));
        } else {
            let l = *look;
            if b.query.is_empty() {
                page = page.push(text("Popular right now", ty::S15, look.ink).font(SEMIBOLD).boxed());
            }
            page = page.push((layout::poster_grid(results.len(), move |i| discover_card(b, results[i], &l))).boxed());
        }
    } else if b.query.chars().count() != 1 {
        page = page.push((pending(b, &key, look)).boxed());
    }
    column![title, page.spacing(20)].boxed()
}
pub(super) fn discover_card<'a>(b: &'a Browsing, r: &'a Discover, look: &Look) -> View<'a> {
    let status = if r.title_id.is_some() {
        Some("In your library")
    } else if r.series_id.is_some() && r.monitor.as_deref().is_some_and(|m| m != "NONE") {
        Some("Downloading")
    } else if r.request_state.as_deref() == Some("PENDING") {
        Some("Requested")
    } else {
        None
    };
    let action = if let Some(id) = r.title_id {
        Some(AppMessage::Navigate(Route::Title(id as u64)))
    } else if status.is_none() && r.category != "MOVIES" && (b.can().manage_shows || b.can().request) {
        Some(msg(b, Message::Dialog(Some(Dialog::Pick(std::sync::Arc::new(r.clone()))))))
    } else {
        None
    };
    let art = crate::ui::tilt::art(r.poster.as_deref().and_then(|p| b.images.get(p)).cloned(), 14.0, 1.5, look);
    let mut layers = vec![art.boxed()];
    if let Some(status) = status {
        layers.push(container(badge(status, Tone::Media, look)).align_right(Fill).padding(8).boxed());
    }
    let card = card_button(look)
        .content(
            column![
                iced_widget::Stack::with_children(layers),
                truncate(&r.name, ty::S13, look.ink).font(MEDIUM),
                truncate(
                    format!(
                        "{} · {}",
                        media_label(&r.category),
                        r.because
                            .as_ref()
                            .map(|s| format!("Like {s}"))
                            .unwrap_or_else(|| r.year.map(|y| y.to_string()).unwrap_or("Upcoming".into()))
                    ),
                    ty::XS,
                    look.ink_3
                )
            ]
            .spacing(6),
        )
        .plain()
        .on_press_maybe(action)
        .boxed();
    iced_widget::tooltip(
        watch_image(b, r.poster.as_deref(), card),
        panel(
            column![
                text(&r.name, ty::S15, look.ink).font(SEMIBOLD),
                text(r.overview.as_deref().unwrap_or(""), ty::S13, look.ink_2),
                if r.category == "MOVIES" {
                    text(
                        "Place a movie folder in your library to watch. Automatic downloads are available for shows.",
                        ty::XS,
                        look.ink_3,
                    )
                    .boxed()
                } else {
                    space().boxed()
                }
            ]
            .spacing(8),
            14.0,
            look,
        )
        .padding(16)
        .width(320),
        iced_widget::tooltip::Position::FollowCursor,
    )
    .boxed()
}
