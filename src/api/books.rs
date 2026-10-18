// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use async_graphql::{Context, Enum, Object, SimpleObject};
use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::music::CoverQuery;
use super::schema::Ctx;
use crate::auth::User;
use crate::books::catalog::{self, BookRow, Reading};
use crate::books::{self, online};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tint::Source;

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Epub,
    #[graphql(name = "FB2")]
    Fb2,
    Txt,
}

#[derive(SimpleObject)]
pub struct ReadingState {
    /// Characters in, of the book's `length`.
    position: i64,
    /// From 0 to 1.
    progress: f64,
    finished: bool,
    updated_at: i64,
}

impl ReadingState {
    fn of(r: Reading, length: i64) -> Self {
        Self { position: r.position_in(length), progress: r.progress(), finished: r.finished, updated_at: r.updated_at }
    }
}

pub struct BookView {
    b: BookRow,
    reading: Option<Reading>,
}

#[Object(name = "Book")]
impl BookView {
    async fn id(&self) -> i64 {
        self.b.id
    }

    async fn library(&self) -> &str {
        &self.b.library
    }

    async fn title(&self) -> &str {
        &self.b.title
    }

    async fn sort_title(&self) -> &str {
        &self.b.sort_title
    }

    async fn authors(&self) -> Vec<String> {
        self.b.authors()
    }

    async fn series(&self) -> Option<&str> {
        self.b.series.as_deref()
    }

    async fn series_index(&self) -> Option<f64> {
        self.b.series_index
    }

    /// The book's own, or else what was found online.
    async fn description(&self) -> Option<&str> {
        self.b.description.as_deref().or(self.b.overview.as_deref())
    }

    async fn publisher(&self) -> Option<&str> {
        self.b.publisher.as_deref()
    }

    /// A date, or just a year.
    async fn published(&self) -> Option<&str> {
        self.b.published.as_deref().or(self.b.online_date.as_deref())
    }

    async fn year(&self) -> Option<i64> {
        let date = self.b.published.as_deref().or(self.b.online_date.as_deref())?;
        date.get(..4)?.parse().ok()
    }

    async fn language(&self) -> Option<&str> {
        self.b.language.as_deref()
    }

    async fn isbn(&self) -> Option<&str> {
        self.b.isbn.as_deref()
    }

    async fn genres(&self) -> Vec<String> {
        self.b.genres()
    }

    async fn format(&self) -> BookFormat {
        match self.b.format.as_str() {
            "fb2" => BookFormat::Fb2,
            "txt" => BookFormat::Txt,
            _ => BookFormat::Epub,
        }
    }

    /// Characters of text, which reading positions count in.
    async fn length(&self) -> i64 {
        self.b.length
    }

    async fn chapter_count(&self) -> i64 {
        self.b.chapters
    }

    /// Pages in print, when that was found online.
    async fn pages(&self) -> Option<i64> {
        self.b.pages
    }

    /// Out of 10.
    async fn rating(&self) -> Option<f64> {
        self.b.rating
    }

    async fn size(&self) -> i64 {
        self.b.size
    }

    async fn added_at(&self) -> i64 {
        self.b.added_at
    }

    /// Where what the book doesn't say came from: `apple-books` or `open-library`.
    async fn provider(&self) -> Option<&str> {
        self.b.provider.as_deref()
    }

    /// The page about it where its details came from.
    async fn provider_url(&self) -> Option<String> {
        let id = self.b.provider_id.as_deref()?;

        match self.b.provider.as_deref()? {
            "apple-books" => Some(format!("https://books.apple.com/book/id{id}")),
            "open-library" => Some(format!("https://openlibrary.org/works/{id}")),
            _ => None,
        }
    }

    /// Whether it's been looked up online yet.
    async fn fetched(&self) -> bool {
        self.b.fetched_at.is_some()
    }

    async fn cover(&self) -> Option<String> {
        self.b.has_cover().then(|| format!("/api/books/{}/cover?v={}", self.b.id, self.b.mtime))
    }

    /// The most vivid colour of the cover as "r g b", empty until it's been worked out.
    async fn cover_tint(&self, ctx: &Context<'_>) -> Option<String> {
        let state = ctx.state();
        let source = cover_source(state, &self.b).await?;
        state.tints.get(state, source).await
    }

    /// The text, as the readers show it: JSON, as `packages/shared/src/books.ts` describes it.
    async fn content(&self) -> String {
        format!("/api/books/{}/content?v={}", self.b.id, self.b.mtime)
    }

