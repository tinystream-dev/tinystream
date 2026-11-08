// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub enum PaletteTarget {
    Route(Route),
    Action(Action),
    Overlay(crate::app::Overlay),
    Discover(String),
}
pub struct PaletteItem {
    pub label: String,
    pub target: PaletteTarget,
    pub group: &'static str,
    pub image: Option<String>,
}
impl PaletteItem {
    pub fn message(&self, generation: u64) -> AppMessage {
        match &self.target {
            PaletteTarget::Route(route) => AppMessage::Navigate(route.clone()),
            PaletteTarget::Action(action) => AppMessage::Browsing(generation, Message::Act(action.clone())),
            PaletteTarget::Overlay(overlay) => AppMessage::Overlay(Some(*overlay)),
            PaletteTarget::Discover(query) => AppMessage::Browsing(generation, Message::FindShows(query.clone())),
        }
    }
}
pub fn palette(app: &App) -> Vec<PaletteItem> {
    let b = &app.browsing;
    let needle = app.query.trim().to_lowercase();
    let mut items = crate::shell::commands("")
        .into_iter()
        .filter(|(_, route)| allowed(b, route))
        .map(|(label, route)| PaletteItem {
            label: if route == Route::Discover && !b.can().manage_shows {
                "Request shows".into()
            } else {
                label.into()
            },
            target: PaletteTarget::Route(route),
            group: "Go to",
            image: None,
        })
        .filter(|item| item.label.to_lowercase().contains(&needle))
        .collect::<Vec<_>>();
    for l in b.libraries().iter().filter(|l| l.name.to_lowercase().contains(&needle)) {
        items.push(PaletteItem {
            label: l.name.clone(),
            target: PaletteTarget::Route(Route::Library(l.name.clone())),
            group: "Go to",
            image: None,
        });
    }
    for (label, target) in [
        ("Pause all downloads", PaletteTarget::Action(Action::Bulk(true))),
        ("Resume all downloads", PaletteTarget::Action(Action::Bulk(false))),
        ("Notifications", PaletteTarget::Overlay(crate::app::Overlay::Notifications)),
        ("Keyboard shortcuts", PaletteTarget::Overlay(crate::app::Overlay::Shortcuts)),
    ] {
        if label.to_lowercase().contains(&needle)
            && (!matches!(target, PaletteTarget::Action(_)) || (b.features() && b.can().downloads))
        {
            items.push(PaletteItem { label: label.into(), target, group: "Actions", image: None });
        }
    }
    if needle.is_empty() {
        for c in &b.recent {
            items.push(PaletteItem {
                label: c.name.clone(),
                target: PaletteTarget::Route(Route::Title(c.id as u64)),
                group: "Recent",
                image: c.poster.clone(),
            });
        }
    } else if b.search_query.to_lowercase() == needle
        && let Some(d) = b.data(&Key::Search(b.search_query.clone()))
    {
        if let Some(s) = &d.search {
            for c in &s.titles {
                items.push(PaletteItem {
                    label: c.name.clone(),
                    target: PaletteTarget::Route(Route::Title(c.id as u64)),
                    group: "Titles",
                    image: c.poster.clone(),
                });
            }
            for v in &s.videos {
                items.push(PaletteItem {
                    label: format!(
                        "{} · {} · {}",
                        v.title.name,
                        v.label.as_deref().unwrap_or(""),
                        v.name.as_deref().unwrap_or("")
                    ),
                    target: PaletteTarget::Route(Route::Watch(v.id as u64, None)),
                    group: "Episodes",
                    image: None,
                });
            }
        }
        if let Some(s) = &d.music_search {
            for a in &s.artists {
                items.push(PaletteItem {
                    label: a.name.clone(),
                    target: PaletteTarget::Route(Route::Artist(a.id as u64)),
                    group: "Artists",
                    image: a.cover.clone(),
                });
            }
            for a in &s.albums {
                items.push(PaletteItem {
                    label: format!("{} · {}", a.name, a.artist),
                    target: PaletteTarget::Route(Route::Album(a.id as u64)),
                    group: "Albums",
                    image: a.cover.clone(),
                });
            }
            for t in &s.tracks {
                let Some(album) = t.album_id else { continue };
                items.push(PaletteItem {
                    label: format!("{} · {}", t.title, t.artist),
                    target: PaletteTarget::Route(Route::Album(album as u64)),
                    group: "Songs",
                    image: t.cover.clone(),
                });
            }
        }
    }
    if !needle.is_empty() && b.features() && (b.can().manage_shows || b.can().request) {
        items.push(PaletteItem {
            label: format!("{} “{}”", if b.can().manage_shows { "Add" } else { "Request" }, app.query.trim()),
            target: PaletteTarget::Discover(app.query.trim().into()),
            group: "Discover",
            image: None,
        });
    }
    items
}

