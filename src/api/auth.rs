// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use async_graphql::{Context, Json, Object, SimpleObject};
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use serde_json::Value;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential, SecurityKey, Uuid};

use super::schema::Ctx;
use crate::auth::{self, User, passkeys};
use crate::config::SignInStyle;
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::media::hw::Capabilities;
use crate::state::AppState;

async fn user_count(state: &AppState) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&state.db).await?)
}

fn profiles_on(state: &AppState) -> bool {
    state.config.current().sign_in.style == SignInStyle::Profiles
}

async fn account(
    state: &AppState,
    username: Option<String>,
    profile: Option<String>,
) -> ApiResult<Option<(i64, String)>> {
    let (column, value) = match (username, profile) {
        (Some(u), None) => ("username", u.trim().to_string()),
        (None, Some(p)) if profiles_on(state) => ("handle", p),
        (None, Some(_)) => return Ok(None),
        _ => return Err(ApiError::bad_request("sign in with either a username or a profile")),
    };
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT id, password_hash FROM users WHERE {column} = ?")))
        .bind(value)
        .fetch_optional(&state.db)
        .await?)
}

async fn keys_of(state: &AppState, user_id: i64) -> ApiResult<Vec<(i64, SecurityKey)>> {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, credential FROM passkeys WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;
    Ok(rows.into_iter().filter_map(|(id, c)| Some((id, serde_json::from_str(&c).ok()?))).collect())
}

pub(super) async fn sign_in(ctx: &Context<'_>, user_id: i64) -> ApiResult<SignedIn> {
    let state = ctx.state();
    let token = auth::create_session(state, user_id).await?;
    ctx.append_http_header(header::SET_COOKIE, auth::session_cookie(&token));
    let user = auth::load_user(state, user_id).await?.ok_or_else(ApiError::unauthorized)?;
    Ok(SignedIn { user, token })
}

#[derive(SimpleObject)]
pub struct SignedIn {
    user: User,

    token: String,
}

#[derive(SimpleObject)]
pub struct PasskeyChallenge {
    challenge: String,

    options: Json<Value>,
}

#[derive(SimpleObject, sqlx::FromRow)]
pub struct SignInProfile {
    key: String,
    avatar: Option<String>,

    passkey: bool,
}

#[derive(SimpleObject, sqlx::FromRow)]
pub struct Passkey {
    id: i64,
    name: String,
    created_at: i64,
    last_used: Option<i64>,
}

pub(super) async fn passkeys(state: &AppState, user_id: i64) -> ApiResult<Vec<Passkey>> {
    Ok(sqlx::query_as("SELECT id, name, created_at, last_used FROM passkeys WHERE user_id = ? ORDER BY created_at")
        .bind(user_id)
        .fetch_all(&state.db)
        .await?)
}

pub struct Server;

#[Object]
impl Server {
    async fn setup_required(&self, ctx: &Context<'_>) -> ApiResult<bool> {
        Ok(user_count(ctx.state()).await? == 0)
    }

    async fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    async fn sign_in_style(&self, ctx: &Context<'_>) -> SignInStyle {
        ctx.state().config.current().sign_in.style
    }

    async fn clips(&self, ctx: &Context<'_>) -> bool {
        ctx.state().config.current().clips.enabled
    }

    async fn downloads(&self) -> bool {
        cfg!(feature = "torrent")
    }

    async fn sources(&self, ctx: &Context<'_>) -> usize {
        ctx.state().config.current().sources.iter().filter(|s| s.enabled).count()
    }

    async fn transcoding(&self, ctx: &Context<'_>) -> Capabilities {
        ctx.state().media.hw.capabilities()
    }
}

#[derive(Default)]
pub struct AuthQuery;

#[Object]
impl AuthQuery {
    async fn viewer(&self, ctx: &Context<'_>) -> Option<User> {
        ctx.maybe_user().cloned()
    }

    async fn server(&self) -> Server {
        Server
    }

    async fn sign_in_profiles(&self, ctx: &Context<'_>) -> ApiResult<Vec<SignInProfile>> {
        let state = ctx.state();
        if !profiles_on(state) {
            return Ok(Vec::new());
        }
        Ok(sqlx::query_as(
            "SELECT u.handle AS key,
                    CASE WHEN a.updated_at IS NULL THEN NULL
                         ELSE '/api/sign-in/' || u.handle || '/avatar?v=' || a.updated_at END AS avatar,
                    EXISTS (SELECT 1 FROM passkeys p WHERE p.user_id = u.id) AS passkey
             FROM users u LEFT JOIN avatars a ON a.user_id = u.id ORDER BY u.handle",
        )
        .fetch_all(&state.db)
        .await?)
    }
}

pub async fn profile_avatar(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let id: Option<i64> = match profiles_on(&state) {
        true => sqlx::query_scalar("SELECT id FROM users WHERE handle = ?").bind(key).fetch_optional(&state.db).await?,
        false => None,
    };
    super::users::avatar_image(&state, id.ok_or_else(|| ApiError::not_found("picture"))?).await
}

#[derive(Default)]
pub struct AuthMutation;

#[Object]
impl AuthMutation {
    async fn setup(&self, ctx: &Context<'_>, username: String, password: String) -> ApiResult<SignedIn> {
        let state = ctx.state();
        auth::validate_credentials(&username, &password)?;
        let hash = auth::hash_password(password).await?;
        let mut tx = state.db.begin().await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&mut *tx).await?;
        if count > 0 {
            return Err(ApiError::conflict("this server is already set up; sign in instead"));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (handle, username, password_hash, is_admin, created_at) VALUES (?, ?, ?, 1, ?) RETURNING id",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(username.trim())
        .bind(hash)
        .bind(now())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!("created the admin account {:?}", username.trim());
        sign_in(ctx, id).await
    }

