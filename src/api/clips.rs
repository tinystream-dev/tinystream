// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use async_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::books::{self, BookView};
use super::library::{Title, TitleKind, Video, episode_label};
use super::schema::{Access, Ctx};
use crate::auth::{self, User};
use crate::books::catalog::{self, BookRow};
use crate::clips::{self, BITMAP_SUBTITLES};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::events::Event;
use crate::media::clip::HEIGHTS;
use crate::media::{burn, thumb};
use crate::notifications::{self, New};
use crate::state::AppState;

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    code: String,
    owner_id: i64,
    owner_name: String,
    title: String,
    media_id: Option<i64>,
    item_id: Option<i64>,
    source_path: String,
    source_size: i64,
    source_mtime: i64,
    show_title: String,
    kind: String,
    label: Option<String>,
    year: Option<i64>,
    range_start: f64,
    range_end: f64,
    audio: Option<i64>,
    subtitles: Option<String>,
    height: i64,
    half_rate: bool,
    state: String,
    error: Option<String>,
    bytes: Option<i64>,
    width_px: Option<i64>,
    height_px: Option<i64>,
    fps: Option<f64>,
    rendered_at: Option<i64>,
    public: bool,
    screenshot: bool,
    quote: Option<String>,
    author: Option<String>,
    book_length: Option<i64>,
    created_at: i64,
    shared_at: Option<i64>,
}

const SELECT: &str = "SELECT c.*, u.username AS owner_name, s.shared_at AS shared_at
    FROM clips c JOIN users u ON u.id = c.owner_id
    LEFT JOIN clip_shares s ON s.clip_id = c.id AND s.user_id = ?1";

async fn row(state: &AppState, me: i64, id: i64) -> ApiResult<Row> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE c.id = ?2")))
        .bind(me)
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("clip"))
}

async fn watchable(state: &AppState, user: &User, id: i64) -> ApiResult<Row> {
    let r = row(state, user.id, id).await?;

    if r.owner_id == user.id || r.shared_at.is_some() || user.is_admin {
        Ok(r)
    } else {
        Err(ApiError::not_found("clip"))
    }
}

async fn manageable(state: &AppState, user: &User, id: i64) -> ApiResult<Row> {
    let r = row(state, user.id, id).await?;
    if r.owner_id == user.id || user.is_admin { Ok(r) } else { Err(ApiError::not_found("clip")) }
}

fn enabled(state: &AppState) -> ApiResult<()> {
    if state.config.current().clips.enabled {
        Ok(())
    } else {
        Err(ApiError::new(StatusCode::FORBIDDEN, "clipping is turned off on this server"))
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipState {
    Queued,
    Rendering,
    Ready,
    Failed,

    Evicted,
}

impl ClipState {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "rendering" => Some(Self::Rendering),
            "ready" => Some(Self::Ready),
            "failed" => Some(Self::Failed),
            "evicted" => Some(Self::Evicted),
            _ => None,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    Ok,
    Changed,
    Gone,
}

async fn source_status(state: &AppState, r: &Row) -> ApiResult<SourceStatus> {
    if r.quote.is_some() {
        return Ok(match catalog::by_path(&state.db, &r.source_path).await? {
            None => SourceStatus::Gone,
            Some(b) if (b.size, b.mtime) != (r.source_size, r.source_mtime) => SourceStatus::Changed,
            Some(_) => SourceStatus::Ok,
        });
    }

    Ok(match clips::source_file(state, r.media_id, &r.source_path).await? {
        None => SourceStatus::Gone,
        Some(p) if clips::fingerprint(&p) != Some((r.source_size, r.source_mtime)) => SourceStatus::Changed,
        Some(_) => SourceStatus::Ok,
    })
}

pub struct Clip {
    r: Row,
    viewer: User,
}

impl Clip {
    fn manage(&self) -> bool {
        self.r.owner_id == self.viewer.id || self.viewer.is_admin
    }
}

pub struct ClipSource<'a>(&'a Clip);

#[Object]
impl ClipSource<'_> {
    async fn video(&self, ctx: &Context<'_>) -> ApiResult<Option<Video>> {
        let r = &self.0.r;
        let (Some(id), SourceStatus::Ok | SourceStatus::Changed) = (r.media_id, source_status(ctx.state(), r).await?)
        else {
            return Ok(None);
        };

        Video::load(ctx.state(), &Arc::new(Access::User(self.0.viewer.clone())), id).await
    }

    async fn title(&self, ctx: &Context<'_>) -> ApiResult<Option<Title>> {
        let Some(id) = self.0.r.item_id else { return Ok(None) };
        Title::load(ctx.state(), &Arc::new(Access::User(self.0.viewer.clone())), id).await
    }

    async fn name(&self) -> &str {
        &self.0.r.show_title
    }

    async fn kind(&self) -> TitleKind {
        TitleKind::parse(&self.0.r.kind)
    }

    async fn label(&self) -> Option<&str> {
        self.0.r.label.as_deref()
    }

    async fn year(&self) -> Option<i64> {
        self.0.r.year
    }

    /// The book a quote is from, while it's there and you can see it.
    async fn book(&self, ctx: &Context<'_>) -> ApiResult<Option<BookView>> {
        let (r, viewer, state) = (&self.0.r, &self.0.viewer, ctx.state());

        if r.quote.is_none() {
            return Ok(None);
        }

        match catalog::by_path(&state.db, &r.source_path).await?.filter(|b| books::sees(state, viewer, &b.library)) {
            Some(b) => Ok(Some(books::view(state, viewer, b).await?)),
            None => Ok(None),
        }
    }

    async fn status(&self, ctx: &Context<'_>) -> ApiResult<SourceStatus> {
        source_status(ctx.state(), &self.0.r).await
    }
}

#[derive(SimpleObject)]
pub struct QuoteRange {
    start: i64,
    end: i64,
}

pub struct ClipQuote<'a>(&'a Clip);

#[Object]
impl ClipQuote<'_> {
    /// What was quoted, its paragraphs a line apart.
    async fn text(&self) -> &str {
        self.0.r.quote.as_deref().unwrap_or_default()
    }

    async fn author(&self) -> Option<&str> {
        self.0.r.author.as_deref()
    }

    /// Where it is in the book as the book is now, in characters; null when the book was changed
    /// and the text can't be found in it anymore.
    async fn range(&self, ctx: &Context<'_>) -> ApiResult<Option<QuoteRange>> {
        let (r, state) = (&self.0.r, ctx.state());
        let Some(b) = catalog::by_path(&state.db, &r.source_path).await? else { return Ok(None) };

        if (b.size, b.mtime) == (r.source_size, r.source_mtime) {
            return Ok(Some(QuoteRange { start: r.range_start as i64, end: r.range_end as i64 }));
        }

        let Ok(book) = state.books.get(FsPath::new(&b.path), b.mtime).await else { return Ok(None) };
        let was = r.book_length.filter(|&l| l > 0).unwrap_or(b.length);
        let near = (r.range_start * b.length as f64 / was as f64).max(0.0) as u64;
        let text = r.quote.clone().unwrap_or_default();
        let found = tokio::task::spawn_blocking(move || book.document.find(&text, near))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let Some((start, end)) = found else { return Ok(None) };

        sqlx::query(
            "UPDATE clips SET range_start = ?, range_end = ?, source_size = ?, source_mtime = ?, book_length = ?
             WHERE id = ?",
        )
        .bind(start as f64)
        .bind(end as f64)
        .bind(b.size)
        .bind(b.mtime)
        .bind(b.length)
        .bind(r.id)
        .execute(&state.db)
        .await?;

        Ok(Some(QuoteRange { start: start as i64, end: end as i64 }))
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipKind {
    Clip,
    Screenshot,
    Quote,
}

impl ClipKind {
    fn filter(self) -> &'static str {
        match self {
            ClipKind::Clip => " AND c.screenshot = 0 AND c.quote IS NULL",
            ClipKind::Screenshot => " AND c.screenshot = 1",
            ClipKind::Quote => " AND c.quote IS NOT NULL",
        }
    }
}

