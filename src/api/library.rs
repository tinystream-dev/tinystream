// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Reverse;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use async_graphql::{Context, Enum, Object, SimpleObject, Upload};
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use tokio::sync::OnceCell;

use super::schema::{Access, Ctx};
use crate::auth::User;
use crate::config::{LibraryKind, Provider};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::library::{BACKDROP_NAMES, POSTER_NAMES, local_art, local_still, local_version, validate_image};
use crate::media::probe::MediaInfo;
use crate::metadata::{Candidate, ItemKind};
use crate::state::AppState;

#[derive(sqlx::FromRow, Debug, Clone)]
pub(super) struct ItemRow {
    pub id: i64,
    pub library: String,
    pub kind: String,
    pub path: String,
    pub title: String,
    pub year: Option<i64>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub custom_poster: bool,
    pub custom_backdrop: bool,
    pub artwork_version: i64,
    pub updated_at: i64,
}

const FRESH_FOR: i64 = 3 * 86400;

const ITEM_COLUMNS: &str = "i.id, i.library, i.kind, i.path, i.title, i.year, i.poster, i.backdrop, i.updated_at, i.poster_override IS NOT NULL AS custom_poster, i.backdrop_override IS NOT NULL AS custom_backdrop, i.artwork_version";

pub(super) async fn item_row(state: &AppState, id: i64) -> ApiResult<Option<ItemRow>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {ITEM_COLUMNS} FROM items i WHERE i.id = ?")))
        .bind(id)
        .fetch_optional(&state.db)
        .await?)
}

pub(super) fn episode_label(season: Option<i64>, episode: Option<i64>, episode_end: Option<i64>) -> Option<String> {
    let s = season?;

    let Some(e) = episode else {
        return Some(if s == 0 { "Special".into() } else { "Extra".into() });
    };

    let end = episode_end.map(|x| format!("–E{x:02}")).unwrap_or_default();
    Some(if s == 0 { format!("Special {e}") } else { format!("S{s:02}E{e:02}{end}") })
}

