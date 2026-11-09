// SPDX-License-Identifier: AGPL-3.0-or-later

//! Clips (#93): the Clips page, and a clip's dialog with its player, link and who it was sent to,
//! as `routes/clips.tsx` has them. Clips are made and changed in the player's editor
//! (`player/clip.rs`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use graphql_client::GraphQLQuery;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};
use crate::player::clip_player::{self, ClipPlayer};
use crate::route::Route;
use crate::ui::overlay::Tone;

pub mod model;
pub mod view;

pub use model::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Scope {
    #[default]
    Mine,
    Received,
    Sent,
}

impl Scope {
    fn name(self) -> &'static str {
        match self {
            Self::Mine => "MINE",
            Self::Received => "RECEIVED",
            Self::Sent => "SENT",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Kind {
    #[default]
    All,
    Clips,
    Screenshots,
    Quotes,
}

impl Kind {
    fn name(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Clips => Some("CLIP"),
            Self::Screenshots => Some("SCREENSHOT"),
            Self::Quotes => Some("QUOTE"),
        }
    }
}

/// Which list: the tab, the kind, and the book its quotes are from.
pub type Filter = (Scope, Kind, Option<i64>);

/// A list of clips, and what you may do with them (`ClipList`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct List {
    pub clips: Vec<Clip>,
    #[serde(rename = "clipAllowance")]
    pub you: Allowance,
}

struct Entry<T> {
    data: Option<T>,
    error: Option<String>,
    pending: bool,
    dirty: bool,
    serial: u64,
}

impl<T> Default for Entry<T> {
    fn default() -> Self {
        Self { data: None, error: None, pending: false, dirty: false, serial: 0 }
    }
}

impl<T> Entry<T> {
    /// Whether it needs asking for, marking it asked with `serial` when it does.
    fn wants(&mut self, serial: u64) -> bool {
        if self.pending || (!self.dirty && (self.data.is_some() || self.error.is_some())) {
            return false;
        }
        self.pending = true;
        self.dirty = false;
        self.serial = serial;
        true
    }

    fn settle(&mut self, serial: u64, result: Result<T, api::Error>) -> bool {
        if serial != self.serial {
            return false;
        }
        self.pending = false;
        match result {
            Ok(data) => {
                self.data = Some(data);
                self.error = None;
            },
            Err(error) => self.error = Some(error.message),
        }
        true
    }
}

/// The clip open in its dialog (`ClipDialog`).
pub struct Open {
    pub id: i64,
    clip: Entry<Option<Arc<Clip>>>,
    pub title: String,
    pub copied: bool,
    pub send_to: Vec<Person>,
    pub picking: bool,
    pub confirm: bool,
    pub saving: bool,
    asked_render: bool,
    pub player: Option<ClipPlayer>,
    /// The render the player plays, so a new one starts it over.
    played: Option<Option<i64>>,
}

impl Open {
    fn new(id: i64) -> Self {
        Self {
            id,
            clip: Entry::default(),
            title: String::new(),
            copied: false,
            send_to: Vec::new(),
            picking: false,
            confirm: false,
            saving: false,
            asked_render: false,
            player: None,
            played: None,
        }
    }

    /// The clip, once it's here; `Some(None)` when it isn't anymore.
    pub fn clip(&self) -> Option<Option<&Clip>> {
        self.clip.data.as_ref().map(|c| c.as_deref())
    }