    async fn sign_in(
        &self,
        ctx: &Context<'_>,
        username: Option<String>,
        profile: Option<String>,
        password: String,
    ) -> ApiResult<SignedIn> {
        let message = if profile.is_some() { "wrong password" } else { "wrong username or password" };
        let wrong = || ApiError::new(axum::http::StatusCode::UNAUTHORIZED, message);
        let Some((id, hash)) = account(ctx.state(), username, profile).await? else {
            return Err(wrong());
        };
        if !auth::verify_password(password, hash).await {
            return Err(wrong());
        }
        sign_in(ctx, id).await
    }

    async fn sign_out(&self, ctx: &Context<'_>) -> ApiResult<bool> {
        if let Some(t) = &ctx.session().token {
            auth::delete_session(ctx.state(), t).await?;
        }
        ctx.append_http_header(header::SET_COOKIE, auth::clear_cookie());
        Ok(true)
    }

    async fn start_passkey_sign_in(
        &self,
        ctx: &Context<'_>,
        username: Option<String>,
        profile: Option<String>,
    ) -> ApiResult<PasskeyChallenge> {
        let state = ctx.state();
        let wa = passkeys::webauthn_for(&ctx.session().headers)?;
        let user_id = account(state, username, profile).await?.map(|(id, _)| id);
        let keys = match user_id {
            Some(id) => keys_of(state, id).await?,
            None => Vec::new(),
        };
        let (Some(user_id), false) = (user_id, keys.is_empty()) else {
            return Err(ApiError::bad_request("no passkeys on this account; sign in with your password"));
        };
        let keys: Vec<SecurityKey> = keys.into_iter().map(|(_, k)| k).collect();
        let (challenge, options) = passkeys::start_login(&state.passkeys, &wa, user_id, &keys)?;
        Ok(PasskeyChallenge { challenge, options: Json(serde_json::to_value(options).unwrap_or_default()) })
    }

    async fn finish_passkey_sign_in(
        &self,
        ctx: &Context<'_>,
        challenge: String,
        credential: Json<PublicKeyCredential>,
    ) -> ApiResult<SignedIn> {
        let state = ctx.state();
        let wa = passkeys::webauthn_for(&ctx.session().headers)?;
        let (user_id, result) = passkeys::finish_login(&state.passkeys, &wa, &challenge, &credential)?;
        for (id, mut key) in keys_of(state, user_id).await? {
            if key.cred_id() == result.cred_id() {
                key.update_credential(&result);
                sqlx::query("UPDATE passkeys SET credential = ?, last_used = ? WHERE id = ?")
                    .bind(serde_json::to_string(&key).unwrap_or_default())
                    .bind(now())
                    .bind(id)
                    .execute(&state.db)
                    .await?;
            }
        }
        sign_in(ctx, user_id).await
    }

    async fn change_password(&self, ctx: &Context<'_>, current: String, new: String) -> ApiResult<bool> {
        let (state, user) = (ctx.state(), ctx.user()?);
        auth::validate_credentials(&user.username, &new)?;
        let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
            .bind(user.id)
            .fetch_one(&state.db)
            .await?;
        if !auth::verify_password(current, hash).await {
            return Err(ApiError::bad_request("your current password isn't right"));
        }
        let hash = auth::hash_password(new).await?;
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(hash)
            .bind(user.id)
            .execute(&state.db)
            .await?;
        Ok(true)
    }

    async fn start_passkey_registration(&self, ctx: &Context<'_>, name: Option<String>) -> ApiResult<PasskeyChallenge> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let wa = passkeys::webauthn_for(&ctx.session().headers)?;
        let existing = keys_of(state, user.id).await?.into_iter().map(|(_, k)| k).collect();
        let handle = Uuid::parse_str(&user.handle).map_err(|e| ApiError::from(anyhow::anyhow!(e)))?;
        let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Passkey".into());
        let (challenge, options) =
            passkeys::start_registration(&state.passkeys, &wa, user.id, handle, &user.username, name, existing)?;
        Ok(PasskeyChallenge { challenge, options: Json(serde_json::to_value(options).unwrap_or_default()) })
    }

    async fn finish_passkey_registration(
        &self,
        ctx: &Context<'_>,
        challenge: String,
        credential: Json<RegisterPublicKeyCredential>,
    ) -> ApiResult<Vec<Passkey>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let wa = passkeys::webauthn_for(&ctx.session().headers)?;
        let (name, key) = passkeys::finish_registration(&state.passkeys, &wa, user.id, &challenge, &credential)?;
        sqlx::query("INSERT INTO passkeys (user_id, name, credential, created_at) VALUES (?, ?, ?, ?)")
            .bind(user.id)
            .bind(&name)
            .bind(serde_json::to_string(&key).map_err(|e| ApiError::from(anyhow::anyhow!(e)))?)
            .bind(now())
            .execute(&state.db)
            .await?;
        tracing::info!("{} added a passkey ({name})", user.username);
        passkeys(state, user.id).await
    }

    async fn delete_passkey(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Vec<Passkey>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        sqlx::query("DELETE FROM passkeys WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user.id)
            .execute(&state.db)
            .await?;
        passkeys(state, user.id).await
    }
}
