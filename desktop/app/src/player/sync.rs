// SPDX-License-Identifier: AGPL-3.0-or-later

//! Keeps one mpv on a watch-together room's clock: `sync.ts`, with the same behaviour and
//! constants, so mixed rooms behave the same.
//!
//! The server holds the room's clock: a position, the server time it was true at, a rate, and
//! whether it's moving. We measure how far our clock is from the server's (NTP-style, keeping the
//! quickest round trip), so we can work out exactly where the room is at any moment without
//! asking. A small control loop then steers mpv there: small drift is absorbed by briefly nudging
//! `speed`, which nobody notices; big drift is a seek. mpv reports time exactly, so the web's
//! Firefox workarounds aren't needed, but the loop is otherwise the web's.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::{SinkExt, StreamExt};
use mpv::{Mpv, Node};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use super::tracks::{self, AudioChoice, SubtitleChoice, same_audio, same_subtitle};
use crate::api::Api;
use crate::api::playback::{Playback, PlaybackMediaSubtitles as Subtitle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Joining,
    Loading,
    Ready,
    Buffering,
    Blocked,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: u64,
    pub name: String,
    pub user_id: Option<i64>,
    pub avatar: Option<i64>,
    pub status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Clock {
    pub paused: bool,
    pub running: bool,
    pub position: f64,
    pub at: f64,
    pub rate: f64,
}

/// The audio and subtitles everyone gets unless they pick their own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SharedTracks {
    pub media_id: i64,
    pub audio: Option<i64>,
    pub audio_language: Option<String>,
    pub audio_title: Option<String>,
    pub audio_channels: Option<i64>,
    pub audio_codec: Option<String>,
    /// `None` is off.
    pub subtitle: Option<String>,
    pub subtitle_language: Option<String>,
    pub subtitle_title: Option<String>,
    pub subtitle_forced: bool,
    pub subtitle_codec: Option<String>,
}