    pub fn failed(&self) -> bool {
        self.clip.error.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Render,
    Name,
    Public,
    Share(usize),
    Unshare,
    Delete,
    Hide,
}

#[derive(Debug, Clone)]
pub enum Message {
    Scope(Scope),
    Kind(Kind),
    AllQuotes,
    Listed(Filter, u64, Result<Arc<List>, api::Error>),
    Fetched(i64, u64, Result<Option<Arc<Clip>>, api::Error>),
    People(u64, Result<Arc<Vec<Person>>, api::Error>),
    Changed(i64, Change, Result<Option<Arc<Clip>>, api::Error>),
    Title(String),
    SaveTitle,
    Public(bool),
    CopyLink,
    CopyQuote,
    Rerender,
    Unshare(i64),
    Pick(Person, bool),
    Picker(bool),
    Send,
    Delete,
    Confirm(bool),
    Hide,
    Download,
    Downloaded(Result<Option<PathBuf>, String>),
    Player(i64, clip_player::Message),
    Retry,
    Open(i64),
    Close,
}

/// What the clips page asks of the app.
#[derive(Debug, Clone)]
pub enum Out {
    /// Open a clip's dialog over the page.
    Open(i64),
    /// Close the clip's dialog.
    Close,
    Toast(String, Option<String>, Tone),
    /// Put this on the clipboard, saying so with a toast.
    Copy(String, String, Option<String>),
    Fullscreen,
    Navigate(Route),
}

#[derive(Default)]
pub struct Clips {
    generation: u64,
    pub scope: Scope,
    pub kind: Kind,
    pub book: Option<i64>,
    lists: HashMap<Filter, Entry<Arc<List>>>,
    /// The list on the page: the one asked for once it's here, the one before until then.
    shown: Option<Filter>,
    people: Entry<Arc<Vec<Person>>>,
    pub open: Option<Open>,
    /// Whether the open clip's player fills the window.
    pub expanded: bool,
    /// Where public links point: the server's public address, or the one we reach it at.
    origin: String,
    serial: u64,
}

impl Clips {
    pub fn filter(&self) -> Filter {
        (self.scope, self.kind, self.book)
    }

    /// The list the page shows, and whether it's the one asked for.
    pub fn list(&self) -> Option<(&List, Filter, bool)> {
        let wanted = self.filter();
        if let Some(list) = self.lists.get(&wanted).and_then(|e| e.data.as_deref()) {
            return Some((list, wanted, true));
        }
        let shown = self.shown?;
        Some((self.lists.get(&shown)?.data.as_deref()?, shown, false))
    }

    pub fn list_error(&self) -> Option<&str> {
        self.lists.get(&self.filter())?.error.as_deref()
    }

    /// What you may do with clips, from whichever list came last.
    pub fn allowance(&self) -> Option<&Allowance> {
        self.list().map(|(list, ..)| &list.you)
    }

    /// Everyone else on the server, by name.
    pub fn people(&self, me: Option<i64>) -> Option<Vec<&Person>> {
        let mut people: Vec<&Person> = self.people.data.as_ref()?.iter().filter(|p| Some(p.id) != me).collect();
        people.sort_by_key(|p| p.username.to_lowercase());
        Some(people)
    }