fn season_name(number: i64, count: usize) -> String {
    if number == 0 { if count == 1 { "Special".into() } else { "Specials".into() } } else { format!("Season {number}") }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleKind {
    Show,
    Movie,
}

impl TitleKind {
    pub fn parse(s: &str) -> Self {
        if s == "movie" { TitleKind::Movie } else { TitleKind::Show }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchState {
    Pending,
    Matched,

    Unmatched,

    Manual,
}

impl MatchState {
    fn parse(s: &str) -> Self {
        match s {
            "matched" => MatchState::Matched,
            "unmatched" => MatchState::Unmatched,
            "manual" => MatchState::Manual,
            _ => MatchState::Pending,
        }
    }
}

pub(super) fn parse_provider(s: Option<&str>) -> Option<Provider> {
    match s? {
        "anilist" => Some(Provider::Anilist),
        "tmdb" => Some(Provider::Tmdb),
        _ => None,
    }
}

struct Details {
    overview: Option<String>,
    genres: Vec<String>,
    rating: Option<f64>,
    match_state: String,
    provider: Option<String>,
    provider_id: Option<String>,
}

#[derive(Clone, Copy)]
struct Stats {
    total: i64,
    watched: i64,
    progress: Option<f64>,
    fresh: i64,
}

pub struct Title {
    pub(super) row: ItemRow,
    access: Arc<Access>,
    details: OnceCell<Details>,
    stats: OnceCell<Stats>,
    videos: OnceCell<Vec<Episode>>,
}

impl Title {
    pub(super) fn new(row: ItemRow, access: Arc<Access>) -> Self {
        Self { row, access, details: OnceCell::new(), stats: OnceCell::new(), videos: OnceCell::new() }
    }

    pub(super) async fn load(state: &AppState, access: &Arc<Access>, id: i64) -> ApiResult<Option<Self>> {
        Ok(item_row(state, id).await?.filter(|r| access.sees(r.id, &r.library)).map(|r| Self::new(r, access.clone())))
    }

    pub(super) async fn load_many(
        state: &AppState,
        access: &Arc<Access>,
        ids: impl IntoIterator<Item = i64>,
        limit: usize,
    ) -> ApiResult<Vec<Self>> {
        let mut out = Vec::new();

        for id in ids {
            if let Some(t) = Self::load(state, access, id).await? {
                out.push(t);

                if out.len() >= limit {
                    break;
                }
            }
        }

        Ok(out)
    }

    fn from_rows(rows: Vec<ItemRow>, access: &Arc<Access>) -> Vec<Self> {
        rows.into_iter().filter(|r| access.sees(r.id, &r.library)).map(|r| Self::new(r, access.clone())).collect()
    }

    async fn details(&self, state: &AppState) -> ApiResult<&Details> {
        self.details
            .get_or_try_init(|| async {
                let (overview, genres, rating, match_state, provider, provider_id): (
                    Option<String>,
                    String,
                    Option<f64>,
                    String,
                    Option<String>,
                    Option<String>,
                ) = sqlx::query_as(
                    "SELECT overview, genres, rating, match_state, provider, provider_id FROM items WHERE id = ?",
                )
                .bind(self.row.id)
                .fetch_one(&state.db)
                .await?;

                let genres = serde_json::from_str(&genres).unwrap_or_default();
                Ok::<_, ApiError>(Details { overview, genres, rating, match_state, provider, provider_id })
            })
            .await
    }

    async fn stats(&self, state: &AppState) -> ApiResult<Stats> {
        let person = self.access.person().id;

        self.stats
            .get_or_try_init(|| async {
                let (total, watched, progress, fresh): (i64, i64, Option<f64>, i64) = sqlx::query_as(
                    "SELECT COUNT(*),
                            COALESCE(SUM(p.finished), 0),
                            (SELECT p2.position / p2.duration FROM media m2 JOIN progress p2 ON p2.media_path = m2.path AND p2.user_id = ?1
                              WHERE m2.item_id = ?2 AND p2.finished = 0 AND p2.duration > 0 ORDER BY p2.updated_at DESC LIMIT 1),
                            COALESCE(SUM(COALESCE(p.finished, 0) = 0 AND m.added_at > ?3), 0)
                     FROM media m LEFT JOIN progress p ON p.media_path = m.path AND p.user_id = ?1
                     WHERE m.item_id = ?2",
                )
                .bind(person)
                .bind(self.row.id)
                .bind(now() - FRESH_FOR)
                .fetch_one(&state.db)
                .await?;

                Ok::<_, ApiError>(Stats { total, watched, progress, fresh })
            })
            .await
            .copied()
    }

    pub(super) async fn episodes(&self, state: &AppState) -> ApiResult<&[Episode]> {
        Ok(self.videos.get_or_try_init(|| episodes(state, self.access.person(), self.row.id)).await?.as_slice())
    }

    fn video(&self, ep: &Episode) -> Video {
        Video { ep: ep.clone(), access: self.access.clone() }
    }

    fn art(&self, kind: &str, names: &[&str], remote: &Option<String>) -> Option<String> {
        let local = local_art(FsPath::new(&self.row.path), names);
        let version = local_version(local.as_deref());

        ((if kind == "poster" { self.row.custom_poster } else { self.row.custom_backdrop })
            || remote.is_some()
            || local.is_some())
        .then(|| match self.access.room() {
            Some(code) => {
                format!(
                    "/api/together/{code}/art/{kind}?v={}&a={}&l={version}",
                    self.row.updated_at, self.row.artwork_version
                )
            },
            None => format!(
                "/api/images/item/{}/{kind}?v={}&a={}&l={version}",
                self.row.id, self.row.updated_at, self.row.artwork_version
            ),
        })
    }

    async fn tint(&self, ctx: &Context<'_>, kind: &'static str) -> Option<String> {
        let row = &self.row;

        let (remote, custom) =
            if kind == "poster" { (&row.poster, row.custom_poster) } else { (&row.backdrop, row.custom_backdrop) };

        let source =
            super::images::item_source(row.id, &row.path, kind, remote.as_deref(), custom, row.artwork_version)?;

        ctx.state().tints.get(ctx.state(), source).await
    }

    pub(super) fn poster_url(&self) -> Option<String> {
        self.art("poster", POSTER_NAMES, &self.row.poster)
    }

    fn editor<'a>(&self, ctx: &'a Context<'_>) -> ApiResult<&'a User> {
        if self.access.user().is_none() {
            return Err(ApiError::forbidden());
        }
        ctx.allowed(|p| p.edit_metadata)
    }
}

#[Object]
impl Title {
    async fn id(&self) -> i64 {
        self.row.id
    }

    async fn kind(&self) -> TitleKind {
        TitleKind::parse(&self.row.kind)
    }

    async fn library(&self) -> &str {
        &self.row.library
    }

    async fn name(&self) -> &str {
        &self.row.title
    }

    async fn year(&self) -> Option<i64> {
        self.row.year
    }

    async fn poster(&self) -> Option<String> {
        self.poster_url()
    }

    async fn backdrop(&self) -> Option<String> {
        self.art("backdrop", BACKDROP_NAMES, &self.row.backdrop)
    }

    /// The most vivid colour of the poster as "r g b", empty until it's been worked out.
    async fn poster_tint(&self, ctx: &Context<'_>) -> Option<String> {
        self.tint(ctx, "poster").await
    }

    /// The same for the backdrop.
    async fn backdrop_tint(&self, ctx: &Context<'_>) -> Option<String> {
        self.tint(ctx, "backdrop").await
    }

    async fn custom_poster(&self) -> bool {
        self.row.custom_poster
    }

    async fn custom_backdrop(&self) -> bool {
        self.row.custom_backdrop
    }

    async fn overview(&self, ctx: &Context<'_>) -> ApiResult<Option<String>> {
        Ok(self.details(ctx.state()).await?.overview.clone())
    }

    async fn genres(&self, ctx: &Context<'_>) -> ApiResult<Vec<String>> {
        Ok(self.details(ctx.state()).await?.genres.clone())
    }

    async fn rating(&self, ctx: &Context<'_>) -> ApiResult<Option<f64>> {
        Ok(self.details(ctx.state()).await?.rating)
    }

    async fn path(&self, ctx: &Context<'_>) -> Option<&str> {
        ctx.maybe_user().filter(|u| u.is_admin).map(|_| self.row.path.as_str())
    }

    async fn match_state(&self, ctx: &Context<'_>) -> ApiResult<MatchState> {
        Ok(MatchState::parse(&self.details(ctx.state()).await?.match_state))
    }

    async fn provider(&self, ctx: &Context<'_>) -> ApiResult<Option<Provider>> {
        Ok(parse_provider(self.details(ctx.state()).await?.provider.as_deref()))
    }

    async fn provider_id(&self, ctx: &Context<'_>) -> ApiResult<Option<String>> {
        Ok(self.details(ctx.state()).await?.provider_id.clone())
    }

    async fn library_provider(&self, ctx: &Context<'_>) -> Option<Provider> {
        ctx.state().config.current().library(&self.row.library).and_then(|l| l.metadata_provider)
    }

    async fn watched_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(self.stats(ctx.state()).await?.watched)
    }

    async fn video_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(self.stats(ctx.state()).await?.total)
    }

    async fn progress(&self, ctx: &Context<'_>) -> ApiResult<Option<f64>> {
        Ok(self.stats(ctx.state()).await?.progress)
    }

    async fn fresh_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(self.stats(ctx.state()).await?.fresh)
    }

    async fn videos(&self, ctx: &Context<'_>) -> ApiResult<Vec<Video>> {
        Ok(self.episodes(ctx.state()).await?.iter().map(|e| self.video(e)).collect())
    }

    async fn movie(&self, ctx: &Context<'_>) -> ApiResult<Option<Video>> {
        if self.row.kind != "movie" {
            return Ok(None);
        }
        Ok(self.episodes(ctx.state()).await?.first().map(|e| self.video(e)))
    }

    async fn seasons(&self, ctx: &Context<'_>) -> ApiResult<Vec<Season>> {
        let state = ctx.state();
        let eps = self.episodes(state).await?;

        let meta: Vec<(i64, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as("SELECT number, title, overview, poster FROM seasons WHERE item_id = ?")
                .bind(self.row.id)
                .fetch_all(&state.db)
                .await?;

        let mut numbers: Vec<i64> = eps.iter().filter_map(|e| e.season).collect();
        numbers.dedup();

        Ok(numbers
            .into_iter()
            .map(|n| {
                let m = meta.iter().find(|s| s.0 == n);

                Season {
                    number: n,
                    name: season_name(n, eps.iter().filter(|e| e.season == Some(n)).count()),
                    title: m.and_then(|m| m.1.clone()),
                    overview: m.and_then(|m| m.2.clone()),
                    poster: m.and_then(|m| m.3.as_ref()).map(|_| match self.access.room() {
                        Some(code) => format!("/api/together/{code}/art/poster"),
                        None => format!("/api/images/season/{}/{n}?v={}", self.row.id, self.row.updated_at),
                    }),
                    episodes: eps.iter().filter(|e| e.season == Some(n)).map(|e| self.video(e)).collect(),
                }
            })
            .collect())
    }

    async fn next_up(&self, ctx: &Context<'_>) -> ApiResult<Option<NextUp>> {
        let state = ctx.state();
        let eps = self.episodes(state).await?;

        let next = next_up(eps).or_else(|| {
            eps.iter().find(|e| e.season != Some(0) && e.episode.is_some()).or(eps.first()).map(|e| (e, false))
        });

        Ok(next.map(|(e, resuming)| {
            state.media.prefetch_subtitles(PathBuf::from(&e.path));
            NextUp { video: self.video(e), resuming }
        }))
    }

    async fn similar(&self, ctx: &Context<'_>) -> ApiResult<super::discovery::Similar> {
        if self.access.user().is_none() {
            return Err(ApiError::forbidden());
        }
        super::discovery::similar(ctx, self).await
    }

    async fn match_candidates(
        &self,
        ctx: &Context<'_>,
        query: Option<String>,
        provider: Option<Provider>,
    ) -> ApiResult<MatchCandidates> {
        self.editor(ctx)?;
        let state = ctx.state();

        let (folder_title, folder_year): (String, Option<i64>) =
            sqlx::query_as("SELECT folder_title, folder_year FROM items WHERE id = ?")
                .bind(self.row.id)
                .fetch_one(&state.db)
                .await?;

        let chosen = provider
            .or_else(|| state.config.current().library(&self.row.library).and_then(|l| l.metadata_provider))
            .unwrap_or(Provider::Tmdb);

        let kind = if self.row.kind == "show" { ItemKind::Show } else { ItemKind::Movie };
        let query = query.filter(|q| !q.trim().is_empty()).unwrap_or(folder_title);

        let results = state
            .metadata
            .search(state, chosen, kind, &query, if provider.is_some() { None } else { folder_year })
            .await
            .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

        Ok(MatchCandidates { provider: chosen, query, results })
    }

    #[cfg(feature = "torrent")]
    async fn series(&self, ctx: &Context<'_>) -> ApiResult<Option<super::automation::Series>> {
        if self.access.user().is_none() {
            return Ok(None);
        }
        super::automation::Series::for_path(ctx.state(), &self.row.path).await
    }
}

#[derive(SimpleObject)]
pub struct NextUp {
    pub video: Video,

    pub resuming: bool,
}

#[derive(SimpleObject)]
pub struct Season {
    number: i64,

    name: String,

    title: Option<String>,
    overview: Option<String>,
    poster: Option<String>,
    episodes: Vec<Video>,
}

#[derive(SimpleObject)]
pub struct MatchCandidates {
    provider: Provider,

    query: String,
    results: Vec<Candidate>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub(super) struct Episode {
    pub id: i64,
    pub item_id: i64,
    pub path: String,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub episode_end: Option<i64>,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub duration: Option<f64>,
    pub position: Option<f64>,
    pub finished: Option<bool>,
    pub updated_at: Option<i64>,
    pub added_at: i64,
    pub custom_still: bool,
    pub artwork_version: i64,
}

const EPISODE_COLUMNS: &str = "m.id, m.item_id, m.path, m.season, m.episode, m.episode_end, m.title, m.overview,
        m.air_date, COALESCE(m.duration, p.duration) AS duration,
        p.position, p.finished, p.updated_at, m.added_at, m.still_override IS NOT NULL AS custom_still, m.artwork_version
    FROM media m LEFT JOIN progress p ON p.media_path = m.path AND p.user_id = ?";

pub(super) async fn episodes(state: &AppState, user: &User, item_id: i64) -> ApiResult<Vec<Episode>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {EPISODE_COLUMNS} WHERE m.item_id = ?
         ORDER BY CASE WHEN m.season = 0 THEN 1 ELSE 0 END, m.season, m.episode IS NULL, m.episode, m.path"
    )))
    .bind(user.id)
    .bind(item_id)
    .fetch_all(&state.db)
    .await?)
}

