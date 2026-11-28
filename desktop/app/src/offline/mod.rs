// SPDX-License-Identifier: AGPL-3.0-or-later

//! Offline downloads (#97): episodes, films, albums and playlists kept on this computer with what
//! playing them needs, so they play without the server. It's opt-in on both sides: the server lets
//! someone (the `offlineDownloads` permission), and they turn it on in Settings → Desktop.
//!
//! While the server can't be reached, the API answers from what's kept and holds on to the progress
//! and plays made meanwhile, to send once it's back. Players look for a kept file before asking the
//! server, online or not.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::{Value, json};

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};

pub mod fetch;
pub mod store;
pub mod view;

pub use store::{Index, Key, State, Store};

/// Space recently played tracks may take before the least recent go.
pub const CACHE: u64 = 2 << 30;
/// Tracks after the one playing that are fetched ahead.
pub const AHEAD: usize = 3;

/// Requests kept to send later when they're made offline.
const REPORTED: &[&str] = &["SaveProgress", "MusicPlayed"];

static ON: AtomicBool = AtomicBool::new(false);
static STORES: LazyLock<Mutex<BTreeMap<String, Arc<Store>>>> = LazyLock::new(Default::default);
static AWAY: LazyLock<Mutex<BTreeSet<String>>> = LazyLock::new(Default::default);

/// Turns offline downloads on or off for the whole app.
pub fn turn(on: bool) {
    ON.store(on, Ordering::Release);
}

/// Where the downloads from the server at `origin` are kept.
pub fn root(origin: &url::Url) -> Option<PathBuf> {
    let hash = ring::digest::digest(&ring::digest::SHA256, origin.as_str().as_bytes());
    let name: String = hash.as_ref()[..8].iter().map(|b| format!("{b:02x}")).collect();
    Some(crate::paths::data_dir()?.join("offline").join(name))
}

/// The address a saved server's `url` is reached at, as its downloads are known by.
pub fn origin(url: &str) -> Option<String> {
    api::addresses(url).ok()?.into_iter().next().map(String::from)
}

/// The kept file from `path` on `api`'s server, when downloads are on and it's all here.
pub fn local(api: &Api, path: &str) -> Option<PathBuf> {
    Offline::new(&api.origin)?.store.local(path)
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// One server's offline side, while offline downloads are on.
pub struct Offline {
    origin: String,
    pub store: Arc<Store>,
}

impl Offline {
    pub fn new(origin: &url::Url) -> Option<Self> {
        if !ON.load(Ordering::Acquire) {
            return None;
        }
        let key = origin.as_str().to_owned();
        let mut stores = STORES.lock().unwrap_or_else(|e| e.into_inner());
        let store = match stores.get(&key) {
            Some(store) => store.clone(),
            None => {
                let store = Arc::new(Store::open(root(origin)?));
                stores.insert(key.clone(), store.clone());
                store
            },
        };
        Some(Self { origin: key, store })
    }

    /// Whether the server couldn't be reached last time.
    pub fn away(&self) -> bool {
        AWAY.lock().unwrap_or_else(|e| e.into_inner()).contains(&self.origin)
    }

    pub fn left(&self) {
        AWAY.lock().unwrap_or_else(|e| e.into_inner()).insert(self.origin.clone());
    }

    pub fn back(&self) {
        AWAY.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.origin);
    }

    /// What the server said to `operation` with `variables`, when something kept has it.
    pub fn answer(&self, operation: &str, variables: &Value) -> Option<Value> {
        self.store.read(|index| index.answer(&store::answer_key(operation, variables)).cloned())
    }

    /// Keeps a request the server didn't get (`body`), when it's one to send later.
    pub fn report(&self, operation: &str, body: Value) {
        if REPORTED.contains(&operation) {
            self.store.change(|index| report(index, operation, body, now()));
        }
    }
}

