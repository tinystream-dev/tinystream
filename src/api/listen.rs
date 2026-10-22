// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use async_graphql::{Context, InputObject, Object};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::{SinkExt, StreamExt};
use rand::seq::IndexedRandom;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use super::music::{
    Base, CoverQuery, StreamQuery, TrackView, flac_copy, image, serve_track, track_cover_file, track_views, transcoded,
};
use super::schema::Ctx;
use super::users;
use crate::auth::{self, User};
use crate::error::{ApiError, ApiResult};
use crate::media::audio::Format;
use crate::music::catalog::{self, Track};
use crate::music::listen::{Command, ENDED, NewRoom, Room};
use crate::notifications;
use crate::state::AppState;
use crate::together::now_ms;

/// Every music library, for what's in a room: whoever's in it may hear what the host queued.
fn all_music(state: &AppState) -> Vec<String> {
    state.config.current().libraries.iter().filter(|l| l.is_music()).map(|l| l.name.clone()).collect()
}

async fn enter(state: &AppState, code: &str, headers: &HeaderMap) -> ApiResult<(Arc<Room>, Option<User>)> {
    let room = state.music.listen.open(&state.db, code).await?.ok_or_else(|| ApiError::not_found("room"))?;
    let info = room.info();

    let user = match auth::token_from(headers) {
        Some(t) => auth::user_from_token(state, &t).await?,
        None => None,
    };

    let host = auth::load_user(state, info.host_id).await?.ok_or_else(|| ApiError::not_found("room"))?;

    if !host.permissions.watch_together {
        return Err(ApiError::not_found("room"));
    }

    let public = info.public && host.permissions.share_links;

    let allowed = public
        || match &user {
            Some(u) => {
                u.id == info.host_id || info.invited.contains(&u.id) || queue_visible(state, u, &info.queue).await?
            },
            None => false,
        };

    if !allowed {
        return Err(match user {
            None => ApiError::new(StatusCode::UNAUTHORIZED, "sign in to join this room"),
            Some(_) => {
                ApiError::new(StatusCode::FORBIDDEN, "this room is private; ask whoever started it for an invite")
            },
        });
    }

    Ok((room, user))
}

/// Whether someone could see everything queued anyway.
async fn queue_visible(state: &AppState, user: &User, queue: &[i64]) -> ApiResult<bool> {
    let libs = crate::music::libraries(state, user);
    Ok(!libs.is_empty() && catalog::tracks(&state.db, &libs, queue).await?.len() == queue.len())
}

async fn room_track(state: &AppState, room: &Room, id: i64) -> ApiResult<Track> {
    if !room.holds(id) {
        return Err(ApiError::not_found("track"));
    }
    catalog::track(&state.db, id).await?.ok_or_else(|| ApiError::not_found("track"))
}

pub struct ListenRoomView {
    room: Arc<Room>,
    user: Option<User>,
    host: User,
}

#[Object(name = "ListenRoom")]
impl ListenRoomView {
    async fn code(&self) -> &str {
        &self.room.code
    }

    /// What's queued, in order; the room's socket says which is playing.
    async fn tracks(&self, ctx: &Context<'_>) -> ApiResult<Vec<TrackView>> {
        let state = ctx.state();
        let queue = self.room.info().queue;
        let mut unique = queue.clone();
        unique.sort_unstable();
        unique.dedup();
        let found = catalog::tracks(&state.db, &all_music(state), &unique).await?;
        let ordered: Vec<Track> = queue.iter().filter_map(|id| found.iter().find(|t| t.id == *id).cloned()).collect();
        track_views(state, self.user.as_ref().map(|u| u.id), ordered, &Base::room(&self.room.code)).await
    }

    async fn host_name(&self) -> &str {
        &self.host.username
    }

    async fn signed_in(&self) -> bool {
        self.user.is_some()
    }

    async fn is_host(&self) -> bool {
        self.user.as_ref().is_some_and(|u| u.id == self.host.id)
    }

    async fn can_share(&self) -> bool {
        self.user.as_ref().is_some_and(|u| u.id == self.host.id && u.permissions.share_links)
    }

    async fn can_invite(&self) -> bool {
        self.user.as_ref().is_some_and(|u| u.permissions.watch_together)
    }
}

async fn view(state: &AppState, code: &str, headers: &HeaderMap) -> ApiResult<ListenRoomView> {
    let (room, user) = enter(state, code, headers).await?;
    let host = auth::load_user(state, room.info().host_id).await?.ok_or_else(|| ApiError::not_found("room"))?;
    Ok(ListenRoomView { room, user, host })
}

#[derive(Default)]
pub struct ListenQuery;

#[Object]
impl ListenQuery {
    async fn listen_room(&self, ctx: &Context<'_>, code: String) -> ApiResult<ListenRoomView> {
        view(ctx.state(), &code, &ctx.session().headers).await
    }
}