pub(super) fn next_up(eps: &[Episode]) -> Option<(&Episode, bool)> {
    let last = eps.iter().filter(|e| e.updated_at.is_some()).max_by_key(|e| e.updated_at)?;

    if last.finished != Some(true) && last.position.unwrap_or(0.0) > 5.0 {
        return Some((last, true));
    }

    let idx = eps.iter().position(|e| e.id == last.id)?;

    eps[idx + 1..]
        .iter()
        .find(|e| e.season != Some(0) && e.episode.is_some() && e.finished != Some(true))
        .map(|e| (e, false))
}

pub struct Video {
    pub(super) ep: Episode,
    access: Arc<Access>,
}

impl Video {
    pub(super) async fn load(state: &AppState, access: &Arc<Access>, id: i64) -> ApiResult<Option<Self>> {
        let ep: Option<Episode> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {EPISODE_COLUMNS} WHERE m.id = ?")))
                .bind(access.person().id)
                .bind(id)
                .fetch_optional(&state.db)
                .await?;

        let Some(ep) = ep else { return Ok(None) };

        let library: String =
            sqlx::query_scalar("SELECT library FROM items WHERE id = ?").bind(ep.item_id).fetch_one(&state.db).await?;

        Ok(access.sees(ep.item_id, &library).then(|| Self { ep, access: access.clone() }))
    }