pub fn search<'a>(app: &'a App, look: &Look) -> View<'a> {
    let b = &app.browsing;
    let mut results = Column::<View<'a>>::new().spacing(2).width(Fill);
    let mut group = "";
    let items = palette(app);
    let searching = !app.query.trim().is_empty()
        && (b.search_query != app.query.trim() || b.data(&Key::Search(b.search_query.clone())).is_none());
    for (index, item) in items.iter().enumerate() {
        if item.group != group {
            group = item.group;
            results = results.push(container(text(group, ty::XS2, look.ink_3)).padding([8, 10]).boxed());
        }
        let thumbnail = if item.image.is_some() {
            picture(b, item.image.as_deref(), 28.0, 42.0, 6.0, look)
        } else {
            container(crate::ui::icon::icon(Icon::Search, 18.0).color(look.ink_3)).center_x(28).center_y(28).boxed()
        };
        let content = row![thumbnail, text(item.label.clone(), ty::SM, look.ink).width(Fill)]
            .spacing(12)
            .align_y(Alignment::Center);
        results = results.push(
            iced_widget::mouse_area(menu_item(
                None,
                content,
                None,
                b.search_index == index,
                Some(item.message(b.generation)),
                look,
            ))
            .on_enter(AppMessage::SearchHover(index))
            .boxed(),
        );
    }
    if searching {
        results = results.push(container(spinner(16.0, look)).padding(16).boxed());
    } else if items.is_empty() {
        results = results.push(
            container(text(format!("Nothing called “{}”.", app.query.trim()), ty::SM, look.ink_3)).padding(24).boxed(),
        );
    }
    let l = *look;
    let field = row![
        crate::ui::icon::icon(Icon::Search, 18.0).color(look.ink_3),
        iced_widget::text_input("Search, or jump anywhere", &app.query)
            .id(crate::shell::SEARCH)
            .on_input(AppMessage::Query)
            .on_submit(AppMessage::Submit)
            .size(15)
            .padding([16, 0])
            .style(move |_, _| iced_widget::text_input::Style {
                background: iced_core::Background::Color(Color::TRANSPARENT),
                border: iced_core::Border::default(),
                placeholder: l.ink_3,
                value: l.ink,
                selection: l.selection
            }),
        text("Esc", ty::XS2, look.ink_3)
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([0, 16]);
    panel(
        column![
            field,
            divider(look),
            scrollable(results.padding([0, 8]))
                .direction(iced_widget::scrollable::Direction::Vertical(iced_widget::scrollable::Scrollbar::hidden()))
                .id("palette-results")
                .height(iced_core::Length::Shrink.max(440)),
            container(text("↑ ↓ Navigate    Enter Open    Esc Close", ty::XS2, look.ink_3)).padding([8, 16])
        ],
        16.0,
        look,
    )
    .width(640)
    .boxed()
}

