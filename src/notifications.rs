// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Serialize;

use crate::auth::permissions::{self, Overrides, Permissions};
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

const KEEP: i64 = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: i64,
    pub kind: String,
    pub priority: bool,
    pub title: String,
    pub body: Option<String>,
    pub image: Option<String>,
    pub link: Option<String>,
    pub actor: Option<Actor>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub read_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub id: i64,
    pub username: String,
    pub avatar: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct New {
    pub kind: &'static str,
    pub priority: bool,
    pub title: String,
    pub body: Option<String>,
    pub image: Option<String>,
    pub link: Option<String>,
    pub actor_id: Option<i64>,
    pub expires_at: Option<i64>,
    pub replace: bool,
}

type Row = (
    i64,
    String,
    bool,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<i64>,
    i64,
    Option<i64>,
    Option<i64>,
);

const SELECT: &str =
    "SELECT n.id, n.kind, n.priority, n.title, n.body, n.image, n.link, n.actor_id, u.username, a.updated_at,
        n.created_at, n.expires_at, n.read_at
    FROM notifications n LEFT JOIN users u ON u.id = n.actor_id LEFT JOIN avatars a ON a.user_id = n.actor_id";

fn from_row(
    (
        id,
        kind,
        priority,
        title,
        body,
        image,
        link,
        actor_id,
        actor_name,
        actor_avatar,
        created_at,
        expires_at,
        read_at,
    ): Row,
) -> Notification {
    let actor = actor_id.zip(actor_name).map(|(id, username)| Actor { id, username, avatar: actor_avatar });
    Notification { id, kind, priority, title, body, image, link, actor, created_at, expires_at, read_at }
}

pub async fn list(state: &AppState, user_id: i64, limit: i64) -> sqlx::Result<(Vec<Notification>, i64)> {
    let rows: Vec<Row> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE n.user_id = ? ORDER BY n.id DESC LIMIT ?")))
            .bind(user_id)
            .bind(limit)
            .fetch_all(&state.db)
            .await?;

    let unread: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;

    Ok((rows.into_iter().map(from_row).collect(), unread))
}

pub async fn send(state: &AppState, users: &[i64], n: New) {
    for &user_id in users {
        if let Err(e) = send_one(state, user_id, &n).await {
            tracing::warn!("can't save a notification: {e}");
        }
    }
}

async fn send_one(state: &AppState, user_id: i64, n: &New) -> sqlx::Result<()> {
    let t = now();

    if n.replace && n.link.is_some() {
        sqlx::query("DELETE FROM notifications WHERE user_id = ? AND kind = ? AND link = ? AND read_at IS NULL")
            .bind(user_id)
            .bind(n.kind)
            .bind(&n.link)
            .execute(&state.db)
            .await?;
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO notifications (user_id, kind, priority, title, body, image, link, actor_id, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(user_id)
    .bind(n.kind)
    .bind(n.priority)
    .bind(&n.title)
    .bind(&n.body)
    .bind(&n.image)
    .bind(&n.link)
    .bind(n.actor_id)
    .bind(t)
    .bind(n.expires_at)
    .fetch_one(&state.db)
    .await?;

    sqlx::query("DELETE FROM notifications WHERE user_id = ? AND id NOT IN (SELECT id FROM notifications WHERE user_id = ? ORDER BY id DESC LIMIT ?)")
        .bind(user_id)
        .bind(user_id)
        .bind(KEEP)
        .execute(&state.db)
        .await?;

    let row: Row =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE n.id = ?"))).bind(id).fetch_one(&state.db).await?;

    state.events.send(Event::Notified { user_id, notification: from_row(row) });
    Ok(())
}

pub async fn withdraw(state: &AppState, link: &str) {
    let users: Vec<i64> =
        sqlx::query_scalar("DELETE FROM notifications WHERE link = ? AND read_at IS NULL RETURNING user_id")
            .bind(link)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    let mut users = users;
    users.dedup();

    for user_id in users {
        state.events.send(Event::NotificationsChanged { user_id });
    }
}

#[cfg_attr(not(feature = "torrent"), allow(dead_code))]
pub async fn everyone_who(state: &AppState, keep: impl Fn(&Permissions) -> bool) -> Vec<i64> {
    let Ok(defaults) = permissions::defaults(state).await else {
        return Vec::new();
    };

    let rows: Vec<(i64, bool, String)> =
        sqlx::query_as("SELECT id, is_admin, permissions FROM users").fetch_all(&state.db).await.unwrap_or_default();

    rows.into_iter()
        .filter(|(_, admin, overrides)| keep(&permissions::effective(*admin, &Overrides::parse(overrides), &defaults)))
        .map(|(id, ..)| id)
        .collect()
}

#[cfg_attr(not(feature = "torrent"), allow(dead_code))]
pub async fn who_can_see(state: &AppState, library: &str) -> Vec<i64> {
    everyone_who(state, |p| p.can_see(library)).await
}

#[cfg_attr(not(feature = "torrent"), allow(dead_code))]
pub fn episodes_label(episodes: &[(u32, u32)]) -> String {
    let mut eps = episodes.to_vec();
    eps.sort();
    eps.dedup();

    match eps.as_slice() {
        [] => String::new(),
        [(s, e)] => format!("S{s:02}E{e:02}"),
        [(s, first), .., (s2, last)] if s == s2 && (last - first) as usize == eps.len() - 1 => {
            format!("S{s:02}E{first:02}–E{last:02}")
        },
        _ => format!("{} episodes", eps.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::episodes_label;

    #[test]
    fn labels_episodes() {
        assert_eq!(episodes_label(&[(1, 3)]), "S01E03");
        assert_eq!(episodes_label(&[(1, 4), (1, 3), (1, 5)]), "S01E03–E05");
        assert_eq!(episodes_label(&[(1, 3), (1, 7)]), "2 episodes");
        assert_eq!(episodes_label(&[(1, 12), (2, 1)]), "2 episodes");
    }
}