/// Keeps `body` to send later, with when it happened. A newer position replaces an older one of
/// the same video, and is where it plays from offline.
fn report(index: &mut Index, operation: &str, mut body: Value, now: i64) {
    if body["variables"]["at"].is_null() {
        body["variables"]["at"] = json!(now);
    }
    if operation == "SaveProgress" {
        let variables = &body["variables"];
        if let (Some(video), Some(position), Some(duration)) =
            (variables["videoId"].as_i64(), variables["position"].as_f64(), variables["duration"].as_f64())
        {
            index.reports.retain(|r| r["operationName"] != "SaveProgress" || r["variables"]["videoId"] != video);
            if let Some(item) = index.items.get_mut(&Key::Video(video)) {
                for (_, answer) in item.answers.iter_mut().filter(|(key, _)| key.starts_with("Playback ")) {
                    if answer["video"].is_object() {
                        answer["video"]["position"] = json!(position);
                        answer["video"]["finished"] = json!(position / duration >= 0.92 || duration - position <= 30.0);
                    }
                }
            }
        }
    }
    index.reports.push(body);
}

/// How a download is doing, for buttons and lists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Missing,
    Queued,
    Paused,
    /// How far along, 0 to 1.
    Downloading(f32),
    Done,
    Failed,
}

/// The download under way.
pub struct Active {
    pub key: Key,
    pub done: u64,
    pub total: Option<u64>,
    serial: u64,
    stop: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Keep these.
    Download(Vec<Key>),
    /// Tracks playing now and next: the first is playing, and they're kept only for a while.
    Cache(Vec<Key>),
    Pause(Key),
    Resume(Key),
    Remove(Key),
    PauseAll,
    ResumeAll,
    Progress(u64, u64, Option<u64>),
    Finished(u64, Key, Result<(), fetch::Error>),
    Flushed(Result<(), api::Error>),
    /// What each server's downloads take up, by its address.
    Usage(BTreeMap<String, u64>),
    /// Removes everything downloaded from the server at this address.
    Forget(String),
}

/// The app's downloads, from the server in use.
#[derive(Default)]
pub struct Downloads {
    /// Offline downloads are on in Settings.
    pub on: bool,
    /// The server lets this person download.
    pub allowed: bool,
    api: Option<Api>,
    store: Option<Arc<Store>>,
    /// What's kept, as of the last change.
    pub index: Arc<Index>,
    pub active: Option<Active>,
    serial: u64,
    /// The server couldn't be reached, so the queue waits for it.
    waiting: bool,
    flushing: bool,
    pub usage: BTreeMap<String, u64>,
    /// Whether `usage` was worked out since the last change.
    measured: bool,
}

impl Downloads {
    /// Follows the app's setting and the server in use.
    pub fn follow(&mut self, on: bool, api: Option<&Api>, effects: &mut Vec<Effect>) {
        turn(on);
        if self.on == on && self.api.as_ref().map(|a| &a.origin) == api.map(|a| &a.origin) {
            return;
        }
        self.halt();
        self.on = on;
        self.api = api.cloned();
        self.store = api.and_then(|api| Offline::new(&api.origin)).map(|offline| offline.store);
        self.waiting = false;
        self.refresh();
        self.pump(effects);
    }

    /// Whether the server lets this person download (`permissions.offlineDownloads`).
    pub fn allow(&mut self, allowed: bool, effects: &mut Vec<Effect>) {
        if self.allowed != allowed {
            self.allowed = allowed;
            if !allowed {
                self.halt();
            }
            self.pump(effects);
        }
    }

    /// Whether downloading is possible: it's on, and the server lets this person.
    pub fn ready(&self) -> bool {
        self.on && self.allowed && self.store.is_some()
    }

    /// Whether anything's kept from this server, so there's something to see offline.
    pub fn any(&self) -> bool {
        self.on && self.index.items.values().any(|item| item.kept)
    }

    pub fn status(&self, key: Key) -> Status {
        let Some(item) = self.index.items.get(&key) else { return Status::Missing };
        if let Some(active) = self.active.as_ref().filter(|active| active.key == key) {
            return Status::Downloading(fraction(active.done, active.total));
        }
        if item.parts.is_empty() || item.state != State::Done {
            return match item.state {
                State::Queued => Status::Queued,
                State::Paused => Status::Paused,
                State::Done => Status::Done,
                State::Failed => Status::Failed,
            };
        }
        let parts: Vec<Status> = item.parts.iter().map(|part| self.status(*part)).collect();
        let done = parts.iter().filter(|s| **s == Status::Done).count();
        if done == parts.len() {
            Status::Done
        } else if parts.contains(&Status::Failed) {
            Status::Failed
        } else if parts.iter().all(|s| matches!(s, Status::Paused | Status::Done)) {
            Status::Paused
        } else {
            Status::Downloading(done as f32 / parts.len() as f32)
        }
    }