    fn next(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    pub fn invalidate(&mut self) {
        for entry in self.lists.values_mut() {
            entry.dirty = true;
        }
        if let Some(open) = &mut self.open {
            open.clip.dirty = true;
        }
    }

    /// Asks for what the route shows that isn't here yet, and starts the open clip's player.
    pub fn follow(&mut self, generation: u64, route: &Route, context: Context<'_>, effects: &mut Vec<Effect>) {
        if generation != self.generation {
            *self = Self { generation, ..Self::default() };
        }
        let id = match route {
            Route::Clip(id) => Some(*id as i64),
            Route::Clips => None,
            _ => {
                self.open = None;
                self.expanded = false;
                return;
            },
        };
        let Context { api, runtime, who, origin } = context;
        self.origin = origin;
        let filter = self.filter();
        let serial = self.next();
        if self.lists.entry(filter).or_default().wants(serial) {
            let (api, generation) = (api.clone(), self.generation);
            effects.push(Effect::Request(Box::pin(async move {
                let variables = json!({"scope":filter.0.name(),"kind":filter.1.name(),"book":filter.2});
                let result = call::<api::ClipsList, List>(&api, variables).await.map(Arc::new);
                AppMessage::Clips(generation, Message::Listed(filter, serial, result))
            })));
        }
        let Some(id) = id else {
            self.open = None;
            self.expanded = false;
            return;
        };
        if self.open.as_ref().is_none_or(|open| open.id != id) {
            self.open = Some(Open::new(id));
            self.expanded = false;
        }
        let serial = self.next();
        let generation = self.generation;
        let open = self.open.as_mut().expect("an open clip");
        if open.clip.wants(serial) {
            let api = api.clone();
            effects.push(Effect::Request(Box::pin(async move {
                let result = clip_of::<api::ClipsClip>(&api, json!({"id":id})).await;
                AppMessage::Clips(generation, Message::Fetched(id, serial, result))
            })));
        }
        let Some(Some(clip)) = open.clip.data.clone() else { return };
        // A render that was dropped comes back when someone opens it.
        if clip.state == "EVICTED" && !open.asked_render {
            open.asked_render = true;
            self.change(Change::Render, api, effects);
        }
        let open = self.open.as_mut().expect("an open clip");
        let playable = clip.ready() && !clip.screenshot && clip.quote.is_none();
        if playable && open.played != Some(clip.rendered_at) {
            open.played = Some(clip.rendered_at);
            let wrap: Arc<dyn Fn(clip_player::Message) -> AppMessage + Send + Sync> =
                Arc::new(move |message| AppMessage::Clips(generation, Message::Player(id, message)));
            open.player = Some(ClipPlayer::new(api.clone(), runtime, &clip.file, false, who, wrap, effects));
        } else if !playable {
            open.player = None;
            open.played = None;
        }
        if clip.can_manage {
            let serial = self.next();
            if self.people.wants(serial) {
                let api = api.clone();
                effects.push(Effect::Request(Box::pin(async move {
                    #[derive(Deserialize)]
                    struct Users {
                        users: Vec<Person>,
                    }
                    let result = call::<api::ClipsPeople, Users>(&api, json!({})).await.map(|d| Arc::new(d.users));
                    AppMessage::Clips(generation, Message::People(serial, result))
                })));
            }
        }
    }

    /// Keeps lists and the open clip up to date with what the server says changed.
    pub fn live(&mut self, live: &api::Live) {
        use api::events::EventsEvents as E;
        match live {
            api::Live::Connected => {
                self.invalidate();
                self.people.dirty = true;
            },
            api::Live::Event(event) => match &event.events {
                E::ClipChanged(change) => {
                    let rendering = matches!(change.state, Some(api::events::ClipState::RENDERING));
                    self.changed(change.clip_id, change.progress.filter(|_| rendering));
                },
                E::ConfigChanged(_) => self.invalidate(),
                E::ListChanged(change) if matches!(change.list, api::events::ChangedList::USERS) => {
                    self.people.dirty = true
                },
                _ => {},
            },
            _ => {},
        }
    }

    /// Clip `id` changed: a render's `progress` is patched in where it shows, anything else is
    /// asked for again.
    pub fn changed(&mut self, id: i64, progress: Option<f64>) {
        if let Some(progress) = progress {
            let mut found = false;
            let tick = |clip: &mut Clip| {
                if clip.id == id && clip.state == "RENDERING" {
                    clip.progress = Some(progress);
                    true
                } else {
                    false
                }
            };
            if let Some(open) = &mut self.open
                && let Some(Some(clip)) = &mut open.clip.data
                && clip.id == id
                && clip.state == "RENDERING"
            {
                tick(Arc::make_mut(clip));
                found = true;
            }
            for entry in self.lists.values_mut() {
                if let Some(list) = &mut entry.data
                    && list.clips.iter().any(|c| c.id == id && c.state == "RENDERING")
                {
                    Arc::make_mut(list).clips.iter_mut().for_each(|c| {
                        tick(c);
                    });
                    found = true;
                }
            }
            if found {
                return;
            }
        }
        for entry in self.lists.values_mut() {
            entry.dirty = true;
        }
        if let Some(open) = &mut self.open
            && open.id == id
        {
            open.clip.dirty = true;
        }
    }

    pub fn update(&mut self, message: Message, api: &Api, effects: &mut Vec<Effect>) -> Option<Out> {
        match message {
            Message::Scope(scope) => self.scope = scope,
            Message::Kind(kind) => self.kind = kind,
            Message::AllQuotes => {
                self.book = None;
                self.kind = Kind::Quotes;
            },
            Message::Open(id) => return Some(Out::Open(id)),
            // Leaving the title saves it, as leaving the field does on web.
            Message::Close => {
                if let Some(open) = &self.open
                    && let Some(Some(clip)) = open.clip()
                    && clip.can_manage
                    && open.title.trim() != clip.name
                {
                    self.change(Change::Name, api, effects);
                }
                return Some(Out::Close);
            },
            Message::Retry => {
                if let Some(entry) = self.lists.get_mut(&self.filter()) {
                    entry.error = None;
                    entry.dirty = true;
                }
            },
            Message::Listed(filter, serial, result) => {
                let entry = self.lists.entry(filter).or_default();
                let ok = result.is_ok();
                if entry.settle(serial, result) && ok {
                    self.shown = Some(filter);
                }
            },
            Message::Fetched(id, serial, result) => {
                let open = self.open.as_mut().filter(|open| open.id == id)?;
                // The title follows the clip's, unless it's being changed.
                let before = open.clip().flatten().map(|c| c.name.clone());
                if open.clip.settle(serial, result)
                    && let Some(Some(clip)) = &open.clip.data
                    && !open.saving
                    && before.as_deref().is_none_or(|name| name == open.title)
                {
                    open.title = clip.name.clone();
                }
            },
            Message::People(serial, result) => {
                self.people.settle(serial, result);
            },
            Message::Changed(id, change, result) => return self.changed_by(id, change, result),
            Message::Title(title) => {
                if let Some(open) = &mut self.open {
                    open.title = title.chars().take(120).collect();
                }
            },
            Message::SaveTitle => {
                let open = self.open.as_ref()?;
                let clip = open.clip()??;
                if clip.can_manage && open.title.trim() != clip.name {
                    self.change(Change::Name, api, effects);
                }
            },
            Message::Public(on) => {
                let open = self.open.as_mut()?;
                if let Some(Some(clip)) = &mut open.clip.data {
                    Arc::make_mut(clip).public = on;
                }
                self.change(Change::Public, api, effects);
            },
            Message::CopyLink => {
                let open = self.open.as_mut()?;
                let link = open.clip()??.link.clone()?;
                open.copied = true;
                let url = absolute(&link, &self.origin);
                return Some(Out::Copy(url.clone(), "Link copied".into(), Some(url)));
            },
            Message::CopyQuote => {
                let quote = self.open.as_ref()?.clip()??.quote.as_ref()?.text.clone();
                return Some(Out::Copy(quote, "Copied".into(), None));
            },
            Message::Rerender => self.change(Change::Render, api, effects),
            Message::Unshare(user) => {
                let open = self.open.as_ref()?;
                let id = open.id;
                self.mutate(id, Change::Unshare, api, effects, move |api| async move {
                    clip_of::<api::ClipsUnshare>(&api, json!({"id":id,"userId":user})).await
                });
            },
            Message::Pick(person, on) => {
                let open = self.open.as_mut()?;
                open.send_to.retain(|p| p.id != person.id);
                if on {
                    open.send_to.push(person);
                }
            },
            Message::Picker(open_it) => self.open.as_mut()?.picking = open_it,
            Message::Send => {
                let open = self.open.as_mut()?;
                if open.send_to.is_empty() {
                    return None;
                }
                open.picking = false;
                let count = open.send_to.len();
                self.change(Change::Share(count), api, effects);
            },
            Message::Delete => self.open.as_mut()?.confirm = true,
            Message::Confirm(yes) => {
                self.open.as_mut()?.confirm = false;
                if yes {
                    self.change(Change::Delete, api, effects);
                }
            },
            Message::Hide => self.change(Change::Hide, api, effects),
            Message::Download => {
                let file = self.open.as_ref()?.clip()??.file.clone();
                let (api, generation) = (api.clone(), self.generation);
                effects.push(Effect::Task(Box::pin(async move {
                    AppMessage::Clips(generation, Message::Downloaded(download(api, file).await))
                })));
            },
            Message::Downloaded(result) => return downloaded(result),
            Message::Player(id, message) => {
                let player = self.open.as_mut().filter(|open| open.id == id)?.player.as_mut()?;
                if player.update(message) == Some(clip_player::Out::Fullscreen) {
                    return Some(Out::Fullscreen);
                }
            },
        }
        None
    }

    fn change(&mut self, change: Change, api: &Api, effects: &mut Vec<Effect>) {
        let Some(open) = &mut self.open else { return };
        let id = open.id;
        let name = open.title.clone();
        let users: Vec<i64> = open.send_to.iter().map(|p| p.id).collect();
        let public = open.clip().flatten().is_some_and(|c| c.public);
        if matches!(change, Change::Name) {
            open.saving = true;
        }
        self.mutate(id, change, api, effects, move |api| async move {
            match change {
                Change::Render => clip_of::<api::ClipsRender>(&api, json!({"id":id})).await,
                Change::Name => clip_of::<api::ClipsUpdate>(&api, json!({"id":id,"input":{"name":name}})).await,
                Change::Public => clip_of::<api::ClipsUpdate>(&api, json!({"id":id,"input":{"public":public}})).await,
                Change::Share(_) => clip_of::<api::ClipsShare>(&api, json!({"id":id,"users":users})).await,
                Change::Delete => clip_of::<api::ClipsDelete>(&api, json!({"id":id})).await.map(|_| None),
                Change::Hide => clip_of::<api::ClipsHide>(&api, json!({"id":id})).await.map(|_| None),
                Change::Unshare => Ok(None),
            }
        });
    }

    fn mutate<F: Future<Output = Result<Option<Arc<Clip>>, api::Error>> + Send + 'static>(
        &self,
        id: i64,
        change: Change,
        api: &Api,
        effects: &mut Vec<Effect>,
        request: impl FnOnce(Api) -> F,
    ) {
        let (future, generation) = (request(api.clone()), self.generation);
        effects.push(Effect::Request(Box::pin(async move {
            AppMessage::Clips(generation, Message::Changed(id, change, future.await))
        })));
    }

