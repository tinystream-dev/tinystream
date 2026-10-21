// SPDX-License-Identifier: AGPL-3.0-or-later

mod appearance;
mod auth;
#[cfg(feature = "torrent")]
mod automation;
mod clips;
mod discovery;
mod events;
mod image_cache;
mod images;
mod invites;
mod library;
mod listen;
mod media;
mod music;
mod notifications;
mod schema;
mod settings;
mod subsonic;
mod together;
mod upload;
mod users;

use std::sync::Arc;

use async_graphql::Data;
use async_graphql::http::{ALL_WEBSOCKET_PROTOCOLS, GraphiQLSource};
use async_graphql_axum::{GraphQLProtocol, GraphQLResponse, GraphQLWebSocket};
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use schema::{AppSchema, Session};
use serde_json::json;

use crate::auth::{token_from, user_from_token};
use crate::state::AppState;

#[derive(Clone)]
struct Api {
    state: Arc<AppState>,
    schema: AppSchema,
}

pub fn router(state: Arc<AppState>) -> Router {
    let api = Api { state: state.clone(), schema: schema::build(state.clone()) };
    let graphql = Router::new().route("/graphql", get(graphql_get).post(graphql_post)).with_state(api);

    let files = Router::new()
        .route("/users/{id}/avatar", get(users::avatar))
        .route("/sign-in/{key}/avatar", get(auth::profile_avatar))
        .route("/media/{id}/stream", get(media::stream))
        .route("/media/{id}/subtitles/{track}", get(media::subtitles))
        .route("/media/{id}/fonts/{index}", get(media::font))
        .route("/media/{id}/preview/{at}", get(media::preview))
        .route("/images/item/{id}/{kind}", get(images::item))
        .route("/images/season/{id}/{number}", get(images::season))
        .route("/images/media/{id}", get(images::still))
        .route("/together/{code}/ws", get(together::socket))
        .route("/together/{code}/media/{id}/stream", get(together::stream))
        .route("/together/{code}/media/{id}/subtitles/{track}", get(together::subtitles))
        .route("/together/{code}/media/{id}/fonts/{index}", get(together::font))
        .route("/together/{code}/media/{id}/preview/{at}", get(together::preview))
        .route("/together/{code}/stills/{id}", get(together::still))
        .route("/together/{code}/art/{kind}", get(together::art))
        .route("/together/{code}/avatars/{user}", get(together::avatar))
        .route("/music/tracks/{id}/file", get(music::file))
        .route("/music/tracks/{id}/flac", get(music::flac))
        .route("/music/tracks/{id}/stream", get(music::stream))
        .route("/music/tracks/{id}/cover", get(music::track_cover))
        .route("/music/albums/{id}/cover", get(music::album_cover))
        .route("/music/artists/{id}/cover", get(music::artist_cover))
        .route("/listen/{code}/ws", get(listen::socket))
        .route("/listen/{code}/tracks/{id}/file", get(listen::file))
        .route("/listen/{code}/tracks/{id}/flac", get(listen::flac))
        .route("/listen/{code}/tracks/{id}/stream", get(listen::stream))
        .route("/listen/{code}/tracks/{id}/cover", get(listen::cover))
        .route("/listen/{code}/avatars/{user}", get(listen::avatar))
        .route("/clips/still/{media}", get(clips::still))
        .route("/clips/{id}/video", get(clips::file))
        .route("/clips/{id}/image", get(clips::file))
        .route("/clips/{id}/poster", get(clips::poster))
        .with_state(state.clone());
    let api = graphql.merge(files).fallback(|| async {
        (StatusCode::NOT_FOUND, Json(json!({ "error": "no such endpoint; the API is at /api/graphql" })))
    });

    let public = Router::new()
        .route("/c/{code}", get(clips::page))
        .route("/c/{code}/video.mp4", get(clips::public_file))
        .route("/c/{code}/image.png", get(clips::public_file))
        .route("/c/{code}/poster.jpg", get(clips::public_poster))
        .layer(middleware::from_fn(clips::rate_limit))
        .with_state(state.clone());
    let app = Router::new().nest("/api", api).merge(public).merge(subsonic::router(state.clone()));
    #[cfg(feature = "web-ui")]
    let app = app.merge(crate::web::router());
    #[cfg(not(feature = "web-ui"))]
    let app = app.route(
        "/",
        get(|| async { "tinystream is running. This build has no web UI (built without the `web-ui` feature)." }),
    );

    app.layer(middleware::from_fn_with_state(state, cors)).layer(middleware::from_fn(isolation))
}

async fn session(state: &AppState, headers: HeaderMap) -> Session {
    let token = token_from(&headers);
    let user = match &token {
        Some(t) => user_from_token(state, t).await.ok().flatten(),
        None => None,
    };
    Session { user, token, headers }
}

async fn graphql_post(State(api): State<Api>, headers: HeaderMap, req: Request) -> Response {
    let req = match upload::graphql_request(req).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    let session = session(&api.state, headers).await;
    GraphQLResponse::from(api.schema.execute(req.data(session)).await).into_response()
}

async fn graphql_get(State(api): State<Api>, req: Request) -> Response {
    let (mut parts, _) = req.into_parts();
    let protocol = GraphQLProtocol::from_request_parts(&mut parts, &()).await;
    let upgrade = WebSocketUpgrade::from_request_parts(&mut parts, &()).await;
    let (Ok(protocol), Ok(upgrade)) = (protocol, upgrade) else {
        return Html(GraphiQLSource::build().endpoint("/api/graphql").subscription_endpoint("/api/graphql").finish())
            .into_response();
    };
    let headers = parts.headers;
    let session = session(&api.state, headers).await;
    let state = api.state.clone();
    upgrade
        .protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |socket| async move {
            GraphQLWebSocket::new(socket, api.schema, protocol)
                .on_connection_init(move |payload| async move {
                    let mut session = session;
                    if session.user.is_none()
                        && let Some(token) = payload.get("token").and_then(|t| t.as_str())
                    {
                        session.user = user_from_token(&state, token).await.ok().flatten();
                        session.token = Some(token.to_string());
                    }
                    let mut data = Data::default();
                    data.insert(session);
                    Ok(data)
                })
                .serve()
                .await
        })
        .into_response()
}

async fn isolation(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    h.insert("cross-origin-embedder-policy", HeaderValue::from_static("credentialless"));
    res
}

async fn cors(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let origin = req.headers().get(header::ORIGIN).cloned();
    let allowed = origin.as_ref().and_then(|o| {
        let config = state.config.current();
        let list = &config.network.cors;
        if list.iter().any(|c| c == "*") {
            Some(HeaderValue::from_static("*"))
        } else {
            let o_str = o.to_str().ok()?;
            list.iter().any(|c| c.trim_end_matches('/') == o_str).then(|| o.clone())
        }
    });
    let Some(allow) = allowed else {
        return next.run(req).await;
    };
    let preflight =
        req.method() == Method::OPTIONS && req.headers().contains_key(header::ACCESS_CONTROL_REQUEST_METHOD);
    let mut res = if preflight { StatusCode::NO_CONTENT.into_response() } else { next.run(req).await };
    let h = res.headers_mut();
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, allow);
    h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE"));
    h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("authorization, content-type"));
    h.insert(header::ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("3600"));
    h.append(header::VARY, HeaderValue::from_static("origin"));
    res
}