impl SharedTracks {
    /// How the room's tracks describe `audio` and `subtitle` of `pb` (`roomAudio`, `roomSubtitle`).
    pub fn of(pb: &Playback, audio: Option<i64>, subtitle: Option<&str>) -> Self {
        let a = pb.media.audio.iter().find(|a| Some(a.index) == audio);
        let s = pb.media.subtitles.iter().find(|s| Some(s.id.as_str()) == subtitle);
        Self {
            media_id: pb.id,
            audio,
            audio_language: a.and_then(|a| a.language.clone()),
            audio_title: a.and_then(|a| a.title.clone()),
            audio_channels: a.map(|a| a.channels),
            audio_codec: a.map(|a| a.codec.clone()),
            subtitle: subtitle.map(str::to_owned),
            subtitle_language: s.and_then(|s| s.language.clone()),
            subtitle_title: s.and_then(|s| s.title.clone()),
            subtitle_forced: s.is_some_and(|s| s.forced),
            subtitle_codec: s.map(|s| s.codec.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Control {
    Everyone,
    Host,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub control: Control,
    pub wait_for_all: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Host {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    pub seq: u64,
    pub member: u64,
    pub by: String,
    pub kind: String,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomState {
    pub media_id: i64,
    pub host: Host,
    pub public: bool,
    /// Who the host asked along, by user id.
    #[serde(default)]
    pub invited: Vec<i64>,
    pub settings: Settings,
    pub tracks: SharedTracks,
    pub clock: Clock,
    pub members: Vec<Member>,
    pub action: Option<Action>,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub room: Option<RoomState>,
    /// Our member id in the room.
    pub you: Option<u64>,
    pub connected: bool,
    pub ended: bool,
    /// The last thing the server refused, in words.
    pub error: Option<(String, Instant)>,
}

/// Drift we leave alone, in seconds.
const DEADBAND: f64 = 0.015;
/// Beyond this, seek instead of speeding up or slowing down.
const HARD_DRIFT: f64 = 0.25;
/// How many loop readings small drift is judged over.
const WINDOW: usize = 10;
/// How long a nudge takes to close the drift, in seconds.
const CATCH_UP: f64 = 1.0;
/// The most we'll bend the playback rate to catch up (±).
const MAX_NUDGE: f64 = 0.08;
/// Starving for this long (ms) while playing counts as buffering.
const STARVE_MS: f64 = 400.0;
/// Seconds buffered ahead before someone who ran dry is ready again. Saying so at the first frame
/// back has a slow connection stop the room every few seconds; this makes it fewer, longer waits.
const REFILL: f64 = 5.0;
/// How often the loop looks.
const LOOP: Duration = Duration::from_millis(100);

pub fn position_at(c: &Clock, t: f64) -> f64 {
    if c.running { c.position + ((t - c.at).max(0.0) / 1000.0) * c.rate } else { c.position }
}

pub fn can_control(room: &RoomState, you: Option<u64>) -> bool {
    if room.settings.control == Control::Everyone {
        return true;
    }
    let host_here = room.members.iter().any(|m| m.user_id == Some(room.host.id));
    !host_here || room.members.iter().any(|m| Some(m.id) == you && m.user_id == Some(room.host.id))
}

/// The shared tracks for this video. Everyone works this out the same way from the same facts, so
/// moving to a new episode needs no extra round trip.
pub fn pick_shared(pb: &Playback, t: &SharedTracks) -> (Option<i64>, Option<String>) {
    let same = t.media_id == pb.id;
    let audio = pb.media.audio.iter().find(|a| same && Some(a.index) == t.audio).or_else(|| {
        let want = AudioChoice {
            language: t.audio_language.clone(),
            title: t.audio_title.clone(),
            channels: t.audio_channels,
            codec: t.audio_codec.clone(),
        };
        same_audio(&pb.media.audio, &want)
            .or_else(|| pb.media.audio.iter().find(|a| a.default))
            .or_else(|| pb.media.audio.first())
    });
    let usable: Vec<&Subtitle> = pb.media.subtitles.iter().filter(|s| tracks::usable(s)).collect();
    let subtitle = t.subtitle.as_ref().and_then(|id| {
        let want = SubtitleChoice {
            language: t.subtitle_language.clone(),
            title: t.subtitle_title.clone(),
            forced: t.subtitle_forced,
            codec: t.subtitle_codec.clone(),
        };
        usable
            .iter()
            .find(|s| same && &s.id == id)
            .map(|s| s.id.clone())
            .or_else(|| same_subtitle(&usable, &want))
            .or_else(|| usable.iter().find(|s| s.default).map(|s| s.id.clone()))
            .or_else(|| usable.iter().find(|s| !s.forced).map(|s| s.id.clone()))
    });
    (audio.map(|a| a.index), subtitle)
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum Outgoing {
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
    Rate {
        rate: f64,
    },
    Status {
        status: Status,
    },
    Tracks {
        tracks: SharedTracks,
    },
    Media {
        id: i64,
        from: i64,
    },
    Name {
        name: String,
    },
    Settings {
        #[serde(skip_serializing_if = "Option::is_none")]
        control: Option<Control>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait_for_all: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        public: Option<bool>,
    },
    Invite {
        user_id: i64,
    },
    End,
}

/// Our clock, in ms since the epoch, moving steadily even if the wall clock is set.
fn local_now() -> f64 {
    static BASE: OnceLock<(Instant, f64)> = OnceLock::new();
    let (instant, wall) = BASE.get_or_init(|| {
        (Instant::now(), SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0))
    });
    wall + instant.elapsed().as_secs_f64() * 1000.0
}

/// The video being steered.
struct Steer {
    mpv: Mpv,
    media_id: i64,
    duration: f64,
    /// Whether mpv plays the file itself, so it can seek anywhere; a transcode is rebuilt instead.
    direct: bool,
    rebuild: Arc<dyn Fn(f64) + Send + Sync>,
    starving_since: Option<f64>,
    last_seek: f64,
    last_rebuild: f64,
    /// Whether we ran out while playing and are still refilling.
    ran_dry: bool,
    /// Recent drift readings at the room's own rate, for judging small drift.
    drifts: Vec<f64>,
    /// When the nudge under way ends (server time, ms).
    nudge_until: Option<f64>,
    /// The clock the readings were taken against; a new one starts them over.
    steered: Option<Clock>,
}

struct Inner {
    snap: Snapshot,
    name: Option<String>,
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
    outbox: Option<mpsc::UnboundedSender<String>>,
    /// Server time minus ours, in ms, from the quickest recent round trip.
    offset: f64,
    samples: Vec<(f64, f64)>,
    ping_seq: u64,
    steer: Option<Steer>,
    sent_status: Option<(Status, Instant)>,
}

/// One room's connection, and the loop that keeps a player on its clock.
#[derive(Clone)]
pub struct SyncClient {
    code: String,
    inner: Arc<Mutex<Inner>>,
    closed: Arc<AtomicBool>,
    /// Bumped on each attach and detach, so an old loop knows to stop.
    attached: Arc<AtomicU64>,
}

impl SyncClient {
    pub fn new(code: String, name: Option<String>) -> Self {
        Self {
            code,
            inner: Arc::new(Mutex::new(Inner {
                snap: Snapshot::default(),
                name,
                notify: None,
                outbox: None,
                offset: 0.0,
                samples: Vec::new(),
                ping_seq: 0,
                steer: None,
                sent_status: None,
            })),
            closed: Arc::new(AtomicBool::new(true)),
            attached: Arc::new(AtomicU64::new(0)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn snapshot(&self) -> Snapshot {
        self.lock().snap.clone()
    }

    /// Joins the room, calling `notify` whenever the snapshot changes, and keeps trying until
    /// closed.
    pub fn connect(&self, api: Api, runtime: &tokio::runtime::Handle, notify: Arc<dyn Fn() + Send + Sync>) {
        if !self.closed.swap(false, Ordering::Relaxed) {
            return;
        }
        self.lock().notify = Some(notify);
        let client = self.clone();
        runtime.spawn(async move { client.run(api).await });
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        self.detach();
        let mut inner = self.lock();
        inner.outbox = None;
        inner.notify = None;
    }

    async fn run(self, api: Api) {
        let mut retry = 0;
        while !self.closed.load(Ordering::Relaxed) {
            if let Err(error) = self.session(&api, &mut retry).await {
                tracing::info!("watch together connection: {error}");
            }
            {
                let mut inner = self.lock();
                inner.outbox = None;
                inner.snap.connected = false;
                inner.changed();
                if inner.snap.ended {
                    return;
                }
            }
            if self.closed.load(Ordering::Relaxed) {
                return;
            }
            // Back off, but not by much: people are waiting.
            let wait = (400u64 << retry.min(4)).min(5000);
            retry += 1;
            tokio::time::sleep(Duration::from_millis(wait)).await;
        }
    }

    async fn session(&self, api: &Api, retry: &mut u32) -> Result<(), String> {
        let mut url = api.origin.join(&format!("api/together/{}/ws", self.code)).map_err(|e| e.to_string())?;
        url.set_scheme(if api.origin.scheme() == "https" { "wss" } else { "ws" }).map_err(|_| "bad address")?;
        if let Some(name) = self.lock().name.clone() {
            url.query_pairs_mut().append_pair("name", &name);
        }
        let mut request = url.as_str().into_client_request().map_err(|e| e.to_string())?;
        if let Some(token) = &api.token {
            request
                .headers_mut()
                .insert("Authorization", format!("Bearer {}", token.0).parse().map_err(|_| "bad token")?);
        }
        let (socket, _) = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(request))
            .await
            .map_err(|_| "timed out")?
            .map_err(|e| e.to_string())?;
        let (mut write, mut read) = socket.split();
        let (send, mut outbox) = mpsc::unbounded_channel();
        {
            let mut inner = self.lock();
            inner.outbox = Some(send);
            inner.samples.clear();
            inner.snap.connected = true;
            inner.changed();
        }
        *retry = 0;

        let writer = async {
            while let Some(text) = outbox.recv().await {
                if write.send(Frame::Text(text.into())).await.is_err() {
                    break;
                }
            }
        };
        let reader = async {
            while let Some(Ok(frame)) = read.next().await {
                match frame {
                    Frame::Text(text) => self.lock().receive(&text, self),
                    Frame::Close(_) => break,
                    _ => {},
                }
            }
        };
        // A quick burst to lock onto the server's clock, then a steady trickle.
        let pinger = async {
            let mut sent = 0;
            loop {
                self.lock().ping();
                sent += 1;
                tokio::time::sleep(Duration::from_millis(if sent < 6 { 120 } else { 4000 })).await;
            }
        };
        futures::pin_mut!(writer, reader, pinger);
        futures::future::select(futures::future::select(writer, reader), pinger).await;
        Ok(())
    }

    /// Where the room is right now, in seconds.
    pub fn target(&self) -> f64 {
        let inner = self.lock();
        let Some(room) = &inner.snap.room else { return 0.0 };
        let t = position_at(&room.clock, local_now() + inner.offset);
        match inner.steer.as_ref().map(|steer| steer.duration).filter(|d| *d > 0.0) {
            Some(duration) => t.min(duration),
            None => t,
        }
    }

    fn send(&self, message: Outgoing) {
        self.lock().send(&message);
    }

    pub fn play(&self, position: f64) {
        self.send(Outgoing::Play { position });
        // The server starts everyone a moment from now; so do we.
        self.lock().predict(|c, now| {
            c.paused = false;
            c.running = true;
            c.position = position;
            c.at = now + 150.0;
        });
    }

    pub fn pause(&self, position: f64) {
        self.send(Outgoing::Pause { position });
        self.lock().predict(|c, _| {
            c.paused = true;
            c.running = false;
            c.position = position;
        });
    }

    pub fn seek(&self, position: f64) {
        self.send(Outgoing::Seek { position });
        self.lock().predict(|c, _| {
            c.running = false;
            c.position = position;
        });
    }

    pub fn set_rate(&self, rate: f64) {
        self.send(Outgoing::Rate { rate });
        self.lock().predict(|c, now| {
            c.position = position_at(c, now);
            c.rate = rate;
        });
    }

    pub fn set_tracks(&self, tracks: SharedTracks) {
        self.send(Outgoing::Tracks { tracks });
    }

    pub fn change_media(&self, id: i64, from: i64) {
        self.send(Outgoing::Media { id, from });
    }

    pub fn rename(&self, name: String) {
        self.lock().name = Some(name.clone());
        self.send(Outgoing::Name { name });
    }

    pub fn settings(&self, control: Option<Control>, wait_for_all: Option<bool>, public: Option<bool>) {
        self.send(Outgoing::Settings { control, wait_for_all, public });
    }

    pub fn invite(&self, user_id: i64) {
        self.send(Outgoing::Invite { user_id });
    }

    pub fn end(&self) {
        self.send(Outgoing::End);
    }

    /// Starts steering `mpv`, which is playing `media_id`. `rebuild` starts a transcode again at a
    /// time outside what it has cached.
    pub fn attach(
        &self,
        mpv: Mpv,
        media_id: i64,
        duration: f64,
        direct: bool,
        rebuild: Arc<dyn Fn(f64) + Send + Sync>,
    ) {
        let generation = self.attached.fetch_add(1, Ordering::Relaxed) + 1;
        {
            let mut inner = self.lock();
            inner.sent_status = None;
            inner.steer = Some(Steer {
                mpv,
                media_id,
                duration,
                direct,
                rebuild,
                starving_since: None,
                last_seek: 0.0,
                last_rebuild: 0.0,
                ran_dry: false,
                drifts: Vec::new(),
                nudge_until: None,
                steered: None,
            });
        }
        let client = self.clone();
        let spawned = std::thread::Builder::new().name("room sync".into()).spawn(move || {
            while client.attached.load(Ordering::Relaxed) == generation {
                let wait = client.lock().tick();
                std::thread::sleep(wait);
            }
        });
        if let Err(error) = spawned {
            tracing::error!("can't keep in step with the room: {error}");
        }
    }

    /// Whether the player now plays the file itself rather than a transcode.
    pub fn set_direct(&self, direct: bool) {
        if let Some(steer) = &mut self.lock().steer {
            steer.direct = direct;
        }
    }

    pub fn detach(&self) {
        self.attached.fetch_add(1, Ordering::Relaxed);
        self.lock().steer = None;
    }
}

impl Inner {
    fn changed(&self) {
        if let Some(notify) = &self.notify {
            notify();
        }
    }

    fn send(&self, message: &Outgoing) {
        if let (Some(outbox), Ok(text)) = (&self.outbox, serde_json::to_string(message)) {
            let _ = outbox.send(text);
        }
    }

    fn ping(&mut self) {
        self.ping_seq += 1;
        self.send(&Outgoing::Ping { id: self.ping_seq, c: local_now() });
    }

    fn receive(&mut self, text: &str, client: &SyncClient) {
        let Ok(message) = serde_json::from_str::<Value>(text) else { return };
        match message["type"].as_str() {
            Some("pong") => {
                let (Some(c), Some(s)) = (message["c"].as_f64(), message["s"].as_f64()) else { return };
                let rtt = local_now() - c;
                // The server read its clock about halfway through the round trip.
                if self.samples.len() >= 12 {
                    self.samples.remove(0);
                }
                self.samples.push((rtt, s - (c + rtt / 2.0)));
                if let Some(&(_, offset)) = self.samples.iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
                    self.offset = offset;
                }
            },
            Some("welcome") => {
                self.sent_status = None;
                self.snap.you = message["you"].as_u64();
                self.changed();
            },
            Some("state") => match serde_json::from_value::<RoomState>(message) {
                Ok(room) => {
                    self.snap.room = Some(room);
                    self.changed();
                    self.tick();
                },
                Err(error) => tracing::warn!("can't read the room: {error}"),
            },
            Some("ended") => {
                self.snap.ended = true;
                self.changed();
                client.closed.store(true, Ordering::Relaxed);
                client.attached.fetch_add(1, Ordering::Relaxed);
                self.steer = None;
                self.outbox = None;
            },
            Some("error") => {
                let text = message["message"].as_str().unwrap_or("The room said no.").to_owned();
                self.snap.error = Some((text, Instant::now()));
                self.changed();
            },
            _ => {},
        }
    }

    /// Applies a change to our copy of the room right away, so the video responds instantly; the
    /// server's answer follows a moment later.
    fn predict(&mut self, change: impl FnOnce(&mut Clock, f64)) {
        let now = local_now() + self.offset;
        let Some(room) = &mut self.snap.room else { return };
        room.clock.at = now;
        change(&mut room.clock, now);
        self.changed();
        self.tick();
    }

    /// One look at the video; returns how long until the next.
    fn tick(&mut self) -> Duration {
        let now = local_now() + self.offset;
        let Some(room) = &self.snap.room else { return LOOP };
        let (clock, media_id) = (room.clock, room.media_id);
        let Some(steer) = self.steer.as_mut().filter(|steer| steer.media_id == media_id) else { return LOOP };
        let (status, wait) = steer.tick(clock, now);
        self.report(status);
        wait
    }

    /// Tells the room how we're doing, when the room thinks otherwise.
    fn report(&mut self, status: Status) {
        let Some(room) = &self.snap.room else { return };
        let Some(me) = room.members.iter().find(|m| Some(m.id) == self.snap.you) else { return };
        // Someone still arriving isn't waited for, whatever they say.
        let agrees = me.status == status
            || (status == Status::Buffering && matches!(me.status, Status::Joining | Status::Loading));
        if agrees {
            return;
        }
        if self.sent_status.is_some_and(|(sent, at)| sent == status && at.elapsed() < Duration::from_millis(500)) {
            return;
        }
        self.sent_status = Some((status, Instant::now()));
        self.send(&Outgoing::Status { status });
    }
}

impl Steer {
    fn tick(&mut self, c: Clock, now: f64) -> (Status, Duration) {
        let mpv = self.mpv.clone();
        // Nothing to steer until the stream has something to show.
        let Ok(current) = mpv.get_property::<f64>("time-pos") else { return (Status::Buffering, LOOP) };
        let flag = |name| mpv.get_property::<bool>(name).unwrap_or(false);
        let (paused, seeking) = (flag("pause"), flag("seeking"));
        let starving = seeking || flag("paused-for-cache");
        let speed = mpv.get_property::<f64>("speed").unwrap_or(1.0);
        let end = if self.duration > 0.0 { self.duration } else { f64::INFINITY };
        let target = position_at(&c, now).min(end);
        self.starving_since = if starving { Some(self.starving_since.unwrap_or(now)) } else { None };
        if self.steered != Some(c) {
            self.steered = Some(c);
            self.restart();
        }

        if !c.running || now < c.at {
            // Hold still where the room is, ready to go.
            if !paused {
                set(&mpv, "pause", true);
            }
            if (current - target).abs() > 0.04 && !seeking {
                self.seek(target, now);
            }
            if speed != c.rate {
                set(&mpv, "speed", c.rate);
            }
            // Starting shortly: start on the exact moment rather than catching up.
            let wait =
                if c.running { Duration::from_secs_f64(((c.at - now) / 1000.0).max(0.0)).min(LOOP) } else { LOOP };
            let ahead = mpv.get_property::<f64>("demuxer-cache-duration").unwrap_or(0.0);
            if ahead >= REFILL || current + ahead >= end - 0.5 {
                self.ran_dry = false;
            }
            return (if starving || self.ran_dry { Status::Buffering } else { Status::Ready }, wait);
        }

        // The credits are over for everyone.
        if flag("eof-reached") || target >= end - 0.05 {
            return (Status::Ready, LOOP);
        }

        let drift = current - target;
        if drift.abs() > HARD_DRIFT {
            // One seek at a time: let the last one land before judging again.
            if !seeking && now - self.last_seek > 600.0 {
                self.last_seek = now;
                self.seek(target, now);
            }
            self.restart();
            if speed != c.rate {
                set(&mpv, "speed", c.rate);
            }
        } else if starving {
            // Where a seek or a stall leaves the playhead says nothing about drift.
            self.restart();
        } else {
            self.steer(&c, drift, now, speed);
        }
        if paused {
            set(&mpv, "pause", false);
        }
        let buffering = self.starving_since.is_some_and(|since| now - since > STARVE_MS);
        if buffering {
            self.ran_dry = true;
        }
        (if buffering { Status::Buffering } else { Status::Ready }, LOOP)
    }

    fn seek(&mut self, to: f64, now: f64) {
        if self.direct || cached(&self.mpv, to) {
            if let Err(error) = self.mpv.command_async(0, &["seek", &format!("{to:.3}"), "absolute+exact"]) {
                tracing::debug!("can't seek: {error}");
            }
        } else if now - self.last_rebuild > 2000.0 {
            self.last_rebuild = now;
            (self.rebuild)(to);
        }
    }

    fn restart(&mut self) {
        self.drifts.clear();
        self.nudge_until = None;
    }

    /// Absorbs small drift. It's judged over a second of readings, then closed with one nudge held
    /// for just long enough. Bending the rate on every reading chases noise.
    fn steer(&mut self, c: &Clock, drift: f64, now: f64, speed: f64) {
        if let Some(until) = self.nudge_until {
            if now < until {
                return;
            }
            self.restart();
        }
        if speed != c.rate {
            set(&self.mpv, "speed", c.rate);
        }
        self.drifts.push(drift);
        if self.drifts.len() < WINDOW {
            return;
        }
        let average = self.drifts.iter().sum::<f64>() / self.drifts.len() as f64;
        self.drifts.remove(0);
        if average.abs() <= DEADBAND {
            return;
        }
        // Ahead: a touch slower. Behind: a touch faster.
        let nudge = (-average / CATCH_UP).clamp(-MAX_NUDGE, MAX_NUDGE);
        let r = (c.rate * (1.0 + nudge) * 200.0).round() / 200.0;
        if r == c.rate {
            return;
        }
        set(&self.mpv, "speed", r);
        self.drifts.clear();
        self.nudge_until = Some(now + (-average / (r - c.rate)) * 1000.0);
    }
}

fn set<T: mpv::SetProperty>(mpv: &Mpv, name: &str, value: T) {
    if let Err(error) = mpv.set_property(name, value) {
        tracing::debug!("can't set mpv's {name}: {error}");
    }
}

/// Whether `at` is in what mpv has cached, so a transcode can seek there.
fn cached(mpv: &Mpv, at: f64) -> bool {
    let Ok(state) = mpv.get_property::<Node>("demuxer-cache-state") else { return false };
    let ranges = state.get("seekable-ranges").map(Node::as_array).unwrap_or_default();
    ranges.iter().any(|range| {
        range.get("start").and_then(Node::as_f64).is_some_and(|start| start <= at)
            && range.get("end").and_then(Node::as_f64).is_some_and(|end| end >= at)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(running: bool, position: f64, at: f64, rate: f64) -> Clock {
        Clock { paused: !running, running, position, at, rate }
    }

    #[test]
    fn positions_move_at_the_room_rate_only_once_started() {
        let c = clock(true, 10.0, 1000.0, 1.5);
        assert_eq!(position_at(&c, 500.0), 10.0);
        assert_eq!(position_at(&c, 3000.0), 13.0);
        assert_eq!(position_at(&clock(false, 10.0, 1000.0, 1.0), 9000.0), 10.0);
    }

    #[test]
    fn reads_the_servers_room_state() {
        let state = r#"{"type":"state","code":"abc","mediaId":3,"itemId":1,"host":{"id":1,"name":"david"},
            "public":false,"invited":[7],"settings":{"control":"host","waitForAll":true},
            "tracks":{"mediaId":3,"audio":1,"audioLanguage":"jpn","audioTitle":null,"audioChannels":2,
            "audioCodec":"opus","subtitle":"s2","subtitleLanguage":"eng","subtitleTitle":null,
            "subtitleForced":false,"subtitleCodec":"ass"},
            "clock":{"paused":false,"running":true,"position":12.5,"at":1700000000000.0,"rate":1.0},
            "members":[{"id":1,"name":"david","userId":1,"avatar":null,"status":"ready"},
            {"id":2,"name":"Guest Otter","userId":null,"avatar":null,"status":"buffering"}],
            "action":{"seq":4,"member":2,"by":"Guest Otter","kind":"seek","value":12.5}}"#;
        let room: RoomState = serde_json::from_str(state).unwrap();
        assert_eq!(room.members[1].status, Status::Buffering);
        assert_eq!(room.invited, [7]);
        assert!(can_control(&room, Some(1)));
        assert!(!can_control(&room, Some(2)));
    }

    #[test]
    #[ignore = "plays TINYSTREAM_TEST_VIDEO (a minute or longer) through mpv without output"]
    fn steers_mpv_onto_the_room_clock() {
        let video = std::env::var("TINYSTREAM_TEST_VIDEO").expect("set TINYSTREAM_TEST_VIDEO");
        let mpv = Mpv::new(&[("vo", "null"), ("ao", "null"), ("idle", "yes"), ("keep-open", "yes")]).unwrap();
        mpv.command(&["loadfile", &video, "replace", "-1", "pause=yes"]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while mpv.get_property::<f64>("time-pos").is_err() {
            assert!(Instant::now() < deadline, "the video didn't load");
            std::thread::sleep(Duration::from_millis(20));
        }
        let duration = mpv.get_property::<f64>("duration").unwrap();
        let client = SyncClient::new("test".into(), None);
        let state = |position: f64, at: f64| {
            format!(
                r#"{{"type":"state","mediaId":1,"host":{{"id":1,"name":"host"}},"public":false,
                "settings":{{"control":"everyone","waitForAll":true}},"tracks":{{}},
                "clock":{{"paused":false,"running":true,"position":{position},"at":{at},"rate":1.0}},
                "members":[{{"id":1,"name":"host","userId":1,"avatar":null,"status":"ready"}}],"action":null}}"#
            )
        };
        client.lock().receive(r#"{"type":"welcome","you":1}"#, &client);
        client.lock().receive(&state(10.0, local_now()), &client);
        client.attach(mpv.clone(), 1, duration, true, Arc::new(|_| {}));
        let drift = || mpv.get_property::<f64>("time-pos").unwrap() - client.target();
        let settle = |seconds: u64, within: f64, what: &str| {
            std::thread::sleep(Duration::from_secs(seconds));
            let mut worst: f64 = 0.0;
            for _ in 0..20 {
                worst = worst.max(drift().abs());
                std::thread::sleep(Duration::from_millis(50));
            }
            println!("{what}: worst drift {:.1} ms", worst * 1000.0);
            assert!(worst < within, "{what}: {worst:.3}s off the room");
        };
        settle(2, 0.05, "joining");

        // A small slip is absorbed by bending the speed rather than seeking.
        let at = mpv.get_property::<f64>("time-pos").unwrap();
        mpv.command(&["seek", &format!("{:.3}", at + 0.12), "absolute+exact"]).unwrap();
        let mut nudged = false;
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            nudged |= mpv.get_property::<f64>("speed").unwrap() < 1.0;
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(nudged, "small drift wasn't nudged");
        settle(0, 0.03, "after a nudge");

        client.pause(30.0);
        std::thread::sleep(Duration::from_millis(800));
        assert!(mpv.get_property::<bool>("pause").unwrap());
        assert!((mpv.get_property::<f64>("time-pos").unwrap() - 30.0).abs() < 0.05);

        client.play(30.0);
        settle(2, 0.05, "playing again");

        client.set_rate(1.5);
        settle(2, 0.05, "at 1.5x");
        assert!((mpv.get_property::<f64>("speed").unwrap() - 1.5).abs() < 0.1);
        client.close();
    }

    #[test]
    #[ignore = "needs a test server (TINYSTREAM_TEST_SERVER, TINYSTREAM_TEST_TOKEN, TINYSTREAM_TEST_VIDEO_ID)"]
    fn hosts_and_guests_share_a_room_on_the_server() {
        use crate::player::engine::{Engine, Source};

        let origin: url::Url =
            std::env::var("TINYSTREAM_TEST_SERVER").expect("set TINYSTREAM_TEST_SERVER").parse().unwrap();
        let token = crate::api::Token(std::env::var("TINYSTREAM_TEST_TOKEN").expect("set TINYSTREAM_TEST_TOKEN"));
        let id: i64 = std::env::var("TINYSTREAM_TEST_VIDEO_ID").unwrap_or_else(|_| "1".into()).parse().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let host_api = Api::new(origin.clone(), Some(token)).unwrap();
        let guest_api = Api::new(origin, None).unwrap();
        let variables = serde_json::from_value(serde_json::json!({
            "input": {"videoId": id, "position": 5.0, "paused": true, "public": true, "tracks": {"subtitleForced": false}}
        }))
        .unwrap();
        let code = runtime.block_on(host_api.request::<crate::api::StartRoom>(variables)).unwrap().start_room.code;
        let info = runtime
            .block_on(guest_api.request::<crate::api::Room>(crate::api::room::Variables { code: code.clone() }))
            .unwrap();
        assert!(!info.room.signed_in);

        let host = SyncClient::new(code.clone(), None);
        let guest = SyncClient::new(code.clone(), Some("Cranky".into()));
        host.connect(host_api.clone(), runtime.handle(), Arc::new(|| {}));
        guest.connect(guest_api.clone(), runtime.handle(), Arc::new(|| {}));
        let wait = |what: &str, done: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !done() {
                assert!(Instant::now() < deadline, "{what} didn't happen");
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        wait("both joining", &|| {
            [&host, &guest].iter().all(|c| {
                let snap = c.snapshot();
                snap.you.is_some() && snap.room.is_some_and(|room| room.members.len() == 2)
            })
        });
        let room = guest.snapshot().room.unwrap();
        assert!(room.members.iter().any(|m| m.name == "Cranky" && m.user_id.is_none()));
        assert!(can_control(&room, guest.snapshot().you));

        // The guest plays the original file through the room's own route.
        let engine = Engine::with_config(guest_api, runtime.handle().clone(), None).unwrap();
        engine.set("vo", "null");
        engine.set("ao", "null");
        engine.load(&Source::Direct.path(&format!("api/together/{code}/media/{id}"), None, None), guest.target(), true);
        wait("the file loading", &|| engine.mpv.get_property::<f64>("time-pos").is_ok());
        let duration = engine.mpv.get_property::<f64>("duration").unwrap();
        guest.attach(engine.mpv.clone(), id, duration, true, Arc::new(|_| {}));
        let me = guest.snapshot().you;
        wait("the guest saying it's ready", &|| {
            host.snapshot()
                .room
                .is_some_and(|room| room.members.iter().any(|m| Some(m.id) == me && m.status == Status::Ready))
        });

        host.play(5.0);
        wait("the room starting", &|| guest.snapshot().room.is_some_and(|room| room.clock.running));
        std::thread::sleep(Duration::from_secs(3));
        let drift = |what: &str| {
            let mut worst: f64 = 0.0;
            for _ in 0..20 {
                let at = engine.mpv.get_property::<f64>("time-pos").unwrap();
                worst = worst.max((at - host.target()).abs());
                std::thread::sleep(Duration::from_millis(50));
            }
            println!("{what}: worst drift from the host's clock {:.1} ms", worst * 1000.0);
            assert!(worst < 0.08, "{what}: {worst:.3}s off");
        };
        drift("playing");

        host.seek(40.0);
        wait("the guest following a seek", &|| {
            engine.mpv.get_property::<f64>("time-pos").is_ok_and(|at| (at - 40.0).abs() < 0.05)
        });
        host.play(40.0);
        std::thread::sleep(Duration::from_secs(3));
        drift("after the host's seek");

        guest.pause(42.0);
        wait("the host seeing the guest's pause", &|| {
            host.snapshot().room.is_some_and(|room| room.clock.paused && room.clock.position == 42.0)
        });
        wait("the guest holding still", &|| {
            engine.mpv.get_property::<bool>("pause").unwrap_or(false)
                && engine.mpv.get_property::<f64>("time-pos").is_ok_and(|at| (at - 42.0).abs() < 0.05)
        });

        // Guests can't invite; the host's invites show in the room.
        guest.invite(room.host.id);
        wait("the room refusing a guest's invite", &|| guest.snapshot().error.is_some());
        let people = runtime.block_on(host_api.request::<crate::api::People>(crate::api::people::Variables)).unwrap();
        if let Some(other) = people.users.iter().find(|p| p.id != room.host.id) {
            host.invite(other.id);
            wait("the host's invite", &|| host.snapshot().room.is_some_and(|room| room.invited.contains(&other.id)));
        }

        host.end();
        wait("the room ending", &|| guest.snapshot().ended);
        host.close();
        guest.close();
    }

    #[test]
    fn writes_messages_as_the_server_reads_them() {
        let text = |m: Outgoing| serde_json::to_string(&m).unwrap();
        assert_eq!(text(Outgoing::Seek { position: 4.0 }), r#"{"type":"seek","position":4.0}"#);
        assert_eq!(text(Outgoing::Status { status: Status::Buffering }), r#"{"type":"status","status":"buffering"}"#);
        assert_eq!(
            text(Outgoing::Settings { control: None, wait_for_all: Some(false), public: None }),
            r#"{"type":"settings","waitForAll":false}"#
        );
        assert_eq!(text(Outgoing::Invite { user_id: 7 }), r#"{"type":"invite","userId":7}"#);
        assert_eq!(text(Outgoing::End), r#"{"type":"end"}"#);
    }
}
