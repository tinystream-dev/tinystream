// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::db::now;
use crate::together::{Clock, Control, EXPIRY_SECS, STALL_LIMIT, START_LEAD_MS, Settings, Status, now_ms};

pub const ENDED: &str = r#"{"type":"ended"}"#;

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

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    Ping {
        id: u64,
        c: f64,
    },
    Play {
        position: f64,
    },
    Pause {
        position: f64,
    },
    Seek {
        position: f64,
    },
    /// Jumps to another place in the queue; `from` is where the sender thought it was.
    Skip {
        index: usize,
        from: usize,
    },
    Queue {
        tracks: Vec<i64>,
        current: usize,
        position: f64,
    },
    Add {
        tracks: Vec<i64>,
        next: bool,
    },
    Remove {
        index: usize,
    },
    Move {
        from: usize,
        to: usize,
    },
    Status {
        status: Status,
    },
    Name {
        name: String,
    },
    Settings {
        control: Option<Control>,
        wait_for_all: Option<bool>,
        public: Option<bool>,
    },
    Invite {
        user_id: i64,
    },
    End,
}

#[derive(Debug, Clone)]
pub struct Info {
    pub host_id: i64,
    pub public: bool,
    pub invited: Vec<i64>,
    pub queue: Vec<i64>,
    pub current: usize,
}

struct State {
    info: Info,
    durations: HashMap<i64, f64>,
    host_name: String,
    settings: Settings,
    clock: Clock,
    members: Vec<Member>,
    action: Option<(u64, u64, String, &'static str)>,
    seq: u64,
}

impl State {
    fn duration(&self) -> f64 {
        self.info.queue.get(self.info.current).and_then(|id| self.durations.get(id)).copied().unwrap_or(0.0)
    }

    fn settle(&mut self, t: f64) {
        let waiting = self.settings.wait_for_all && self.members.iter().any(Member::holds_up);
        let running = !self.clock.paused && !waiting && !self.info.queue.is_empty();

        if running != self.clock.running {
            self.clock.rebase(t);
            self.clock.running = running;

            if running {
                self.clock.at = t + START_LEAD_MS;
            }
        }
    }

    /// Moves on to the next track once this one's over, as every player does
    /// on its own; the room only keeps count.
    fn advance(&mut self, t: f64) -> bool {
        let mut moved = false;

        while self.clock.running {
            let d = self.duration();

            if d <= 0.0 {
                break;
            }

            let pos = self.clock.position_at(t);

            if pos < d {
                break;
            }

            let over = pos - d;

            if self.info.current + 1 >= self.info.queue.len() {
                self.clock = Clock { paused: true, running: false, position: d, at: t, rate: 1.0 };
                moved = true;
                break;
            }

            self.info.current += 1;
            self.clock.position = over;
            self.clock.at = t;
            moved = true;
        }

        moved
    }

    fn host_here(&self) -> bool {
        self.members.iter().any(|m| m.user_id == Some(self.info.host_id))
    }

    fn can_control(&self, member: u64) -> bool {
        self.settings.control == Control::Everyone
            || !self.host_here()
            || self.members.iter().any(|m| m.id == member && m.user_id == Some(self.info.host_id))
    }

    fn act(&mut self, member: u64, kind: &'static str) {
        self.seq += 1;
        let by = self.members.iter().find(|m| m.id == member).map(|m| m.name.clone()).unwrap_or_default();
        self.action = Some((self.seq, member, by, kind));
    }

    fn snapshot(&self, code: &str) -> String {
        json!({
            "type": "state",
            "code": code,
            "queue": self.info.queue,
            "current": self.info.current,
            "host": { "id": self.info.host_id, "name": self.host_name },
            "public": self.info.public,
            "invited": self.info.invited,
            "settings": self.settings,
            "clock": self.clock,
            "members": self.members.iter().map(|m| json!({
                "id": m.id,
                "name": m.name,
                "userId": m.user_id,
                "avatar": m.avatar,
                "status": m.status,
            })).collect::<Vec<_>>(),
            "action": self.action.as_ref().map(|(seq, member, by, kind)| json!({
                "seq": seq, "member": member, "by": by, "kind": kind,
            })),
        })
        .to_string()
    }

    fn reload_everyone(&mut self) {
        for m in self.members.iter_mut().filter(|m| m.status != Status::Blocked) {
            m.status = Status::Loading;
            m.since = Instant::now();
        }
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
        st.advance(t);
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
            st.act(id, "join");
        });

