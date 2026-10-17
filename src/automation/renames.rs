// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use anyhow::bail;
use regex::Regex;
use serde::Serialize;

use super::fsops::Batch;
use super::matching::{Match, Matcher, Numbering, Scope};
use super::naming::{self, Style, Values};
use super::{release, series};
use crate::config::{Monitor, Provider};
use crate::db::now;
use crate::events::Event;
use crate::library::parse::{self, FileKind};
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq)]
struct Suggestion {
    src: PathBuf,
    dst: PathBuf,
    reason: String,
    confidence: &'static str,
}

static ORDINAL_SEASON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\d{1,2})(?:st|nd|rd|th)[ ._-]+(?:season|series)\b").unwrap());

const SPLITS_FOR: Duration = Duration::from_secs(12 * 3600);

static SPLITS: LazyLock<Mutex<HashMap<(String, String), (Instant, Option<Split>)>>> = LazyLock::new(Default::default);

/// A show added as a later AniList season of another one in the same library, and where it goes in
/// that one.
#[derive(Clone)]
struct Split {
    parent: PathBuf,
    season: i64,
    offset: i64,
}

impl Split {
    fn place(&self, season: i64, episode: i64) -> (u32, u32) {
        match season {
            0 => (0, episode as u32),
            1 => (self.season as u32, (episode + self.offset) as u32),
            n => ((self.season + n - 1) as u32, episode as u32),
        }
    }
}

async fn split_of(state: &AppState, library: &str, provider_id: &str) -> anyhow::Result<Option<Split>> {
    let key = (library.to_string(), provider_id.to_string());

    if let Some((at, split)) = SPLITS.lock().unwrap().get(&key)
        && at.elapsed() < SPLITS_FOR
    {
        return Ok(split.clone());
    }

    let split = find_split(state, library, provider_id).await?;
    SPLITS.lock().unwrap().insert(key, (Instant::now(), split.clone()));
    Ok(split)
}

async fn find_split(state: &AppState, library: &str, provider_id: &str) -> anyhow::Result<Option<Split>> {
    let Some(first) = state.metadata.first_season(Provider::Anilist, provider_id).await? else { return Ok(None) };

    if first.id == provider_id {
        return Ok(None);
    }

    let parent: Option<(String, Option<i64>)> = sqlx::query_as(
        "SELECT path, id FROM items WHERE library = ?1 AND kind = 'show' AND provider = 'anilist' AND provider_id = ?2
         UNION ALL SELECT path, NULL FROM series WHERE library = ?1 AND provider = 'anilist' AND provider_id = ?2
         LIMIT 1",
    )
    .bind(library)
    .bind(&first.id)
    .fetch_optional(&state.db)
    .await?;

    let Some((parent, item)) = parent else { return Ok(None) };

    let seasons = match item {
        Some(item) => crate::metadata::library_seasons(&state.db, item).await?,
        None => Default::default(),
    };

    let schedule = state.metadata.schedule(state, Provider::Anilist, &first.id, &seasons).await?;

    let place = schedule.seasons.iter().find_map(|s| {
        if s.provider_id.as_deref() == Some(provider_id) {
            return Some((s.number, 0));
        }

        s.parts.iter().find(|p| p.provider_id.as_deref() == Some(provider_id)).map(|p| (s.number, p.offset))
    });

    Ok(place.map(|(season, offset)| Split { parent: PathBuf::from(parent), season, offset }))
}

struct Show {
    path: PathBuf,
    title: String,
    year: Option<i64>,
    matcher: Matcher,
    style: Style,
    series_id: Option<i64>,
}

async fn show(state: &AppState, library: &str, path: &Path) -> anyhow::Result<Show> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (title, year) = parse::title_and_year(&name);
    let p = path.to_string_lossy().to_string();
    let row = series::by_path(state, &p).await?;

    let (matcher, style, series_id) = match &row {
        Some(r) => (Matcher::load(&state.db, r.id).await?, series::style(state, r).await?, Some(r.id)),
        None => {
            let style = match naming::show_style(&state.db, &p).await? {
                Some(s) => s,
                None => naming::library_style(&state.db, library).await?,
            };

            (Matcher::new(&[title.clone()], &[], BTreeMap::new(), Numbering::Auto), style, None)
        },
    };

    Ok(Show { path: path.to_path_buf(), title, year, matcher, style, series_id })
}

