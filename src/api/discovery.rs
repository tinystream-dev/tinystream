// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::{Context, Enum, Object, SimpleObject};

use super::library::{PopularTitle, Title, parse_provider};
use super::schema::{Access, Ctx};
use crate::auth::User;
use crate::config::{Monitor, Provider};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::metadata::{Candidate, ItemKind, MediaCategory};
use crate::state::AppState;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestState {
    Pending,
    Approved,
    Declined,
}

impl RequestState {
    pub fn parse(s: &str) -> Self {
        match s {
            "approved" => RequestState::Approved,
            "declined" => RequestState::Declined,
            _ => RequestState::Pending,
        }
    }
}

pub(super) fn parse_monitor(s: &str) -> Monitor {
    match s {
        "future" => Monitor::Future,
        "missing" => Monitor::Missing,
        _ => Monitor::None,
    }
}

#[derive(SimpleObject)]
#[graphql(name = "DiscoverResult")]
pub(super) struct Found {
    #[graphql(flatten)]
    pub candidate: Candidate,

    pub library: String,

    pub title_id: Option<i64>,

    pub series_id: Option<i64>,
    pub monitor: Option<Monitor>,

    pub request_state: Option<RequestState>,

    pub because: Option<String>,
}

impl Found {
    pub fn here(&self) -> bool {
        self.title_id.is_some() || self.series_id.is_some()
    }
}

pub(super) async fn annotate(
    state: &AppState,
    provider: Provider,
    library: &str,
    candidates: Vec<Candidate>,
) -> ApiResult<Vec<Found>> {
    let mut out = Vec::with_capacity(candidates.len());

    for c in candidates {
        let movie = c.category == MediaCategory::Movies;

        let managed: Option<(i64, String, Option<i64>)> = if movie {
            None
        } else {
            sqlx::query_as(
                "SELECT s.id, s.monitor, i.id FROM series s LEFT JOIN items i ON i.path = s.path
             WHERE s.provider = ?1 AND (s.provider_id = ?2 OR EXISTS (
                 SELECT 1 FROM series_seasons ss WHERE ss.series_id = s.id AND (ss.provider_id = ?2
                     OR EXISTS (SELECT 1 FROM json_each(ss.parts) WHERE json_extract(value, '$.provider_id') = ?2))))
             LIMIT 1",
            )
            .bind(provider.as_str())
            .bind(&c.id)
            .fetch_optional(&state.db)
            .await?
        };

        let item_id: Option<i64> =
            match managed.as_ref().and_then(|m| m.2) {
                Some(id) => Some(id),
                None => sqlx::query_scalar(
                    "SELECT i.id FROM items i WHERE i.provider = ?1 AND i.kind = ?3 AND (i.provider_id = ?2 OR EXISTS (
                     SELECT 1 FROM seasons s, json_each(s.provider_ids) WHERE s.item_id = i.id AND value = ?2))
                 LIMIT 1",
                )
                .bind(provider.as_str())
                .bind(&c.id)
                .bind(if movie { "movie" } else { "show" })
                .fetch_optional(&state.db)
                .await?,
            };

        let request_state: Option<String> = if movie {
            None
        } else {
            sqlx::query_scalar(
            "SELECT state FROM requests WHERE provider = ? AND provider_id = ? AND state != 'declined' ORDER BY id DESC LIMIT 1",
        )
        .bind(provider.as_str())
        .bind(&c.id)
        .fetch_optional(&state.db)
        .await?
        };

        out.push(Found {
            candidate: c,
            library: library.to_string(),
            title_id: item_id,
            series_id: managed.as_ref().map(|m| m.0),
            monitor: managed.map(|m| parse_monitor(&m.1)),
            request_state: request_state.as_deref().map(RequestState::parse),
            because: None,
        });
    }

    Ok(out)
}

pub(super) async fn pick_library(
    state: &AppState,
    user: &User,
    library: Option<String>,
) -> ApiResult<(String, Provider)> {
    if !user.permissions.request && !user.permissions.manage_shows {
        return Err(ApiError::forbidden());
    }

    let config = state.config.current();
    let libraries = user.libraries(state).await?;

    let library = library
        .or_else(|| {
            libraries.iter().find(|l| config.library(l).is_some_and(|x| x.metadata_provider.is_some())).cloned()
        })
        .ok_or_else(|| ApiError::bad_request("no library uses AniList or TMDB"))?;

    if !libraries.contains(&library) {
        return Err(ApiError::not_found("library"));
    }

    let provider =
        config.library(&library).ok_or_else(|| ApiError::not_found("library"))?.metadata_provider.ok_or_else(|| {
            ApiError::bad_request(format!("{library} doesn't use AniList or TMDB, so there's nothing to search"))
        })?;

    Ok((library, provider))
}

