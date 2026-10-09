// SPDX-License-Identifier: AGPL-3.0-or-later

use async_graphql::SimpleObject;
use rand::RngCore;
use sqlx::SqlitePool;

use super::token_hash;
use crate::db::now;
use crate::error::{ApiError, ApiResult};

#[derive(SimpleObject, sqlx::FromRow)]
pub struct Invite {
    pub id: i64,
    pub label: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub max_uses: i64,
    pub uses: i64,
    pub revoked: bool,
}

pub async fn list(db: &SqlitePool) -> ApiResult<Vec<Invite>> {
    Ok(sqlx::query_as(
        "SELECT id, label, created_at, expires_at, max_uses, uses, revoked FROM invites ORDER BY id DESC",
    )
    .fetch_all(db)
    .await?)
}

pub async fn create(
    db: &SqlitePool,
    creator: i64,
    label: &str,
    max_uses: i64,
    hours: i64,
) -> ApiResult<(Invite, String)> {
    let label = label.trim();
    if label.is_empty() || label.len() > 64 || label.chars().any(char::is_control) {
        return Err(ApiError::bad_request("give the invite a name (up to 64 characters)"));
    }
    if !(1..=1000).contains(&max_uses) || !(1..=8760).contains(&hours) {
        return Err(ApiError::bad_request("invites need 1–1000 uses and a lifetime of 1–8760 hours"));
    }
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    let created = now();
    let invite = sqlx::query_as(
        "INSERT INTO invites (token_hash, label, created_by, created_at, expires_at, max_uses) VALUES (?, ?, ?, ?, ?, ?)
         RETURNING id, label, created_at, expires_at, max_uses, uses, revoked",
    )
    .bind(token_hash(&token))
    .bind(label)
    .bind(creator)
    .bind(created)
    .bind(created + hours * 3600)
    .bind(max_uses)
    .fetch_one(db)
    .await?;
    Ok((invite, token))
}

pub async fn available(db: &SqlitePool, token: &str) -> ApiResult<Option<(i64, i64)>> {
    if token.len() != 64 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Ok(None);
    }
    Ok(sqlx::query_as(
        "SELECT expires_at, max_uses - uses FROM invites
         WHERE token_hash = ? AND revoked = 0 AND expires_at > ? AND uses < max_uses",
    )
    .bind(token_hash(token))
    .bind(now())
    .fetch_optional(db)
    .await?)
}

pub async fn revoke(db: &SqlitePool, id: i64) -> ApiResult<bool> {
    Ok(sqlx::query("UPDATE invites SET revoked = 1 WHERE id = ?").bind(id).execute(db).await?.rows_affected() > 0)
}

