// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub(super) fn wanted<'a>(b: &'a Browsing, route: &Route, look: &Look) -> View<'a> {
    let mut page = column![heading("Wanted", look)].width(Fill);
    let Some(d) = b.data(&Key::Wanted) else { return page.push((pending(b, &Key::Wanted, look)).boxed()).boxed() };
    let filter = b.filter.get(route).map_or("all", String::as_str);
    let filter_of =
        |e: &Wanted| if e.state == "WANTED" && !e.aired { "upcoming".to_owned() } else { e.state.to_lowercase() };
    page = page.push(
        (line(
            [
                ("all", "All"),
                ("wanted", "Searching"),
                ("grabbed", "Downloading"),
                ("upcoming", "Upcoming"),
                ("missing", "Missing"),
            ]
            .into_iter()
            .map(|(v, l)| {
                chip(
                    format!("{l} {}", d.wanted.iter().filter(|e| v == "all" || filter_of(e) == v).count()),
                    filter == v,
                    msg(b, Message::Filter(v.into())),
                    look,
                )
            })
            .collect(),
        ))
        .boxed(),
    );
    let mut groups: std::collections::BTreeMap<(i64, i64), Vec<&Wanted>> = Default::default();
    for e in &d.wanted {
        if filter == "all" || filter_of(e) == filter {
            groups.entry((e.series_id, e.season)).or_default().push(e);
        }
    }
    if groups.is_empty() {
        return page
            .push(
                (layout::empty(if filter == "all" { "Nothing is missing" } else { "Nothing here" }, None, look))
                    .boxed(),
            )
            .boxed();
    }
    for ((series, season), mut episodes) in groups {
        episodes.sort_by_key(|e| e.episode);
        let first = episodes[0];
        let mut header = row![
            title_link(&first.show, first.title.as_ref().map(|t| Route::Title(t.id as u64)), look),
            text(format!("Season {season}"), ty::SM, look.ink_3),
            space().width(Fill),
            button("Search", look)
                .small()
                .on_press(msg(
                    b,
                    Message::Dialog(Some(Dialog::Releases {
                        series,
                        season,
                        episodes: if episodes.len() == 1 { vec![first.episode] } else { vec![] },
                        name: first.show.clone()
                    }))
                ))
                .boxed()
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if b.can().downloads {
            header = header.push(icon_button(
                Icon::Trash2,
                "Delete downloaded season",
                Some(confirm(
                    b,
                    "Delete downloaded season?",
                    "This permanently deletes the season's downloaded files.",
                    Action::DeleteDownloaded(series, Some(season)),
                )),
                look,
            ));
        }
        let mut content = Column::<View<'a>>::new().push((header).boxed()).spacing(12);
        for e in episodes {
            content = content.push(
                (row![
                    text(code(e.season, e.episode), ty::XS, look.ink_3).width(70),
                    text(e.name.clone().unwrap_or_else(|| format!("Episode {}", e.episode)), ty::SM, look.ink)
                        .width(Fill),
                    text(e.air_at.map(airs).unwrap_or_default(), ty::XS, look.ink_3),
                    text(
                        if filter_of(e) == "upcoming" {
                            String::new()
                        } else {
                            e.next_search
                                .filter(|_| e.state == "WANTED")
                                .map(|t| format!("next look {}", countdown(t).replace("airing now", "now")))
                                .or_else(|| e.searched_at.map(|t| format!("looked {}", relative(t))))
                                .unwrap_or_default()
                        },
                        ty::XS,
                        look.ink_3
                    )
                    .width(144)
                    .align_x(Alignment::End),
                    if filter_of(e) == "upcoming" {
                        badge("Not aired", Tone::Quiet, look)
                    } else {
                        state(&e.state, look)
                    }
                ]
                .spacing(12)
                .align_y(Alignment::Center))
                .boxed(),
            );
        }
        let card = raised(content.boxed(), look);
        let menu = if b.can().downloads {
            vec![
                (
                    "Delete season".into(),
                    confirm(
                        b,
                        "Delete downloaded season?",
                        "Files you added yourself stay.",
                        Action::DeleteDownloaded(series, Some(season)),
                    ),
                ),
                (
                    "Delete show".into(),
                    confirm(
                        b,
                        "Delete downloaded show?",
                        "Downloaded episodes are deleted and skipped. Files you added yourself stay.",
                        Action::DeleteDownloaded(series, None),
                    ),
                ),
            ]
        } else {
            vec![]
        };
        page = page.push(crate::ui::context_menu::context_menu(card, menu, look));
    }
    page.spacing(16).boxed()
}