    fn changed_by(&mut self, id: i64, change: Change, result: Result<Option<Arc<Clip>>, api::Error>) -> Option<Out> {
        for entry in self.lists.values_mut() {
            entry.dirty = true;
        }
        if change == Change::Name
            && let Some(open) = &mut self.open
        {
            open.saving = false;
        }
        let open = self.open.as_mut().filter(|open| open.id == id);
        match result {
            Err(error) => {
                if let Some(open) = open {
                    open.clip.dirty = true;
                }
                Some(Out::Toast(error.message, None, Tone::Danger))
            },
            Ok(clip) => {
                if matches!(change, Change::Delete | Change::Hide) {
                    return Some(Out::Close);
                }
                let open = open?;
                if let Some(clip) = clip {
                    if change == Change::Name {
                        open.title = clip.name.clone();
                    }
                    open.clip.data = Some(Some(clip));
                }
                match change {
                    Change::Share(count) => {
                        let to = if count == 1 {
                            open.send_to.first().map_or_else(String::new, |p| p.username.clone())
                        } else {
                            format!("{count} people")
                        };
                        open.send_to.clear();
                        Some(Out::Toast(format!("Sent to {to}"), None, Tone::Ok))
                    },
                    _ => None,
                }
            },
        }
    }
}

/// What following the route needs from the rest of the app.
pub struct Context<'a> {
    pub api: &'a Api,
    pub runtime: Option<tokio::runtime::Handle>,
    /// The saved server and the user signed in to it, for what the player remembers.
    pub who: (u64, i64),
    /// Where public links point.
    pub origin: String,
}