    fn refresh(&mut self) {
        self.index = Arc::new(self.store.as_ref().map(|store| store.read(Clone::clone)).unwrap_or_default());
        self.measured = false;
    }

    /// Stops the download under way; what came in stays for next time.
    fn halt(&mut self) {
        if let Some(active) = self.active.take() {
            active.stop.store(true, Ordering::Release);
        }
    }

    /// Whether `key` is downloading, or is an album or playlist with the track that is.
    fn busy(&self, key: Key) -> bool {
        self.active.as_ref().is_some_and(|active| {
            active.key == key || self.index.items.get(&key).is_some_and(|item| item.parts.contains(&active.key))
        })
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) {
        match message {
            Message::Usage(usage) => {
                self.usage = usage;
                return;
            },
            Message::Forget(origin) => {
                self.forget(&origin);
                self.refresh();
                self.pump(effects);
                return;
            },
            _ => {},
        }
        let Some(store) = self.store.clone() else { return };
        match message {
            Message::Download(keys) if self.ready() => {
                store.change(|index| {
                    for key in keys {
                        index.add(key, true);
                    }
                });
            },
            Message::Download(_) => {},
            Message::Cache(keys) if self.ready() => {
                store.change(|index| {
                    for (n, key) in keys.into_iter().filter(|key| key.is_track()).enumerate() {
                        index.add(key, false);
                        if n == 0
                            && let Some(item) = index.items.get_mut(&key)
                        {
                            item.used = now();
                        }
                    }
                });
                store.evict(CACHE);
            },
            Message::Cache(_) => {},
            Message::Pause(key) => {
                if self.busy(key) {
                    self.halt();
                }
                store.change(|index| set(index, key, State::Paused));
            },
            Message::Resume(key) => {
                store.change(|index| set(index, key, State::Queued));
            },
            Message::Remove(key) => {
                if self.busy(key) {
                    self.halt();
                }
                store.remove(key);
            },
            Message::PauseAll => {
                self.halt();
                store.change(|index| {
                    for item in index.items.values_mut().filter(|item| item.state == State::Queued) {
                        item.state = State::Paused;
                    }
                });
            },
            Message::ResumeAll => {
                store.change(|index| {
                    for item in
                        index.items.values_mut().filter(|item| matches!(item.state, State::Paused | State::Failed))
                    {
                        item.state = State::Queued;
                        item.error = None;
                    }
                });
            },
            Message::Progress(serial, done, total) => {
                if let Some(active) = self.active.as_mut().filter(|active| active.serial == serial) {
                    active.done = done;
                    active.total = total;
                }
                return;
            },
            Message::Finished(serial, key, result) => {
                if self.active.as_ref().is_some_and(|active| active.serial == serial) {
                    self.active = None;
                }
                store.change(|index| {
                    let Some(item) = index.items.get_mut(&key) else { return };
                    match result {
                        Ok(()) => item.state = State::Done,
                        Err(fetch::Error::Failed(why)) => {
                            item.state = State::Failed;
                            item.error = Some(why);
                        },
                        Err(fetch::Error::Unreachable) => self.waiting = true,
                        Err(fetch::Error::Stopped) => {},
                    }
                });
            },
            Message::Flushed(result) => {
                self.flushing = false;
                if result.is_ok() {
                    self.flush(effects);
                }
                return;
            },
            Message::Usage(_) | Message::Forget(_) => {},
        }
        self.refresh();
        self.pump(effects);
    }

