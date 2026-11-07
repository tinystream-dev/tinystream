// SPDX-License-Identifier: AGPL-3.0-or-later

//! Browsing state, cached independently of navigation. An invalidation during a request is
//! retained until that request settles, and replies from previous sessions are discarded.

use std::collections::{HashMap, HashSet};
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
mod view;

use model::*;
pub use view::{allowed, dialog, notifications, search, view};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Shell,
    Home,
    Music,
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
    Options,
    Aired(String, String),
    Releases(i64, i64, Vec<i64>, String),
    Match(i64, String, String),
}

#[derive(Default)]
struct Entry {
    data: Option<Arc<Document>>,
    error: Option<String>,
    pending: bool,
    dirty: bool,
    serial: u64,
    fetched: Option<Instant>,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    Pick(Discover),
    Releases { series: i64, season: i64, episodes: Vec<i64>, name: String },
    Match(i64),
    Confirm { title: String, body: String, action: Action },
    Context(Card),
}

#[derive(Debug, Clone)]
pub enum Action {
    Watched(Vec<i64>, bool),
    TitleWatched(i64, bool),
    Refresh(i64),
    Approve(i64),
    Decline(i64),
    CancelRequest(i64),
    Pause(Vec<i64>),
    Resume(Vec<i64>),
    Recheck(Vec<i64>),
    Import(i64),
    Remove(Vec<i64>, bool),
    Read(Option<Vec<i64>>),
    DeleteNotice(Option<i64>),
    Add(Discover, String, String, String),
    Request(Discover, String),
    Manage(i64),
    UpdateSeries(i64, Value),
    RefreshSchedule(i64),
    RemoveSeries(i64),
    LookAgain(i64, Option<i64>, Option<i64>),
    DeleteDownloaded(i64, Option<i64>),
    Grab(i64, ReleaseCandidate),
    Match(i64, String, String),
    Artwork { id: i64, kind: Option<String>, reset: bool },
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(u64, Key, u64, Result<Arc<Document>, api::Error>),
    Mutated(u64, Result<(), api::Error>),
    Picture(u64, String, Option<iced_widget::image::Handle>, bool),
    Retry(Key),
    Query(String),
    Library(String),
    Category(String),
    Filter(String),
    Sort(bool),
    Season(i64),
    Week(i64),
    Dialog(Option<Dialog>),
    Note(String),
    Monitor(String),
    Profile(String),
    PickLibrary(String),
    ReleaseQuery(String),
    MatchQuery(String),
    Provider(String),
    Act(Action),
    Follow(Notice),
    Select(i64, bool),
    SelectAll(bool),
    Tick,
    SearchReady(u64, String),
    DiscoverReady(u64, String),
}

#[derive(Default)]
pub struct Browsing {
    cache: HashMap<Key, Entry>,
    pub images: Images,
    pub generation: u64,
    serial: u64,
    pub query: String,
    pub discover_input: String,
    pub library: String,
    pub category: String,
    pub filter: HashMap<Route, String>,
    pub newest: HashSet<Route>,
    pub seasons: HashMap<i64, i64>,
    pub week: i64,
    pub dialog: Option<Dialog>,
    pub note: String,
    pub monitor: String,
    pub profile: String,
    pub pick_library: String,
    pub release_query: String,
    pub match_query: String,
    pub provider: String,
    pub selected: HashSet<i64>,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    search_serial: u64,
    discover_serial: u64,
    pub search_query: String,
    tick_pending: bool,
}

impl Browsing {
    pub fn data(&self, key: &Key) -> Option<&Document> {
        self.cache.get(key)?.data.as_deref()
    }