    /// Where the pictures in `content` are, each at this followed by its `src`.
    async fn files(&self) -> String {
        format!("/api/books/{}/files/", self.b.id)
    }

    /// The file as it is.
    async fn file(&self) -> String {
        format!("/api/books/{}/file", self.b.id)
    }

    async fn reading(&self) -> Option<ReadingState> {
        self.reading.map(|r| ReadingState::of(r, self.b.length))
    }
}

async fn views(state: &AppState, user: &User, books: Vec<BookRow>) -> ApiResult<Vec<BookView>> {
    let paths: Vec<&str> = books.iter().map(|b| b.path.as_str()).collect();
    let mut reading = catalog::reading(&state.db, user.id, &paths).await?;

    Ok(books.into_iter().map(|b| BookView { reading: reading.remove(&b.path), b }).collect())
}

fn sees(state: &AppState, user: &User, library: &str) -> bool {
    user.permissions.can_see(library) && state.config.current().library(library).is_some_and(|l| l.is_books())
}

async fn visible(state: &AppState, user: &User, id: i64) -> ApiResult<BookRow> {
    catalog::book(&state.db, id)
        .await?
        .filter(|b| sees(state, user, &b.library))
        .ok_or_else(|| ApiError::not_found("book"))
}

async fn view(state: &AppState, user: &User, b: BookRow) -> ApiResult<BookView> {
    Ok(views(state, user, vec![b]).await?.pop().expect("one book in, one out"))
}

#[derive(Default)]
pub struct BookQuery;

#[Object]
impl BookQuery {
    /// A book library's books, a series' volumes together.
    async fn books(&self, ctx: &Context<'_>, library: String) -> ApiResult<Vec<BookView>> {
        let state = ctx.state();
        let user = ctx.user()?;

        if !sees(state, user, &library) {
            return Err(ApiError::not_found("library"));
        }

        views(state, user, catalog::books(&state.db, &library).await?).await
    }

    async fn book(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<BookView>> {
        let state = ctx.state();
        let user = ctx.user()?;

        match catalog::book(&state.db, id).await?.filter(|b| sees(state, user, &b.library)) {
            Some(b) => Ok(Some(view(state, user, b).await?)),
            None => Ok(None),
        }
    }

    /// Books part read, the latest first.
    async fn reading_now(&self, ctx: &Context<'_>, #[graphql(default = 16)] limit: i64) -> ApiResult<Vec<BookView>> {
        let state = ctx.state();
        let user = ctx.user()?;
        let books = catalog::reading_now(&state.db, user.id, limit.clamp(1, 100) * 2).await?;
        let mut out = views(state, user, books.into_iter().filter(|b| sees(state, user, &b.library)).collect()).await?;
        out.truncate(limit.clamp(1, 100) as usize);
        Ok(out)
    }

    /// The books added last to each book library.
    async fn new_books(&self, ctx: &Context<'_>, #[graphql(default = 24)] limit: i64) -> ApiResult<Vec<BookShelf>> {
        let state = ctx.state();
        let user = ctx.user()?;
        let mut out = Vec::new();

        for library in books::libraries(state, user) {
            let rows = catalog::recently_added(&state.db, &library, limit.clamp(1, 100)).await?;

            if !rows.is_empty() {
                out.push(BookShelf { books: views(state, user, rows).await?, library });
            }
        }

        Ok(out)
    }
}

#[derive(SimpleObject)]
pub struct BookShelf {
    library: String,
    books: Vec<BookView>,
}

#[derive(Default)]
pub struct BookMutation;

#[Object]
impl BookMutation {
    /// Where someone is in a book, in characters of its `length`; reaching the end finishes it.
    async fn save_reading(&self, ctx: &Context<'_>, id: i64, position: i64) -> ApiResult<BookView> {
        let state = ctx.state();
        let user = ctx.user()?;
        let b = visible(state, user, id).await?;
        let r = catalog::save_reading(&state.db, user.id, &b, position, None).await?;
        Ok(BookView { b, reading: Some(r) })
    }

    /// Marks a book read, or not started (which forgets where it was left).
    async fn set_book_finished(&self, ctx: &Context<'_>, id: i64, finished: bool) -> ApiResult<BookView> {
        let state = ctx.state();
        let user = ctx.user()?;
        let b = visible(state, user, id).await?;

        if finished {
            let r = catalog::save_reading(&state.db, user.id, &b, b.length, Some(true)).await?;
            return Ok(BookView { b, reading: Some(r) });
        }

        catalog::forget_reading(&state.db, user.id, &b.path).await?;
        Ok(BookView { b, reading: None })
    }

