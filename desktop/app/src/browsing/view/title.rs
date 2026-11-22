// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;
use crate::ui::icon::icon;
use crate::ui::text::inherit;

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
    let done = c.video_count > 0 && c.watched_count >= c.video_count;
    let primary = |verb: &str, video: i64, extra: Vec<String>| {
        let mut content = row![icon(Icon::Play, 18.0), inherit(verb.to_owned(), ty::S15).font(MEDIUM)]
            .spacing(10)
            .align_y(Alignment::Center);
        for words in extra.into_iter().filter(|w| !w.is_empty()) {
            content = content.push(text(words, ty::S15, look.on_accent.scale_alpha(0.6)).boxed());
        }
        button("", look)
            .primary()
            .large()
            .content(content)
            .on_press(AppMessage::Navigate(Route::Watch(video as u64, None)))
            .boxed()
    };
    if let Some(n) = &t.next_up {
        let verb = if n.resuming {
            "Resume"
        } else if done {
            "Watch again"
        } else {
            "Play"
        };
        let mut extra = Vec::new();
        if c.kind == "SHOW" {
            extra.extend(n.video.label.clone());
        }
        if n.resuming {
            extra.push(remaining(n.video.position.unwrap_or(0.0), n.video.duration));
        }
        actions.push(primary(verb, n.video.id, extra));
    } else if let Some(v) = &t.movie {
        actions.push(primary("Play", v.id, Vec::new()));
    }
    if can.watch_together
        && let Some(video) =
            t.next_up.as_ref().map(|next| next.video.id).or_else(|| t.movie.as_ref().map(|video| video.id))
    {
        actions.push(
            button("Watch together", look).large().icon(Icon::Users).on_press(AppMessage::HostRoom(video)).boxed(),
        );
    }
    actions.push(title_menu(b, t, done, look));
    let mut meta = Row::<View<'a>>::new().spacing(16);
    for words in [
        c.year.map(|y| y.to_string()),
        Some(if c.kind == "SHOW" {
            let regular = t.seasons.iter().filter(|s| s.number > 0).count();
            let n = if regular > 0 { regular } else { t.seasons.len() };
            format!("{n} season{}", if n == 1 { "" } else { "s" })
        } else {
            t.movie.as_ref().and_then(|v| v.duration).map(runtime).unwrap_or_default()
        }),
        t.rating.map(|r| format!("{r:.1} rating")),
    ]
    .into_iter()
    .flatten()
    .filter(|w| !w.is_empty())
    {
        meta = meta.push(text(words, ty::SM, look.ink_2).boxed());
    }
    if !t.genres.is_empty() {
        meta = meta
            .push(text(t.genres.iter().take(4).cloned().collect::<Vec<_>>().join(", "), ty::SM, look.ink_3).boxed());
    }
    let details = column![
        container(text(&c.name, ty::Size { px: 42.0, line: 44.5 }, look.ink).font(SEMIBOLD)).width(Fill.max(600)),
        meta.wrap()
    ]
    .spacing(12)
    .width(Fill);
    let header =
        row![picture(b, c.poster.as_deref(), 152.0, 228.0, 16.0, look), details].spacing(28).align_y(Alignment::End);
    page = page.push(container(header).padding(Padding { top: 64.0, ..Padding::ZERO }).boxed());
    page = page.push(container(line(actions)).padding(Padding { top: 20.0, ..Padding::ZERO }).boxed());

    let show = c.kind == "SHOW";
    let mut side = Column::<View<'a>>::new().spacing(28).width(Fill);
    if let Some(overview) = &t.overview {
        let (lines, after) = if show { (7.0, 280) } else { (4.0, 420) };
        let long = overview.chars().count() > after;
        let expanded = b.expanded.contains(&id);
        let text = text(overview, ty::S15, look.ink_2).line_height(iced_core::text::LineHeight::Relative(1.5));
        let text = if long && !expanded {
            text.height(ty::S15.px * 1.5 * lines).ellipsis(iced_core::text::Ellipsis::End)
        } else {
            text
        };
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
        side = side.push(overview.boxed());
    }
    if let Some(next) = schedule.and_then(|s| s.next.as_ref()).filter(|e| e.air_at.is_some()) {
        side = side.push(
            row![
                icon(Icon::CalendarDays, 16.0).color(look.ink_3),
                text(
                    format!(
                        "{}{} airs {}, {}",
                        code(next.season, next.episode),
                        next.name.as_deref().map(|n| format!(" “{n}”")).unwrap_or_default(),
                        airs(next.air_at.unwrap_or_default()),
                        countdown(next.air_at.unwrap_or_default())
                    ),
                    ty::SM,
                    look.ink_2
                )
            ]
            .spacing(8)
            .boxed(),
        );
    }
    if let Some(series) = b.data(&Key::Series(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.series.as_ref()) {
        if can.manage_shows {
            side = side.push(series_panel(b, series, selected_season, look));
        }
    } else if b.features() && can.manage_shows && show {
        side = side.push(button("Manage downloads", look).on_press(act(b, Action::Manage(id))).boxed());
    }

    if !show || tabs.is_empty() {
        page = page.push(container(side).width(Fill.max(768)).padding(Padding { top: 40.0, ..Padding::ZERO }).boxed());
    } else {
        let season = selected_season;
        let current = t.seasons.iter().find(|s| s.number == season);
        let mut episodes = Column::<View<'a>>::new().spacing(8).width(Fill);
        let mut bar = Row::<View<'a>>::new().spacing(4).align_y(Alignment::Center);
        if tabs.len() > 1 {
            for (n, label) in &tabs {
                bar = bar.push(chip(label.clone(), *n == season, msg(b, Message::Season(*n)), look));
            }
        } else if let Some(s) = current.filter(|_| season == 0) {
            bar = bar.push(text(&s.name, ty::S15, look.ink).font(SEMIBOLD).boxed());
        }
        bar = bar.push(space().width(Fill).boxed());
        if let Some(s) = current.filter(|s| !s.episodes.is_empty()) {
            let ids: Vec<_> = s.episodes.iter().map(|e| e.id).collect();
            let all = s.episodes.iter().all(|e| e.finished);
            bar = bar.push(
                button(if all { "Mark season not watched" } else { "Mark season watched" }, look)
                    .plain()
                    .small()
                    .on_press(act(b, Action::Watched(ids, !all)))
                    .boxed(),
            );
        }
        episodes = episodes.push(container(bar).height(32).align_y(Alignment::Center).boxed());
        if let Some(title) = current.and_then(|s| s.title.as_deref()).filter(|title| *title != c.name) {
            episodes = episodes.push(text(title, ty::SM, look.ink_3).boxed());
        }
        if let Some(s) = current {
            for (i, v) in s.episodes.iter().enumerate() {
                episodes = episodes.push(episode(b, t, v, i, look));
            }
        }
        if let Some(series) = b.data(&Key::Series(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.series.as_ref()) {
            for e in series
                .episodes
                .iter()
                .filter(|e| e.season == season && e.video.is_none() && (e.aired || e.air_at.is_some()))
            {
                episodes = episodes.push(ghost(b, series, e, look));
            }
        }
        page = page.push(
            container(
                row![episodes, container(side).width(300).padding(Padding { top: 40.0, ..Padding::ZERO })].spacing(56),
            )
            .padding(Padding { top: 28.0, ..Padding::ZERO })
            .boxed(),
        );
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
fn episode<'a>(b: &'a Browsing, t: &'a Title, v: &'a Video, order: usize, look: &Look) -> View<'a> {
    let next = t.next_up.as_ref().is_some_and(|n| n.video.id == v.id)
        && !t.seasons.iter().all(|s| s.episodes.iter().all(|e| e.finished));
    let value = v
        .position
        .zip(v.duration)
        .filter(|(_, duration)| *duration > 0.0)
        .map(|(p, d)| (p / d) as f32)
        .filter(|_| !v.finished);
    let mut art_layers = vec![media::drain(
        picture(b, v.still.as_deref(), 176.0, 99.0, 10.0, look),
        v.finished,
        v.id,
        order,
        look.radius(10.0),
        look,
    )];
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
/// What's done to a title less often, behind its ⋯: marking all of it watched, its artwork and its
/// details.
#[derive(Debug, Clone, PartialEq)]
enum TitleChoice {
    Watched(bool),
    Artwork { id: i64, kind: Option<&'static str>, label: &'static str, reset: bool },
    Match,
    Refresh,
}

fn title_menu<'a>(b: &'a Browsing, t: &'a Title, done: bool, look: &Look) -> View<'a> {
    let id = t.card.id;
    let mut choices = vec![TitleChoice::Watched(!done)];
    if b.can().edit_metadata {
        let mut artwork = vec![
            (id, Some("POSTER"), "thumbnail", t.custom_poster),
            (id, Some("BACKDROP"), "banner", t.custom_backdrop),
        ];
        if let Some(movie) = &t.movie {
            artwork.push((movie.id, None, "video thumbnail", movie.custom_still));
        }
        for (id, kind, label, custom) in artwork {
            choices.push(TitleChoice::Artwork { id, kind, label, reset: false });
            if custom {
                choices.push(TitleChoice::Artwork { id, kind, label, reset: true });
            }
        }
        choices.extend([TitleChoice::Match, TitleChoice::Refresh]);
    }
    let trigger = squircle(container(icon(Icon::Ellipsis, 18.0)).center(44), 14.0, look).fill(look.panel).edge();
    crate::ui::select::menu(
        trigger.boxed(),
        choices,
        |choice| match choice {
            TitleChoice::Watched(true) => "Mark watched".into(),
            TitleChoice::Watched(false) => "Mark not watched".into(),
            TitleChoice::Artwork { label, reset, .. } => format!("{} {label}", if *reset { "Reset" } else { "Change" }),
            TitleChoice::Match => "Fix match".into(),
            TitleChoice::Refresh => "Refresh details".into(),
        },
        move |choice| match choice {
            TitleChoice::Watched(watched) => act(b, Action::TitleWatched(id, watched)),
            TitleChoice::Artwork { id, kind, reset, .. } => {
                act(b, Action::Artwork { id, kind: kind.map(str::to_owned), reset })
            },
            TitleChoice::Match => msg(b, Message::Dialog(Some(Dialog::Match(id)))),
            TitleChoice::Refresh => act(b, Action::Refresh(id)),
        },
        240.0,
        look,
    )
}

const MONITORS: &[&str] = &["NONE", "FUTURE", "MISSING"];

/// What's in the ⋯ of a show's downloads card.
#[derive(Debug, Clone, PartialEq)]
enum SeriesChoice {
    Schedule,
    LookAgain,
    Remove,
}

fn series_panel<'a>(b: &'a Browsing, s: &'a Series, season: i64, look: &Look) -> View<'a> {
    let monitor = crate::ui::select::select(
        Some(s.monitor.as_str()),
        MONITORS,
        |m| {
            match *m {
                "NONE" => "Off",
                "FUTURE" => "New episodes",
                _ => "Everything",
            }
            .into()
        },
        move |m| act(b, Action::UpdateSeries(s.id, json!({ "monitor": m }))),
        look,
    );
    let c = &s.counts;
    let status = if c.total == 0 {
        None
    } else if c.grabbed > 0 {
        Some((look.info, format!("{} downloading", c.grabbed)))
    } else if c.missing > 0 {
        Some((look.danger, format!("{} missing", c.missing)))
    } else if c.wanted > 0 {
        Some((look.warn, format!("{} to find", c.wanted)))
    } else if c.have >= c.total {
        Some((look.ok, format!("All {} episodes are in the library", c.total)))
    } else {
        Some((look.ink_3, format!("{} of {} in the library", c.have, c.total)))
    };
    let more = [
        ((c.grabbed > 0 || c.missing > 0 || c.wanted > 0) && c.have < c.total)
            .then(|| format!("{} of {} in the library", c.have, c.total)),
        (c.grabbed > 0 && c.wanted > 0).then(|| format!("{} to find", c.wanted)),
        (c.upcoming > 0).then(|| format!("{} not aired yet", c.upcoming)),
        (c.skipped > 0).then(|| format!("{} skipped", c.skipped)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let mut card = column![
        row![text("Downloads", ty::SM, look.ink).font(MEDIUM).width(Fill), container(monitor).width(160)]
            .align_y(Alignment::Center)
    ]
    .spacing(12);
    if !s.managed && s.monitor != "NONE" {
        card = card.push(text(format!("{} isn't managed; can't import.", s.library), ty::XS, look.warn).boxed());
    }
    if let Some((tone, words)) = status {
        let mut lines = column![
            row![squircle(space().width(6).height(6), 3.0, look).fill(tone), text(words, ty::S13, look.ink_2)]
                .spacing(10)
                .align_y(Alignment::Center)
        ]
        .spacing(4);
        if !more.is_empty() {
            lines = lines.push(
                container(text(more.join(", "), ty::XS, look.ink_3))
                    .padding(Padding { left: 16.0, ..Padding::ZERO })
                    .boxed(),
            );
        }
        card = card.push(lines.boxed());
    }
    let mut choices = vec![SeriesChoice::Schedule];
    if c.skipped > 0 {
        choices.push(SeriesChoice::LookAgain);
    }
    choices.push(SeriesChoice::Remove);
    let more = crate::ui::select::menu(
        container(icon(Icon::Ellipsis, 16.0)).center(32).boxed(),
        choices,
        |choice| {
            match choice {
                SeriesChoice::Schedule => "Refresh the schedule",
                SeriesChoice::LookAgain => "Look for skipped episodes again",
                SeriesChoice::Remove => "Remove from managed shows",
            }
            .into()
        },
        move |choice| match choice {
            SeriesChoice::Schedule => act(b, Action::RefreshSchedule(s.id)),
            SeriesChoice::LookAgain => act(b, Action::LookAgain(s.id, None, None)),
            SeriesChoice::Remove => {
                confirm(b, "Remove show?", "Existing files will stay in your library.", Action::RemoveSeries(s.id))
            },
        },
        256.0,
        look,
    );
    card = card.push(divider(look)).push(
        row![
            truncate(s.effective_profile.clone(), ty::S13, look.ink_3).width(Fill),
            icon_button(
                Icon::Search,
                "Search for releases",
                Some(msg(
                    b,
                    Message::Dialog(Some(Dialog::Releases {
                        series: s.id,
                        season,
                        episodes: vec![],
                        name: s.name.clone()
                    })),
                )),
                look,
            ),
            icon_button(
                Icon::Settings2,
                "Settings",
                Some(msg(b, Message::Dialog(Some(Dialog::SeriesSettings(std::sync::Arc::new(s.clone())))))),
                look,
            ),
            more
        ]
        .spacing(2)
        .align_y(Alignment::Center)
        .boxed(),
    );
    raised(card.boxed(), look)
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