    pub(super) fn path(&self) -> PathBuf {
        PathBuf::from(&self.ep.path)
    }

    async fn neighbours(&self, state: &AppState) -> ApiResult<(Option<Video>, Option<Video>)> {
        let mut eps = episodes(state, self.access.person(), self.ep.item_id).await?;

        eps.retain(|e| {
            if self.ep.episode.is_none() {
                e.episode.is_none() && e.season == self.ep.season
            } else {
                e.episode.is_some()
            }
        });

        let Some(p) = eps.iter().position(|e| e.id == self.ep.id) else { return Ok((None, None)) };
        let wrap = |e: &Episode| Video { ep: e.clone(), access: self.access.clone() };
        Ok((p.checked_sub(1).map(|p| wrap(&eps[p])), eps.get(p + 1).map(wrap)))
    }
}

#[Object]
impl Video {
    async fn id(&self) -> i64 {
        self.ep.id
    }

    async fn title(&self, ctx: &Context<'_>) -> ApiResult<Title> {
        Title::load(ctx.state(), &self.access, self.ep.item_id).await?.ok_or_else(|| ApiError::not_found("title"))
    }

    async fn season(&self) -> Option<i64> {
        self.ep.season
    }

    async fn episode(&self) -> Option<i64> {
        self.ep.episode
    }

    async fn episode_end(&self) -> Option<i64> {
        self.ep.episode_end
    }

    async fn label(&self) -> Option<String> {
        episode_label(self.ep.season, self.ep.episode, self.ep.episode_end)
    }

    async fn name(&self) -> Option<&str> {
        self.ep.title.as_deref().or_else(|| {
            (self.ep.season.is_some() && self.ep.episode.is_none())
                .then(|| FsPath::new(&self.ep.path).file_stem().and_then(|s| s.to_str()))
                .flatten()
        })
    }

