// SPDX-License-Identifier: AGPL-3.0-or-later
mod engine;
mod listen;
pub mod model;
pub(crate) mod output;
mod ring;
mod view;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use graphql_client::GraphQLQuery;
use model::{Entry, Repeat, Settings, Track};
use serde::Deserialize;
use serde_json::{Value, json};
pub use view::{page, pill};

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};
use crate::route::Route;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Card {
    pub id: i64,
    pub name: String,
    pub artist: String,
    pub cover: Option<String>,
    pub covers: Vec<String>,
    pub tracks: Vec<Track>,
    pub albums: Vec<Card>,
    pub genres: Vec<String>,
    pub comment: Option<String>,
    pub mine: bool,
    pub public: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Tab {
    #[default]
    Albums,
    Artists,
    Songs,
    Playlists,
}
#[derive(Clone)]
pub enum Message {
    Picture(String, Option<iced_widget::image::Handle>),
    RoomChanged,
    CopyRoom,
    Edited(Result<Value, api::Error>),
    Remote(i64, f64, Result<Value, api::Error>),
    Deleted(Result<Value, api::Error>),
    PlaylistMove(usize, i32),
    PlaylistRemove(usize),
    PlaylistPublic,
    AskDelete(i64),
    Measured(i64, Result<Value, api::Error>),
    RoomCommand(Value),
    JoinRoom,
    RoomStarted(Result<Value, api::Error>),
    Ready(Arc<Mutex<engine::Engine>>),
    Audio(engine::Update),
    Loaded(String, Result<Value, api::Error>),
    Restored(Result<Value, api::Error>),
    Done(Result<Value, api::Error>),
    Tab(Tab),
    Query(String),
    More,
    Play(Vec<Track>, usize),
    Add(Arc<Track>, bool),
    Toggle,
    Pause,
    Next,
    Previous,
    Jump(usize),
    Seek(f64),
    Remove(usize),
    Move(usize, i32),
    Shuffle,
    Repeat,
    Volume(f32),
    Gain(model::Gain),
    Fade(f64),
    Dismiss,
    Expand,
    Fullscreen,
    Panel(u8),
    Star(i64, bool),
    Rating(i64, i64),
    Name(String),
    CreatePlaylist,
    DeletePlaylist(i64),
    SavePlaylist(i64),
    Go(Route),
    StartRoom,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MusicMessage")
    }
}

pub struct Music {
    pub images: crate::images::Images,
    pub people: Vec<(i64, String)>,
    pub can_invite: bool,
    pub delete: Option<i64>,
    measured: std::collections::HashSet<i64>,
    pub generation: u64,
    pub room_code: Option<String>,
    room: Option<listen::Client>,
    pub snapshot: listen::Snapshot,
    room_ids: Vec<i64>,
    room_slot: Option<Arc<Mutex<Option<listen::Client>>>>,
    room_started: bool,
    pub guest_name: String,
    pub room_joined: bool,
    api: Option<Api>,
    engine: Option<Arc<Mutex<engine::Engine>>>,
    pub queue: Vec<Entry>,
    pub index: usize,
    pub position: f64,
    pub playing: bool,
    pub waiting: bool,
    pub rate: u32,
    pub settings: Settings,
    pub repeat: Repeat,
    pub shuffled: bool,
    unshuffled: Option<Vec<Entry>>,
    pub dismissed: bool,
    pub suspended: bool,
    pub blocked: bool,
    pub expanded: bool,
    pub fullscreen: bool,
    pub panel: u8,
    pub tab: Tab,
    pub query: String,
    pub name: String,
    pub cards: Vec<Card>,
    pub songs: Vec<Track>,
    pub detail: Option<Card>,
    pub error: Option<String>,
    pub loading: bool,
    pub more: bool,
    pub lyrics: Vec<(Option<i64>, String)>,
    route: Option<Route>,
    library: bool,
    key: String,
    offset: usize,
    uid: u64,
    saved: Instant,
}
impl Default for Music {
    fn default() -> Self {
        Self {
            images: crate::images::Images::default(),
            people: vec![],
            can_invite: false,
            delete: None,
            measured: std::collections::HashSet::new(),
            generation: 0,
            room_code: None,
            room: None,
            snapshot: listen::Snapshot::default(),
            room_ids: vec![],
            room_slot: None,
            room_started: false,
            guest_name: crate::player::room::guest_name().unwrap_or_default(),
            room_joined: false,
            api: None,
            engine: None,
            queue: vec![],
            index: 0,
            position: 0.0,
            playing: false,
            waiting: false,
            rate: 48000,
            settings: Settings::default(),
            repeat: Repeat::Off,
            shuffled: false,
            unshuffled: None,
            dismissed: false,
            suspended: false,
            blocked: false,
            expanded: false,
            fullscreen: false,
            panel: 0,
            tab: Tab::Albums,
            query: String::new(),
            name: String::new(),
            cards: vec![],
            songs: vec![],
            detail: None,
            error: None,
            loading: false,
            more: false,
            lyrics: vec![],
            route: None,
            library: false,
            key: String::new(),
            offset: 0,
            uid: 0,
            saved: Instant::now(),
        }
    }
}
impl Music {
    pub fn current(&self) -> Option<&Entry> {
        self.queue.get(self.index)
    }

