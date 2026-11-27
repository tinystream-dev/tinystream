// SPDX-License-Identifier: AGPL-3.0-or-later
mod dismiss;
mod engine;
mod listen;
pub mod model;
pub(crate) mod output;
mod pages;
pub mod player;
mod ring;
mod room;
mod view;
use std::sync::{Arc, Mutex};

use graphql_client::GraphQLQuery;
use model::{Entry, Repeat, Track};
pub use pages::{dialog, page};
use player::{Change, Player};
pub use room::view as room;
use serde::Deserialize;
use serde_json::{Value, json};
pub use view::{now_playing, pill};

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};
use crate::route::Route;

/// What the server calls this app, on the queue it saves and what it says it's playing.
const CLIENT: &str = "Desktop";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Card {
    pub id: i64,
    pub name: String,
    pub artist: String,
    pub artists: Vec<model::Ref>,
    pub year: Option<i64>,
    pub cover: Option<String>,
    pub covers: Vec<String>,
    pub cover_tint: Option<String>,
    pub tracks: Vec<Track>,
    pub albums: Vec<Card>,
    pub appears_on: Vec<Card>,
    pub genres: Vec<String>,
    pub release_types: Vec<String>,
    pub labels: Vec<String>,
    pub release_date: Option<String>,
    pub original_date: Option<String>,
    pub disc_titles: Vec<DiscTitle>,
    pub compilation: bool,
    pub starred: bool,
    pub album_count: i64,
    pub track_count: i64,
    pub duration: f64,
    pub comment: Option<String>,
    pub owner: model::Ref,
    pub mine: bool,
    pub public: bool,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DiscTitle {
    pub disc: i64,
    pub title: String,
}
/// How the albums of a library are put in order.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Sort {
    #[default]
    Name,
    Artist,
    Newest,
    Year,
    Recent,
}
/// What the listening room's menu does.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomAction {
    Copy,
    Public(bool),
    Everyone(bool),
    Invite(i64, String),
    End,
    Leave,
}
/// What's asked of the person over the page.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    /// A new playlist, with these tracks in it.
    NewPlaylist(Vec<i64>),
    /// One of your playlists to put these tracks in.
    AddTo(Vec<i64>),
    DeletePlaylist(i64, String),
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
    Deleted(Result<Value, api::Error>),
    PlaylistRemove(usize),
    PlaylistPublic,
    Dialog(Option<Dialog>),
    Created(Result<Value, api::Error>),
    AddTo(i64, String, Vec<i64>),
    Added(String, Result<Value, api::Error>),
    Rename(bool),
    Popular(bool),
    Measured(i64, Result<Value, api::Error>),
    Queued(player::Ask, Result<Value, api::Error>),
    Found(u64, Result<Value, api::Error>),
    Timer(player::Timer),
    RoomMenu(RoomAction),
    JoinRoom,
    RoomStarted(Result<Value, api::Error>),
    Ready(Arc<Mutex<engine::Engine>>),
    Audio(engine::Update),
    Loaded(String, Result<Value, api::Error>),
    Done(Result<Value, api::Error>),
    Tab(Tab),
    Sort(Sort),
    Query(String),
    PlayFrom(usize, usize),
    ShuffleSongs,
    PlayAlbum(i64),
    Add(Vec<Track>, bool),
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
    Collapse,
    Mute,
    Fullscreen,
    Panel(u8),
    Star(i64, bool),
    StarDetail(bool),
    Name(String),
    CreatePlaylist,
    DeletePlaylist(i64),
    SavePlaylist,
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
    pub player: Player,
    pub people: Vec<(i64, String)>,
    pub can_invite: bool,
    pub can_share: bool,
    pub is_host: bool,
    pub host_name: String,
    /// Why there's no room to join, once that's known.
    pub room_gone: Option<String>,
    pub room_menu: Vec<RoomAction>,
    pub dialog: Option<Dialog>,
    /// Your playlists, for adding to one.
    pub lists: Vec<(i64, String)>,
    /// Passing notices for the app to show.
    pub notices: Vec<String>,
    pub renaming: bool,
    pub popular: bool,
    pub generation: u64,
    /// The room on the page; the player follows it once joined.
    pub room_code: Option<String>,
    room: Option<listen::Client>,
    pub snapshot: listen::Snapshot,
    /// The room's tracks as last fetched, by id.
    room_tracks: Vec<Track>,
    room_slot: Option<Arc<Mutex<Option<listen::Client>>>>,
    /// The status last told to the room.
    room_status: Option<&'static str>,
    pub guest_name: String,
    pub room_joined: bool,
    api: Option<Api>,
    engine: Option<Arc<Mutex<engine::Engine>>>,
    /// What's for the engine before it's ready.
    held: Vec<player::Do>,
    /// The rate sound goes out at.
    pub rate: u32,
    pub blocked: bool,
    pub expanded: bool,
    pub fullscreen: bool,
    pub panel: u8,
    pub tab: Tab,
    pub sort: Sort,
    pub query: String,
    pub name: String,
    pub cards: Vec<Card>,
    pub songs: Vec<Track>,
    pub detail: Option<Card>,
    pub more_by: Vec<Card>,
    pub error: Option<String>,
    pub loading: bool,
    pub lyrics: Vec<(Option<i64>, String)>,
    /// The track the lyrics are for.
    lyrics_for: Option<i64>,
    route: Option<Route>,
    library: bool,
    key: String,
}
impl Default for Music {
    fn default() -> Self {
        Self {
            images: crate::images::Images::default(),
            player: Player::default(),
            people: vec![],
            can_invite: false,
            can_share: false,
            is_host: false,
            host_name: String::new(),
            room_gone: None,
            room_menu: vec![],
            dialog: None,
            lists: vec![],
            notices: vec![],
            renaming: false,
            popular: false,
            generation: 0,
            room_code: None,
            room: None,
            snapshot: listen::Snapshot::default(),
            room_tracks: vec![],
            room_slot: None,
            room_status: None,
            guest_name: crate::player::room::guest_name().unwrap_or_default(),
            room_joined: false,
            api: None,
            engine: None,
            held: vec![],
            rate: 48000,
            blocked: false,
            expanded: false,
            fullscreen: false,
            panel: 0,
            tab: Tab::Albums,
            sort: Sort::Name,
            query: String::new(),
            name: String::new(),
            cards: vec![],
            songs: vec![],
            detail: None,
            more_by: vec![],
            error: None,
            loading: false,
            lyrics: vec![],
            lyrics_for: None,
            route: None,
            library: false,
            key: String::new(),
        }
    }
}

