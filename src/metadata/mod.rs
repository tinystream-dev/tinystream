// SPDX-License-Identifier: AGPL-3.0-or-later

#![cfg_attr(not(feature = "metadata"), allow(dead_code, unused_imports, unused_variables))]

#[cfg(feature = "metadata")]
mod anilist;
#[cfg(feature = "metadata")]
mod tmdb;
#[cfg(feature = "metadata")]
mod tvmaze;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::config::Provider;
use crate::db::now;
use crate::events::Event;
use crate::library::parse::sort_title;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, async_graphql::Enum)]
#[serde(rename_all = "camelCase")]
pub enum MediaCategory {
    Episodes,
    Specials,
    Movies,
    Other,
}

impl MediaCategory {
    pub fn anilist(format: Option<&str>) -> Self {
        match format {
            Some("MOVIE") => Self::Movies,
            Some("OVA" | "SPECIAL") => Self::Specials,
            Some("TV" | "TV_SHORT" | "ONA") => Self::Episodes,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[graphql(name = "ProviderTitle")]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub category: MediaCategory,
    pub provider: Provider,
    pub id: String,
    #[graphql(name = "name")]
    pub title: String,

    pub romaji: Option<String>,
    pub year: Option<i64>,
    pub poster: Option<String>,
    pub overview: Option<String>,
}

#[derive(Debug, Default)]
pub struct Details {
    pub title: String,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub rating: Option<f64>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub aliases: Vec<String>,
    pub seasons: Vec<SeasonDetails>,
}

#[derive(Debug, Default)]
pub struct SeasonDetails {
    pub number: i64,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub poster: Option<String>,
    pub length: Option<i64>,
    pub provider_ids: Vec<String>,
    pub episodes: Vec<EpisodeDetails>,
}

#[derive(Debug, Default)]
pub struct EpisodeDetails {
    pub number: i64,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub still: Option<String>,
    pub air_date: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct ShowSchedule {
    pub status: Option<String>,
    pub aliases: Vec<String>,
    pub seasons: Vec<SeasonSchedule>,
}

#[derive(Debug, Default, Clone)]
pub struct SeasonSchedule {
    pub number: i64,
    pub provider_id: Option<String>,
    pub title: Option<String>,
    pub aliases: Vec<String>,
    pub episodes: Option<i64>,
    pub airing: Vec<ScheduledEpisode>,
    pub parts: Vec<PartSchedule>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PartSchedule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub offset: i64,
    pub episodes: Option<i64>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct ScheduledEpisode {
    pub number: i64,
    pub title: Option<String>,
    pub air_at: Option<i64>,
    pub aired: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Show,
    Movie,
}

pub struct MetadataService {
    wake: Notify,
    #[cfg(feature = "metadata")]
    anilist: anilist::Client,
    #[cfg(feature = "metadata")]
    tmdb: tmdb::Client,
    #[cfg(feature = "metadata")]
    tvmaze: tvmaze::Client,
    backoff: Mutex<HashMap<i64, Instant>>,
    recommended: Mutex<HashMap<(Provider, ItemKind, String), (Instant, Vec<Candidate>)>>,
}

const RECOMMENDED_FOR: Duration = Duration::from_secs(12 * 3600);

#[cfg(not(feature = "metadata"))]
fn unavailable<T>() -> anyhow::Result<T> {
    anyhow::bail!("this build of tinystream has no metadata providers (built without the `metadata` feature)")
}

#[cfg(not(feature = "metadata"))]
impl MetadataService {
    pub fn new(_http: reqwest::Client) -> Self {
        Self { wake: Notify::new(), backoff: Mutex::new(HashMap::new()), recommended: Mutex::new(HashMap::new()) }
    }

    pub async fn schedule(
        &self,
        _state: &AppState,
        _provider: Provider,
        _id: &str,
        _library: &BTreeMap<i64, i64>,
    ) -> anyhow::Result<ShowSchedule> {
        unavailable()
    }

    pub async fn first_season(&self, _provider: Provider, _id: &str) -> anyhow::Result<Option<Candidate>> {
        Ok(None)
    }

    pub async fn search(
        &self,
        _state: &AppState,
        _provider: Provider,
        _kind: ItemKind,
        _query: &str,
        _year: Option<i64>,
    ) -> anyhow::Result<Vec<Candidate>> {
        unavailable()
    }

    pub async fn trending(
        &self,
        _state: &AppState,
        _provider: Provider,
        _kind: ItemKind,
    ) -> anyhow::Result<Vec<Candidate>> {
        unavailable()
    }

    async fn fetch_recommendations(
        &self,
        _state: &AppState,
        _provider: Provider,
        _kind: ItemKind,
        _ids: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<Candidate>>> {
        unavailable()
    }

    async fn details(
        &self,
        _state: &AppState,
        _provider: Provider,
        _kind: ItemKind,
        _id: &str,
        _library: &BTreeMap<i64, i64>,
    ) -> anyhow::Result<Details> {
        unavailable()
    }
}

#[cfg(feature = "metadata")]
impl MetadataService {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            wake: Notify::new(),
            anilist: anilist::Client::new(http.clone()),
            tmdb: tmdb::Client::new(http.clone()),
            tvmaze: tvmaze::Client::new(http),
            backoff: Mutex::new(HashMap::new()),
            recommended: Mutex::new(HashMap::new()),
        }
    }

    pub async fn schedule(
        &self,
        state: &AppState,
        provider: Provider,
        id: &str,
        library: &BTreeMap<i64, i64>,
    ) -> anyhow::Result<ShowSchedule> {
        match provider {
            Provider::Anilist => self.anilist.schedule(id, library).await,
            Provider::Tmdb => {
                let config = state.config.current();
                let (mut schedule, external) = self.tmdb.schedule(&config.metadata, id).await?;

                match self.tvmaze.air_times(&external).await {
                    Ok(times) if !times.is_empty() => {
                        for season in &mut schedule.seasons {
                            for e in &mut season.airing {
                                if let Some(t) = times.get(&(season.number, e.number)) {
                                    e.air_at = Some(*t);
                                }
                            }
                        }
                    },
                    Ok(_) => {},
                    Err(e) => tracing::debug!("TVmaze doesn't know TMDB show {id}: {e:#}"),
                }

                Ok(schedule)
            },
        }
    }

    pub async fn first_season(&self, provider: Provider, id: &str) -> anyhow::Result<Option<Candidate>> {
        match provider {
            Provider::Anilist => Ok(Some(self.anilist.first_season(id).await?)),
            Provider::Tmdb => Ok(None),
        }
    }

    pub async fn search(
        &self,
        state: &AppState,
        provider: Provider,
        kind: ItemKind,
        query: &str,
        year: Option<i64>,
    ) -> anyhow::Result<Vec<Candidate>> {
        match provider {
            Provider::Anilist => self.anilist.search(kind, query).await,
            Provider::Tmdb => {
                let config = state.config.current();
                self.tmdb.search(&config.metadata, kind, query, year).await
            },
        }
    }

    pub async fn trending(
        &self,
        state: &AppState,
        provider: Provider,
        kind: ItemKind,
    ) -> anyhow::Result<Vec<Candidate>> {
        match provider {
            Provider::Anilist => self.anilist.trending(kind).await,
            Provider::Tmdb => {
                let config = state.config.current();
                self.tmdb.trending(&config.metadata, kind).await
            },
        }
    }

    async fn fetch_recommendations(
        &self,
        state: &AppState,
        provider: Provider,
        kind: ItemKind,
        ids: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<Candidate>>> {
        match provider {
            Provider::Anilist => self.anilist.recommendations(kind, ids).await,
            Provider::Tmdb => {
                let config = state.config.current();
                self.tmdb.recommendations(&config.metadata, kind, ids).await
            },
        }
    }

    async fn details(
        &self,
        state: &AppState,
        provider: Provider,
        kind: ItemKind,
        id: &str,
        library: &BTreeMap<i64, i64>,
    ) -> anyhow::Result<Details> {
        match provider {
            Provider::Anilist => {
                let mut details = self.anilist.details(kind, id, library).await?;

                if let Err(e) = self.fill_titles(&mut details, library).await {
                    tracing::debug!("TVmaze episode titles for AniList show {id}: {e:#}");
                }

                Ok(details)
            },
            Provider::Tmdb => {
                let config = state.config.current();
                let seasons: Vec<i64> = library.keys().copied().collect();
                self.tmdb.details(&config.metadata, kind, id, &seasons).await
            },
        }
    }

    async fn fill_titles(&self, details: &mut Details, library: &BTreeMap<i64, i64>) -> anyhow::Result<()> {
        let incomplete = |s: &SeasonDetails| {
            let have: Vec<i64> = s.episodes.iter().map(|e| e.number).collect();
            (1..=library.get(&s.number).copied().unwrap_or(0)).any(|n| !have.contains(&n))
        };

        if !details.seasons.iter().any(incomplete) {
            return Ok(());
        }

        let Some(year) = details.year else {
            return Ok(());
        };

        let Some(episodes) = self.tvmaze.episodes_by_title(&details.aliases, year).await? else {
            return Ok(());
        };

        let placeholder = |t: &str| t.strip_prefix("Episode ").is_some_and(|n| n.trim().parse::<i64>().is_ok());

        for season in details.seasons.iter_mut().filter(|s| incomplete(s)) {
            let theirs: BTreeMap<i64, &tvmaze::Episode> =
                episodes.iter().filter(|e| e.season == season.number).filter_map(|e| Some((e.number?, e))).collect();

            let Some(&last) = theirs.keys().last() else {
                continue;
            };

            let fits = match season.length {
                Some(length) => last == length,
                None => last >= library.get(&season.number).copied().unwrap_or(0),
            };

            if !fits {
                continue;
            }

            let mut ours: BTreeMap<i64, EpisodeDetails> = season.episodes.drain(..).map(|e| (e.number, e)).collect();

            for (&number, e) in &theirs {
                let Some(title) = e.name.clone().filter(|t| !t.trim().is_empty() && !placeholder(t)) else {
                    continue;
                };

                let old = ours.remove(&number).unwrap_or_default();
                let image = e.image.as_ref().and_then(|i| i.medium.clone().or_else(|| i.original.clone()));

                ours.insert(
                    number,
                    EpisodeDetails {
                        number,
                        title: Some(title),
                        overview: e.summary.as_deref().map(strip_html).filter(|s| !s.is_empty()).or(old.overview),
                        still: image.or(old.still),
                        air_date: e.airdate.clone().filter(|d| !d.is_empty()).or(old.air_date),
                    },
                );
            }

            season.episodes = ours.into_values().collect();
        }

        Ok(())
    }
}

impl MetadataService {
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    pub async fn recommendations(
        &self,
        state: &AppState,
        provider: Provider,
        kind: ItemKind,
        ids: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<Candidate>>> {
        let mut out = HashMap::new();
        let mut missing = Vec::new();

        {
            let cache = self.recommended.lock().unwrap();

            for id in ids {
                match cache.get(&(provider, kind, id.clone())) {
                    Some((at, list)) if at.elapsed() < RECOMMENDED_FOR => {
                        out.insert(id.clone(), list.clone());
                    },
                    _ => missing.push(id.clone()),
                }
            }
        }

        if missing.is_empty() {
            return Ok(out);
        }

        let fetched = self.fetch_recommendations(state, provider, kind, &missing).await?;
        let mut cache = self.recommended.lock().unwrap();
        cache.retain(|_, (at, _)| at.elapsed() < RECOMMENDED_FOR);

        for id in missing {
            let list = fetched.get(&id).cloned().unwrap_or_default();
            cache.insert((provider, kind, id.clone()), (Instant::now(), list.clone()));
            out.insert(id, list);
        }

        Ok(out)
    }

    pub async fn apply(
        &self,
        state: &AppState,
        item_id: i64,
        provider: Provider,
        provider_id: &str,
        manual: bool,
    ) -> anyhow::Result<()> {
        state.events.send(Event::MetadataFetching { item_id });
        let result = self.fetch_and_store(state, item_id, provider, provider_id, manual).await;

        state.events.send(match result {
            Ok(()) => Event::MetadataUpdated { item_id },
            Err(_) => Event::MetadataFailed { item_id },
        });

        result
    }

    async fn fetch_and_store(
        &self,
        state: &AppState,
        item_id: i64,
        provider: Provider,
        provider_id: &str,
        manual: bool,
    ) -> anyhow::Result<()> {
        let (kind,): (String,) =
            sqlx::query_as("SELECT kind FROM items WHERE id = ?").bind(item_id).fetch_one(&state.db).await?;

        let kind = if kind == "show" { ItemKind::Show } else { ItemKind::Movie };
        let library = library_seasons(&state.db, item_id).await?;
        let d = self.details(state, provider, kind, provider_id, &library).await?;

        let mut tx = state.db.begin().await?;

        sqlx::query(
            "UPDATE items SET title = ?, sort_title = ?, year = COALESCE(?, year), overview = ?, genres = ?,
                rating = ?, poster = ?, backdrop = ?, provider = ?, provider_id = ?, match_state = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&d.title)
        .bind(sort_title(&d.title))
        .bind(d.year)
        .bind(&d.overview)
        .bind(serde_json::to_string(&d.genres)?)
        .bind(d.rating)
        .bind(&d.poster)
        .bind(&d.backdrop)
        .bind(provider.as_str())
        .bind(provider_id)
        .bind(if manual { "manual" } else { "matched" })
        .bind(now())
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

        let numbers = serde_json::to_string(&d.seasons.iter().map(|s| s.number).collect::<Vec<_>>())?;

        sqlx::query("DELETE FROM seasons WHERE item_id = ? AND number NOT IN (SELECT value FROM json_each(?))")
            .bind(item_id)
            .bind(&numbers)
            .execute(&mut *tx)
            .await?;

        sqlx::query("UPDATE media SET title = NULL, overview = NULL, still = NULL, air_date = NULL WHERE item_id = ? AND season IS NOT NULL")
            .bind(item_id)
            .execute(&mut *tx)
            .await?;

        for s in &d.seasons {
            sqlx::query(
                "INSERT INTO seasons (item_id, number, title, overview, poster, provider_ids) VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(item_id, number) DO UPDATE SET title = excluded.title,
                    overview = excluded.overview, poster = excluded.poster, provider_ids = excluded.provider_ids",
            )
            .bind(item_id)
            .bind(s.number)
            .bind(&s.title)
            .bind(&s.overview)
            .bind(&s.poster)
            .bind(serde_json::to_string(&s.provider_ids)?)
            .execute(&mut *tx)
            .await?;

            for e in &s.episodes {
                sqlx::query(
                    "UPDATE media SET title = ?, overview = ?, still = ?, air_date = ?
                     WHERE item_id = ? AND season = ? AND episode = ?",
                )
                .bind(&e.title)
                .bind(&e.overview)
                .bind(&e.still)
                .bind(&e.air_date)
                .bind(item_id)
                .bind(s.number)
                .bind(e.number)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn auto_match(&self, state: &AppState, item: &PendingItem, provider: Provider) -> anyhow::Result<()> {
        let kind = if item.kind == "show" { ItemKind::Show } else { ItemKind::Movie };
        state.events.send(Event::MetadataFetching { item_id: item.id });

        let results = match self.search(state, provider, kind, &item.folder_title, item.folder_year).await {
            Ok(results) => results,
            Err(e) => {
                state.events.send(Event::MetadataFailed { item_id: item.id });
                return Err(e);
            },
        };

        let best = pick_best(&results, &item.folder_title, item.folder_year);

        match best {
            Some(c) => {
                tracing::info!("matched {:?} → {:?} ({} {})", item.folder_title, c.title, provider.as_str(), c.id);
                self.apply(state, item.id, provider, &c.id.clone(), false).await
            },
            None => {
                tracing::warn!(
                    "no {} match for {:?}; pick one by hand from its page in the UI",
                    provider.as_str(),
                    item.folder_title
                );

                sqlx::query("UPDATE items SET match_state = 'unmatched', provider = ? WHERE id = ?")
                    .bind(provider.as_str())
                    .bind(item.id)
                    .execute(&state.db)
                    .await?;

                state.events.send(Event::MetadataUpdated { item_id: item.id });
                Ok(())
            },
        }
    }
}

pub async fn library_seasons(db: &sqlx::SqlitePool, item_id: i64) -> anyhow::Result<BTreeMap<i64, i64>> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT season, MAX(COALESCE(episode_end, episode, 0)) FROM media WHERE item_id = ? AND season IS NOT NULL GROUP BY season",
    )
    .bind(item_id)
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().collect())
}

fn normalize(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn pick_best<'a>(results: &'a [Candidate], title: &str, year: Option<i64>) -> Option<&'a Candidate> {
    let want = normalize(title);
    let exact = |c: &&Candidate| normalize(&c.title) == want;
    let same_year = |c: &&Candidate| year.is_some() && c.year == year;

    results
        .iter()
        .find(|c| exact(c) && same_year(c))
        .or_else(|| results.iter().find(exact))
        .or_else(|| results.iter().find(same_year))
        .or_else(|| results.first())
}

#[derive(sqlx::FromRow)]
struct PendingItem {
    id: i64,
    library: String,
    kind: String,
    folder_title: String,
    folder_year: Option<i64>,
    match_state: String,
    provider: Option<String>,
}

pub fn spawn_worker(state: Arc<AppState>) {
    #[cfg(not(feature = "metadata"))]
    {
        let config = state.config.current();

        if config.libraries.iter().any(|l| l.metadata_provider.is_some()) {
            tracing::warn!(
                "libraries have a metadata-provider, but this build has no metadata providers; titles keep their folder names"
            );
        }

        return;
    }

    #[cfg(feature = "metadata")]
    tokio::spawn(async move {
        loop {
            if let Err(e) = run_once(&state).await {
                tracing::warn!("metadata: {e:#}");
            }
            tokio::select! {
                _ = state.metadata.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(3600)) => {}
            }
        }
    });
}

async fn run_once(state: &Arc<AppState>) -> anyhow::Result<()> {
    let config = state.config.current();
    let provider_of = |library: &str| config.library(library).and_then(|l| l.metadata_provider);

    let pending: Vec<PendingItem> = sqlx::query_as(
        "SELECT id, library, kind, folder_title, folder_year, match_state, provider FROM items
         WHERE match_state IN ('pending', 'unmatched') ORDER BY added_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    for item in pending {
        let Some(provider) = provider_of(&item.library) else {
            continue;
        };

        if item.match_state == "unmatched" && item.provider.as_deref() == Some(provider.as_str()) {
            continue;
        }

        if let Err(e) = state.metadata.auto_match(state, &item, provider).await {
            tracing::warn!("metadata for {:?}: {e:#}", item.folder_title);
        }
    }

    let stale: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT DISTINCT i.id, i.library, i.provider, i.provider_id FROM items i JOIN media m ON m.item_id = i.id
         WHERE i.match_state IN ('matched', 'manual') AND i.kind = 'show' AND m.title IS NULL AND m.episode IS NOT NULL AND i.provider_id IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await?;

    for (id, library, provider, provider_id) in stale {
        let Some(p) = provider_of(&library) else {
            continue;
        };

        if p.as_str() != provider {
            continue;
        }

        {
            let mut backoff = state.metadata.backoff.lock().unwrap();

            if backoff.get(&id).is_some_and(|t| t.elapsed() < Duration::from_secs(12 * 3600)) {
                continue;
            }

            backoff.insert(id, Instant::now());
        }

        if let Err(e) = state.metadata.apply(state, id, p, &provider_id, false).await {
            tracing::debug!("refreshing episodes of item {id}: {e:#}");
        }
    }

    Ok(())
}

#[cfg(feature = "metadata")]
pub(crate) fn strip_html(s: &str) -> String {
    let s = s.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;

    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {},
        }
    }

    let out = out
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");

    let mut collapsed = String::new();

    for line in out.lines() {
        let line = line.trim();

        if line.is_empty() && collapsed.ends_with("\n\n") {
            continue;
        }

        collapsed.push_str(line);
        collapsed.push('\n');
    }

    collapsed.trim().to_string()
}

#[cfg(test)]
mod category_tests {
    use super::MediaCategory;

    #[test]
    fn anilist_formats() {
        for format in ["TV", "TV_SHORT", "ONA"] {
            assert_eq!(MediaCategory::anilist(Some(format)), MediaCategory::Episodes);
        }

        for format in ["OVA", "SPECIAL"] {
            assert_eq!(MediaCategory::anilist(Some(format)), MediaCategory::Specials);
        }

        assert_eq!(MediaCategory::anilist(Some("MOVIE")), MediaCategory::Movies);
        assert_eq!(MediaCategory::anilist(Some("MUSIC")), MediaCategory::Other);
        assert_eq!(MediaCategory::anilist(None), MediaCategory::Other);
    }
}
