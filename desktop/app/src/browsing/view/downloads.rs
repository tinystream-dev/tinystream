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
    let aside = (!bulk.is_empty()).then(|| {
        container(Row::with_children(bulk).spacing(6)).padding(Padding { bottom: 2.0, ..Padding::ZERO }).boxed()
    });
    let title = page_title("Downloads", aside, look);
    let mut page = column![
        container(media_select(&b.download_category, move |v| msg(b, Message::DownloadCategory(v)), look))
            .width(224)
            .padding(Padding { bottom: 8.0, ..Padding::ZERO })
    ]
    .width(Fill);
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
        let peers =
            d.downloads.iter().filter_map(|d| d.live.as_ref()).filter(|l| !l.paused).map(|l| l.peers).sum::<i64>();
        let shared = d.downloads.iter().filter_map(|d| d.live.as_ref()).map(|l| l.uploaded).sum::<i64>();
        let stats = line(vec![
            column![
                text("Download", ty::XS, look.ink_3),
                crate::ui::numbers::ticker(
                    &format!("{}/s", bytes(e.download_rate)),
                    ty::Size { px: 32.0, line: 40.0 },
                    look
                )
            ]
            .spacing(4)
            .boxed(),
            column![
                text("Upload", ty::XS, look.ink_3),
                crate::ui::numbers::ticker(
                    &format!("{}/s", bytes(e.upload_rate)),
                    ty::Size { px: 32.0, line: 40.0 },
                    look
                )
            ]
            .spacing(4)
            .boxed(),
            text(
                format!("{peers} peers · {} shared{}", bytes(shared), if e.slow_hours { " · slow hours" } else { "" }),
                ty::XS,
                look.ink_3,
            )
            .boxed(),
        ]);
        let chart = iced_widget::stack![
            container(crate::ui::numbers::sparkline(
                down,
                look.info,
                Fill,
                80,
                std::time::Duration::from_secs(2),
                look
            ))
            .align_bottom(Fill),
            container(crate::ui::numbers::sparkline(up, look.ok, Fill, 80, std::time::Duration::from_secs(2), look))
                .align_bottom(Fill),
            container(stats).padding(20)
        ];
        page = page.push(squircle(chart, 18.0, look).fill(look.raised).edge().width(Fill).height(140).boxed());
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
            let mut actions = vec![
                button(if open { "Collapse" } else { "Expand" }, look)
                    .plain()
                    .small()
                    .on_press(msg(b, Message::DownloadGroup(key)))
                    .boxed(),
            ];
            for (label, pause) in [("Pause", true), ("Resume", false)] {
                let ids = items
                    .iter()
                    .filter(|d| {
                        if pause { matches!(d.state.as_str(), "DOWNLOADING" | "SEEDING") } else { d.state == "PAUSED" }
                    })
                    .map(|d| d.id)
                    .collect::<Vec<_>>();
                if !ids.is_empty() {
                    actions.push(
                        button(label, look)
                            .small()
                            .on_press(act(b, if pause { Action::Pause(ids) } else { Action::Resume(ids) }))
                            .boxed(),
                    );
                }
            }
            if let Some(series) = lead.series_id {
                actions.push(
                    button("Delete season", look)
                        .danger()
                        .small()
                        .on_press(confirm(
                            b,
                            "Delete downloaded season?",
                            "Files downloaded by tinystream will be deleted. Files you added yourself stay.",
                            Action::DeleteDownloaded(series, Some(season)),
                        ))
                        .boxed(),
                );
            }
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
                    .width(Fill)
                ]
                .spacing(14),
                progress(if size > 0 { Some(done as f32 / size as f32) } else { None }, look.info, look),
                line(actions)
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
        details = details.push(
            text(
                format!(
                    "{:.1}% · ↓ {}/s · ↑ {}/s · {} peers · {} seeds · ratio {:.2}{}",
                    l.progress * 100.0,
                    bytes(l.download_rate),
                    bytes(l.upload_rate),
                    l.peers,
                    l.seeds,
                    l.ratio,
                    l.eta.map(|t| format!(" · {} left", runtime(t as f64))).unwrap_or_default()
                ),
                ty::XS,
                look.ink_3,
            )
            .boxed(),
        );
        if let Some(ratio) = dl.seed_goal.ratio {
            details = details.push(text(format!("Seeding to ratio {ratio:.1}"), ty::XS, look.ink_3).boxed());
        }
        if let Some(values) = b.rates.get(&format!("dl{}", dl.id)) {
            details = details.push(crate::ui::numbers::sparkline(
                values,
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
    let mut actions = Vec::new();
    let mut menu = Vec::new();
    if live_state {
        let action = if paused { Action::Resume(vec![dl.id]) } else { Action::Pause(vec![dl.id]) };
        actions.push(
            button(if paused { "Resume" } else { "Pause" }, look)
                .small()
                .on_press_maybe((!b.busy).then(|| act(b, action.clone())))
                .boxed(),
        );
        menu.push((if paused { "Resume" } else { "Pause" }.into(), act(b, action)));
        menu.push(("Recheck".into(), act(b, Action::Recheck(vec![dl.id]))));
    }
    if dl.finished_at.is_some() && dl.import_state != "DONE" && dl.state != "REMOVED" {
        actions.push(
            button("Import", look).small().on_press_maybe((!b.busy).then(|| act(b, Action::Import(dl.id)))).boxed(),
        );
    }
    if !matches!(dl.state.as_str(), "REMOVED" | "DONE") {
        actions.push(button("Recheck", look).plain().small().on_press(act(b, Action::Recheck(vec![dl.id]))).boxed());
        for (label, delete) in [("Remove", false), ("Delete files", true)] {
            let message = confirm(
                b,
                if delete { "Delete download and files?" } else { "Remove download?" },
                if delete {
                    "This permanently deletes the downloaded files."
                } else {
                    "The downloaded files will stay on disk."
                },
                Action::Remove(vec![dl.id], delete),
            );
            menu.push((label.into(), message.clone()));
            actions.push(button(label, look).danger().small().on_press(message).boxed());
        }
    }
    details = details.push(line(actions));
    let row = row![
        if nested { space().width(0).boxed() } else { picture(b, dl.poster.as_deref(), 40.0, 60.0, 7.0, look) },
        details
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