    async fn overview(&self) -> Option<&str> {
        self.ep.overview.as_deref()
    }

    async fn still(&self) -> String {
        let local = local_still(FsPath::new(&self.ep.path));
        let version = local_version(local.as_deref());

        match self.access.room() {
            Some(code) => {
                format!("/api/together/{code}/stills/{}?v={}&l={version}", self.ep.id, self.ep.artwork_version)
            },
            None => format!("/api/images/media/{}?v={}&l={version}", self.ep.id, self.ep.artwork_version),
        }
    }

    async fn custom_still(&self) -> bool {
        self.ep.custom_still
    }

    async fn air_date(&self) -> Option<&str> {
        self.ep.air_date.as_deref()
    }

    async fn duration(&self) -> Option<f64> {
        self.ep.duration
    }

    async fn position(&self) -> Option<f64> {
        self.access.user().and(self.ep.position)
    }

    async fn finished(&self) -> Option<bool> {
        self.access.user().and(self.ep.finished)
    }

    async fn watched_at(&self) -> Option<i64> {
        self.access.user().and(self.ep.updated_at)
    }

    async fn previous(&self, ctx: &Context<'_>) -> ApiResult<Option<Video>> {
        Ok(self.neighbours(ctx.state()).await?.0)
    }

    async fn next(&self, ctx: &Context<'_>) -> ApiResult<Option<Video>> {
        Ok(self.neighbours(ctx.state()).await?.1)
    }

    async fn media(&self, ctx: &Context<'_>) -> ApiResult<Arc<MediaInfo>> {
        let state = ctx.state();
        let path = self.path();

        let info = state.media.probe(&path).await.map_err(|e| {
            ApiError::new(axum::http::StatusCode::UNPROCESSABLE_ENTITY, format!("can't read this file: {e:#}"))
        })?;

        if let Some(d) = info.duration {
            sqlx::query("UPDATE media SET duration = ? WHERE id = ? AND duration IS NULL")
                .bind(d)
                .bind(self.ep.id)
                .execute(&state.db)
                .await?;
        }

        state.media.prefetch_subtitles(path);

        if let (_, Some(next)) = self.neighbours(state).await? {
            state.media.prefetch_subtitles(next.path());
        }

        Ok(info)
    }
}

pub struct Library {
    name: String,
    access: Arc<Access>,
}

#[Object]
impl Library {
    async fn name(&self) -> &str {
        &self.name
    }

    async fn kind(&self, ctx: &Context<'_>) -> LibraryKind {
        ctx.state().config.current().library(&self.name).map(|l| l.kind).unwrap_or_default()
    }

    async fn album_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE library = ?")
            .bind(&self.name)
            .fetch_one(&ctx.state().db)
            .await?)
    }

    async fn track_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE library = ?")
            .bind(&self.name)
            .fetch_one(&ctx.state().db)
            .await?)
    }

    /// A series counting as one, as its books are shown.
    async fn book_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(crate::books::catalog::count(&ctx.state().db, &self.name).await? as i64)
    }

    async fn show_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE library = ? AND kind = 'show'")
            .bind(&self.name)
            .fetch_one(&ctx.state().db)
            .await?)
    }

    async fn movie_count(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE library = ? AND kind = 'movie'")
            .bind(&self.name)
            .fetch_one(&ctx.state().db)
            .await?)
    }

    async fn titles(&self, ctx: &Context<'_>) -> ApiResult<Vec<Title>> {
        let rows: Vec<ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {ITEM_COLUMNS} FROM items i WHERE i.library = ? ORDER BY i.sort_title"
        )))
        .bind(&self.name)
        .fetch_all(&ctx.state().db)
        .await?;

        Ok(Title::from_rows(rows, &self.access))
    }
}

pub struct Home {
    access: Arc<Access>,
}

#[derive(SimpleObject)]
pub struct ContinueEntry {
    video: Video,

    position: f64,

    up_next: bool,

    new_episode: bool,

    watched_at: Option<i64>,
}

#[derive(SimpleObject)]
pub struct Shelf {
    library: String,
    titles: Vec<Title>,
}

#[derive(SimpleObject)]
pub struct PopularTitle {
    pub title: Title,

    pub people: i64,
}

#[Object]
impl Home {
    async fn continue_watching(&self, ctx: &Context<'_>) -> ApiResult<Vec<ContinueEntry>> {
        let state = ctx.state();
        let user = self.access.person();

        let recent: Vec<i64> = sqlx::query_scalar(
            "SELECT m.item_id FROM progress p JOIN media m ON m.path = p.media_path
             WHERE p.user_id = ? GROUP BY m.item_id ORDER BY MAX(p.updated_at) DESC LIMIT 30",
        )
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;

        let mut out = Vec::new();

        for item_id in recent {
            let Some(title) = Title::load(state, &self.access, item_id).await? else { continue };
            let eps = title.episodes(state).await?;
            let Some((ep, resuming)) = next_up(eps) else { continue };
            let watched_at = eps.iter().filter_map(|e| e.updated_at).max();

            out.push(ContinueEntry {
                position: if resuming { ep.position.unwrap_or(0.0) } else { 0.0 },
                up_next: !resuming,
                new_episode: !resuming
                    && watched_at.is_some_and(|w| ep.added_at > w)
                    && ep.added_at > now() - 2 * FRESH_FOR,
                watched_at,
                video: title.video(ep),
            });

            if out.len() >= 16 {
                break;
            }
        }

        Ok(out)
    }