    /// Removes everything downloaded from the server at `origin`.
    fn forget(&mut self, origin: &str) {
        let current = self.api.as_ref().is_some_and(|api| api.origin.as_str() == origin);
        if current {
            self.halt();
        }
        STORES.lock().unwrap_or_else(|e| e.into_inner()).remove(origin);
        if let Some(root) = url::Url::parse(origin).ok().as_ref().and_then(root) {
            let _ = std::fs::remove_dir_all(root);
        }
        if current {
            self.store = self.api.as_ref().and_then(|api| Offline::new(&api.origin)).map(|offline| offline.store);
        }
        self.usage.remove(origin);
    }

    /// Starts the next download, when nothing's downloading and the queue isn't waiting.
    fn pump(&mut self, effects: &mut Vec<Effect>) {
        if self.active.is_some() || self.waiting || !self.ready() {
            return;
        }
        let (Some(api), Some(store)) = (self.api.clone(), self.store.clone()) else { return };
        let Some(key) = store.read(Index::next_queued) else { return };
        self.serial += 1;
        let serial = self.serial;
        let stop = Arc::new(AtomicBool::new(false));
        self.active = Some(Active { key, done: 0, total: None, serial, stop: stop.clone() });
        effects.push(Effect::Listen(Box::new(move |send| {
            tokio::spawn(async move {
                let tell = send.clone();
                let progress = move |done, total| tell(AppMessage::Offline(Message::Progress(serial, done, total)));
                let result = download(&api, &store, key, &stop, &progress).await;
                send(AppMessage::Offline(Message::Finished(serial, key, result)));
            });
        })));
    }

    /// The server's back: send what happened while it was away, and carry on downloading.
    pub fn reconnected(&mut self, effects: &mut Vec<Effect>) {
        if let Some(offline) = self.api.as_ref().and_then(|api| Offline::new(&api.origin)) {
            offline.back();
        }
        self.waiting = false;
        self.flush(effects);
        self.pump(effects);
    }

    /// Sends what was done offline, oldest first, until it's all sent or the server's gone again.
    fn flush(&mut self, effects: &mut Vec<Effect>) {
        let (Some(api), Some(store)) = (self.api.clone(), self.store.clone()) else { return };
        let Some(first) = store.read(|index| index.reports.first().cloned()) else { return };
        if self.flushing {
            return;
        }
        self.flushing = true;
        effects.push(Effect::Task(Box::pin(async move {
            let result = api.resend(&first).await;
            if result.is_ok() {
                store.change(|index| {
                    if let Some(at) = index.reports.iter().position(|report| *report == first) {
                        index.reports.remove(at);
                    }
                });
            }
            AppMessage::Offline(Message::Flushed(result))
        })));
    }

    /// Works out what each server's downloads take up (`origins`), for Settings, unless that's
    /// known since the last change.
    pub fn measure(&mut self, origins: Vec<String>, effects: &mut Vec<Effect>) {
        if std::mem::replace(&mut self.measured, true) {
            return;
        }
        effects.push(Effect::Task(Box::pin(async move {
            let usage = tokio::task::spawn_blocking(move || {
                origins
                    .into_iter()
                    .filter_map(|origin| {
                        let root = root(&url::Url::parse(&origin).ok()?)?;
                        Some((origin, store::usage(&root.join("files"))))
                    })
                    .collect()
            })
            .await
            .unwrap_or_default();
            AppMessage::Offline(Message::Usage(usage))
        })));
    }
}

/// Sets the state of `key`, and of an album's or playlist's tracks, for pausing and resuming.
fn set(index: &mut Index, key: Key, state: State) {
    let parts = index.items.get(&key).map(|item| item.parts.clone()).unwrap_or_default();
    for key in std::iter::once(key).chain(parts) {
        if let Some(item) = index.items.get_mut(&key)
            && item.state != State::Done
        {
            item.state = state;
            item.error = None;
        }
    }
}

fn fraction(done: u64, total: Option<u64>) -> f32 {
    total.filter(|total| *total > 0).map_or(0.0, |total| (done as f64 / total as f64).clamp(0.0, 1.0) as f32)
}

/// A file a download needs, and whether it can do without it.
struct Wanted {
    path: String,
    optional: bool,
}

fn picture(value: &Value) -> Option<Wanted> {
    value.as_str().filter(|path| !path.is_empty()).map(|path| Wanted { path: path.to_owned(), optional: true })
}

