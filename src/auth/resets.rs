// SPDX-License-Identifier: AGPL-3.0-or-later

use rand::RngCore;
use sqlx::SqlitePool;

use super::token_hash;
use crate::db::now;
use crate::error::{ApiError, ApiResult};

pub const LIFETIME: i64 = 3600;
const COOLDOWN: i64 = 120;

pub struct Recipient {
    pub id: i64,
    pub username: String,
    pub email: String,
}

/// Everyone `who` (a username or an email address) could mean, if they gave an address to send to.
pub async fn recipients(db: &SqlitePool, who: &str) -> ApiResult<Vec<Recipient>> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, username, email FROM users
         WHERE email IS NOT NULL AND email <> '' AND (username = ?1 OR email = ?1 COLLATE NOCASE)",
    )
    .bind(who.trim())
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(|(id, username, email)| Recipient { id, username, email }).collect())
}

/// A new link for `user_id`, unless one was made a moment ago.
pub async fn create(db: &SqlitePool, user_id: i64) -> ApiResult<Option<String>> {
    let t = now();
    sqlx::query("DELETE FROM password_resets WHERE expires_at <= ?").bind(t).execute(db).await?;

    let recent: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM password_resets WHERE user_id = ? AND created_at > ? LIMIT 1")
            .bind(user_id)
            .bind(t - COOLDOWN)
            .fetch_optional(db)
            .await?;

    if recent.is_some() {
        return Ok(None);
    }

    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);

    sqlx::query("INSERT INTO password_resets (token_hash, user_id, created_at, expires_at) VALUES (?, ?, ?, ?)")
        .bind(token_hash(&token))
        .bind(user_id)
        .bind(t)
        .bind(t + LIFETIME)
        .execute(db)
        .await?;

    Ok(Some(token))
}

fn well_formed(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Whose password the link resets, while it still can.
pub async fn available(db: &SqlitePool, token: &str) -> ApiResult<Option<String>> {
    if !well_formed(token) {
        return Ok(None);
    }

    Ok(sqlx::query_scalar(
        "SELECT u.username FROM password_resets r JOIN users u ON u.id = r.user_id
         WHERE r.token_hash = ? AND r.expires_at > ?",
    )
    .bind(token_hash(token))
    .bind(now())
    .fetch_optional(db)
    .await?)
}

/// Sets the new password and signs the account out everywhere; every link it had stops working.
pub async fn redeem(db: &SqlitePool, token: &str, hash: &str) -> ApiResult<i64> {
    let gone = || ApiError::bad_request("this link doesn't work anymore; ask for a new one");

    if !well_formed(token) {
        return Err(gone());
    }

    let mut tx = db.begin().await?;

    let user_id: i64 =
        sqlx::query_scalar("DELETE FROM password_resets WHERE token_hash = ? AND expires_at > ? RETURNING user_id")
            .bind(token_hash(token))
            .bind(now())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(gone)?;

    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?").bind(hash).bind(user_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM password_resets WHERE user_id = ?").bind(user_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(user_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> SqlitePool {
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();

        sqlx::raw_sql(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT UNIQUE COLLATE NOCASE, password_hash TEXT);
             CREATE TABLE sessions (token_hash TEXT PRIMARY KEY, user_id INTEGER);
             INSERT INTO users VALUES (1, 'Admin', 'old'), (2, 'Guest', 'old'), (3, 'Nobody', 'old');
             INSERT INTO sessions VALUES ('a', 1), ('b', 2);",
        )
        .execute(&db)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!("../../migrations/0017_password_resets.sql")).execute(&db).await.unwrap();

        sqlx::raw_sql("UPDATE users SET email = 'home@example.com' WHERE id IN (1, 2)").execute(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn finds_accounts_by_name_or_address() {
        let db = database().await;
        let ids = |r: Vec<Recipient>| r.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids(recipients(&db, " admin ").await.unwrap()), [1]);
        assert_eq!(ids(recipients(&db, "HOME@example.com").await.unwrap()), [1, 2]);
        assert!(recipients(&db, "Nobody").await.unwrap().is_empty());
        assert!(recipients(&db, "").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn links_work_once_and_sign_out_everywhere() {
        let db = database().await;
        let token = create(&db, 1).await.unwrap().unwrap();
        assert_eq!(available(&db, &token).await.unwrap().as_deref(), Some("Admin"));
        assert_eq!(redeem(&db, &token, "new").await.unwrap(), 1);
        assert!(available(&db, &token).await.unwrap().is_none());
        assert!(redeem(&db, &token, "newer").await.is_err());

        let hash: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE id = 1").fetch_one(&db).await.unwrap();
        assert_eq!(hash, "new");
        let sessions: Vec<i64> = sqlx::query_scalar("SELECT user_id FROM sessions").fetch_all(&db).await.unwrap();
        assert_eq!(sessions, [2]);
        let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM password_resets").fetch_one(&db).await.unwrap();
        assert_eq!(stored, 0);
    }

    #[tokio::test]
    async fn asking_again_right_away_sends_nothing() {
        let db = database().await;
        assert!(create(&db, 1).await.unwrap().is_some());
        assert!(create(&db, 1).await.unwrap().is_none());
        assert!(create(&db, 2).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn expired_and_unknown_links_change_nothing() {
        let db = database().await;
        let token = create(&db, 1).await.unwrap().unwrap();
        sqlx::query("UPDATE password_resets SET expires_at = ?").bind(now()).execute(&db).await.unwrap();

        for token in [token, "0".repeat(64), "bad".into()] {
            assert!(available(&db, &token).await.unwrap().is_none());
            assert!(redeem(&db, &token, "new").await.is_err());
        }

        let hash: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE id = 1").fetch_one(&db).await.unwrap();
        assert_eq!(hash, "old");
    }
}