        id
    }

    pub fn leave(&self, member: u64) -> bool {
        self.change(|st, _| {
            st.act(member, "leave");
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

    /// Whether the queue holds a track, so its files may be handed out to whoever's in the room.
    pub fn holds(&self, track_id: i64) -> bool {
        self.lock().info.queue.contains(&track_id)
    }

    pub fn command(&self, member: u64, cmd: Command) -> Result<(), Refusal> {
        let controlled = !matches!(
            cmd,
            Command::Ping { .. }
                | Command::Status { .. }
                | Command::Name { .. }
                | Command::Settings { .. }
                | Command::Invite { .. }
                | Command::End
        );

        if controlled && !self.can_control(member) {
            return Err("only the host can control playback in this room");
        }

        let valid = |p: f64| if p.is_finite() { Ok(p.max(0.0)) } else { Err("bad position") };

        match cmd {
            Command::Play { position } => {
                let position = valid(position)?;

                self.change(|st, t| {
                    st.clock = Clock { paused: false, running: false, position, at: t, rate: 1.0 };
                    st.act(member, "play");
                });
            },
            Command::Pause { position } => {
                let position = valid(position)?;

                self.change(|st, t| {
                    st.clock = Clock { paused: true, running: false, position, at: t, rate: 1.0 };
                    st.act(member, "pause");
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

                    st.act(member, "seek");
                });
            },
            Command::Skip { index, from } => self.change(|st, t| {
                if st.info.current != from || index >= st.info.queue.len() {
                    return;
                }

                st.info.current = index;
                st.clock = Clock { paused: st.clock.paused, running: false, position: 0.0, at: t, rate: 1.0 };
                st.reload_everyone();
                st.act(member, "skip");
            }),
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
            Command::Remove { index } => self.change(|st, t| {
                if index >= st.info.queue.len() {
                    return;
                }

                st.info.queue.remove(index);

                if index < st.info.current {
                    st.info.current -= 1;
                } else if index == st.info.current {
                    st.info.current = st.info.current.min(st.info.queue.len().saturating_sub(1));
                    st.clock = Clock { paused: st.clock.paused, running: false, position: 0.0, at: t, rate: 1.0 };
                    st.reload_everyone();
                }

                st.act(member, "queue");
            }),
            Command::Move { from, to } => self.change(|st, _| {
                let n = st.info.queue.len();

                if from >= n || to >= n || from == to {
                    return;
                }

                let id = st.info.queue.remove(from);
                st.info.queue.insert(to, id);
                let c = st.info.current;

                st.info.current = if c == from {
                    to
                } else if from < c && to >= c {
                    c - 1
                } else if from > c && to <= c {
                    c + 1
                } else {
                    c
                };

                st.act(member, "queue");
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
            | Command::Queue { .. }
            | Command::Add { .. }
            | Command::Settings { .. }
            | Command::Invite { .. }
            | Command::End => unreachable!("handled by the socket"),
        }

        Ok(())
    }

    /// Changes what's queued; the tracks have been checked already, and come with how long each is.
    pub fn set_queue(&self, member: u64, tracks: Vec<(i64, f64)>, current: usize, position: f64) {
        self.change(|st, t| {
            st.durations.extend(tracks.iter().copied());
            st.info.queue = tracks.into_iter().map(|(id, _)| id).collect();
            st.info.current = current.min(st.info.queue.len().saturating_sub(1));
            st.clock = Clock { paused: st.clock.paused, running: false, position: position.max(0.0), at: t, rate: 1.0 };
            st.reload_everyone();
            st.act(member, "queue");
        });
    }

    pub fn add(&self, member: u64, tracks: Vec<(i64, f64)>, next: bool) {
        self.change(|st, _| {
            st.durations.extend(tracks.iter().copied());
            let at = if next { (st.info.current + 1).min(st.info.queue.len()) } else { st.info.queue.len() };
            let was_empty = st.info.queue.is_empty();
            st.info.queue.splice(at..at, tracks.into_iter().map(|(id, _)| id));

            if was_empty {
                st.reload_everyone();
            }

            st.act(member, "queue");
        });
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

            st.act(member, "settings");
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
        let t = now_ms();
        let before = (st.clock.running, st.info.current);
        let moved = st.advance(t);
        st.settle(t);

        if moved || (st.clock.running, st.info.current) != before {
            let _ = self.tx.send(st.snapshot(&self.code).into());
        }
    }
}

#[derive(Default)]
pub struct Rooms {
    rooms: Mutex<HashMap<String, Arc<Room>>>,
}

pub struct NewRoom {
    pub host_id: i64,
    pub host_name: String,
    pub tracks: Vec<(i64, f64)>,
    pub current: usize,
    pub position: f64,
    pub paused: bool,
    pub public: bool,
}

type Row = (i64, String, String, i64, bool, String, String, f64, bool);

async fn durations(db: &SqlitePool, ids: &[i64]) -> sqlx::Result<HashMap<i64, f64>> {
    let mut out = HashMap::new();

    for id in ids {
        if out.contains_key(id) {
            continue;
        }
        if let Some(d) =
            sqlx::query_scalar::<_, f64>("SELECT duration FROM tracks WHERE id = ?").bind(id).fetch_optional(db).await?
        {
            out.insert(*id, d);
        }
    }

    Ok(out)
}

impl Rooms {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Room>>> {
        self.rooms.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn room(
        code: String,
        info: Info,
        durations: HashMap<i64, f64>,
        host_name: String,
        settings: Settings,
        clock: Clock,
    ) -> Room {
        Room {
            code,
            st: Mutex::new(State {
                info,
                durations,
                host_name,
                settings,
                clock,
                members: Vec::new(),
                action: None,
                seq: 0,
            }),
            tx: broadcast::channel(64).0,
            next_member: AtomicU64::new(1),
        }
    }

    pub async fn open(&self, db: &SqlitePool, code: &str) -> sqlx::Result<Option<Arc<Room>>> {
        if let Some(room) = self.map().get(code) {
            return Ok(Some(room.clone()));
        }

        let row: Option<Row> = sqlx::query_as(
            "SELECT r.host_id, u.username, r.queue, r.current, r.public, r.invited, r.settings, r.position, r.paused
             FROM listen_rooms r JOIN users u ON u.id = r.host_id WHERE r.code = ? AND r.last_active > ?",
        )
        .bind(code)
        .bind(now() - EXPIRY_SECS)
        .fetch_optional(db)
        .await?;

        let Some((host_id, host_name, queue, current, public, invited, settings, position, paused)) = row else {
            return Ok(None);
        };

        let queue: Vec<i64> = serde_json::from_str(&queue).unwrap_or_default();
        let lengths = durations(db, &queue).await?;
        let queue: Vec<i64> = queue.into_iter().filter(|id| lengths.contains_key(id)).collect();

        let info = Info {
            host_id,
            public,
            invited: serde_json::from_str(&invited).unwrap_or_default(),
            current: (current.max(0) as usize).min(queue.len().saturating_sub(1)),
            queue,
        };

        let clock = Clock { paused, running: false, position, at: now_ms(), rate: 1.0 };

        let room = Self::room(
            code.to_string(),
            info,
            lengths,
            host_name,
            serde_json::from_str(&settings).unwrap_or_default(),
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
            public: new.public,
            invited: Vec::new(),
            current: new.current.min(new.tracks.len().saturating_sub(1)),
            queue: new.tracks.iter().map(|(id, _)| *id).collect(),
        };

        let clock =
            Clock { paused: new.paused, running: false, position: new.position.max(0.0), at: now_ms(), rate: 1.0 };

        let room = Arc::new(Self::room(
            code.clone(),
            info,
            new.tracks.into_iter().collect(),
            new.host_name,
            Settings::default(),
            clock,
        ));

        sqlx::query("INSERT INTO listen_rooms (code, host_id, created_at, last_active) VALUES (?, ?, ?, ?)")
            .bind(&code)
            .bind(new.host_id)
            .bind(now())
            .bind(now())
            .execute(db)
            .await?;

        self.save(db, &room).await?;
        self.map().insert(code, room.clone());
        Ok(room)
    }

    pub async fn save(&self, db: &SqlitePool, room: &Room) -> sqlx::Result<()> {
        let (info, settings, clock) = {
            let st = room.lock();
            (st.info.clone(), st.settings.clone(), st.clock)
        };

        sqlx::query(
            "UPDATE listen_rooms SET queue = ?, current = ?, public = ?, invited = ?, settings = ?, position = ?,
                paused = ?, last_active = ? WHERE code = ?",
        )
        .bind(serde_json::to_string(&info.queue).unwrap_or_else(|_| "[]".into()))
        .bind(info.current as i64)
        .bind(info.public)
        .bind(serde_json::to_string(&info.invited).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&settings).unwrap_or_else(|_| "{}".into()))
        .bind(clock.position_at(now_ms()))
        .bind(clock.paused)
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

        sqlx::query("DELETE FROM listen_rooms WHERE code = ?").bind(code).execute(db).await?;
        Ok(())
    }

    pub fn spawn(self: Arc<Self>, db: SqlitePool) {
        tokio::spawn(async move {
            let mut ticks = 0u64;
            let mut interval = tokio::time::interval(Duration::from_millis(250));

            loop {
                interval.tick().await;
                let rooms: Vec<Arc<Room>> = self.map().values().cloned().collect();

                for r in rooms.iter().filter(|r| !r.is_empty()) {
                    r.tick();
                }

                ticks += 1;

                if ticks % 14400 == 1 {
                    self.map().retain(|_, r| !r.is_empty());

                    if let Err(e) = sqlx::query("DELETE FROM listen_rooms WHERE last_active < ?")
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
