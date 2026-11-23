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
        let mut header = row![
            row![
                title_link(&first.show, first.title.as_ref().map(|t| Route::Title(t.id as u64)), look),
                text(format!("Season {season}"), ty::SM, look.ink_3)
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .width(Fill),
            button("Search", look)
                .small()
                .icon(Icon::Search)
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
            header = header.push(more(menu.clone(), look));
        }
        let mut content = Column::<View<'a>>::new().push((header).boxed()).spacing(12);
        for e in episodes {
            let looked = if filter_of(e) == "upcoming" {
                String::new()
            } else {
                e.next_search
                    .filter(|_| e.state == "WANTED")
                    .map(|t| format!("next look {}", countdown(t).replace("airing now", "now")))
                    .or_else(|| e.searched_at.map(|t| format!("looked {}", relative(t))))
                    .unwrap_or_default()
            };
            let mut line = row![
                text(code(e.season, e.episode), ty::XS, look.ink_3).width(70),
                text(e.name.clone().unwrap_or_else(|| format!("Episode {}", e.episode)), ty::SM, look.ink).width(Fill),
                text(e.air_at.map(airs).unwrap_or_default(), ty::XS, look.ink_3)
            ]
            .spacing(12)
            .align_y(Alignment::Center);
            if !looked.is_empty() {
                line = line.push(text(looked, ty::XS, look.ink_3).width(144).align_x(Alignment::End).boxed());
            }
            line = line.push(if filter_of(e) == "upcoming" {
                badge("Not aired", Tone::Quiet, look)
            } else {
                state(&e.state, look)
            });
            content = content.push(line.boxed());
        }
        let card = raised(content.boxed(), look);
        page = page.push(crate::ui::context_menu::context_menu(card, menu, look));
    }
    page.spacing(16).boxed()
}