pub fn notifications<'a>(app: &'a App, look: &Look) -> View<'a> {
    let b = &app.browsing;
    let inbox = b.data(&Key::Inbox).and_then(|d| d.notifications.as_ref());
    let unread = inbox.map_or(0, |i| i.unread);
    let mut items = Column::<View<'a>>::new().spacing(2);
    if let Some(i) = inbox {
        for fresh in [true, false] {
            if i.items.iter().any(|n| n.read_at.is_none() == fresh) {
                items = items.push(
                    (container(text(if fresh { "New" } else { "Earlier" }, ty::XS, look.ink_3)).padding([6, 10]))
                        .boxed(),
                );
            }
            for n in i.items.iter().filter(|n| n.read_at.is_none() == fresh) {
                let mut details =
                    column![text(&n.title, ty::S13, if fresh { look.ink } else { look.ink_2 }).font(if fresh {
                        MEDIUM
                    } else {
                        ty::REGULAR
                    })]
                    .spacing(4)
                    .width(Fill);
                if let Some(body) = &n.body {
                    details = details.push((text(body, ty::XS, look.ink_3)).boxed());
                }
                details = details.push((text(relative(n.created_at), ty::XS2, look.ink_3)).boxed());
                items = items.push(
                    (row![
                        card_button(look)
                            .plain()
                            .content(row![notice_thumb(b, n, look), details].spacing(12))
                            .on_press(msg(b, Message::Follow(n.clone())))
                            .boxed(),
                        icon_button(Icon::X, "Remove", Some(act(b, Action::DeleteNotice(Some(n.id)))), look)
                    ]
                    .align_y(Alignment::Start))
                    .boxed(),
                );
            }
        }
        if i.items.is_empty() {
            items = items.push((layout::empty("Nothing yet", None, look)).boxed());
        }
    } else {
        items = items.push((pending(b, &Key::Inbox, look)).boxed());
    }
    panel(
        column![
            row![
                text("Notifications", ty::SM, look.ink).font(MEDIUM).width(Fill),
                button(if unread > 0 { "Mark all read" } else { "Clear" }, look)
                    .plain()
                    .small()
                    .on_press_maybe(
                        (!b.busy)
                            .then(|| act(b, if unread > 0 { Action::Read(None) } else { Action::DeleteNotice(None) }))
                    )
                    .boxed()
            ]
            .spacing(8)
            .padding([8, 10]),
            divider(look),
            scrollable(items).height(iced_core::Length::Shrink.max(544))
        ],
        16.0,
        look,
    )
    .padding(6)
    .width(384)
    .boxed()
}