    async fn recently_added(&self, ctx: &Context<'_>) -> ApiResult<Vec<Shelf>> {
        let state = ctx.state();
        let mut shelves = Vec::new();

        for library in self.access.person().libraries(state).await? {
            let rows: Vec<ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {ITEM_COLUMNS} FROM items i WHERE i.library = ?
                 ORDER BY (SELECT MAX(added_at) FROM media WHERE item_id = i.id) DESC LIMIT 24"
            )))
            .bind(&library)
            .fetch_all(&state.db)
            .await?;

            shelves.push(Shelf { library, titles: Title::from_rows(rows, &self.access) });
        }

        Ok(shelves)
    }

    async fn popular_here(&self, ctx: &Context<'_>) -> ApiResult<Vec<PopularTitle>> {
        super::discovery::popular_here(ctx.state(), &self.access).await
    }
}

#[derive(SimpleObject)]
pub struct SearchResults {
    titles: Vec<Title>,

    videos: Vec<Video>,
}

struct Fuzzy {
    pattern: Pattern,

    floor: u32,
}

impl Fuzzy {
    fn new(q: &str) -> Option<Self> {
        let pattern = Pattern::new(q, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
        let chars: usize = pattern.atoms.iter().map(|a| a.needle_text().len()).sum();
        (chars > 0).then(|| Fuzzy { pattern, floor: chars as u32 * 16 })
    }

    fn rank<T, const N: usize>(&self, candidates: Vec<T>, limit: usize, fields: impl Fn(&T) -> [&str; N]) -> Vec<T> {
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut buf = Vec::new();

        let mut scored: Vec<(u32, usize, T)> = candidates
            .into_iter()
            .filter_map(|c| {
                let best = fields(&c)
                    .into_iter()
                    .filter_map(|f| {
                        let score = self.pattern.score(Utf32Str::new(f, &mut buf), &mut matcher)?;
                        (score >= self.floor).then_some((score, f.chars().count()))
                    })
                    .max_by_key(|&(score, len)| (score, Reverse(len)))?;

                Some((best.0, best.1, c))
            })
            .collect();

        scored.sort_by_key(|&(score, len, _)| (Reverse(score), len));
        scored.into_iter().take(limit).map(|(_, _, c)| c).collect()
    }
}

#[derive(sqlx::FromRow)]
struct SearchRow {
    #[sqlx(flatten)]
    item: ItemRow,
    folder_title: String,
}

async fn search(state: &AppState, access: &Arc<Access>, q: &str) -> ApiResult<SearchResults> {
    let Some(fuzzy) = Fuzzy::new(q) else {
        return Ok(SearchResults { titles: Vec::new(), videos: Vec::new() });
    };

    let rows: Vec<SearchRow> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {ITEM_COLUMNS}, i.folder_title FROM items i")))
            .fetch_all(&state.db)
            .await?;

    let rows = rows.into_iter().filter(|r| access.sees(r.item.id, &r.item.library)).collect();
    let rows = fuzzy.rank(rows, 12, |r| [r.item.title.as_str(), r.folder_title.as_str()]);
    let titles = Title::from_rows(rows.into_iter().map(|r| r.item).collect(), access);

    let eps: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT m.id, i.id, i.library, m.title FROM media m JOIN items i ON i.id = m.item_id WHERE m.title IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await?;

    let eps = eps.into_iter().filter(|e| access.sees(e.1, &e.2)).collect();
    let mut videos = Vec::new();

    for (id, ..) in fuzzy.rank(eps, 8, |e| [e.3.as_str()]) {
        videos.extend(Video::load(state, access, id).await?);
    }

    Ok(SearchResults { titles, videos })
}

#[derive(Default)]
pub struct LibraryQuery;

#[Object]
impl LibraryQuery {
    async fn libraries(&self, ctx: &Context<'_>) -> ApiResult<Vec<Library>> {
        let access = ctx.access()?;
        let names = access.person().libraries(ctx.state()).await?;
        Ok(names.into_iter().map(|name| Library { name, access: access.clone() }).collect())
    }

    async fn library(&self, ctx: &Context<'_>, name: String) -> ApiResult<Option<Library>> {
        let access = ctx.access()?;
        let visible = access.person().libraries(ctx.state()).await?.contains(&name);
        Ok(visible.then(|| Library { name, access }))
    }

