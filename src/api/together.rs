// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;

use async_graphql::{Context, InputObject, Object};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::{SinkExt, StreamExt};
use rand::seq::IndexedRandom;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use super::library::{Title, Video};
use super::media::{StreamQuery, font_file, preview_frame, stream_file, subtitle_file};
use super::schema::{Access, Ctx};
use super::{images, users};
use crate::auth::{self, User};
use crate::error::{ApiError, ApiResult};
use crate::notifications;
use crate::state::AppState;
use crate::together::{Command, ENDED, Room, Tracks, now_ms};

pub(super) async fn enter(state: &AppState, code: &str, headers: &HeaderMap) -> ApiResult<(Arc<Room>, Option<User>)> {
    let room = state.together.open(&state.db, code).await?.ok_or_else(|| ApiError::not_found("room"))?;
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
                u.id == info.host_id
                    || info.invited.contains(&u.id)
                    || u.libraries(state).await?.contains(&info.library)
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

async fn room_media(state: &AppState, room: &Room, id: i64) -> ApiResult<(PathBuf, i64)> {
    let (path, item_id): (String, i64) = sqlx::query_as("SELECT path, item_id FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("video"))?;

    if item_id != room.info().item_id {
        return Err(ApiError::not_found("video"));
    }

    Ok((PathBuf::from(path), item_id))
}

#[derive(InputObject, Default)]
pub struct TracksInput {
    audio: Option<i64>,
    audio_language: Option<String>,

    subtitle: Option<String>,
    subtitle_language: Option<String>,
    subtitle_title: Option<String>,
    #[graphql(default)]
    subtitle_forced: bool,
    subtitle_codec: Option<String>,
}

#[derive(InputObject)]
pub struct NewRoom {
    video_id: i64,

    #[graphql(default)]
    position: f64,
    #[graphql(default)]
    paused: bool,
    #[graphql(default)]
    tracks: TracksInput,

    #[graphql(default)]
    public: bool,
}

pub struct RoomView {
    room: Arc<Room>,
    user: Option<User>,
    host: User,
}

impl RoomView {
    fn access(&self) -> Arc<Access> {
        Arc::new(Access::Room {
            code: self.room.code.clone(),
            host: self.host.clone(),
            item_id: self.room.info().item_id,
        })
    }
}

#[Object(name = "Room")]
impl RoomView {
    async fn code(&self) -> &str {
        &self.room.code
    }

    async fn title(&self, ctx: &Context<'_>) -> ApiResult<Title> {
        Title::load(ctx.state(), &self.access(), self.room.info().item_id)
            .await?
            .ok_or_else(|| ApiError::not_found("title"))
    }

    async fn video(&self, ctx: &Context<'_>, id: Option<i64>) -> ApiResult<Option<Video>> {
        Video::load(ctx.state(), &self.access(), id.unwrap_or(self.room.info().media_id)).await
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

async fn view(state: &AppState, code: &str, headers: &HeaderMap) -> ApiResult<RoomView> {
    let (room, user) = enter(state, code, headers).await?;
    let host = auth::load_user(state, room.info().host_id).await?.ok_or_else(|| ApiError::not_found("room"))?;
    Ok(RoomView { room, user, host })
}

#[derive(Default)]
pub struct RoomQuery;

#[Object]
impl RoomQuery {
    async fn room(&self, ctx: &Context<'_>, code: String) -> ApiResult<RoomView> {
        view(ctx.state(), &code, &ctx.session().headers).await
    }
}

#[derive(Default)]
pub struct RoomMutation;

#[Object]
impl RoomMutation {
    async fn start_room(&self, ctx: &Context<'_>, input: NewRoom) -> ApiResult<RoomView> {
        let state = ctx.state();
        let user = ctx.allowed(|p| p.watch_together)?;

        let (item_id, library): (i64, String) =
            sqlx::query_as("SELECT m.item_id, i.library FROM media m JOIN items i ON i.id = m.item_id WHERE m.id = ?")
                .bind(input.video_id)
                .fetch_optional(&state.db)
                .await?
                .ok_or_else(|| ApiError::not_found("video"))?;

        user.can_access(state, &library).await?;

        if input.public && !user.permissions.share_links {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "you can't make public links"));
        }

        let position = if input.position.is_finite() { input.position } else { 0.0 };
        let t = input.tracks;

        let room = state
            .together
            .create(
                &state.db,
                crate::together::NewRoom {
                    host_id: user.id,
                    host_name: user.username.clone(),
                    media_id: input.video_id,
                    item_id,
                    library,
                    public: input.public,
                    tracks: Tracks {
                        media_id: input.video_id,
                        audio: t.audio,
                        audio_language: t.audio_language,
                        subtitle: t.subtitle,
                        subtitle_language: t.subtitle_language,
                        subtitle_title: t.subtitle_title,
                        subtitle_forced: t.subtitle_forced,
                        subtitle_codec: t.subtitle_codec,
                    },
                    position,
                    paused: input.paused,
                },
            )
            .await?;

        tracing::info!("{} started watching together", user.username);
        Ok(RoomView { room, user: Some(user.clone()), host: user.clone() })
    }
}

#[derive(Deserialize)]
pub struct JoinQuery {
    name: Option<String>,
}

const GUEST_NAMES: &[&str] = &[
    "Otter", "Heron", "Lynx", "Marten", "Puffin", "Ibex", "Wren", "Gecko", "Tapir", "Finch", "Axolotl", "Koala",
    "Badger", "Quokka", "Moth", "Kestrel",
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
        && let Err(e) = state.together.save(&state.db, &room).await
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

    let result = match cmd {
        Command::Ping { id, c } => return Some(json!({ "type": "pong", "id": id, "c": c, "s": now_ms() }).to_string()),
        Command::Media { id, from } => {
            if !room.can_control(me) {
                Err("only the host can change episodes in this room")
            } else {
                match room_media(state, room, id).await {
                    Ok(_) => {
                        if room.set_media(me, id, from)
                            && let Err(e) = state.together.save(&state.db, room).await
                        {
                            tracing::warn!("can't save the room: {e}");
                        }
                        Ok(())
                    },
                    Err(_) => Err("that isn't part of this room"),
                }
            }
        },
        Command::Settings { control, wait_for_all, public } => {
            if !is_host {
                Err("only whoever started the room can change it")
            } else if public == Some(true) && !user.is_some_and(|u| u.permissions.share_links) {
                Err("you can't make public links")
            } else {
                room.set_settings(me, control, wait_for_all, public);
                let _ = state.together.save(&state.db, room).await;
                Ok(())
            }
        },
        Command::Invite { user_id } => match user {
            Some(u) if u.permissions.watch_together => invite(state, room, u, user_id).await,
            _ => Err("you can't invite people"),
        },
        Command::End => {
            if is_host {
                if let Err(e) = state.together.end(&state.db, &room.code).await {
                    tracing::warn!("can't end the room: {e}");
                }

                notifications::withdraw(state, &format!("/together/{}", room.code)).await;
                Ok(())
            } else {
                Err("only whoever started the room can end it")
            }
        },
        other => room.command(me, other),
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
    let _ = state.together.save(&state.db, room).await;
    let info = room.info();
    let title: String = sqlx::query_scalar("SELECT title FROM items WHERE id = ?")
        .bind(info.item_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or_default();
    let code = &room.code;
    notifications::send(
        state,
        &[user_id],
        notifications::New {
            kind: "invite",
            priority: true,
            title: format!("{} invited you to watch {title}", from.username),
            body: Some("Join them and you'll be in sync.".into()),
            image: Some(format!("/api/together/{code}/art/poster")),
            link: Some(format!("/together/{code}")),
            actor_id: Some(from.id),

            expires_at: Some(crate::db::now() + 6 * 3600),
            replace: true,
        },
    )
    .await;
    tracing::info!("{} invited someone to watch together", from.username);
    Ok(())
}

pub async fn stream(
    State(state): State<Arc<AppState>>,
    Path((code, id)): Path<(String, i64)>,
    Query(q): Query<StreamQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, user) = enter(&state, &code, &headers).await?;
    let (path, _) = room_media(&state, &room, id).await?;
    let who = user.map(|u| u.username).unwrap_or_else(|| "a guest".into());
    Ok(stream_file(&state, &who, path, q))
}

pub async fn subtitles(
    State(state): State<Arc<AppState>>,
    Path((code, id, track)): Path<(String, i64, String)>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    let (path, _) = room_media(&state, &room, id).await?;
    subtitle_file(&state, &path, &track).await
}

pub async fn font(
    State(state): State<Arc<AppState>>,
    Path((code, id, index)): Path<(String, i64, usize)>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    let (path, _) = room_media(&state, &room, id).await?;
    font_file(path, index).await
}

pub async fn preview(
    State(state): State<Arc<AppState>>,
    Path((code, id, at)): Path<(String, i64, u32)>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    let (path, _) = room_media(&state, &room, id).await?;
    preview_frame(&state, &path, at).await
}

pub async fn still(
    State(state): State<Arc<AppState>>,
    Path((code, id)): Path<(String, i64)>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    room_media(&state, &room, id).await?;
    images::episode_still(&state, id).await
}

pub async fn art(
    State(state): State<Arc<AppState>>,
    Path((code, kind)): Path<(String, String)>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let (room, _) = enter(&state, &code, &headers).await?;
    images::item_art(&state, room.info().item_id, &kind).await
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