impl Show {
    fn episode_of(&self, file: &Path, folder_season: Option<u32>) -> Option<(u32, u32, bool)> {
        let name = file.file_name()?.to_string_lossy().to_string();

        if let Some(ep) = parse::episode_number(&name) {
            return Some((folder_season.unwrap_or(ep.season), ep.episode, true));
        }

        let normalized = release::normalized_name(&name);

        let (rest, titled) = match release::after_title(&normalized, &release::normalize(&self.title)) {
            Some(r) => (r.to_string(), true),
            None => (normalized.clone(), false),
        };

        if release::is_recap(&name) {
            return None;
        }

        let numbers = release::numbers(&rest)?;
        let Match { episodes, batch, .. } = self.matcher.map(folder_season.map(Scope::season), numbers)?;

        if batch || episodes.len() != 1 {
            return None;
        }

        Some((episodes[0].0, episodes[0].1, titled))
    }

    async fn target(&self, state: &AppState, file: &Path, season: u32, episode: u32) -> anyhow::Result<PathBuf> {
        let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let stem = file.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        let title: Option<String> = match self.series_id {
            Some(id) => {
                sqlx::query_scalar("SELECT title FROM episodes WHERE series_id = ? AND season = ? AND episode = ?")
                    .bind(id)
                    .bind(season)
                    .bind(episode)
                    .fetch_optional(&state.db)
                    .await?
                    .flatten()
            },
            None => None,
        };

        let attrs = release::attributes(&name);

        let values = Values {
            show: self.title.clone(),
            year: self.year,
            season,
            episode,
            title,
            group: attrs.group,
            quality: attrs.resolution.map(|r| format!("{r}p")),
            codec: naming::CODEC.find(&name).map(|m| m.as_str().to_string()),
            original: stem,
        };

        let dir = if season == 0 { "Specials".to_string() } else { naming::render(&self.style.folder, &values) };
        let ext = file.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "mkv".into());
        Ok(self.path.join(dir).join(format!("{}.{ext}", naming::render(&self.style.file, &values))))
    }
}

fn videos_in(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && matches!(parse::file_kind(p), FileKind::Video))
        .filter(|p| !parse::is_sample(&p.file_stem().unwrap_or_default().to_string_lossy()))
        .collect();

    out.sort();
    out
}