/// `value` as an operation's variables, then its reply as a `T`.
pub async fn call<Q: GraphQLQuery, T: DeserializeOwned>(api: &Api, value: Value) -> Result<T, api::Error>
where
    Q::Variables: DeserializeOwned,
{
    api.request_data::<Q, T>(crate::browsing::variables(value)?).await
}

/// The clip an operation answers with, under whatever its one field is called; `None` when the
/// answer isn't a clip (null, or a deletion's `true`).
pub async fn clip_of<Q: GraphQLQuery>(api: &Api, value: Value) -> Result<Option<Arc<Clip>>, api::Error>
where
    Q::Variables: DeserializeOwned,
{
    let data: HashMap<String, Value> = call::<Q, _>(api, value).await?;
    Ok(data
        .into_values()
        .next()
        .filter(Value::is_object)
        .and_then(|value| serde_json::from_value::<Clip>(value).ok())
        .map(Arc::new))
}

/// Saves a clip's `file` where the person picks, named as the server names it.
pub async fn download(api: Api, file: String) -> Result<Option<PathBuf>, String> {
    use crate::platform::{Current, Desktop};
    let path = format!("{}?download=1", file.trim_start_matches('/'));
    let unreachable = |_| "Couldn't reach the server.".to_owned();
    // Asked for first, so the save dialog can offer the server's name for it.
    let name = api
        .head(&path)
        .map_err(|e| e.message)?
        .send()
        .await
        .ok()
        .and_then(|response| {
            response.headers().get(reqwest::header::CONTENT_DISPOSITION)?.to_str().ok().and_then(filename)
        })
        .unwrap_or_else(|| if file.ends_with("/image") { "screenshot.png" } else { "clip.mp4" }.to_owned());
    let Some(target) = Current::save_file(name, "Save the clip".into()).await? else { return Ok(None) };
    let mut response = api.stream(&path).map_err(|e| e.message)?.send().await.map_err(unreachable)?;
    if !response.status().is_success() {
        return Err(format!("The server couldn't send it ({}).", response.status()));
    }
    let mut partial = target.clone().into_os_string();
    partial.push(".partial");
    let failed = |e: std::io::Error| format!("Couldn't save it: {e}");
    let result = async {
        use tokio::io::AsyncWriteExt;
        let mut out = tokio::fs::File::create(&partial).await.map_err(failed)?;
        while let Some(chunk) = response.chunk().await.map_err(|_| "The download broke off.".to_owned())? {
            out.write_all(&chunk).await.map_err(failed)?;
        }
        out.flush().await.map_err(failed)?;
        tokio::fs::rename(&partial, &target).await.map_err(failed)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&partial).await;
    }
    result.map(|_| Some(target))
}

