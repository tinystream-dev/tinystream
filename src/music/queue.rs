// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, async_graphql::Enum)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    fn parse(s: &str) -> Self {
        match s {
            "all" => Repeat::All,
            "one" => Repeat::One,
            _ => Repeat::Off,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::All => "all",
            Repeat::One => "one",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Queue {
    pub tracks: Vec<i64>,
    pub current: usize,
    pub position: f64,
    pub shuffled: bool,
    pub repeat: Repeat,
    pub changed_by: Option<String>,
    pub updated_at: i64,
}

pub async fn load(db: &SqlitePool, user_id: i64) -> sqlx::Result<Queue> {
    let row: Option<(String, i64, f64, bool, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT tracks, current, position, shuffled, repeat, changed_by, updated_at FROM play_queues WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(match row {
        Some((tracks, current, position, shuffled, repeat, changed_by, updated_at)) => Queue {
            tracks: serde_json::from_str(&tracks).unwrap_or_default(),
            current: current.max(0) as usize,
            position,
            shuffled,
            repeat: Repeat::parse(&repeat),
            changed_by,
            updated_at,
        },
        None => Queue::default(),
    })
}

pub async fn save(state: &AppState, user_id: i64, mut q: Queue) -> sqlx::Result<Queue> {
    q.current = q.current.min(q.tracks.len().saturating_sub(1));
    q.position = if q.position.is_finite() { q.position.max(0.0) } else { 0.0 };
    q.updated_at = now();

    sqlx::query(
        "INSERT INTO play_queues (user_id, tracks, current, position, shuffled, repeat, changed_by, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET tracks = excluded.tracks, current = excluded.current,
            position = excluded.position, shuffled = excluded.shuffled, repeat = excluded.repeat,
            changed_by = excluded.changed_by, updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(serde_json::to_string(&q.tracks).unwrap_or_else(|_| "[]".into()))
    .bind(q.current as i64)
    .bind(q.position)
    .bind(q.shuffled)
    .bind(q.repeat.as_str())
    .bind(&q.changed_by)
    .bind(q.updated_at)
    .execute(&state.db)
    .await?;

    state.events.send(Event::QueueChanged { user_id, by: q.changed_by.clone() });
    Ok(q)
}

#[derive(Debug, Clone)]
pub struct Playing {
    pub track_id: i64,
    pub client: String,
    pub since: i64,
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
}

/// What everyone's playing right now, across apps.
#[derive(Default)]
pub struct NowPlaying(Mutex<HashMap<i64, Playing>>);

impl NowPlaying {
    pub fn set(&self, user_id: i64, playing: Option<Playing>) {
        let mut map = self.0.lock().unwrap();

        match playing {
            Some(p) => map.insert(user_id, p),
            None => map.remove(&user_id),
        };
    }

    /// Who's playing what, leaving out what's surely over by now.
    pub fn all(&self) -> Vec<(i64, Playing)> {
        let mut map = self.0.lock().unwrap();
        let t = now();
        map.retain(|_, p| t - p.since < (p.duration - p.position).max(0.0) as i64 + 600);
        map.iter().map(|(u, p)| (*u, p.clone())).collect()
    }
}