/// Downloads `key`: asks the server what it is and what it needs, keeps the answers, then fetches
/// the files, the one played first (as a download) with its progress told.
async fn download(
    api: &Api,
    store: &Store,
    key: Key,
    stop: &AtomicBool,
    progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
) -> Result<(), fetch::Error> {
    let gone = || fetch::Error::Failed("It isn't on the server anymore.".into());
    let (answers, files, parts) = match key {
        Key::Video(id) => {
            let data: Value = api.request_data::<api::Playback, Value>(api::playback::Variables { id }).await?;
            let video = &data["video"];
            if !video.is_object() {
                return Err(gone());
            }
            let base = format!("api/media/{id}");
            let mut files = vec![Wanted { path: format!("{base}/file"), optional: false }];
            for track in video["media"]["subtitles"].as_array().into_iter().flatten() {
                let bitmap =
                    matches!(track["codec"].as_str(), Some("hdmv_pgs_subtitle" | "dvd_subtitle" | "pgs" | "vobsub"));
                if track["supported"] == true
                    && !bitmap
                    && let Some(track) = track["id"].as_str()
                {
                    files.push(Wanted { path: format!("{base}/subtitles/{track}"), optional: false });
                }
            }
            for font in video["media"]["fonts"].as_array().into_iter().flatten() {
                if let Some(index) = font["index"].as_i64() {
                    files.push(Wanted { path: format!("{base}/fonts/{index}"), optional: false });
                }
            }
            files.extend(
                [&video["still"], &video["title"]["backdrop"], &video["next"]["still"]].into_iter().filter_map(picture),
            );
            (vec![(store::answer_key("Playback", &json!({ "id": id })), data)], files, Vec::new())
        },
        Key::Track(id) => {
            let asked = store::answer_key("MusicTrack", &json!({ "id": id }));
            let data =
                match store.read(|index| index.items.get(&key).and_then(|item| item.answers.get(&asked)).cloned()) {
                    Some(data) => data,
                    None => api.request_data::<api::MusicTrack, Value>(api::music_track::Variables { id }).await?,
                };
            let track = &data["track"];
            if !track.is_object() {
                return Err(gone());
            }
            let direct = track["codec"].as_str().is_some_and(|codec| crate::music::player::DECODES.contains(&codec));
            let main = track[if direct { "file" } else { "flac" }].as_str().unwrap_or_default().to_owned();
            let mut files = vec![Wanted { path: main, optional: false }];
            files.extend(picture(&track["cover"]));
            let mut answers = vec![(asked, data.clone())];
            if let Ok(lyrics) =
                api.request_data::<api::MusicLyrics, Value>(api::music_lyrics::Variables { track_id: id }).await
            {
                answers.push((store::answer_key("MusicLyrics", &json!({ "trackId": id })), lyrics));
            }
            (answers, files, Vec::new())
        },
        Key::Album(id) | Key::Playlist(id) => {
            let (operation, field, data) = match key {
                Key::Album(_) => (
                    "MusicAlbum",
                    "album",
                    api.request_data::<api::MusicAlbum, Value>(api::music_album::Variables { id }).await?,
                ),
                _ => (
                    "MusicPlaylist",
                    "playlist",
                    api.request_data::<api::MusicPlaylist, Value>(api::music_playlist::Variables { id }).await?,
                ),
            };
            let list = &data[field];
            if !list.is_object() {
                return Err(gone());
            }
            let tracks: Vec<Value> = list["tracks"].as_array().cloned().unwrap_or_default();
            let parts: Vec<Key> = tracks.iter().filter_map(|t| t["id"].as_i64()).map(Key::Track).collect();
            let mut files: Vec<Wanted> = picture(&list["cover"]).into_iter().collect();
            files.extend(list["covers"].as_array().into_iter().flatten().filter_map(picture));
            store.change(|index| {
                for track in &tracks {
                    let Some(id) = track["id"].as_i64() else { continue };
                    index.add(Key::Track(id), false);
                    if let Some(item) = index.items.get_mut(&Key::Track(id)) {
                        item.answers
                            .entry(store::answer_key("MusicTrack", &json!({ "id": id })))
                            .or_insert_with(|| json!({ "track": track }));
                    }
                }
            });
            (vec![(store::answer_key(operation, &json!({ "id": id })), data)], files, parts)
        },
    };
    store.change(|index| {
        if let Some(item) = index.items.get_mut(&key) {
            item.answers.extend(answers);
            item.files = files.iter().map(|file| file.path.clone()).collect();
            item.parts = parts;
        }
    });
    let mut size = 0;
    for (n, file) in files.iter().enumerate() {
        let main = n == 0 && matches!(key, Key::Video(_) | Key::Track(_));
        let url = match (main, file.path.contains('?')) {
            (false, _) => file.path.clone(),
            (true, false) => format!("{}?download=1", file.path),
            (true, true) => format!("{}&download=1", file.path),
        };
        let tell = |done, total| {
            if main {
                progress(done, total);
            }
        };
        match fetch::fetch(api, &url, &store.part(&file.path), &store.file(&file.path), stop, tell).await {
            Ok(bytes) => size += bytes,
            Err(fetch::Error::Failed(_)) if file.optional => {},
            Err(error) => return Err(error),
        }
        if main {
            store.change(|index| {
                if let Some(item) = index.items.get_mut(&key) {
                    item.total = Some(size);
                }
            });
        }
    }
    store.change(|index| {
        if let Some(item) = index.items.get_mut(&key) {
            item.size = size;
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_made_offline_keeps_only_the_newest_position_and_plays_from_it() {
        let mut index = Index::default();
        index.add(Key::Video(7), true);
        index.items.get_mut(&Key::Video(7)).unwrap().answers.insert(
            store::answer_key("Playback", &json!({ "id": 7 })),
            json!({ "video": { "id": 7, "position": 12.0, "finished": false } }),
        );
        let save = |position: f64| {
            json!({
                "operationName": "SaveProgress",
                "query": "mutation SaveProgress",
                "variables": { "videoId": 7, "position": position, "duration": 1400.0, "at": null },
            })
        };
        report(&mut index, "SaveProgress", save(300.0), 1_000);
        report(
            &mut index,
            "MusicPlayed",
            json!({ "operationName": "MusicPlayed", "variables": { "trackId": 3 } }),
            1_005,
        );
        report(&mut index, "SaveProgress", save(1390.0), 1_010);

        assert_eq!(index.reports.len(), 2);
        assert_eq!(index.reports[0]["variables"], json!({ "trackId": 3, "at": 1_005 }));
        assert_eq!(index.reports[1]["variables"]["position"], 1390.0);
        assert_eq!(index.reports[1]["variables"]["at"], 1_010);
        let video = &index.answer(&store::answer_key("Playback", &json!({ "id": 7 }))).unwrap()["video"];
        assert_eq!(video["position"], 1390.0);
        assert_eq!(video["finished"], true);
    }

    #[test]
    fn an_album_is_done_when_its_tracks_are() {
        let mut downloads = Downloads::default();
        let mut index = Index::default();
        index.add(Key::Album(1), true);
        index.add(Key::Track(1), false);
        index.add(Key::Track(2), false);
        let album = index.items.get_mut(&Key::Album(1)).unwrap();
        album.parts = vec![Key::Track(1), Key::Track(2)];
        album.state = State::Done;
        index.items.get_mut(&Key::Track(1)).unwrap().state = State::Done;
        downloads.index = Arc::new(index.clone());
        assert_eq!(downloads.status(Key::Album(1)), Status::Downloading(0.5));
        assert_eq!(downloads.status(Key::Video(1)), Status::Missing);

        index.items.get_mut(&Key::Track(2)).unwrap().state = State::Paused;
        downloads.index = Arc::new(index.clone());
        assert_eq!(downloads.status(Key::Album(1)), Status::Paused);

        set(&mut index, Key::Album(1), State::Queued);
        assert_eq!(index.items[&Key::Track(1)].state, State::Done);
        assert_eq!(index.items[&Key::Track(2)].state, State::Queued);
        index.items.get_mut(&Key::Track(2)).unwrap().state = State::Done;
        downloads.index = Arc::new(index);
        assert_eq!(downloads.status(Key::Album(1)), Status::Done);
    }
}