/// The toast for how a download went.
pub fn downloaded(result: Result<Option<PathBuf>, String>) -> Option<Out> {
    match result {
        Ok(Some(path)) => Some(Out::Toast("Saved".into(), Some(path.display().to_string()), Tone::Ok)),
        Ok(None) => None,
        Err(error) => Some(Out::Toast(error, None, Tone::Danger)),
    }
}

/// The file name in a `Content-Disposition`, preferring its UTF-8 form.
fn filename(disposition: &str) -> Option<String> {
    let parts: Vec<&str> = disposition.split(';').map(str::trim).collect();
    if let Some(encoded) = parts.iter().find_map(|p| p.strip_prefix("filename*=UTF-8''")) {
        let bytes = encoded.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            match (bytes[i], encoded.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())) {
                (b'%', Some(byte)) => {
                    out.push(byte);
                    i += 3;
                },
                (byte, _) => {
                    out.push(byte);
                    i += 1;
                },
            }
        }
        return String::from_utf8(out).ok().filter(|name| !name.contains(['/', '\\']));
    }
    parts.iter().find_map(|p| p.strip_prefix("filename=")).map(|name| name.trim_matches('"').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: i64, state: &str) -> Clip {
        Clip { id, state: state.into(), ..Clip::default() }
    }

    #[test]
    fn render_progress_is_patched_in_and_anything_else_asked_for_again() {
        let mut clips = Clips::default();
        let list = List { clips: vec![clip(1, "RENDERING"), clip(2, "READY")], you: Allowance::default() };
        let filter = clips.filter();
        clips.lists.insert(filter, Entry { data: Some(Arc::new(list)), ..Entry::default() });
        clips.changed(1, Some(0.6));
        assert_eq!(clips.list().unwrap().0.clips[0].progress, Some(0.6));
        assert!(!clips.lists[&filter].dirty);
        // A clip the page doesn't have rendering means the list is behind.
        clips.changed(3, Some(0.1));
        assert!(clips.lists[&filter].dirty);
    }

    #[test]
    fn the_dialog_opens_and_closes_in_place() {
        let mut app = crate::app::App::new(Route::Home, crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        app.session.api = Some(Api::new(url::Url::parse("http://127.0.0.1:9/").unwrap(), None).unwrap());
        let generation = app.session.generation;
        app.update(AppMessage::Navigate(Route::Clips), &mut Vec::new());
        app.history.set_scroll(320.0);
        app.update(AppMessage::Clips(generation, Message::Open(7)), &mut Vec::new());
        assert_eq!(app.history.current().route, Route::Clip(7));
        assert_eq!(app.clips.open.as_ref().map(|open| open.id), Some(7));
        app.update(AppMessage::Clips(generation, Message::Close), &mut Vec::new());
        assert_eq!(app.history.current().route, Route::Clips);
        assert_eq!(app.history.current().scroll, 320.0);
        assert!(app.clips.open.is_none());
        app.update(AppMessage::Navigate(Route::Home), &mut Vec::new());
        app.update(AppMessage::Navigate(Route::Clips), &mut Vec::new());
        assert!(app.history.back().is_some_and(|entry| entry.route == Route::Home));
    }

    #[test]
    fn download_names_prefer_the_utf8_form() {
        assert_eq!(
            filename("attachment; filename=\"Show - Clip.mp4\"; filename*=UTF-8''Sh%C5%8Dw%20-%20Clip.mp4").as_deref(),
            Some("Shōw - Clip.mp4")
        );
        assert_eq!(filename("attachment; filename=\"Show.png\"").as_deref(), Some("Show.png"));
        assert_eq!(filename("attachment; filename*=UTF-8''..%2F..%2Fx").as_deref(), None);
    }
}

