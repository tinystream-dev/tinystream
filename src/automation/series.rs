// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use super::matching::Numbering;
use super::naming::{self, Style};
use crate::config::{Monitor, Provider, Seeding};
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Row {
    pub id: i64,
    pub library: String,
    pub path: String,
    pub title: String,
    pub year: Option<i64>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub overview: Option<String>,
    pub monitor: String,
    pub profile: Option<String>,
    pub sources: String,
    pub groups: String,
    pub aliases: String,
    pub known_as: String,
    pub numbering: String,
    pub naming: Option<String>,
    pub seeding: Option<String>,
    pub status: Option<String>,
    pub monitored_at: Option<i64>,
    pub schedule_at: Option<i64>,
    pub added_at: i64,
}

pub fn list(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

pub async fn get(state: &AppState, id: i64) -> anyhow::Result<Row> {
    sqlx::query_as("SELECT * FROM series WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .context("no such show")
}

pub async fn by_path(state: &AppState, path: &str) -> anyhow::Result<Option<Row>> {
    Ok(sqlx::query_as("SELECT * FROM series WHERE path = ?").bind(path).fetch_optional(&state.db).await?)
}

pub async fn item_id(state: &AppState, path: &str) -> anyhow::Result<Option<i64>> {
    Ok(sqlx::query_scalar("SELECT id FROM items WHERE path = ?").bind(path).fetch_optional(&state.db).await?)
}

pub async fn ensure_for_item(state: &Arc<AppState>, item_id: i64) -> anyhow::Result<i64> {
    let (library, kind, path, title, year, provider, provider_id, poster, backdrop, overview): (
        String,
        String,
        String,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT library, kind, path, title, year, provider, provider_id, poster, backdrop, overview FROM items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_optional(&state.db)
    .await?
    .context("no such title")?;

    if kind != "show" {
        bail!("only shows can be managed for now");
    }

    if let Some(row) = by_path(state, &path).await? {
        sqlx::query("UPDATE series SET title = ?, year = ?, provider = ?, provider_id = ?, poster = ?, backdrop = ?, overview = ?, library = ? WHERE id = ?")
            .bind(&title)
            .bind(year)
            .bind(&provider)
            .bind(&provider_id)
            .bind(&poster)
            .bind(&backdrop)
            .bind(&overview)
            .bind(&library)
            .bind(row.id)
            .execute(&state.db)
            .await?;

        return Ok(row.id);
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO series (library, path, title, year, provider, provider_id, poster, backdrop, overview, added_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&library)
    .bind(&path)
    .bind(&title)
    .bind(year)
    .bind(&provider)
    .bind(&provider_id)
    .bind(&poster)
    .bind(&backdrop)
    .bind(&overview)
    .bind(now())
    .fetch_one(&state.db)
    .await?;

    Ok(id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSeries {
    pub library: String,
    pub provider: Provider,
    pub provider_id: String,
    pub title: String,
    pub year: Option<i64>,
    pub poster: Option<String>,
    pub overview: Option<String>,
    pub monitor: Option<Monitor>,
    pub profile: Option<String>,
}

pub async fn create(state: &Arc<AppState>, mut new: NewSeries) -> anyhow::Result<i64> {
    let config = state.config.current();
    let library = config.library(&new.library).context("no such library")?;
    let root = library.resolved_path(state.config.config_dir())?;
    let mut found = find(state, &new).await?;
    let mut stale = false;

    if found.is_none() {
        match state.metadata.first_season(new.provider, &new.provider_id).await {
            Ok(Some(first)) if first.id != new.provider_id => {
                new.provider_id = first.id;
                new.title = first.title;
                new.year = first.year;
                new.poster = first.poster.or(new.poster);
                new.overview = first.overview;
                found = find(state, &new).await?;
                stale = found.is_some();
            },
            Ok(_) => {},
            Err(e) => tracing::warn!("first season of {}: {e:#}", new.title),
        }
    }

    let id = if let Some(id) = found {
        id
    } else {
        let years = naming::library_uses_years(&state.db, &new.library).await?;
        let mut folder = naming::sanitize(&new.title);

        if years && let Some(y) = new.year {
            folder = format!("{folder} ({y})");
        }

        let path = unique_path(&root.join(folder.trim()));

        sqlx::query_scalar(
            "INSERT INTO series (library, path, title, year, provider, provider_id, poster, overview, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(&new.library)
        .bind(path.to_string_lossy().to_string())
        .bind(&new.title)
        .bind(new.year)
        .bind(new.provider.as_str())
        .bind(&new.provider_id)
        .bind(&new.poster)
        .bind(&new.overview)
        .bind(now())
        .fetch_one(&state.db)
        .await?
    };

    let monitor = new.monitor.unwrap_or(config.automation.default_monitor);

    sqlx::query("UPDATE series SET profile = COALESCE(?, profile) WHERE id = ?")
        .bind(&new.profile)
        .bind(id)
        .execute(&state.db)
        .await?;

    set_monitor(state, id, monitor).await?;

    // The show is here but its schedule predates the season that was asked for.
    if stale {
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(e) = refresh_schedule(&state, id).await {
                tracing::warn!("schedule for series {id}: {e:#}");
            }
        });
    }

    Ok(id)
}

/// The show `new` belongs to in its library, if it's already there, by its own id or any of its
/// seasons'.
async fn find(state: &Arc<AppState>, new: &NewSeries) -> anyhow::Result<Option<i64>> {
    let item: Option<i64> = sqlx::query_scalar(
        "SELECT i.id FROM items i WHERE i.provider = ?1 AND i.library = ?3 AND i.kind = 'show' AND (i.provider_id = ?2
            OR EXISTS (SELECT 1 FROM seasons s, json_each(s.provider_ids) WHERE s.item_id = i.id AND value = ?2))
         LIMIT 1",
    )
    .bind(new.provider.as_str())
    .bind(&new.provider_id)
    .bind(&new.library)
    .fetch_optional(&state.db)
    .await?;

    if let Some(item) = item {
        return Ok(Some(ensure_for_item(state, item).await?));
    }

    Ok(sqlx::query_scalar(
        "SELECT s.id FROM series s WHERE s.provider = ?1 AND s.library = ?3 AND (s.provider_id = ?2 OR EXISTS (
            SELECT 1 FROM series_seasons ss WHERE ss.series_id = s.id AND (ss.provider_id = ?2
                OR EXISTS (SELECT 1 FROM json_each(ss.parts) WHERE json_extract(value, '$.provider_id') = ?2))))
         LIMIT 1",
    )
    .bind(new.provider.as_str())
    .bind(&new.provider_id)
    .bind(&new.library)
    .fetch_optional(&state.db)
    .await?)
}

fn unique_path(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    (2..).map(|n| PathBuf::from(format!("{} {n}", p.display()))).find(|c| !c.exists()).unwrap()
}

pub async fn set_monitor(state: &Arc<AppState>, id: i64, monitor: Monitor) -> anyhow::Result<()> {
    let row = get(state, id).await?;

    if monitor != Monitor::None && row.provider_id.is_none() {
        bail!("this show isn't matched to AniList or TMDB yet, so there's no schedule to follow; fix its match first");
    }

    let turning_on = row.monitor == "none" && monitor != Monitor::None;

    sqlx::query("UPDATE series SET monitor = ?, monitored_at = CASE WHEN ? THEN ? ELSE monitored_at END WHERE id = ?")
        .bind(monitor.as_str())
        .bind(turning_on)
        .bind(now())
        .bind(id)
        .execute(&state.db)
        .await?;

    if monitor == Monitor::None {
        sqlx::query("UPDATE episodes SET state = 'idle', next_search = NULL WHERE series_id = ? AND state IN ('wanted', 'missing')")
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    if turning_on && row.schedule_at.is_none() {
        refresh_schedule(state, id).await?;
    }

    state.automation.wake();
    state.events.send(Event::SeriesChanged { series_id: id });
    Ok(())
}

pub async fn refresh_schedule(state: &Arc<AppState>, id: i64) -> anyhow::Result<()> {
    let row = get(state, id).await?;

    let (Some(provider), Some(provider_id)) = (row.provider.as_deref(), row.provider_id.as_deref()) else {
        bail!("{} isn't matched to AniList or TMDB", row.title);
    };

    let provider = match provider {
        "anilist" => Provider::Anilist,
        _ => Provider::Tmdb,
    };

    let library = match item_id(state, &row.path).await? {
        Some(item) => crate::metadata::library_seasons(&state.db, item).await?,
        None => Default::default(),
    };

    let schedule = state.metadata.schedule(state, provider, provider_id, &library).await?;
    let mut tx = state.db.begin().await?;

    sqlx::query("UPDATE series SET status = ?, known_as = ?, schedule_at = ? WHERE id = ?")
        .bind(&schedule.status)
        .bind(serde_json::to_string(&schedule.aliases)?)
        .bind(now())
        .bind(id)
        .execute(&mut *tx)
        .await?;

    let mut before: Option<i64> = Some(0);

    for s in &schedule.seasons {
        sqlx::query(
            "INSERT INTO series_seasons (series_id, season, provider_id, title, aliases, episodes, parts) VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(series_id, season) DO UPDATE SET provider_id = excluded.provider_id, title = excluded.title,
                aliases = excluded.aliases, episodes = excluded.episodes, parts = excluded.parts",
        )
        .bind(id)
        .bind(s.number)
        .bind(&s.provider_id)
        .bind(&s.title)
        .bind(serde_json::to_string(&s.aliases)?)
        .bind(s.episodes)
        .bind(serde_json::to_string(&s.parts)?)
        .execute(&mut *tx)
        .await?;

        for e in &s.airing {
            let absolute = if s.number > 0 { before.map(|b| b + e.number) } else { None };

            sqlx::query(
                "INSERT INTO episodes (series_id, season, episode, absolute, title, air_at, aired) VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(series_id, season, episode) DO UPDATE SET absolute = excluded.absolute,
                    title = COALESCE(excluded.title, episodes.title), air_at = excluded.air_at, aired = excluded.aired",
            )
            .bind(id)
            .bind(s.number)
            .bind(e.number)
            .bind(absolute)
            .bind(&e.title)
            .bind(e.air_at)
            .bind(e.aired)
            .execute(&mut *tx)
            .await?;
        }

        if s.number > 0 {
            before = match (before, s.episodes) {
                (Some(b), Some(n)) => Some(b + n),
                _ => None,
            };
        }
    }

    if !schedule.seasons.is_empty() {
        let seasons: Vec<i64> = schedule.seasons.iter().map(|s| s.number).collect();

        let episodes: Vec<(i64, i64)> =
            schedule.seasons.iter().flat_map(|s| s.airing.iter().map(|e| (s.number, e.number))).collect();

        sqlx::query("DELETE FROM series_seasons WHERE series_id = ? AND season > 0 AND season NOT IN (SELECT value FROM json_each(?))")
            .bind(id)
            .bind(serde_json::to_string(&seasons)?)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "DELETE FROM episodes WHERE series_id = ? AND season > 0 AND NOT EXISTS (SELECT 1 FROM json_each(?) j
                WHERE json_extract(j.value, '$[0]') = episodes.season AND json_extract(j.value, '$[1]') = episodes.episode)",
        )
        .bind(id)
        .bind(serde_json::to_string(&episodes)?)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    state.events.send(Event::SeriesChanged { series_id: id });
    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeView {
    pub season: i64,
    pub episode: i64,
    pub absolute: Option<i64>,
    pub title: Option<String>,
    pub air_at: Option<i64>,
    pub aired: bool,
    pub state: String,
    pub attempts: i64,
    pub searched_at: Option<i64>,
    pub next_search: Option<i64>,
    pub download_id: Option<i64>,

    pub media_id: Option<i64>,
}

pub async fn episodes(state: &AppState, row: &Row) -> anyhow::Result<Vec<EpisodeView>> {
    Ok(sqlx::query_as(
        "SELECT e.season, e.episode, e.absolute, e.title, e.air_at, e.aired, e.state, e.attempts, e.searched_at,
                e.next_search, e.download_id,
                (SELECT m.id FROM media m JOIN items i ON i.id = m.item_id WHERE i.path = ?2 AND m.season = e.season
                   AND e.episode BETWEEN m.episode AND COALESCE(m.episode_end, m.episode) LIMIT 1) AS media_id
         FROM episodes e WHERE e.series_id = ?1 ORDER BY e.season, e.episode",
    )
    .bind(row.id)
    .bind(&row.path)
    .fetch_all(&state.db)
    .await?)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub id: i64,
    pub item_id: Option<i64>,
    pub library: String,
    pub managed: bool,
    pub path: String,
    pub title: String,
    pub year: Option<i64>,
    pub poster: Option<String>,
    pub overview: Option<String>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub monitor: String,
    pub profile: Option<String>,
    pub effective_profile: String,
    pub sources: Vec<String>,
    pub groups: Vec<String>,
    pub aliases: Vec<String>,
    pub known_as: Vec<String>,
    pub numbering: Numbering,
    pub naming: Option<String>,
    pub style: Style,
    pub seeding: Option<Seeding>,
    pub status: Option<String>,
    pub schedule_at: Option<i64>,
    pub added_at: i64,
    pub counts: Counts,
    pub next: Option<EpisodeView>,
}

#[derive(Debug, Default, Serialize, async_graphql::SimpleObject)]
#[graphql(name = "EpisodeCounts")]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub have: i64,
    pub wanted: i64,
    pub missing: i64,
    pub grabbed: i64,

    pub total: i64,

    pub upcoming: i64,
    pub skipped: i64,
}

pub async fn style(state: &AppState, row: &Row) -> anyhow::Result<Style> {
    let inferred = match naming::show_style(&state.db, &row.path).await? {
        Some(s) => s,
        None => naming::library_style(&state.db, &row.library).await?,
    };

    Ok(match &row.naming {
        Some(t) => Style { file: t.clone(), agreement: None, ..inferred },
        None => inferred,
    })
}

pub async fn view(state: &AppState, row: Row) -> anyhow::Result<View> {
    let config = state.config.current();
    let item_id = item_id(state, &row.path).await?;
    let library = config.library(&row.library);

    let counts: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(state = 'done'), 0), COALESCE(SUM(state = 'wanted' AND aired), 0), COALESCE(SUM(state = 'missing'), 0),
                COALESCE(SUM(state = 'grabbed'), 0), COALESCE(SUM(aired OR state IN ('done', 'grabbed')), 0),
                COALESCE(SUM(state = 'wanted' AND NOT aired), 0), COALESCE(SUM(state = 'skipped'), 0)
         FROM (SELECT state, (aired = 1 OR COALESCE(air_at <= ?, 0)) AS aired FROM episodes WHERE series_id = ? AND season > 0)",
    )
    .bind(now())
    .bind(row.id)
    .fetch_one(&state.db)
    .await?;

    let next: Option<EpisodeView> = sqlx::query_as(
        "SELECT season, episode, absolute, title, air_at, aired, state, attempts, searched_at, next_search, download_id, NULL AS media_id
         FROM episodes WHERE series_id = ? AND air_at > ? AND season > 0 ORDER BY air_at LIMIT 1",
    )
    .bind(row.id)
    .bind(now() - 3600)
    .fetch_optional(&state.db)
    .await?;

    let style = style(state, &row).await?;

    let poster = match item_id {
        Some(id) => Some(format!("/api/images/item/{id}/poster")),
        None => row.poster.clone(),
    };

    Ok(View {
        id: row.id,
        item_id,
        managed: library.is_some_and(|l| l.managed),
        effective_profile: row
            .profile
            .clone()
            .or_else(|| library.and_then(|l| l.profile.clone()))
            .unwrap_or_else(|| config.profiles.first().map(|p| p.name.clone()).unwrap_or_else(|| "Any".into())),
        library: row.library,
        path: row.path,
        title: row.title,
        year: row.year,
        poster,
        overview: row.overview,
        provider: row.provider,
        provider_id: row.provider_id,
        monitor: row.monitor,
        profile: row.profile,
        sources: list(&row.sources),
        groups: list(&row.groups),
        aliases: list(&row.aliases),
        known_as: list(&row.known_as),
        numbering: Numbering::parse(&row.numbering),
        naming: row.naming,
        style,
        seeding: row.seeding.as_deref().and_then(|s| serde_json::from_str(s).ok()),
        status: row.status,
        schedule_at: row.schedule_at,
        added_at: row.added_at,
        counts: Counts {
            have: counts.0,
            wanted: counts.1,
            missing: counts.2,
            grabbed: counts.3,
            total: counts.4,
            upcoming: counts.5,
            skipped: counts.6,
        },
        next,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    pub monitor: Option<Monitor>,
    #[serde(default, with = "double_option")]
    pub profile: Option<Option<String>>,
    pub sources: Option<Vec<String>>,
    pub groups: Option<Vec<String>>,
    pub aliases: Option<Vec<String>>,
    pub numbering: Option<Numbering>,
    #[serde(default, with = "double_option")]
    pub naming: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub seeding: Option<Option<Seeding>>,
}

mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

impl<'de> Deserialize<'de> for Numbering {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Numbering::parse(&String::deserialize(d)?))
    }
}

