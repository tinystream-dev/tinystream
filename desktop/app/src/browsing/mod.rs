// SPDX-License-Identifier: AGPL-3.0-or-later

//! Browsing state, cached independently of navigation. An invalidation during a request is
//! retained until that request settles, and replies from previous sessions are discarded.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use graphql_client::GraphQLQuery;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};
use crate::images::{self, Images};
use crate::route::Route;

#[allow(dead_code)]
pub mod model;
#[cfg(debug_assertions)]
pub mod preview;
mod view;

use model::*;
pub use view::{allowed, dialog, notifications, palette, search, view};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Shell,
    Home,
    Music,
    BookHome,
    Coming,
    Library(String),
    Books(String),
    Title(i64),
    Similar(i64),
    Series(i64),
    Schedule(i64),
    Calendar(i64),
    Soon,
    Requests,
    Wanted,
    Downloads,
    Engine,
    Inbox,
    Discover(String, String),
    ForYou(String),
    Search(String),
    Recents(Vec<i64>),
    Options,
    Aired(String, String),
    Releases(i64, i64, Vec<i64>, String),
    Match(i64, String, String),
    Naming(i64, String),
}

#[derive(Default)]
struct Entry {
    data: Option<Arc<Document>>,
    error: Option<String>,
    pending: bool,
    dirty: bool,
    serial: u64,
    fetched: Option<Instant>,
    updates: u8,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    Pick(Arc<Discover>),
    Releases { series: i64, season: i64, episodes: Vec<i64>, name: String },
    Match(i64),
    Confirm { title: String, body: String, action: Arc<Action> },
    SeriesSettings(Arc<Series>),
}

#[derive(Debug, Clone)]
pub enum Action {
    Watched(Vec<i64>, bool),
    TitleWatched(i64, bool),
    RestoreTitle(i64, Vec<i64>),
    RestoreEpisodes(Vec<i64>, Vec<i64>),
    Refresh(i64),
    Approve(i64),
    Decline(i64),
    CancelRequest(i64),
    Pause(Vec<i64>),
    Bulk(bool),
    Resume(Vec<i64>),
    Recheck(Vec<i64>),
    Import(i64),
    Remove(Vec<i64>, bool),
    Read(Option<Vec<i64>>),
    DeleteNotice(Option<i64>),
    Add(Arc<Discover>, String, String, String),
    Request(Arc<Discover>, String),
    Manage(i64),
    UpdateSeries(i64, Value),
    RefreshSchedule(i64),
    RemoveSeries(i64),
    LookAgain(i64, Option<i64>, Option<i64>),
    DeleteDownloaded(i64, Option<i64>),
    Grab(i64, Arc<ReleaseCandidate>),
    Match(i64, String, String),
    Artwork { id: i64, kind: Option<String>, reset: bool },
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(u64, Key, u64, Result<Arc<Document>, api::Error>),
    Mutated(u64, Result<(), api::Error>),
    Picture(u64, String, Option<iced_widget::image::Handle>, bool),
    Retry(Key),
    WantImage(String),
    Query(String),
    FindShows(String),
    Library(String),
    Category(String),
    Filter(String),
    Sort(bool),
    BookSort(String),
    Season(i64),
    Overview(i64),
    Week(i64),
    Dialog(Option<Dialog>),
    Note(String),
    Monitor(String),
    Profile(String),
    PickLibrary(String),
    ReleaseQuery(String),
    ReleaseSubmit,
    ShowRejected,
    ManualRelease(Option<Arc<ReleaseCandidate>>),
    ManualSeason(String),
    ManualEpisode(String),
    MatchReady(u64, String),
    DownloadCategory(String),
    MatchQuery(String),
    Provider(String),
    Act(Action),
    Follow(Notice),
    DownloadGroup(String),
    Draft(&'static str, String),
    Source(String),
    Shelf(String, iced_widget::scrollable::Viewport),
    PageShelf(String, f32),
    Tick,
    SearchReady(u64, String),
    RecentLoaded(Vec<i64>),
    RecentSaved,
    InboxSettled(Result<(), api::Error>),
    DiscoverReady(u64, String),
}

#[derive(Default)]
pub struct Browsing {
    cache: HashMap<Key, Entry>,
    pub images: Images,
    pub generation: u64,
    pub reduce_motion: bool,
    serial: u64,
    pub query: String,
    pub discover_input: String,
    pub previous_discover: Option<Key>,
    pub library: String,
    pub category: String,
    pub filter: HashMap<Route, String>,
    pub newest: HashSet<Route>,
    pub book_sort: HashMap<Route, String>,
    pub seasons: HashMap<i64, i64>,
    pub expanded: HashSet<i64>,
    pub undo: Option<Action>,
    notice_at: Option<Instant>,
    pending_notice: Option<String>,
    pub week: i64,
    pub shelves: HashMap<String, iced_widget::scrollable::Viewport>,
    pub dialog: Option<Dialog>,
    pub note: String,
    pub monitor: String,
    pub profile: String,
    pub pick_library: String,
    pub release_query: String,
    pub release_input: String,
    pub show_rejected: bool,
    pub manual_release: Option<Arc<ReleaseCandidate>>,
    pub manual_season: String,
    pub manual_episode: String,
    pub grabbed: HashSet<String>,
    pending_action: Option<Action>,
    previous_dialog: Option<Dialog>,
    pub match_input: String,
    match_serial: u64,
    pub download_category: String,
    pub match_query: String,
    pub provider: String,
    pub download_groups: HashSet<String>,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    search_serial: u64,
    discover_serial: u64,
    pub search_query: String,
    pub search_index: usize,
    pub recent: Vec<Card>,
    recent_file: Option<std::path::PathBuf>,
    recent_loaded: bool,
    recent_writing: bool,
    recent_dirty: bool,
    pub rates: HashMap<String, Vec<f32>>,
    tick_pending: bool,
    image_queue: VecDeque<String>,
    image_active: usize,
    pub draft: SeriesDraft,
}

#[derive(Default)]
pub struct SeriesDraft {
    pub profile: String,
    pub sources: Vec<String>,
    pub groups: String,
    pub aliases: String,
    pub numbering: String,
    pub naming: String,
    pub seeding: String,
    pub inherited_naming: Option<String>,
}
impl SeriesDraft {
    pub fn patch(&self) -> Value {
        let list =
            |s: &str| s.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let seeding = match self.seeding.as_str() {
            "none" => json!({"ratio":0,"then":"REMOVE"}),
            "ratio" => json!({"ratio":1,"then":"REMOVE"}),
            "week" => json!({"time":"7days","then":"REMOVE"}),
            "forever" => json!({"then":"PAUSE"}),
            _ => Value::Null,
        };
        json!({"profile":if self.profile.is_empty(){None}else{Some(&self.profile)},"sources":self.sources,"groups":list(&self.groups),"aliases":list(&self.aliases),"numbering":self.numbering,"naming":if self.naming.trim().is_empty()||self.inherited_naming.as_deref()==Some(self.naming.trim()){None}else{Some(self.naming.trim())},"seeding":seeding})
    }
}

impl Browsing {
    pub fn data(&self, key: &Key) -> Option<&Document> {
        self.cache.get(key)?.data.as_deref()
    }

