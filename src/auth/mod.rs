// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod invites;
pub mod passkeys;
pub mod permissions;

use std::sync::Arc;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, header};
use permissions::{Overrides, Permissions};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const COOKIE: &str = "tinystream_session";
const SESSION_DAYS: i64 = 365;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    #[serde(skip)]
    pub handle: String,
    pub username: String,
    pub is_admin: bool,

    pub avatar: Option<i64>,

    pub permissions: Permissions,
}

pub async fn load_user(state: &AppState, id: i64) -> ApiResult<Option<User>> {
    let row: Option<(i64, String, String, bool, String, Option<i64>)> = sqlx::query_as(
        "SELECT u.id, u.handle, u.username, u.is_admin, u.permissions, a.updated_at
         FROM users u LEFT JOIN avatars a ON a.user_id = u.id WHERE u.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some((id, handle, username, is_admin, overrides, avatar)) = row else { return Ok(None) };
    let permissions =
        permissions::effective(is_admin, &Overrides::parse(&overrides), &permissions::defaults(state).await?);
    Ok(Some(User { id, handle, username, is_admin, avatar, permissions }))
}

pub async fn hash_password(password: String) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        let salt = SaltString::encode_b64(&bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| anyhow::anyhow!("can't hash password: {e}"))
    })
    .await?
}

pub async fn verify_password(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash)
            .map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

pub fn validate_credentials(username: &str, password: &str) -> ApiResult<()> {
    validate_username(username)?;
    if password.chars().count() < 4 {
        return Err(ApiError::bad_request("passwords need at least 4 characters"));
    }
    Ok(())
}

pub fn validate_username(username: &str) -> ApiResult<()> {
    let u = username.trim();
    if u.is_empty() || u.len() > 64 {
        return Err(ApiError::bad_request("pick a username (up to 64 characters)"));
    }
    if u.chars().any(|c| c.is_control()) {
        return Err(ApiError::bad_request("usernames can't contain control characters"));
    }
    Ok(())
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn create_session(state: &AppState, user_id: i64) -> ApiResult<String> {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    sqlx::query("INSERT INTO sessions (token_hash, user_id, created_at, last_seen) VALUES (?, ?, ?, ?)")
        .bind(token_hash(&token))
        .bind(user_id)
        .bind(now())
        .bind(now())
        .execute(&state.db)
        .await?;
    Ok(token)
}

pub fn session_cookie(token: &str) -> String {
    format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}", SESSION_DAYS * 24 * 3600)
}

pub fn clear_cookie() -> String {
    format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

pub fn token_from(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok())
        && let Some(t) = auth.strip_prefix("Bearer ")
    {
        return Some(t.trim().to_string());
    }
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| kv.trim().strip_prefix(&format!("{COOKIE}=")).map(str::to_string))
}

pub async fn delete_session(state: &AppState, token: &str) -> ApiResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(token_hash(token)).execute(&state.db).await?;
    Ok(())
}

pub async fn user_from_token(state: &AppState, token: &str) -> ApiResult<Option<User>> {
    let hash = token_hash(token);
    let row: Option<(i64, i64)> =
        sqlx::query_as("SELECT user_id, last_seen FROM sessions WHERE token_hash = ? AND last_seen > ?")
            .bind(&hash)
            .bind(now() - SESSION_DAYS * 24 * 3600)
            .fetch_optional(&state.db)
            .await?;
    let Some((id, last_seen)) = row else { return Ok(None) };
    if now() - last_seen > 3600 {
        sqlx::query("UPDATE sessions SET last_seen = ? WHERE token_hash = ?")
            .bind(now())
            .bind(&hash)
            .execute(&state.db)
            .await?;
    }
    load_user(state, id).await
}

impl FromRequestParts<Arc<AppState>> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let token = token_from(&parts.headers).ok_or_else(ApiError::unauthorized)?;
        user_from_token(state, &token).await?.ok_or_else(ApiError::unauthorized)
    }
}

impl User {
    pub async fn libraries(&self, state: &AppState) -> ApiResult<Vec<String>> {
        let config = state.config.current();
        Ok(config.libraries.iter().map(|l| l.name.clone()).filter(|l| self.permissions.can_see(l)).collect())
    }

    pub async fn can_access(&self, state: &AppState, library: &str) -> ApiResult<()> {
        if self.libraries(state).await?.iter().any(|l| l == library) {
            Ok(())
        } else {
            Err(ApiError::not_found("title"))
        }
    }
}