/// Where the player's settings and whether it was put away are kept.
fn kept(name: &str) -> Option<std::path::PathBuf> {
    crate::paths::config_dir().map(|d| d.join(name))
}

impl Music {
    pub fn current(&self) -> Option<&Entry> {
        self.player.current()
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
            let settings = kept("music-settings")
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| serde_json::from_slice(&b).ok());
            let dismissed = kept("music-dismissed").is_some_and(|p| p.exists());
            self.player = Player::new(settings.unwrap_or_default(), dismissed);
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
                self.player.restore();
            }
        }
        if !matches!(route, Route::Listen(_)) && self.room_code.is_some() {
            if let Some(room) = self.room.take() {
                room.close();
            }
            self.room_slot = None;
            self.room_code = None;
            self.room_tracks.clear();
            if self.player.room.is_some() {
                self.player.leave_room();
            }
        }
        self.library = library;
        let suspended = self.blocked || matches!(route, Route::Watch(..) | Route::Together(_) | Route::Clip(_));
        if suspended && !self.player.suspended {
            self.player.suspend();
        } else if !suspended && self.player.suspended {
            self.player.unsuspend();
        }
        if suspended {
            self.expanded = false;
            self.fullscreen = false;
        }
        if self.route.as_ref() != Some(route) {
            self.route = Some(route.clone());
            self.detail = None;
            self.more_by.clear();
            self.renaming = false;
            self.popular = false;
            self.dialog = None;
            self.cards.clear();
            self.songs.clear();
            if let Route::Listen(code) = route {
                self.begin_room(code.clone(), effects);
            }
            if library || matches!(route, Route::Album(_) | Route::Artist(_) | Route::Playlist(_)) {
                self.fetch(effects);
            }
        }
        self.run(effects);
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

    /// Does what the player asked for.
    fn run(&mut self, effects: &mut Vec<Effect>) {
        use player::Do;
        let mut work = std::mem::take(&mut self.held);
        work.extend(self.player.take());
        for task in work {
            match task {
                Do::Load(..) | Do::Upcoming(..) | Do::Gain(..) | Do::Play | Do::Pause | Do::Volume(_) | Do::Stop => {
                    let Some(engine) = &self.engine else {
                        self.held.push(task);
                        continue;
                    };
                    let mut engine = engine.lock().unwrap();
                    match task {
                        Do::Load(items, start) => engine.load(items, start),
                        Do::Upcoming(after, items) => engine.upcoming(after, items),
                        Do::Gain(key, gain) => engine.gain(key, gain),
                        Do::Play => engine.play(),
                        Do::Pause => engine.pause(),
                        Do::Volume(volume) => engine.volume(volume),
                        _ => engine.stop(),
                    }
                },
                Do::Save(input) => {
                    let input = api::music_save::QueueInput {
                        tracks: input.tracks,
                        current: input.current as i64,
                        position: input.position,
                        shuffled: input.shuffled,
                        repeat: match input.repeat {
                            Repeat::Off => api::music_save::Repeat::OFF,
                            Repeat::All => api::music_save::Repeat::ALL,
                            Repeat::One => api::music_save::Repeat::ONE,
                        },
                    };
                    self.request::<api::MusicSave>(api::music_save::Variables { input }, Message::Done, effects);
                },
                Do::Played(track_id) => {
                    self.request::<api::MusicPlayed>(api::music_played::Variables { track_id }, Message::Done, effects)
                },
                Do::NowPlaying(track_id, position, paused) => self.request::<api::MusicPlaying>(
                    api::music_playing::Variables { track_id, position, paused },
                    Message::Done,
                    effects,
                ),
                Do::Measure(track_id) => self.request::<api::MusicMeasure>(
                    api::music_measure::Variables { track_id },
                    move |r| Message::Measured(track_id, r),
                    effects,
                ),
                Do::Queue(ask) => {
                    self.request::<api::MusicQueue>(
                        api::music_queue::Variables {},
                        move |r| Message::Queued(ask, r),
                        effects,
                    );
                },
                Do::Track(ticket, id) => self.request::<api::MusicTrack>(
                    api::music_track::Variables { id },
                    move |r| Message::Found(ticket, r),
                    effects,
                ),
                Do::After(delay, timer) => {
                    let generation = self.generation;
                    effects.push(Effect::Request(Box::pin(async move {
                        tokio::time::sleep(delay).await;
                        AppMessage::Music(generation, Message::Timer(timer))
                    })));
                },
                Do::Started(_) => {
                    let id = self.current().map(|e| e.track.id);
                    if id != self.lyrics_for
                        && let Some(track_id) = id
                    {
                        self.lyrics_for = id;
                        self.lyrics.clear();
                        self.request::<api::MusicLyrics>(
                            api::music_lyrics::Variables { track_id },
                            move |r| Message::Loaded(format!("lyrics:{track_id}"), r),
                            effects,
                        );
                    }
                },
                Do::Remember(settings) => {
                    if let (Some(path), Ok(bytes)) = (kept("music-settings"), serde_json::to_vec(&settings)) {
                        let _ = path.parent().map(std::fs::create_dir_all);
                        let _ = std::fs::write(path, bytes);
                    }
                },
                Do::Dismissed(dismissed) => {
                    if let Some(path) = kept("music-dismissed") {
                        if dismissed {
                            let _ = path.parent().map(std::fs::create_dir_all);
                            let _ = std::fs::write(path, b"1");
                        } else if path.exists() {
                            let _ = std::fs::remove_file(path);
                        }
                    }
                },
                Do::Room(command) => {
                    if !self.snapshot.control() {
                        self.error = Some("Only the host can control this room".into());
                        continue;
                    }
                    let message = match command {
                        player::Room::Play(position) => json!({"type":"play","position":position}),
                        player::Room::Pause(position) => json!({"type":"pause","position":position}),
                        player::Room::Seek(position) => json!({"type":"seek","position":position}),
                        player::Room::Skip(index) => json!({
                            "type":"skip",
                            "index":index,
                            "from":self.snapshot.room.as_ref().map_or(0, |r| r.current),
                        }),
                        player::Room::Add(tracks, next) => json!({"type":"add","tracks":tracks,"next":next}),
                        player::Room::Remove(index) => json!({"type":"remove","index":index}),
                        player::Room::Move(from, to) => json!({"type":"move","from":from,"to":to}),
                    };
                    if let Some(room) = &self.room {
                        room.send(message);
                    }
                },
            }
        }
    }

    fn fetch(&mut self, effects: &mut Vec<Effect>) {
        self.loading = true;
        self.error = None;
        self.key = format!("{:?}:{:?}:{:?}:{}", self.route, self.tab, self.sort, self.query);
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
                        sort: match self.sort {
                            Sort::Name => api::music_albums::AlbumSort::NAME,
                            Sort::Artist => api::music_albums::AlbumSort::ARTIST,
                            Sort::Newest => api::music_albums::AlbumSort::NEWEST,
                            Sort::Year => api::music_albums::AlbumSort::YEAR,
                            Sort::Recent => api::music_albums::AlbumSort::RECENT,
                        },
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
                    api::music_songs::Variables { library: Some(library), query: self.query.clone() },
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

    /// Saves the queue now, before the app goes away.
    pub fn save(&mut self, effects: &mut Vec<Effect>) {
        self.player.flush();
        self.run(effects);
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) -> Option<Route> {
        let route = self.handle(message, effects);
        if self.current().is_none() || self.player.dismissed || self.player.suspended {
            self.expanded = false;
            self.fullscreen = false;
        }
        self.run(effects);
        self.fetch_art(effects);
        route
    }

    fn handle(&mut self, message: Message, effects: &mut Vec<Effect>) -> Option<Route> {
        match message {
            Message::Edited(result) => match result {
                Ok(_) => self.fetch(effects),
                Err(e) => self.error = Some(e.message),
            },
            Message::Created(result) => match result {
                Ok(value) => {
                    let filled = matches!(&self.dialog, Some(Dialog::NewPlaylist(tracks)) if !tracks.is_empty());
                    self.dialog = None;
                    if filled {
                        let name = value["createPlaylist"]["name"].as_str().unwrap_or_default();
                        self.notices.push(format!("Made “{name}”"));
                    } else if let Some(id) = value["createPlaylist"]["id"].as_u64() {
                        return Some(Route::Playlist(id));
                    }
                },
                Err(e) => self.error = Some(e.message),
            },
            Message::AddTo(id, name, tracks) => {
                self.dialog = None;
                self.request::<api::MusicAddToPlaylist>(
                    api::music_add_to_playlist::Variables { id, tracks },
                    move |r| Message::Added(name, r),
                    effects,
                );
            },
            Message::Added(name, result) => match result {
                Ok(_) => {
                    self.notices.push(format!("Added to “{name}”"));
                    if matches!(self.route, Some(Route::Playlist(_))) {
                        self.fetch(effects);
                    }
                },
                Err(e) => self.notices.push(e.message),
            },
            Message::Deleted(result) => match result {
                Ok(_) => return Some(Route::Home),
                Err(e) => self.error = Some(e.message),
            },
            Message::PlaylistRemove(index) => {
                if index < self.songs.len() {
                    self.songs.remove(index);
                    self.save_playlist(None, None, true, effects);
                }
            },
            Message::PlaylistPublic => {
                if let Some(public) = self.detail.as_ref().map(|d| !d.public) {
                    self.save_playlist(None, Some(public), false, effects);
                }
            },
            Message::Rename(renaming) => {
                self.renaming = renaming;
                if let Some(detail) = &self.detail {
                    self.name = detail.name.clone();
                }
            },
            Message::Popular(all) => self.popular = all,
            Message::Measured(id, result) => {
                let track =
                    result.ok().and_then(|v| serde_json::from_value::<Track>(v["measureLoudness"].clone()).ok());
                self.player.measured(id, track);
            },
            Message::Queued(ask, result) => {
                let queue = result.ok().map(|value| {
                    let q = &value["playQueue"];
                    player::PlayQueue {
                        tracks: serde_json::from_value(q["tracks"].clone()).unwrap_or_default(),
                        current: q["current"].as_u64().unwrap_or(0) as usize,
                        position: q["position"].as_f64().unwrap_or(0.0),
                        shuffled: q["shuffled"].as_bool().unwrap_or(false),
                        repeat: serde_json::from_value(q["repeat"].clone()).unwrap_or_default(),
                    }
                });
                self.player.queue(ask, queue);
            },
            Message::Found(ticket, result) => {
                let track = result.ok().and_then(|v| serde_json::from_value::<Track>(v["track"].clone()).ok());
                self.player.track(ticket, track);
            },
            Message::Timer(timer) => self.player.timer(timer),
            Message::Dialog(dialog) => {
                match &dialog {
                    Some(Dialog::NewPlaylist(_)) => self.name.clear(),
                    Some(Dialog::AddTo(_)) => self.request::<api::MusicPlaylistNames>(
                        api::music_playlist_names::Variables {},
                        |r| Message::Loaded("lists".into(), r),
                        effects,
                    ),
                    _ => {},
                }
                self.dialog = dialog;
            },
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
            Message::RoomMenu(action) => {
                let command = match action {
                    RoomAction::Copy => return self.handle(Message::CopyRoom, effects),
                    RoomAction::Public(public) => json!({"type":"settings","public":public}),
                    RoomAction::Everyone(everyone) => {
                        json!({"type":"settings","control":if everyone {"everyone"} else {"host"}})
                    },
                    RoomAction::Invite(id, _) => json!({"type":"invite","userId":id}),
                    RoomAction::End => {
                        if let Some(room) = &self.room {
                            room.send(json!({"type":"end"}));
                        }
                        return Some(Route::Home);
                    },
                    RoomAction::Leave => return Some(Route::Home),
                };
                if let Some(room) = &self.room {
                    room.send(command);
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
            Message::Ready(engine) => self.engine = Some(engine),
            Message::Loaded(key, result) => return self.loaded(key, result, effects),
            Message::Done(Err(error)) => self.error = Some(error.message),
            Message::Done(Ok(_)) => {},
            Message::Audio(update) => match update {
                engine::Update::Reload => self.player.engine_reload(),
                engine::Update::Position { key, time, rate } => {
                    self.rate = rate;
                    self.player.engine_position(key, time);
                    self.player.tick();
                },
                engine::Update::Track(key) => self.player.engine_track(&key),
                engine::Update::Ended => self.player.engine_ended(),
                engine::Update::Starved(waiting) => {
                    self.player.engine_starved(waiting);
                    self.tell_room();
                },
                engine::Update::Error(key, error) => self.player.engine_error(&key, &error),
            },
            Message::Tab(tab) => {
                self.tab = tab;
                self.cards.clear();
                self.songs.clear();
                self.fetch(effects);
            },
            Message::Sort(sort) => {
                self.sort = sort;
                self.fetch(effects);
            },
            Message::Query(query) => {
                self.query = query;
                self.fetch(effects);
            },
            Message::PlayFrom(index, end) => {
                let tracks = self.songs[..end.min(self.songs.len())].to_vec();
                self.player.play(tracks, index, false);
            },
            Message::ShuffleSongs => self.player.play(self.songs.clone(), 0, true),
            Message::PlayAlbum(id) => self.request::<api::MusicAlbum>(
                api::music_album::Variables { id },
                |r| Message::Loaded("play-album".into(), r),
                effects,
            ),
            Message::Add(tracks, true) => self.player.play_next(tracks),
            Message::Add(tracks, false) => self.player.add(tracks),
            Message::Toggle => self.player.toggle(),
            Message::Pause => self.player.pause(),
            Message::Next => self.player.next(),
            Message::Previous => self.player.previous(),
            Message::Jump(index) => self.player.jump_to(index),
            Message::Seek(time) => self.player.seek(time),
            Message::Remove(index) => {
                if let Some(key) = self.player.queue.get(index).map(|e| e.key.clone()) {
                    self.player.remove(&key);
                }
            },
            Message::Move(index, by) => self.player.move_entry(index, (index as i32 + by).max(0) as usize),
            Message::Shuffle => self.player.shuffle(!self.player.shuffled),
            Message::Repeat => self.player.repeat(match self.player.repeat {
                Repeat::Off => Repeat::All,
                Repeat::All => Repeat::One,
                Repeat::One => Repeat::Off,
            }),
            Message::Volume(volume) => {
                self.player.settings(Change::Volume(volume));
                if self.player.settings.muted {
                    self.player.settings(Change::Muted(false));
                }
            },
            Message::Gain(gain) => self.player.settings(Change::Gain(gain)),
            Message::Fade(fade) => self.player.settings(Change::Crossfade(fade)),
            Message::Mute => self.player.settings(Change::Muted(!self.player.settings.muted)),
            Message::Dismiss => {
                self.player.dismiss();
                self.expanded = false;
                self.fullscreen = false;
            },
            Message::Collapse => {
                self.expanded = false;
                self.fullscreen = false;
            },
            Message::Expand => {
                self.expanded = !self.expanded;
                self.fullscreen = false;
            },
            Message::Fullscreen => {
                let open = !self.fullscreen;
                self.fullscreen = open;
                self.expanded = open;
            },
            Message::Panel(panel) => {
                self.panel = panel;
                self.fullscreen = true;
                self.expanded = true;
            },
            Message::Star(id, starred) => {
                for t in &mut self.songs {
                    if t.id == id {
                        t.starred = starred;
                    }
                }
                if let Some(mut track) = self.player.queue.iter().find(|e| e.track.id == id).map(|e| e.track.clone()) {
                    track.starred = starred;
                    self.player.update(&track);
                }
                self.request::<api::MusicStar>(
                    api::music_star::Variables { kind: api::music_star::MusicKind::TRACK, id, starred },
                    Message::Done,
                    effects,
                );
            },
            Message::StarDetail(starred) => {
                let kind = match self.route {
                    Some(Route::Album(_)) => api::music_star::MusicKind::ALBUM,
                    Some(Route::Artist(_)) => api::music_star::MusicKind::ARTIST,
                    _ => return None,
                };
                if let Some(detail) = &mut self.detail {
                    detail.starred = starred;
                    let id = detail.id;
                    self.request::<api::MusicStar>(
                        api::music_star::Variables { kind, id, starred },
                        Message::Done,
                        effects,
                    );
                }
            },
            Message::Name(name) => {
                self.name = name.clone();
                self.guest_name = name;
            },
            Message::CreatePlaylist => {
                let tracks = match &self.dialog {
                    Some(Dialog::NewPlaylist(tracks)) => tracks.clone(),
                    _ => vec![],
                };
                if !self.name.trim().is_empty() {
                    self.request::<api::MusicCreatePlaylist>(
                        api::music_create_playlist::Variables { name: self.name.trim().to_owned(), tracks },
                        Message::Created,
                        effects,
                    )
                }
            },
            Message::DeletePlaylist(id) => {
                self.dialog = None;
                self.request::<api::MusicDeletePlaylist>(
                    api::music_delete_playlist::Variables { id },
                    Message::Deleted,
                    effects,
                )
            },
            Message::SavePlaylist => {
                self.renaming = false;
                let name = self.name.trim().to_owned();
                if !name.is_empty() && self.detail.as_ref().is_some_and(|d| d.name != name) {
                    if let Some(detail) = &mut self.detail {
                        detail.name = name.clone();
                    }
                    self.save_playlist(Some(name), None, false, effects);
                }
            },
            Message::Go(route) => return Some(route),
            Message::StartRoom => {
                let tracks = self.songs.iter().map(|t| t.id).collect();
                self.request::<api::MusicStartRoom>(
                    api::music_start_room::Variables {
                        input: api::music_start_room::NewListenRoom {
                            tracks,
                            current: 0,
                            position: 0.0,
                            paused: true,
                            public: false,
                        },
                    },
                    Message::RoomStarted,
                    effects,
                );
            },
        }
        None
    }

    fn loaded(&mut self, key: String, result: Result<Value, api::Error>, effects: &mut Vec<Effect>) -> Option<Route> {
        if key == "people" {
            if let Ok(value) = result {
                self.people = value["users"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| Some((v["id"].as_i64()?, v["username"].as_str()?.to_owned())))
                    .collect();
                self.fill_room_menu();
            }
            return None;
        }
        if key == "lists" {
            if let Ok(value) = result {
                self.lists = value["playlists"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|p| p["mine"].as_bool() == Some(true))
                    .filter_map(|p| Some((p["id"].as_i64()?, p["name"].as_str()?.to_owned())))
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
                    self.can_share = info["canShare"].as_bool().unwrap_or(false);
                    self.is_host = info["isHost"].as_bool().unwrap_or(false);
                    self.host_name = info["hostName"].as_str().unwrap_or_default().to_owned();
                    if self.can_invite && self.people.is_empty() {
                        self.request::<api::MusicPeople>(
                            api::music_people::Variables {},
                            |r| Message::Loaded("people".into(), r),
                            effects,
                        );
                    }
                    self.room_tracks = serde_json::from_value(info["tracks"].clone()).unwrap_or_default();
                    if self.room.is_none() && (info["signedIn"].as_bool() == Some(true) || self.room_joined) {
                        self.connect_room(effects);
                    }
                    self.sync_room(effects);
                },
                Err(e) if self.room_tracks.is_empty() => self.room_gone = Some(e.message),
                Err(e) => self.error = Some(e.message),
            }
            self.fill_room_menu();
            return None;
        }
        if key == "play-album" {
            match result {
                Ok(value) => {
                    let tracks = serde_json::from_value(value["album"]["tracks"].clone()).unwrap_or_default();
                    self.player.play(tracks, 0, false);
                },
                Err(e) => self.error = Some(e.message),
            }
            return None;
        }
        if key == "more" {
            if let Ok(value) = result {
                let id = self.detail.as_ref().map(|d| d.id);
                self.more_by = serde_json::from_value::<Vec<Card>>(value["artist"]["albums"].clone())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|a| Some(a.id) != id)
                    .take(12)
                    .collect();
            }
            return None;
        }
        if let Some(id) = key.strip_prefix("lyrics:") {
            if let Ok(value) = result
                && self.lyrics_for.is_some_and(|l| l.to_string() == id)
            {
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
                    if let (Some(Route::Album(_)), Some(album)) = (&self.route, &self.detail)
                        && !album.compilation
                        && let Some(artist) = album.artists.first()
                    {
                        self.request::<api::MusicMore>(
                            api::music_more::Variables { id: artist.id },
                            |r| Message::Loaded("more".into(), r),
                            effects,
                        );
                    }
                } else if let Some(songs) = value.get("songs") {
                    self.songs = serde_json::from_value(songs.clone()).unwrap_or_default();
                } else if let Some(rows) =
                    value.get("albums").or_else(|| value.get("artists")).or_else(|| value.get("playlists"))
                {
                    self.cards = serde_json::from_value(rows.clone()).unwrap_or_default();
                }
            },
            Err(error) => self.error = Some(error.message),
        }
        None
    }

    pub fn live(&mut self, live: &crate::api::Live, effects: &mut Vec<Effect>) {
        match live {
            crate::api::Live::Connected => self.player.refresh_from_server(),
            crate::api::Live::Event(event) => match &event.events {
                api::events::EventsEvents::LibraryChanged(_) | api::events::EventsEvents::ScanFinished(_) => {
                    if self.route.as_ref().is_some_and(|r| self.owns(r) && !matches!(r, Route::Listen(_))) {
                        self.fetch(effects);
                    }
                },
                api::events::EventsEvents::ListChanged(change) => {
                    if matches!(change.list, api::events::ChangedList::PLAYLISTS)
                        && (self.tab == Tab::Playlists || matches!(self.route, Some(Route::Playlist(_))))
                    {
                        self.fetch(effects);
                    }
                },
                // Another app moved the queue on; follow it unless we're the ones playing.
                api::events::EventsEvents::QueueChanged(change) => {
                    if change.by.as_deref() != Some(CLIENT) {
                        self.player.refresh_from_server();
                    }
                },
                api::events::EventsEvents::PlaybackChanged(change) => {
                    self.player.follow_remote(&change.client, change.track_id, change.position, change.paused)
                },
                _ => {},
            },
            _ => {},
        }
        self.run(effects);
    }

    fn fetch_art(&mut self, effects: &mut Vec<Effect>) {
        let cards = self.cards.iter().chain(&self.more_by).chain(self.detail.iter().flat_map(|d| &d.appears_on));
        let queue = &self.player.queue;
        let paths: Vec<String> = cards
            .chain(self.detail.as_ref())
            .flat_map(|c| c.cover.iter().chain(c.covers.iter().take(4)))
            .chain(self.songs.iter().filter_map(|t| t.cover.as_ref()))
            .chain(
                queue.iter().skip(self.player.index.saturating_sub(1)).take(30).filter_map(|e| e.track.cover.as_ref()),
            )
            .cloned()
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

    /// What the system's media controls show.
    pub fn desktop_media(&self) -> crate::platform::integration::Media {
        let Some(current) = self.current().filter(|_| !self.player.suspended) else {
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
            position: self.player.shown(),
            playing: self.player.playing,
            rate: 1.0,
            volume: self.player.settings.volume as f64,
            next: self.player.index + 1 < self.player.queue.len(),
            previous: self.player.index > 0,
            seek: true,
            discontinuities: 0,
            tray: false,
        }
    }

    fn begin_room(&mut self, code: String, effects: &mut Vec<Effect>) {
        if let Some(room) = self.room.take() {
            room.close();
        }
        self.room_slot = None;
        self.room_code = Some(code.clone());
        self.room_joined = false;
        self.room_gone = None;
        self.room_tracks.clear();
        self.room_status = None;
        self.loading = true;
        self.snapshot = listen::Snapshot::default();
        self.request::<api::MusicRoom>(
            api::music_room::Variables { code },
            |r| Message::Loaded("room-info".into(), r),
            effects,
        );
    }

    fn connect_room(&mut self, effects: &mut Vec<Effect>) {
        if self.room.is_some() || self.room_slot.is_some() {
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
        self.player.enter_room(code.clone());
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

    /// Brings the player in line with the room, one change at a time.
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
        self.fill_room_menu();
        if self.snapshot.ended {
            if self.room_gone.is_none() {
                self.room_gone = Some("Whoever started this room has ended it.".into());
                self.player.leave_room();
            }
            return;
        }
        let Some(state) = &self.snapshot.room else {
            return;
        };
        let Some((index, time, playing)) = self.snapshot.target() else {
            return;
        };
        if state.queue.is_empty() {
            return;
        }
        let tracks: Vec<Track> =
            state.queue.iter().filter_map(|id| self.room_tracks.iter().find(|t| t.id == *id).cloned()).collect();
        if tracks.len() != state.queue.len() {
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
        if self.player.index != index || self.current().is_none() {
            self.tell("loading");
        }
        self.player.follow(tracks, index, time, playing);
        self.tell_room();
    }

    fn tell_room(&mut self) {
        if self.room.is_some() && self.player.room.is_some() {
            self.tell(if self.player.waiting && self.player.playing { "buffering" } else { "ready" });
        }
    }

    fn tell(&mut self, status: &'static str) {
        if self.room_status == Some(status) {
            return;
        }
        if let Some(room) = &self.room {
            self.room_status = Some(status);
            room.send(json!({"type":"status","status":status}));
        }
    }

    /// What the room's menu offers, for who's asking and who's in it.
    fn fill_room_menu(&mut self) {
        let mut menu = vec![RoomAction::Copy];
        if let Some(room) = &self.snapshot.room {
            if self.can_share {
                menu.push(RoomAction::Public(!room.public));
            }
            if self.is_host {
                menu.push(RoomAction::Everyone(room.settings.control != crate::player::sync::Control::Everyone));
            }
            if self.can_invite {
                let here = room.members.iter().filter_map(|m| m.user_id).collect::<Vec<_>>();
                menu.extend(
                    self.people
                        .iter()
                        .filter(|(id, _)| !here.contains(id))
                        .map(|(id, name)| RoomAction::Invite(*id, name.clone())),
                );
            }
        }
        menu.push(if self.is_host { RoomAction::End } else { RoomAction::Leave });
        self.room_menu = menu;
    }

    /// Sends a change to the playlist on the page: its name, who sees it, or its songs as listed.
    fn save_playlist(&self, name: Option<String>, public: Option<bool>, tracks: bool, effects: &mut Vec<Effect>) {
        let Some(id) = self.detail.as_ref().map(|d| d.id) else { return };
        self.request::<api::MusicUpdatePlaylist>(
            api::music_update_playlist::Variables {
                id,
                input: api::music_update_playlist::PlaylistInput {
                    name,
                    comment: None,
                    public,
                    tracks: tracks.then(|| self.songs.iter().map(|t| t.id).collect()),
                },
            },
            Message::Edited,
            effects,
        );
    }
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
        (1..=3).map(|id| Track { id, title: format!("Track {id}"), duration: 200.0, ..Track::default() }).collect()
    }
    fn entries(tracks: Vec<Track>) -> Vec<Entry> {
        tracks.into_iter().map(|track| Entry { key: format!("t{}", track.id), track, fallback: false }).collect()
    }
    #[test]
    fn closing_a_playing_player_pauses_and_dismisses_without_opening_fullscreen() {
        let mut music = Music::default();
        let mut effects = vec![];
        music.player.play(tracks(), 0, false);
        music.expanded = true;
        music.update(Message::Dismiss, &mut effects);
        assert!(music.player.dismissed, "Close must dismiss while playing");
        assert!(!music.player.playing);
        assert!(!music.expanded);
        assert!(!music.fullscreen);
    }
    #[test]
    fn leaving_now_playing_returns_to_the_pill() {
        let mut music = Music::default();
        music.player.state_mut().queue = entries(tracks());
        let mut effects = vec![];
        music.update(Message::Fullscreen, &mut effects);
        assert!(music.fullscreen);
        music.update(Message::Fullscreen, &mut effects);
        assert!(!music.fullscreen);
        assert!(!music.expanded, "Closing Now Playing must return to the pill");
    }
    #[test]
    fn queue_edits_and_shuffle_preserve_the_audible_track() {
        let mut music = Music::default();
        let mut effects = vec![];
        music.player.play(tracks(), 1, false);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Shuffle, &mut effects);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Shuffle, &mut effects);
        assert_eq!(music.player.queue.iter().map(|e| e.track.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Remove(0), &mut effects);
        assert_eq!(music.current().unwrap().track.id, 2);
        music.update(Message::Seek(42.0), &mut effects);
        music.update(Message::Pause, &mut effects);
        assert!(!music.player.playing);
        assert_eq!(music.player.shown(), 42.0);
        music.update(Message::Dismiss, &mut effects);
        assert!(music.player.dismissed);
    }
    #[test]
    fn server_queue_restores_paused_without_starting_audio() {
        let mut music = Music::default();
        let mut effects = vec![];
        music.player.restore();
        music.update(Message::Queued(player::Ask::Restore, Ok(json!({"playQueue":{"tracks":[{"id":5,"title":"Five"}],"current":0,"position":17.25,"repeat":"ALL","shuffled":false}}))),&mut effects);
        assert_eq!(music.current().unwrap().track.id, 5);
        assert_eq!(music.player.shown(), 17.25);
        assert!(!music.player.playing);
        assert!(music.engine.is_none());
        assert_eq!(music.player.repeat, Repeat::All);
    }
    #[test]
    fn engine_work_waits_for_the_engine() {
        let mut music = Music::default();
        let mut effects = vec![];
        music.player.play(tracks(), 0, false);
        music.update(Message::Next, &mut effects);
        assert!(music.held.iter().filter(|d| matches!(d, player::Do::Load(..))).count() == 2);
    }
    #[test]
    #[ignore = "requires Vulkan; draws the library and the album, artist and playlist pages"]
    fn music_pages_render() {
        use iced_core::{Point, Size};
        let dir = std::env::var_os("MUSIC_PAGES").map_or_else(|| std::env::temp_dir().join("music-pages"), Into::into);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = crate::app::App::new(Route::Library("Music".into()), crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        let shades: [(u8, u8, u8); 6] =
            [(200, 40, 70), (40, 90, 200), (230, 160, 40), (30, 150, 110), (120, 60, 180), (90, 90, 90)];
        for (i, (r, g, b)) in shades.iter().enumerate() {
            let pixels = (0..64 * 64).flat_map(|p| {
                let k = (p % 64 + p / 64) as u8;
                [r.saturating_add(k), *g, b.saturating_sub(k / 2), 255]
            });
            let handle = iced_widget::image::Handle::from_rgba(64, 64, pixels.collect::<Vec<u8>>());
            app.music.images.arrived(format!("/c/{i}"), Some(handle), false);
        }
        let card = |id: i64, name: &str| Card {
            id,
            name: name.into(),
            artist: "The River Quartet".into(),
            artists: vec![model::Ref { id: 1, name: "The River Quartet".into() }],
            year: Some(2019 + id),
            cover: (id != 3).then(|| format!("/c/{}", id % 6)),
            covers: (0..4).map(|i| format!("/c/{}", (id + i) % 6)).collect(),
            track_count: 9,
            album_count: 3,
            duration: 2460.0,
            owner: model::Ref { id: 2, name: "sol".into() },
            ..Card::default()
        };
        let names = [
            "A quiet afternoon",
            "Northern lights",
            "Paper boats",
            "Signal fires",
            "Low tide",
            "Glass houses",
            "Late trains",
        ];
        app.music.library = true;
        app.music.route = Some(Route::Library("Music".into()));
        app.music.cards = names.iter().enumerate().map(|(i, n)| card(i as i64, n)).collect();
        let mut songs: Vec<Track> = (1..=7)
            .map(|id| Track {
                id,
                title: format!("{} part {id}", names[id as usize - 1]),
                artist: "The River Quartet".into(),
                artists: vec![model::Ref { id: 1, name: "The River Quartet".into() }],
                album: "A quiet afternoon".into(),
                album_id: Some(0),
                number: Some(id),
                disc: Some(if id > 4 { 2 } else { 1 }),
                duration: 200.0 + id as f64 * 13.0,
                codec: "flac".into(),
                lossless: true,
                sample_rate: Some(if id == 2 { 96000 } else { 44100 }),
                bit_depth: Some(if id == 2 { 24 } else { 16 }),
                cover: Some(format!("/c/{}", id % 6)),
                starred: id == 3,
                ..Track::default()
            })
            .collect();
        let size = Size::new(1280, 1500);
        let draw = |app: &mut crate::app::App, name: &str, cursor: Option<Point>| {
            crate::snapshot::snapshot(app, size, 1.0, cursor, &dir.join(name), &[]).unwrap();
        };
        for tab in [Tab::Albums, Tab::Artists, Tab::Playlists] {
            app.music.tab = tab;
            draw(&mut app, &format!("library-{tab:?}.png"), Some(Point::new(400.0, 300.0)));
        }
        app.music.tab = Tab::Songs;
        app.music.songs = songs.clone();
        draw(&mut app, "library-songs.png", Some(Point::new(600.0, 330.0)));

        app.history = crate::route::History::new(Route::Album(0));
        app.music.route = Some(Route::Album(0));
        let mut album = card(0, names[0]);
        album.cover_tint = Some("200 40 70".into());
        album.release_types = vec!["album".into()];
        album.genres = vec!["Jazz".into(), "Ambient".into()];
        album.labels = vec!["Quiet Records".into()];
        album.release_date = Some("2019-04-12".into());
        album.disc_titles = vec![DiscTitle { disc: 2, title: "Night".into() }];
        album.track_count = 7;
        app.music.detail = Some(album);
        app.music.songs = songs.clone();
        app.music.more_by = app.music.cards[1..5].to_vec();
        let state = app.music.player.state_mut();
        state.queue = entries(songs.clone());
        state.index = 1;
        state.playing = true;
        draw(&mut app, "album.png", Some(Point::new(700.0, 520.0)));

        app.history = crate::route::History::new(Route::Artist(1));
        app.music.route = Some(Route::Artist(1));
        let mut artist = card(1, "The River Quartet");
        artist.cover_tint = Some("40 90 200".into());
        artist.albums = app.music.cards[..4].to_vec();
        artist.albums[3].release_types = vec!["single".into()];
        app.music.detail = Some(artist);
        songs[0].album = "Northern lights".into();
        app.music.songs = songs.clone();
        draw(&mut app, "artist.png", None);

        app.history = crate::route::History::new(Route::Playlist(4));
        app.music.route = Some(Route::Playlist(4));
        let mut list = card(4, "Sunday morning");
        list.mine = true;
        app.music.detail = Some(list);
        draw(&mut app, "playlist.png", Some(Point::new(700.0, 470.0)));
        app.music.dialog = Some(Dialog::NewPlaylist(vec![]));
        draw(&mut app, "new-playlist.png", None);
        app.music.dialog = None;

        app.history = crate::route::History::new(Route::Listen("ROOM".into()));
        app.music.route = Some(Route::Listen("ROOM".into()));
        app.music.room_code = Some("ROOM".into());
        app.music.host_name = "sol".into();
        app.session.screen = crate::session::Screen::Connect;
        draw(&mut app, "room-join.png", None);
        app.session.screen = crate::session::Screen::Ready;
        app.music.room_joined = true;
        app.music.is_host = true;
        app.music.can_share = true;
        app.music.people = vec![(7, "cranky".into())];
        app.music.player.state_mut().queue[1].track.cover_tint = Some("40 90 200".into());
        app.music.snapshot.connected = true;
        app.music.snapshot.room = serde_json::from_value(json!({
            "queue": [1, 2, 3, 4, 5, 6, 7],
            "current": 1,
            "clock": { "paused": false, "running": true, "position": 63.0, "at": 0.0, "rate": 1.0 },
            "members": [
                { "id": 1, "name": "David", "userId": 1, "status": "ready" },
                { "id": 2, "name": "Guest Fox", "status": "buffering" }
            ],
            "host": { "id": 1, "name": "David" },
            "public": false,
            "settings": { "control": "everyone", "waitForAll": false }
        }))
        .ok();
        assert!(app.music.snapshot.room.is_some());
        app.music.fill_room_menu();
        app.music.player.state_mut().resume_at = 63.0;
        draw(&mut app, "room.png", Some(Point::new(1000.0, 260.0)));
        let click = [
            iced_core::Event::Mouse(iced_core::mouse::Event::ButtonPressed(iced_core::mouse::Button::Left)),
            iced_core::Event::Mouse(iced_core::mouse::Event::ButtonReleased(iced_core::mouse::Button::Left)),
        ];
        crate::snapshot::snapshot(
            &mut app,
            size,
            1.0,
            Some(Point::new(1077.0, 44.0)),
            &dir.join("room-menu.png"),
            &click,
        )
        .unwrap();
        app.music.room_gone = Some("Whoever started this room has ended it.".into());
        draw(&mut app, "room-gone.png", None);
    }
    #[test]
    #[ignore = "requires Vulkan; clicks native player controls and verifies close/collapse behavior"]
    fn native_music_render() {
        use iced_core::{Event, Point, Size, mouse};
        let dir = std::path::Path::new("/tmp/tinystream-music-ui-qa");
        std::fs::create_dir_all(dir).unwrap();
        let mut app = crate::app::App::new(Route::Album(1), crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        app.music.route = Some(Route::Album(1));
        app.music.detail = Some(Card {
            id: 1,
            name: "A quiet afternoon".into(),
            artist: "The River Quartet".into(),
            ..Default::default()
        });
        let pixels = (0..64 * 64).flat_map(|p| {
            let (x, y) = ((p % 64) as u8, (p / 64) as u8);
            [200u8.saturating_sub(y * 2), 40 + x * 2, 70 + y, 255]
        });
        let handle = iced_widget::image::Handle::from_rgba(64, 64, pixels.collect::<Vec<u8>>());
        app.music.images.arrived("/c/now".into(), Some(handle), false);
        let mut songs = tracks();
        for t in &mut songs {
            t.cover = Some("/c/now".into());
            t.cover_tint = Some("200 40 70".into());
            t.artist = "The River Quartet".into();
            t.album = "A quiet afternoon".into();
            t.duration = 245.0;
            t.codec = "flac".into();
            t.sample_rate = Some(48000);
            t.bit_depth = Some(24);
        }
        app.music.songs = songs.clone();
        let state = app.music.player.state_mut();
        state.queue = entries(songs);
        state.resume_at = 63.0;
        state.playing = true;
        let draw = |app: &mut crate::app::App, name: &str, cursor: Option<Point>, events: &[Event]| {
            crate::snapshot::snapshot(app, Size::new(1280, 800), 1.0, cursor, &dir.join(name), events).unwrap()
        };
        let click = [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ];
        draw(&mut app, "pill.png", None, &[]);
        draw(&mut app, "pause.png", Some(Point::new(721.0, 754.0)), &click);
        assert!(!app.music.player.playing, "Pill play button should pause");
        assert!(!app.music.expanded, "Transport click must not expand the pill");
        assert!(!app.music.fullscreen);
        app.music.player.state_mut().playing = true;
        let messages = draw(&mut app, "closed.png", Some(Point::new(811.0, 754.0)), &click);
        assert!(
            messages.iter().any(|m| matches!(m, crate::app::Message::Music(_, Message::Dismiss))),
            "Click must reach close"
        );
        assert!(app.music.player.dismissed);
        assert!(!app.music.player.playing);
        assert!(!app.music.fullscreen);
        assert_eq!(app.music.player.queue.len(), 3);
        app.music.player.state_mut().dismissed = false;
        app.music.player.state_mut().playing = true;
        app.music.expanded = true;
        draw(&mut app, "card.png", None, &[]);
        draw(&mut app, "collapsed.png", Some(Point::new(300.0, 100.0)), &click);
        assert!(!app.music.expanded, "Clicking outside should collapse the card");
        assert!(app.music.player.playing);
        app.music.fullscreen = true;
        app.music.expanded = true;
        app.music.panel = 1;
        draw(&mut app, "now-playing-queue.png", None, &[]);
        draw(&mut app, "now-playing-closed.png", Some(Point::new(52.0, 68.0)), &click);
        assert!(!app.music.fullscreen);
        assert!(!app.music.expanded);
        assert!(app.music.player.playing);
        app.music.fullscreen = true;
        app.music.expanded = true;
        app.music.panel = 2;
        draw(&mut app, "now-playing-details.png", None, &[]);
        crate::snapshot::snapshot(&mut app, Size::new(768, 600), 1.0, None, &dir.join("small-now-playing.png"), &[])
            .unwrap();
    }
}