#[derive(SimpleObject)]
pub struct ClipQuality {
    height: i64,

    half_rate: bool,
}

#[derive(SimpleObject)]
pub struct ClipRecipient {
    user: User,
    shared_at: i64,

    hidden: bool,
}

#[Object]
impl Clip {
    async fn id(&self) -> i64 {
        self.r.id
    }

    async fn screenshot(&self) -> bool {
        self.r.screenshot
    }

    async fn kind(&self) -> ClipKind {
        if self.r.quote.is_some() {
            ClipKind::Quote
        } else if self.r.screenshot {
            ClipKind::Screenshot
        } else {
            ClipKind::Clip
        }
    }

    async fn quote(&self) -> Option<ClipQuote<'_>> {
        self.r.quote.is_some().then_some(ClipQuote(self))
    }

    async fn name(&self) -> &str {
        &self.r.title
    }

    async fn mine(&self) -> bool {
        self.r.owner_id == self.viewer.id
    }

    async fn can_manage(&self) -> bool {
        self.manage()
    }

    async fn owner(&self, ctx: &Context<'_>) -> ApiResult<User> {
        auth::load_user(ctx.state(), self.r.owner_id).await?.ok_or_else(|| ApiError::not_found("account"))
    }

    async fn source(&self) -> ClipSource<'_> {
        ClipSource(self)
    }

    async fn start(&self) -> f64 {
        self.r.range_start
    }

    async fn end(&self) -> f64 {
        self.r.range_end
    }

    async fn audio(&self) -> Option<i64> {
        self.r.audio
    }

    async fn subtitles(&self) -> Option<&str> {
        self.r.subtitles.as_deref()
    }

    async fn quality(&self) -> ClipQuality {
        ClipQuality { height: self.r.height, half_rate: self.r.half_rate }
    }

    async fn state(&self) -> ClipState {
        ClipState::parse(&self.r.state).unwrap_or(ClipState::Failed)
    }

    async fn progress(&self, ctx: &Context<'_>) -> Option<f32> {
        ctx.state().clips.progress(self.r.id)
    }

    async fn error(&self) -> Option<&str> {
        self.r.error.as_deref()
    }

    async fn bytes(&self) -> Option<i64> {
        if self.r.state == "ready" { self.r.bytes } else { None }
    }

    async fn width(&self) -> Option<i64> {
        self.r.width_px
    }

    async fn height(&self) -> Option<i64> {
        self.r.height_px
    }

    async fn fps(&self) -> Option<f64> {
        self.r.fps
    }

    async fn rendered_at(&self) -> Option<i64> {
        self.r.rendered_at
    }

    async fn created_at(&self) -> i64 {
        self.r.created_at
    }

    async fn shared_at(&self) -> Option<i64> {
        self.r.shared_at
    }

    async fn public(&self) -> bool {
        self.r.public
    }

    async fn link(&self) -> Option<String> {
        (self.manage() && self.r.public).then(|| format!("/c/{}", self.r.code))
    }

    async fn link_live(&self, ctx: &Context<'_>) -> bool {
        self.r.public && clips::pinned(ctx.state(), self.r.owner_id).await
    }

    async fn recipients(&self, ctx: &Context<'_>) -> ApiResult<Vec<ClipRecipient>> {
        if !self.manage() {
            return Ok(Vec::new());
        }

        let state = ctx.state();

        let rows: Vec<(i64, i64, bool)> =
            sqlx::query_as("SELECT user_id, shared_at, hidden FROM clip_shares WHERE clip_id = ? ORDER BY shared_at")
                .bind(self.r.id)
                .fetch_all(&state.db)
                .await?;

        let mut out = Vec::new();

        for (user_id, shared_at, hidden) in rows {
            if let Some(user) = auth::load_user(state, user_id).await? {
                out.push(ClipRecipient { user, shared_at, hidden });
            }
        }

        Ok(out)
    }

    async fn file(&self) -> String {
        format!("/api/clips/{}/{}", self.r.id, if self.r.screenshot { "image" } else { "video" })
    }

    async fn poster(&self) -> Option<String> {
        self.r.rendered_at.map(|t| format!("/api/clips/{}/poster?v={t}", self.r.id))
    }
}

async fn clip(state: &AppState, user: &User, id: i64) -> ApiResult<Clip> {
    Ok(Clip { r: row(state, user.id, id).await?, viewer: user.clone() })
}

/// Someone's quotes from a book and the ones sent to them, in the order they're in it.
pub async fn quotes_of(state: &AppState, user: &User, b: &BookRow) -> ApiResult<Vec<Clip>> {
    if !state.config.current().clips.enabled {
        return Ok(Vec::new());
    }

    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE c.source_path = ?2 AND c.quote IS NOT NULL
             AND (c.owner_id = ?1 OR (s.user_id IS NOT NULL AND s.hidden = 0))
         ORDER BY c.range_start, c.id"
    )))
    .bind(user.id)
    .bind(&b.path)
    .fetch_all(&state.db)
    .await?;

    Ok(rows.into_iter().map(|r| Clip { r, viewer: user.clone() }).collect())
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipScope {
    Mine,

    Received,

    Sent,

    Rendering,
}

#[derive(SimpleObject)]
pub struct ClipAllowance {
    can_clip: bool,
    can_link: bool,

    max_length: u32,

    bytes: i64,

    rendered: i64,

    storage: u32,

    limit: u32,

    custom_default_font: bool,
}

async fn allowance(state: &AppState, user: &User) -> ApiResult<ClipAllowance> {
    let (bytes, rendered): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(bytes), 0), COUNT(CASE WHEN screenshot = 0 AND quote IS NULL THEN 1 END) FROM clips
         WHERE owner_id = ? AND state = 'ready'",
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;

    let p = &user.permissions;
    let config = state.config.current();

    Ok(ClipAllowance {
        can_clip: p.clip,
        can_link: config.clips.public_links && p.clip_links,
        max_length: p.clip_max_length,
        bytes,
        rendered,
        storage: p.clip_storage,
        limit: p.clip_limit,
        custom_default_font: config.clips.default_font.is_some(),
    })
}

#[derive(SimpleObject)]
pub struct ClipUsage {
    user: User,
    bytes: i64,
    rendered: i64,
    clips: i64,

    storage: u32,
    limit: u32,
}

#[derive(SimpleObject)]
pub struct ClipStorage {
    usage: Vec<ClipUsage>,

    public_clips: Vec<Clip>,

    bytes: i64,

    dir: Option<String>,
}

#[derive(Default)]
pub struct ClipQuery;

#[Object]
impl ClipQuery {
    /// `kind` only those of a kind, and `book` only quotes from that book.
    async fn clips(
        &self,
        ctx: &Context<'_>,
        #[graphql(default_with = "ClipScope::Mine")] scope: ClipScope,
        kind: Option<ClipKind>,
        book: Option<i64>,
    ) -> ApiResult<Vec<Clip>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;