    async fn title(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<Title>> {
        Title::load(ctx.state(), &ctx.access()?, id).await
    }

    /// The titles among `ids` that still exist and are visible, in the given order.
    async fn titles(&self, ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<Vec<Title>> {
        let limit = ids.len();
        Title::load_many(ctx.state(), &ctx.access()?, ids, limit).await
    }

    async fn video(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<Video>> {
        Video::load(ctx.state(), &ctx.access()?, id).await
    }

    async fn home(&self, ctx: &Context<'_>) -> ApiResult<Home> {
        Ok(Home { access: ctx.access()? })
    }

    async fn search(&self, ctx: &Context<'_>, query: String) -> ApiResult<SearchResults> {
        search(ctx.state(), &ctx.access()?, &query).await
    }
}

#[derive(Default)]
pub struct LibraryMutation;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleArtwork {
    Poster,
    Backdrop,
}

async fn artwork_upload(ctx: &Context<'_>, image: Option<Upload>) -> ApiResult<Option<Vec<u8>>> {
    let Some(image) = image else { return Ok(None) };
    let value = image.value(ctx).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let content = value.content;

    tokio::task::spawn_blocking(move || {
        validate_image(&content).map_err(ApiError::bad_request)?;
        Ok(Some(content.to_vec()))
    })
    .await
    .map_err(|e| ApiError::from(anyhow::anyhow!(e)))?
}

async fn set_watched(state: &AppState, user: &User, media: &[(String, Option<f64>)], watched: bool) -> ApiResult<()> {
    let mut tx = state.db.begin().await?;

    for (path, duration) in media {
        if watched {
            let d = duration.unwrap_or(1.0);

            sqlx::query(
                "INSERT INTO progress (user_id, media_path, position, duration, finished, updated_at) VALUES (?, ?, ?, ?, 1, ?)
                 ON CONFLICT(user_id, media_path) DO UPDATE SET position = excluded.position, duration = excluded.duration,
                    finished = 1, updated_at = excluded.updated_at",
            )
            .bind(user.id)
            .bind(path)
            .bind(d)
            .bind(d)
            .bind(now())
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query("DELETE FROM progress WHERE user_id = ? AND media_path = ?")
                .bind(user.id)
                .bind(path)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

#[Object]
impl LibraryMutation {
    async fn set_watched(&self, ctx: &Context<'_>, video_ids: Vec<i64>, watched: bool) -> ApiResult<Vec<Video>> {
        let (state, access) = (ctx.state(), ctx.access()?);
        let mut media = Vec::new();

        for id in &video_ids {
            let video = Video::load(state, &access, *id).await?.ok_or_else(|| ApiError::not_found("video"))?;
            media.push((video.ep.path.clone(), video.ep.duration));
        }

        set_watched(state, access.person(), &media, watched).await?;
        let mut out = Vec::new();

        for id in video_ids {
            out.extend(Video::load(state, &access, id).await?);
        }

        Ok(out)
    }

    async fn set_title_watched(&self, ctx: &Context<'_>, id: i64, watched: bool) -> ApiResult<Title> {
        let (state, access) = (ctx.state(), ctx.access()?);
        Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))?;

        let media: Vec<(String, Option<f64>)> =
            sqlx::query_as("SELECT path, duration FROM media WHERE item_id = ?").bind(id).fetch_all(&state.db).await?;

        set_watched(state, access.person(), &media, watched).await?;
        Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))
    }

    async fn save_progress(&self, ctx: &Context<'_>, video_id: i64, position: f64, duration: f64) -> ApiResult<Video> {
        let (state, access) = (ctx.state(), ctx.access()?);
        let video = Video::load(state, &access, video_id).await?.ok_or_else(|| ApiError::not_found("video"))?;

        if !(position.is_finite() && duration.is_finite() && duration > 0.0) {
            return Err(ApiError::bad_request("bad position"));
        }

        let finished = position / duration >= 0.92 || duration - position <= 30.0;

        sqlx::query(
            "INSERT INTO progress (user_id, media_path, position, duration, finished, updated_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id, media_path) DO UPDATE SET position = excluded.position, duration = excluded.duration,
                finished = MAX(excluded.finished, CASE WHEN excluded.position < 5 THEN 0 ELSE progress.finished END),
                updated_at = excluded.updated_at",
        )
        .bind(access.person().id)
        .bind(&video.ep.path)
        .bind(position)
        .bind(duration)
        .bind(finished)
        .bind(now())
        .execute(&state.db)
        .await?;

        Video::load(state, &access, video_id).await?.ok_or_else(|| ApiError::not_found("video"))
    }

    async fn match_title(
        &self,
        ctx: &Context<'_>,
        id: i64,
        provider: Provider,
        provider_id: String,
    ) -> ApiResult<Title> {
        let (state, access) = (ctx.state(), ctx.access()?);
        let title = Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))?;
        title.editor(ctx)?;

        state
            .metadata
            .apply(state, id, provider, &provider_id, true)
            .await
            .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

        Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))
    }

    async fn set_title_artwork(
        &self,
        ctx: &Context<'_>,
        id: i64,
        kind: TitleArtwork,
        image: Option<Upload>,
    ) -> ApiResult<Title> {
        let (state, access) = (ctx.state(), ctx.access()?);
        let title = Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))?;
        title.editor(ctx)?;
        let data = artwork_upload(ctx, image).await?;

        let sql = match kind {
            TitleArtwork::Poster => {
                "UPDATE items SET poster_override = ?, artwork_version = artwork_version + 1 WHERE id = ?"
            },
            TitleArtwork::Backdrop => {
                "UPDATE items SET backdrop_override = ?, artwork_version = artwork_version + 1 WHERE id = ?"
            },
        };

        state.tints.forget_uploads(state, id).await?;
        sqlx::query(sql).bind(data).bind(id).execute(&state.db).await?;
        state.events.send(crate::events::Event::MetadataUpdated { item_id: id });
        Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))
    }

    async fn set_video_artwork(&self, ctx: &Context<'_>, video_id: i64, image: Option<Upload>) -> ApiResult<Video> {
        let (state, access) = (ctx.state(), ctx.access()?);
        let video = Video::load(state, &access, video_id).await?.ok_or_else(|| ApiError::not_found("video"))?;
        let title = Title::load(state, &access, video.ep.item_id).await?.ok_or_else(|| ApiError::not_found("title"))?;
        title.editor(ctx)?;
        let data = artwork_upload(ctx, image).await?;

        sqlx::query("UPDATE media SET still_override = ?, artwork_version = artwork_version + 1 WHERE id = ?")
            .bind(data)
            .bind(video_id)
            .execute(&state.db)
            .await?;

        state.events.send(crate::events::Event::MetadataUpdated { item_id: video.ep.item_id });
        Video::load(state, &access, video_id).await?.ok_or_else(|| ApiError::not_found("video"))
    }

    async fn refresh_title(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Title> {
        let (state, access) = (ctx.state(), ctx.access()?);
        let title = Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))?;
        title.editor(ctx)?;

        let (match_state, provider, provider_id): (String, Option<String>, Option<String>) =
            sqlx::query_as("SELECT match_state, provider, provider_id FROM items WHERE id = ?")
                .bind(id)
                .fetch_one(&state.db)
                .await?;

        match (parse_provider(provider.as_deref()), provider_id) {
            (Some(p), Some(pid)) => state
                .metadata
                .apply(state, id, p, &pid, match_state == "manual")
                .await
                .map_err(|e| ApiError::bad_request(format!("{e:#}")))?,
            _ => {
                sqlx::query("UPDATE items SET match_state = 'pending', provider = NULL WHERE id = ?")
                    .bind(id)
                    .execute(&state.db)
                    .await?;

                state.metadata.wake();
            },
        }

        Title::load(state, &access, id).await?.ok_or_else(|| ApiError::not_found("title"))
    }
}