async fn suggest(state: &AppState, library: &str) -> anyhow::Result<Vec<Suggestion>> {
    let config = state.config.current();
    let Some(lib) = config.library(library) else { return Ok(Vec::new()) };
    let Ok(root) = lib.resolved_path(state.config.config_dir()) else { return Ok(Vec::new()) };

    let skipped: Vec<(String, String)> =
        sqlx::query_as("SELECT path, reason FROM skipped WHERE library = ?").bind(library).fetch_all(&state.db).await?;

    let mut shows: HashMap<PathBuf, Show> = HashMap::new();
    let mut out = Vec::new();

    for (path, reason) in skipped {
        let path = PathBuf::from(path);

        let (show_dir, folder_season, files): (PathBuf, Option<u32>, Vec<PathBuf>) = if reason.starts_with("no SxxEyy")
        {
            let season_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
            let season = parse::season_number(&season_dir.file_name().unwrap_or_default().to_string_lossy());
            (season_dir.parent().map(Path::to_path_buf).unwrap_or_default(), season, vec![path.clone()])
        } else if reason.starts_with("video outside a season folder") {
            (path.parent().map(Path::to_path_buf).unwrap_or_default(), None, vec![path.clone()])
        } else if reason.starts_with("looks like a show") {
            (path.clone(), None, videos_in(&path))
        } else if reason.starts_with("folder isn't a season folder") {
            if let Some(c) = ORDINAL_SEASON.captures(&path.file_name().unwrap_or_default().to_string_lossy())
                && let Ok(n) = c[1].parse::<u32>()
                && let Some(parent) = path.parent()
            {
                let v = Values { season: n, ..Values::default() };
                let style = naming::library_style(&state.db, library).await?;
                let dst = parent.join(naming::render(&style.folder, &v));

                if !dst.exists() {
                    out.push(Suggestion {
                        src: path.clone(),
                        dst,
                        reason: format!("season {n} folder, named so the scanner recognizes it"),
                        confidence: "high",
                    });
                }
            }

            continue;
        } else {
            continue;
        };

        if !show_dir.starts_with(&root) || show_dir == root {
            continue;
        }

        if !shows.contains_key(&show_dir) {
            match show(state, library, &show_dir).await {
                Ok(s) => {
                    shows.insert(show_dir.clone(), s);
                },
                Err(e) => {
                    tracing::debug!("rename suggestions for {}: {e:#}", show_dir.display());
                    continue;
                },
            }
        }

        let s = &shows[&show_dir];

        for file in files {
            let Some((season, episode, titled)) = s.episode_of(&file, folder_season) else { continue };
            let dst = s.target(state, &file, season, episode).await?;

            if dst == file || dst.exists() {
                continue;
            }

            out.push(Suggestion {
                src: file,
                dst,
                reason: format!("S{season:02}E{episode:02}, named like the rest of {}", s.title),
                confidence: if titled { "high" } else { "low" },
            });
        }
    }

    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        "SELECT i.path, m.path, m.season, m.episode FROM media m JOIN items i ON i.id = m.item_id
         WHERE i.library = ? AND i.kind = 'show' AND m.season IS NOT NULL AND m.episode_end IS NULL",
    )
    .bind(library)
    .fetch_all(&state.db)
    .await?;

    let mut by_show: HashMap<String, Vec<(String, i64, i64)>> = HashMap::new();

    for (show_path, path, season, episode) in rows {
        by_show.entry(show_path).or_default().push((path, season, episode));
    }

    for (show_path, files) in by_show {
        let Some(style) = naming::show_style(&state.db, &show_path).await? else { continue };

        if style.samples < 4 || style.agreement.unwrap_or(0.0) < 0.6 || style.agreement == Some(1.0) {
            continue;
        }

        let show_dir = PathBuf::from(&show_path);

        if !shows.contains_key(&show_dir) {
            shows.insert(show_dir.clone(), show(state, library, &show_dir).await?);
        }

        let s = &shows[&show_dir];

        for (path, season, episode) in files {
            let file = PathBuf::from(&path);
            let stem = file.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let template = naming::template_of(&stem, &[s.title.clone()], s.year, None);

            if template.as_deref() == Some(style.file.as_str()) {
                continue;
            }

            let dst = s.target(state, &file, season as u32, episode as u32).await?;

            if dst == file || dst.exists() || dst.parent() != file.parent() {
                continue;
            }

            out.push(Suggestion {
                src: file,
                dst,
                reason: format!("{:.0}% of {} is named this way", style.agreement.unwrap_or(0.0) * 100.0, s.title),
                confidence: "low",
            });
        }
    }

    let anilist: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT path, title, provider_id FROM items
         WHERE library = ? AND kind = 'show' AND provider = 'anilist' AND provider_id IS NOT NULL",
    )
    .bind(library)
    .fetch_all(&state.db)
    .await?;

    // Only shows that share their first words with another are worth asking AniList about.
    let lead = |t: &str| release::normalize(t).split_whitespace().take(2).collect::<Vec<_>>().join(" ");

    for (path, title, provider_id) in &anilist {
        if !anilist.iter().any(|(p, t, _)| p != path && lead(t) == lead(title)) {
            continue;
        }

        let split = match split_of(state, library, provider_id).await {
            Ok(Some(split)) => split,
            Ok(None) => continue,
            Err(e) => {
                tracing::debug!("seasons of {title}: {e:#}");
                continue;
            },
        };

        if !shows.contains_key(&split.parent) {
            shows.insert(split.parent.clone(), show(state, library, &split.parent).await?);
        }

        let s = &shows[&split.parent];

        let files: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT m.path, m.season, m.episode FROM media m JOIN items i ON i.id = m.item_id
             WHERE i.path = ? AND m.season IS NOT NULL AND m.episode IS NOT NULL AND m.episode_end IS NULL",
        )
        .bind(path)
        .fetch_all(&state.db)
        .await?;

        for (file, season, episode) in files {
            let file = PathBuf::from(file);
            let (season, episode) = split.place(season, episode);
            let dst = s.target(state, &file, season, episode).await?;

            if dst.exists() {
                continue;
            }

            out.retain(|o| o.src != file);

            out.push(Suggestion {
                src: file,
                dst,
                reason: format!("S{season:02}E{episode:02}, {title} continues {}", s.title),
                confidence: "high",
            });
        }
    }

    Ok(out)
}