        let (filter, order) = match scope {
            ClipScope::Received => ("s.user_id = ?1 AND s.hidden = 0", "s.shared_at DESC"),
            ClipScope::Sent => {
                ("c.owner_id = ?1 AND EXISTS (SELECT 1 FROM clip_shares WHERE clip_id = c.id)", "c.created_at DESC")
            },
            ClipScope::Mine => ("c.owner_id = ?1", "c.created_at DESC"),
            ClipScope::Rendering => {
                ("c.owner_id = ?1 AND c.screenshot = 0 AND c.state IN ('queued', 'rendering')", "c.created_at")
            },
        };

        let path = match book {
            Some(id) => Some(catalog::book(&state.db, id).await?.ok_or_else(|| ApiError::not_found("book"))?.path),
            None => None,
        };

        let kind = kind.map_or("", ClipKind::filter);

        let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE {filter}{kind} AND (?2 IS NULL OR (c.quote IS NOT NULL AND c.source_path = ?2))
             ORDER BY {order} LIMIT 500"
        )))
        .bind(user.id)
        .bind(path)
        .fetch_all(&state.db)
        .await?;

        Ok(rows.into_iter().map(|r| Clip { r, viewer: user.clone() }).collect())
    }

    async fn clip(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<Clip>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;

        match watchable(state, user, id).await {
            Ok(r) => Ok(Some(Clip { r, viewer: user.clone() })),
            Err(e) if e.status == StatusCode::NOT_FOUND => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn clip_allowance(&self, ctx: &Context<'_>) -> ApiResult<ClipAllowance> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;
        allowance(state, user).await
    }

    async fn clip_storage(&self, ctx: &Context<'_>) -> ApiResult<ClipStorage> {
        let (state, admin) = (ctx.state(), ctx.admin()?);
        let defaults = auth::permissions::defaults(state).await?;

        let rows: Vec<(i64, bool, String, i64, i64, i64)> = sqlx::query_as(
            "SELECT u.id, u.is_admin, u.permissions,
                    COALESCE(SUM(CASE WHEN c.state = 'ready' THEN c.bytes END), 0),
                    COUNT(CASE WHEN c.state = 'ready' AND c.screenshot = 0 AND c.quote IS NULL THEN 1 END), COUNT(c.id)
             FROM users u LEFT JOIN clips c ON c.owner_id = u.id
             GROUP BY u.id ORDER BY 4 DESC, u.username COLLATE NOCASE",
        )
        .fetch_all(&state.db)
        .await?;

        let mut usage = Vec::new();

        for (id, is_admin, overrides, bytes, rendered, clips) in rows {
            let p = auth::permissions::effective(is_admin, &auth::permissions::Overrides::parse(&overrides), &defaults);

            if let Some(user) = auth::load_user(state, id).await? {
                usage.push(ClipUsage { user, bytes, rendered, clips, storage: p.clip_storage, limit: p.clip_limit });
            }
        }

        let public: Vec<Row> =
            sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE c.public = 1 ORDER BY c.created_at DESC")))
                .bind(admin.id)
                .fetch_all(&state.db)
                .await?;

        let config = state.config.current();

        let bytes: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(bytes), 0) FROM clips WHERE state = 'ready'")
            .fetch_one(&state.db)
            .await?;

        Ok(ClipStorage {
            usage,
            public_clips: public.into_iter().map(|r| Clip { r, viewer: admin.clone() }).collect(),
            bytes,
            dir: config
                .clips
                .dir(state.config.config_dir(), &state.paths.data_dir)
                .ok()
                .map(|d| d.display().to_string()),
        })
    }
}

#[derive(InputObject)]
pub struct RecipeInput {
    start: f64,
    end: f64,

    audio: Option<i64>,

    subtitles: Option<String>,

    height: i32,
    #[graphql(default)]
    half_rate: bool,
}

struct Source {
    media_id: i64,
    item_id: i64,
    library: String,
    path: PathBuf,
    size: i64,
    mtime: i64,
    show_title: String,
    kind: String,
    label: Option<String>,
    year: Option<i64>,
}

async fn source(state: &AppState, user: &User, media_id: i64, room: Option<(&str, &HeaderMap)>) -> ApiResult<Source> {
    let row: Option<(String, i64, String, String, String, Option<i64>, Option<i64>, Option<i64>, Option<i64>)> =
        sqlx::query_as(
            "SELECT m.path, m.item_id, i.library, i.title, i.kind, i.year, m.season, m.episode, m.episode_end
             FROM media m JOIN items i ON i.id = m.item_id WHERE m.id = ?",
        )
        .bind(media_id)
        .fetch_optional(&state.db)
        .await?;

    let (path, item_id, library, show_title, kind, year, season, episode, episode_end) =
        row.ok_or_else(|| ApiError::not_found("video"))?;

    if user.can_access(state, &library).await.is_err() {
        let in_room = match room {
            Some((code, headers)) => super::together::enter(state, code, headers)
                .await
                .is_ok_and(|(r, u)| r.info().item_id == item_id && u.is_some_and(|u| u.id == user.id)),
            None => false,
        };

        if !in_room {
            return Err(ApiError::not_found("video"));
        }
    }

    let path = PathBuf::from(path);
    let (size, mtime) = clips::fingerprint(&path).ok_or_else(|| ApiError::not_found("video"))?;

    Ok(Source {
        media_id,
        item_id,
        library,
        path,
        size,
        mtime,
        show_title,
        kind,
        label: episode_label(season, episode, episode_end),
        year,
    })
}

async fn check_recipe(state: &AppState, owner: &User, src: &Source, r: &RecipeInput) -> ApiResult<()> {
    if !owner.permissions.clip {
        return Err(ApiError::forbidden());
    }

    if !(r.start.is_finite() && r.end.is_finite()) || r.start < 0.0 || r.end - r.start < 0.5 {
        return Err(ApiError::bad_request("pick a range at least half a second long"));
    }

    let max = owner.permissions.clip_max_length;

    if max > 0 && r.end - r.start > max as f64 + 0.05 {
        return Err(ApiError::bad_request(format!("clips can be up to {max} seconds long")));
    }

    if !HEIGHTS.contains(&r.height) {
        return Err(ApiError::bad_request("pick 1080p, 720p or 480p"));
    }

    let info = state
        .media
        .probe(&src.path)
        .await
        .map_err(|e| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("can't read this file: {e:#}")))?;

    if let Some(d) = info.duration
        && r.end > d + 0.5
    {
        return Err(ApiError::bad_request("the clip runs past the end of the video"));
    }

    if let Some(a) = r.audio
        && !info.audio.iter().any(|t| t.index as i64 == a)
    {
        return Err(ApiError::bad_request("there's no such audio track"));
    }

    if let Some(s) = &r.subtitles {
        let t = info
            .subtitles
            .iter()
            .find(|t| &t.id == s)
            .ok_or_else(|| ApiError::bad_request("there's no such subtitle track"))?;

        if !t.supported && !BITMAP_SUBTITLES.contains(&t.codec.as_str()) {
            return Err(ApiError::bad_request(format!("{} subtitles can't be burned in", t.codec)));
        }
    }

    Ok(())
}

async fn check_space(state: &AppState, owner: &User) -> ApiResult<()> {
    let p = &owner.permissions;

    if !clips::pinned(state, owner.id).await || (p.clip_storage == 0 && p.clip_limit == 0) {
        return Ok(());
    }

    let (bytes, count): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(bytes), 0), COUNT(CASE WHEN screenshot = 0 AND quote IS NULL THEN 1 END) FROM clips
         WHERE owner_id = ? AND state = 'ready' AND public = 1",
    )
    .bind(owner.id)
    .fetch_one(&state.db)
    .await?;

    let full = (p.clip_storage > 0 && bytes as u64 >= p.clip_storage as u64 * 1024 * 1024)
        || (p.clip_limit > 0 && count as u64 >= p.clip_limit as u64);

    if full {
        return Err(ApiError::conflict(
            "your public clips and screenshots take up all your clip space; turn off some links or delete some first",
        ));
    }

    Ok(())
}