    pub fn owns(&self, route: &Route) -> bool {
        matches!(route, Route::Album(_) | Route::Artist(_) | Route::Playlist(_) | Route::Listen(_))
            || self.library && self.route.as_ref() == Some(route) && matches!(route, Route::Library(_))
    }

    pub fn follow(&mut self, generation: u64, route: &Route, api: &Api, library: bool, effects: &mut Vec<Effect>) {
        if self.api.is_none()
            || self.generation != generation
            || self.api.as_ref().is_some_and(|old| old.origin != api.origin)
        {
            self.save(effects);
            let mut next = Self::default();
            next.generation = generation;
            next.api = Some(api.clone());
            *self = next;
            if let Some(path) = crate::paths::config_dir().map(|d| d.join("music-settings")) {
                self.settings =
                    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            }
            let signed_in = api.token.is_some();
            let api = api.clone();
            effects.push(Effect::Listen(Box::new(move |send| {
                let notify = send.clone();
                let engine = engine::Engine::new(
                    api,
                    tokio::runtime::Handle::current(),
                    Arc::new(move |update| notify(AppMessage::Music(generation, Message::Audio(update)))),
                );
                send(AppMessage::Music(generation, Message::Ready(Arc::new(Mutex::new(engine)))));
            })));
            if signed_in {
                self.restore(effects);
            }
        }
        if !matches!(route, Route::Listen(_)) && self.room_code.is_some() {
            if let Some(room) = self.room.take() {
                room.close();
            }
            self.room_code = None;
            self.room_ids.clear();
            self.queue.clear();
            self.playing = false;
            if let Some(engine) = &self.engine {
                engine.lock().unwrap().stop();
            }
            self.restore(effects);
        }
        self.library = library;
        let suspended = self.blocked || matches!(route, Route::Watch(..) | Route::Together(_) | Route::Clip(_));
        if suspended && !self.suspended {
            self.pause(effects);
        }
        self.suspended = suspended;
        if self.route.as_ref() != Some(route) {
            self.route = Some(route.clone());
            self.offset = 0;
            self.detail = None;
            self.cards.clear();
            self.songs.clear();
            if let Route::Listen(code) = route {
                self.begin_room(code.clone(), effects);
            }
            if library || matches!(route, Route::Album(_) | Route::Artist(_) | Route::Playlist(_)) {
                self.fetch(effects);
            }
        }
    }

