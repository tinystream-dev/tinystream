// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path as FsPath;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::auth::User;
use crate::error::{ApiError, ApiResult};
use crate::library::{BACKDROP_NAMES, POSTER_NAMES, image_type, local_art, local_still};
use crate::state::AppState;
use crate::tint::Source;

const CACHE: &str = "public, max-age=604800, immutable";

async fn serve_file(path: &FsPath) -> ApiResult<Response> {
    let data = tokio::fs::read(path).await.map_err(|_| ApiError::not_found("image"))?;
    let mime = mime_guess::from_path(path).first_or_octet_stream().to_string();
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, CACHE.into())], data).into_response())
}

fn serve_upload(data: Vec<u8>) -> ApiResult<Response> {
    let mime = image_type(&data).map_err(ApiError::bad_request)?;

    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=604800, immutable")], data)
        .into_response())
}

async fn serve_remote(state: &AppState, url: &str) -> ApiResult<Response> {
    let dir = state.paths.cache_dir().join("images");

    let file = super::image_cache::remote(&state.http, &dir, url).await.map_err(|e| {
        tracing::debug!("image {url}: {e:#}");
        if e.is::<reqwest::Error>() { ApiError::not_found("image") } else { ApiError::from(e) }
    })?;

    serve_file(&file).await
}

/// Where `item_art` gets the picture from, without fetching it.
pub(super) fn item_source(
    id: i64,
    path: &str,
    kind: &'static str,
    remote: Option<&str>,
    custom: bool,
    version: i64,
) -> Option<Source> {
    let names = if kind == "poster" { POSTER_NAMES } else { BACKDROP_NAMES };

    if custom {
        return Some(Source::Upload { id, kind, version });
    }

    if let Some(local) = local_art(FsPath::new(path), names) {
        return Some(Source::File(local));
    }

    remote.map(|url| Source::Remote(url.into()))
}

pub async fn item(
    State(state): State<Arc<AppState>>,
    user: User,
    Path((id, kind)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let library: String = sqlx::query_scalar("SELECT library FROM items WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("image"))?;

    user.can_access(&state, &library).await?;
    item_art(&state, id, &kind).await
}

pub(super) async fn item_art(state: &AppState, id: i64, kind: &str) -> ApiResult<Response> {
    let (path, poster, backdrop, custom_poster, custom_backdrop): (
        String,
        Option<String>,
        Option<String>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
    ) = sqlx::query_as("SELECT path, poster, backdrop, poster_override, backdrop_override FROM items WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("image"))?;

    let (names, remote, custom) = match kind {
        "poster" => (POSTER_NAMES, poster, custom_poster),
        "backdrop" => (BACKDROP_NAMES, backdrop, custom_backdrop),
        _ => return Err(ApiError::not_found("image")),
    };

    if let Some(data) = custom {
        return serve_upload(data);
    }

    if let Some(local) = local_art(FsPath::new(&path), names) {
        return serve_file(&local).await;
    }

    serve_remote(state, &remote.ok_or_else(|| ApiError::not_found("image"))?).await
}

pub async fn season(
    State(state): State<Arc<AppState>>,
    user: User,
    Path((id, number)): Path<(i64, i64)>,
) -> ApiResult<Response> {
    let (library, poster): (String, Option<String>) = sqlx::query_as(
        "SELECT i.library, s.poster FROM seasons s JOIN items i ON i.id = s.item_id WHERE s.item_id = ? AND s.number = ?",
    )
    .bind(id)
    .bind(number)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("image"))?;

    user.can_access(&state, &library).await?;
    serve_remote(&state, &poster.ok_or_else(|| ApiError::not_found("image"))?).await
}

pub async fn still(State(state): State<Arc<AppState>>, user: User, Path(id): Path<i64>) -> ApiResult<Response> {
    let library: String =
        sqlx::query_scalar("SELECT i.library FROM media m JOIN items i ON i.id = m.item_id WHERE m.id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| ApiError::not_found("image"))?;

    user.can_access(&state, &library).await?;
    episode_still(&state, id).await
}

pub(super) async fn episode_still(state: &AppState, id: i64) -> ApiResult<Response> {
    let (path, still, custom): (String, Option<String>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT path, still, still_override FROM media WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| ApiError::not_found("image"))?;

    if let Some(data) = custom {
        return serve_upload(data);
    }

    if let Some(local) = local_still(FsPath::new(&path)) {
        return serve_file(&local).await;
    }

    if let Some(url) = still
        && let Ok(res) = serve_remote(state, &url).await
    {
        return Ok(res);
    }

    let thumb = state.media.thumbnail(FsPath::new(&path)).await.map_err(|e| {
        tracing::debug!("thumbnail for {path}: {e:#}");
        ApiError::not_found("image")
    })?;

    serve_file(&thumb).await
}