fn clean_title(t: &str) -> String {
    t.trim().chars().filter(|c| !c.is_control()).take(120).collect()
}

#[derive(InputObject)]
pub struct NewClip {
    video_id: i64,
    recipe: RecipeInput,
    #[graphql(default)]
    name: String,
    #[graphql(default)]
    public: bool,

    #[graphql(default)]
    recipients: Vec<i64>,

    room: Option<String>,
}

#[derive(InputObject)]
pub struct NewScreenshot {
    video_id: i64,

    at: f64,

    subtitles: Option<String>,
    room: Option<String>,
}

#[derive(InputObject)]
pub struct NewQuote {
    book_id: i64,

    /// Characters into the book, as reading positions count them.
    start: i64,
    end: i64,
    /// A note of your own about it.
    #[graphql(default)]
    name: String,
    #[graphql(default)]
    public: bool,

    #[graphql(default)]
    recipients: Vec<i64>,
}

/// The most characters a quote can have.
pub const QUOTE_LENGTH: i64 = 3000;

#[derive(InputObject)]
pub struct ClipPatch {
    name: Option<String>,
    public: Option<bool>,

    recipe: Option<RecipeInput>,
}

fn new_code() -> String {
    let mut bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[derive(Default)]
pub struct ClipMutation;

#[Object]
impl ClipMutation {
    async fn create_clip(&self, ctx: &Context<'_>, input: NewClip) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;
        let headers = &ctx.session().headers;
        let src = source(state, user, input.video_id, input.room.as_deref().map(|r| (r, headers))).await?;
        check_recipe(state, user, &src, &input.recipe).await?;
        check_space(state, user).await?;

        if input.public && !clips::pinned(state, user.id).await {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "you can't make public links"));
        }

        let r = &input.recipe;

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO clips (code, owner_id, title, media_id, item_id, library, source_path, source_size, source_mtime,
                show_title, kind, label, year, range_start, range_end, audio, subtitles, height, half_rate, public, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(new_code())
        .bind(user.id)
        .bind(clean_title(&input.name))
        .bind(src.media_id)
        .bind(src.item_id)
        .bind(&src.library)
        .bind(src.path.to_string_lossy().to_string())
        .bind(src.size)
        .bind(src.mtime)
        .bind(&src.show_title)
        .bind(&src.kind)
        .bind(&src.label)
        .bind(src.year)
        .bind(r.start)
        .bind(r.end)
        .bind(r.audio)
        .bind(&r.subtitles)
        .bind(r.height)
        .bind(r.half_rate)
        .bind(input.public)
        .bind(now())
        .fetch_one(&state.db)
        .await?;

        tracing::info!(
            "{} clipped {:.1}s of {} {}",
            user.username,
            r.end - r.start,
            src.show_title,
            src.label.as_deref().unwrap_or_default()
        );

        state.clips.enqueue(state, id, user.id);

        if !input.recipients.is_empty() {
            share_with(state, user, id, &input.recipients).await?;
        }

        clips::changed(state, id).await;
        clip(state, user, id).await
    }

    async fn take_screenshot(&self, ctx: &Context<'_>, input: NewScreenshot) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;

        if !user.permissions.clip {
            return Err(ApiError::forbidden());
        }

        let headers = &ctx.session().headers;
        let src = source(state, user, input.video_id, input.room.as_deref().map(|r| (r, headers))).await?;

        if !input.at.is_finite() || input.at < 0.0 {
            return Err(ApiError::bad_request("pick a moment in the video"));
        }

        let info = state
            .media
            .probe(&src.path)
            .await
            .map_err(|e| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("can't read this file: {e:#}")))?;

        if info.video.is_none() {
            return Err(ApiError::bad_request("this file has no picture"));
        }

        let at = info.duration.map_or(input.at, |d| input.at.min((d - 0.1).max(0.0)));

        if let Some(sub) = &input.subtitles {
            let t = info
                .subtitles
                .iter()
                .find(|t| &t.id == sub)
                .ok_or_else(|| ApiError::bad_request("there's no such subtitle track"))?;

            if !t.supported && !BITMAP_SUBTITLES.contains(&t.codec.as_str()) {
                return Err(ApiError::bad_request(format!("{} subtitles can't be burned in", t.codec)));
            }
        }

        check_space(state, user).await?;

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO clips (code, owner_id, media_id, item_id, library, source_path, source_size, source_mtime,
                show_title, kind, label, year, range_start, range_end, subtitles, height, screenshot, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 1, ?) RETURNING id",
        )
        .bind(new_code())
        .bind(user.id)
        .bind(src.media_id)
        .bind(src.item_id)
        .bind(&src.library)
        .bind(src.path.to_string_lossy().to_string())
        .bind(src.size)
        .bind(src.mtime)
        .bind(&src.show_title)
        .bind(&src.kind)
        .bind(&src.label)
        .bind(src.year)
        .bind(at)
        .bind(at)
        .bind(&input.subtitles)
        .bind(now())
        .fetch_one(&state.db)
        .await?;

        if let Err(e) = state.clips.shoot(state, id).await {
            clips::delete(state, id, user.id).await?;
            return Err(ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("couldn't take a screenshot: {e:#}")));
        }

        tracing::info!(
            "{} took a screenshot of {} {}",
            user.username,
            src.show_title,
            src.label.as_deref().unwrap_or_default()
        );

        clip(state, user, id).await
    }

    async fn create_quote(&self, ctx: &Context<'_>, input: NewQuote) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;

        if !user.permissions.clip {
            return Err(ApiError::forbidden());
        }

        let b = catalog::book(&state.db, input.book_id)
            .await?
            .filter(|b| books::sees(state, user, &b.library))
            .ok_or_else(|| ApiError::not_found("book"))?;

        let (start, end) = (input.start, input.end);

        if start < 0 || end <= start || end > b.length {
            return Err(ApiError::bad_request("pick some of the book's text"));
        }

        if end - start > QUOTE_LENGTH {
            return Err(ApiError::bad_request(format!("quotes can be up to {QUOTE_LENGTH} characters long")));
        }

        if input.public && !clips::pinned(state, user.id).await {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "you can't make public links"));
        }

        let book = state
            .books
            .get(FsPath::new(&b.path), b.mtime)
            .await
            .map_err(|e| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("can't read this book: {e:#}")))?;

        let text = book.document.text(start as u64, end as u64);

        if text.trim().is_empty() {
            return Err(ApiError::bad_request("there's no text there to quote"));
        }

        let chapter =
            book.document.chapters.iter().take_while(|c| c.at <= start as u64).last().map(|c| c.title.clone());
        let year: Option<i64> =
            b.published.as_deref().or(b.online_date.as_deref()).and_then(|d| d.get(..4)?.parse().ok());
        let authors = b.authors();

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO clips (code, owner_id, title, library, source_path, source_size, source_mtime, show_title, kind,
                label, year, range_start, range_end, height, state, public, quote, author, book_length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'book', ?, ?, ?, ?, 0, 'ready', ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(new_code())
        .bind(user.id)
        .bind(clean_title(&input.name))
        .bind(&b.library)
        .bind(&b.path)
        .bind(b.size)
        .bind(b.mtime)
        .bind(&b.title)
        .bind(chapter)
        .bind(year)
        .bind(start as f64)
        .bind(end as f64)
        .bind(input.public)
        .bind(&text)
        .bind((!authors.is_empty()).then(|| authors.join(", ")))
        .bind(b.length)
        .bind(now())
        .fetch_one(&state.db)
        .await?;

        tracing::info!("{} quoted {} characters of {}", user.username, end - start, b.title);

        if !input.recipients.is_empty() {
            share_with(state, user, id, &input.recipients).await?;
        }

        clips::changed(state, id).await;
        clip(state, user, id).await
    }

    async fn update_clip(&self, ctx: &Context<'_>, id: i64, input: ClipPatch) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;
        let r = manageable(state, user, id).await?;
        let owner = auth::load_user(state, r.owner_id).await?.ok_or_else(|| ApiError::not_found("clip"))?;

        if let Some(t) = &input.name {
            sqlx::query("UPDATE clips SET title = ? WHERE id = ?")
                .bind(clean_title(t))
                .bind(id)
                .execute(&state.db)
                .await?;
        }

        if let Some(public) = input.public {
            if public && !clips::pinned(state, owner.id).await {
                return Err(ApiError::new(StatusCode::FORBIDDEN, "public links aren't allowed for this clip's owner"));
            }

            sqlx::query("UPDATE clips SET public = ? WHERE id = ?").bind(public).bind(id).execute(&state.db).await?;

            if public && r.state == "evicted" && r.quote.is_none() {
                queue(state, &r).await?;
            }

            if !public {
                clips::make_room(state, owner.id).await?;
            }
        }

        if let Some(recipe) = &input.recipe {
            if r.screenshot {
                return Err(ApiError::bad_request("a screenshot can't be changed; take another"));
            }

            if r.quote.is_some() {
                return Err(ApiError::bad_request("a quote can't be changed; quote it again"));
            }

            let media_id = match clips::source_file(state, r.media_id, &r.source_path).await? {
                Some(p) => sqlx::query_scalar("SELECT id FROM media WHERE path = ?")
                    .bind(p.to_string_lossy().to_string())
                    .fetch_optional(&state.db)
                    .await?
                    .ok_or_else(|| ApiError::not_found("video"))?,
                None => return Err(ApiError::conflict("the video this clip came from is gone")),
            };

            let src = source(state, &owner, media_id, None)
                .await
                .map_err(|_| ApiError::conflict("the clip's owner can't see this video anymore"))?;

            check_recipe(state, &owner, &src, recipe).await?;
            state.clips.cancel(id);

            if let Ok(f) = clips::file(state, owner.id, id) {
                clips::remove_files(&f);
            }

            sqlx::query(
                "UPDATE clips SET media_id = ?, source_path = ?, source_size = ?, source_mtime = ?, range_start = ?,
                    range_end = ?, audio = ?, subtitles = ?, height = ?, half_rate = ?, state = 'queued', error = NULL,
                    bytes = NULL, rendered_at = NULL WHERE id = ?",
            )
            .bind(src.media_id)
            .bind(src.path.to_string_lossy().to_string())
            .bind(src.size)
            .bind(src.mtime)
            .bind(recipe.start)
            .bind(recipe.end)
            .bind(recipe.audio)
            .bind(&recipe.subtitles)
            .bind(recipe.height)
            .bind(recipe.half_rate)
            .bind(id)
            .execute(&state.db)
            .await?;

            state.clips.enqueue(state, id, owner.id);
        }

        clips::changed(state, id).await;
        clip(state, user, id).await
    }

    async fn delete_clip(&self, ctx: &Context<'_>, id: i64) -> ApiResult<i64> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let r = manageable(state, user, id).await?;
        clips::delete(state, id, r.owner_id).await?;

        if r.owner_id != user.id {
            tracing::info!("{} deleted a clip of {}'s", user.username, r.owner_name);
        }

        Ok(id)
    }

    async fn render_clip(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;
        let r = watchable(state, user, id).await?;
        let manage = r.owner_id == user.id || user.is_admin;
        let missing =
            r.quote.is_none() && (r.state == "evicted" || (r.state == "ready" && on_disk(state, &r).is_none()));

        if missing || (r.state == "failed" && manage) {
            match source_status(state, &r).await? {
                SourceStatus::Gone => {
                    return Err(ApiError::conflict(
                        "the video this clip came from is gone, so it can't be rendered again",
                    ));
                },
                SourceStatus::Changed if r.screenshot => {
                    return Err(ApiError::conflict(
                        "the video this screenshot came from has changed, so it can't be taken again",
                    ));
                },
                SourceStatus::Changed => {
                    return Err(ApiError::conflict(
                        "the video this clip came from has changed; its owner can open it in the editor to check the range",
                    ));
                },
                SourceStatus::Ok => {},
            }

            let owner = auth::load_user(state, r.owner_id).await?.ok_or_else(|| ApiError::not_found("clip"))?;

            if !owner.permissions.clip {
                return Err(ApiError::conflict(
                    "this clip's owner can't make clips anymore, so it can't be rendered again",
                ));
            }

            queue(state, &r).await?;
        }

        clip(state, user, id).await
    }

    async fn cancel_clip_render(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        manageable(state, user, id).await?;

        if !state.clips.cancel(id) {
            sqlx::query("UPDATE clips SET state = 'failed', error = 'Stopped' WHERE id = ? AND state IN ('queued', 'rendering')")
                .bind(id)
                .execute(&state.db)
                .await?;

            clips::changed(state, id).await;
        }

        clip(state, user, id).await
    }

    async fn share_clip(&self, ctx: &Context<'_>, id: i64, users: Vec<i64>) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        enabled(state)?;
        manageable(state, user, id).await?;
        share_with(state, user, id, &users).await?;
        clips::changed(state, id).await;
        clip(state, user, id).await
    }

    async fn unshare_clip(&self, ctx: &Context<'_>, id: i64, user_id: i64) -> ApiResult<Clip> {
        let (state, user) = (ctx.state(), ctx.user()?);
        manageable(state, user, id).await?;

        sqlx::query("DELETE FROM clip_shares WHERE clip_id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(&state.db)
            .await?;

        sqlx::query("DELETE FROM notifications WHERE user_id = ? AND link = ?")
            .bind(user_id)
            .bind(format!("/clips?clip={id}"))
            .execute(&state.db)
            .await?;

        state.events.send(Event::NotificationsChanged { user_id });

        state.events.send(Event::ClipChanged {
            clip_id: id,
            users: vec![user_id],
            state: "deleted".into(),
            progress: None,
        });

        clips::changed(state, id).await;
        clip(state, user, id).await
    }

    async fn hide_clip(&self, ctx: &Context<'_>, id: i64) -> ApiResult<i64> {
        let (state, user) = (ctx.state(), ctx.user()?);

        sqlx::query("UPDATE clip_shares SET hidden = 1 WHERE clip_id = ? AND user_id = ?")
            .bind(id)
            .bind(user.id)
            .execute(&state.db)
            .await?;

        state.events.send(Event::ClipChanged {
            clip_id: id,
            users: vec![user.id],
            state: "hidden".into(),
            progress: None,
        });

        Ok(id)
    }

    async fn drop_clip_renders(&self, ctx: &Context<'_>) -> ApiResult<usize> {
        let (state, admin) = (ctx.state(), ctx.admin()?);

        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "UPDATE clips SET state = 'evicted', bytes = NULL WHERE state = 'ready' AND quote IS NULL RETURNING id, owner_id",
        )
        .fetch_all(&state.db)
        .await?;

        for &(id, owner) in &rows {
            if let Ok(f) = clips::file(state, owner, id) {
                clips::remove_files(&f);
            }

            clips::changed(state, id).await;
        }

        tracing::info!("{} dropped all {} clip renders", admin.username, rows.len());
        Ok(rows.len())
    }
}