pub fn dialog<'a>(app: &'a App, look: &Look) -> Option<View<'a>> {
    let b = &app.browsing;
    let dialog = b.dialog.as_ref()?;
    let close = msg(b, Message::Dialog(None));
    let mut content = Column::<View<'a>>::new().spacing(16).width(Fill);
    match dialog {
        Dialog::Confirm { title, body, action } => {
            return Some(crate::ui::overlay::confirm(
                title,
                Some(body),
                "Confirm",
                true,
                move |yes| if yes { act(b, action.as_ref().clone()) } else { close.clone() },
                look,
            ));
        },
        Dialog::Pick(r) => {
            content = content.push(
                (row![
                    picture(b, r.poster.as_deref(), 80.0, 120.0, 10.0, look),
                    column![
                        text(&r.name, ty::S15, look.ink).font(SEMIBOLD),
                        text(r.overview.as_deref().unwrap_or(""), ty::S13, look.ink_2)
                    ]
                    .spacing(8)
                    .width(Fill)
                ]
                .spacing(16))
                .boxed(),
            );
            if b.can().manage_shows {
                let options = b.data(&Key::Options).and_then(|d| d.settings.as_ref());
                if let Some(options) = options {
                    content = content.push((text("Library", ty::S13, look.ink_2)).boxed()).push(
                        (line(
                            options
                                .libraries
                                .iter()
                                .filter(|l| l.metadata_provider.as_deref() == Some(&r.provider))
                                .map(|l| {
                                    chip(
                                        &l.name,
                                        b.pick_library == l.name,
                                        msg(b, Message::PickLibrary(l.name.clone())),
                                        look,
                                    )
                                })
                                .collect(),
                        ))
                        .boxed(),
                    );
                    if options.libraries.iter().find(|l| l.name == b.pick_library).is_some_and(|l| !l.managed) {
                        content = content.push(
                            (text(
                                format!("{} isn't managed; can't download into it.", b.pick_library),
                                ty::XS,
                                look.warn,
                            ))
                            .boxed(),
                        );
                    }
                    content = content.push((text("Quality", ty::S13, look.ink_2)).boxed()).push(
                        (line(
                            std::iter::once(chip(
                                "Library default",
                                b.profile.is_empty(),
                                msg(b, Message::Profile(String::new())),
                                look,
                            ))
                            .chain(options.profiles.iter().map(|p| {
                                chip(&p.name, b.profile == p.name, msg(b, Message::Profile(p.name.clone())), look)
                            }))
                            .collect(),
                        ))
                        .boxed(),
                    );
                } else {
                    content = content.push((pending(b, &Key::Options, look)).boxed());
                }
                content = content.push((text("Download", ty::S13, look.ink_2)).boxed()).push(
                    (line(
                        [("NONE", "Off"), ("FUTURE", "New episodes"), ("MISSING", "Everything")]
                            .into_iter()
                            .map(|(v, l)| chip(l, b.chosen_monitor() == v, msg(b, Message::Monitor(v.into())), look))
                            .collect(),
                    ))
                    .boxed(),
                );
                if b.chosen_monitor() == "MISSING"
                    && let Some(aired) = b
                        .data(&Key::Aired(r.provider.clone(), r.id.clone()))
                        .and_then(|d| d.aired_episodes)
                        .filter(|n| *n >= 100)
                {
                    content=content.push((text(format!("{} has {aired} aired episodes, and all of them will be downloaded. Pick Future to only get new ones.",r.name),ty::XS,look.warn)).boxed());
                }
                content = content.push(
                    (button("Add show", look)
                        .primary()
                        .on_press_maybe((!b.busy && options.is_some() && !b.pick_library.is_empty()).then(|| {
                            act(
                                b,
                                Action::Add(
                                    r.clone(),
                                    b.pick_library.clone(),
                                    b.chosen_monitor().to_owned(),
                                    b.profile.clone(),
                                ),
                            )
                        }))
                        .boxed())
                    .boxed(),
                );
            } else {
                content = content
                    .push(
                        (field(
                            "A note for the admin (optional)",
                            input(
                                "request-note",
                                &b.note,
                                "Dubbed if there is one, please",
                                move |n| msg(b, Message::Note(n)),
                                look,
                            )
                            .boxed(),
                            None,
                            look,
                        ))
                        .boxed(),
                    )
                    .push(
                        (button("Request", look)
                            .primary()
                            .on_press_maybe((!b.busy).then(|| act(b, Action::Request(r.clone(), b.note.clone()))))
                            .boxed())
                        .boxed(),
                    );
            }
        },
        Dialog::Releases { series, season, episodes, name } => {
            content = content.push(
                row![
                    text(
                        format!(
                            "{name} · {}",
                            if episodes.len() == 1 { code(*season, episodes[0]) } else { format!("season {season}") }
                        ),
                        ty::S15,
                        look.ink
                    )
                    .font(MEDIUM)
                    .width(Fill),
                    button("Search again", look).plain().small().on_press(msg(b, Message::ReleaseSubmit)).boxed()
                ]
                .spacing(8)
                .boxed(),
            );
            content = content.push(
                row![
                    input(
                        "release-query",
                        &b.release_input,
                        "Search releases",
                        move |q| msg(b, Message::ReleaseQuery(q)),
                        look
                    )
                    .on_submit(msg(b, Message::ReleaseSubmit))
                    .boxed(),
                    button("Search", look).on_press(msg(b, Message::ReleaseSubmit)).boxed()
                ]
                .spacing(8)
                .boxed(),
            );
            if let Some(r) = &b.manual_release {
                let season = b.manual_season.parse::<i64>().ok().filter(|s| (0..=999).contains(s));
                let episode = b.manual_episode.parse::<i64>().ok().filter(|e| (1..=9999).contains(e));
                let candidate = season.zip(episode).map(|(season, episode)| {
                    let mut candidate = r.as_ref().clone();
                    candidate.episodes = vec![EpisodeNumber { season, episode }];
                    candidate
                });
                content = content.push(raised(
                    column![
                        text(
                            format!(
                                "Couldn't resolve the episode for {}. Choose it below, or search for another release.",
                                r.release.title
                            ),
                            ty::SM,
                            look.ink_2
                        ),
                        text("For a single-episode release only.", ty::XS, look.ink_3),
                        row![
                            input(
                                "manual-season",
                                &b.manual_season,
                                "Season",
                                move |s| msg(b, Message::ManualSeason(s)),
                                look
                            )
                            .boxed(),
                            input(
                                "manual-episode",
                                &b.manual_episode,
                                "Episode",
                                move |s| msg(b, Message::ManualEpisode(s)),
                                look
                            )
                            .boxed()
                        ]
                        .spacing(8),
                        button("Download this episode", look)
                            .primary()
                            .on_press_maybe(
                                candidate
                                    .filter(|_| !b.busy)
                                    .map(|r| act(b, Action::Grab(*series, std::sync::Arc::new(r))))
                            )
                            .boxed(),
                        button("Cancel", look).plain().on_press(msg(b, Message::ManualRelease(None))).boxed()
                    ]
                    .spacing(10)
                    .boxed(),
                    look,
                ));
            }
            let key = Key::Releases(*series, *season, episodes.clone(), b.release_query.clone());
            if let Some(s) = b.data(&key).and_then(|d| d.series.as_ref()) {
                let accepted = s.releases.iter().filter(|r| r.verdict.accepted && !r.episodes.is_empty()).count();
                let rejected = s.releases.iter().filter(|r| !r.verdict.accepted).count();
                content = content.push(
                    text(format!("{accepted} good matches out of {} results", s.releases.len()), ty::SM, look.ink_3)
                        .boxed(),
                );
                let mut list = Column::<View<'a>>::new().spacing(4);
                for (i, r) in s
                    .releases
                    .iter()
                    .filter(|r| (r.verdict.accepted && !r.episodes.is_empty()) || b.show_rejected)
                    .enumerate()
                {
                    let attributes = &r.attributes;
                    let mut labels = Vec::new();
                    if i == 0 && r.verdict.accepted && !r.verdict.nonstandard {
                        labels.push(badge("Best match", Tone::Strong, look));
                    }
                    for value in [
                        attributes.resolution.map(|n| format!("{n}p")),
                        attributes.group.clone(),
                        attributes.codec.clone(),
                        attributes.source.clone(),
                        if attributes.dual_audio { Some("Dual audio".into()) } else { None },
                        if r.batch { Some(format!("{} episodes", r.episodes.len())) } else { None },
                    ]
                    .into_iter()
                    .flatten()
                    {
                        labels.push(badge(value, Tone::Quiet, look));
                    }
                    for warning in &r.verdict.warnings {
                        labels.push(badge(warning.clone(), Tone::Warn, look));
                    }
                    let done = b.grabbed.contains(&r.release.link);
                    let message = if r.episodes.is_empty() {
                        msg(b, Message::ManualRelease(Some(std::sync::Arc::new(r.clone()))))
                    } else if !r.verdict.warnings.is_empty() || !r.verdict.accepted {
                        confirm(
                            b,
                            "Download this release?",
                            &r.verdict
                                .rejections
                                .iter()
                                .chain(&r.verdict.warnings)
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", "),
                            Action::Grab(*series, std::sync::Arc::new(r.clone())),
                        )
                    } else {
                        act(b, Action::Grab(*series, std::sync::Arc::new(r.clone())))
                    };
                    list = list.push(raised(
                        column![
                            text(&r.release.title, ty::S13, look.ink).font(MEDIUM),
                            line(labels),
                            row![
                                text(
                                    format!(
                                        "{} · {} · {} seeds",
                                        r.release.source,
                                        bytes(r.release.size.unwrap_or(0)),
                                        r.release.seeders.unwrap_or(0)
                                    ),
                                    ty::XS,
                                    look.ink_3
                                )
                                .width(Fill),
                                button(if done { "Grabbed" } else { "Grab" }, look)
                                    .primary()
                                    .small()
                                    .on_press_maybe((!b.busy && !done).then_some(message))
                                    .boxed()
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            text(r.verdict.rejections.join(" · "), ty::XS, look.warn)
                        ]
                        .spacing(6)
                        .boxed(),
                        look,
                    ));
                }
                if s.releases.is_empty() {
                    list = list.push(layout::empty("No results", None, look));
                }
                if rejected > 0 {
                    list = list.push(
                        button(
                            format!("{} {rejected} that don't fit", if b.show_rejected { "Hide" } else { "Show" }),
                            look,
                        )
                        .plain()
                        .small()
                        .on_press(msg(b, Message::ShowRejected))
                        .boxed(),
                    );
                }
                content = content.push(scrollable(list).height(iced_core::Length::Shrink.max(420)).boxed());
            } else {
                content = content.push(pending(b, &key, look));
            }
        },
        Dialog::SeriesSettings(s) => {
            let draft = &b.draft;
            content = content.push(text(&s.name, ty::S15, look.ink).font(MEDIUM).boxed());
            let mut form = Column::<View<'a>>::new().spacing(16);
            form = form.push(text("Quality profile", ty::S13, look.ink_2).boxed());
            if let Some(options) = b.data(&Key::Options).and_then(|d| d.settings.as_ref()) {
                form = form.push(line(
                    std::iter::once(chip(
                        "Library default",
                        draft.profile.is_empty(),
                        msg(b, Message::Draft("profile", String::new())),
                        look,
                    ))
                    .chain(options.profiles.iter().map(|p| {
                        chip(&p.name, draft.profile == p.name, msg(b, Message::Draft("profile", p.name.clone())), look)
                    }))
                    .collect(),
                ));
                form = form.push(text("Sources", ty::S13, look.ink_2).boxed()).push(line(
                    std::iter::once(chip(
                        "All sources",
                        draft.sources.is_empty(),
                        msg(b, Message::Source(String::new())),
                        look,
                    ))
                    .chain(options.sources.iter().map(|source| {
                        chip(
                            &source.name,
                            draft.sources.contains(&source.name),
                            msg(b, Message::Source(source.name.clone())),
                            look,
                        )
                    }))
                    .collect(),
                ));
            }
            for (id, label, value) in [
                ("groups", "Preferred groups", &draft.groups),
                ("aliases", "Also known as", &draft.aliases),
                ("naming", "File names", &draft.naming),
            ] {
                form = form.push(field(
                    label,
                    input(id, value, "", move |value| msg(b, Message::Draft(id, value)), look).boxed(),
                    None,
                    look,
                ));
            }
            if let Some(preview) = b
                .data(&Key::Naming(s.id, draft.naming.clone()))
                .and_then(|d| d.series.as_ref())
                .and_then(|s| s.naming_preview.as_ref())
            {
                for sample in &preview.samples {
                    form = form.push(text(sample, ty::XS, look.ink_3).boxed());
                }
                if let Some(error) = &preview.error {
                    form = form.push(text(error, ty::XS, look.danger).boxed());
                }
            }
            form = form.push(text("Episode numbers in releases", ty::S13, look.ink_2).boxed()).push(line(
                [("AUTO", "Work it out"), ("SEASONAL", "Per season"), ("ABSOLUTE", "From the first episode")]
                    .into_iter()
                    .map(|(v, l)| chip(l, draft.numbering == v, msg(b, Message::Draft("numbering", v.into())), look))
                    .collect(),
            ));
            form = form.push(text("Seeding", ty::S13, look.ink_2).boxed()).push(line(
                [
                    ("default", "Default"),
                    ("none", "Don't seed"),
                    ("ratio", "Ratio 1.0"),
                    ("week", "7 days"),
                    ("forever", "Forever"),
                ]
                .into_iter()
                .map(|(v, l)| chip(l, draft.seeding == v, msg(b, Message::Draft("seeding", v.into())), look))
                .collect(),
            ));
            content = content.push(scrollable(form).height(iced_core::Length::Shrink.max(440)).boxed()).push(
                button("Save", look)
                    .primary()
                    .on_press_maybe((!b.busy).then(|| act(b, Action::UpdateSeries(s.id, draft.patch()))))
                    .boxed(),
            );
        },
        Dialog::Match(id) => {
            content = content
                .push((text("Fix match", ty::S15, look.ink).font(SEMIBOLD)).boxed())
                .push(
                    (input(
                        "match-query",
                        &b.match_input,
                        "Search titles",
                        move |q| msg(b, Message::MatchQuery(q)),
                        look,
                    )
                    .boxed())
                    .boxed(),
                )
                .push(
                    (line(
                        [("", "Library provider"), ("TMDB", "TMDB"), ("ANILIST", "AniList")]
                            .into_iter()
                            .map(|(v, l)| chip(l, b.provider == v, msg(b, Message::Provider(v.into())), look))
                            .collect(),
                    ))
                    .boxed(),
                );
            let key = Key::Match(*id, b.match_query.clone(), b.provider.clone());
            if let Some(c) = b.data(&key).and_then(|d| d.title.as_ref()).and_then(|t| t.match_candidates.as_ref()) {
                let mut list = Column::<View<'a>>::new().spacing(8);
                for r in &c.results {
                    list = list.push(
                        (card_button(look)
                            .plain()
                            .content(
                                row![
                                    picture(b, r.poster.as_deref(), 48.0, 72.0, 8.0, look),
                                    column![
                                        text(&r.name, ty::SM, look.ink),
                                        text(r.year.map(|y| y.to_string()).unwrap_or_default(), ty::XS, look.ink_3)
                                    ]
                                ]
                                .spacing(12),
                            )
                            .on_press(act(b, Action::Match(*id, r.provider.clone(), r.id.clone())))
                            .boxed())
                        .boxed(),
                    );
                }
                content = content.push((scrollable(list).height(iced_core::Length::Shrink.max(420))).boxed());
            } else {
                content = content.push((pending(b, &key, look)).boxed());
            }
        },
    }
    if let Some(error) = &b.error {
        content = content.push((text(error, ty::SM, look.danger)).boxed());
    }
    if b.busy {
        content = content.push((spinner(20.0, look)).boxed());
    }
    content = content.push((button("Close", look).plain().on_press(close.clone()).boxed()).boxed());
    Some(crate::ui::overlay::dialog(content, 640.0, close, look))
}

fn notice_thumb<'a>(b: &'a Browsing, n: &'a Notice, look: &Look) -> View<'a> {
    if let Some(image) = &n.image {
        return picture(
            b,
            Some(image),
            36.0,
            if matches!(n.kind.as_str(), "CLIP" | "CLIP_READY") { 36.0 } else { 54.0 },
            8.0,
            look,
        );
    }
    if let Some(actor) = &n.actor {
        return watch_image(
            b,
            actor.avatar.as_deref(),
            media::avatar(&actor.username, actor.avatar.as_deref().and_then(|p| b.images.get(p)), 36.0, None, look),
        );
    }
    let (glyph, color) = match n.kind.as_str() {
        "READY" => (Icon::Play, look.info),
        "INVITE" => (Icon::Users, look.social),
        "REQUEST_APPROVED" => (Icon::CircleCheck, look.ok),
        "REQUEST_DECLINED" => (Icon::CircleAlert, look.danger),
        "CLIP" | "CLIP_READY" => (Icon::Scissors, look.highlight),
        _ => (Icon::Inbox, look.ink_2),
    };
    squircle(container(crate::ui::icon::icon(glyph, 16.0).color(color)).center(36), 9.0, look).fill(look.panel).boxed()
}