    pub fn can(&self) -> Permissions {
        self.data(&Key::Shell).and_then(|d| d.viewer.as_ref()).map(|v| v.permissions.clone()).unwrap_or_default()
    }

    pub fn chosen_monitor(&self) -> &str {
        if !self.monitor.is_empty() {
            &self.monitor
        } else {
            self.data(&Key::Options)
                .and_then(|d| d.settings.as_ref())
                .map_or("NONE", |s| s.automation.default_monitor.as_str())
        }
    }

    pub fn viewer(&self) -> Option<&Viewer> {
        self.data(&Key::Shell)?.viewer.as_ref()
    }

    pub fn libraries(&self) -> &[Library] {
        self.data(&Key::Shell).map_or(&[], |d| &d.libraries)
    }

    pub fn features(&self) -> bool {
        self.data(&Key::Shell).and_then(|d| d.server.as_ref()).is_some_and(|s| s.downloads)
    }

    pub fn unread(&self) -> i64 {
        self.data(&Key::Inbox).and_then(|d| d.notifications.as_ref()).map_or(0, |i| i.unread)
    }

    pub fn reset(&mut self, generation: u64) {
        *self = Self { generation, ..Self::default() };
    }

    pub fn follow(&mut self, route: &Route, api: &Api, effects: &mut Vec<Effect>) {
        self.ensure(Key::Shell, api, effects);
        self.ensure(Key::Inbox, api, effects);
        if !self.recent_loaded
            && let Some(viewer) = self.viewer()
        {
            use std::hash::{Hash, Hasher};
            let mut hash = std::hash::DefaultHasher::new();
            (api.origin.as_str(), viewer.id).hash(&mut hash);
            self.recent_file =
                crate::paths::state_dir().map(|dir| dir.join("recents").join(format!("{:016x}.json", hash.finish())));
            self.recent_loaded = true;
            if let Some(path) = self.recent_file.clone() {
                let generation = self.generation;
                effects.push(Effect::Task(Box::pin(async move {
                    let ids = tokio::fs::read(path)
                        .await
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<Vec<i64>>(&bytes).ok())
                        .unwrap_or_default();
                    AppMessage::Browsing(generation, Message::RecentLoaded(ids))
                })));
            }
        }
        if self.recent_dirty
            && !self.recent_writing
            && let Some(path) = self.recent_file.clone()
        {
            self.recent_dirty = false;
            self.recent_writing = true;
            let ids = self.recent.iter().map(|c| c.id).collect::<Vec<_>>();
            let generation = self.generation;
            effects.push(Effect::Task(Box::pin(async move {
                let result = async {
                    if let Some(parent) = path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    let partial = path.with_extension(format!("{}.partial", std::process::id()));
                    tokio::fs::write(&partial, serde_json::to_vec(&ids).expect("recent IDs")).await?;
                    tokio::fs::rename(partial, path).await
                }
                .await;
                if let Err(error) = result {
                    tracing::debug!("can't remember recent titles: {error}");
                }
                AppMessage::Browsing(generation, Message::RecentSaved)
            })));
        }