    pub fn can(&self) -> Permissions {
        self.data(&Key::Shell).and_then(|d| d.viewer.as_ref()).map(|v| v.permissions.clone()).unwrap_or_default()
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
        match route {
            Route::Home => {
                for key in [Key::Home, Key::Music] {
                    self.ensure(key, api, effects);
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
            },
            Route::Calendar => {
                self.ensure(Key::Calendar(week_start(now()) + self.week * 7 * 86400), api, effects);
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
                self.ensure(Key::Aired(r.provider, r.id), api, effects);
            },
            Some(Dialog::Releases { series, season, episodes, .. }) => {
                self.ensure(Key::Releases(series, season, episodes, self.release_query.clone()), api, effects)
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
                E::LibraryChanged(_)
                | E::ScanFinished(_)
                | E::MetadataChanged(_)
                | E::EpisodesImported(_)
                | E::SeriesChanged(_)
                | E::ListChanged(_) => self.invalidate(|_| true),
                _ => {},
            },
            _ => {},
        }
    }

    pub fn search(&mut self, query: String, effects: &mut Vec<Effect>) {
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
                let Some(entry) = self.cache.get_mut(&key).filter(|e| e.serial == serial) else { return None };
                entry.pending = false;
                entry.fetched = Some(Instant::now());
                match result {
                    Ok(data) => {
                        let paths = picture_paths(&data);
                        entry.data = Some(data);
                        entry.error = None;
                        self.pictures(paths, api, effects);
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
                        self.dialog = None;
                        self.notice = Some("Saved".into());
                        self.error = None;
                        self.images.clear();
                        self.invalidate(|_| true);
                    },
                    Err(error) => self.error = Some(error.message),
                }
            },
            Message::Picture(generation, path, handle, fresh) if generation == self.generation => {
                self.images.arrived(path, handle, fresh)
            },
            Message::Retry(key) => {
                if let Some(e) = self.cache.get_mut(&key) {
                    e.dirty = true;
                }
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
            Message::DiscoverReady(serial, query) if serial == self.discover_serial => self.query = query,
            Message::SearchReady(serial, query) if serial == self.search_serial => self.search_query = query,
            Message::Library(library) => self.library = library,
            Message::Category(category) => self.category = category,
            Message::Filter(filter) => {
                self.filter.insert(route.clone(), filter);
            },
            Message::Sort(newest) => {
                if newest {
                    self.newest.insert(route.clone());
                } else {
                    self.newest.remove(route);
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
                self.note.clear();
                self.profile.clear();
                self.release_query.clear();
                self.match_query.clear();
                self.provider.clear();
                if let Some(Dialog::Pick(r)) = &dialog {
                    self.pick_library = r.library.clone();
                    self.monitor = self
                        .data(&Key::Options)
                        .and_then(|d| d.settings.as_ref())
                        .map_or("NONE".into(), |s| s.automation.default_monitor.clone());
                }
                self.dialog = dialog;
            },
            Message::Note(note) => self.note = note,
            Message::Monitor(monitor) => self.monitor = monitor,
            Message::Profile(profile) => self.profile = profile,
            Message::PickLibrary(library) => self.pick_library = library,
            Message::ReleaseQuery(query) => self.release_query = query,
            Message::MatchQuery(query) => self.match_query = query,
            Message::Provider(provider) => self.provider = provider,
            Message::Follow(n) => {
                self.act(Action::Read(Some(vec![n.id])), api, effects);
                return n.link.and_then(|link| Route::parse(&link));
            },
            Message::Select(id, selected) => {
                if selected {
                    self.selected.insert(id);
                } else {
                    self.selected.remove(&id);
                }
            },
            Message::SelectAll(on) => {
                self.selected = if on {
                    self.data(&Key::Downloads).map(|d| d.downloads.iter().map(|d| d.id).collect()).unwrap_or_default()
                } else {
                    HashSet::new()
                };
            },
            Message::Act(action) => self.act(action, api, effects),
            Message::Tick => {
                self.tick_pending = false;
                for (key, entry) in &mut self.cache {
                    let interval = match key {
                        Key::Downloads | Key::Engine | Key::Calendar(_) | Key::Soon | Key::Coming => 2,
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
        if self.busy {
            return;
        }
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

async fn query<Q: GraphQLQuery>(api: &Api, variables: Value) -> Result<Document, api::Error>
where
    Q::Variables: DeserializeOwned,
{
    let variables =
        serde_json::from_value(variables).map_err(|e| api::Error::message(format!("Invalid request: {e}")))?;
    api.request_data::<Q, Document>(variables).await
}

async fn mutation<Q: GraphQLQuery>(api: &Api, variables: Value) -> Result<(), api::Error>
where
    Q::Variables: DeserializeOwned,
{
    let variables =
        serde_json::from_value(variables).map_err(|e| api::Error::message(format!("Invalid request: {e}")))?;
    api.request::<Q>(variables).await.map(|_| ())
}

async fn load(api: &Api, key: &Key) -> Result<Document, api::Error> {
    use api::*;
    let null_string = |s: &str| if s.is_empty() { Value::Null } else { json!(s) };
    match key {
        Key::Shell => query::<BrowseShell>(api, json!({})).await,
        Key::Home => query::<BrowseHome>(api, json!({})).await,
        Key::Music => query::<BrowseMusicHome>(api, json!({})).await,
        Key::Coming => query::<BrowseComingUp>(api, json!({"from":now()-3600,"to":now()+7*86400})).await,
        Key::Library(name) => query::<BrowseLibrary>(api, json!({"name":name})).await,
        Key::Books(name) => query::<BrowseBooks>(api, json!({"library":name})).await,
        Key::Title(id) => query::<BrowseTitle>(api, json!({"id":id})).await,
        Key::Similar(id) => query::<BrowseSimilar>(api, json!({"id":id})).await,
        Key::Schedule(id) => query::<BrowseTitleSchedule>(api, json!({"id":id})).await,
        Key::Series(id) => query::<BrowseTitleSeries>(api, json!({"id":id})).await,
        Key::Calendar(start) => query::<BrowseCalendar>(api, json!({"from":start,"to":start+7*86400})).await,
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
        Key::Search(q) => query::<BrowseSearch>(api, json!({"query":q})).await,
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
        Action::TitleWatched(id, watched) => mutation::<BrowseSetTitleWatched>(api, json!({"id":id,"watched":watched})).await,
        Action::Refresh(id) => mutation::<BrowseRefreshTitle>(api, json!({"id":id})).await,
        Action::Approve(id) => mutation::<BrowseApproveRequest>(api, json!({"id":id})).await,
        Action::Decline(id) => mutation::<BrowseDeclineRequest>(api, json!({"id":id})).await,
        Action::CancelRequest(id) => mutation::<BrowseDeleteRequest>(api, json!({"id":id})).await,
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
        Action::DeleteDownloaded(id, season) => mutation::<BrowseDeleteDownloaded>(api, json!({"seriesId":id,"season":season})).await,
        Action::Grab(id, r) => mutation::<BrowseGrabRelease>(api, json!({"seriesId":id,"episodes":r.episodes.iter().map(|e|json!({"season":e.season,"episode":e.episode})).collect::<Vec<_>>(),"release":{"title":r.release.title,"source":r.release.source,"link":r.release.link,"infoHash":r.release.info_hash,"size":r.release.size,"seeders":r.release.seeders,"leechers":r.release.leechers,"published":r.release.published,"page":r.release.page}})).await,
        Action::Match(id, provider, provider_id) => mutation::<BrowseMatchTitle>(api, json!({"id":id,"provider":provider,"providerId":provider_id})).await,
        Action::Artwork { id, kind, reset } => api.artwork(id, kind, reset).await,
    }
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

pub fn week_start(at: i64) -> i64 {
    use crate::platform::{Current, Desktop};
    let offset = Current::utc_offset(at);
    let day = (at + offset).div_euclid(86400);
    (day - (day + 3).rem_euclid(7)) * 86400 - offset
}

fn picture_paths(d: &Document) -> Vec<String> {
    let mut paths = Vec::new();
    let mut add = |p: &Option<String>| {
        if let Some(p) = p {
            paths.push(p.clone());
        }
    };
    for c in d.library.iter().flat_map(|l| &l.titles) {
        add(&c.poster);
    }
    if let Some(h) = &d.home {
        for e in &h.continue_watching {
            add(&e.video.still);
            add(&e.video.title.backdrop);
        }
        for c in h.recently_added.iter().flat_map(|s| &s.titles).chain(h.popular_here.iter().map(|p| &p.title)) {
            add(&c.poster);
        }
    }
    if let Some(t) = &d.title {
        add(&t.card.poster);
        add(&t.card.backdrop);
        for s in &t.seasons {
            add(&s.poster);
            for v in &s.episodes {
                add(&v.still);
            }
        }
        if let Some(s) = &t.similar {
            for c in &s.also_watched {
                add(&c.poster);
            }
            for c in &s.recommendations {
                add(&c.poster);
            }
        }
        if let Some(c) = &t.match_candidates {
            for c in &c.results {
                add(&c.poster);
            }
        }
    }
    for e in &d.calendar {
        add(&e.poster);
        add(&e.backdrop);
    }
    for r in &d.requests {
        add(&r.poster);
        if let Some(u) = &r.user {
            add(&u.avatar);
        }
    }
    for r in &d.downloads {
        add(&r.poster);
    }
    if let Some(i) = &d.notifications {
        for n in &i.items {
            add(&n.image);
            if let Some(a) = &n.actor {
                add(&a.avatar);
            }
        }
    }
    if let Some(s) = &d.search {
        for c in &s.titles {
            add(&c.poster);
        }
    }
    if let Some(s) = &d.music_search {
        for a in s.albums.iter().chain(&s.artists) {
            add(&a.cover);
        }
    }
    if let Some(h) = &d.music_home {
        for a in h.recently_added.iter().chain(&h.recently_played) {
            add(&a.cover);
        }
    }
    if let Some(s) = &d.discover {
        for c in &s.results {
            add(&c.poster);
        }
    }
    if let Some(s) = &d.for_you {
        for c in s.shelves.iter().flat_map(|s| &s.results) {
            add(&c.poster);
        }
    }
    for b in &d.books {
        add(&b.cover);
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api() -> Api {
        Api::new(url::Url::parse("http://localhost:3000").unwrap(), None).unwrap()
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