pub async fn patch(state: &Arc<AppState>, id: i64, p: Patch) -> anyhow::Result<()> {
    let config = state.config.current();

    let clean = |v: Vec<String>| -> String {
        let v: Vec<String> = v.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        serde_json::to_string(&v).unwrap()
    };

    if let Some(Some(profile)) = &p.profile
        && config.profile(profile).is_none()
    {
        bail!("there's no profile called {profile:?}");
    }

    if let Some(sources) = &p.sources
        && let Some(bad) = sources.iter().find(|s| config.source(s).is_none())
    {
        bail!("there's no source called {bad:?}");
    }

    if let Some(Some(t)) = &p.naming {
        naming::validate(t).map_err(|e| anyhow::anyhow!(e))?;
    }

    let mut tx = state.db.begin().await?;

    if let Some(profile) = p.profile {
        sqlx::query("UPDATE series SET profile = ? WHERE id = ?").bind(profile).bind(id).execute(&mut *tx).await?;
    }

    if let Some(v) = p.sources {
        sqlx::query("UPDATE series SET sources = ? WHERE id = ?").bind(clean(v)).bind(id).execute(&mut *tx).await?;
    }

    if let Some(v) = p.groups {
        sqlx::query("UPDATE series SET groups = ? WHERE id = ?").bind(clean(v)).bind(id).execute(&mut *tx).await?;
    }

    if let Some(v) = p.aliases {
        sqlx::query("UPDATE series SET aliases = ? WHERE id = ?").bind(clean(v)).bind(id).execute(&mut *tx).await?;
    }

    if let Some(n) = p.numbering {
        let s = match n {
            Numbering::Auto => "auto",
            Numbering::Seasonal => "seasonal",
            Numbering::Absolute => "absolute",
        };

        sqlx::query("UPDATE series SET numbering = ? WHERE id = ?").bind(s).bind(id).execute(&mut *tx).await?;
    }

    if let Some(naming) = p.naming {
        let naming = naming.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        sqlx::query("UPDATE series SET naming = ? WHERE id = ?").bind(naming).bind(id).execute(&mut *tx).await?;
    }

    if let Some(seeding) = p.seeding {
        let json = seeding.map(|s| serde_json::to_string(&s)).transpose()?;
        sqlx::query("UPDATE series SET seeding = ? WHERE id = ?").bind(json).bind(id).execute(&mut *tx).await?;
    }

    tx.commit().await?;

    if let Some(m) = p.monitor {
        set_monitor(state, id, m).await?;
    }

    state.events.send(Event::SeriesChanged { series_id: id });
    Ok(())
}