/// Folds each show whose files were all moved into the one it continues into that one's downloads
/// and monitoring.
async fn merge_splits(state: &Arc<AppState>, library: &str) -> anyhow::Result<()> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, path, provider_id FROM series WHERE library = ? AND provider = 'anilist' AND provider_id IS NOT NULL",
    )
    .bind(library)
    .fetch_all(&state.db)
    .await?;

    for (id, path, provider_id) in rows {
        if has_videos(Path::new(&path)) {
            continue;
        }

        let Some(split) = split_of(state, library, &provider_id).await? else { continue };
        let parent_path = split.parent.to_string_lossy().to_string();

        let parent = match series::by_path(state, &parent_path).await? {
            Some(row) => row,
            None => match series::item_id(state, &parent_path).await? {
                Some(item) => series::get(state, series::ensure_for_item(state, item).await?).await?,
                None => continue,
            },
        };

        let child = series::get(state, id).await?;

        let downloads: Vec<(i64, String)> = sqlx::query_as("SELECT id, episodes FROM downloads WHERE series_id = ?")
            .bind(id)
            .fetch_all(&state.db)
            .await?;

        for (download, episodes) in downloads {
            let episodes: Vec<(i64, i64)> = serde_json::from_str(&episodes).unwrap_or_default();
            let episodes: Vec<(u32, u32)> = episodes.into_iter().map(|(s, e)| split.place(s, e)).collect();

            sqlx::query("UPDATE downloads SET series_id = ?, episodes = ? WHERE id = ?")
                .bind(parent.id)
                .bind(serde_json::to_string(&episodes)?)
                .bind(download)
                .execute(&state.db)
                .await?;
        }

        sqlx::query("UPDATE requests SET series_id = ? WHERE series_id = ?")
            .bind(parent.id)
            .bind(id)
            .execute(&state.db)
            .await?;

        sqlx::query("UPDATE series SET profile = COALESCE(profile, ?) WHERE id = ?")
            .bind(&child.profile)
            .bind(parent.id)
            .execute(&state.db)
            .await?;

        sqlx::query("DELETE FROM series WHERE id = ?").bind(id).execute(&state.db).await?;
        state.events.send(Event::SeriesChanged { series_id: id });

        let monitor = match child.monitor.as_str() {
            "missing" => Monitor::Missing,
            "future" => Monitor::Future,
            _ => Monitor::None,
        };

        if monitor != Monitor::None && (parent.monitor == "none" || monitor == Monitor::Missing) {
            series::set_monitor(state, parent.id, monitor).await?;
        }

        let state = state.clone();

        tokio::spawn(async move {
            if let Err(e) = series::refresh_schedule(&state, parent.id).await {
                tracing::warn!("schedule for {}: {e:#}", parent.title);
            }
        });
    }

    Ok(())
}

fn has_videos(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .any(|p| if p.is_dir() { has_videos(&p) } else { matches!(parse::file_kind(&p), FileKind::Video) })
}

