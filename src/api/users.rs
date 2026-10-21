// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use async_graphql::{Context, InputObject, Object, Upload};
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use webauthn_rs::prelude::Uuid;

use super::auth::{Passkey, passkeys};
use super::schema::Ctx;
use crate::auth::permissions::{self, Overrides, Permissions};
use crate::auth::{self, User};
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::events::Event;
use crate::state::AppState;

fn own_or_admin<'a>(ctx: &'a Context<'_>, user: &User) -> ApiResult<&'a User> {
    let me = ctx.user()?;
    if me.id == user.id || me.is_admin { Ok(me) } else { Err(ApiError::forbidden()) }
}

#[Object]
impl User {
    async fn id(&self) -> i64 {
        self.id
    }

    async fn username(&self) -> &str {
        &self.username
    }

    async fn is_admin(&self) -> bool {
        self.is_admin
    }

    async fn avatar(&self) -> Option<String> {
        self.avatar.map(|v| format!("/api/users/{}/avatar?v={v}", self.id))
    }

    async fn permissions(&self, ctx: &Context<'_>) -> ApiResult<&Permissions> {
        own_or_admin(ctx, self)?;
        Ok(&self.permissions)
    }

    async fn overrides(&self, ctx: &Context<'_>) -> ApiResult<Overrides> {
        ctx.admin()?;

        let json: String = sqlx::query_scalar("SELECT permissions FROM users WHERE id = ?")
            .bind(self.id)
            .fetch_one(&ctx.state().db)
            .await?;

        Ok(Overrides::parse(&json))
    }

    async fn created_at(&self, ctx: &Context<'_>) -> ApiResult<i64> {
        own_or_admin(ctx, self)?;

        Ok(sqlx::query_scalar("SELECT created_at FROM users WHERE id = ?")
            .bind(self.id)
            .fetch_one(&ctx.state().db)
            .await?)
    }

    async fn last_seen(&self, ctx: &Context<'_>) -> ApiResult<Option<i64>> {
        ctx.admin()?;

        Ok(sqlx::query_scalar("SELECT MAX(last_seen) FROM sessions WHERE user_id = ?")
            .bind(self.id)
            .fetch_one(&ctx.state().db)
            .await?)
    }

    async fn passkeys(&self, ctx: &Context<'_>) -> ApiResult<Vec<Passkey>> {
        if ctx.user()?.id != self.id {
            return Err(ApiError::forbidden());
        }
        passkeys(ctx.state(), self.id).await
    }

    async fn email(&self, ctx: &Context<'_>) -> ApiResult<Option<String>> {
        if ctx.user()?.id != self.id {
            return Err(ApiError::forbidden());
        }

        Ok(sqlx::query_scalar("SELECT email FROM users WHERE id = ?").bind(self.id).fetch_one(&ctx.state().db).await?)
    }
}

async fn load(state: &AppState, id: i64) -> ApiResult<User> {
    auth::load_user(state, id).await?.ok_or_else(|| ApiError::not_found("account"))
}

#[derive(Default)]
pub struct UserQuery;

#[Object]
impl UserQuery {
    async fn users(&self, ctx: &Context<'_>) -> ApiResult<Vec<User>> {
        let state = ctx.state();
        ctx.user()?;
        let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM users ORDER BY created_at").fetch_all(&state.db).await?;
        let mut out = Vec::with_capacity(ids.len());

        for id in ids {
            out.extend(auth::load_user(state, id).await?);
        }

        Ok(out)
    }

    async fn user(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<User>> {
        ctx.user()?;
        auth::load_user(ctx.state(), id).await
    }

    async fn permission_defaults(&self, ctx: &Context<'_>) -> ApiResult<Permissions> {
        ctx.admin()?;
        permissions::defaults(ctx.state()).await
    }
}

#[derive(InputObject)]
pub struct NewUser {
    username: String,
    password: String,
    #[graphql(default)]
    is_admin: bool,
    #[graphql(default)]
    permissions: Overrides,
}

#[derive(InputObject)]
pub struct UserPatch {
    username: Option<String>,
    is_admin: Option<bool>,

    password: Option<String>,

    permissions: Option<Overrides>,
}

async fn admin_count(state: &AppState) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_admin = 1").fetch_one(&state.db).await?)
}

const AVATAR_MAX: usize = 512 * 1024;

fn image_type(data: &[u8]) -> ApiResult<&'static str> {
    if data.len() > AVATAR_MAX {
        return Err(ApiError::bad_request("that picture is too big (512 KB at most)"));
    }
    match data {
        [0x89, b'P', b'N', b'G', ..] => Ok("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Ok("image/jpeg"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Ok("image/webp"),
        [b'G', b'I', b'F', b'8', ..] => Ok("image/gif"),
        _ => Err(ApiError::bad_request("pictures have to be PNG, JPEG, WebP or GIF")),
    }
}

fn avatar_owner(ctx: &Context<'_>, user_id: Option<i64>) -> ApiResult<i64> {
    let me = ctx.user()?;

    match user_id {
        Some(id) if id != me.id => ctx.admin().map(|_| id),
        _ => Ok(me.id),
    }
}

#[derive(Default)]
pub struct UserMutation;