struct Seed {
    id: String,
    title: String,
    weight: f64,
}

#[derive(Clone, Copy)]
enum Whose {
    Mine(i64),

    Others(i64),
}

fn recency(at: i64) -> f64 {
    let days = (now() - at).max(0) as f64 / 86400.0;
    (-days / 120.0).exp()
}

async fn taste(state: &AppState, libraries: &[String], provider: Provider, whose: Whose) -> ApiResult<Vec<Seed>> {
    let (user, others) = match whose {
        Whose::Mine(u) => (u, false),
        Whose::Others(u) => (u, true),
    };

    let rows: Vec<(String, String, String, i64, f64, i64)> = sqlx::query_as(
        "SELECT i.provider_id, i.title, i.library, p.user_id,
                CASE WHEN p.finished THEN 1.0 ELSE MIN(p.position / MAX(p.duration, 1.0), 1.0) END, p.updated_at
         FROM progress p JOIN media m ON m.path = p.media_path JOIN items i ON i.id = m.item_id
         WHERE i.kind = 'show' AND i.provider = ?1 AND i.provider_id IS NOT NULL AND (p.user_id != ?2) = ?3",
    )
    .bind(provider.as_str())
    .bind(user)
    .bind(others)
    .fetch_all(&state.db)
    .await?;

    let mut watched: HashMap<(String, i64), f64> = HashMap::new();
    let mut titles: HashMap<String, String> = HashMap::new();

    for (id, title, library, person, amount, at) in rows {
        if !libraries.contains(&library) {
            continue;
        }

        *watched.entry((id.clone(), person)).or_default() += amount * recency(at);
        titles.entry(id).or_insert(title);
    }

    let mut weights: HashMap<String, f64> = HashMap::new();

    for ((id, _), amount) in watched {
        *weights.entry(id).or_default() += if others { (amount / 3.0).min(1.0) } else { amount.sqrt() };
    }

    if !others {
        let asked: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT provider_id, title, created_at FROM requests WHERE user_id = ? AND provider = ? AND state != 'declined'",
        )
        .bind(user)
        .bind(provider.as_str())
        .fetch_all(&state.db)
        .await?;

        for (id, title, at) in asked {
            *weights.entry(id.clone()).or_default() += 1.5 * recency(at);
            titles.entry(id).or_insert(title);
        }
    }

    let mut seeds: Vec<Seed> = weights
        .into_iter()
        .filter(|(_, w)| *w > 0.05)
        .map(|(id, weight)| Seed { title: titles.remove(&id).unwrap_or_default(), id, weight })
        .collect();

    seeds.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    seeds.truncate(10);
    Ok(seeds)
}

fn blend(seeds: &[Seed], recs: &HashMap<String, Vec<Candidate>>) -> Vec<(Candidate, String)> {
    let own: HashSet<&str> = seeds.iter().map(|s| s.id.as_str()).collect();
    let mut scored: HashMap<String, (f64, f64, String, Candidate)> = HashMap::new();

    for seed in seeds {
        for (rank, c) in recs.get(&seed.id).into_iter().flatten().enumerate() {
            if own.contains(c.id.as_str()) {
                continue;
            }

            let add = seed.weight / (1.0 + rank as f64 * 0.25);
            let e = scored.entry(c.id.clone()).or_insert_with(|| (0.0, 0.0, seed.title.clone(), c.clone()));
            e.0 += add;

            if add > e.1 {
                e.1 = add;
                e.2 = seed.title.clone();
            }
        }
    }

    let mut list: Vec<_> = scored.into_values().collect();
    list.sort_by(|a, b| b.0.total_cmp(&a.0));
    list.into_iter().map(|(_, _, because, c)| (c, because)).collect()
}

async fn fresh(
    state: &AppState,
    provider: Provider,
    library: &str,
    candidates: Vec<Candidate>,
    shown: &mut HashSet<String>,
    limit: usize,
) -> ApiResult<Vec<Found>> {
    let candidates: Vec<Candidate> =
        candidates.into_iter().filter(|c| !shown.contains(&c.id)).take(limit * 2).collect();

    let mut found: Vec<Found> =
        annotate(state, provider, library, candidates).await?.into_iter().filter(|f| !f.here()).collect();

    found.truncate(limit);
    shown.extend(found.iter().map(|f| f.candidate.id.clone()));
    Ok(found)
}

const ROW: usize = 20;

#[derive(SimpleObject)]
pub struct ForYou {
    library: String,
    shelves: Vec<ForYouShelf>,
}