        match route {
            Route::Home => {
                self.ensure(Key::Home, api, effects);
                if self.libraries().iter().any(|l| l.kind == "MUSIC") {
                    self.ensure(Key::Music, api, effects);
                }
                if self.libraries().iter().any(|l| l.kind == "BOOKS") {
                    self.ensure(Key::BookHome, api, effects);
                }
                if self.features() {
                    self.ensure(Key::Coming, api, effects);
                }
            },
            Route::Library(name) | Route::Series { library: name, .. } => {
                let books = self.libraries().iter().any(|l| l.name == *name && l.kind == "BOOKS");
                self.ensure(if books { Key::Books(name.clone()) } else { Key::Library(name.clone()) }, api, effects);
            },
            Route::Title(id) => {
                for key in [Key::Title(*id as i64), Key::Similar(*id as i64)] {
                    self.ensure(key, api, effects);
                }
                if self.features() {
                    self.ensure(
                        if self.can().manage_shows { Key::Series(*id as i64) } else { Key::Schedule(*id as i64) },
                        api,
                        effects,
                    );
                }
                if self.can().downloads
                    && self
                        .data(&Key::Series(*id as i64))
                        .and_then(|d| d.title.as_ref())
                        .and_then(|t| t.series.as_ref())
                        .is_some_and(|s| s.counts.grabbed > 0)
                {
                    self.ensure(Key::Downloads, api, effects);
                }
            },
            Route::Calendar => {
                self.ensure(Key::Calendar(calendar_start(self.week)), api, effects);
                self.ensure(Key::Soon, api, effects);
            },
            Route::Requests => self.ensure(Key::Requests, api, effects),
            Route::Wanted => self.ensure(Key::Wanted, api, effects),
            Route::Downloads if self.can().downloads => {
                self.ensure(Key::Downloads, api, effects);
                self.ensure(Key::Engine, api, effects);
            },
            Route::Discover if self.can().manage_shows || self.can().request => {
                if self.query.chars().count() != 1 {
                    self.ensure(Key::Discover(self.library.clone(), self.query.clone()), api, effects);
                }
                if self.query.is_empty() {
                    self.ensure(Key::ForYou(self.library.clone()), api, effects);
                }
            },
            _ => {},
        }
        match self.dialog.clone() {
            Some(Dialog::Pick(r)) if self.can().manage_shows => {
                self.ensure(Key::Options, api, effects);
                self.ensure(Key::Aired(r.provider.clone(), r.id.clone()), api, effects);
            },
            Some(Dialog::Releases { series, season, episodes, .. }) => {
                self.ensure(Key::Releases(series, season, episodes, self.release_query.clone()), api, effects)
            },
            Some(Dialog::SeriesSettings(s)) => {
                self.ensure(Key::Options, api, effects);
                self.ensure(Key::Naming(s.id, self.draft.naming.clone()), api, effects);
            },
            Some(Dialog::Match(id)) => {
                self.ensure(Key::Match(id, self.match_query.clone(), self.provider.clone()), api, effects)
            },
            _ => {},
        }
        if !self.search_query.is_empty() {
            self.ensure(Key::Search(self.search_query.clone()), api, effects);
        }
        if !self.tick_pending {
            self.tick_pending = true;
            let generation = self.generation;
            effects.push(Effect::Task(Box::pin(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                AppMessage::Browsing(generation, Message::Tick)
            })));
        }
    }

    fn ensure(&mut self, key: Key, api: &Api, effects: &mut Vec<Effect>) {
        let entry = self.cache.entry(key.clone()).or_default();
        if entry.pending || (!entry.dirty && (entry.data.is_some() || entry.error.is_some())) {
            return;
        }
        self.serial += 1;
        entry.serial = self.serial;
        entry.pending = true;
        entry.dirty = false;
        entry.error = None;
        let serial = self.serial;
        let generation = self.generation;
        let api = api.clone();
        effects.push(Effect::Task(Box::pin(async move {
            let result = load(&api, &key).await.map(Arc::new);
            AppMessage::Browsing(generation, Message::Loaded(generation, key, serial, result))
        })));
    }

    pub fn invalidate(&mut self, predicate: impl Fn(&Key) -> bool) {
        for (key, entry) in &mut self.cache {
            if predicate(key) {
                entry.dirty = true;
            }
        }
    }

    pub fn live(&mut self, live: &api::Live) {
        use api::events::EventsEvents as E;
        match live {
            api::Live::Connected => self.invalidate(|_| true),
            api::Live::Event(event) => match &event.events {
                E::NotificationReceived(_) => self.invalidate(|k| matches!(k, Key::Inbox)),
                E::ConfigChanged(_) => self.invalidate(|_| true),
                E::ListChanged(change) => match change.list {
                    api::events::ChangedList::DOWNLOADS => self.invalidate(|k| {
                        matches!(
                            k,
                            Key::Downloads
                                | Key::Engine
                                | Key::Wanted
                                | Key::Coming
                                | Key::Calendar(_)
                                | Key::Soon
                                | Key::Series(_)
                        )
                    }),
                    api::events::ChangedList::NOTIFICATIONS => self.invalidate(|k| matches!(k, Key::Inbox)),
                    api::events::ChangedList::REQUESTS => self.invalidate(|k| {
                        matches!(
                            k,
                            Key::Requests
                                | Key::Discover(..)
                                | Key::ForYou(_)
                                | Key::Coming
                                | Key::Calendar(_)
                                | Key::Soon
                        )
                    }),
                    api::events::ChangedList::USERS => self.invalidate(|_| true),
                    _ => {},
                },
                E::LibraryChanged(_) | E::ScanFinished(_) | E::MetadataChanged(_) | E::EpisodesImported(_) => {
                    self.invalidate(|_| true)
                },
                E::SeriesChanged(_) => self.invalidate(|k| {
                    matches!(
                        k,
                        Key::Series(_)
                            | Key::Schedule(_)
                            | Key::Wanted
                            | Key::Calendar(_)
                            | Key::Soon
                            | Key::Coming
                            | Key::Discover(..)
                    )
                }),
                _ => {},
            },
            _ => {},
        }
    }

    pub fn search(&mut self, query: String, effects: &mut Vec<Effect>) {
        self.search_index = 0;
        self.search_serial += 1;
        let serial = self.search_serial;
        let generation = self.generation;
        if query.trim().is_empty() {
            self.search_query.clear();
            return;
        }
        effects.push(Effect::Task(Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            AppMessage::Browsing(generation, Message::SearchReady(serial, query.trim().to_owned()))
        })));
    }

    pub fn update(&mut self, message: Message, api: &Api, route: &Route, effects: &mut Vec<Effect>) -> Option<Route> {
        match message {
            Message::Loaded(generation, key, serial, result) => {
                if generation != self.generation {
                    return None;
                }
                let entry = self.cache.get_mut(&key).filter(|e| e.serial == serial)?;
                entry.pending = false;
                entry.fetched = Some(Instant::now());
                match result {
                    Ok(data) => {
                        let mut rates = Vec::new();
                        if let Some(e) = &data.download_engine {
                            rates.push(("download".to_owned(), e.download_rate));
                            rates.push(("upload".to_owned(), e.upload_rate));
                        }
                        for d in &data.downloads {
                            if let Some(l) = &d.live {
                                rates.push((format!("dl{}", d.id), l.download_rate));
                            }
                        }
                        for (key, rate) in rates {
                            let values = self.rates.entry(key).or_default();
                            values.push(rate as f32);
                            if values.len() > 90 {
                                values.remove(0);
                            }
                        }

                        if let Key::Title(_) = key
                            && let Some(title) = &data.title
                        {
                            self.recent.retain(|c| c.id != title.card.id);
                            self.recent.insert(0, title.card.clone());
                            self.recent.truncate(12);
                            self.recent_dirty = true;
                        }
                        if let Key::Recents(ids) = &key {
                            self.recent.retain(|c| !ids.contains(&c.id) || data.titles.iter().any(|t| t.id == c.id));
                            for card in &data.titles {
                                if !self.recent.iter().any(|c| c.id == card.id) {
                                    self.recent.push(card.clone());
                                }
                            }
                            self.recent.truncate(12);
                        }
                        entry.updates = entry.updates.saturating_add(1);
                        entry.data = Some(data);
                        entry.error = None;
                    },
                    Err(error) => {
                        entry.error = Some(error.message);
                        if error.unauthorized {
                            return None;
                        }
                    },
                }
            },
            Message::Mutated(generation, result) if generation == self.generation => {
                self.busy = false;
                match result {
                    Ok(()) => {
                        let grabbed = match self.pending_action.take() {
                            Some(Action::Grab(_, r)) => {
                                self.grabbed.insert(r.release.link.clone());
                                self.manual_release = None;
                                true
                            },
                            _ => false,
                        };
                        if grabbed {
                            if self.previous_dialog.is_some() {
                                self.dialog = self.previous_dialog.take();
                            }
                        } else {
                            self.dialog = None;
                            self.previous_dialog = None;
                        }

                        self.notice = self.pending_notice.take();
                        self.notice_at = Some(Instant::now());
                        self.error = None;
                        self.invalidate(|k| !grabbed || !matches!(k, Key::Releases(..)));
                    },
                    Err(error) => {
                        self.error = Some(error.message);
                        self.undo = None;
                        if matches!(self.dialog, Some(Dialog::Confirm { .. })) {
                            self.dialog = self.previous_dialog.take();
                        }
                    },
                }
            },
            Message::Picture(generation, path, handle, fresh) if generation == self.generation => {
                self.images.arrived(path, handle, fresh);
                self.images.limit_ready(256);
                if fresh {
                    self.image_active = self.image_active.saturating_sub(1);
                    self.fetch_pictures(api, effects);
                }
            },
            Message::RecentLoaded(ids) => {
                if !ids.is_empty() {
                    self.ensure(Key::Recents(ids.into_iter().take(12).collect()), api, effects);
                }
            },
            Message::InboxSettled(result) => {
                self.invalidate(|k| matches!(k, Key::Inbox));
                if let Err(error) = result {
                    self.error = Some(error.message);
                }
            },
            Message::RecentSaved => self.recent_writing = false,
            Message::WantImage(path) => self.pictures(vec![path], api, effects),
            Message::Retry(key) => {
                if let Some(e) = self.cache.get_mut(&key) {
                    e.dirty = true;
                }
            },
            Message::FindShows(query) => {
                self.discover_input = query.clone();
                self.query = query;
                return Some(Route::Discover);
            },
            Message::Query(query) => {
                self.discover_serial += 1;
                let serial = self.discover_serial;
                let generation = self.generation;
                self.discover_input = query.clone();
                effects.push(Effect::Task(Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    AppMessage::Browsing(generation, Message::DiscoverReady(serial, query.trim().to_owned()))
                })));
            },
            Message::DiscoverReady(serial, query) if serial == self.discover_serial => {
                self.previous_discover = Some(Key::Discover(self.library.clone(), self.query.clone()));
                self.query = query;
            },
            Message::SearchReady(serial, query) if serial == self.search_serial => self.search_query = query,
            Message::Library(library) => self.library = library,
            Message::Category(category) => self.category = category,
            Message::Filter(filter) => {
                self.filter.insert(route.clone(), filter);
            },
            Message::BookSort(sort) => {
                self.book_sort.insert(route.clone(), sort);
            },
            Message::Sort(newest) => {
                if newest {
                    self.newest.insert(route.clone());
                } else {
                    self.newest.remove(route);
                }
            },
            Message::Overview(id) => {
                if !self.expanded.remove(&id) {
                    self.expanded.insert(id);
                }
            },
            Message::Season(season) => {
                if let Route::Title(id) = route {
                    self.seasons.insert(*id as i64, season);
                }
            },
            Message::Week(offset) => self.week = offset,
            Message::Dialog(dialog) => {
                self.error = None;
                if dialog.is_none()
                    && matches!(self.dialog, Some(Dialog::Confirm { .. }))
                    && self.previous_dialog.is_some()
                {
                    self.dialog = self.previous_dialog.take();
                    return None;
                }
                if matches!(dialog, Some(Dialog::Confirm { .. })) {
                    self.previous_dialog = self.dialog.clone();
                    self.dialog = dialog;
                    return None;
                }
                self.previous_dialog = None;
                self.note.clear();
                self.profile.clear();
                self.match_serial += 1;
                self.release_query.clear();
                self.release_input.clear();
                self.match_input.clear();
                self.match_query.clear();
                self.provider.clear();
                self.manual_release = None;
                self.show_rejected = false;
                self.grabbed.clear();
                if let Some(Dialog::Pick(r)) = &dialog {
                    self.pick_library = r.library.clone();
                    self.monitor.clear();
                }
                if let Some(Dialog::SeriesSettings(s)) = &dialog {
                    self.draft = SeriesDraft {
                        profile: s.profile.clone().unwrap_or_default(),
                        sources: s.sources.clone(),
                        groups: s.groups.join(", "),
                        aliases: s.aliases.join(", "),
                        numbering: s.numbering.clone(),
                        naming: s.naming.clone().unwrap_or_else(|| s.style.file.clone()),
                        inherited_naming: if s.naming.is_none() { Some(s.style.file.clone()) } else { None },
                        seeding: match &s.seeding {
                            None => "default",
                            Some(r) if r.ratio == Some(0.0) => "none",
                            Some(r) if r.ratio == Some(1.0) && r.time.is_none() && r.idle.is_none() => "ratio",
                            Some(r) if r.ratio.is_none() && r.time.is_some() && r.idle.is_none() => "week",
                            Some(r) if r.ratio.is_none() && r.time.is_none() && r.idle.is_none() => "forever",
                            _ => "default",
                        }
                        .into(),
                    };
                }
                self.dialog = dialog;
            },
            Message::Note(note) => self.note = note,
            Message::Monitor(monitor) => self.monitor = monitor,
            Message::Profile(profile) => self.profile = profile,
            Message::PickLibrary(library) => self.pick_library = library,
            Message::ReleaseQuery(query) => self.release_input = query,
            Message::ReleaseSubmit => {
                self.release_query = self.release_input.trim().to_owned();
                self.manual_release = None;
                if let Some(Dialog::Releases { series, season, episodes, .. }) = &self.dialog {
                    let key = Key::Releases(*series, *season, episodes.clone(), self.release_query.clone());
                    if let Some(entry) = self.cache.get_mut(&key) {
                        entry.dirty = true;
                        entry.data = None;
                    }
                }
            },
            Message::ShowRejected => self.show_rejected = !self.show_rejected,
            Message::ManualRelease(release) => {
                self.manual_release = release;
                if let Some(Dialog::Releases { season, episodes, .. }) = &self.dialog {
                    self.manual_season = season.to_string();
                    self.manual_episode = if episodes.len() == 1 { episodes[0].to_string() } else { String::new() };
                }
            },
            Message::ManualSeason(season) => self.manual_season = season,
            Message::ManualEpisode(episode) => self.manual_episode = episode,
            Message::MatchReady(serial, query) if serial == self.match_serial => self.match_query = query,
            Message::DownloadCategory(category) => self.download_category = category,
            Message::MatchQuery(query) => {
                self.match_input = query.clone();
                self.match_serial += 1;
                let serial = self.match_serial;
                let generation = self.generation;
                effects.push(Effect::Task(Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    AppMessage::Browsing(generation, Message::MatchReady(serial, query))
                })));
            },
            Message::Provider(provider) => self.provider = provider,
            Message::Follow(n) => {
                self.act(Action::Read(Some(vec![n.id])), api, effects);
                return n.link.and_then(|link| Route::parse(&link));
            },
            Message::DownloadGroup(key) => {
                if !self.download_groups.remove(&key) {
                    self.download_groups.insert(key);
                }
            },
            Message::Draft(field, value) => match field {
                "profile" => self.draft.profile = value,
                "groups" => self.draft.groups = value,
                "aliases" => self.draft.aliases = value,
                "numbering" => self.draft.numbering = value,
                "naming" => self.draft.naming = value,
                "seeding" => self.draft.seeding = value,
                _ => {},
            },
            Message::Source(source) => {
                if source.is_empty() {
                    self.draft.sources.clear();
                } else if let Some(i) = self.draft.sources.iter().position(|s| *s == source) {
                    self.draft.sources.remove(i);
                } else {
                    self.draft.sources.push(source);
                }
            },
            Message::Act(action) => self.act(action, api, effects),
            Message::Shelf(id, viewport) => {
                self.shelves.insert(id, viewport);
            },
            Message::PageShelf(id, direction) => {
                let width = self.shelves.get(&id).map_or(800.0, |v| v.bounds.width);
                let animation = if self.reduce_motion {
                    iced_core::widget::operation::Animation::Instant
                } else {
                    iced_core::widget::operation::Animation::default()
                };
                effects.push(Effect::Operate(Box::new(iced_core::widget::operation::scrollable::scroll_by(
                    id.into(),
                    iced_core::widget::operation::scrollable::AbsoluteOffset { x: width * direction * 0.85, y: 0.0 },
                    animation,
                ))));
            },
            Message::Tick => {
                self.tick_pending = false;
                if self.notice_at.is_some_and(|at| at.elapsed().as_secs() >= 5) {
                    self.notice = None;
                    self.undo = None;
                    self.notice_at = None;
                }
                for (key, entry) in &mut self.cache {
                    let downloading =
                        entry.data.as_ref().is_some_and(|d| d.calendar.iter().any(|e| e.download.is_some()));
                    let interval = match key {
                        Key::Downloads | Key::Engine => 2,
                        Key::Calendar(_) => {
                            if downloading {
                                2
                            } else {
                                60
                            }
                        },
                        Key::Soon => {
                            if downloading {
                                2
                            } else if entry.data.as_ref().is_some_and(|d| {
                                d.calendar
                                    .iter()
                                    .any(|e| e.air_at <= now() && e.air_at > now() - 12 * 3600 && e.state == "WANTED")
                            }) {
                                10
                            } else {
                                60
                            }
                        },
                        Key::Coming => {
                            if downloading {
                                3
                            } else {
                                300
                            }
                        },
                        Key::Title(_)
                            if entry.updates < 4
                                && entry.data.as_ref().and_then(|d| d.title.as_ref()).is_some_and(|t| {
                                    if t.card.backdrop.is_some() {
                                        t.card.backdrop_tint.is_none()
                                    } else {
                                        t.card.poster.is_some() && t.card.poster_tint.is_none()
                                    }
                                }) =>
                        {
                            2
                        },
                        Key::Wanted => 10,
                        Key::Requests => 15,
                        Key::Inbox => 60,
                        _ => continue,
                    };
                    if entry.fetched.is_some_and(|t| t.elapsed().as_secs() >= interval) {
                        entry.dirty = true;
                    }
                }
            },
            _ => {},
        }
        None
    }

    fn pictures(&mut self, paths: Vec<String>, api: &Api, effects: &mut Vec<Effect>) {
        for path in paths {
            if !self.images.want(&path) {
                continue;
            }
            self.image_queue.push_front(path);
        }
        self.fetch_pictures(api, effects);
    }

    fn fetch_pictures(&mut self, api: &Api, effects: &mut Vec<Effect>) {
        while self.image_active < 6 {
            let Some(path) = self.image_queue.pop_front() else { break };
            self.image_active += 1;
            let api = api.clone();
            let generation = self.generation;
            effects.push(Effect::Listen(Box::new(move |send| {
                tokio::spawn(async move {
                    if let Some(bytes) = images::cached(&api, &path).await {
                        send(AppMessage::Browsing(
                            generation,
                            Message::Picture(generation, path.clone(), images::decode(bytes).await, false),
                        ));
                    }
                    let handle = match images::fetch(&api, &path).await {
                        Some(bytes) => images::decode(bytes).await,
                        None => None,
                    };
                    send(AppMessage::Browsing(generation, Message::Picture(generation, path, handle, true)));
                });
            })));
        }
    }

    fn act(&mut self, action: Action, api: &Api, effects: &mut Vec<Effect>) {
        if matches!(action, Action::Read(_) | Action::DeleteNotice(_)) {
            if let Some(inbox) = self
                .cache
                .get_mut(&Key::Inbox)
                .and_then(|e| e.data.as_mut())
                .and_then(|d| Arc::make_mut(d).notifications.as_mut())
            {
                match &action {
                    Action::Read(ids) => {
                        for n in &mut inbox.items {
                            if n.read_at.is_none() && ids.as_ref().is_none_or(|ids| ids.contains(&n.id)) {
                                n.read_at = Some(now());
                            }
                        }
                    },
                    Action::DeleteNotice(id) => inbox.items.retain(|n| id.is_some_and(|id| id != n.id)),
                    _ => {},
                }
                inbox.unread = inbox.items.iter().filter(|n| n.read_at.is_none()).count() as i64;
            }
            let api = api.clone();
            let generation = self.generation;
            effects.push(Effect::Task(Box::pin(async move {
                AppMessage::Browsing(generation, Message::InboxSettled(mutate(&api, action).await))
            })));
            return;
        }
        if self.busy {
            return;
        }
        self.undo = match &action {
            Action::TitleWatched(id, _) => self.data(&Key::Title(*id)).and_then(|d| d.title.as_ref()).map(|t| {
                Action::RestoreTitle(
                    *id,
                    t.seasons
                        .iter()
                        .flat_map(|s| &s.episodes)
                        .chain(t.movie.iter())
                        .filter(|v| v.finished)
                        .map(|v| v.id)
                        .collect(),
                )
            }),
            Action::Watched(ids, _) => {
                let mut watched = Vec::new();
                let mut unwatched = Vec::new();
                for data in self.cache.values().filter_map(|e| e.data.as_deref()) {
                    if let Some(t) = &data.title {
                        for v in t
                            .seasons
                            .iter()
                            .flat_map(|s| &s.episodes)
                            .chain(t.movie.iter())
                            .filter(|v| ids.contains(&v.id))
                        {
                            if v.finished {
                                watched.push(v.id);
                            } else {
                                unwatched.push(v.id);
                            }
                        }
                    }
                }
                Some(Action::RestoreEpisodes(watched, unwatched))
            },
            _ => None,
        };
        self.pending_notice = match action {
            Action::Read(_) | Action::DeleteNotice(_) | Action::Artwork { .. } => None,
            Action::TitleWatched(_, true) | Action::Watched(_, true) => Some("Marked watched".into()),
            Action::TitleWatched(_, false) | Action::Watched(_, false) => Some("Marked unwatched".into()),
            _ => Some("Saved".into()),
        };
        self.pending_action = Some(action.clone());
        self.busy = true;
        self.error = None;
        self.notice = None;
        let api = api.clone();
        let generation = self.generation;
        effects.push(Effect::Task(Box::pin(async move {
            AppMessage::Browsing(generation, Message::Mutated(generation, mutate(&api, action).await))
        })));
    }
}

