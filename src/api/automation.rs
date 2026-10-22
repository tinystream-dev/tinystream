// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use async_graphql::{ComplexObject, Context, Enum, InputObject, MaybeUndefined, Object, SimpleObject};
use tokio::sync::OnceCell;

use super::discovery::{RequestState, parse_monitor};
use super::images;
use super::library::{self, Title, Video};
use super::schema::{Ctx, EpisodeNumber};
use super::settings::Settings;
use crate::auth::{self, User};
use crate::automation::fsops::UndoReport;
use crate::automation::matching::Numbering;
use crate::automation::naming::Style;
use crate::automation::renames::Applied;
use crate::automation::series::Counts;
use crate::automation::sources::{Detected, Release};
use crate::automation::{self, Candidate, Grab, fsops, naming, release, renames, series};
use crate::config::{List, Monitor, Profile, Provider, Seeding, Source};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::events::Event;
use crate::notifications;
use crate::state::AppState;

fn bad(e: anyhow::Error) -> ApiError {
    ApiError::bad_request(format!("{e:#}"))
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Downloading,
    Seeding,
    Paused,
    Done,
    Failed,
    Removed,
}

impl DownloadState {
    fn parse(s: &str) -> Self {
        match s {
            "seeding" => Self::Seeding,
            "paused" => Self::Paused,
            "done" => Self::Done,
            "failed" => Self::Failed,
            "removed" => Self::Removed,
            _ => Self::Downloading,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportState {
    Pending,
    Done,
    Failed,
    Skipped,
}

impl ImportState {
    fn parse(s: &str) -> Self {
        match s {
            "done" => Self::Done,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => Self::Pending,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorrentStage {
    Checking,

    Metadata,
    Downloading,
    Seeding,
}

impl TorrentStage {
    fn of(s: &libtorrent_sys::Status) -> Self {
        match s.state {
            1 | 7 => Self::Checking,
            2 => Self::Metadata,
            3 => Self::Downloading,
            _ => Self::Seeding,
        }
    }
}

fn eta(s: &libtorrent_sys::Status) -> Option<i64> {
    (s.download_rate > 0 && s.total_wanted > s.total_done).then(|| (s.total_wanted - s.total_done) / s.download_rate)
}

#[derive(SimpleObject)]
pub struct TorrentStatus {
    stage: TorrentStage,
    paused: bool,

    progress: f32,

    download_rate: i64,
    upload_rate: i64,

    done: i64,
    uploaded: i64,
    ratio: f64,
    peers: i32,
    seeds: i32,
    seeding_seconds: i64,
    eta: Option<i64>,

    pieces: Vec<u8>,
}

#[derive(SimpleObject)]
pub struct SeedGoal {
    ratio: Option<f64>,
    seconds: Option<u64>,
}

#[derive(SimpleObject)]
pub struct TorrentFile {
    path: String,
    size: i64,
    done: i64,
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Download {
    category: crate::metadata::MediaCategory,
    id: i64,
    name: String,
    series_id: Option<i64>,
    series_name: Option<String>,

    title: Option<Title>,

    poster: Option<String>,
    episodes: Vec<EpisodeNumber>,

    source: Option<String>,
    size: Option<i64>,
    save_path: String,
    state: DownloadState,
    import_state: ImportState,
    import_error: Option<String>,

    import_mode: Option<String>,
    error: Option<String>,
    added_at: i64,
    finished_at: Option<i64>,
    imported_at: Option<i64>,
    requested_by: Option<User>,

    live: Option<TorrentStatus>,

    seed_goal: SeedGoal,
    #[graphql(skip)]
    hash: Option<String>,
}

#[ComplexObject]
impl Download {
    async fn files(&self, ctx: &Context<'_>) -> Vec<TorrentFile> {
        let files = self.hash.as_deref().map(|h| ctx.state().automation.engine.files(h)).unwrap_or_default();
        files.into_iter().map(|f| TorrentFile { path: f.path, size: f.size, done: f.done }).collect()
    }
}

#[derive(sqlx::FromRow)]
struct DownloadRow {
    id: i64,
    hash: Option<String>,
    name: String,
    series_id: Option<i64>,
    episodes: String,
    source: Option<String>,
    size: Option<i64>,
    save_path: String,
    state: String,
    import_state: String,
    import_error: Option<String>,
    import_mode: Option<String>,
    error: Option<String>,
    added_at: i64,
    finished_at: Option<i64>,
    imported_at: Option<i64>,
    seeding: Option<String>,
    series_title: Option<String>,
    series_path: Option<String>,
    series_poster: Option<String>,
    requested_by: Option<i64>,
}

async fn downloads(ctx: &Context<'_>, id: Option<i64>) -> ApiResult<Vec<Download>> {
    let state = ctx.state();
    let access = ctx.access()?;

    let rows: Vec<DownloadRow> = sqlx::query_as(
        "SELECT d.id, d.hash, d.name, d.series_id, d.episodes, d.source, d.size, d.save_path, d.state, d.import_state,
                d.import_error, d.import_mode, d.error, d.added_at, d.finished_at, d.imported_at, d.seeding,
                s.title AS series_title, s.path AS series_path, s.poster AS series_poster, d.requested_by
         FROM downloads d LEFT JOIN series s ON s.id = d.series_id
         WHERE (?1 IS NULL AND (d.state IN ('downloading', 'seeding', 'paused') OR d.added_at > ?2)) OR d.id = ?1
         ORDER BY d.added_at DESC LIMIT 300",
    )
    .bind(id)
    .bind(now() - 14 * 86400)
    .fetch_all(&state.db)
    .await?;

    let config = state.config.current();
    let mut out = Vec::new();

    for r in rows {
        let rules: Seeding = r
            .seeding
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_else(|| config.downloads.seeding.clone());

        let live = r.hash.as_deref().and_then(|h| state.automation.engine.status(h));

        let item_id = match &r.series_path {
            Some(p) => series::item_id(state, p).await?,
            None => None,
        };

        let title = match item_id {
            Some(id) => Title::load(state, &access, id).await?,
            None => None,
        };

        let requested_by = match r.requested_by {
            Some(u) => auth::load_user(state, u).await?,
            None => None,
        };

        let episodes: Vec<(u32, u32)> = serde_json::from_str(&r.episodes).unwrap_or_default();

        out.push(Download {
            category: release::media_category(&r.name, &episodes),
            id: r.id,
            name: r.name,
            series_id: r.series_id,
            series_name: r.series_title,
            poster: match item_id {
                Some(id) => Some(format!("/api/images/item/{id}/poster")),
                None => r.series_poster,
            },
            title,
            episodes: episodes.into_iter().map(Into::into).collect(),
            source: r.source,
            size: live.as_ref().map(|s| s.total_wanted).filter(|s| *s > 0).or(r.size),
            save_path: r.save_path,
            state: DownloadState::parse(&r.state),
            import_state: ImportState::parse(&r.import_state),
            import_error: r.import_error,
            import_mode: r.import_mode,
            error: r.error.or_else(|| live.as_ref().map(|s| s.error.clone()).filter(|e| !e.is_empty())),
            added_at: r.added_at,
            finished_at: r.finished_at,
            imported_at: r.imported_at,
            requested_by,
            live: live.map(|s| TorrentStatus {
                stage: TorrentStage::of(&s),
                paused: s.paused,
                progress: s.progress,
                download_rate: s.download_rate,
                upload_rate: s.upload_rate,
                done: s.total_done,
                uploaded: s.all_time_upload,
                ratio: s.all_time_upload as f64 / s.total_wanted.max(1) as f64,
                peers: s.num_peers,
                seeds: s.num_seeds,
                seeding_seconds: s.seeding_seconds,
                eta: eta(&s),
                pieces: s.pieces.clone(),
            }),
            seed_goal: SeedGoal { ratio: rules.ratio, seconds: rules.time.map(|t| t.as_secs()) },
            hash: r.hash,
        });
    }

    Ok(out)
}

#[derive(SimpleObject)]
pub struct DownloadEngine {
    version: String,
    download_rate: i64,
    upload_rate: i64,

    active: usize,

    kill_switch: Option<String>,

    listening: Option<String>,
    listen_error: Option<String>,

    slow_hours: bool,
    download_path: String,
}

async fn download_hash(state: &AppState, id: i64) -> ApiResult<(Option<String>, String)> {
    sqlx::query_as("SELECT hash, state FROM downloads WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("download"))
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeState {
    Idle,

    Wanted,
    Grabbed,

    Missing,

    Done,
    Skipped,
}

impl EpisodeState {
    fn parse(s: &str) -> Self {
        match s {
            "wanted" => Self::Wanted,
            "grabbed" => Self::Grabbed,
            "missing" => Self::Missing,
            "done" => Self::Done,
            "skipped" => Self::Skipped,
            _ => Self::Idle,
        }
    }
}

#[derive(SimpleObject)]
pub struct EpisodeDownload {
    stage: TorrentStage,
    progress: f32,
    download_rate: i64,
    eta: Option<i64>,
}

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct CalendarEntry {
    series_id: i64,

    #[graphql(skip)]
    item_id: Option<i64>,

    title: Option<Title>,
    library: String,
    show: String,
    poster: Option<String>,
    backdrop: Option<String>,
    monitor: Monitor,
    season: i64,
    episode: i64,
    absolute: Option<i64>,

    name: Option<String>,
    air_at: i64,
    state: EpisodeState,

    video: Option<Video>,
    download: Option<EpisodeDownload>,
}

#[derive(SimpleObject)]
pub struct WantedEpisode {
    series_id: i64,
    title: Option<Title>,
    show: String,
    season: i64,
    episode: i64,
    name: Option<String>,
    air_at: Option<i64>,
    aired: bool,
    state: EpisodeState,

    attempts: i64,
    searched_at: Option<i64>,
    next_search: Option<i64>,
}

#[ComplexObject]
impl CalendarEntry {
    /// The most vivid colour of the backdrop as "r g b", empty until it's been worked out.
    async fn backdrop_tint(&self, ctx: &Context<'_>) -> ApiResult<Option<String>> {
        let state = ctx.state();
        let Some(id) = self.item_id else { return Ok(None) };
        let Some(row) = library::item_row(state, id).await? else { return Ok(None) };

        let source = images::item_source(
            row.id,
            &row.path,
            "backdrop",
            row.backdrop.as_deref(),
            row.custom_backdrop,
            row.artwork_version,
        );

        Ok(match source {
            Some(source) => state.tints.get(state, source).await,
            None => None,
        })
    }
}

pub struct SeriesEpisode(series::EpisodeView);

#[Object]
impl SeriesEpisode {
    async fn season(&self) -> i64 {
        self.0.season
    }

    async fn episode(&self) -> i64 {
        self.0.episode
    }

    async fn absolute(&self) -> Option<i64> {
        self.0.absolute
    }

    async fn name(&self) -> Option<&str> {
        self.0.title.as_deref()
    }

    async fn air_at(&self) -> Option<i64> {
        self.0.air_at
    }

    async fn aired(&self) -> bool {
        self.0.aired
    }

    async fn state(&self) -> EpisodeState {
        EpisodeState::parse(&self.0.state)
    }

    async fn attempts(&self) -> i64 {
        self.0.attempts
    }

    async fn searched_at(&self) -> Option<i64> {
        self.0.searched_at
    }

    async fn next_search(&self) -> Option<i64> {
        self.0.next_search
    }

    async fn download_id(&self) -> Option<i64> {
        self.0.download_id
    }

    async fn video(&self, ctx: &Context<'_>) -> ApiResult<Option<Video>> {
        match self.0.media_id {
            Some(id) => Video::load(ctx.state(), &ctx.access()?, id).await,
            None => Ok(None),
        }
    }
}

pub struct Series {
    row: series::Row,
    view: OnceCell<series::View>,
}

impl Series {
    fn new(row: series::Row) -> Self {
        Self { row, view: OnceCell::new() }
    }

    pub(super) async fn for_path(state: &AppState, path: &str) -> ApiResult<Option<Self>> {
        Ok(series::by_path(state, path).await?.map(Self::new))
    }

    async fn load(state: &AppState, user: &User, id: i64) -> ApiResult<Self> {
        let row = series::get(state, id).await.map_err(|_| ApiError::not_found("show"))?;

        if !user.permissions.can_see(&row.library) {
            return Err(ApiError::not_found("show"));
        }

        Ok(Self::new(row))
    }

    async fn view(&self, state: &AppState) -> ApiResult<&series::View> {
        Ok(self.view.get_or_try_init(|| series::view(state, self.row.clone())).await?)
    }

    async fn full<'a>(&'a self, ctx: &Context<'_>) -> ApiResult<&'a series::View> {
        ctx.allowed(|p| p.manage_shows)?;
        self.view(ctx.state()).await
    }
}

#[Object]
impl Series {
    async fn id(&self) -> i64 {
        self.row.id
    }

    async fn monitor(&self) -> Monitor {
        parse_monitor(&self.row.monitor)
    }

    async fn status(&self) -> Option<&str> {
        self.row.status.as_deref()
    }

    async fn next(&self, ctx: &Context<'_>) -> ApiResult<Option<SeriesEpisode>> {
        let view = self.view(ctx.state()).await?;
        Ok(view.next.as_ref().map(|e| SeriesEpisode(clone_episode(e))))
    }

    async fn title(&self, ctx: &Context<'_>) -> ApiResult<Option<Title>> {
        match self.full(ctx).await?.item_id {
            Some(id) => Title::load(ctx.state(), &ctx.access()?, id).await,
            None => Ok(None),
        }
    }

    async fn library(&self, ctx: &Context<'_>) -> ApiResult<&str> {
        Ok(&self.full(ctx).await?.library)
    }

    async fn managed(&self, ctx: &Context<'_>) -> ApiResult<bool> {
        Ok(self.full(ctx).await?.managed)
    }

    async fn path(&self, ctx: &Context<'_>) -> ApiResult<&str> {
        Ok(&self.full(ctx).await?.path)
    }

    async fn name(&self, ctx: &Context<'_>) -> ApiResult<&str> {
        Ok(&self.full(ctx).await?.title)
    }

    async fn year(&self, ctx: &Context<'_>) -> ApiResult<Option<i64>> {
        Ok(self.full(ctx).await?.year)
    }

    async fn poster(&self, ctx: &Context<'_>) -> ApiResult<Option<&str>> {
        Ok(self.full(ctx).await?.poster.as_deref())
    }

    async fn overview(&self, ctx: &Context<'_>) -> ApiResult<Option<&str>> {
        Ok(self.full(ctx).await?.overview.as_deref())
    }

    async fn provider(&self, ctx: &Context<'_>) -> ApiResult<Option<Provider>> {
        Ok(super::library::parse_provider(self.full(ctx).await?.provider.as_deref()))
    }

    async fn provider_id(&self, ctx: &Context<'_>) -> ApiResult<Option<&str>> {
        Ok(self.full(ctx).await?.provider_id.as_deref())
    }

    async fn profile(&self, ctx: &Context<'_>) -> ApiResult<Option<&str>> {
        Ok(self.full(ctx).await?.profile.as_deref())
    }

    async fn effective_profile(&self, ctx: &Context<'_>) -> ApiResult<&str> {
        Ok(&self.full(ctx).await?.effective_profile)
    }

    async fn sources(&self, ctx: &Context<'_>) -> ApiResult<&[String]> {
        Ok(&self.full(ctx).await?.sources)
    }

    async fn groups(&self, ctx: &Context<'_>) -> ApiResult<&[String]> {
        Ok(&self.full(ctx).await?.groups)
    }

    async fn aliases(&self, ctx: &Context<'_>) -> ApiResult<&[String]> {
        Ok(&self.full(ctx).await?.aliases)
    }

    async fn known_as(&self, ctx: &Context<'_>) -> ApiResult<&[String]> {
        Ok(&self.full(ctx).await?.known_as)
    }

    async fn numbering(&self, ctx: &Context<'_>) -> ApiResult<Numbering> {
        Ok(self.full(ctx).await?.numbering)
    }

    async fn naming(&self, ctx: &Context<'_>) -> ApiResult<Option<&str>> {
        Ok(self.full(ctx).await?.naming.as_deref())
    }

    async fn style(&self, ctx: &Context<'_>) -> ApiResult<&Style> {
        Ok(&self.full(ctx).await?.style)
    }

    async fn seeding(&self, ctx: &Context<'_>) -> ApiResult<Option<&Seeding>> {
        Ok(self.full(ctx).await?.seeding.as_ref())
    }

    async fn schedule_at(&self, ctx: &Context<'_>) -> ApiResult<Option<i64>> {
        Ok(self.full(ctx).await?.schedule_at)
    }

    async fn added_at(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        Ok(self.full(ctx).await?.added_at)
    }

    async fn counts(&self, ctx: &Context<'_>) -> ApiResult<&Counts> {
        Ok(&self.full(ctx).await?.counts)
    }

    async fn episodes(&self, ctx: &Context<'_>) -> ApiResult<Vec<SeriesEpisode>> {
        ctx.allowed(|p| p.manage_shows)?;
        Ok(series::episodes(ctx.state(), &self.row).await?.into_iter().map(SeriesEpisode).collect())
    }

    async fn releases(
        &self,
        ctx: &Context<'_>,
        season: u32,
        #[graphql(default)] episodes: Vec<u32>,
        query: Option<String>,
    ) -> ApiResult<Vec<Candidate>> {
        ctx.allowed(|p| p.manage_shows)?;
        automation::search(ctx.state(), self.row.id, season, &episodes, query.as_deref()).await.map_err(bad)
    }

    async fn naming_preview(
        &self,
        ctx: &Context<'_>,
        file: String,
        folder: Option<String>,
    ) -> ApiResult<NamingPreview> {
        ctx.allowed(|p| p.manage_shows)?;
        let state = ctx.state();
        let row = &self.row;
        let style = series::style(state, row).await?;

        let eps: Vec<(i64, i64, Option<String>)> =
            sqlx::query_as("SELECT season, episode, title FROM episodes WHERE series_id = ? AND season > 0 ORDER BY air_at DESC LIMIT 3")
                .bind(row.id)
                .fetch_all(&state.db)
                .await?;

        let eps = if eps.is_empty() { vec![(1, 1, None)] } else { eps };

        let folder_name =
            std::path::Path::new(&row.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        let (show, year) = crate::library::parse::title_and_year(&folder_name);
        let folder = folder.unwrap_or(style.folder);

        let samples = eps
            .into_iter()
            .map(|(s, e, title)| {
                let v = naming::Values {
                    show: show.clone(),
                    year: year.or(row.year),
                    season: s as u32,
                    episode: e as u32,
                    title,
                    group: Some("SubsPlease".into()),
                    quality: Some("1080p".into()),
                    codec: Some("HEVC".into()),
                    original: "[SubsPlease] Original Release Name (1080p)".into(),
                };

                format!("{}/{}.mkv", naming::render(&folder, &v), naming::render(&file, &v))
            })
            .collect();

        Ok(NamingPreview { samples, error: naming::validate(&file).err() })
    }
}

fn clone_episode(e: &series::EpisodeView) -> series::EpisodeView {
    series::EpisodeView {
        season: e.season,
        episode: e.episode,
        absolute: e.absolute,
        title: e.title.clone(),
        air_at: e.air_at,
        aired: e.aired,
        state: e.state.clone(),
        attempts: e.attempts,
        searched_at: e.searched_at,
        next_search: e.next_search,
        download_id: e.download_id,
        media_id: e.media_id,
    }
}

#[derive(SimpleObject)]
pub struct NamingPreview {
    samples: Vec<String>,

    error: Option<String>,
}

#[Object(name = "ReleaseCandidate")]
impl Candidate {
    async fn release(&self) -> &Release {
        &self.release
    }

    async fn attributes(&self) -> &crate::automation::release::Attributes {
        &self.attributes
    }

    async fn episodes(&self) -> Vec<EpisodeNumber> {
        self.episodes.iter().copied().map(Into::into).collect()
    }

    async fn batch(&self) -> bool {
        self.batch
    }

    async fn verdict(&self) -> &crate::automation::profile::Verdict {
        &self.verdict
    }
}

#[derive(InputObject)]
pub struct NewSeries {
    library: String,
    provider: Provider,
    provider_id: String,
    name: String,
    year: Option<i64>,
    poster: Option<String>,
    overview: Option<String>,

    monitor: Option<Monitor>,
    profile: Option<String>,
}

#[derive(InputObject)]
pub struct SeriesPatch {
    monitor: Option<Monitor>,
    profile: MaybeUndefined<String>,
    sources: Option<Vec<String>>,
    groups: Option<Vec<String>>,
    aliases: Option<Vec<String>>,
    numbering: Option<Numbering>,
    naming: MaybeUndefined<String>,
    seeding: MaybeUndefined<Seeding>,
}

fn maybe<T>(m: MaybeUndefined<T>) -> Option<Option<T>> {
    match m {
        MaybeUndefined::Undefined => None,
        MaybeUndefined::Null => Some(None),
        MaybeUndefined::Value(v) => Some(Some(v)),
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Low,
}

#[derive(SimpleObject)]
pub struct RenameSuggestion {
    id: i64,
    library: String,

    managed: bool,

    root: Option<String>,
    src: String,
    dst: String,
    reason: String,
    confidence: Confidence,
}

#[derive(SimpleObject)]
pub struct FileOperation {
    kind: String,
    src: Option<String>,
    dst: String,
}

#[derive(SimpleObject)]
pub struct HistoryBatch {
    batch: String,
    label: String,
    at: i64,
    count: i64,
    undone: bool,

    operations: Vec<FileOperation>,
}

#[derive(SimpleObject)]
pub struct Request {
    id: i64,

    user: Option<User>,
    provider: Provider,
    provider_id: String,
    name: String,
    year: Option<i64>,
    poster: Option<String>,
    overview: Option<String>,
    library: Option<String>,
    state: RequestState,
    series_id: Option<i64>,
    title: Option<Title>,

    note: Option<String>,
    created_at: i64,
    decided_at: Option<i64>,

    have: Option<i64>,

    aired: Option<i64>,
}

async fn requests(ctx: &Context<'_>, id: Option<i64>) -> ApiResult<Vec<Request>> {
    let (state, user) = (ctx.state(), ctx.user()?);
    let access = ctx.access()?;

    type Row = (
        i64,
        i64,
        String,
        String,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<i64>,
        Option<String>,
        i64,
        Option<i64>,
    );

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT r.id, r.user_id, r.provider, r.provider_id, r.title, r.year, r.poster, r.overview, r.library, r.state,
                r.series_id, r.note, r.created_at, r.decided_at
         FROM requests r WHERE (?1 OR r.user_id = ?2) AND (?3 IS NULL OR r.id = ?3) ORDER BY r.created_at DESC LIMIT 300",
    )
    .bind(user.permissions.manage_requests)
    .bind(user.id)
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();

    for (
        id,
        user_id,
        provider,
        provider_id,
        title,
        year,
        poster,
        overview,
        library,
        st,
        series_id,
        note,
        created_at,
        decided_at,
    ) in rows
    {
        let progress = match series_id {
            Some(s) => sqlx::query_as::<_, (i64, i64)>(
                "SELECT COALESCE(SUM(state = 'done'), 0), COUNT(*) FROM episodes WHERE series_id = ? AND season > 0 AND (aired = 1 OR air_at <= ?)",
            )
            .bind(s)
            .bind(now())
            .fetch_optional(&state.db)
            .await?,
            None => None,
        };

        let item = match series_id {
            Some(s) => match series::get(state, s).await {
                Ok(row) => series::item_id(state, &row.path).await?,
                Err(_) => None,
            },
            None => None,
        };

        let title_obj = match item {
            Some(i) => Title::load(state, &access, i).await?,
            None => None,
        };

        out.push(Request {
            id,
            user: auth::load_user(state, user_id).await?,
            provider: if provider == "anilist" { Provider::Anilist } else { Provider::Tmdb },
            provider_id,
            name: title,
            year,
            poster,
            overview,
            library,
            state: RequestState::parse(&st),
            series_id,
            title: title_obj,
            note,
            created_at,
            decided_at,
            have: progress.map(|p| p.0),
            aired: progress.map(|p| p.1),
        });
    }

    Ok(out)
}

async fn request(ctx: &Context<'_>, id: i64) -> ApiResult<Request> {
    requests(ctx, Some(id)).await?.pop().ok_or_else(|| ApiError::not_found("request"))
}

#[derive(InputObject)]
pub struct NewRequest {
    library: String,
    provider_id: String,
    name: String,
    year: Option<i64>,
    poster: Option<String>,
    overview: Option<String>,
    note: Option<String>,
}

fn library_provider(state: &AppState, library: &str) -> ApiResult<Provider> {
    state.config.current().library(library).ok_or_else(|| ApiError::not_found("library"))?.metadata_provider.ok_or_else(
        || ApiError::bad_request(format!("{library} doesn't use AniList or TMDB, so there's nothing to search")),
    )
}

async fn approve(state: &Arc<AppState>, id: i64, admin: Option<i64>, library: Option<String>) -> ApiResult<i64> {
    let (provider, provider_id, title, year, poster, overview, req_library): (
        String,
        String,
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT provider, provider_id, title, year, poster, overview, library FROM requests WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("request"))?;

    let library = library.or(req_library).ok_or_else(|| ApiError::bad_request("pick a library for it"))?;
    let config = state.config.current();

    let series_id = series::create(
        state,
        series::NewSeries {
            library,
            provider: if provider == "anilist" { Provider::Anilist } else { Provider::Tmdb },
            provider_id,
            title,
            year,
            poster,
            overview,
            monitor: Some(if config.requests.monitor == Monitor::None {
                Monitor::Missing
            } else {
                config.requests.monitor
            }),
            profile: None,
        },
    )
    .await
    .map_err(bad)?;

    sqlx::query("UPDATE requests SET state = 'approved', series_id = ?, decided_at = ?, decided_by = ? WHERE id = ?")
        .bind(series_id)
        .bind(now())
        .bind(admin)
        .bind(id)
        .execute(&state.db)
        .await?;

    state.events.send(Event::RequestsChanged);
    Ok(series_id)
}

async fn tell_requester(state: &AppState, id: i64, admin: &User, decision: Result<(), Option<String>>) {
    let row: Option<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT user_id, title, poster FROM requests WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

    let Some((user_id, title, poster)) = row else { return };
    let approved = decision.is_ok();

    if user_id == admin.id {
        return;
    }

    notifications::send(
        state,
        &[user_id],
        notifications::New {
            kind: if approved { "requestApproved" } else { "requestDeclined" },
            title: if approved {
                format!("{} approved {title}", admin.username)
            } else {
                format!("{} declined {title}", admin.username)
            },
            body: match decision {
                Ok(()) => Some("It'll show up once the first episodes are in.".into()),
                Err(reason) => reason.filter(|r| !r.trim().is_empty()),
            },
            image: poster,
            link: Some("/requests".into()),
            actor_id: Some(admin.id),
            ..Default::default()
        },
    )
    .await;
}

#[derive(Default)]
pub struct AutomationQuery;

#[Object]
impl AutomationQuery {
    async fn downloads(&self, ctx: &Context<'_>) -> ApiResult<Vec<Download>> {
        ctx.allowed(|p| p.downloads)?;
        downloads(ctx, None).await
    }

    async fn download(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<Download>> {
        ctx.allowed(|p| p.downloads)?;
        Ok(downloads(ctx, Some(id)).await?.pop())
    }

    async fn download_engine(&self, ctx: &Context<'_>) -> ApiResult<DownloadEngine> {
        let state = ctx.state();
        ctx.allowed(|p| p.downloads)?;
        let o = state.automation.engine.overview(state);

        Ok(DownloadEngine {
            version: o.version,
            download_rate: o.download_rate,
            upload_rate: o.upload_rate,
            active: o.active,
            kill_switch: o.kill_switch,
            listening: o.listening,
            listen_error: o.listen_error,
            slow_hours: o.slow_hours,
            download_path: o.download_path.display().to_string(),
        })
    }

    async fn calendar(&self, ctx: &Context<'_>, from: Option<i64>, to: Option<i64>) -> ApiResult<Vec<CalendarEntry>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let access = ctx.access()?;
        let from = from.unwrap_or_else(|| now() - 7 * 86400);
        let to = to.unwrap_or_else(|| now() + 14 * 86400);
        let libraries = user.libraries(state).await?;

        type Row =
            (i64, String, String, String, Option<String>, String, i64, i64, Option<String>, i64, String, Option<i64>);

        let rows: Vec<Row> = sqlx::query_as(
            "SELECT s.id, s.library, s.title, s.path, s.poster, s.monitor, e.season, e.episode, e.title, e.air_at, e.state, e.absolute
             FROM episodes e JOIN series s ON s.id = e.series_id
             WHERE e.air_at BETWEEN ? AND ? AND e.season > 0 ORDER BY e.air_at",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&state.db)
        .await?;

        let active: Vec<(i64, String, String)> = sqlx::query_as(
            "SELECT series_id, episodes, hash FROM downloads WHERE state = 'downloading' AND series_id IS NOT NULL AND hash IS NOT NULL",
        )
        .fetch_all(&state.db)
        .await?;

        let active: Vec<(i64, Vec<(i64, i64)>, String)> =
            active.into_iter().map(|(s, e, h)| (s, serde_json::from_str(&e).unwrap_or_default(), h)).collect();

        let mut out = Vec::new();

        for (id, library, show, path, poster, monitor, season, episode, name, air_at, ep_state, absolute) in rows {
            if !libraries.contains(&library) {
                continue;
            }

            let item_id = series::item_id(state, &path).await?;

            let media_id: Option<i64> = sqlx::query_scalar(
                "SELECT m.id FROM media m JOIN items i ON i.id = m.item_id WHERE i.path = ? AND m.season = ? AND ? BETWEEN m.episode AND COALESCE(m.episode_end, m.episode)",
            )
            .bind(&path)
            .bind(season)
            .bind(episode)
            .fetch_optional(&state.db)
            .await?;

            let title = match item_id {
                Some(i) => Title::load(state, &access, i).await?,
                None => None,
            };

            let video = match media_id {
                Some(m) => Video::load(state, &access, m).await?,
                None => None,
            };

            out.push(CalendarEntry {
                series_id: id,
                item_id,
                poster: item_id.map(|i| format!("/api/images/item/{i}/poster")).or(poster),
                backdrop: item_id.map(|i| format!("/api/images/item/{i}/backdrop")),
                title,
                library,
                show,
                monitor: parse_monitor(&monitor),
                season,
                episode,
                absolute,
                name,
                air_at,
                state: EpisodeState::parse(&ep_state),
                video,
                download: active
                    .iter()
                    .find(|(s, eps, _)| *s == id && eps.contains(&(season, episode)))
                    .and_then(|(_, _, h)| state.automation.engine.status(h))
                    .map(|st| EpisodeDownload {
                        stage: TorrentStage::of(&st),
                        progress: st.progress,
                        download_rate: st.download_rate,
                        eta: eta(&st),
                    }),
            });
        }

        Ok(out)
    }

    async fn wanted(&self, ctx: &Context<'_>) -> ApiResult<Vec<WantedEpisode>> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.manage_shows)?;
        let access = ctx.access()?;
        let libraries = user.libraries(state).await?;

        type Row = (
            i64,
            String,
            String,
            String,
            i64,
            i64,
            Option<String>,
            Option<i64>,
            String,
            i64,
            Option<i64>,
            Option<i64>,
            bool,
        );

        let rows: Vec<Row> = sqlx::query_as(
            "SELECT s.id, s.library, s.title, s.path, e.season, e.episode, e.title, e.air_at, e.state, e.attempts, e.searched_at, e.next_search,
                    (e.aired = 1 OR COALESCE(e.air_at <= ?, 0)) AS aired
             FROM episodes e JOIN series s ON s.id = e.series_id
             WHERE e.state IN ('wanted', 'missing', 'grabbed') AND s.monitor != 'none'
             ORDER BY CASE e.state WHEN 'grabbed' THEN 0 WHEN 'wanted' THEN 1 ELSE 2 END, e.air_at DESC LIMIT 500",
        )
        .bind(now())
        .fetch_all(&state.db)
        .await?;

        let mut out = Vec::new();

        for (id, library, show, path, season, episode, name, air_at, st, attempts, searched_at, next_search, aired) in
            rows
        {
            if !libraries.contains(&library) {
                continue;
            }

            let title = match series::item_id(state, &path).await? {
                Some(i) => Title::load(state, &access, i).await?,
                None => None,
            };

            out.push(WantedEpisode {
                series_id: id,
                title,
                show,
                season,
                episode,
                name,
                air_at,
                aired,
                state: EpisodeState::parse(&st),
                attempts,
                searched_at,
                next_search,
            });
        }

        Ok(out)
    }

    async fn all_series(&self, ctx: &Context<'_>) -> ApiResult<Vec<Series>> {
        let user = ctx.allowed(|p| p.manage_shows)?;

        let rows: Vec<series::Row> =
            sqlx::query_as("SELECT * FROM series ORDER BY title COLLATE NOCASE").fetch_all(&ctx.state().db).await?;

        Ok(rows.into_iter().filter(|r| user.permissions.can_see(&r.library)).map(Series::new).collect())
    }

    async fn series(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<Series>> {
        let user = ctx.allowed(|p| p.manage_shows)?;

        match Series::load(ctx.state(), user, id).await {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.status == axum::http::StatusCode::NOT_FOUND => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn detect_source(&self, ctx: &Context<'_>, url: String, api_key: Option<String>) -> ApiResult<Detected> {
        ctx.admin()?;
        ctx.state().automation.sources.detect(&url, api_key.as_deref()).await.map_err(bad)
    }

    async fn rename_suggestions(&self, ctx: &Context<'_>) -> ApiResult<Vec<RenameSuggestion>> {
        let state = ctx.state();
        ctx.admin()?;
        let config = state.config.current();

        let rows: Vec<(i64, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, library, src, dst, reason, confidence FROM rename_suggestions WHERE state = 'pending'
             ORDER BY library, confidence, src",
        )
        .fetch_all(&state.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, library, src, dst, reason, confidence)| {
                let lib = config.library(&library);

                RenameSuggestion {
                    id,
                    managed: lib.is_some_and(|l| l.managed),
                    root: lib
                        .and_then(|l| l.resolved_path(state.config.config_dir()).ok())
                        .map(|p| p.display().to_string()),
                    library,
                    src,
                    dst,
                    reason,
                    confidence: if confidence == "high" { Confidence::High } else { Confidence::Low },
                }
            })
            .collect())
    }

    async fn file_history(&self, ctx: &Context<'_>) -> ApiResult<Vec<HistoryBatch>> {
        let state = ctx.state();
        ctx.admin()?;

        let batches: Vec<(String, String, i64, i64, i64)> = sqlx::query_as(
            "SELECT batch, label, MAX(at), COUNT(*), SUM(undone_at IS NOT NULL) FROM file_ops GROUP BY batch ORDER BY MAX(at) DESC LIMIT 100",
        )
        .fetch_all(&state.db)
        .await?;

        let mut out = Vec::new();

        for (batch, label, at, count, undone) in batches {
            let ops: Vec<(String, Option<String>, String)> = sqlx::query_as(
                "SELECT kind, src, dst FROM file_ops WHERE batch = ? AND kind != 'mkdir' ORDER BY id LIMIT 20",
            )
            .bind(&batch)
            .fetch_all(&state.db)
            .await?;

            out.push(HistoryBatch {
                batch,
                label,
                at,
                count,
                undone: undone == count,
                operations: ops.into_iter().map(|(kind, src, dst)| FileOperation { kind, src, dst }).collect(),
            });
        }

        Ok(out)
    }

    async fn requests(&self, ctx: &Context<'_>) -> ApiResult<Vec<Request>> {
        requests(ctx, None).await
    }
}

async fn save_entry<T: serde::Serialize>(
    state: &AppState,
    list: List,
    index: Option<usize>,
    value: &T,
) -> ApiResult<Settings> {
    state.config.put_entry(list, index, value).await.map_err(ApiError::bad_request)?;
    state.automation.wake();
    Ok(Settings::now(state))
}

fn source_index(state: &AppState, name: &str) -> ApiResult<usize> {
    state.config.current().sources.iter().position(|s| s.name == name).ok_or_else(|| ApiError::not_found("source"))
}

fn profile_index(state: &AppState, name: &str) -> ApiResult<usize> {
    state.config.current().profiles.iter().position(|p| p.name == name).ok_or_else(|| ApiError::not_found("profile"))
}

#[derive(Default)]
pub struct AutomationMutation;

#[Object]
impl AutomationMutation {
    async fn grab_release(
        &self,
        ctx: &Context<'_>,
        release: Release,
        series_id: Option<i64>,
        #[graphql(default)] episodes: Vec<EpisodeNumber>,
    ) -> ApiResult<Download> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.downloads)?;

        let grab = Grab {
            release,
            series_id,
            episodes: episodes.into_iter().map(Into::into).collect(),
            requested_by: Some(user.id),
        };

        let id = automation::grab(state, grab).await.map_err(bad)?;
        downloads(ctx, Some(id)).await?.pop().ok_or_else(|| ApiError::not_found("download"))
    }

    async fn pause_downloads(&self, ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<Vec<Download>> {
        let state = ctx.state();
        ctx.allowed(|p| p.downloads)?;

        for &id in &ids {
            let (hash, _) = download_hash(state, id).await?;
            state.automation.engine.pause(&hash.unwrap_or_default());
            sqlx::query("UPDATE downloads SET state = 'paused' WHERE id = ?").bind(id).execute(&state.db).await?;
        }

        state.events.send(Event::DownloadsChanged);
        changed(ctx, ids).await
    }

    async fn resume_downloads(&self, ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<Vec<Download>> {
        let state = ctx.state();
        ctx.allowed(|p| p.downloads)?;

        for &id in &ids {
            let (hash, _) = download_hash(state, id).await?;
            state.automation.engine.resume(&hash.unwrap_or_default());

            let finished: Option<i64> = sqlx::query_scalar("SELECT finished_at FROM downloads WHERE id = ?")
                .bind(id)
                .fetch_one(&state.db)
                .await?;

            let next = if finished.is_some() { "seeding" } else { "downloading" };
            sqlx::query("UPDATE downloads SET state = ? WHERE id = ?").bind(next).bind(id).execute(&state.db).await?;
        }

        state.events.send(Event::DownloadsChanged);
        changed(ctx, ids).await
    }

    async fn recheck_downloads(&self, ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<Vec<Download>> {
        let state = ctx.state();
        ctx.allowed(|p| p.downloads)?;

        for &id in &ids {
            let (hash, _) = download_hash(state, id).await?;
            state.automation.engine.recheck(&hash.unwrap_or_default());
        }

        state.events.send(Event::DownloadsChanged);
        changed(ctx, ids).await
    }

    async fn import_download(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Download> {
        let state = ctx.state();
        ctx.allowed(|p| p.downloads)?;
        let (_, current) = download_hash(state, id).await?;

        if !matches!(current.as_str(), "seeding" | "paused" | "done") {
            return Err(ApiError::bad_request("it hasn't finished downloading yet"));
        }

        sqlx::query("UPDATE downloads SET import_state = 'pending', import_error = NULL WHERE id = ?")
            .bind(id)
            .execute(&state.db)
            .await?;

        automation::import::run(state, id).await.map_err(bad)?;
        state.events.send(Event::DownloadsChanged);
        changed(ctx, vec![id]).await?.pop().ok_or_else(|| ApiError::not_found("download"))
    }

    async fn remove_downloads(
        &self,
        ctx: &Context<'_>,
        ids: Vec<i64>,
        #[graphql(default)] delete_files: bool,
    ) -> ApiResult<Vec<i64>> {
        let state = ctx.state();
        ctx.allowed(|p| p.downloads)?;

        for &id in &ids {
            let (hash, _) = download_hash(state, id).await?;

            if let Some(h) = hash {
                state.automation.engine.remove(&h, delete_files);
            }

            sqlx::query("UPDATE downloads SET state = 'removed', removed_at = ? WHERE id = ?")
                .bind(now())
                .bind(id)
                .execute(&state.db)
                .await?;
        }

        state.events.send(Event::DownloadsChanged);
        state.automation.wake();
        Ok(ids)
    }

    async fn delete_downloaded(&self, ctx: &Context<'_>, series_id: i64, season: Option<u32>) -> ApiResult<UndoReport> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.downloads && p.manage_shows)?;
        Series::load(state, user, series_id).await?;
        let show = series::get(state, series_id).await.map_err(bad)?;

        let downloads: Vec<(i64, Option<String>, String, String)> =
            sqlx::query_as("SELECT id, hash, state, episodes FROM downloads WHERE series_id = ?")
                .bind(series_id)
                .fetch_all(&state.db)
                .await?;

        let mut report = UndoReport::default();

        for (id, hash, current, episodes) in downloads {
            let seasons: Vec<u32> = serde_json::from_str::<Vec<(u32, u32)>>(&episodes)
                .unwrap_or_default()
                .into_iter()
                .map(|(s, _)| s)
                .collect();

            if season.is_some_and(|n| !seasons.contains(&n)) {
                continue;
            }

            let whole = season.is_none_or(|n| seasons.iter().all(|&s| s == n));

            if whole && !matches!(current.as_str(), "removed" | "done") {
                if let Some(h) = &hash {
                    state.automation.engine.remove(h, true);
                }

                sqlx::query("UPDATE downloads SET state = 'removed', removed_at = ? WHERE id = ?")
                    .bind(now())
                    .bind(id)
                    .execute(&state.db)
                    .await?;
            }

            let placed = fsops::delete_placed(&state.db, &format!("import-{id}"), season).await.map_err(bad)?;
            report.undone += placed.undone;
            report.problems.extend(placed.problems);
        }

        if season.is_none() {
            series::set_monitor(state, series_id, Monitor::None).await.map_err(bad)?;
        }

        sqlx::query(
            "UPDATE episodes SET state = 'skipped', download_id = NULL, next_search = NULL
             WHERE series_id = ? AND (? IS NULL OR season = ?)",
        )
        .bind(series_id)
        .bind(season)
        .bind(season)
        .execute(&state.db)
        .await?;

        state.scanner.request(&show.library);
        state.events.send(Event::DownloadsChanged);
        state.events.send(Event::SeriesChanged { series_id });
        state.automation.wake();
        Ok(report)
    }

    async fn look_for_again(
        &self,
        ctx: &Context<'_>,
        series_id: i64,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> ApiResult<bool> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.manage_shows)?;
        Series::load(state, user, series_id).await?;

        sqlx::query(
            "UPDATE episodes SET state = 'idle'
             WHERE series_id = ?1 AND state = 'skipped' AND (?2 IS NULL OR season = ?2) AND (?3 IS NULL OR episode = ?3)",
        )
        .bind(series_id)
        .bind(season)
        .bind(episode)
        .execute(&state.db)
        .await?;

        state.events.send(Event::SeriesChanged { series_id });
        state.automation.wake();
        Ok(true)
    }

    async fn add_series(&self, ctx: &Context<'_>, input: NewSeries) -> ApiResult<Series> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.manage_shows)?;
        user.can_access(state, &input.library).await.map_err(|_| ApiError::not_found("library"))?;

        let id = series::create(
            state,
            series::NewSeries {
                library: input.library,
                provider: input.provider,
                provider_id: input.provider_id,
                title: input.name,
                year: input.year,
                poster: input.poster,
                overview: input.overview,
                monitor: input.monitor,
                profile: input.profile,
            },
        )
        .await
        .map_err(bad)?;

        Series::load(state, user, id).await
    }

    async fn manage_title(&self, ctx: &Context<'_>, title_id: i64) -> ApiResult<Series> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.manage_shows)?;
        Title::load(state, &ctx.access()?, title_id).await?.ok_or_else(|| ApiError::not_found("title"))?;
        let id = series::ensure_for_item(state, title_id).await.map_err(bad)?;
        let row = series::get(state, id).await?;

        if row.schedule_at.is_none()
            && row.provider_id.is_some()
            && let Err(e) = series::refresh_schedule(state, id).await
        {
            tracing::warn!("schedule for {}: {e:#}", row.title);
        }

        Series::load(state, user, id).await
    }

    async fn update_series(&self, ctx: &Context<'_>, id: i64, patch: SeriesPatch) -> ApiResult<Series> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.manage_shows)?;
        Series::load(state, user, id).await?;

        let p = series::Patch {
            monitor: patch.monitor,
            profile: maybe(patch.profile),
            sources: patch.sources,
            groups: patch.groups,
            aliases: patch.aliases,
            numbering: patch.numbering,
            naming: maybe(patch.naming),
            seeding: maybe(patch.seeding),
        };

        series::patch(state, id, p).await.map_err(bad)?;
        Series::load(state, user, id).await
    }

    async fn remove_series(&self, ctx: &Context<'_>, id: i64) -> ApiResult<i64> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.manage_shows)?;
        Series::load(state, user, id).await?;
        sqlx::query("DELETE FROM series WHERE id = ?").bind(id).execute(&state.db).await?;
        state.events.send(Event::SeriesChanged { series_id: id });
        Ok(id)
    }

    async fn refresh_series_schedule(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Series> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.manage_shows)?;
        Series::load(state, user, id).await?;
        series::refresh_schedule(state, id).await.map_err(bad)?;
        state.automation.wake();
        Series::load(state, user, id).await
    }

    async fn add_source(&self, ctx: &Context<'_>, input: Source) -> ApiResult<Settings> {
        ctx.admin()?;
        save_entry(ctx.state(), List::Sources, None, &input).await
    }

    async fn update_source(&self, ctx: &Context<'_>, name: String, input: Source) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        let index = source_index(state, &name)?;
        let settings = save_entry(state, List::Sources, Some(index), &input).await?;

        if name != input.name {
            let rows: Vec<(i64, String)> =
                sqlx::query_as("SELECT id, sources FROM series WHERE sources != '[]'").fetch_all(&state.db).await?;

            for (id, json) in rows {
                let list: Vec<String> =
                    series::list(&json).into_iter().map(|n| if n == name { input.name.clone() } else { n }).collect();

                sqlx::query("UPDATE series SET sources = ? WHERE id = ?")
                    .bind(serde_json::to_string(&list).unwrap())
                    .bind(id)
                    .execute(&state.db)
                    .await?;
            }
        }

        Ok(settings)
    }

    async fn remove_source(&self, ctx: &Context<'_>, name: String) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        let index = source_index(state, &name)?;
        state.config.remove_entry(List::Sources, index).await.map_err(ApiError::bad_request)?;
        Ok(Settings::now(state))
    }

    async fn add_profile(&self, ctx: &Context<'_>, input: Profile) -> ApiResult<Settings> {
        ctx.admin()?;
        save_entry(ctx.state(), List::Profiles, None, &input).await
    }

    async fn update_profile(&self, ctx: &Context<'_>, name: String, input: Profile) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        let index = profile_index(state, &name)?;
        let settings = save_entry(state, List::Profiles, Some(index), &input).await?;

        if name != input.name {
            sqlx::query("UPDATE series SET profile = ? WHERE profile = ?")
                .bind(&input.name)
                .bind(&name)
                .execute(&state.db)
                .await?;
        }

        Ok(settings)
    }

    async fn remove_profile(&self, ctx: &Context<'_>, name: String) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        let index = profile_index(state, &name)?;
        let config = state.config.current();

        if let Some(lib) = config.libraries.iter().find(|l| l.profile.as_deref() == Some(name.as_str())) {
            return Err(ApiError::bad_request(format!(
                "{} uses this profile; pick another one for it first",
                lib.name
            )));
        }

        sqlx::query("UPDATE series SET profile = NULL WHERE profile = ?").bind(&name).execute(&state.db).await?;
        state.config.remove_entry(List::Profiles, index).await.map_err(ApiError::bad_request)?;
        Ok(Settings::now(state))
    }

    async fn refresh_rename_suggestions(&self, ctx: &Context<'_>) -> ApiResult<bool> {
        let state = ctx.state();
        ctx.admin()?;

        for lib in &state.config.current().libraries {
            renames::refresh(state, &lib.name).await?;
        }

        Ok(true)
    }

    async fn apply_renames(&self, ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<Applied> {
        ctx.admin()?;
        Ok(renames::apply(ctx.state(), &ids).await?)
    }

    async fn dismiss_renames(&self, ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<bool> {
        let state = ctx.state();
        ctx.admin()?;

        for id in ids {
            sqlx::query("UPDATE rename_suggestions SET state = 'dismissed' WHERE id = ?")
                .bind(id)
                .execute(&state.db)
                .await?;
        }

        state.events.send(Event::RenamesChanged);
        Ok(true)
    }

    async fn undo_file_changes(&self, ctx: &Context<'_>, batch: String) -> ApiResult<UndoReport> {
        let state = ctx.state();
        ctx.admin()?;
        let report = fsops::undo(&state.db, &batch).await?;

        for l in &state.config.current().libraries {
            state.scanner.request(&l.name);
        }

        Ok(report)
    }

    async fn create_request(&self, ctx: &Context<'_>, input: NewRequest) -> ApiResult<Request> {
        let state = ctx.state();
        let user = ctx.user()?;
        let perms = &user.permissions;

        if !perms.request {
            return Err(ApiError::forbidden());
        }

        user.can_access(state, &input.library).await?;
        let provider = library_provider(state, &input.library)?;

        if perms.request_limit > 0 && !perms.manage_requests {
            let open: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requests WHERE user_id = ? AND state = 'pending'")
                .bind(user.id)
                .fetch_one(&state.db)
                .await?;

            if open >= perms.request_limit as i64 {
                return Err(ApiError::bad_request(format!(
                    "you already have {open} requests waiting; that's the limit"
                )));
            }
        }

        let existing: Option<i64> =
            sqlx::query_scalar("SELECT id FROM requests WHERE provider = ? AND provider_id = ? AND state = 'pending'")
                .bind(provider.as_str())
                .bind(&input.provider_id)
                .fetch_optional(&state.db)
                .await?;

        if existing.is_some() {
            return Err(ApiError::conflict("someone already asked for this one"));
        }

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO requests (user_id, provider, provider_id, title, year, poster, overview, library, note, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user.id)
        .bind(provider.as_str())
        .bind(&input.provider_id)
        .bind(&input.name)
        .bind(input.year)
        .bind(&input.poster)
        .bind(&input.overview)
        .bind(&input.library)
        .bind(&input.note)
        .bind(now())
        .fetch_one(&state.db)
        .await?;

        if perms.auto_approve {
            approve(state, id, None, None).await?;
        } else {
            let managers = notifications::everyone_who(state, |p| p.manage_requests).await;
            let managers: Vec<i64> = managers.into_iter().filter(|&m| m != user.id).collect();

            notifications::send(
                state,
                &managers,
                notifications::New {
                    kind: "request",
                    title: format!("{} requested {}", user.username, input.name),
                    body: input
                        .note
                        .clone()
                        .filter(|n| !n.trim().is_empty())
                        .or_else(|| Some(format!("For {}.", input.library))),
                    image: input.poster.clone(),
                    link: Some("/requests".into()),
                    actor_id: Some(user.id),
                    ..Default::default()
                },
            )
            .await;
        }

        state.events.send(Event::RequestsChanged);
        request(ctx, id).await
    }

    async fn approve_request(&self, ctx: &Context<'_>, id: i64, library: Option<String>) -> ApiResult<Request> {
        let state = ctx.state();
        let admin = ctx.allowed(|p| p.manage_requests)?;

        if let Some(library) = &library {
            admin.can_access(state, library).await.map_err(|_| ApiError::not_found("library"))?;
        }

        approve(state, id, Some(admin.id), library).await?;
        tell_requester(state, id, admin, Ok(())).await;
        request(ctx, id).await
    }

    async fn decline_request(&self, ctx: &Context<'_>, id: i64, note: Option<String>) -> ApiResult<Request> {
        let state = ctx.state();
        let admin = ctx.allowed(|p| p.manage_requests)?;

        sqlx::query("UPDATE requests SET state = 'declined', note = COALESCE(?, note), decided_at = ?, decided_by = ? WHERE id = ?")
            .bind(&note)
            .bind(now())
            .bind(admin.id)
            .bind(id)
            .execute(&state.db)
            .await?;

        tell_requester(state, id, admin, Err(note)).await;
        state.events.send(Event::RequestsChanged);
        request(ctx, id).await
    }

    async fn delete_request(&self, ctx: &Context<'_>, id: i64) -> ApiResult<i64> {
        let (state, user) = (ctx.state(), ctx.user()?);

        let owner: Option<(i64, String)> = sqlx::query_as("SELECT user_id, state FROM requests WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;

        let (owner, st) = owner.ok_or_else(|| ApiError::not_found("request"))?;

        if !user.permissions.manage_requests && (owner != user.id || st != "pending") {
            return Err(ApiError::forbidden());
        }

        sqlx::query("DELETE FROM requests WHERE id = ?").bind(id).execute(&state.db).await?;
        state.events.send(Event::RequestsChanged);
        Ok(id)
    }
}

async fn changed(ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<Vec<Download>> {
    let mut out = Vec::new();

    for id in ids {
        out.extend(downloads(ctx, Some(id)).await?);
    }

    Ok(out)
}