#[derive(SimpleObject)]
pub struct ForYouShelf {
    key: String,
    name: String,
    results: Vec<Found>,
}

async fn for_you(state: &AppState, user: &User, library: Option<String>) -> ApiResult<ForYou> {
    let (library, provider) = pick_library(state, user, library).await?;
    let libraries = user.libraries(state).await?;
    let mine = taste(state, &libraries, provider, Whose::Mine(user.id)).await?;
    let others = taste(state, &libraries, provider, Whose::Others(user.id)).await?;

    let ids: Vec<String> =
        mine.iter().chain(&others).map(|s| s.id.clone()).collect::<HashSet<_>>().into_iter().collect();

    if ids.is_empty() {
        return Ok(ForYou { library, shelves: Vec::new() });
    }

    let recs = match state.metadata.recommendations(state, provider, ItemKind::Show, &ids).await {
        Ok(recs) => recs,
        Err(e) => {
            tracing::debug!("recommendations: {e:#}");
            return Ok(ForYou { library, shelves: Vec::new() });
        },
    };

    let mut shown = HashSet::new();
    let mut shelves = Vec::new();

    if !mine.is_empty() {
        let blended = blend(&mine, &recs);
        let because: HashMap<String, String> = blended.iter().map(|(c, b)| (c.id.clone(), b.clone())).collect();

        let mut found =
            fresh(state, provider, &library, blended.into_iter().map(|(c, _)| c).collect(), &mut shown, ROW).await?;

        for f in &mut found {
            f.because = because.get(&f.candidate.id).cloned();
        }

        if !found.is_empty() {
            shelves.push(ForYouShelf { key: "mine".into(), name: "For you".into(), results: found });
        }

        for seed in mine.iter().take(2) {
            let list = recs.get(&seed.id).cloned().unwrap_or_default();
            let found = fresh(state, provider, &library, list, &mut shown, ROW).await?;

            if found.len() >= 4 {
                shelves.push(ForYouShelf {
                    key: format!("seed-{}", seed.id),
                    name: format!("Because you watched {}", seed.title),
                    results: found,
                });
            }
        }
    }

    if !others.is_empty() {
        let list = blend(&others, &recs).into_iter().map(|(c, _)| c).collect();
        let found = fresh(state, provider, &library, list, &mut shown, ROW).await?;

        if !found.is_empty() {
            shelves.push(ForYouShelf {
                key: "others".into(),
                name: "Based on what people here watch".into(),
                results: found,
            });
        }
    }

    Ok(ForYou { library, shelves })
}

#[derive(SimpleObject)]
pub struct Similar {
    recommendations: Vec<Found>,

    also_watched: Vec<Title>,
}

pub(super) async fn similar(ctx: &Context<'_>, title: &Title) -> ApiResult<Similar> {
    let state = ctx.state();
    let access = ctx.access()?;
    let user = access.person();
    let id = title.row.id;

    let (provider, provider_id): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT provider, provider_id FROM items WHERE id = ?").bind(id).fetch_one(&state.db).await?;

    let kind = if title.row.kind == "show" { ItemKind::Show } else { ItemKind::Movie };

    let mut recommendations = Vec::new();

    if let (Some(provider), Some(pid)) = (parse_provider(provider.as_deref()), provider_id) {
        match state.metadata.recommendations(state, provider, kind, std::slice::from_ref(&pid)).await {
            Ok(mut recs) => {
                let list: Vec<Candidate> = recs.remove(&pid).unwrap_or_default().into_iter().take(24).collect();
                recommendations = annotate(state, provider, &title.row.library, list).await?;

                recommendations.retain(|f| f.title_id != Some(id));
            },
            Err(e) => tracing::debug!("recommendations for item {id}: {e:#}"),
        }
    }

    let also: Vec<i64> = sqlx::query_scalar(
        "WITH viewers AS (
             SELECT DISTINCT p.user_id FROM progress p JOIN media m ON m.path = p.media_path
             WHERE m.item_id = ?1 AND p.user_id != ?2
         ), mine AS (
             SELECT DISTINCT m.item_id FROM progress p JOIN media m ON m.path = p.media_path WHERE p.user_id = ?2
         )
         SELECT m.item_id FROM progress p JOIN media m ON m.path = p.media_path
         WHERE p.user_id IN (SELECT user_id FROM viewers) AND m.item_id != ?1 AND m.item_id NOT IN (SELECT item_id FROM mine)
         GROUP BY m.item_id ORDER BY COUNT(DISTINCT p.user_id) DESC, MAX(p.updated_at) DESC LIMIT 60",
    )
    .bind(id)
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let also_watched = Title::load_many(state, &access, also, 16).await?;
    Ok(Similar { recommendations, also_watched })
}