/// `value` as an operation's generated variables. Operations without any get a unit struct, which
/// only `null` reads as.
fn variables<V: DeserializeOwned>(value: Value) -> Result<V, api::Error> {
    let value = if value.as_object().is_some_and(serde_json::Map::is_empty) { Value::Null } else { value };
    serde_json::from_value(value).map_err(|e| api::Error::message(format!("Invalid request: {e}")))
}

async fn query<Q: GraphQLQuery>(api: &Api, value: Value) -> Result<Document, api::Error>
where
    Q::Variables: DeserializeOwned,
{
    api.request_data::<Q, Document>(variables(value)?).await
}

async fn mutation<Q: GraphQLQuery>(api: &Api, value: Value) -> Result<(), api::Error>
where
    Q::Variables: DeserializeOwned,
{
    api.request::<Q>(variables(value)?).await.map(|_| ())
}

async fn load(api: &Api, key: &Key) -> Result<Document, api::Error> {
    use api::*;
    let null_string = |s: &str| if s.is_empty() { Value::Null } else { json!(s) };
    match key {
        Key::Shell => {
            let data = query::<BrowseShell>(api, json!({})).await?;
            if data.viewer.is_none() { Err(Error::unauthorized()) } else { Ok(data) }
        },
        Key::Home => query::<BrowseHome>(api, json!({})).await,
        Key::BookHome => query::<BrowseBookHome>(api, json!({})).await,
        Key::Music => query::<BrowseMusicHome>(api, json!({})).await,
        Key::Coming => query::<BrowseComingUp>(api, json!({"from":now()-3600,"to":now()+7*86400})).await,
        Key::Library(name) => query::<BrowseLibrary>(api, json!({"name":name})).await,
        Key::Books(name) => query::<BrowseBooks>(api, json!({"library":name})).await,
        Key::Title(id) => query::<BrowseTitle>(api, json!({"id":id})).await,
        Key::Similar(id) => query::<BrowseSimilar>(api, json!({"id":id})).await,
        Key::Schedule(id) => query::<BrowseTitleSchedule>(api, json!({"id":id})).await,
        Key::Series(id) => query::<BrowseTitleSeries>(api, json!({"id":id})).await,
        Key::Calendar(start) => query::<BrowseCalendar>(api, json!({"from":start,"to":shift_days(*start,7)})).await,
        Key::Soon => query::<BrowseCalendar>(api, json!({"from":now()-12*3600,"to":now()+30*86400})).await,
        Key::Requests => query::<BrowseRequests>(api, json!({})).await,
        Key::Wanted => query::<BrowseWanted>(api, json!({})).await,
        Key::Downloads => query::<BrowseDownloads>(api, json!({})).await,
        Key::Engine => query::<BrowseEngine>(api, json!({})).await,
        Key::Inbox => query::<BrowseInbox>(api, json!({})).await,
        Key::Discover(library, q) => {
            query::<BrowseDiscover>(api, json!({"library":null_string(library),"query":q})).await
        },
        Key::ForYou(library) => query::<BrowseForYou>(api, json!({"library":null_string(library)})).await,
        Key::Recents(ids) => query::<BrowseRecents>(api, json!({"ids":ids})).await,
        Key::Search(q) => query::<BrowseSearch>(api, json!({"query":q})).await,
        Key::Naming(id, file) => query::<BrowseNamingPreview>(api, json!({"id":id,"file":file})).await,
        Key::Options => query::<BrowseOptions>(api, json!({})).await,
        Key::Aired(provider, id) => query::<BrowseAiredEpisodes>(api, json!({"provider":provider,"id":id})).await,
        Key::Releases(id, season, episodes, q) => {
            query::<BrowseReleases>(
                api,
                json!({"seriesId":id,"season":season,"episodes":episodes,"query":null_string(q)}),
            )
            .await
        },
        Key::Match(id, q, provider) => {
            query::<BrowseMatchCandidates>(
                api,
                json!({"id":id,"query":null_string(q),"provider":null_string(provider)}),
            )
            .await
        },
    }
}

