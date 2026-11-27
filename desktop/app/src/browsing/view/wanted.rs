// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::Length;
use iced_widget::{column, row};

use super::*;

/// How wide the page has to be for an episode's title, when it's looked for and its state to fit
/// on one line.
const ROW_MIN: f32 = 640.0;

pub(super) fn wanted<'a>(b: &'a Browsing, route: &Route, look: &Look) -> View<'a> {
    let (route, l) = (route.clone(), *look);
    iced_widget::responsive(move |size| page(b, &route, size.width >= ROW_MIN, &l)).height(Length::Shrink).boxed()
}

fn page<'a>(b: &'a Browsing, route: &Route, wide: bool, look: &Look) -> View<'a> {
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
        let backdrop = first.title.as_ref().and_then(|t| t.backdrop.as_deref().or(t.poster.as_deref()));
        let name = text(first.show.clone(), ty::XL, look.ink).font(SEMIBOLD);
        let name = match first.title.as_ref() {
            Some(t) => {
                let ink = look.ink;
                crate::ui::pressable::pressable(name, 6.0, look, move |_| crate::ui::pressable::Appearance {
                    paint: crate::ui::squircle::Paint::NONE,
                    ink,
                })
                .on_press(AppMessage::Navigate(Route::Title(t.id as u64)))
                .boxed()
            },
            None => name.boxed(),
        };
        let header = row![
            picture(b, first.title.as_ref().and_then(|t| t.poster.as_deref()), 64.0, 96.0, 10.0, look),
            column![
                name,
                text(
                    format!("Season {season} · {count} episode{}", if count == 1 { "" } else { "s" }),
                    ty::S13,
                    look.ink_2
                )
            ]
            .spacing(2)
            .width(Fill),
            actions
        ]
        .spacing(18)
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
                    .map(|t| match t - now() {
                        s if s <= 0 => "Looking now".to_owned(),
                        s => format!("Next look in {}", duration(if s >= 60 { (s + 59) / 60 * 60 } else { s })),
                    })
                    .or_else(|| e.searched_at.map(|t| format!("Looked {}", relative(t))))
                    .unwrap_or_default()
            };
            let (word, tone) = match (upcoming, e.state.as_str()) {
                (true, _) => ("Upcoming", look.ink_3),
                (_, "WANTED") => ("Searching", look.warn),
                (_, "GRABBED") => ("Downloading", look.info),
                (_, "MISSING") => ("Missing", look.danger),
                (_, "SKIPPED") => ("Skipped", look.ink_3),
                _ => ("Waiting", look.ink_3),
            };
            if i > 0 {
                list = list.push(divider(look));
            }
            let name = truncate(e.name.clone().unwrap_or_else(|| format!("Episode {}", e.episode)), ty::SM, look.ink);
            let when = truncate(when, ty::XS, look.ink_3);
            list = list.push(
                row![
                    text(code(e.season, e.episode), ty::XS, look.ink_3).width(64),
                    if wide {
                        row![name.width(Fill), when].spacing(16).width(Fill).align_y(Alignment::Center).boxed()
                    } else {
                        column![name, when].spacing(2).width(Fill).boxed()
                    },
                    row![
                        squircle(space().width(7).height(7), 3.5, look).fill(tone),
                        text(word, ty::S13, if upcoming { look.ink_2 } else { tone })
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .width(112)
                ]
                .spacing(16)
                .align_y(Alignment::Center)
                .padding([11, 0])
                .boxed(),
            );
        }
        let raised = look.raised;
        let band = 156.0;
        let content = column![
            container(header).padding(Padding { top: 20.0, right: 20.0, bottom: 0.0, left: 20.0 }),
            container(list).padding(Padding {
                top: 8.0,
                right: 20.0,
                bottom: 8.0,
                left: if wide { 102.0 } else { 20.0 }
            })
        ];
        let layers = [
            container(art(b, backdrop, 20.0, look)).width(Fill).height(band).boxed(),
            container(crate::ui::squircle::gradient(
                20.0,
                std::f32::consts::FRAC_PI_2,
                vec![(0.0, raised), (0.35, raised), (0.85, raised.scale_alpha(0.45))],
                look,
            ))
            .width(Fill)
            .height(band)
            .boxed(),
            container(crate::ui::squircle::gradient(
                20.0,
                std::f32::consts::PI,
                vec![(0.0, raised.scale_alpha(0.2)), (0.5, raised.scale_alpha(0.45)), (1.0, raised)],
                look,
            ))
            .width(Fill)
            .height(band)
            .boxed(),
        ];
        let mut card = iced_widget::Stack::with_children([content.width(Fill).boxed()]).width(Fill);
        for layer in layers.into_iter().rev() {
            card = card.push_under(layer);
        }
        let card = squircle(card.boxed(), 20.0, look).fill(raised).edge().width(Fill).boxed();
        page = page.push(crate::ui::context_menu::context_menu(card, menu, look));
    }
    page.spacing(16).boxed()
}