pub(super) async fn popular_here(state: &AppState, access: &Arc<Access>) -> ApiResult<Vec<PopularTitle>> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT m.item_id, COUNT(DISTINCT p.user_id) FROM progress p JOIN media m ON m.path = p.media_path
         WHERE p.user_id != ?1 AND p.updated_at > ?2 AND m.item_id NOT IN (
             SELECT m2.item_id FROM progress p2 JOIN media m2 ON m2.path = p2.media_path WHERE p2.user_id = ?1)
         GROUP BY m.item_id ORDER BY COUNT(DISTINCT p.user_id) DESC, MAX(p.updated_at) DESC LIMIT 60",
    )
    .bind(access.person().id)
    .bind(now() - 30 * 86400)
    .fetch_all(&state.db)
    .await?;

    let people: HashMap<i64, i64> = rows.iter().copied().collect();
    let titles = Title::load_many(state, access, rows.into_iter().map(|r| r.0), 20).await?;

    Ok(titles
        .into_iter()
        .map(|title| PopularTitle { people: people.get(&title.row.id).copied().unwrap_or(1), title })
        .collect())
}

#[derive(SimpleObject)]
pub struct Discovery {
    library: String,
    results: Vec<Found>,
}

#[derive(Default)]
pub struct DiscoveryQuery;

#[Object]
impl DiscoveryQuery {
    async fn discover(
        &self,
        ctx: &Context<'_>,
        library: Option<String>,
        query: Option<String>,
    ) -> ApiResult<Discovery> {
        let state = ctx.state();
        let (library, provider) = pick_library(state, ctx.user()?, library).await?;
        let query = query.unwrap_or_default();

        let trending = query.trim().is_empty();
        let bad = |e: anyhow::Error| ApiError::bad_request(format!("{e:#}"));
        let mut results = Vec::new();

        for kind in [ItemKind::Show, ItemKind::Movie] {
            let found = if trending {
                state.metadata.trending(state, provider, kind).await.map_err(bad)?
            } else {
                state.metadata.search(state, provider, kind, query.trim(), None).await.map_err(bad)?
            };

            results.extend(found);
        }

        let mut results = annotate(state, provider, &library, results).await?;

        if trending {
            results.retain(|f| !f.here());
        }

        Ok(Discovery { library, results })
    }

    async fn aired_episodes(&self, ctx: &Context<'_>, provider: Provider, id: String) -> ApiResult<i64> {
        let state = ctx.state();

        if !ctx.user()?.permissions.manage_shows {
            return Err(ApiError::forbidden());
        }

        let schedule = state
            .metadata
            .schedule(state, provider, &id, &Default::default())
            .await
            .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

        let aired = schedule.seasons.iter().filter(|s| s.number > 0).flat_map(|s| &s.airing).filter(|e| e.aired);
        Ok(aired.count() as i64)
    }

    async fn for_you(&self, ctx: &Context<'_>, library: Option<String>) -> ApiResult<ForYou> {
        for_you(ctx.state(), ctx.user()?, library).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(id: &str) -> Candidate {
        Candidate {
            category: crate::metadata::MediaCategory::Episodes,
            provider: Provider::Anilist,
            id: id.into(),
            title: id.into(),
            romaji: None,
            year: None,
            poster: None,
            overview: None,
        }
    }

    fn seed(id: &str, weight: f64) -> Seed {
        Seed { id: id.into(), title: format!("seed {id}"), weight }
    }

    #[test]
    fn shows_several_seeds_agree_on_come_first() {
        let seeds = [seed("a", 1.0), seed("b", 1.0)];

        let recs = HashMap::from([
            ("a".to_string(), vec![show("x"), show("shared"), show("b")]),
            ("b".to_string(), vec![show("y"), show("shared")]),
        ]);

        let ids: Vec<String> = blend(&seeds, &recs).into_iter().map(|(c, _)| c.id).collect();
        assert_eq!(ids[0], "shared");

        assert!(!ids.contains(&"b".to_string()));
    }

    #[test]
    fn credits_the_seed_that_counted_most() {
        let seeds = [seed("a", 0.2), seed("b", 3.0)];
        let recs = HashMap::from([("a".to_string(), vec![show("x")]), ("b".to_string(), vec![show("y"), show("x")])]);
        let blended = blend(&seeds, &recs);
        let x = blended.iter().find(|(c, _)| c.id == "x").unwrap();
        assert_eq!(x.1, "seed b");
    }
}