#[cfg(all(test, debug_assertions))]
mod render_tests {
    use serde_json::json;

    use super::*;

    impl Clips {
        fn put(&mut self, filter: Filter, list: Value) {
            let list = serde_json::from_value(list).expect("a clip list");
            self.lists.insert(filter, Entry { data: Some(Arc::new(list)), ..Entry::default() });
            self.shown = Some(filter);
        }
    }

    fn clip(id: i64, extra: Value) -> Value {
        let mut clip = json!({
            "id": id, "kind": "CLIP", "name": "", "mine": true, "canManage": true,
            "owner": {"id": 1, "username": "David"},
            "source": {"video": {"id": 10}, "title": {"id": 1}, "name": "The quiet orbit", "kind": "SHOW", "label": "S01E04", "status": "OK"},
            "start": 312.4, "end": 341.9, "quality": {"height": 1080, "halfRate": false}, "state": "READY",
            "bytes": 18_400_000, "width": 1920, "height": 1080, "fps": 23.976, "renderedAt": 5, "createdAt": now() - 7200,
            "public": false, "linkLive": true, "recipients": [], "file": format!("/api/clips/{id}/video"), "poster": "poster",
        });
        for (key, value) in extra.as_object().unwrap() {
            clip[key] = value.clone();
        }
        clip
    }

