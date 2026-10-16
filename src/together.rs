// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::db::now;

pub(crate) const STALL_LIMIT: Duration = Duration::from_secs(12);
pub(crate) const START_LEAD_MS: f64 = 150.0;
pub(crate) const EXPIRY_SECS: i64 = 7 * 24 * 3600;

pub const ENDED: &str = r#"{"type":"ended"}"#;

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Control {
    #[default]
    Everyone,
    Host,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub control: Control,
    pub wait_for_all: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { control: Control::Everyone, wait_for_all: true }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tracks {
    pub media_id: i64,
    pub audio: Option<i64>,
    pub audio_language: Option<String>,
    pub subtitle: Option<String>,
    pub subtitle_language: Option<String>,
    pub subtitle_title: Option<String>,
    pub subtitle_forced: bool,
    pub subtitle_codec: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    pub paused: bool,
    pub running: bool,
    pub position: f64,
    pub at: f64,
    pub rate: f64,
}

impl Clock {
    pub fn position_at(&self, t: f64) -> f64 {
        if self.running { self.position + ((t - self.at).max(0.0) / 1000.0) * self.rate } else { self.position }
    }

    pub(crate) fn rebase(&mut self, t: f64) {
        self.position = self.position_at(t);
        self.at = t;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Joining,
    Loading,
    Ready,
    Buffering,
    Blocked,
}

struct Member {
    id: u64,
    name: String,
    user_id: Option<i64>,
    avatar: Option<i64>,
    status: Status,
    since: Instant,
}

impl Member {
    fn holds_up(&self) -> bool {
        matches!(self.status, Status::Loading | Status::Buffering) && self.since.elapsed() < STALL_LIMIT
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Action {
    seq: u64,
    member: u64,
    by: String,
    kind: &'static str,
    value: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    Ping { id: u64, c: f64 },
    Play { position: f64 },
    Pause { position: f64 },
    Seek { position: f64 },
    Rate { rate: f64 },
    Status { status: Status },
    Tracks { tracks: Tracks },
    Media { id: i64, from: i64 },
    Name { name: String },
    Settings { control: Option<Control>, wait_for_all: Option<bool>, public: Option<bool> },
    Invite { user_id: i64 },
    End,
}

#[derive(Debug, Clone)]
pub struct Info {
    pub host_id: i64,
    pub media_id: i64,
    pub item_id: i64,
    pub library: String,
    pub public: bool,
    pub invited: Vec<i64>,
}

struct State {
    info: Info,
    host_name: String,
    settings: Settings,
    tracks: Tracks,
    clock: Clock,
    members: Vec<Member>,
    action: Option<Action>,
    seq: u64,
}

impl State {
    fn settle(&mut self, t: f64) {
        let waiting = self.settings.wait_for_all && self.members.iter().any(Member::holds_up);
        let running = !self.clock.paused && !waiting;

        if running != self.clock.running {
            self.clock.rebase(t);
            self.clock.running = running;

            if running {
                self.clock.at = t + START_LEAD_MS;
            }
        }
    }

    fn host_here(&self) -> bool {
        self.members.iter().any(|m| m.user_id == Some(self.info.host_id))
    }

    fn can_control(&self, member: u64) -> bool {
        self.settings.control == Control::Everyone
            || !self.host_here()
            || self.members.iter().any(|m| m.id == member && m.user_id == Some(self.info.host_id))
    }

    fn name_of(&self, member: u64) -> String {
        self.members.iter().find(|m| m.id == member).map(|m| m.name.clone()).unwrap_or_default()
    }

    fn act(&mut self, member: u64, kind: &'static str, value: Option<f64>) {
        self.seq += 1;
        self.action = Some(Action { seq: self.seq, member, by: self.name_of(member), kind, value });
    }

    fn snapshot(&self, code: &str) -> String {
        json!({
            "type": "state",
            "code": code,
            "mediaId": self.info.media_id,
            "itemId": self.info.item_id,
            "host": { "id": self.info.host_id, "name": self.host_name },
            "public": self.info.public,
            "invited": self.info.invited,
            "settings": self.settings,
            "tracks": self.tracks,
            "clock": self.clock,
            "members": self.members.iter().map(|m| json!({
                "id": m.id,
                "name": m.name,
                "userId": m.user_id,
                "avatar": m.avatar,
                "status": m.status,
            })).collect::<Vec<_>>(),
            "action": self.action,
        })
        .to_string()
    }
}

pub type Refusal = &'static str;

pub struct Room {
    pub code: String,
    st: Mutex<State>,
    tx: broadcast::Sender<Arc<str>>,
    next_member: AtomicU64,
}

impl Room {
    fn new(code: String, info: Info, host_name: String, settings: Settings, tracks: Tracks, clock: Clock) -> Self {
        Self {
            code,
            st: Mutex::new(State {
                info,
                host_name,
                settings,
                tracks,
                clock,
                members: Vec::new(),
                action: None,
                seq: 0,
            }),
            tx: broadcast::channel(64).0,
            next_member: AtomicU64::new(1),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn info(&self) -> Info {
        self.lock().info.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<str>> {
        self.tx.subscribe()
    }

    pub fn current(&self) -> Arc<str> {
        self.lock().snapshot(&self.code).into()
    }

    fn change<R>(&self, f: impl FnOnce(&mut State, f64) -> R) -> R {
        let mut st = self.lock();
        let t = now_ms();
        let r = f(&mut st, t);
        st.settle(t);
        let _ = self.tx.send(st.snapshot(&self.code).into());
        r
    }

    pub fn close(&self) {
        let _ = self.tx.send(Arc::from(ENDED));
    }

    pub fn join(&self, name: String, user_id: Option<i64>, avatar: Option<i64>) -> u64 {
        let id = self.next_member.fetch_add(1, Ordering::Relaxed);

        self.change(|st, _| {
            st.members.push(Member { id, name, user_id, avatar, status: Status::Joining, since: Instant::now() });
            st.act(id, "join", None);
        });

        id
    }

    pub fn leave(&self, member: u64) -> bool {
        self.change(|st, _| {
            st.act(member, "leave", None);
            st.members.retain(|m| m.id != member);
            st.members.is_empty()
        })
    }

    pub fn is_empty(&self) -> bool {
        self.lock().members.is_empty()
    }

    pub fn has_user(&self, user_id: i64) -> bool {
        self.lock().members.iter().any(|m| m.user_id == Some(user_id))
    }

    pub fn can_control(&self, member: u64) -> bool {
        self.lock().can_control(member)
    }

    pub fn command(&self, member: u64, cmd: Command) -> Result<(), Refusal> {
        let controlled = matches!(
            cmd,
            Command::Play { .. }
                | Command::Pause { .. }
                | Command::Seek { .. }
                | Command::Rate { .. }
                | Command::Tracks { .. }
        );

        if controlled && !self.can_control(member) {
            return Err("only the host can control playback in this room");
        }

        let valid = |p: f64| {
            if p.is_finite() { Ok(p.max(0.0)) } else { Err("bad position") }
        };

        match cmd {
            Command::Play { position } => {
                let position = valid(position)?;

                self.change(|st, t| {
                    st.clock = Clock { paused: false, running: false, position, at: t, rate: st.clock.rate };
                    st.act(member, "play", Some(position));
                });
            },
            Command::Pause { position } => {
                let position = valid(position)?;

                self.change(|st, t| {
                    st.clock = Clock { paused: true, running: false, position, at: t, rate: st.clock.rate };
                    st.act(member, "pause", Some(position));
                });
            },
            Command::Seek { position } => {
                let position = valid(position)?;

                self.change(|st, t| {
                    st.clock.position = position;
                    st.clock.at = t;
                    st.clock.running = false;

                    for m in st.members.iter_mut().filter(|m| m.status == Status::Ready) {
                        m.status = Status::Buffering;
                        m.since = Instant::now();
                    }

                    st.act(member, "seek", Some(position));
                });
            },
            Command::Rate { rate } => {
                if !(rate.is_finite() && (0.25..=3.0).contains(&rate)) {
                    return Err("bad speed");
                }

                self.change(|st, t| {
                    st.clock.rebase(t);
                    st.clock.rate = rate;
                    st.act(member, "rate", Some(rate));
                });
            },
            Command::Status { status } => self.change(|st, _| {
                if let Some(m) = st.members.iter_mut().find(|m| m.id == member) {
                    let status = if status == Status::Buffering && m.status == Status::Joining {
                        Status::Joining
                    } else {
                        status
                    };

                    if m.status != status {
                        m.status = status;
                        m.since = Instant::now();
                    }
                }
            }),
            Command::Tracks { tracks } => self.change(|st, _| {
                if tracks.media_id == st.info.media_id {
                    st.tracks = tracks;
                    st.act(member, "tracks", None);
                }
            }),
            Command::Name { name } => {
                let name = name.trim();

                if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
                    return Err("pick a name up to 40 characters");
                }

                self.change(|st, _| {
                    if let Some(m) = st.members.iter_mut().find(|m| m.id == member && m.user_id.is_none()) {
                        m.name = name.to_string();
                    }
                });
            },
            Command::Ping { .. }
            | Command::Media { .. }
            | Command::Settings { .. }
            | Command::Invite { .. }
            | Command::End => {
                unreachable!("handled by the socket")
            },
        }

        Ok(())
    }

    pub fn set_media(&self, member: u64, id: i64, from: i64) -> bool {
        self.change(|st, t| {
            if st.info.media_id != from || id == from {
                return false;
            }

            st.info.media_id = id;
            st.clock = Clock { paused: st.clock.paused, running: false, position: 0.0, at: t, rate: st.clock.rate };

            for m in st.members.iter_mut().filter(|m| m.status != Status::Blocked) {
                m.status = Status::Loading;
                m.since = Instant::now();
            }

            st.act(member, "media", None);
            true
        })
    }

    pub fn set_settings(
        &self,
        member: u64,
        control: Option<Control>,
        wait_for_all: Option<bool>,
        public: Option<bool>,
    ) {
        self.change(|st, _| {
            if let Some(c) = control {
                st.settings.control = c;
            }

            if let Some(w) = wait_for_all {
                st.settings.wait_for_all = w;
            }

            if let Some(p) = public {
                st.info.public = p;
            }

            st.act(member, "settings", None);
        });
    }

    pub fn invite(&self, user_id: i64) {
        self.change(|st, _| {
            if !st.info.invited.contains(&user_id) {
                st.info.invited.push(user_id);
            }
        });
    }

    fn tick(&self) {
        let mut st = self.lock();
        let before = st.clock.running;
        st.settle(now_ms());

        if st.clock.running != before {
            let _ = self.tx.send(st.snapshot(&self.code).into());
        }
    }
}

#[derive(Default)]
pub struct Together {
    rooms: Mutex<HashMap<String, Arc<Room>>>,
}

type Row = (i64, String, i64, i64, String, bool, String, String, String, f64, bool, f64);

pub struct NewRoom {
    pub host_id: i64,
    pub host_name: String,
    pub media_id: i64,
    pub item_id: i64,
    pub library: String,
    pub public: bool,
    pub tracks: Tracks,
    pub position: f64,
    pub paused: bool,
}

impl Together {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Room>>> {
        self.rooms.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub async fn open(&self, db: &SqlitePool, code: &str) -> sqlx::Result<Option<Arc<Room>>> {
        if let Some(room) = self.map().get(code) {
            return Ok(Some(room.clone()));
        }

        let row: Option<Row> = sqlx::query_as(
            "SELECT r.host_id, u.username, r.media_id, m.item_id, i.library, r.public, r.invited, r.settings, r.tracks,
                    r.position, r.paused, r.rate
             FROM watch_rooms r JOIN users u ON u.id = r.host_id JOIN media m ON m.id = r.media_id JOIN items i ON i.id = m.item_id
             WHERE r.code = ? AND r.last_active > ?",
        )
        .bind(code)
        .bind(now() - EXPIRY_SECS)
        .fetch_optional(db)
        .await?;

        let Some((
            host_id,
            host_name,
            media_id,
            item_id,
            library,
            public,
            invited,
            settings,
            tracks,
            position,
            paused,
            rate,
        )) = row
        else {
            return Ok(None);
        };

        let info = Info {
            host_id,
            media_id,
            item_id,
            library,
            public,
            invited: serde_json::from_str(&invited).unwrap_or_default(),
        };

        let clock = Clock { paused, running: false, position, at: now_ms(), rate };

        let room = Room::new(
            code.to_string(),
            info,
            host_name,
            serde_json::from_str(&settings).unwrap_or_default(),
            serde_json::from_str(&tracks).unwrap_or_default(),
            clock,
        );

        Ok(Some(self.map().entry(code.to_string()).or_insert_with(|| Arc::new(room)).clone()))
    }

    pub async fn create(&self, db: &SqlitePool, new: NewRoom) -> sqlx::Result<Arc<Room>> {
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        let code = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

        let info = Info {
            host_id: new.host_id,
            media_id: new.media_id,
            item_id: new.item_id,
            library: new.library,
            public: new.public,
            invited: Vec::new(),
        };

        let clock =
            Clock { paused: new.paused, running: false, position: new.position.max(0.0), at: now_ms(), rate: 1.0 };

        let room = Arc::new(Room::new(code.clone(), info, new.host_name, Settings::default(), new.tracks, clock));

        sqlx::query(
            "INSERT INTO watch_rooms (code, host_id, media_id, created_at, last_active) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&code)
        .bind(new.host_id)
        .bind(new.media_id)
        .bind(now())
        .bind(now())
        .execute(db)
        .await?;

        self.save(db, &room).await?;
        self.map().insert(code, room.clone());
        Ok(room)
    }

    pub async fn save(&self, db: &SqlitePool, room: &Room) -> sqlx::Result<()> {
        let (info, settings, tracks, clock) = {
            let st = room.lock();
            (st.info.clone(), st.settings.clone(), st.tracks.clone(), st.clock)
        };

        sqlx::query(
            "UPDATE watch_rooms SET media_id = ?, public = ?, invited = ?, settings = ?, tracks = ?, position = ?, paused = ?,
                rate = ?, last_active = ? WHERE code = ?",
        )
        .bind(info.media_id)
        .bind(info.public)
        .bind(json(&info.invited))
        .bind(json(&settings))
        .bind(json(&tracks))
        .bind(clock.position_at(now_ms()))
        .bind(clock.paused)
        .bind(clock.rate)
        .bind(now())
        .bind(&room.code)
        .execute(db)
        .await?;

        Ok(())
    }

    pub async fn end(&self, db: &SqlitePool, code: &str) -> sqlx::Result<()> {
        if let Some(room) = self.map().remove(code) {
            room.close();
        }

        sqlx::query("DELETE FROM watch_rooms WHERE code = ?").bind(code).execute(db).await?;
        Ok(())
    }

    pub fn forget_host(&self, host_id: i64) {
        self.map().retain(|_, r| {
            let keep = r.info().host_id != host_id;

            if !keep {
                r.close();
            }

            keep
        });
    }

    pub fn spawn(self: Arc<Self>, db: SqlitePool) {
        tokio::spawn(async move {
            let mut ticks = 0u64;
            let mut interval = tokio::time::interval(Duration::from_millis(500));

            loop {
                interval.tick().await;
                let rooms: Vec<Arc<Room>> = self.map().values().cloned().collect();

                for r in rooms.iter().filter(|r| !r.is_empty()) {
                    r.tick();
                }

                ticks += 1;

                if ticks % 7200 == 1 {
                    self.map().retain(|_, r| !r.is_empty());

                    if let Err(e) = sqlx::query("DELETE FROM watch_rooms WHERE last_active < ?")
                        .bind(now() - EXPIRY_SECS)
                        .execute(&db)
                        .await
                    {
                        tracing::warn!("can't clean up old rooms: {e}");
                    }
                }
            }
        });
    }
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}