    fn request<Q: GraphQLQuery + 'static>(
        &self,
        variables: Q::Variables,
        done: impl FnOnce(Result<Value, api::Error>) -> Message + Send + 'static,
        effects: &mut Vec<Effect>,
    ) where
        Q::Variables: Send,
    {
        let Some(api) = self.api.clone() else { return };
        let generation = self.generation;
        effects.push(Effect::Request(Box::pin(async move {
            AppMessage::Music(generation, done(api.request_data::<Q, Value>(variables).await))
        })));
    }

    pub fn restore(&self, effects: &mut Vec<Effect>) {
        self.request::<api::MusicQueue>(api::music_queue::Variables {}, Message::Restored, effects);
    }

    fn fetch(&mut self, effects: &mut Vec<Effect>) {
        self.loading = true;
        self.error = None;
        self.key = format!("{:?}:{:?}:{}:{}", self.route, self.tab, self.query, self.offset);
        let key = self.key.clone();
        let done = move |r| Message::Loaded(key, r);
        match self.route.clone() {
            Some(Route::Album(id)) => {
                self.request::<api::MusicAlbum>(api::music_album::Variables { id: id as i64 }, done, effects)
            },
            Some(Route::Artist(id)) => {
                self.request::<api::MusicArtist>(api::music_artist::Variables { id: id as i64 }, done, effects)
            },
            Some(Route::Playlist(id)) => {
                self.request::<api::MusicPlaylist>(api::music_playlist::Variables { id: id as i64 }, done, effects)
            },
            Some(Route::Library(library)) => match self.tab {
                Tab::Albums => self.request::<api::MusicAlbums>(
                    api::music_albums::Variables {
                        library: Some(library),
                        sort: api::music_albums::AlbumSort::NAME,
                        offset: self.offset as i64,
                    },
                    done,
                    effects,
                ),
                Tab::Artists => self.request::<api::MusicArtists>(
                    api::music_artists::Variables { library: Some(library) },
                    done,
                    effects,
                ),
                Tab::Songs => self.request::<api::MusicSongs>(
                    api::music_songs::Variables {
                        library: Some(library),
                        query: self.query.clone(),
                        offset: self.offset as i64,
                    },
                    done,
                    effects,
                ),
                Tab::Playlists => {
                    self.request::<api::MusicPlaylists>(api::music_playlists::Variables {}, done, effects)
                },
            },
            _ => {},
        }
    }

    fn entries(&mut self, tracks: Vec<Track>) -> Vec<Entry> {
        tracks
            .into_iter()
            .map(|track| {
                self.uid += 1;
                Entry { key: format!("e{}", self.uid), track, fallback: false }
            })
            .collect()
    }

    fn start(&mut self, index: usize, time: f64, playing: bool, effects: &mut Vec<Effect>) {
        if self.queue.is_empty() {
            return;
        }
        self.index = index.min(self.queue.len() - 1);
        self.position = time.max(0.0);
        self.playing = playing;
        self.waiting = true;
        self.dismissed = false;
        self.error = None;
        let mut entries = self.queue[self.index..].to_vec();
        if self.repeat == Repeat::One {
            entries = vec![self.queue[self.index].clone()];
        } else if self.repeat == Repeat::All {
            entries.extend(self.queue[..self.index].to_vec());
        }
        if let Some(engine) = &self.engine {
            let mut settings = self.settings.clone();
            if matches!(settings.gain, model::Gain::Auto) {
                if self.shuffled {
                    settings.gain = model::Gain::Track;
                } else if self.index > 0
                    && self.queue[self.index].track.album_id.is_some()
                    && self.queue[self.index - 1].track.album_id == self.queue[self.index].track.album_id
                    && let Some(first) = entries.first_mut()
                {
                    first.track.gains.track_gain = first.track.gains.album_gain.or(first.track.gains.track_gain);
                    first.track.gains.track_peak = first.track.gains.album_peak.or(first.track.gains.track_peak);
                }
            }
            engine.lock().unwrap().load(entries, settings, self.position, playing, self.repeat);
        }
        for entry in self.queue.iter().skip(self.index).take(3) {
            if entry.track.gains.pending && self.measured.insert(entry.track.id) {
                let id = entry.track.id;
                self.request::<api::MusicMeasure>(
                    api::music_measure::Variables { track_id: id },
                    move |r| Message::Measured(id, r),
                    effects,
                );
            }
        }
        self.save(effects);
    }

    fn pause(&mut self, effects: &mut Vec<Effect>) {
        self.playing = false;
        if let Some(engine) = &self.engine {
            engine.lock().unwrap().playing(false);
        }
        self.save(effects);
    }

    pub fn save(&self, effects: &mut Vec<Effect>) {
        if self.queue.is_empty() || self.room_code.is_some() {
            return;
        }
        let input = api::music_save::QueueInput {
            tracks: self.queue.iter().map(|e| e.track.id).collect(),
            current: self.index as i64,
            position: self.position,
            shuffled: self.shuffled,
            repeat: match self.repeat {
                Repeat::Off => api::music_save::Repeat::OFF,
                Repeat::All => api::music_save::Repeat::ALL,
                Repeat::One => api::music_save::Repeat::ONE,
            },
        };
        self.request::<api::MusicSave>(api::music_save::Variables { input }, Message::Done, effects);
        self.request::<api::MusicPlaying>(
            api::music_playing::Variables {
                track_id: self.current().map(|e| e.track.id),
                position: self.position,
                paused: !self.playing,
            },
            Message::Done,
            effects,
        );
    }

    fn changed_queue(&mut self, effects: &mut Vec<Effect>) {
        if self.playing {
            self.start(self.index, self.position, true, effects);
        } else {
            self.save(effects);
        }
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) -> Option<Route> {
        if self.room_code.is_some() {
            let outgoing = match &message {
                Message::Toggle => {
                    Some(json!({"type": if self.playing {"pause"} else {"play"},"position":self.position}))
                },
                Message::Pause => Some(json!({"type":"pause","position":self.position})),
                Message::Seek(position) => Some(json!({"type":"seek","position":position})),
                Message::Next => Some(json!({"type":"skip","index":self.index+1,"from":self.index})),
                Message::Previous => {
                    Some(json!({"type":"skip","index":self.index.saturating_sub(1),"from":self.index}))
                },
                Message::Jump(index) => Some(json!({"type":"skip","index":index,"from":self.index})),
                Message::Remove(index) => Some(json!({"type":"remove","index":index})),
                Message::Move(from, by) => Some(json!({"type":"move","from":from,"to":(*from as i32+by).max(0)})),
                Message::Add(track, next) => Some(json!({"type":"add","tracks":[track.id],"next":next})),
                _ => None,
            };
            if let Some(outgoing) = outgoing {
                if !self.snapshot.control() {
                    self.error = Some("Only the host can control this room".into());
                    return None;
                }
                if let Some(room) = &self.room {
                    room.send(outgoing);
                }
                return None;
            }
        }
        match message {
            Message::Remote(id, time, result) => {
                if self.room_code.is_none()
                    && let Ok(value) = result
                    && let Ok(track) = serde_json::from_value::<Track>(value["track"].clone())
                    && track.id == id
                {
                    if let Some(engine) = &self.engine {
                        engine.lock().unwrap().stop();
                    }
                    self.queue = self.entries(vec![track]);
                    self.index = 0;
                    self.position = time;
                    self.playing = false;
                    self.waiting = false;
                    self.dismissed = false;
                }
            },
            Message::Edited(result) => match result {
                Ok(_) => self.fetch(effects),
                Err(e) => self.error = Some(e.message),
            },
            Message::Deleted(result) => match result {
                Ok(_) => return Some(Route::Home),
                Err(e) => self.error = Some(e.message),
            },
            Message::PlaylistMove(index, by) => {
                let to = (index as i32 + by).max(0) as usize;
                if index < self.songs.len() && to < self.songs.len() {
                    let track = self.songs.remove(index);
                    self.songs.insert(to, track);
                }
            },
            Message::PlaylistRemove(index) => {
                if index < self.songs.len() {
                    self.songs.remove(index);
                }
            },
            Message::PlaylistPublic => {
                if let Some(detail) = &mut self.detail {
                    detail.public = !detail.public;
                }
            },
            Message::Measured(id, result) => {
                if let Ok(value) = result
                    && let Ok(track) = serde_json::from_value::<Track>(value["measureLoudness"].clone())
                {
                    for e in &mut self.queue {
                        if e.track.id == id {
                            e.track = track.clone();
                        }
                    }
                }
                if let Some(engine) = &self.engine {
                    for (i, e) in self.queue.iter().enumerate() {
                        if e.track.id == id {
                            engine
                                .lock()
                                .unwrap()
                                .gain(e.key.clone(), model::gain(i, &self.queue, &self.settings, self.shuffled));
                        }
                    }
                }
            },
            Message::AskDelete(id) => self.delete = Some(id),
            Message::CopyRoom => {
                if let (Some(api), Some(code)) = (&self.api, &self.room_code)
                    && let Ok(url) = api.origin.join(&format!("listen/{code}"))
                {
                    effects.push(Effect::Copy(url.to_string()));
                }
            },
            Message::Picture(path, handle) => {
                self.images.arrived(path, handle, true);
                self.images.limit_ready(256);
            },
            Message::RoomCommand(value) => {
                if let Some(room) = &self.room {
                    room.send(value);
                }
            },
            Message::JoinRoom => self.connect_room(effects),
            Message::RoomChanged => self.sync_room(effects),
            Message::RoomStarted(result) => match result {
                Ok(value) => {
                    if let Some(code) = value["startListenRoom"]["code"].as_str() {
                        return Some(Route::Listen(code.into()));
                    }
                },
                Err(e) => self.error = Some(e.message),
            },
            Message::Ready(engine) => {
                self.engine = Some(engine);
                if self.playing {
                    self.start(self.index, self.position, true, effects);
                }
            },
            Message::Loaded(key, result) => {
                if key == "people" {
                    if let Ok(value) = result {
                        self.people = value["users"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| Some((v["id"].as_i64()?, v["username"].as_str()?.to_owned())))
                            .collect();
                    }
                    return None;
                }
                if key == "room-info" {
                    self.loading = false;
                    match result {
                        Ok(value) => {
                            let info = &value["listenRoom"];
                            self.can_invite = info["canInvite"].as_bool().unwrap_or(false);
                            if self.can_invite {
                                self.request::<api::MusicPeople>(
                                    api::music_people::Variables {},
                                    |r| Message::Loaded("people".into(), r),
                                    effects,
                                );
                            }
                            let tracks: Vec<Track> = serde_json::from_value(info["tracks"].clone()).unwrap_or_default();
                            self.room_started = false;
                            self.room_ids = tracks.iter().map(|t| t.id).collect();
                            self.queue = self.entries(tracks);
                            if info["signedIn"].as_bool() == Some(true) || self.room_joined {
                                self.connect_room(effects);
                            }
                            self.sync_room(effects);
                        },
                        Err(e) => self.error = Some(e.message),
                    }
                    return None;
                }
                if key == "lyrics" {
                    if let Ok(value) = result {
                        self.lyrics = value["lyrics"]["lines"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|l| (l["start"].as_i64(), l["text"].as_str().unwrap_or("").to_string()))
                            .collect();
                    }
                    return None;
                }
                if key != self.key {
                    return None;
                }
                self.loading = false;
                match result {
                    Ok(value) => {
                        if let Some(detail) =
                            value.get("album").or_else(|| value.get("artist")).or_else(|| value.get("playlist"))
                        {
                            self.detail = serde_json::from_value(detail.clone()).ok();
                            self.songs = self.detail.as_ref().map_or(vec![], |d| d.tracks.clone());
                            self.cards = self.detail.as_ref().map_or(vec![], |d| d.albums.clone());
                            self.name = self.detail.as_ref().map_or(String::new(), |d| d.name.clone());
                        } else if let Some(songs) = value.get("songs") {
                            let next: Vec<Track> = serde_json::from_value(songs.clone()).unwrap_or_default();
                            self.more = next.len() == 60;
                            if self.offset == 0 {
                                self.songs.clear();
                            }
                            self.songs.extend(next);
                        } else if let Some(rows) =
                            value.get("albums").or_else(|| value.get("artists")).or_else(|| value.get("playlists"))
                        {
                            let next: Vec<Card> = serde_json::from_value(rows.clone()).unwrap_or_default();
                            self.more = next.len() == 60 && self.tab == Tab::Albums;
                            if self.offset == 0 {
                                self.cards.clear();
                            }
                            self.cards.extend(next);
                        }
                    },
                    Err(error) => self.error = Some(error.message),
                }
            },
            Message::Restored(result) => {
                if !self.playing && self.room_code.is_none() {
                    match result {
                        Ok(value) => {
                            let q = &value["playQueue"];
                            let tracks = serde_json::from_value(q["tracks"].clone()).unwrap_or_default();
                            self.queue = self.entries(tracks);
                            self.index = q["current"].as_u64().unwrap_or(0) as usize;
                            self.index = self.index.min(self.queue.len().saturating_sub(1));
                            self.position = q["position"].as_f64().unwrap_or(0.0);
                            self.repeat = serde_json::from_value(q["repeat"].clone()).unwrap_or_default();
                            self.shuffled = q["shuffled"].as_bool().unwrap_or(false);
                        },
                        Err(error) => self.error = Some(error.message),
                    }
                }
            },
            Message::Done(Err(error)) => self.error = Some(error.message),
            Message::Done(Ok(_)) => {},
            Message::Audio(update) => match update {
                engine::Update::Reload => self.start(self.index, self.position, self.playing, effects),
                engine::Update::Position { key, time, rate } => {
                    if self.current().is_some_and(|e| e.key == key.split('@').next().unwrap_or(&key)) {
                        self.position = time;
                        self.rate = rate;
                        if self.saved.elapsed().as_secs() >= 30 {
                            self.saved = Instant::now();
                            self.save(effects);
                        }
                    }
                },
                engine::Update::Started(key) => {
                    self.waiting = false;
                    if let Some(i) = self.queue.iter().position(|e| e.key == key.split('@').next().unwrap_or(&key)) {
                        self.index = i;
                        self.position = 0.0;
                        let id = self.queue[i].track.id;
                        self.request::<api::MusicPlayed>(
                            api::music_played::Variables { track_id: id },
                            Message::Done,
                            effects,
                        );
                        self.request::<api::MusicLyrics>(
                            api::music_lyrics::Variables { track_id: id },
                            |r| Message::Loaded("lyrics".into(), r),
                            effects,
                        );
                    }
                },
                engine::Update::Waiting(value) => {
                    self.waiting = value;
                    if let Some(room) = &self.room {
                        room.send(json!({"type":"status","status":if value {"buffering"} else {"ready"}}));
                    }
                },
                engine::Update::Ended => {
                    self.playing = false;
                    if self.repeat != Repeat::Off {
                        self.start(if self.repeat == Repeat::One { self.index } else { 0 }, 0.0, true, effects);
                    } else {
                        self.save(effects);
                    }
                },
                engine::Update::Error(key, error) => {
                    if let Some(i) = self.queue.iter().position(|e| e.key == key && !e.fallback) {
                        self.queue[i].fallback = true;
                        self.start(self.index, self.position, self.playing, effects);
                    } else {
                        self.error = Some(error);
                        self.pause(effects);
                    }
                },
            },
            Message::Tab(tab) => {
                self.tab = tab;
                self.offset = 0;
                self.fetch(effects);
            },
            Message::Query(query) => {
                self.query = query;
                self.offset = 0;
                self.fetch(effects);
            },
            Message::More => {
                if !self.loading {
                    self.offset += 60;
                    self.fetch(effects);
                }
            },
            Message::Play(tracks, index) => {
                self.queue = self.entries(tracks);
                self.shuffled = false;
                self.unshuffled = None;
                self.start(index, 0.0, true, effects);
            },
            Message::Add(track, next) => {
                let entries = self.entries(vec![(*track).clone()]);
                let at = if next { (self.index + 1).min(self.queue.len()) } else { self.queue.len() };
                self.queue.splice(at..at, entries);
                if self.playing {
                    if let (Some(engine), Some(current)) = (&self.engine, self.current()) {
                        engine.lock().unwrap().upcoming(
                            current.key.clone(),
                            self.queue[self.index + 1..].to_vec(),
                            self.settings.clone(),
                        );
                    }
                    self.save(effects);
                } else {
                    self.changed_queue(effects);
                }
            },
            Message::Toggle => {
                if self.playing {
                    self.pause(effects)
                } else {
                    self.start(self.index, self.position, true, effects);
                }
            },
            Message::Pause => self.pause(effects),
            Message::Next => {
                if self.index + 1 < self.queue.len() {
                    self.start(self.index + 1, 0.0, true, effects);
                } else if self.repeat == Repeat::All {
                    self.start(0, 0.0, true, effects);
                }
            },
            Message::Previous => self.start(
                if self.position > 3.0 { self.index } else { self.index.saturating_sub(1) },
                0.0,
                true,
                effects,
            ),
            Message::Jump(index) => self.start(index, 0.0, true, effects),
            Message::Seek(time) => self.start(self.index, time, self.playing, effects),
            Message::Remove(index) => {
                if index < self.queue.len() {
                    self.queue.remove(index);
                    if index < self.index {
                        self.index -= 1;
                    }
                    self.index = self.index.min(self.queue.len().saturating_sub(1));
                    if self.queue.is_empty() {
                        self.playing = false;
                        if let Some(e) = &self.engine {
                            e.lock().unwrap().stop();
                        }
                    } else {
                        self.changed_queue(effects);
                    }
                }
            },
            Message::Move(index, by) => {
                let to = (index as i32 + by).max(0) as usize;
                if index < self.queue.len() && to < self.queue.len() {
                    let key = self.current().map(|e| e.key.clone());
                    let entry = self.queue.remove(index);
                    self.queue.insert(to, entry);
                    self.index = key.and_then(|k| self.queue.iter().position(|e| e.key == k)).unwrap_or(0);
                    self.changed_queue(effects);
                }
            },
            Message::Shuffle => {
                if let Some(original) = self.unshuffled.take() {
                    let key = self.current().map(|e| e.key.clone());
                    self.queue = original;
                    self.index = key.and_then(|k| self.queue.iter().position(|e| e.key == k)).unwrap_or(0);
                    self.shuffled = false;
                } else {
                    self.unshuffled = Some(self.queue.clone());
                    shuffle(&mut self.queue, self.index);
                    self.index = 0;
                    self.shuffled = true;
                }
                self.changed_queue(effects);
            },
            Message::Repeat => {
                self.repeat = match self.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                };
                self.changed_queue(effects);
            },
            Message::Volume(volume) => {
                self.settings.volume = volume;
                self.settings_changed();
            },
            Message::Gain(gain) => {
                self.settings.gain = gain;
                self.settings_changed();
                self.changed_queue(effects);
            },
            Message::Fade(fade) => {
                self.settings.crossfade = fade;
                self.settings_changed();
                self.changed_queue(effects);
            },
            Message::Dismiss => {
                if !self.playing {
                    self.dismissed = true;
                    self.fullscreen = false;
                }
            },
            Message::Expand => {
                self.expanded = !self.expanded;
                self.fullscreen = false;
            },
            Message::Fullscreen => {
                self.expanded = true;
                self.fullscreen = !self.fullscreen;
            },
            Message::Panel(panel) => self.panel = panel,
            Message::Star(id, starred) => {
                for e in &mut self.queue {
                    if e.track.id == id {
                        e.track.starred = starred;
                    }
                }
                self.request::<api::MusicStar>(
                    api::music_star::Variables { kind: api::music_star::MusicKind::TRACK, id, starred },
                    Message::Done,
                    effects,
                );
            },
            Message::Rating(id, rating) => self.request::<api::MusicRate>(
                api::music_rate::Variables { kind: api::music_rate::MusicKind::TRACK, id, rating },
                Message::Done,
                effects,
            ),
            Message::Name(name) => {
                self.name = name.clone();
                self.guest_name = name;
            },
            Message::CreatePlaylist => self.request::<api::MusicCreatePlaylist>(
                api::music_create_playlist::Variables { name: self.name.clone() },
                Message::Edited,
                effects,
            ),
            Message::DeletePlaylist(id) => self.request::<api::MusicDeletePlaylist>(
                api::music_delete_playlist::Variables { id },
                Message::Deleted,
                effects,
            ),
            Message::SavePlaylist(id) => self.request::<api::MusicUpdatePlaylist>(
                api::music_update_playlist::Variables {
                    id,
                    input: api::music_update_playlist::PlaylistInput {
                        name: Some(self.name.clone()),
                        comment: None,
                        public: self.detail.as_ref().map(|d| d.public),
                        tracks: Some(self.songs.iter().map(|t| t.id).collect()),
                    },
                },
                Message::Edited,
                effects,
            ),
            Message::Go(route) => return Some(route),
            Message::StartRoom => {
                let tracks = if self.queue.is_empty() {
                    self.songs.iter().map(|t| t.id).collect()
                } else {
                    self.queue.iter().map(|e| e.track.id).collect()
                };
                self.request::<api::MusicStartRoom>(
                    api::music_start_room::Variables {
                        input: api::music_start_room::NewListenRoom {
                            tracks,
                            current: self.index as i64,
                            position: self.position,
                            paused: true,
                            public: false,
                        },
                    },
                    Message::RoomStarted,
                    effects,
                );
            },
        }
        self.fetch_art(effects);
        None
    }

    pub fn live(&mut self, live: &crate::api::Live, effects: &mut Vec<Effect>) {
        if self.room_code.is_some() {
            return;
        }
        match live {
            crate::api::Live::Connected => {
                if !self.playing {
                    self.restore(effects);
                }
            },
            crate::api::Live::Event(event) => match &event.events {
                api::events::EventsEvents::LibraryChanged(_) | api::events::EventsEvents::ScanFinished(_) => {
                    if self.route.as_ref().is_some_and(|r| self.owns(r)) {
                        self.offset = 0;
                        self.fetch(effects);
                    }
                },
                api::events::EventsEvents::ListChanged(change) => {
                    if matches!(change.list, api::events::ChangedList::PLAYLISTS)
                        && (self.tab == Tab::Playlists || matches!(self.route, Some(Route::Playlist(_))))
                    {
                        self.offset = 0;
                        self.fetch(effects);
                    }
                },
                api::events::EventsEvents::QueueChanged(change) => {
                    if change.by.as_deref() != Some("Desktop") && !self.playing {
                        self.restore(effects);
                    }
                },
                api::events::EventsEvents::PlaybackChanged(change) => {
                    if change.client != "Desktop"
                        && (!self.playing || !change.paused)
                        && let Some(id) = change.track_id
                    {
                        let time = change.position;
                        self.request::<api::MusicTrack>(
                            api::music_track::Variables { id },
                            move |r| Message::Remote(id, time, r),
                            effects,
                        );
                    }
                },
                _ => {},
            },
            _ => {},
        }
    }

    fn fetch_art(&mut self, effects: &mut Vec<Effect>) {
        let paths: Vec<String> = self
            .cards
            .iter()
            .filter_map(|c| c.cover.clone().or_else(|| c.covers.first().cloned()))
            .chain(self.detail.as_ref().and_then(|d| d.cover.clone()))
            .chain(self.current().and_then(|e| e.track.cover.clone()))
            .collect();
        for path in paths {
            if self.images.want(&path)
                && let Some(api) = self.api.clone()
            {
                let generation = self.generation;
                effects.push(Effect::Request(Box::pin(async move {
                    let handle = match crate::images::fetch(&api, &path).await {
                        Some(bytes) => crate::images::decode(bytes).await,
                        None => None,
                    };
                    AppMessage::Music(generation, Message::Picture(path, handle))
                })));
            }
        }
    }

    pub fn desktop_media(&self) -> crate::platform::integration::Media {
        let Some(current) = self.current().filter(|_| !self.suspended) else {
            return Default::default();
        };
        crate::platform::integration::Media {
            track: format!("/dev/tinystream/music/{}", current.track.id),
            title: current.track.title.clone(),
            artist: current.track.artist.clone(),
            album: current.track.album.clone(),
            art: self
                .api
                .as_ref()
                .and_then(|a| current.track.cover.as_ref().and_then(|p| a.origin.join(p).ok()))
                .map_or(String::new(), |u| u.to_string()),
            length: current.track.duration,
            position: self.position,
            playing: self.playing,
            rate: 1.0,
            volume: self.settings.volume as f64,
            next: self.index + 1 < self.queue.len(),
            previous: self.index > 0,
            seek: true,
            discontinuities: 0,
            tray: false,
        }
    }

    fn begin_room(&mut self, code: String, effects: &mut Vec<Effect>) {
        self.save(effects);
        self.pause(effects);
        if let Some(room) = self.room.take() {
            room.close();
        }
        self.room_started = false;
        self.room_code = Some(code.clone());
        self.room_joined = false;
        self.loading = true;
        self.expanded = true;
        self.snapshot = listen::Snapshot::default();
        self.request::<api::MusicRoom>(
            api::music_room::Variables { code },
            |r| Message::Loaded("room-info".into(), r),
            effects,
        );
    }

    fn connect_room(&mut self, effects: &mut Vec<Effect>) {
        if self.room.is_some() {
            return;
        }
        let (Some(api), Some(code)) = (self.api.clone(), self.room_code.clone()) else {
            return;
        };
        if api.token.is_none() && self.guest_name.trim().is_empty() {
            self.error = Some("Enter a name to join".into());
            return;
        }
        self.room_joined = true;
        let generation = self.generation;
        let name = (api.token.is_none()).then(|| self.guest_name.clone());
        let slot = Arc::new(Mutex::new(None));
        let receive = slot.clone();
        effects.push(Effect::Listen(Box::new(move |send| {
            let notify = send.clone();
            let client = listen::Client::connect(
                api,
                code,
                name,
                Arc::new(move || notify(AppMessage::Music(generation, Message::RoomChanged))),
            );
            *slot.lock().unwrap() = Some(client);
            send(AppMessage::Music(generation, Message::RoomChanged));
        })));
        self.room_slot = Some(receive);
    }

    fn sync_room(&mut self, effects: &mut Vec<Effect>) {
        if self.room.is_none()
            && let Some(slot) = &self.room_slot
        {
            self.room = slot.lock().unwrap().take();
        }
        let Some(room) = &self.room else {
            return;
        };
        self.snapshot = room.snapshot();
        if self.snapshot.ended {
            self.pause(effects);
            self.error = Some("This listening room has ended".into());
            return;
        }
        let Some(state) = &self.snapshot.room else {
            return;
        };
        if state.queue != self.room_ids {
            if !self.loading {
                self.loading = true;
                let code = self.room_code.clone().unwrap_or_default();
                self.request::<api::MusicRoom>(
                    api::music_room::Variables { code },
                    |r| Message::Loaded("room-info".into(), r),
                    effects,
                );
            }
            return;
        }
        let Some((index, time, playing)) = self.snapshot.target() else {
            return;
        };
        let changed = index != self.index || (self.position - time).abs() > if playing { 0.35 } else { 1.0 };
        if changed || self.engine.is_some() && self.current().is_some() && !self.room_started {
            self.room_started = true;
            self.start(index, time, playing, effects);
        } else if playing != self.playing {
            self.playing = playing;
            if let Some(engine) = &self.engine {
                engine.lock().unwrap().playing(playing);
            }
        }
        if let Some(room) = &self.room {
            room.send(json!({"type":"status","status":if self.waiting && self.playing{"buffering"}else{"ready"}}));
        }
    }

    fn settings_changed(&self) {
        if let Some(e) = &self.engine {
            e.lock().unwrap().settings(self.settings.clone());
        }
        if let Some(dir) = crate::paths::config_dir() {
            let _ = std::fs::create_dir_all(&dir);
            if let Ok(bytes) = serde_json::to_vec(&self.settings) {
                let _ = std::fs::write(dir.join("music-settings"), bytes);
            }
        }
    }
}
fn shuffle(queue: &mut Vec<Entry>, index: usize) {
    if queue.is_empty() {
        return;
    }
    let current = queue.remove(index.min(queue.len() - 1));
    for i in (1..queue.len()).rev() {
        let mut bytes = [0; 8];
        if getrandom::fill(&mut bytes).is_ok() {
            queue.swap(i, (u64::from_le_bytes(bytes) % (i as u64 + 1)) as usize);
        }
    }
    queue.insert(0, current);
}