async fn queue(state: &Arc<AppState>, r: &Row) -> ApiResult<()> {
    sqlx::query("UPDATE clips SET state = 'queued', error = NULL WHERE id = ?").bind(r.id).execute(&state.db).await?;
    clips::changed(state, r.id).await;
    state.clips.render_later(state, r.id, r.owner_id, r.screenshot);
    Ok(())
}

fn on_disk(state: &AppState, r: &Row) -> Option<PathBuf> {
    clips::rendered_file(state, r.owner_id, r.id, r.screenshot).ok().filter(|f| f.is_file())
}

#[derive(Deserialize)]
pub struct StillQuery {
    at: f64,
    subtitles: String,
    room: Option<String>,
}

pub async fn still(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(media_id): Path<i64>,
    Query(q): Query<StillQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    enabled(&state)?;

    if !user.permissions.clip {
        return Err(ApiError::forbidden());
    }

    let src = source(&state, &user, media_id, q.room.as_deref().map(|r| (r, &headers))).await?;

    let sub = clips::subtitle_source(&state, &src.path, &q.subtitles)
        .await
        .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

    let (dir, default) = clips::fonts(&state)?;
    let info = state.media.probe(&src.path).await?;
    let (w, h) = info.video.as_ref().map(|v| (v.width, v.height)).ok_or_else(|| ApiError::not_found("video"))?;
    let (path, at) = (src.path.clone(), q.at.max(0.0));

    let jpeg = tokio::task::spawn_blocking(move || {
        const WIDTH: i32 = 960;
        let fonts = burn::Fonts { dir: dir.as_deref(), default: &default };
        let size = (WIDTH, thumb::scaled_height(w, h, WIDTH));
        let mut overlay = burn::Overlay::new(sub, &path, &fonts, size, (w, h), at, at + 0.1)?;
        thumb::still(&path, at, WIDTH, &mut |f| overlay.draw(f, at))
    })
    .await
    .map_err(|e| ApiError::from(anyhow::anyhow!(e)))??;

    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=3600")], jpeg).into_response())
}