    /// Looks the book up online again.
    async fn refresh_book(&self, ctx: &Context<'_>, id: i64) -> ApiResult<BookView> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.edit_metadata)?;
        let b = visible(state, user, id).await?;

        sqlx::query("UPDATE books SET fetched_at = NULL WHERE id = ?").bind(b.id).execute(&state.db).await?;
        online::wake(state);
        view(state, user, b).await
    }
}

/// Where a book's cover comes from: the picture in it, written out once, or the one found online.
async fn cover_source(state: &AppState, b: &BookRow) -> Option<Source> {
    if b.embedded_cover
        && let Some(file) = embedded_cover(state, b).await
    {
        return Some(Source::File(file));
    }

    b.online_cover.clone().map(Source::Remote)
}

async fn embedded_cover(state: &AppState, b: &BookRow) -> Option<PathBuf> {
    let dir = state.paths.cache_dir().join("covers");
    let key = hex::encode(&Sha256::digest(format!("book\0{}\0{}", b.path, b.mtime))[..12]);

    for ext in ["jpg", "png", "webp", "gif"] {
        let f = dir.join(format!("{key}.{ext}"));

        if f.exists() {
            return Some(f);
        }
    }

    let path = PathBuf::from(&b.path);
    let (mime, data) = tokio::task::spawn_blocking(move || books::cover(&path)).await.ok()??;

    let ext = match mime.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    };

    let file = dir.join(format!("{key}.{ext}"));
    tokio::fs::create_dir_all(&dir).await.ok()?;
    let tmp = file.with_extension("part");
    tokio::fs::write(&tmp, data).await.ok()?;
    tokio::fs::rename(&tmp, &file).await.ok()?;
    Some(file)
}

pub async fn cover(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<CoverQuery>,
) -> ApiResult<Response> {
    let b = visible(&state, &user, id).await?;

    let file = match cover_source(&state, &b).await {
        Some(Source::File(f)) => Some(f),
        Some(Source::Remote(url)) => {
            let dir = state.paths.cache_dir().join("images");
            super::image_cache::remote(&state.http, &dir, &url).await.ok()
        },
        _ => None,
    };

    super::music::image(&state, file, q.size).await
}

pub async fn content(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let b = visible(&state, &user, id).await?;
    let etag = format!("\"{}-{}-{}\"", b.id, b.mtime, b.size);

    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    let book = state.books.get(FsPath::new(&b.path), b.mtime).await.map_err(|e| {
        tracing::warn!("reading {}: {e:#}", b.path);
        ApiError::not_found("book")
    })?;

    Ok((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache")),
            (header::ETAG, HeaderValue::from_str(&etag).map_err(|e| anyhow::anyhow!("{e}"))?),
        ],
        book.json(),
    )
        .into_response())
}

pub async fn resource(
    State(state): State<Arc<AppState>>,
    user: User,
    Path((id, name)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let b = visible(&state, &user, id).await?;
    let book = state.books.get(FsPath::new(&b.path), b.mtime).await.map_err(|_| ApiError::not_found("book"))?;
    let path = PathBuf::from(&b.path);
    let found =
        tokio::task::spawn_blocking(move || book.resource(&path, &name)).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    let (mime, data) = found.ok_or_else(|| ApiError::not_found("file"))?;

    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=86400".into())], data).into_response())
}

pub async fn file(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    req: Request,
) -> ApiResult<Response> {
    let b = visible(&state, &user, id).await?;
    let Ok(res) = ServeFile::new(&b.path).oneshot(req).await;

    if res.status() == StatusCode::NOT_FOUND {
        return Err(ApiError::not_found("file"));
    }

    let name = FsPath::new(&b.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut res = res.map(Body::new);
    let encoded: String = name
        .bytes()
        .map(|c| {
            if c.is_ascii_alphanumeric() || b"-._~".contains(&c) {
                (c as char).to_string()
            } else {
                format!("%{c:02X}")
            }
        })
        .collect();
    let disposition = format!("attachment; filename*=UTF-8''{encoded}");

    if let Ok(v) = HeaderValue::from_str(&disposition) {
        res.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(res)
}