#[derive(InputObject)]
pub struct NewListenRoom {
    tracks: Vec<i64>,
    #[graphql(default)]
    current: i64,
    #[graphql(default)]
    position: f64,
    #[graphql(default)]
    paused: bool,
    #[graphql(default)]
    public: bool,
}

/// Tracks someone may queue, with how long each is.
async fn checked(state: &AppState, user: &User, ids: &[i64]) -> ApiResult<Vec<(i64, f64)>> {
    let libs = crate::music::libraries(state, user);
    let found = catalog::tracks(&state.db, &libs, ids).await?;
    Ok(found.into_iter().map(|t| (t.id, t.duration)).collect())
}

#[derive(Default)]
pub struct ListenMutation;

#[Object]
impl ListenMutation {
    async fn start_listen_room(&self, ctx: &Context<'_>, input: NewListenRoom) -> ApiResult<ListenRoomView> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.watch_together)?;

        if input.public && !user.permissions.share_links {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "you can't make public links"));
        }

        let tracks = checked(state, user, &input.tracks).await?;

        if tracks.is_empty() {
            return Err(ApiError::bad_request("queue something to listen to first"));
        }

        let room = state
            .music
            .listen
            .create(
                &state.db,
                NewRoom {
                    host_id: user.id,
                    host_name: user.username.clone(),
                    tracks,
                    current: input.current.max(0) as usize,
                    position: if input.position.is_finite() { input.position } else { 0.0 },
                    paused: input.paused,
                    public: input.public,
                },
            )
            .await?;

        tracing::info!("{} started listening together", user.username);
        Ok(ListenRoomView { room, user: Some(user.clone()), host: user.clone() })
    }
}

#[derive(Deserialize)]
pub struct JoinQuery {
    name: Option<String>,
}

const GUEST_NAMES: &[&str] = &[
    "Starling", "Nightjar", "Warbler", "Lark", "Thrush", "Oriole", "Bunting", "Linnet", "Siskin", "Bulbul", "Tanager",
    "Vireo", "Wagtail", "Dipper", "Pipit", "Robin",
];

pub async fn socket(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Query(q): Query<JoinQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    let (room, user) = enter(&state, &code, &headers).await?;

    let name = match &user {
        Some(u) => u.username.clone(),
        None => q
            .name
            .map(|n| n.trim().chars().take(40).collect::<String>())
            .filter(|n| !n.is_empty() && !n.chars().any(char::is_control))
            .unwrap_or_else(|| format!("Guest {}", GUEST_NAMES.choose(&mut rand::rng()).unwrap_or(&"Guest"))),
    };

    Ok(ws.on_upgrade(move |socket| connection(state, room, user, name, socket)))
}

