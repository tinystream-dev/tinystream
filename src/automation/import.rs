// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};

use super::fsops::{Batch, Transfer};
use super::matching::Matcher;
use super::naming::{self, Values};
use super::{release, series};
use crate::config::{ImportMode, Seeding};
use crate::db::now;
use crate::events::Event;
use crate::library::parse::{self, FileKind};
use crate::notifications;
use crate::state::AppState;

#[derive(Debug)]
pub struct Planned {
    pub from: PathBuf,
    pub to: PathBuf,
    pub season: u32,
    pub episode: u32,

    pub extras: Vec<(PathBuf, PathBuf)>,
}

async fn fail(state: &AppState, id: i64, why: &str) -> anyhow::Result<()> {
    tracing::warn!("download {id} wasn't imported: {why}");

    sqlx::query("UPDATE downloads SET import_state = 'failed', import_error = ? WHERE id = ?")
        .bind(why)
        .bind(id)
        .execute(&state.db)
        .await?;

    state.events.send(Event::DownloadsChanged);
    Ok(())
}

pub async fn run(state: &Arc<AppState>, id: i64) -> anyhow::Result<()> {
    let (hash, name, series_id, episodes, save_path, seeding): (
        String,
        String,
        Option<i64>,
        String,
        String,
        Option<String>,
    ) = sqlx::query_as("SELECT hash, name, series_id, episodes, save_path, seeding FROM downloads WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    let Some(series_id) = series_id else {
        sqlx::query(
            "UPDATE downloads SET import_state = 'skipped', import_error = 'not for a show in a library' WHERE id = ?",
        )
        .bind(id)
        .execute(&state.db)
        .await?;

        return Ok(());
    };

    let config = state.config.current();
    let show = series::get(state, series_id).await?;

    let Some(library) = config.library(&show.library) else {
        return fail(state, id, &format!("the library {:?} doesn't exist anymore", show.library)).await;
    };

    if !library.managed {
        return fail(state, id, &format!("{} isn't a managed library, so tinystream won't write to it; turn on “Managed” in its settings, then import again", library.name)).await;
    }

    let files = state.automation.engine.files(&hash);

    if files.is_empty() {
        return fail(state, id, "the torrent engine doesn't know this download's files").await;
    }

    let grabbed_for: Vec<(u32, u32)> = serde_json::from_str(&episodes).unwrap_or_default();

    let plan = match plan(state, &show, &name, Path::new(&save_path), &files, &grabbed_for).await {
        Ok(p) => p,
        Err(e) => return fail(state, id, &format!("{e:#}")).await,
    };

    if plan.is_empty() {
        return fail(state, id, "none of its files could be matched to an episode the library is missing").await;
    }

    let rules: Seeding = seeding
        .as_deref()
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_else(|| config.downloads.seeding.clone());

    let will_seed = rules.ratio != Some(0.0) && rules.time.is_none_or(|t| !t.is_zero());

    let how: Vec<Transfer> = match config.downloads.import {
        ImportMode::Auto if will_seed => vec![Transfer::Hardlink, Transfer::Copy],
        ImportMode::Auto => vec![Transfer::Hardlink, Transfer::Move],
        ImportMode::Hardlink => vec![Transfer::Hardlink],
        ImportMode::Copy => vec![Transfer::Copy],
        ImportMode::Move => vec![Transfer::Move],
    };

    let batch = Batch::new(&state.db, format!("import-{id}"), format!("Imported {name}"));
    let mut used = None;
    let mut imported = 0;
    let mut problems = Vec::new();

    for p in &plan {
        match batch.transfer(&p.from, &p.to, &how).await {
            Ok(t) => {
                used = Some(t);
                imported += 1;

                for (from, to) in &p.extras {
                    if let Err(e) = batch.transfer(from, to, &how).await {
                        tracing::debug!("extra file {}: {e:#}", from.display());
                    }
                }

                sqlx::query("UPDATE episodes SET state = 'done', next_search = NULL WHERE series_id = ? AND season = ? AND episode = ?")
                    .bind(series_id)
                    .bind(p.season)
                    .bind(p.episode)
                    .execute(&state.db)
                    .await?;

                tracing::info!("imported {}", p.to.display());
            },
            Err(e) => problems.push(format!("{e:#}")),
        }
    }

    if imported == 0 {
        return fail(state, id, &problems.join("; ")).await;
    }

    let used = used.unwrap();

    sqlx::query(
        "UPDATE downloads SET import_state = 'done', import_mode = ?, imported_at = ?, import_error = ? WHERE id = ?",
    )
    .bind(used.as_str())
    .bind(now())
    .bind((!problems.is_empty()).then(|| problems.join("; ")))
    .bind(id)
    .execute(&state.db)
    .await?;

    if used == Transfer::Move {
        state.automation.engine.remove(&hash, false);

        sqlx::query("UPDATE downloads SET state = 'done', removed_at = ? WHERE id = ?")
            .bind(now())
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    state.scanner.request(&show.library);
    state.events.send(Event::DownloadsChanged);
    state.events.send(Event::SeriesChanged { series_id });
    let item_id = series::item_id(state, &show.path).await.ok().flatten();
    let episodes: Vec<(u32, u32)> = plan.iter().map(|p| (p.season, p.episode)).collect();

    state.events.send(Event::Imported {
        library: show.library.clone(),
        show: show.title.clone(),
        item_id,
        episodes: episodes.clone(),
    });

    announce_ready(state, &show, series_id, item_id, &episodes).await;
    Ok(())
}

async fn announce_ready(
    state: &AppState,
    show: &series::Row,
    series_id: i64,
    item_id: Option<i64>,
    episodes: &[(u32, u32)],
) {
    let one = episodes.len() == 1;

    let name: Option<String> = match episodes {
        [(s, e)] => sqlx::query_scalar("SELECT title FROM episodes WHERE series_id = ? AND season = ? AND episode = ?")
            .bind(series_id)
            .bind(s)
            .bind(e)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .flatten(),
        _ => None,
    };

    let users = notifications::who_can_see(state, &show.library).await;

    notifications::send(
        state,
        &users,
        notifications::New {
            kind: "ready",
            priority: true,
            title: format!(
                "{} {} {} ready",
                show.title,
                notifications::episodes_label(episodes),
                if one { "is" } else { "are" }
            ),
            body: Some(name.unwrap_or_else(|| "In your library now.".into())),
            image: item_id.map(|id| format!("/api/images/item/{id}/poster")).or_else(|| show.poster.clone()),
            link: item_id.map(|id| format!("/title/{id}")),
            ..Default::default()
        },
    )
    .await;
}

pub async fn plan(
    state: &AppState,
    show: &series::Row,
    release_name: &str,
    save_path: &Path,
    files: &[libtorrent_sys::FileEntry],
    grabbed_for: &[(u32, u32)],
) -> anyhow::Result<Vec<Planned>> {
    let matcher = Matcher::load(&state.db, show.id).await?;
    let style = series::style(state, show).await?;

    let videos: Vec<&libtorrent_sys::FileEntry> = files
        .iter()
        .filter(|f| matches!(parse::file_kind(Path::new(&f.path)), FileKind::Video))
        .filter(|f| !parse::is_sample(&file_stem(&f.path)))
        .collect();

    if videos.is_empty() {
        bail!("it has no video files");
    }

    let folder = Path::new(&show.path);
    let folder_name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (show_title, folder_year) = parse::title_and_year(&folder_name);
    let release_attrs = release::attributes(release_name);

    let mut plan = Vec::new();

    for f in &videos {
        let file_name = Path::new(&f.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let hit = file_episodes(&matcher, &file_name, videos.len(), grabbed_for);

        let Some(&(season, episode)) = hit.as_ref().and_then(|h| h.first()) else {
            tracing::info!("{file_name}: can't tell which episode this is; leaving it in the download folder");
            continue;
        };

        if already_have(state, &show.path, season, episode).await? {
            tracing::info!("{file_name}: the library already has S{season:02}E{episode:02}; skipping");
            continue;
        }

        let title: Option<String> =
            sqlx::query_scalar("SELECT title FROM episodes WHERE series_id = ? AND season = ? AND episode = ?")
                .bind(show.id)
                .bind(season)
                .bind(episode)
                .fetch_optional(&state.db)
                .await?
                .flatten();

        let attrs = release::attributes(&file_name);
        let stem = file_stem(&f.path);

        let values = Values {
            show: if show_title.is_empty() { show.title.clone() } else { show_title.clone() },
            year: folder_year.or(show.year),
            season,
            episode,
            title,
            group: attrs.group.or(release_attrs.group.clone()),
            quality: attrs.resolution.or(release_attrs.resolution).map(|r| format!("{r}p")),
            codec: naming::CODEC
                .find(&file_name)
                .or_else(|| naming::CODEC.find(release_name))
                .map(|m| m.as_str().to_string()),
            original: stem.clone(),
        };

        let season_dir = if season == 0 { "Specials".to_string() } else { naming::render(&style.folder, &values) };

        let ext =
            Path::new(&f.path).extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "mkv".into());

        let new_stem = naming::render(&style.file, &values);
        let to = folder.join(&season_dir).join(format!("{new_stem}.{ext}"));
        let from = save_path.join(&f.path);

        let mut extras = Vec::new();

        for other in files.iter().filter(|o| o.path != f.path) {
            let o = Path::new(&other.path);
            let o_name = o.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

            if o.parent() == Path::new(&f.path).parent()
                && let Some(suffix) = o_name.strip_prefix(&stem)
                && suffix.starts_with('.')
                && matches!(parse::file_kind(o), FileKind::Quiet)
            {
                extras
                    .push((save_path.join(&other.path), folder.join(&season_dir).join(format!("{new_stem}{suffix}"))));
            }
        }

        plan.push(Planned { from, to, season, episode, extras });
    }

    Ok(plan)
}

fn file_episodes(matcher: &Matcher, name: &str, videos: usize, grabbed_for: &[(u32, u32)]) -> Option<Vec<(u32, u32)>> {
    if videos == 1 && grabbed_for.len() == 1 {
        return Some(grabbed_for.to_vec());
    }
    matcher.matches_file(name).map(|m| m.episodes)
}

fn file_stem(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

async fn already_have(state: &AppState, show_path: &str, season: u32, episode: u32) -> anyhow::Result<bool> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM media m JOIN items i ON i.id = m.item_id
         WHERE i.path = ? AND m.season = ? AND ? BETWEEN m.episode AND COALESCE(m.episode_end, m.episode)",
    )
    .bind(show_path)
    .bind(season)
    .bind(episode)
    .fetch_one(&state.db)
    .await
    .context("checking the library")?;

    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::super::matching::Numbering;
    use super::*;

    #[test]
    fn selected_episode_takes_precedence_for_a_single_video() {
        let matcher = Matcher::new(&[], &[], Default::default(), Numbering::Auto);
        assert_eq!(file_episodes(&matcher, "S01E18.mkv", 1, &[(4, 18)]), Some(vec![(4, 18)]));
        assert_eq!(file_episodes(&matcher, "unknown.mkv", 1, &[(4, 18)]), Some(vec![(4, 18)]));
        assert_eq!(file_episodes(&matcher, "S01E18.mkv", 2, &[(4, 18)]), Some(vec![(1, 18)]));
        assert_eq!(file_episodes(&matcher, "S01E18.mkv", 1, &[]), Some(vec![(1, 18)]));
        assert_eq!(file_episodes(&matcher, "unknown.mkv", 2, &[(4, 18)]), None);
    }
}