#[Object]
impl UserMutation {
    async fn create_user(&self, ctx: &Context<'_>, input: NewUser) -> ApiResult<User> {
        let (state, admin) = (ctx.state(), ctx.admin()?);
        auth::validate_credentials(&input.username, &input.password)?;
        let hash = auth::hash_password(input.password).await?;

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (handle, username, password_hash, is_admin, permissions, created_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(username) DO NOTHING RETURNING id",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(input.username.trim())
        .bind(hash)
        .bind(input.is_admin)
        .bind(input.permissions.to_json())
        .bind(now())
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::conflict(format!("{:?} is taken", input.username.trim())))?;

        tracing::info!("{} created the account {:?}", admin.username, input.username.trim());
        state.events.send(Event::UsersChanged);
        load(state, id).await
    }

    async fn update_user(&self, ctx: &Context<'_>, id: i64, input: UserPatch) -> ApiResult<User> {
        let (state, admin) = (ctx.state(), ctx.admin()?);

        let (mut username, was_admin): (String, bool) =
            sqlx::query_as("SELECT username, is_admin FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&state.db)
                .await?
                .ok_or_else(|| ApiError::not_found("account"))?;

        if let Some(new) = input.username.as_deref().map(str::trim).filter(|n| *n != username) {
            auth::validate_username(new)?;

            sqlx::query("UPDATE users SET username = ? WHERE id = ?")
                .bind(new)
                .bind(id)
                .execute(&state.db)
                .await
                .map_err(|e| match e.as_database_error() {
                    Some(db) if db.is_unique_violation() => ApiError::conflict(format!("{new:?} is taken")),
                    _ => e.into(),
                })?;

            tracing::info!("{} renamed {username:?} to {new:?}", admin.username);
            username = new.to_string();
        }

        if let Some(is_admin) = input.is_admin {
            if was_admin && !is_admin && admin_count(state).await? <= 1 {
                return Err(ApiError::bad_request("there has to be at least one admin"));
            }

            if id == admin.id && !is_admin {
                return Err(ApiError::bad_request("ask another admin to take away your admin rights"));
            }

            sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
                .bind(is_admin)
                .bind(id)
                .execute(&state.db)
                .await?;
        }

        if let Some(overrides) = input.permissions {
            sqlx::query("UPDATE users SET permissions = ? WHERE id = ?")
                .bind(overrides.to_json())
                .bind(id)
                .execute(&state.db)
                .await?;
        }

        if let Some(password) = input.password {
            auth::validate_credentials(&username, &password)?;
            let hash = auth::hash_password(password).await?;

            sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
                .bind(hash)
                .bind(id)
                .execute(&state.db)
                .await?;

            if id != admin.id {
                sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&state.db).await?;
            }
        }

        state.events.send(Event::UsersChanged);
        load(state, id).await
    }

    async fn delete_user(&self, ctx: &Context<'_>, id: i64) -> ApiResult<i64> {
        let (state, admin) = (ctx.state(), ctx.admin()?);

        if id == admin.id {
            return Err(ApiError::bad_request("you can't delete your own account"));
        }

        sqlx::query("DELETE FROM schemes WHERE owner_id = ? AND published = 0").bind(id).execute(&state.db).await?;
        sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&state.db).await?;
        state.together.forget_host(id);
        state.events.send(Event::UsersChanged);
        Ok(id)
    }

    async fn set_permission_defaults(&self, ctx: &Context<'_>, permissions: Permissions) -> ApiResult<Permissions> {
        let (state, admin) = (ctx.state(), ctx.admin()?);
        permissions::set_defaults(state, &permissions).await?;
        tracing::info!("{} changed the default permissions", admin.username);
        state.events.send(Event::UsersChanged);
        Ok(permissions)
    }

    async fn set_avatar(&self, ctx: &Context<'_>, image: Upload, user_id: Option<i64>) -> ApiResult<User> {
        let state = ctx.state();
        let id = avatar_owner(ctx, user_id)?;
        let data = image.value(ctx).map_err(|e| ApiError::bad_request(e.to_string()))?.content.to_vec();
        let mime = image_type(&data)?;

        sqlx::query(
            "INSERT INTO avatars (user_id, mime, data, updated_at) VALUES (?, ?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET mime = excluded.mime, data = excluded.data, updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(mime)
        .bind(&data)
        .bind(now())
        .execute(&state.db)
        .await?;

        state.events.send(Event::UsersChanged);
        load(state, id).await
    }

    async fn remove_avatar(&self, ctx: &Context<'_>, user_id: Option<i64>) -> ApiResult<User> {
        let state = ctx.state();
        let id = avatar_owner(ctx, user_id)?;
        sqlx::query("DELETE FROM avatars WHERE user_id = ?").bind(id).execute(&state.db).await?;
        state.events.send(Event::UsersChanged);
        load(state, id).await
    }
}

pub async fn avatar(State(state): State<Arc<AppState>>, _: User, Path(id): Path<i64>) -> ApiResult<impl IntoResponse> {
    avatar_image(&state, id).await
}

pub(super) async fn avatar_image(state: &AppState, id: i64) -> ApiResult<impl IntoResponse + use<>> {
    let (mime, data): (String, Vec<u8>) = sqlx::query_as("SELECT mime, data FROM avatars WHERE user_id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("picture"))?;

    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=31536000, immutable".into())], data))
}
