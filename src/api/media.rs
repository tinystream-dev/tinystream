// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::Deserialize;
use tokio_stream_shim::ReceiverStream;
use tower::ServiceExt;
use tower_http::services::ServeFile;

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

#[derive(Deserialize)]
pub(super) struct FileQuery {
    download: Option<String>,
}

pub async fn file(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<FileQuery>,
    req: Request,
) -> ApiResult<Response> {
    let (path, _, _) = media_path(&state, &user, id).await?;
    original_file(&user.username, &path, q, req).await
}

/// The file as it is, in whatever range was asked for.
pub(super) async fn original_file(who: &str, path: &FsPath, q: FileQuery, req: Request) -> ApiResult<Response> {
    let download = q.download.is_some();
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    let first =
        req.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).is_none_or(|v| v.starts_with("bytes=0-"));

    if first && req.method() != Method::HEAD {
        if download {
            tracing::info!("{who} is downloading {name}");
        } else {
            tracing::info!("{who} is playing {name} (direct)");
        }
    }

    let Ok(res) = ServeFile::new(path).oneshot(req).await;

    if res.status() == StatusCode::NOT_FOUND {
        return Err(ApiError::not_found("file"));
    }

    let mut res = res.map(Body::new);
    let h = res.headers_mut();
    let mime = mime_guess::from_path(path).first_or_octet_stream().to_string();

    if let Ok(v) = HeaderValue::from_str(&mime) {
        h.insert(header::CONTENT_TYPE, v);
    }

    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));

    if download && let Ok(v) = HeaderValue::from_str(&super::clips::content_disposition("attachment", &name)) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }

    Ok(res)
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

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::http::Request as HttpRequest;

    use super::*;

    fn video() -> (PathBuf, Vec<u8>) {
        let path = std::env::temp_dir().join(format!("tinystream-file-{}", uuid::Uuid::new_v4())).join("Ça va.mkv");
        let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &data).unwrap();
        (path, data)
    }

    async fn fetch(path: &FsPath, method: Method, range: Option<&str>, download: bool) -> (Response, Vec<u8>) {
        let mut req = HttpRequest::builder().method(method);

        if let Some(r) = range {
            req = req.header(header::RANGE, r);
        }

        let q = FileQuery { download: download.then(|| "1".into()) };
        let res = original_file("someone", path, q, req.body(Body::empty()).unwrap()).await.unwrap();
        let (parts, body) = res.into_parts();
        let body = to_bytes(body, usize::MAX).await.unwrap().to_vec();
        (Response::from_parts(parts, Body::empty()), body)
    }

    fn header<'a>(res: &'a Response, name: header::HeaderName) -> &'a str {
        res.headers().get(name).unwrap().to_str().unwrap()
    }

    #[tokio::test]
    async fn ranges() {
        let (path, data) = video();

        let (res, body) = fetch(&path, Method::GET, None, false).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(header(&res, header::CONTENT_TYPE), "video/x-matroska");
        assert_eq!(header(&res, header::ACCEPT_RANGES), "bytes");
        assert_eq!(body, data);

        let (res, body) = fetch(&path, Method::GET, Some("bytes=0-9"), false).await;
        assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&res, header::CONTENT_RANGE), "bytes 0-9/1000");
        assert_eq!(body, data[..10]);

        let (res, body) = fetch(&path, Method::GET, Some("bytes=400-599"), false).await;
        assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&res, header::CONTENT_LENGTH), "200");
        assert_eq!(body, data[400..600]);

        let (res, body) = fetch(&path, Method::GET, Some("bytes=-16"), false).await;
        assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&res, header::CONTENT_RANGE), "bytes 984-999/1000");
        assert_eq!(body, data[984..]);

        let (res, _) = fetch(&path, Method::GET, Some("bytes=5000-"), false).await;
        assert_eq!(res.status(), StatusCode::RANGE_NOT_SATISFIABLE);

        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn head_and_download() {
        let (path, _) = video();

        let (res, body) = fetch(&path, Method::HEAD, None, false).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(header(&res, header::CONTENT_LENGTH), "1000");
        assert!(res.headers().get(header::CONTENT_DISPOSITION).is_none());
        assert!(body.is_empty());

        let (res, _) = fetch(&path, Method::GET, None, true).await;
        assert_eq!(
            header(&res, header::CONTENT_DISPOSITION),
            "attachment; filename=\"_a va.mkv\"; filename*=UTF-8''%C3%87a%20va.mkv"
        );

        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        let req = HttpRequest::builder().body(Body::empty()).unwrap();
        let err = original_file("someone", &path, FileQuery { download: None }, req).await.err().unwrap();
        assert_eq!(err.status, StatusCode::NOT_FOUND);
    }
}
