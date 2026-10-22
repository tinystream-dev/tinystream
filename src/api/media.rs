// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::Deserialize;
use tokio_stream_shim::ReceiverStream;

use crate::auth::User;
use crate::error::{ApiError, ApiResult};
use crate::media::stream::{self, AudioMode, StreamRequest, VideoMode};
use crate::state::AppState;

async fn media_path(state: &AppState, user: &User, id: i64) -> ApiResult<(PathBuf, i64, String)> {
    let row: Option<(String, i64, String)> = sqlx::query_as(
        "SELECT m.path, m.item_id, i.library FROM media m JOIN items i ON i.id = m.item_id WHERE m.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let (path, item_id, library) = row.ok_or_else(|| ApiError::not_found("video"))?;
    user.can_access(state, &library).await?;
    Ok((PathBuf::from(path), item_id, library))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StreamQuery {
    #[serde(default)]
    start: f64,

    #[serde(default)]
    video: Option<String>,
    height: Option<i32>,
    audio: Option<usize>,

    audio_mode: Option<String>,
}

pub async fn stream(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<StreamQuery>,
) -> ApiResult<Response> {
    let (path, _, _) = media_path(&state, &user, id).await?;
    Ok(stream_file(&state, &user.username, path, q))
}

pub(super) fn stream_file(state: &AppState, who: &str, path: PathBuf, q: StreamQuery) -> Response {
    let video = match q.video.as_deref() {
        Some("transcode") => VideoMode::Transcode { max_height: q.height.unwrap_or(1080) },
        _ => VideoMode::Copy,
    };

    let audio = match q.audio_mode.as_deref() {
        Some("aac") => AudioMode::Aac,
        _ => AudioMode::Copy,
    };

    tracing::info!(
        "{who} is playing {} from {:.0}s ({})",
        path.file_name().unwrap_or_default().to_string_lossy(),
        q.start,
        match (video, audio) {
            (VideoMode::Copy, AudioMode::Copy) => "direct".to_string(),
            (VideoMode::Copy, AudioMode::Aac) => "audio transcode".to_string(),
            (VideoMode::Transcode { max_height }, _) => format!("transcode to {max_height}p"),
        }
    );

    let hw = match video {
        VideoMode::Transcode { .. } => state.media.hw.device(),
        VideoMode::Copy => None,
    };

    let hold = (video != VideoMode::Copy || audio != AudioMode::Copy).then(|| state.media.busy.hold());
    let rx = stream::spawn(StreamRequest { path, start: q.start.max(0.0), video, audio_stream: q.audio, audio }, hw);

    let chunks = ReceiverStream(rx).map(move |c| {
        let _ = &hold;
        c
    });

    let mut res = Body::from_stream(chunks).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));

    h.insert("x-accel-buffering", HeaderValue::from_static("no"));
    res
}

pub async fn subtitles(
    State(state): State<Arc<AppState>>,
    user: User,
    Path((id, track)): Path<(i64, String)>,
) -> ApiResult<Response> {
    let (path, _, _) = media_path(&state, &user, id).await?;
    subtitle_file(&state, &path, &track).await
}

pub(super) async fn subtitle_file(state: &AppState, path: &FsPath, track: &str) -> ApiResult<Response> {
    let text = state.media.subtitles(path, track).await.map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

    Ok(([(header::CONTENT_TYPE, "text/x-ssa; charset=utf-8"), (header::CACHE_CONTROL, "private, max-age=3600")], text)
        .into_response())
}

pub async fn font(
    State(state): State<Arc<AppState>>,
    user: User,
    Path((id, index)): Path<(i64, usize)>,
) -> ApiResult<Response> {
    let (path, _, _) = media_path(&state, &user, id).await?;
    font_file(path, index).await
}

pub(super) async fn font_file(path: PathBuf, index: usize) -> ApiResult<Response> {
    let (name, data) = tokio::task::spawn_blocking(move || crate::media::probe::attachment(&path, index))
        .await
        .map_err(|e| ApiError::from(anyhow::anyhow!(e)))?
        .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

    let mime = mime_guess::from_path(&name).first_or_octet_stream().to_string();
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=86400".into())], data).into_response())
}

pub(super) mod tokio_stream_shim {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use tokio::sync::mpsc::Receiver;

    pub struct ReceiverStream<T>(pub Receiver<T>);

    impl<T> futures::Stream for ReceiverStream<T> {
        type Item = T;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
            self.0.poll_recv(cx)
        }
    }
}

pub async fn preview(
    State(state): State<Arc<AppState>>,
    user: User,
    Path((id, at)): Path<(i64, u32)>,
) -> ApiResult<Response> {
    let (path, _, _) = media_path(&state, &user, id).await?;
    preview_frame(&state, &path, at).await
}

pub(super) async fn preview_frame(state: &AppState, path: &FsPath, at: u32) -> ApiResult<Response> {
    let file = state.media.preview(path, at).await.map_err(|e| {
        tracing::debug!("preview for {}: {e:#}", path.display());
        ApiError::not_found("frame")
    })?;

    let data = tokio::fs::read(&file).await.map_err(|_| ApiError::not_found("frame"))?;

    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=604800, immutable")], data)
        .into_response())
}
