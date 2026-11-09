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

    let plan = match plan(state, id, &show, &name, Path::new(&save_path), &files, &grabbed_for).await {
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
    let placed = place_files(&state.db, &batch, &plan, &how).await?;
    let imported = placed.completed;
    let problems = placed.problems;
    let used = placed.used;
    for &(season, episode) in &placed.episodes {
        sqlx::query(
            "UPDATE episodes SET state = 'done', next_search = NULL WHERE series_id = ? AND season = ? AND episode = ?",
        )
        .bind(series_id)
        .bind(season)
        .bind(episode)
        .execute(&state.db)
        .await?;
    }

    if imported == 0 {
        return fail(state, id, &problems.join("; ")).await;
    }

    let used = used.unwrap();

    sqlx::query(
        "UPDATE downloads SET import_state = ?, import_mode = ?, imported_at = ?, import_error = ? WHERE id = ?",
    )
    .bind(if problems.is_empty() { "done" } else { "partial" })
    .bind(used.as_str())
    .bind(now())
    .bind((!problems.is_empty()).then(|| problems.join("; ")))
    .bind(id)
    .execute(&state.db)
    .await?;

    if used == Transfer::Move && problems.is_empty() {
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
    let episodes = placed.episodes;

    if episodes.is_empty() {
        return Ok(());
    }
    state.events.send(Event::Imported {
        library: show.library.clone(),
        show: show.title.clone(),
        item_id,
        episodes: episodes.clone(),
    });

    announce_ready(state, &show, series_id, item_id, &episodes).await;
    Ok(())
}

#[derive(Default)]
struct Placed {
    completed: usize,
    used: Option<Transfer>,
    episodes: Vec<(u32, u32)>,
    problems: Vec<String>,
}

async fn recorded(
    db: &sqlx::SqlitePool,
    batch: &Batch<'_>,
    from: &Path,
    to: &Path,
) -> anyhow::Result<Option<Transfer>> {
    if !to.is_file() {
        return Ok(None);
    }
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM file_ops WHERE batch = ? AND src = ? AND dst = ? AND undone_at IS NULL ORDER BY id DESC LIMIT 1")
        .bind(&batch.id).bind(from.to_string_lossy().as_ref()).bind(to.to_string_lossy().as_ref()).fetch_optional(db).await?;
    Ok(match kind.as_deref() {
        Some("hardlink") => Some(Transfer::Hardlink),
        Some("copy") => Some(Transfer::Copy),
        Some("move") => Some(Transfer::Move),
        _ => None,
    })
}

async fn place_files(
    db: &sqlx::SqlitePool,
    batch: &Batch<'_>,
    plan: &[Planned],
    how: &[Transfer],
) -> anyhow::Result<Placed> {
    let mut out = Placed::default();
    for p in plan {
        let previous = recorded(db, batch, &p.from, &p.to).await?;
        let transfer = match previous {
            Some(mode) => Ok(mode),
            None => batch.transfer(&p.from, &p.to, how).await,
        };
        match transfer {
            Ok(mode) => {
                out.completed += 1;
                out.used = Some(mode);
                if previous.is_none() {
                    out.episodes.push((p.season, p.episode));
                }
                for (from, to) in &p.extras {
                    if recorded(db, batch, from, to).await?.is_none() {
                        if let Err(error) = batch.transfer(from, to, how).await {
                            out.problems.push(format!("{}: {error:#}", from.display()));
                        }
                    }
                }
            },
            Err(error) => out.problems.push(format!("{}: {error:#}", p.from.display())),
        }
    }
    Ok(out)
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

async fn plan(
    state: &AppState,
    download: i64,
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

        let previously_placed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM file_ops WHERE batch = ? AND src = ? AND kind IN ('hardlink', 'copy', 'move') AND undone_at IS NULL)")
            .bind(format!("import-{download}"))
            .bind(save_path.join(&f.path).to_string_lossy().as_ref()).fetch_one(&state.db).await?;
        if !previously_placed && already_have(state, &show.path, season, episode).await? {
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

    #[tokio::test]
    async fn partial_imports_keep_failures_retryable_and_only_announce_new_successes() {
        let dir = std::env::temp_dir().join(format!("ts-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::open(&dir.join("test.db")).await.unwrap();
        let mut plan = Vec::new();
        for episode in [1, 2] {
            let from = dir.join(format!("source-{episode}"));
            std::fs::write(&from, "video").unwrap();
            plan.push(Planned { from, to: dir.join(format!("dest-{episode}")), season: 1, episode, extras: vec![] });
        }
        std::fs::write(&plan[1].to, "occupied").unwrap();
        let batch = Batch::new(&db, "partial", "test");
        let first = place_files(&db, &batch, &plan, &[Transfer::Move]).await.unwrap();
        assert_eq!(first.completed, 1);
        assert_eq!(first.episodes, [(1, 1)]);
        assert_eq!(first.problems.len(), 1);
        assert!(plan[1].from.exists());
        std::fs::remove_file(&plan[1].to).unwrap();
        let retry = place_files(&db, &batch, &plan, &[Transfer::Move]).await.unwrap();
        assert_eq!(retry.completed, 2);
        assert_eq!(retry.episodes, [(1, 2)]);
        assert!(retry.problems.is_empty());
        db.close().await;
        std::fs::remove_dir_all(dir).unwrap();
    }

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
