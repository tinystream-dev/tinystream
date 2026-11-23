// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub(super) fn wanted<'a>(b: &'a Browsing, route: &Route, look: &Look) -> View<'a> {
    let Some(d) = b.data(&Key::Wanted) else {
        return column![heading("Wanted", look), pending(b, &Key::Wanted, look)].width(Fill).boxed();
    };
    let filter = b.filter.get(route).map_or("all", String::as_str);
    let filter_of =
        |e: &Wanted| if e.state == "WANTED" && !e.aired { "upcoming".to_owned() } else { e.state.to_lowercase() };
    let filters = [
        ("all", "All"),
        ("wanted", "Searching"),
        ("grabbed", "Downloading"),
        ("upcoming", "Upcoming"),
        ("missing", "Missing"),
    ]
    .map(|(v, l)| {
        let n = d.wanted.iter().filter(|e| v == "all" || filter_of(e) == v).count();
        (v, if n > 0 { format!("{l} {n}") } else { l.to_owned() })
    });
    let switcher = segmented(filter, &filters, move |v| msg(b, Message::Filter(v.into())), look);
    let mut page = column![page_title("Wanted", Some(switcher), look)].width(Fill);
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
        let count = episodes.len();
        let mut actions = row![
            button("Search", look)
                .small()
                .icon(Icon::Search)
                .on_press(msg(
                    b,
                    Message::Dialog(Some(Dialog::Releases {
                        series,
                        season,
                        episodes: if count == 1 { vec![first.episode] } else { vec![] },
                        name: first.show.clone()
                    }))
                ))
                .boxed()
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        let menu = if b.can().downloads {
            vec![
                (
                    "Delete the downloaded season".into(),
                    confirm(
                        b,
                        "Delete downloaded season?",
                        "Files you added yourself stay.",
                        Action::DeleteDownloaded(series, Some(season)),
                    ),
                ),
                (
                    "Delete the downloaded show".into(),
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
        if !menu.is_empty() {
            actions = actions.push(more(menu.clone(), look));
        }
        let header = row![
            picture(b, first.title.as_ref().and_then(|t| t.poster.as_deref()), 40.0, 60.0, 7.0, look),
            column![
                title_link(&first.show, first.title.as_ref().map(|t| Route::Title(t.id as u64)), look),
                text(
                    format!("Season {season} · {count} episode{}", if count == 1 { "" } else { "s" }),
                    ty::XS,
                    look.ink_3
                )
            ]
            .spacing(6)
            .width(Fill),
            actions
        ]
        .spacing(14)
        .align_y(Alignment::Center);
        let mut list = Column::<View<'a>>::new();
        for (i, e) in episodes.into_iter().enumerate() {
            let upcoming = filter_of(e) == "upcoming";
            let when = if upcoming {
                e.air_at
                    .map(|t| {
                        let at = airs(t);
                        match at.split_once(' ') {
                            Some((day @ ("Today" | "Tomorrow"), rest)) => format!("Airs {} {rest}", day.to_lowercase()),
                            _ => format!("Airs {at}"),
                        }
                    })
                    .unwrap_or_default()
            } else {
                e.next_search
                    .filter(|_| e.state == "WANTED")
                    .map(|t| format!("Next look {}", countdown(t).replace("airing now", "now")))
                    .or_else(|| e.searched_at.map(|t| format!("Looked {}", relative(t))))
                    .unwrap_or_default()
            };
            if i > 0 {
                list = list.push(divider(look));
            }
            list = list.push(
                row![
                    text(code(e.season, e.episode), ty::XS, look.ink_3).width(64),
                    text(e.name.clone().unwrap_or_else(|| format!("Episode {}", e.episode)), ty::SM, look.ink)
                        .width(Fill),
                    text(when, ty::XS, look.ink_3),
                    container(if upcoming { badge("Upcoming", Tone::Quiet, look) } else { state(&e.state, look) })
                        .width(96)
                        .align_x(Alignment::End)
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 0])
                .boxed(),
            );
        }
        let content = column![header, container(list).padding(Padding { left: 54.0, right: 4.0, ..Padding::ZERO })]
            .spacing(6)
            .boxed();
        page = page.push(crate::ui::context_menu::context_menu(raised(content, look), menu, look));
    }
    page.spacing(16).boxed()
}