async fn share_with(state: &AppState, user: &User, id: i64, users: &[i64]) -> ApiResult<()> {
    let r = row(state, user.id, id).await?;
    let (owner, title, show, label) = (r.owner_id, r.title.clone(), r.show_title.clone(), r.label.clone());

    let mut added = Vec::new();

    for &u in users.iter().filter(|&&u| u != owner) {
        let inserted = sqlx::query(
            "INSERT INTO clip_shares (clip_id, user_id, shared_at) SELECT ?, id, ? FROM users WHERE id = ?
             ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(now())
        .bind(u)
        .execute(&state.db)
        .await?
        .rows_affected();

        if inserted > 0 {
            added.push(u);
        } else {
            sqlx::query(
                "UPDATE clip_shares SET hidden = 0, shared_at = ? WHERE clip_id = ? AND user_id = ? AND hidden = 1",
            )
            .bind(now())
            .bind(id)
            .bind(u)
            .execute(&state.db)
            .await?;
        }
    }

    if !added.is_empty() {
        let (what, about, image) = match &r.quote {
            Some(_) => {
                let cover = catalog::by_path(&state.db, &r.source_path)
                    .await?
                    .filter(BookRow::has_cover)
                    .map(|b| format!("/api/books/{}/cover?v={}", b.id, b.mtime));
                ("quote", show, cover)
            },
            None => (
                if r.screenshot { "screenshot" } else { "clip" },
                [show, label.unwrap_or_default()].join(" ").trim().to_string(),
                r.rendered_at.map(|t| format!("/api/clips/{id}/poster?v={t}")),
            ),
        };

        notifications::send(
            state,
            &added,
            New {
                kind: "clip",
                priority: true,
                title: format!("{} sent you a {what}", user.username),
                body: Some(if title.is_empty() { about } else { format!("{title} · {about}") }),
                image,
                link: Some(format!("/clips?clip={id}")),
                actor_id: Some(user.id),
                ..Default::default()
            },
        )
        .await;

        tracing::info!("{} sent clip #{id} to {} people", user.username, added.len());
    }

    Ok(())
}

#[derive(Deserialize)]
pub struct FileQuery {
    download: Option<String>,
}

pub async fn file(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<FileQuery>,
    req: Request,
) -> ApiResult<Response> {
    enabled(&state)?;
    let r = watchable(&state, &user, id).await?;
    serve_file(&state, &r, q.download.is_some(), req, "private").await
}

async fn serve_file(
    state: &AppState,
    r: &Row,
    download: bool,
    req: Request,
    cache: &'static str,
) -> ApiResult<Response> {
    if r.quote.is_some() {
        return Err(ApiError::not_found("file"));
    }

    let file = match (r.state.as_str(), on_disk(state, r)) {
        ("ready", Some(f)) => f,
        _ => return Err(ApiError::conflict("this clip isn't rendered right now")),
    };

    let first =
        req.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).is_none_or(|v| v.starts_with("bytes=0-"));

    if first {
        sqlx::query("UPDATE clips SET viewed_at = ? WHERE id = ?").bind(now()).bind(r.id).execute(&state.db).await?;
    }

    let Ok(res) = ServeFile::new(file).oneshot(req).await;
    let mut res = res.map(Body::new);
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(if r.screenshot { "image/png" } else { "video/mp4" }));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_str(&format!("{cache}, max-age=3600")).unwrap());
    h.insert("cross-origin-resource-policy", HeaderValue::from_static("cross-origin"));
    let disposition = if download { "attachment" } else { "inline" };

    if let Ok(v) = HeaderValue::from_str(&content_disposition(disposition, &filename(r))) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(res)
}

pub async fn poster(State(state): State<Arc<AppState>>, user: User, Path(id): Path<i64>) -> ApiResult<Response> {
    let r = watchable(&state, &user, id).await?;
    serve_poster(&state, &r, "private").await
}

async fn serve_poster(state: &AppState, r: &Row, cache: &str) -> ApiResult<Response> {
    if r.quote.is_some() {
        let b = catalog::by_path(&state.db, &r.source_path).await?.ok_or_else(|| ApiError::not_found("picture"))?;
        return super::music::image(state, books::cover_file(state, &b).await, Some(600)).await;
    }

    let file = clips::file(state, r.owner_id, r.id)?.with_extension("jpg");
    let data = tokio::fs::read(&file).await.map_err(|_| ApiError::not_found("picture"))?;

    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg".to_string()),
            (header::CACHE_CONTROL, format!("{cache}, max-age=604800")),
            (header::HeaderName::from_static("cross-origin-resource-policy"), "cross-origin".to_string()),
        ],
        data,
    )
        .into_response())
}

