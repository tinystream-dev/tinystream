// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub(super) fn title<'a>(b: &'a Browsing, id: i64, look: &Look) -> View<'a> {
    let key = Key::Title(id);
    let Some(d) = b.data(&key) else { return pending(b, &key, look) };
    let Some(t) = &d.title else { return layout::empty("This title isn't here anymore", None, look) };
    let c = &t.card;
    let can = b.can();
    let mut page = Column::<View<'a>>::new().width(Fill);
    let schedule = b
        .data(&Key::Series(id))
        .or_else(|| b.data(&Key::Schedule(id)))
        .and_then(|d| d.title.as_ref())
        .and_then(|t| t.series.as_ref());
    let mut tabs = t.seasons.iter().map(|s| (s.number, s.name.clone())).collect::<Vec<_>>();
    if let Some(series) = schedule {
        for e in &series.episodes {
            if e.video.is_none() && (e.aired || e.air_at.is_some()) && !tabs.iter().any(|(n, _)| *n == e.season) {
                tabs.push((e.season, if e.season == 0 { "Specials".into() } else { format!("Season {}", e.season) }));
            }
        }
    }
    tabs.sort_by_key(|(n, _)| if *n == 0 { i64::MAX } else { *n });
    let selected_season =
        b.seasons.get(&id).copied().unwrap_or_else(|| {
            if t.seasons.is_empty() { tabs.first().map_or(1, |(n, _)| *n) } else { initial_season(t) }
        });
    let mut actions = Vec::new();
    if let Some(n) = &t.next_up {
        actions.push(
            button(if n.resuming { "Resume" } else { "Play" }, look)
                .primary()
                .large()
                .icon(Icon::Play)
                .on_press(AppMessage::Navigate(Route::Watch(n.video.id as u64, None)))
                .boxed(),
        );
    } else if let Some(v) = &t.movie {
        actions.push(
            button("Play", look)
                .primary()
                .large()
                .icon(Icon::Play)
                .on_press(AppMessage::Navigate(Route::Watch(v.id as u64, None)))
                .boxed(),
        );
    }
    let done = c.video_count > 0 && c.watched_count >= c.video_count;
    actions.push(
        button(if done { "Watched" } else { "Mark watched" }, look)
            .large()
            .on_press_maybe((!b.busy).then(|| act(b, Action::TitleWatched(id, !done))))
            .boxed(),
    );
    if can.edit_metadata {
        actions.push(icon_button(
            Icon::Sparkles,
            "Fix match",
            (!b.busy).then(|| msg(b, Message::Dialog(Some(Dialog::Match(id))))),
            look,
        ));
        actions.push(icon_button(
            Icon::RotateCw,
            "Refresh details",
            (!b.busy).then(|| act(b, Action::Refresh(id))),
            look,
        ));
    }
    let details = column![
        text(&c.name, ty::Size { px: 44.0, line: 49.0 }, look.ink).font(SEMIBOLD),
        text(
            [
                c.year.map(|y| y.to_string()),
                Some(if c.kind == "SHOW" {
                    format!("{} season{}", t.seasons.len(), if t.seasons.len() == 1 { "" } else { "s" })
                } else {
                    t.movie.as_ref().and_then(|v| v.duration).map(runtime).unwrap_or_default()
                }),
                t.rating.map(|r| format!("{r:.1} rating"))
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
            ty::SM,
            look.ink_2
        ),
        text(t.genres.join(" · "), ty::S13, look.ink_3),
        if let Some(next) = schedule.and_then(|s| s.next.as_ref()).filter(|e| e.air_at.is_some()) {
            text(
                format!(
                    "{} {} · {}",
                    code(next.season, next.episode),
                    next.name.as_deref().unwrap_or(""),
                    countdown(next.air_at.unwrap_or_default())
                ),
                ty::SM,
                look.ink_2,
            )
            .boxed()
        } else {
            space().boxed()
        },
        line(actions)
    ]
    .spacing(14)
    .width(Fill);
    let header =
        row![picture(b, c.poster.as_deref(), 208.0, 312.0, 14.0, look), details].spacing(32).align_y(Alignment::End);
    page = page.push(container(header).padding(Padding { top: 40.0, ..Padding::ZERO }).boxed());
    if can.edit_metadata {
        page = page.push(
            (container(line(vec![
                artwork(b, id, Some("POSTER"), "thumbnail", t.custom_poster, look),
                artwork(b, id, Some("BACKDROP"), "banner", t.custom_backdrop, look),
            ]))
            .padding([16, 0]))
            .boxed(),
        );
    }
    if can.edit_metadata
        && let Some(movie) = &t.movie
    {
        page = page.push(artwork(b, movie.id, None, "video thumbnail", movie.custom_still, look));
    }
    if let Some(overview) = &t.overview {
        let long = overview.chars().count() > 420;
        let expanded = b.expanded.contains(&id);
        let text = text(overview, ty::S15, look.ink_2).line_height(iced_core::text::LineHeight::Relative(1.5));
        let text = if long && !expanded { text.height(90).ellipsis(iced_core::text::Ellipsis::End) } else { text };
        let mut overview = column![text].width(Fill);
        if long {
            overview = overview.push(
                button(if expanded { "Less" } else { "More" }, look)
                    .plain()
                    .small()
                    .on_press(msg(b, Message::Overview(id)))
                    .boxed(),
            );
        }
        page = page.push(container(overview).width(Fill.max(768)).padding([24, 0]).boxed());
    }
    if let Some(series) = b.data(&Key::Series(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.series.as_ref()) {
        if can.manage_shows {
            page = page.push((series_panel(b, series, selected_season, look)).boxed());
        }
    } else if b.features() && can.manage_shows && c.kind == "SHOW" {
        page = page.push((button("Manage downloads", look).on_press(act(b, Action::Manage(id))).boxed()).boxed());
    }
    if !tabs.is_empty() {
        let season = selected_season;
        if tabs.len() > 1 {
            page = page.push(
                container(line(
                    tabs.iter()
                        .map(|(n, label)| chip(label.clone(), *n == season, msg(b, Message::Season(*n)), look))
                        .collect(),
                ))
                .padding([20, 0])
                .boxed(),
            );
        }
        if let Some(s) = t.seasons.iter().find(|s| s.number == season) {
            let ids: Vec<_> = s.episodes.iter().map(|e| e.id).collect();
            let all = s.episodes.iter().all(|e| e.finished);
            page = page.push(
                (row![
                    text(s.title.as_deref().unwrap_or(&s.name), ty::S15, look.ink).font(SEMIBOLD).width(Fill),
                    button(if all { "Mark season unwatched" } else { "Mark season watched" }, look)
                        .plain()
                        .small()
                        .on_press(act(b, Action::Watched(ids, !all)))
                        .boxed()
                ]
                .align_y(Alignment::Center))
                .boxed(),
            );
            if let Some(o) = &s.overview {
                page = page.push((text(o, ty::SM, look.ink_2)).boxed());
            }
            for v in &s.episodes {
                page = page.push((episode(b, t, v, look)).boxed());
            }
        }
        if let Some(series) = b.data(&Key::Series(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.series.as_ref()) {
            for e in series
                .episodes
                .iter()
                .filter(|e| e.season == season && e.video.is_none() && (e.aired || e.air_at.is_some()))
            {
                page = page.push(ghost(b, series, e, look));
            }
        }
    }
    if let Some(s) = b.data(&Key::Similar(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.similar.as_ref()) {
        if !s.also_watched.is_empty() {
            page = page.push(
                (section(
                    "People also watched",
                    shelf(
                        b,
                        format!("title/{id}/also"),
                        s.also_watched.iter().map(|c| container(poster(b, c, None, look)).width(152).boxed()).collect(),
                        look,
                    ),
                    look,
                ))
                .boxed(),
            );
        }
        if !s.recommendations.is_empty() {
            page = page.push(
                (section(
                    "More like this",
                    shelf(
                        b,
                        format!("title/{id}/recommendations"),
                        s.recommendations
                            .iter()
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
    page.spacing(8).boxed()
}
fn initial_season(t: &Title) -> i64 {
    let regular: Vec<_> = t.seasons.iter().filter(|s| s.number > 0).collect();
    if t.seasons.iter().all(|s| s.episodes.iter().all(|e| e.finished)) {
        return regular.last().copied().or_else(|| t.seasons.last()).map_or(0, |s| s.number);
    }
    t.next_up
        .as_ref()
        .and_then(|n| t.seasons.iter().find(|s| s.episodes.iter().any(|e| e.id == n.video.id)))
        .or_else(|| regular.first().copied())
        .or_else(|| t.seasons.first())
        .map_or(0, |s| s.number)
}
fn artwork<'a>(b: &'a Browsing, id: i64, kind: Option<&str>, label: &str, custom: bool, look: &Look) -> View<'a> {
    let mut buttons = vec![
        button(format!("Change {label}"), look)
            .small()
            .on_press_maybe(
                (!b.busy).then(|| act(b, Action::Artwork { id, kind: kind.map(str::to_owned), reset: false })),
            )
            .boxed(),
    ];
    if custom {
        buttons.push(
            button(format!("Reset {label}"), look)
                .plain()
                .small()
                .on_press_maybe(
                    (!b.busy).then(|| act(b, Action::Artwork { id, kind: kind.map(str::to_owned), reset: true })),
                )
                .boxed(),
        );
    }
    line(buttons)
}
fn episode<'a>(b: &'a Browsing, t: &'a Title, v: &'a Video, look: &Look) -> View<'a> {
    let next = t.next_up.as_ref().is_some_and(|n| n.video.id == v.id)
        && !t.seasons.iter().all(|s| s.episodes.iter().all(|e| e.finished));
    let value = v
        .position
        .zip(v.duration)
        .filter(|(_, duration)| *duration > 0.0)
        .map(|(p, d)| (p / d) as f32)
        .filter(|_| !v.finished);
    let mut art_layers = vec![picture(b, v.still.as_deref(), 176.0, 99.0, 10.0, look)];
    if let Some(value) = value {
        art_layers.push(container(progress(Some(value), look.media_ink, look)).align_bottom(Fill).boxed());
    }
    let mut details = column![
        row![
            text(v.label.as_deref().unwrap_or(""), ty::XS, look.ink_3),
            truncate(
                v.name.clone().unwrap_or_else(|| format!("Episode {}", v.episode.unwrap_or(0))),
                ty::S15,
                if v.finished { look.ink_2 } else { look.ink }
            )
            .font(MEDIUM)
        ]
        .spacing(10)
    ]
    .width(Fill)
    .spacing(6);
    if let Some(overview) = &v.overview {
        details = details
            .push(text(overview, ty::S13, look.ink_3).height(39).ellipsis(iced_core::text::Ellipsis::End).boxed());
    }
    details = details.push(
        text(
            if value.is_some() {
                remaining(v.position.unwrap_or(0.0), v.duration)
            } else {
                v.duration.map(runtime).unwrap_or_default()
            },
            ty::XS,
            look.ink_3,
        )
        .boxed(),
    );
    let l = *look;
    let base = crate::ui::pressable::pressable(
        row![
            container(iced_widget::Stack::with_children(art_layers)).width(176),
            container(details).padding(Padding { right: 96.0, ..Padding::ZERO })
        ]
        .spacing(16),
        14.0,
        look,
        move |status| crate::ui::pressable::Appearance {
            paint: crate::ui::squircle::Paint {
                fill: if matches!(status, crate::ui::pressable::Status::Hovered | crate::ui::pressable::Status::Pressed)
                {
                    Some(l.hover)
                } else if next {
                    Some(l.raised)
                } else {
                    None
                },
                edge: next,
                ..crate::ui::squircle::Paint::NONE
            },
            ink: l.ink,
        },
    )
    .padding(12)
    .width(Fill)
    .on_press(AppMessage::Navigate(Route::Watch(v.id as u64, None)))
    .boxed();
    let up_to = t
        .seasons
        .iter()
        .find(|s| s.number == v.season.unwrap_or(0))
        .map(|s| {
            s.episodes
                .iter()
                .take_while(|e| e.id != v.id)
                .chain(std::iter::once(v))
                .filter(|e| !e.finished)
                .map(|e| e.id)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let from = t
        .seasons
        .iter()
        .flat_map(|s| &s.episodes)
        .filter(|e| {
            e.finished
                && (e.season.unwrap_or(0) > v.season.unwrap_or(0) || (e.season == v.season && e.episode >= v.episode))
        })
        .map(|e| e.id)
        .collect::<Vec<_>>();
    let mut buttons = Row::<View<'a>>::new().spacing(1);
    let mut menu = vec![(
        if v.finished { "Mark unwatched" } else { "Mark watched" }.into(),
        act(b, Action::Watched(vec![v.id], !v.finished)),
    )];
    if !up_to.is_empty() {
        buttons = buttons.push(icon_button(
            Icon::CheckCheck,
            "Mark watched up to here",
            (!b.busy).then(|| act(b, Action::Watched(up_to.clone(), true))),
            look,
        ));
        menu.push(("Mark watched up to here".into(), act(b, Action::Watched(up_to, true))));
    }
    if !from.is_empty() {
        buttons = buttons.push(icon_button(
            Icon::Undo2,
            "Mark unwatched from here",
            (!b.busy).then(|| act(b, Action::Watched(from.clone(), false))),
            look,
        ));
        menu.push(("Mark unwatched from here".into(), act(b, Action::Watched(from, false))));
    }
    buttons = buttons.push(icon_button(
        if v.finished { Icon::CircleCheck } else { Icon::Check },
        if v.finished { "Mark unwatched" } else { "Mark watched" },
        (!b.busy).then(|| act(b, Action::Watched(vec![v.id], !v.finished))),
        look,
    ));
    let mut layers = vec![base, container(buttons).align_right(Fill).padding(12).boxed()];
    if b.can().edit_metadata {
        let mut editors = row![icon_button(
            Icon::ImageUp,
            "Change episode thumbnail",
            (!b.busy).then(|| act(b, Action::Artwork { id: v.id, kind: None, reset: false })),
            look
        )]
        .spacing(1);
        if v.custom_still {
            editors = editors.push(icon_button(
                Icon::RotateCcw,
                "Reset episode thumbnail",
                (!b.busy).then(|| act(b, Action::Artwork { id: v.id, kind: None, reset: true })),
                look,
            ));
        }
        layers.push(container(editors).align_right(Fill).align_bottom(Fill).padding(12).boxed());
    }
    crate::ui::context_menu::context_menu(iced_widget::Stack::with_children(layers).width(Fill).boxed(), menu, look)
}
fn series_panel<'a>(b: &'a Browsing, s: &'a Series, season: i64, look: &Look) -> View<'a> {
    let monitors = line(
        [("NONE", "Off"), ("FUTURE", "New episodes"), ("MISSING", "Everything")]
            .into_iter()
            .map(|(v, l)| chip(l, s.monitor == v, act(b, Action::UpdateSeries(s.id, json!({"monitor":v}))), look))
            .collect(),
    );
    raised(
        column![
            row![text("Download automatically", ty::S15, look.ink).font(SEMIBOLD).width(Fill), state(&s.status, look)],
            monitors,
            text(format!("{} of {} in library · {} downloading · {} to find · {} missing · {} not aired yet · {} skipped · {}",s.counts.have,s.counts.total,s.counts.grabbed,s.counts.wanted,s.counts.missing,s.counts.upcoming,s.counts.skipped,s.effective_profile),ty::XS,look.ink_3),
            line(vec![
                button("Settings", look)
                    .small()
                    .on_press(msg(b, Message::Dialog(Some(Dialog::SeriesSettings(std::sync::Arc::new(s.clone()))))))
                    .boxed(),
                button("Look for skipped episodes again", look)
                    .plain()
                    .small()
                    .on_press(act(b, Action::LookAgain(s.id, None, None)))
                    .boxed(),
                button("Refresh schedule", look)
                    .plain()
                    .small()
                    .on_press(act(b, Action::RefreshSchedule(s.id)))
                    .boxed(),
                button("Search", look)
                    .small()
                    .on_press(msg(
                        b,
                        Message::Dialog(Some(Dialog::Releases {
                            series: s.id,
                            season,
                            episodes: vec![],
                            name: s.name.clone()
                        }))
                    ))
                    .boxed(),
                button("Remove from managed shows", look)
                    .danger()
                    .small()
                    .on_press(confirm(
                        b,
                        "Remove show?",
                        "Existing files will stay in your library.",
                        Action::RemoveSeries(s.id)
                    ))
                    .boxed()
            ])
        ]
        .spacing(12)
        .boxed(),
        look,
    )
}

fn ghost<'a>(b: &'a Browsing, series: &'a Series, e: &'a SeriesEpisode, look: &Look) -> View<'a> {
    let download = b.data(&Key::Downloads).and_then(|d| {
        d.downloads.iter().find(|d| {
            d.series_id == Some(series.id) && d.episodes.iter().any(|n| n.season == e.season && n.episode == e.episode)
        })
    });
    let mut actions = vec![
        button("Search", look)
            .small()
            .on_press(msg(
                b,
                Message::Dialog(Some(Dialog::Releases {
                    series: series.id,
                    season: e.season,
                    episodes: vec![e.episode],
                    name: series.name.clone(),
                })),
            ))
            .boxed(),
    ];
    if e.state == "SKIPPED" {
        actions.push(
            button("Look for again", look)
                .plain()
                .small()
                .on_press(act(b, Action::LookAgain(series.id, Some(e.season), Some(e.episode))))
                .boxed(),
        );
    }
    let mut content = column![
        row![
            text(code(e.season, e.episode), ty::XS, look.ink_3),
            text(e.name.as_deref().unwrap_or("Upcoming episode"), ty::SM, look.ink_2).width(Fill),
            if !e.aired { badge("Not aired", Tone::Quiet, look) } else { state(&e.state, look) },
            line(actions)
        ]
        .spacing(12)
        .align_y(Alignment::Center)
    ]
    .spacing(8);
    if let Some(l) = download.and_then(|d| d.live.as_ref()) {
        content = content.push(progress(Some(l.progress as f32), look.info, look)).push(
            text(
                format!(
                    "{:.1}% · {}/s{}",
                    l.progress * 100.0,
                    bytes(l.download_rate),
                    l.eta.map(|eta| format!(" · {} left", runtime(eta as f64))).unwrap_or_default()
                ),
                ty::XS,
                look.info,
            )
            .boxed(),
        );
    } else if let Some(at) = e.air_at {
        content = content.push(text(countdown(at), ty::XS, look.ink_3).boxed());
    }
    raised(content.boxed(), look)
}
