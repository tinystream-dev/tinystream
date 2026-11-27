// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub(super) fn downloads<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    if !b.can().downloads {
        return column![
            heading("Downloads", look),
            layout::empty("You don't have permission to manage downloads", None, look)
        ]
        .boxed();
    }
    let Some(d) = b.data(&Key::Downloads) else { return pending(b, &Key::Downloads, look) };
    let running = d
        .downloads
        .iter()
        .filter(|d| matches!(d.state.as_str(), "DOWNLOADING" | "SEEDING"))
        .map(|d| d.id)
        .collect::<Vec<_>>();
    let paused = d.downloads.iter().filter(|d| d.state == "PAUSED").map(|d| d.id).collect::<Vec<_>>();
    let mut bulk = Vec::new();
    if !running.is_empty() {
        bulk.push(
            button("Pause all", look)
                .plain()
                .small()
                .icon(Icon::Pause)
                .on_press_maybe((!b.busy).then(|| act(b, Action::Pause(running))))
                .boxed(),
        );
    }
    if !paused.is_empty() {
        bulk.push(
            button("Resume all", look)
                .plain()
                .small()
                .icon(Icon::Play)
                .on_press_maybe((!b.busy).then(|| act(b, Action::Resume(paused))))
                .boxed(),
        );
    }
    bulk.insert(
        0,
        container(media_select(&b.download_category, move |v| msg(b, Message::DownloadCategory(v)), look))
            .width(200)
            .boxed(),
    );
    let aside = Some(Row::with_children(bulk).spacing(8).align_y(Alignment::Center).boxed());
    let title = page_title("Downloads", aside, look);
    let mut page = Column::<View<'a>>::new().width(Fill);
    if let Some(e) = b.data(&Key::Engine).and_then(|d| d.download_engine.as_ref()) {
        if let Some(interface) = &e.kill_switch {
            page = page.push(
                (banner(
                    Icon::ShieldAlert,
                    look.danger,
                    text(format!("{interface} is down, so every torrent is paused"), ty::SM, look.danger).font(MEDIUM),
                    None,
                    look,
                ))
                .boxed(),
            );
        }
        if let Some(error) = &e.listen_error {
            page = page.push(
                squircle(
                    row![
                        container(crate::ui::icon::icon(Icon::CircleAlert, 20.0).color(look.warn))
                            .padding(Padding { top: 2.0, ..Padding::ZERO }),
                        text(error, ty::SM, look.ink_2).width(Fill)
                    ]
                    .spacing(12),
                    14.0,
                    look,
                )
                .fill(look.warn_deep.scale_alpha(0.1))
                .edge()
                .padding(16)
                .width(Fill)
                .boxed(),
            );
        }
        let down = b.rates.get("download").map_or(&[][..], Vec::as_slice);
        let up = b.rates.get("upload").map_or(&[][..], Vec::as_slice);
        let max = down.iter().chain(up).copied().fold(1.0f32, f32::max);
        let peers =
            d.downloads.iter().filter_map(|d| d.live.as_ref()).filter(|l| !l.paused).map(|l| l.peers).sum::<i64>();
        let shared = d.downloads.iter().filter_map(|d| d.live.as_ref()).map(|l| l.uploaded).sum::<i64>();
        let stat = |label: &'static str, value: String| {
            column![
                text(label, ty::XS, look.ink_3),
                crate::ui::numbers::ticker(&value, ty::Size { px: 24.0, line: 30.0 }, look)
            ]
            .spacing(4)
            .boxed()
        };
        let mut stats = Row::<View<'a>>::new()
            .spacing(40)
            .push(stat("Download", format!("{}/s", bytes(e.download_rate))))
            .push(stat("Upload", format!("{}/s", bytes(e.upload_rate))))
            .push(stat("Peers", peers.to_string()))
            .push(stat("Shared", bytes(shared)));
        if e.slow_hours {
            stats = stats.push(stat("Limits", "Slow hours".into()));
        }
        let chart = iced_widget::stack![
            container(crate::ui::numbers::sparkline(
                down,
                Some(max),
                look.info,
                Fill,
                80,
                std::time::Duration::from_secs(2),
                look
            ))
            .align_bottom(Fill),
            container(crate::ui::numbers::sparkline(
                up,
                Some(max),
                look.ok,
                Fill,
                80,
                std::time::Duration::from_secs(2),
                look
            ))
            .align_bottom(Fill),
            container(stats).padding([16, 20])
        ];
        page = page.push(
            container(squircle(chart, 18.0, look).fill(look.raised).edge().width(Fill).height(104))
                .padding(Padding { bottom: 12.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    let mine =
        d.downloads.iter().filter(|dl| b.download_category.is_empty() || dl.category == b.download_category).count();
    if mine == 0 {
        page = page.push(layout::empty(
            if b.download_category.is_empty() { "No downloads yet" } else { "No downloads of this type" },
            None,
            look,
        ));
    }
    for group in ["DOWNLOADING", "SEEDING", "DONE", "FAILED"] {
        let filtered = d
            .downloads
            .iter()
            .filter(|dl| b.download_category.is_empty() || dl.category == b.download_category)
            .filter(|dl| match group {
                "DOWNLOADING" => dl.state == "DOWNLOADING" || (dl.state == "PAUSED" && dl.finished_at.is_none()),
                "SEEDING" => dl.state == "SEEDING" || (dl.state == "PAUSED" && dl.finished_at.is_some()),
                "FAILED" => matches!(dl.state.as_str(), "FAILED" | "REMOVED"),
                _ => dl.state == group,
            })
            .collect::<Vec<_>>();
        let count = filtered.len();
        let mut list = Column::<View<'a>>::new().spacing(8);
        for (key, season, items) in by_season(&filtered) {
            if items.len() == 1 {
                list = list.push(download_row(b, items[0], false, look));
                continue;
            }
            let key = format!("{group}/{key}");
            let lead = items[0];
            let open = b.download_groups.contains(&key);
            let size = items.iter().map(|d| d.size.unwrap_or(0)).sum::<i64>();
            let done = items
                .iter()
                .map(|d| {
                    d.live.as_ref().map_or(if d.finished_at.is_some() { d.size.unwrap_or(0) } else { 0 }, |l| l.done)
                })
                .sum::<i64>();
            let rate = items.iter().filter_map(|d| d.live.as_ref()).map(|l| l.download_rate).sum::<i64>();
            let mut actions = Row::<View<'a>>::new().spacing(2).align_y(Alignment::Center);
            for (label, pause) in [("Pause all", true), ("Resume all", false)] {
                let ids = items
                    .iter()
                    .filter(|d| {
                        if pause { matches!(d.state.as_str(), "DOWNLOADING" | "SEEDING") } else { d.state == "PAUSED" }
                    })
                    .map(|d| d.id)
                    .collect::<Vec<_>>();
                if !ids.is_empty() {
                    actions = actions.push(icon_button(
                        if pause { Icon::Pause } else { Icon::Play },
                        label,
                        Some(act(b, if pause { Action::Pause(ids) } else { Action::Resume(ids) })),
                        look,
                    ));
                }
            }
            if let Some(series) = lead.series_id {
                actions = actions.push(more(
                    vec![(
                        "Delete the downloaded season".into(),
                        confirm(
                            b,
                            "Delete downloaded season?",
                            "Files downloaded by tinystream will be deleted. Files you added yourself stay.",
                            Action::DeleteDownloaded(series, Some(season)),
                        ),
                    )],
                    look,
                ));
            }
            actions = actions.push(icon_button(
                if open { Icon::ChevronDown } else { Icon::ChevronRight },
                if open { "Collapse" } else { "Expand" },
                Some(msg(b, Message::DownloadGroup(key))),
                look,
            ));
            let mut content = column![
                row![
                    picture(b, lead.poster.as_deref(), 40.0, 60.0, 7.0, look),
                    column![
                        title_link(
                            lead.series_name.as_deref().unwrap_or(&lead.name),
                            lead.title.as_ref().map(|t| Route::Title(t.id as u64)),
                            look
                        ),
                        text(
                            format!(
                                "Season {season} · {} downloads · {} · {}/s",
                                items.len(),
                                bytes(size),
                                bytes(rate)
                            ),
                            ty::XS,
                            look.ink_3
                        )
                    ]
                    .spacing(6)
                    .width(Fill),
                    actions
                ]
                .spacing(14),
                progress(if size > 0 { Some(done as f32 / size as f32) } else { None }, look.info, look)
            ]
            .spacing(10);
            if open {
                for dl in items {
                    content = content.push(download_row(b, dl, true, look));
                }
            }
            list = list.push(raised(content.boxed(), look));
        }
        if count > 0 {
            let (name, tone) = match group {
                "DOWNLOADING" => ("Downloading", look.ink),
                "SEEDING" => ("Seeding", look.ink),
                "DONE" => ("Finished", look.ink),
                _ => ("Removed & failed", look.danger.scale_alpha(0.9)),
            };
            let aside =
                if matches!(group, "DOWNLOADING" | "SEEDING") { count.to_string() } else { "last two weeks".into() };
            page = page.push(
                column![
                    row![text(name, ty::S15, tone).font(SEMIBOLD).width(Fill), text(aside, ty::XS, look.ink_3)]
                        .spacing(12)
                        .align_y(Alignment::Center),
                    list
                ]
                .spacing(14)
                .padding(Padding { bottom: 28.0, ..Padding::ZERO })
                .width(Fill)
                .boxed(),
            );
        }
    }
    if let Some(e) = b.data(&Key::Engine).and_then(|d| d.download_engine.as_ref()) {
        page = page.push(
            container(text(
                format!("Downloading to {} · libtorrent {}", e.download_path, e.version),
                ty::XS,
                look.ink_3,
            ))
            .padding(Padding { top: 8.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    column![title, page.spacing(16)].boxed()
}

fn by_season<'a>(items: &[&'a Download]) -> Vec<(String, i64, Vec<&'a Download>)> {
    let mut groups: Vec<(String, i64, Vec<&Download>)> = Vec::new();
    for &d in items {
        let seasons = d.episodes.iter().map(|e| e.season).collect::<std::collections::HashSet<_>>();
        let series = d.series_id.map(|id| id.to_string()).or_else(|| d.series_name.clone());
        if let Some(series) = series
            && seasons.len() == 1
        {
            let season = d.episodes[0].season;
            let key = format!("s{series}:{season}:{}", d.category);
            if let Some((_, _, items)) = groups.iter_mut().find(|(k, _, _)| *k == key) {
                items.push(d);
            } else {
                groups.push((key, season, vec![d]));
            }
        } else {
            groups.push((format!("d{}", d.id), 0, vec![d]));
        }
    }
    for (_, _, items) in &mut groups {
        items.sort_by_key(|d| d.episodes.iter().map(|e| e.episode).min().unwrap_or(i64::MAX));
    }
    groups
}
fn download_row<'a>(b: &'a Browsing, dl: &'a Download, nested: bool, look: &Look) -> View<'a> {
    let live = dl.live.as_ref();
    let paused = dl.state == "PAUSED" || live.is_some_and(|l| l.paused);
    let live_state = matches!(dl.state.as_str(), "DOWNLOADING" | "SEEDING" | "PAUSED");
    let mut heading = row![
        title_link(
            dl.series_name.as_deref().filter(|_| !nested).unwrap_or(&dl.name),
            dl.title.as_ref().map(|t| Route::Title(t.id as u64)),
            look
        ),
        state(&dl.state, look)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if dl.import_state == "PARTIAL" {
        heading = heading.push(badge("Partially imported", Tone::Danger, look));
    }
    if dl.import_state == "DONE" {
        heading = heading.push(badge("In library", Tone::Ok, look));
    }
    let mut details = column![
        heading,
        text(
            format!(
                "{} · {}{}",
                dl.source.as_deref().unwrap_or(""),
                bytes(dl.size.unwrap_or(0)),
                if dl.episodes.len() == 1 {
                    format!(" · {}", code(dl.episodes[0].season, dl.episodes[0].episode))
                } else if dl.episodes.len() > 1 {
                    format!(" · {} episodes", dl.episodes.len())
                } else {
                    String::new()
                }
            ),
            ty::XS,
            look.ink_3
        )
    ]
    .spacing(6)
    .width(Fill);
    if let Some(l) = live {
        if dl.state == "DOWNLOADING" {
            details = details.push(progress(
                if l.stage == "METADATA" { None } else { Some(l.progress as f32) },
                look.info,
                look,
            ));
        }
        let words = if dl.state == "DOWNLOADING" {
            let mut words = vec![
                format!("{:.1}%", l.progress * 100.0),
                format!("{} of {}", bytes(l.done), bytes(dl.size.unwrap_or(0))),
            ];
            if l.download_rate > 0 {
                words.push(format!("↓ {}/s", bytes(l.download_rate)));
            }
            words.extend(l.eta.map(|t| format!("{} left", runtime(t as f64))));
            words.push(format!("{} seeds, {} peers", l.seeds, l.peers));
            words
        } else {
            let mut words = vec![format!("ratio {:.2}", l.ratio), format!("{} shared", bytes(l.uploaded))];
            if l.upload_rate > 0 && !paused {
                words.push(format!("↑ {}/s", bytes(l.upload_rate)));
            }
            words
        };
        details = details.push(text(words.join(" · "), ty::XS, look.ink_3).boxed());
        if let Some(ratio) = dl.seed_goal.ratio {
            details = details.push(text(format!("Seeding to ratio {ratio:.1}"), ty::XS, look.ink_3).boxed());
        }
        if let Some(values) = b.rates.get(&format!("dl{}", dl.id)) {
            details = details.push(crate::ui::numbers::sparkline(
                values,
                None,
                look.info,
                Fill,
                28,
                std::time::Duration::from_secs(2),
                look,
            ));
        }
    }
    if let Some(error) = dl.error.as_ref().or(dl.import_error.as_ref()) {
        details = details.push(text(error, ty::S13, look.danger).boxed());
    }
    let mut actions = Row::<View<'a>>::new().spacing(2).align_y(Alignment::Center);
    let mut menu = Vec::new();
    if live_state {
        let action = if paused { Action::Resume(vec![dl.id]) } else { Action::Pause(vec![dl.id]) };
        actions = actions.push(icon_button(
            if paused { Icon::Play } else { Icon::Pause },
            if paused { "Resume" } else { "Pause" },
            (!b.busy).then(|| act(b, action.clone())),
            look,
        ));
        menu.push(("Check files again".into(), act(b, Action::Recheck(vec![dl.id]))));
    }
    if dl.finished_at.is_some() && dl.import_state != "DONE" && dl.state != "REMOVED" {
        actions = actions.push(
            button("Import", look).small().on_press_maybe((!b.busy).then(|| act(b, Action::Import(dl.id)))).boxed(),
        );
    }
    if !matches!(dl.state.as_str(), "REMOVED" | "DONE") {
        for (label, delete) in [("Remove, keep its files", false), ("Remove and delete its files", true)] {
            menu.push((
                label.into(),
                confirm(
                    b,
                    if delete { "Delete download and files?" } else { "Remove download?" },
                    if delete {
                        "This permanently deletes the downloaded files."
                    } else {
                        "The downloaded files will stay on disk."
                    },
                    Action::Remove(vec![dl.id], delete),
                ),
            ));
        }
    }
    if !menu.is_empty() {
        actions = actions.push(more(menu.clone(), look));
    }
    let row = row![
        if nested { space().width(0).boxed() } else { picture(b, dl.poster.as_deref(), 40.0, 60.0, 7.0, look) },
        details,
        actions
    ]
    .spacing(14)
    .boxed();
    let row = if nested { container(row).padding([10, 8]).boxed() } else { raised(row, look) };
    crate::ui::context_menu::context_menu(row, menu, look)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_only_the_same_series_season_and_category_and_keeps_episode_order() {
        let episode = |id, season, episode, category: &str| Download {
            id,
            series_id: Some(1),
            category: category.into(),
            episodes: vec![EpisodeNumber { season, episode }],
            ..Default::default()
        };
        let a = episode(1, 1, 3, "EPISODES");
        let b = episode(2, 1, 1, "EPISODES");
        let c = episode(3, 2, 1, "EPISODES");
        let d = episode(4, 1, 2, "SPECIALS");
        let pack = Download {
            id: 5,
            series_id: Some(1),
            episodes: vec![EpisodeNumber { season: 1, episode: 1 }, EpisodeNumber { season: 2, episode: 1 }],
            ..Default::default()
        };
        let groups = by_season(&[&a, &b, &c, &d, &pack]);
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[0].2.iter().map(|d| d.id).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(groups[3].2[0].id, 5);
    }
}