fn filename(r: &Row) -> String {
    let source = match (&r.label, r.year) {
        (Some(l), _) if r.kind == "show" => format!("{} {l}", r.show_title),
        (_, Some(y)) if r.kind == "movie" => format!("{} ({y})", r.show_title),
        _ => r.show_title.clone(),
    };

    let what = if r.title.is_empty() {
        let s = r.range_start as i64;
        let kind = if r.screenshot { "Screenshot" } else { "Clip" };
        format!("{kind} {}-{:02}-{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        r.title.clone()
    };

    let name: String =
        format!("{source} - {what}")
            .chars()
            .map(|c| {
                if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                    ' '
                } else {
                    c
                }
            })
            .collect();

    let ext = if r.screenshot { "png" } else { "mp4" };
    format!("{}.{ext}", name.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn content_disposition(kind: &str, name: &str) -> String {
    let ascii: String = name.chars().map(|c| if c.is_ascii() && c != '"' && c != '\\' { c } else { '_' }).collect();
    let mut encoded = String::new();

    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }

    format!("{kind}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

async fn public_clip(state: &AppState, code: &str) -> Option<Row> {
    let config = state.config.current();

    if !config.clips.enabled || !config.clips.public_links {
        return None;
    }

    let r: Row = sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE c.code = ?2 AND c.public = 1")))
        .bind(0)
        .bind(code)
        .fetch_optional(&state.db)
        .await
        .ok()??;

    clips::pinned(state, r.owner_id).await.then_some(r)
}

async fn revive(state: &Arc<AppState>, r: &Row) {
    if r.quote.is_none() && r.state == "evicted" || (r.state == "ready" && on_disk(state, r).is_none()) {
        let can = auth::load_user(state, r.owner_id).await.ok().flatten().is_some_and(|u| u.permissions.clip);

        if can
            && let Ok(src) = clips::source_file(state, r.media_id, &r.source_path).await
            && src.as_deref().and_then(clips::fingerprint) == Some((r.source_size, r.source_mtime))
        {
            let _ = queue(state, r).await;
        }
    }
}

fn origin(headers: &HeaderMap) -> String {
    let get = |k: &str| {
        headers.get(k).and_then(|v| v.to_str().ok()).map(|v| v.split(',').next().unwrap_or(v).trim().to_string())
    };

    let proto = get("x-forwarded-proto").unwrap_or_else(|| "http".into());
    let host = get("x-forwarded-host").or_else(|| get("host")).unwrap_or_else(|| "localhost".into());
    format!("{proto}://{host}")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn duration_label(secs: f64) -> String {
    let s = secs.round() as i64;
    if s >= 60 { format!("{}:{:02}", s / 60, s % 60) } else { format!("{s}s") }
}

#[derive(Deserialize)]
pub struct PageQuery {
    embed: Option<String>,
}

pub async fn page(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Query(q): Query<PageQuery>,
    headers: HeaderMap,
) -> Response {
    let theme = super::appearance::public_head(&state).await.unwrap_or_default();

    let Some(r) = public_clip(&state, &code).await else {
        return (
            StatusCode::NOT_FOUND,
            Html(shell("Clip not found", &theme, "<p class=note>This clip isn't available.</p>")),
        )
            .into_response();
    };

    revive(&state, &r).await;
    let base = origin(&headers);
    let page_url = format!("{base}/c/{code}");
    let video = format!("{base}/c/{code}/video.mp4");
    let poster = format!("{base}/c/{code}/poster.jpg");
    let about = [r.show_title.as_str(), r.label.as_deref().unwrap_or_default()].join(" ").trim().to_string();
    let title = if r.title.is_empty() { about.clone() } else { r.title.clone() };
    let length = duration_label(r.range_end - r.range_start);
    let ready = r.state == "ready" && on_disk(&state, &r).is_some();
    let (w, h) = (r.width_px.unwrap_or(1280), r.height_px.unwrap_or(720));

    if r.screenshot {
        return Html(screenshot_page(&r, &base, &code, &title, &about, ready, &theme)).into_response();
    }

    if let Some(quote) = &r.quote {
        let cover = catalog::by_path(&state.db, &r.source_path).await.ok().flatten().is_some_and(|b| b.has_cover());
        return Html(quote_page(&r, quote, &base, &code, cover, &theme)).into_response();
    }

    if q.embed.is_some() {
        let body = format!(
            "<video src=\"{}\" poster=\"{}\" controls autoplay playsinline style=\"width:100vw;height:100vh;background:#000\"></video>",
            escape(&video),
            escape(&poster)
        );

        return Html(format!(
            "<!doctype html><html><head><meta charset=utf-8><meta name=robots content=noindex><title>{}</title></head>\
             <body style=\"margin:0;background:#000\">{body}</body></html>",
            escape(&title)
        ))
        .into_response();
    }

    let meta = format!(
        r##"<meta name="robots" content="noindex">
<meta property="og:site_name" content="tinystream">
<meta property="og:type" content="video.other">
<meta property="og:url" content="{page}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
<meta property="og:image" content="{poster}">
<meta property="og:image:width" content="{w}">
<meta property="og:image:height" content="{h}">
<meta property="og:video" content="{video}">
<meta property="og:video:url" content="{video}">
{secure}<meta property="og:video:type" content="video/mp4">
<meta property="og:video:width" content="{w}">
<meta property="og:video:height" content="{h}">
<meta name="twitter:card" content="player">
<meta name="twitter:title" content="{title}">
<meta name="twitter:image" content="{poster}">
<meta name="twitter:player" content="{page}?embed=1">
<meta name="twitter:player:width" content="{w}">
<meta name="twitter:player:height" content="{h}">
<meta name="twitter:player:stream" content="{video}">
<meta name="twitter:player:stream:content_type" content="video/mp4">"##,
        page = escape(&page_url),
        title = escape(&title),
        desc = escape(&format!("{about} · {length}")),
        poster = escape(&poster),
        video = escape(&video),
        secure = if video.starts_with("https:") {
            format!("<meta property=\"og:video:secure_url\" content=\"{}\">\n", escape(&video))
        } else {
            String::new()
        },
    );

    let body = if ready {
        format!(
            r#"<video src="{video}" poster="{poster}" controls playsinline preload="metadata"></video>
<div class=bar><div><h1>{title}</h1><p>{about} · {length}</p></div><a class=button href="{video}?download=1" download>Download</a></div>"#,
            video = escape(&video),
            poster = escape(&poster),
            title = escape(&title),
            about = escape(&about),
        )
    } else {
        format!(
            r#"<div class=wait><h1>{}</h1><p class=note>This clip is being prepared. The page will refresh on its own.</p></div>"#,
            escape(&title)
        )
    };

    let refresh = if ready { "" } else { r#"<meta http-equiv="refresh" content="5">"# };
    Html(shell(&title, &format!("{meta}\n{theme}\n{refresh}"), &body)).into_response()
}

fn screenshot_page(r: &Row, base: &str, code: &str, title: &str, about: &str, ready: bool, theme: &str) -> String {
    let page_url = format!("{base}/c/{code}");
    let image = format!("{base}/c/{code}/image.png");
    let at = r.range_start as i64;

    let at = if at >= 3600 {
        format!("{}:{:02}:{:02}", at / 3600, at / 60 % 60, at % 60)
    } else {
        format!("{}:{:02}", at / 60, at % 60)
    };

    let (w, h) = (r.width_px.unwrap_or(1920), r.height_px.unwrap_or(1080));

    let meta = format!(
        r##"<meta name="robots" content="noindex">
<meta property="og:site_name" content="tinystream">
<meta property="og:type" content="website">
<meta property="og:url" content="{page}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
<meta property="og:image" content="{image}">
{secure}<meta property="og:image:type" content="image/png">
<meta property="og:image:width" content="{w}">
<meta property="og:image:height" content="{h}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:image" content="{image}">"##,
        page = escape(&page_url),
        title = escape(title),
        desc = escape(&format!("{about} · {at}")),
        image = escape(&image),
        secure = if image.starts_with("https:") {
            format!("<meta property=\"og:image:secure_url\" content=\"{}\">\n", escape(&image))
        } else {
            String::new()
        },
    );

    let body = if ready {
        format!(
            r#"<a href="{image}"><img src="{image}" alt="{title}" width="{w}" height="{h}"></a>
<div class=bar><div><h1>{title}</h1><p>{about} · {at}</p></div><a class=button href="{image}?download=1" download>Download</a></div>"#,
            image = escape(&image),
            title = escape(title),
            about = escape(about),
        )
    } else {
        format!(
            r#"<div class=wait><h1>{}</h1><p class=note>This screenshot is being prepared. The page will refresh on its own.</p></div>"#,
            escape(title)
        )
    };

    let refresh = if ready { "" } else { r#"<meta http-equiv="refresh" content="3">"# };
    shell(title, &format!("{meta}\n{theme}\n{refresh}"), &body)
}

/// The start of a quote, cut at a word, for where there's only room for a line or two.
fn excerpt(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if flat.chars().count() <= max {
        return flat;
    }

    let cut: String = flat.chars().take(max).collect();
    format!("{}…", cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head).trim_end_matches([',', ';', ':']))
}

fn quote_page(r: &Row, quote: &str, base: &str, code: &str, cover: bool, theme: &str) -> String {
    let page_url = format!("{base}/c/{code}");
    let image = format!("{base}/c/{code}/poster.jpg");
    let book = match &r.author {
        Some(a) => format!("{} by {a}", r.show_title),
        None => r.show_title.clone(),
    };
    let image_meta = if cover {
        format!(
            "<meta property=\"og:image\" content=\"{0}\">\n<meta name=\"twitter:image\" content=\"{0}\">\n",
            escape(&image)
        )
    } else {
        String::new()
    };

    let meta = format!(
        r##"<meta name="robots" content="noindex">
<meta property="og:site_name" content="tinystream">
<meta property="og:type" content="article">
<meta property="og:url" content="{page}">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
{image_meta}<meta name="twitter:card" content="summary">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{desc}">
<meta name="description" content="{desc}">"##,
        page = escape(&page_url),
        title = escape(&book),
        desc = escape(&format!("“{}”", excerpt(quote, 280))),
    );

    let paragraphs: String = quote
        .split("\n\n")
        .map(|p| format!("<p>{}</p>", p.split('\n').map(escape).collect::<Vec<_>>().join("<br>")))
        .collect();
    let note = if r.title.is_empty() {
        String::new()
    } else {
        format!("<p class=note>{} · {}</p>", escape(&r.title), escape(&r.owner_name))
    };
    let picture = if cover { format!(r#"<img src="{}" alt="">"#, escape(&image)) } else { String::new() };
    let chapter = r.label.as_deref().map(|l| format!("<span>{}</span>", escape(l))).unwrap_or_default();

    let body = format!(
        r#"<figure class=quote><blockquote>{paragraphs}</blockquote>{note}
<figcaption>{picture}<div><h1>{title}</h1><p>{author}</p>{chapter}</div></figcaption></figure>"#,
        title = escape(&r.show_title),
        author = escape(r.author.as_deref().unwrap_or_default()),
    );

    shell(&book, &format!("{meta}\n{theme}"), &body)
}

fn shell(title: &str, head: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{head}
<style>
html {{ color-scheme: dark; }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; background: var(--color-canvas, #191919);
  color: var(--color-ink, #ebebea);
  font: 15px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; padding: 24px; }}
main {{ width: min(100%, 1100px); }}
video, img {{ display: block; width: 100%; max-height: 80vh; background: #000; border-radius: 14px; }}
img {{ height: auto; object-fit: contain; }}
.bar {{ display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-top: 16px; }}
h1 {{ font-size: 18px; font-weight: 600; margin: 0; }}
p {{ margin: 2px 0 0; color: var(--color-ink-2, #9b9b98); }}
.button {{ flex: none; padding: 9px 16px; border-radius: 999px; background: var(--color-accent, #ebebea);
  color: var(--color-on-accent, #191919); font-weight: 600;
  text-decoration: none; }}
.wait {{ text-align: center; }}
.quote {{ width: min(100%, 38em); margin: 0 auto; }}
blockquote {{ margin: 0; font: 21px/1.65 ui-serif, Georgia, serif; text-wrap: pretty; }}
blockquote p {{ margin: 0 0 0.8em; color: inherit; }}
figcaption {{ display: flex; align-items: center; gap: 14px; margin-top: 28px; padding-top: 20px;
  border-top: 1px solid var(--color-line, #2f2f2f); }}
figcaption img {{ width: 52px; height: 78px; object-fit: cover; border-radius: 6px; background: none; }}
figcaption span {{ display: block; margin-top: 2px; font-size: 13px; color: var(--color-ink-3, #6f6f6c); }}
.note {{ color: var(--color-ink-2, #9b9b98); }}
</style>
</head>
<body><main>{body}</main></body>
</html>"#,
        title = escape(title)
    )
}

#[derive(Deserialize)]
pub struct PublicFileQuery {
    download: Option<String>,
}

pub async fn public_file(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Query(q): Query<PublicFileQuery>,
    req: Request,
) -> ApiResult<Response> {
    let r = public_clip(&state, &code).await.ok_or_else(|| ApiError::not_found("clip"))?;
    revive(&state, &r).await;

    match serve_file(&state, &r, q.download.is_some(), req, "public").await {
        Err(e) if e.status == StatusCode::CONFLICT => {
            let mut res =
                (StatusCode::SERVICE_UNAVAILABLE, "this clip is being prepared; try again in a moment").into_response();

            res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
            Ok(res)
        },
        r => r,
    }
}

pub async fn public_poster(State(state): State<Arc<AppState>>, Path(code): Path<String>) -> ApiResult<Response> {
    let r = public_clip(&state, &code).await.ok_or_else(|| ApiError::not_found("clip"))?;
    serve_poster(&state, &r, "public").await
}

const BURST: f64 = 600.0;
const PER_SECOND: f64 = 10.0;

fn buckets() -> &'static Mutex<HashMap<IpAddr, (f64, Instant)>> {
    static B: OnceLock<Mutex<HashMap<IpAddr, (f64, Instant)>>> = OnceLock::new();
    B.get_or_init(Default::default)
}

fn client(req: &Request) -> Option<IpAddr> {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>()?.0.ip();

    let local = match peer {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    };

    if local {
        let forwarded = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok());

        if let Some(ip) = forwarded.and_then(|v| v.rsplit(',').next()).and_then(|v| v.trim().parse().ok()) {
            return Some(ip);
        }

        if let Some(ip) =
            req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse().ok())
        {
            return Some(ip);
        }
    }

    Some(peer)
}

pub async fn rate_limit(req: Request, next: Next) -> Response {
    if let Some(ip) = client(&req) {
        let mut b = buckets().lock().unwrap();

        if b.len() > 50_000 {
            b.retain(|_, (tokens, at)| *tokens + at.elapsed().as_secs_f64() * PER_SECOND < BURST);
        }

        let (tokens, at) = b.entry(ip).or_insert((BURST, Instant::now()));
        *tokens = (*tokens + at.elapsed().as_secs_f64() * PER_SECOND).min(BURST);
        *at = Instant::now();

        if *tokens < 1.0 {
            drop(b);
            let mut res = (StatusCode::TOO_MANY_REQUESTS, "slow down a little").into_response();
            res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("10"));
            return res;
        }

        *tokens -= 1.0;
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition() {
        assert_eq!(
            content_disposition("attachment", "Frieren S01E05 - Ça va.mp4"),
            "attachment; filename=\"Frieren S01E05 - _a va.mp4\"; filename*=UTF-8''Frieren%20S01E05%20-%20%C3%87a%20va.mp4"
        );
    }
}