async fn mutate(api: &Api, action: Action) -> Result<(), api::Error> {
    use api::*;
    match action {
        Action::Watched(ids, watched) => mutation::<BrowseSetWatched>(api, json!({"videoIds":ids,"watched":watched})).await,
        Action::RestoreTitle(id,ids)=>{
            mutation::<BrowseSetTitleWatched>(api,json!({"id":id,"watched":false})).await?;
            if !ids.is_empty(){mutation::<BrowseSetWatched>(api,json!({"videoIds":ids,"watched":true})).await?;} Ok(())
        },
        Action::RestoreEpisodes(watched,unwatched)=>{
            if !watched.is_empty(){mutation::<BrowseSetWatched>(api,json!({"videoIds":watched,"watched":true})).await?;}
            if !unwatched.is_empty(){mutation::<BrowseSetWatched>(api,json!({"videoIds":unwatched,"watched":false})).await?;}Ok(())
        },
        Action::TitleWatched(id, watched) => mutation::<BrowseSetTitleWatched>(api, json!({"id":id,"watched":watched})).await,
        Action::Refresh(id) => mutation::<BrowseRefreshTitle>(api, json!({"id":id})).await,
        Action::Approve(id) => mutation::<BrowseApproveRequest>(api, json!({"id":id})).await,
        Action::Decline(id) => mutation::<BrowseDeclineRequest>(api, json!({"id":id})).await,
        Action::CancelRequest(id) => mutation::<BrowseDeleteRequest>(api, json!({"id":id})).await,
        Action::Bulk(pause)=>{
            let data=load(api,&Key::Downloads).await?;
            let ids=data.downloads.iter().filter(|d|if pause{matches!(d.state.as_str(),"DOWNLOADING"|"SEEDING")}else{d.state=="PAUSED"}).map(|d|d.id).collect::<Vec<_>>();
            if pause{mutation::<BrowseDownloadsPause>(api,json!({"ids":ids})).await}else{mutation::<BrowseDownloadsResume>(api,json!({"ids":ids})).await}
        },
        Action::Pause(ids) => mutation::<BrowseDownloadsPause>(api, json!({"ids":ids})).await,
        Action::Resume(ids) => mutation::<BrowseDownloadsResume>(api, json!({"ids":ids})).await,
        Action::Recheck(ids) => mutation::<BrowseDownloadsRecheck>(api, json!({"ids":ids})).await,
        Action::Import(id) => mutation::<BrowseImportDownload>(api, json!({"id":id})).await,
        Action::Remove(ids, delete) => mutation::<BrowseRemoveDownloads>(api, json!({"ids":ids,"deleteFiles":delete})).await,
        Action::Read(ids) => mutation::<BrowseMarkNotificationsRead>(api, json!({"ids":ids})).await,
        Action::DeleteNotice(id) => mutation::<BrowseDeleteNotifications>(api, json!({"id":id})).await,
        Action::Add(r, library, monitor, profile) => mutation::<BrowseAddSeries>(api, json!({"input":{"library":library,"provider":r.provider,"providerId":r.id,"name":r.name,"year":r.year,"poster":r.poster,"overview":r.overview,"monitor":monitor,"profile":if profile.is_empty(){None}else{Some(profile)}}})).await,
        Action::Request(r, note) => mutation::<BrowseCreateRequest>(api, json!({"input":{"library":r.library,"providerId":r.id,"name":r.name,"year":r.year,"poster":r.poster,"overview":r.overview,"note":if note.is_empty(){None}else{Some(note)}}})).await,
        Action::Manage(id) => mutation::<BrowseManageTitle>(api, json!({"titleId":id})).await,
        Action::UpdateSeries(id, patch) => mutation::<BrowseUpdateSeries>(api, json!({"id":id,"patch":patch})).await,
        Action::RefreshSchedule(id) => mutation::<BrowseRefreshSeriesSchedule>(api, json!({"id":id})).await,
        Action::RemoveSeries(id) => mutation::<BrowseRemoveSeries>(api, json!({"id":id})).await,
        Action::LookAgain(id, season, episode) => mutation::<BrowseLookForAgain>(api, json!({"seriesId":id,"season":season,"episode":episode})).await,
        Action::DeleteDownloaded(id, season) => {
            let variables=serde_json::from_value(json!({"seriesId":id,"season":season})).map_err(|e|Error::message(format!("Invalid request: {e}")))?;
            let data=api.request::<BrowseDeleteDownloaded>(variables).await?;
            if data.delete_downloaded.problems.is_empty(){Ok(())}else{Err(Error::message(data.delete_downloaded.problems.join("\n")))}
        },
        Action::Grab(id, r) => mutation::<BrowseGrabRelease>(api, json!({"seriesId":id,"episodes":r.episodes.iter().map(|e|json!({"season":e.season,"episode":e.episode})).collect::<Vec<_>>(),"release":{"title":r.release.title,"source":r.release.source,"link":r.release.link,"infoHash":r.release.info_hash,"size":r.release.size,"seeders":r.release.seeders,"leechers":r.release.leechers,"published":r.release.published,"page":r.release.page}})).await,
        Action::Match(id, provider, provider_id) => mutation::<BrowseMatchTitle>(api, json!({"id":id,"provider":provider,"providerId":provider_id})).await,
        Action::Artwork { id, kind, reset } => api.artwork(id, kind, reset).await,
    }
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

pub fn shift_days(at: i64, days: i64) -> i64 {
    use crate::platform::{Current, Desktop};
    let offset = Current::utc_offset(at);
    let wall = ((at + offset).div_euclid(86400) + days) * 86400;
    wall - Current::utc_offset(wall - offset)
}
pub fn week_start(at: i64) -> i64 {
    use crate::platform::{Current, Desktop};
    let day = (at + Current::utc_offset(at)).div_euclid(86400);
    shift_days(at, -(day + 3).rem_euclid(7))
}
pub fn calendar_start(week: i64) -> i64 {
    shift_days(week_start(now()), week * 7)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api() -> Api {
        Api::new(url::Url::parse("http://localhost:3000").unwrap(), None).unwrap()
    }

    #[test]
    fn inbox_reads_and_removals_are_immediate_even_during_another_mutation() {
        let mut b = Browsing { busy: true, ..Default::default() };
        let data: Document =
            serde_json::from_value(json!({"notifications":{"unread":2,"items":[{"id":1},{"id":2}]}})).unwrap();
        b.cache.insert(Key::Inbox, Entry { data: Some(Arc::new(data)), ..Default::default() });
        let mut effects = Vec::new();
        let api = api();
        b.act(Action::Read(Some(vec![1])), &api, &mut effects);
        assert_eq!(b.unread(), 1);
        b.act(Action::DeleteNotice(Some(2)), &api, &mut effects);
        assert_eq!(b.unread(), 0);
        assert_eq!(b.data(&Key::Inbox).unwrap().notifications.as_ref().unwrap().items.len(), 1);
        assert!(b.busy);
        assert_eq!(effects.len(), 2);
    }

    #[test]
    fn grabbing_a_release_returns_to_its_results_without_losing_the_query() {
        let mut b = Browsing {
            dialog: Some(Dialog::Releases { series: 1, season: 2, episodes: vec![3], name: "Example".into() }),
            release_query: "query".into(),
            ..Default::default()
        };
        let api = api();
        let mut effects = Vec::new();
        let candidate = Arc::new(ReleaseCandidate {
            release: Release { link: "release-link".into(), ..Default::default() },
            ..Default::default()
        });
        let action = Action::Grab(1, candidate);
        b.update(
            Message::Dialog(Some(Dialog::Confirm {
                title: "Download?".into(),
                body: String::new(),
                action: Arc::new(action.clone()),
            })),
            &api,
            &Route::Wanted,
            &mut effects,
        );
        b.act(action, &api, &mut effects);
        b.update(Message::Mutated(0, Ok(())), &api, &Route::Wanted, &mut effects);
        assert!(matches!(b.dialog, Some(Dialog::Releases { series: 1, .. })));
        assert_eq!(b.release_query, "query");
        assert!(b.grabbed.contains("release-link"));
    }

    #[test]
    fn calendar_boundaries_stay_at_local_midnight_across_offset_changes() {
        use crate::platform::{Current, Desktop};
        for at in [1711846800, 1729990800] {
            let start = week_start(at);
            let local = start + Current::utc_offset(start);
            assert_eq!(local.rem_euclid(86400), 0);
            assert_eq!((local.div_euclid(86400) + 3).rem_euclid(7), 0);
            let next = shift_days(start, 7);
            assert_eq!((next + Current::utc_offset(next)).rem_euclid(86400), 0);
        }
    }

    #[test]
    fn images_are_deduplicated_and_fetched_with_bounded_concurrency() {
        let mut b = Browsing::default();
        let mut effects = Vec::new();
        let api = api();
        b.pictures((0..20).map(|i| format!("image/{i}")).collect(), &api, &mut effects);
        b.pictures(vec!["image/0".into()], &api, &mut effects);
        assert_eq!(effects.len(), 6);
        assert_eq!(b.image_active, 6);
        assert_eq!(b.image_queue.len(), 14);
        b.update(Message::Picture(0, "image/0".into(), None, true), &api, &Route::Home, &mut effects);
        assert_eq!(effects.len(), 7);
        assert_eq!(b.image_active, 6);
        assert_eq!(b.image_queue.len(), 13);
    }

    #[test]
    fn null_optional_request_and_download_fields_decode() {
        let d:Document=serde_json::from_value(json!({"requests":[{"library":null}],"downloads":[{"source":null,"size":null,"seedGoal":{"ratio":null,"seconds":null}}],"books":[{"reading":{"position":1,"progress":0.5,"finished":false,"updatedAt":9}}]})).unwrap();
        assert!(d.requests[0].library.is_none());
        assert!(d.downloads[0].source.is_none());
        assert_eq!(d.books[0].reading.as_ref().unwrap().updated_at, 9);
    }

    #[test]
    fn series_settings_preserve_server_source_names_and_encode_duration_presets() {
        let draft = SeriesDraft {
            numbering: "AUTO".into(),
            sources: vec!["A server source".into()],
            groups: "Group A, Group B, ".into(),
            seeding: "week".into(),
            ..Default::default()
        };
        let patch = draft.patch();
        assert_eq!(patch["sources"], json!(["A server source"]));
        assert_eq!(patch["groups"], json!(["Group A", "Group B"]));
        let _: api::browse_update_series::Variables = serde_json::from_value(json!({"id":1,"patch":patch})).unwrap();
    }

    #[test]
    fn download_events_do_not_refetch_unrelated_library_pages() {
        let mut b = Browsing::default();
        for key in [Key::Home, Key::Downloads, Key::Inbox, Key::Library("Shows".into())] {
            b.cache.insert(key, Entry::default());
        }
        b.live(&api::Live::Event(Arc::new(
            serde_json::from_value(json!({"events":{"__typename":"ListChanged","list":"DOWNLOADS"}})).unwrap(),
        )));
        assert!(b.cache[&Key::Downloads].dirty);
        assert!(!b.cache[&Key::Home].dirty);
        assert!(!b.cache[&Key::Library("Shows".into())].dirty);
    }

    #[test]
    fn progress_saved_after_the_player_is_closed_invalidates_cached_watched_state() {
        let mut app = crate::app::App::new(Route::Home, crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        app.session.api = Some(api());
        app.browsing.cache.insert(Key::Home, Entry { data: Some(Arc::new(Document::default())), ..Default::default() });
        app.update(crate::app::Message::Player(999, crate::player::Message::Saved), &mut Vec::new());
        assert!(app.browsing.cache[&Key::Home].pending);
    }

    #[test]
    fn back_reuses_data_and_invalidation_during_a_request_is_not_lost() {
        let mut b = Browsing::default();
        let api = api();
        let mut effects = Vec::new();
        b.ensure(Key::Home, &api, &mut effects);
        assert_eq!(effects.len(), 1);
        let serial = b.cache[&Key::Home].serial;
        b.invalidate(|k| *k == Key::Home);
        b.update(
            Message::Loaded(0, Key::Home, serial, Ok(Arc::new(Document::default()))),
            &api,
            &Route::Home,
            &mut effects,
        );
        b.ensure(Key::Home, &api, &mut effects);
        assert_eq!(effects.len(), 2);
        let serial = b.cache[&Key::Home].serial;
        b.update(
            Message::Loaded(0, Key::Home, serial, Ok(Arc::new(Document::default()))),
            &api,
            &Route::Home,
            &mut effects,
        );
        effects.clear();
        b.ensure(Key::Home, &api, &mut effects);
        assert!(effects.is_empty());
    }

    #[test]
    fn server_switch_and_superseded_responses_cannot_replace_current_data() {
        let mut b = Browsing::default();
        let api = api();
        let mut effects = Vec::new();
        b.ensure(Key::Home, &api, &mut effects);
        b.reset(1);
        b.update(Message::Loaded(0, Key::Home, 1, Ok(Arc::new(Document::default()))), &api, &Route::Home, &mut effects);
        assert!(b.data(&Key::Home).is_none());
        b.ensure(Key::Home, &api, &mut effects);
        let serial = b.cache[&Key::Home].serial;
        b.update(
            Message::Loaded(1, Key::Home, serial + 1, Ok(Arc::new(Document::default()))),
            &api,
            &Route::Home,
            &mut effects,
        );
        assert!(b.data(&Key::Home).is_none());
    }

    #[test]
    fn subscription_ack_invalidates_every_cached_screen() {
        let mut b = Browsing::default();
        for key in [Key::Home, Key::Inbox, Key::Library("Shows".into()), Key::Title(1)] {
            b.cache.insert(key, Entry { data: Some(Arc::new(Document::default())), ..Default::default() });
        }
        b.live(&api::Live::Connected);
        assert!(b.cache.values().all(|e| e.dirty));
    }

    #[test]
    fn every_screen_and_action_builds_variables_the_server_can_be_sent() {
        let keys = vec![
            Key::Shell,
            Key::Home,
            Key::Music,
            Key::BookHome,
            Key::Coming,
            Key::Library("Shows".into()),
            Key::Books("Books".into()),
            Key::Title(1),
            Key::Similar(1),
            Key::Series(1),
            Key::Schedule(1),
            Key::Calendar(0),
            Key::Soon,
            Key::Requests,
            Key::Wanted,
            Key::Downloads,
            Key::Engine,
            Key::Inbox,
            Key::Discover(String::new(), String::new()),
            Key::Discover("Shows".into(), "orbit".into()),
            Key::ForYou(String::new()),
            Key::Search("orbit".into()),
            Key::Recents(vec![1]),
            Key::Options,
            Key::Aired("TMDB".into(), "1".into()),
            Key::Releases(1, 1, vec![1], String::new()),
            Key::Match(1, String::new(), String::new()),
            Key::Naming(1, "a.mkv".into()),
        ];
        let found = Arc::new(Discover { provider: "TMDB".into(), id: "1".into(), ..Default::default() });
        let actions = vec![
            Action::Watched(vec![1], true),
            Action::TitleWatched(1, true),
            Action::RestoreTitle(1, vec![1]),
            Action::RestoreEpisodes(vec![1], vec![2]),
            Action::Refresh(1),
            Action::Approve(1),
            Action::Decline(1),
            Action::CancelRequest(1),
            Action::Pause(vec![1]),
            Action::Bulk(true),
            Action::Resume(vec![1]),
            Action::Recheck(vec![1]),
            Action::Import(1),
            Action::Remove(vec![1], false),
            Action::Read(None),
            Action::Read(Some(vec![1])),
            Action::DeleteNotice(None),
            Action::DeleteNotice(Some(1)),
            Action::Add(found.clone(), "Shows".into(), "MISSING".into(), String::new()),
            Action::Request(found, String::new()),
            Action::Manage(1),
            Action::UpdateSeries(1, json!({"monitor":"MISSING"})),
            Action::RefreshSchedule(1),
            Action::RemoveSeries(1),
            Action::LookAgain(1, Some(1), None),
            Action::DeleteDownloaded(1, None),
            Action::Grab(1, Arc::new(ReleaseCandidate::default())),
            Action::Match(1, "TMDB".into(), "1".into()),
        ];
        crate::test_support::run(async move {
            // Refusing everything makes each call send exactly one request, so one that never
            // leaves fails its own assertion instead of leaving the server waiting.
            let (origin, server) = crate::test_support::http(
                keys.len() + actions.len(),
                |_| json!({"data":null,"errors":[{"message":"refused"}]}),
            )
            .await;
            let api = Api::new(origin, Some(api::Token("test-session".into()))).unwrap();
            for key in &keys {
                let error = load(&api, key).await.err().map(|e| e.to_string()).unwrap_or_default();
                assert!(!error.contains("Invalid request"), "{key:?}: {error}");
            }
            for action in actions {
                let name = format!("{action:?}");
                let error = mutate(&api, action).await.err().map(|e| e.to_string()).unwrap_or_default();
                assert!(!error.contains("Invalid request"), "{name}: {error}");
            }
            let requests = tokio::time::timeout(Duration::from_secs(5), server).await.unwrap().unwrap();
            assert!(requests.iter().all(|(_, body)| body["query"].as_str().is_some_and(|q| !q.is_empty())));
        });
    }

    #[test]
    fn queries_and_mutations_use_generated_variable_names_and_bearer_auth() {
        crate::test_support::run(async {
            let (origin, server) = crate::test_support::http(2, |body| match body["operationName"].as_str().unwrap() {
                "BrowseLibrary" => {
                    json!({"data":{"library":{"titles":[{"id":1,"name":"Example","poster":null,"year":2026}]}}})
                },
                "BrowseSetWatched" => json!({"data":{"setWatched":[{"id":5}]}}),
                name => panic!("unexpected operation {name}"),
            })
            .await;
            let api = Api::new(origin, Some(api::Token("test-session".into()))).unwrap();
            let d = load(&api, &Key::Library("Shows".into())).await.unwrap();
            assert_eq!(d.library.unwrap().titles[0].name, "Example");
            mutate(&api, Action::Watched(vec![5], true)).await.unwrap();
            let requests = server.await.unwrap();
            assert!(requests[0].0.to_lowercase().contains("authorization: bearer test-session"));
            assert_eq!(requests[0].1["variables"]["name"], "Shows");
            assert_eq!(requests[1].1["variables"], json!({"videoIds":[5],"watched":true}));
        });
    }
}