pub async fn refresh(state: &AppState, library: &str) -> anyhow::Result<()> {
    let found = suggest(state, library).await?;
    let mut tx = state.db.begin().await?;

    sqlx::query("DELETE FROM rename_suggestions WHERE library = ? AND state = 'pending'")
        .bind(library)
        .execute(&mut *tx)
        .await?;

    for s in &found {
        sqlx::query(
            "INSERT INTO rename_suggestions (library, src, dst, reason, confidence, created_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(src) DO UPDATE SET dst = excluded.dst, reason = excluded.reason, confidence = excluded.confidence,
                state = CASE WHEN rename_suggestions.state = 'applied' THEN 'pending' ELSE rename_suggestions.state END",
        )
        .bind(library)
        .bind(s.src.to_string_lossy().to_string())
        .bind(s.dst.to_string_lossy().to_string())
        .bind(&s.reason)
        .bind(s.confidence)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    state.events.send(Event::RenamesChanged);
    Ok(())
}

#[derive(Debug, Serialize, async_graphql::SimpleObject)]
#[graphql(name = "AppliedRenames")]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub batch: String,
    pub renamed: usize,
    pub problems: Vec<String>,
}

pub async fn apply(state: &Arc<AppState>, ids: &[i64]) -> anyhow::Result<Applied> {
    let config = state.config.current();
    let batch_id = format!("rename-{}", uuid::Uuid::new_v4());

    let batch = Batch::new(
        &state.db,
        &batch_id,
        format!("Renamed {} file{}", ids.len(), if ids.len() == 1 { "" } else { "s" }),
    );

    let mut result = Applied { batch: batch_id, renamed: 0, problems: Vec::new() };
    let mut libraries = Vec::new();

    for &id in ids {
        let Some((library, src, dst)): Option<(String, String, String)> =
            sqlx::query_as("SELECT library, src, dst FROM rename_suggestions WHERE id = ? AND state = 'pending'")
                .bind(id)
                .fetch_optional(&state.db)
                .await?
        else {
            continue;
        };

        if !config.library(&library).is_some_and(|l| l.managed) {
            result.problems.push(format!(
                "{library} isn't a managed library; turn on “Managed” to let tinystream rename files there"
            ));

            continue;
        }

        let (src, dst) = (PathBuf::from(src), PathBuf::from(dst));

        let outcome: anyhow::Result<()> = async {
            if !src.exists() {
                bail!("{} is gone", src.display());
            }

            let sidecars = if src.is_file() { sidecars(&src) } else { Vec::new() };
            batch.rename(&src, &dst).await?;
            let (old_stem, new_stem) = (stem(&src), stem(&dst));

            for side in sidecars {
                let name = side.file_name().unwrap_or_default().to_string_lossy().to_string();
                let new_name = format!("{new_stem}{}", &name[old_stem.len()..]);

                if let Some(dir) = dst.parent()
                    && let Err(e) = batch.rename(&side, &dir.join(new_name)).await
                {
                    tracing::debug!("{}: {e:#}", side.display());
                }
            }

            Ok(())
        }
        .await;

        match outcome {
            Ok(()) => {
                result.renamed += 1;

                sqlx::query("UPDATE rename_suggestions SET state = 'applied' WHERE id = ?")
                    .bind(id)
                    .execute(&state.db)
                    .await?;

                if !libraries.contains(&library) {
                    libraries.push(library);
                }
            },
            Err(e) => result.problems.push(format!("{e:#}")),
        }
    }

    for l in libraries {
        if let Err(e) = merge_splits(state, &l).await {
            result.problems.push(format!("{e:#}"));
        }

        state.scanner.request(&l);
    }

    state.events.send(Event::RenamesChanged);
    Ok(result)
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

fn sidecars(video: &Path) -> Vec<PathBuf> {
    let (Some(dir), s) = (video.parent(), stem(video)) else { return Vec::new() };

    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p != video && p.is_file())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().strip_prefix(&s).is_some_and(|rest| rest.starts_with('.')))
        })
        .collect()
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut rx = state.events.subscribe();

        loop {
            match rx.recv().await {
                Ok(Event::ScanFinished { library, .. }) => {
                    if !state.config.current().automation.rename_suggestions {
                        continue;
                    }
                    if let Err(e) = refresh(&state, &library).await {
                        tracing::warn!("rename suggestions for {library:?}: {e:#}");
                    }
                },
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {},
                Err(_) => break,
            }
        }
    });
}