#[cfg(test)]
mod fuzzy_tests {
    use super::Fuzzy;

    fn hits(q: &str, titles: &[&'static str]) -> Vec<&'static str> {
        Fuzzy::new(q).unwrap().rank(titles.to_vec(), 10, |t| [*t])
    }

    #[test]
    fn matches_across_punctuation_and_accents() {
        let titles =
            ["Re:ZERO -Starting Life in Another World-", "Attack on Titan", "Pokémon", "Neon Genesis Evangelion"];

        assert_eq!(hits("rezero", &titles), ["Re:ZERO -Starting Life in Another World-"]);
        assert_eq!(hits("aot", &titles)[0], "Attack on Titan");
        assert_eq!(hits("pokemon", &titles), ["Pokémon"]);
        assert_eq!(hits("eva", &titles), ["Neon Genesis Evangelion"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(id: i64, season: i64, number: Option<i64>, finished: bool, at: Option<i64>) -> Episode {
        Episode {
            id,
            item_id: 1,
            path: String::new(),
            season: Some(season),
            episode: number,
            episode_end: None,
            title: None,
            overview: None,
            air_date: None,
            duration: None,
            position: None,
            finished: Some(finished),
            updated_at: at,
            added_at: 0,
            custom_still: false,
            artwork_version: 0,
        }
    }

    #[test]
    fn special_and_extra_labels() {
        assert_eq!(season_name(0, 1), "Special");
        assert_eq!(season_name(0, 2), "Specials");
        assert_eq!(season_name(2, 1), "Season 2");
        assert_eq!(episode_label(Some(0), Some(1), None).as_deref(), Some("Special 1"));
        assert_eq!(episode_label(Some(0), None, None).as_deref(), Some("Special"));
        assert_eq!(episode_label(Some(1), None, None).as_deref(), Some("Extra"));
        assert_eq!(episode_label(None, None, None), None);
    }

    #[test]
    fn next_up_skips_extras_but_can_resume_them() {
        let mut eps = vec![
            episode(1, 1, Some(1), true, Some(10)),
            episode(2, 1, None, false, None),
            episode(3, 2, Some(1), false, None),
            episode(4, 0, Some(1), false, None),
        ];

        assert_eq!(next_up(&eps).map(|(e, resume)| (e.id, resume)), Some((3, false)));
        eps[1].updated_at = Some(20);
        eps[1].position = Some(30.0);
        assert_eq!(next_up(&eps).map(|(e, resume)| (e.id, resume)), Some((2, true)));
        eps[1].finished = Some(true);
        assert_eq!(next_up(&eps).map(|(e, resume)| (e.id, resume)), Some((3, false)));
        eps[2].finished = Some(true);
        eps[2].updated_at = Some(30);
        assert!(next_up(&eps).is_none());
    }
}