    fn now() -> i64 {
        crate::browsing::now()
    }

    #[test]
    #[ignore = "requires Vulkan; renders the Clips page, a clip's dialog and the editor"]
    fn clips_render() {
        let output = std::env::var_os("TINYSTREAM_TEST_ARTIFACT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("tinystream-clips-qa"));
        std::fs::create_dir_all(&output).unwrap();
        let mut app = crate::app::App::new(Route::Clips, crate::window_state::WindowState::default());
        tokio::runtime::Runtime::new().unwrap().block_on(app.session.preview("home")).unwrap();
        crate::browsing::preview::prepare(&mut app, "home", crate::ui::Style::Layered, false, Some(true));
        app.history = crate::route::History::new(Route::Clips);
        let clips = vec![
            clip(1, json!({"name": "The signal", "public": true, "link": "/c/abc"})),
            clip(2, json!({"state": "RENDERING", "progress": 0.42, "poster": null})),
            clip(3, json!({"kind": "SCREENSHOT", "screenshot": true, "end": 312.4, "file": "/api/clips/3/image"})),
            clip(
                4,
                json!({"kind": "QUOTE", "quote": {"text": "The road north was longer than any of them remembered, and quieter.", "author": "Mira Vale"}, "source": {"name": "Northbound", "kind": "MOVIE", "status": "OK", "book": {"id": 21, "title": "Northbound", "coverTint": "120 140 200"}}}),
            ),
            clip(5, json!({"state": "FAILED", "poster": null})),
            clip(6, json!({"state": "EVICTED", "poster": null, "name": "Old one"})),
        ];
        let you = json!({"canClip": true, "canLink": true, "maxLength": 60, "bytes": 1_200_000_000i64, "rendered": 4, "storage": 2048, "limit": 10});
        app.clips.put((Scope::Mine, Kind::All, None), json!({"clips": clips, "clipAllowance": you}));
        let draw = |app: &mut crate::app::App, name: &str| {
            crate::snapshot::snapshot(app, iced_core::Size::new(1360, 900), 1.0, None, &output.join(name), &[]).unwrap()
        };
        draw(&mut app, "page.png");

        app.history.replace(Route::Clip(1));
        let mut open = Open::new(1);
        let mut first: Clip = serde_json::from_value(clips[0].clone()).unwrap();
        first.recipients = vec![Recipient {
            user: Person { id: 2, username: "Mira".into(), avatar: None },
            shared_at: now(),
            hidden: true,
        }];
        open.title = first.name.clone();
        open.clip.data = Some(Some(Arc::new(first)));
        open.picking = true;
        open.send_to = vec![Person { id: 3, username: "Cranky".into(), avatar: None }];
        app.clips.people.data = Some(Arc::new(vec![
            Person { id: 2, username: "Mira".into(), avatar: None },
            Person { id: 3, username: "Cranky".into(), avatar: None },
        ]));
        app.clips.open = Some(open);
        draw(&mut app, "dialog.png");

        let mut rendering = Open::new(2);
        rendering.clip.data = Some(Some(Arc::new(serde_json::from_value(clips[1].clone()).unwrap())));
        app.clips.open = Some(rendering);
        app.history.replace(Route::Clip(2));
        draw(&mut app, "rendering.png");

        app.history.replace(Route::Clips);
        app.clips.open = None;
        app.clips.kind = Kind::Quotes;
        app.clips.put(
            (Scope::Mine, Kind::Quotes, None),
            json!({"clips": [clips[3].clone(), clips[3].clone()], "clipAllowance": you}),
        );
        draw(&mut app, "quotes.png");
    }
}