async fn connection(state: Arc<AppState>, room: Arc<Room>, user: Option<User>, name: String, socket: WebSocket) {
    let mut updates = room.subscribe();
    let me = room.join(name, user.as_ref().map(|u| u.id), user.as_ref().and_then(|u| u.avatar));
    let (mut out, mut incoming) = socket.split();
    let welcome = json!({ "type": "welcome", "you": me, "serverTime": now_ms() }).to_string();

    let mut ok = out.send(Message::Text(welcome.into())).await.is_ok()
        && out.send(Message::Text(room.current().to_string().into())).await.is_ok();

    while ok {
        tokio::select! {
            msg = incoming.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    if let Some(reply) = handle(&state, &room, me, user.as_ref(), &text).await {
                        ok = out.send(Message::Text(reply.into())).await.is_ok();
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            update = updates.recv() => match update {
                Ok(snapshot) => {
                    ok = out.send(Message::Text(snapshot.to_string().into())).await.is_ok();

                    if &*snapshot == ENDED {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => ok = out.send(Message::Text(room.current().to_string().into())).await.is_ok(),
                Err(RecvError::Closed) => break,
            },
        }
    }

    if room.leave(me)
        && let Err(e) = state.music.listen.save(&state.db, &room).await
    {
        tracing::warn!("can't save the room: {e}");
    }
}

fn error(message: &str) -> String {
    json!({ "type": "error", "message": message }).to_string()
}

async fn handle(state: &AppState, room: &Arc<Room>, me: u64, user: Option<&User>, text: &str) -> Option<String> {
    let Ok(cmd) = serde_json::from_str::<Command>(text) else {
        return Some(error("didn't understand that"));
    };

    let info = room.info();
    let is_host = user.is_some_and(|u| u.id == info.host_id);

    let save = |r: Result<(), &'static str>| async move {
        if r.is_ok()
            && let Err(e) = state.music.listen.save(&state.db, room).await
        {
            tracing::warn!("can't save the room: {e}");
        }
        r
    };

    let result = match cmd {
        Command::Ping { id, c } => return Some(json!({ "type": "pong", "id": id, "c": c, "s": now_ms() }).to_string()),
        Command::Queue { tracks, current, position } => match user {
            _ if !room.can_control(me) => Err("only the host can change the queue in this room"),
            None => Err("sign in to change what's queued"),
            Some(u) => match checked(state, u, &tracks).await {
                Ok(t) if !t.is_empty() => {
                    room.set_queue(me, t, current, position);
                    save(Ok(())).await
                },
                _ => Err("none of that can be played here"),
            },
        },
        Command::Add { tracks, next } => match user {
            _ if !room.can_control(me) => Err("only the host can change the queue in this room"),
            None => Err("sign in to add to the queue"),
            Some(u) => match checked(state, u, &tracks).await {
                Ok(t) if !t.is_empty() => {
                    room.add(me, t, next);
                    save(Ok(())).await
                },
                _ => Err("none of that can be played here"),
            },
        },
        Command::Settings { control, wait_for_all, public } => {
            if !is_host {
                Err("only whoever started the room can change it")
            } else if public == Some(true) && !user.is_some_and(|u| u.permissions.share_links) {
                Err("you can't make public links")
            } else {
                room.set_settings(me, control, wait_for_all, public);
                save(Ok(())).await
            }
        },
        Command::Invite { user_id } => match user {
            Some(u) if u.permissions.watch_together => invite(state, room, u, user_id).await,
            _ => Err("you can't invite people"),
        },
        Command::End => {
            if is_host {
                if let Err(e) = state.music.listen.end(&state.db, &room.code).await {
                    tracing::warn!("can't end the room: {e}");
                }

                notifications::withdraw(state, &format!("/listen/{}", room.code)).await;
                Ok(())
            } else {
                Err("only whoever started the room can end it")
            }
        },
        other => {
            let structural = matches!(other, Command::Skip { .. } | Command::Remove { .. } | Command::Move { .. });
            let r = room.command(me, other);
            if structural { save(r).await } else { r }
        },
    };

    result.err().map(error)
}

async fn invite(state: &AppState, room: &Room, from: &User, user_id: i64) -> Result<(), &'static str> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    if exists.is_none() || user_id == from.id {
        return Err("there's nobody like that to invite");
    }

    room.invite(user_id);
    let _ = state.music.listen.save(&state.db, room).await;
    let info = room.info();
    let first = info.queue.get(info.current).copied();

    let title = match first {
        Some(id) => catalog::track(&state.db, id).await.ok().flatten().map(|t| t.album).unwrap_or_default(),
        None => String::new(),
    };

    let code = &room.code;

    notifications::send(
        state,
        &[user_id],
        notifications::New {
            kind: "invite",
            priority: true,
            title: if title.is_empty() {
                format!("{} invited you to listen together", from.username)
            } else {
                format!("{} invited you to listen to {title}", from.username)
            },
            body: Some("Join them and you'll hear the same thing at the same time.".into()),
            image: first.map(|id| format!("/api/listen/{code}/tracks/{id}/cover?size=256")),
            link: Some(format!("/listen/{code}")),
            actor_id: Some(from.id),
            expires_at: Some(crate::db::now() + 6 * 3600),
            replace: true,
        },
    )
    .await;

    tracing::info!("{} invited someone to listen together", from.username);
    Ok(())
}

pub async fn file(
    State(state): State<Arc<AppState>>,
    Path((code, id)): Path<(String, i64)>,
    headers: HeaderMap,
    req: Request,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    let t = room_track(&state, &room, id).await?;
    serve_track(&t, req).await
}

pub async fn flac(
    State(state): State<Arc<AppState>>,
    Path((code, id)): Path<(String, i64)>,
    headers: HeaderMap,
    req: Request,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    let t = room_track(&state, &room, id).await?;
    serve_track(&flac_copy(&state, &t).await?, req).await
}

pub async fn stream(
    State(state): State<Arc<AppState>>,
    Path((code, id)): Path<(String, i64)>,
    Query(q): Query<StreamQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, user) = enter(&state, &code, &headers).await?;
    let t = room_track(&state, &room, id).await?;
    let who = user.map(|u| u.username).unwrap_or_else(|| "a guest".into());
    let format = q.format.as_deref().and_then(Format::parse).unwrap_or(Format::Flac);
    Ok(transcoded(&state, &who, &t, format, q.bitrate, q.start))
}

pub async fn cover(
    State(state): State<Arc<AppState>>,
    Path((code, id)): Path<(String, i64)>,
    Query(q): Query<CoverQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    let t = room_track(&state, &room, id).await?;
    image(&state, track_cover_file(&state, &t).await, q.size).await
}

pub async fn avatar(
    State(state): State<Arc<AppState>>,
    Path((code, user_id)): Path<(String, i64)>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;

    if !room.has_user(user_id) {
        return Err(ApiError::not_found("picture"));
    }

    Ok(users::avatar_image(&state, user_id).await?.into_response())
}