impl Drop for Music {
    fn drop(&mut self) {
        if let Some(room) = &self.room {
            room.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tracks() -> Vec<Track> {
        (1..=3).map(|id| Track { id, title: format!("Track {id}"), ..Track::default() }).collect()
    }
    #[test]
    fn queue_edits_and_shuffle_preserve_the_audible_track() {
        let mut music = Music::default();
        let mut effects = vec![];
        music.update(Message::Play(tracks(), 1), &mut effects);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Shuffle, &mut effects);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Shuffle, &mut effects);
        assert_eq!(music.queue.iter().map(|e| e.track.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Remove(0), &mut effects);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Seek(42.0), &mut effects);
        music.update(Message::Pause, &mut effects);
        assert!(!music.playing);
        assert_eq!(music.position, 42.0);
        music.update(Message::Dismiss, &mut effects);
        assert!(music.dismissed);
    }
    #[test]
    fn server_queue_restores_paused_without_starting_audio() {
        let mut music = Music::default();
        let mut effects = vec![];
        music.update(Message::Restored(Ok(json!({"playQueue":{"tracks":[{"id":5,"title":"Five"}],"current":0,"position":17.25,"repeat":"ALL","shuffled":false}}))),&mut effects);
        assert_eq!(music.current().unwrap().track.id, 5);
        assert_eq!(music.position, 17.25);
        assert!(!music.playing);
        assert!(music.engine.is_none());
        assert_eq!(music.repeat, Repeat::All);
    }
    #[test]
    #[ignore = "requires Vulkan; renders native music pages and queue controls"]
    fn native_music_render() {
        let dir = std::path::Path::new("/tmp/tinystream-music-qa");
        std::fs::create_dir_all(dir).unwrap();
        let mut app = crate::app::App::new(Route::Album(1), crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        app.music.route = Some(Route::Album(1));
        app.music.detail = Some(Card {
            id: 1,
            name: "A quiet afternoon".into(),
            artist: "The River Quartet".into(),
            ..Card::default()
        });
        app.music.songs = tracks();
        app.music.queue = app.music.entries(tracks());
        app.music.expanded = true;
        app.music.panel = 1;
        crate::snapshot::snapshot(
            &mut app,
            iced_core::Size::new(1280, 800),
            1.0,
            None,
            &dir.join("album-queue.png"),
            &[],
        )
        .unwrap();
        app.music.panel = 2;
        crate::snapshot::snapshot(
            &mut app,
            iced_core::Size::new(1280, 800),
            1.0,
            None,
            &dir.join("player-info.png"),
            &[],
        )
        .unwrap();
    }
}