pub async fn redeem(db: &SqlitePool, token: &str, username: &str, hash: &str, handle: &str) -> ApiResult<i64> {
    let mut tx = db.begin().await?;
    let claimed = sqlx::query(
        "UPDATE invites SET uses = uses + 1
         WHERE token_hash = ? AND revoked = 0 AND expires_at > ? AND uses < max_uses",
    )
    .bind(token_hash(token))
    .bind(now())
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() == 0 {
        return Err(ApiError::bad_request("this invite has expired, been revoked, or reached its use limit"));
    }
    let id = sqlx::query_scalar(
        "INSERT INTO users (handle, username, password_hash, is_admin, permissions, created_at) VALUES (?, ?, ?, 0, '{}', ?)
         ON CONFLICT(username) DO NOTHING RETURNING id",
    )
    .bind(handle)
    .bind(username.trim())
    .bind(hash)
    .bind(now())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::conflict(format!("{:?} is taken", username.trim())))?;
    tx.commit().await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> SqlitePool {
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        initialize(db).await
    }

    async fn initialize(db: SqlitePool) -> SqlitePool {
        sqlx::raw_sql(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, handle TEXT UNIQUE, username TEXT UNIQUE COLLATE NOCASE, password_hash TEXT, is_admin INTEGER, permissions TEXT, created_at INTEGER);
             INSERT INTO users VALUES (1, 'admin', 'Admin', 'hash', 1, '{}', 0);"
        ).execute(&db).await.unwrap();
        sqlx::raw_sql(include_str!("../../migrations/0013_invites.sql")).execute(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn reusable_invites_stop_at_the_use_limit() {
        let db = database().await;
        let (invite, token) = create(&db, 1, "Family", 2, 24).await.unwrap();
        assert_eq!(available(&db, &token).await.unwrap().unwrap().1, 2);
        redeem(&db, &token, "One", "hash", "one").await.unwrap();
        redeem(&db, &token, "Two", "hash", "two").await.unwrap();
        assert!(redeem(&db, &token, "Three", "hash", "three").await.is_err());
        assert!(available(&db, &token).await.unwrap().is_none());
        let row: (i64, String) = sqlx::query_as("SELECT is_admin, permissions FROM users WHERE username = 'One'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(row, (0, "{}".into()));
        assert_eq!(list(&db).await.unwrap()[0].id, invite.id);
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM invites").fetch_one(&db).await.unwrap();
        assert_ne!(stored, token);
    }

    #[tokio::test]
    async fn expired_revoked_and_unknown_links_create_no_accounts() {
        let db = database().await;
        let (expired, a) = create(&db, 1, "Expired", 5, 1).await.unwrap();
        let (revoked, b) = create(&db, 1, "Revoked", 5, 1).await.unwrap();
        sqlx::query("UPDATE invites SET expires_at = ? WHERE id = ?")
            .bind(now())
            .bind(expired.id)
            .execute(&db)
            .await
            .unwrap();
        assert!(revoke(&db, revoked.id).await.unwrap());
        for token in [a, b, "0".repeat(64), "bad".into()] {
            assert!(available(&db, &token).await.unwrap().is_none());
            assert!(redeem(&db, &token, "Guest", "hash", "guest").await.is_err());
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&db).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn taken_usernames_do_not_consume_invites() {
        let db = database().await;
        let (_, token) = create(&db, 1, "Friends", 1, 1).await.unwrap();
        assert!(redeem(&db, &token, " admin ", "hash", "other").await.is_err());
        assert_eq!(available(&db, &token).await.unwrap().unwrap().1, 1);
        redeem(&db, &token, "Guest", "hash", "guest").await.unwrap();
    }

    #[tokio::test]
    async fn simultaneous_redemptions_cannot_exceed_the_limit() {
        let dir = std::env::temp_dir().join(format!(
            "tinystream-invite-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.join("test.db"))
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(10));
        let db =
            initialize(sqlx::sqlite::SqlitePoolOptions::new().max_connections(8).connect_with(options).await.unwrap())
                .await;
        let (_, token) = create(&db, 1, "Last spots", 5, 1).await.unwrap();
        let mut jobs = tokio::task::JoinSet::new();
        for i in 0..16 {
            let db = db.clone();
            let token = token.clone();
            jobs.spawn(async move { redeem(&db, &token, &format!("User {i}"), "hash", &format!("handle-{i}")).await });
        }
        let mut successes = 0;
        while let Some(result) = jobs.join_next().await {
            successes += usize::from(result.unwrap().is_ok());
        }
        assert_eq!(successes, 5);
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_admin = 0").fetch_one(&db).await.unwrap();
        assert_eq!(count, 5);
        assert_eq!(list(&db).await.unwrap()[0].uses, 5);
        db.close().await;
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn limits_must_be_positive_and_bounded() {
        let db = database().await;
        for (uses, hours) in [(0, 1), (-1, 1), (1001, 1), (1, 0), (1, 8761)] {
            assert!(create(&db, 1, "Invalid", uses, hours).await.is_err());
        }
        assert!(create(&db, 1, " ", 1, 1).await.is_err());
    }
}
