// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native settings editors share the web's operations and the server's validation.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};

mod fields;
mod requests;
pub mod view;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Account,
    Appearance,
    Libraries,
    Music,
    Clips,
    Skipped,
    Torrents,
    Sources,
    Profiles,
    Automation,
    Renames,
    General,
    Users,
    File,
    Desktop,
    Credits,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Account => "Account",
            Self::Appearance => "Appearance",
            Self::Libraries => "Libraries",
            Self::Music => "Music",
            Self::Clips => "Clips",
            Self::Skipped => "Skipped files",
            Self::Torrents => "Torrents",
            Self::Sources => "Sources",
            Self::Profiles => "Quality profiles",
            Self::Automation => "Automation",
            Self::Renames => "Renames",
            Self::General => "General",
            Self::Users => "Users",
            Self::File => "Config file",
            Self::Desktop => "Desktop",
            Self::Credits => "Credits",
        }
    }

    pub fn icon(self) -> crate::ui::icon::Icon {
        use crate::ui::icon::Icon;
        match self {
            Self::Account => Icon::UserRound,
            Self::Appearance => Icon::Sun,
            Self::Libraries => Icon::BookOpen,
            Self::Music => Icon::Music,
            Self::Clips => Icon::Scissors,
            Self::Skipped => Icon::ShieldAlert,
            Self::Torrents => Icon::ArrowDownToLine,
            Self::Sources => Icon::Radio,
            Self::Profiles => Icon::Settings2,
            Self::Automation => Icon::Sparkles,
            Self::Renames => Icon::Film,
            Self::General => Icon::Server,
            Self::Users => Icon::Users,
            Self::File => Icon::Keyboard,
            Self::Desktop => Icon::Settings,
            Self::Credits => Icon::Sparkles,
        }
    }

    pub fn admin(self) -> bool {
        !matches!(self, Self::Account | Self::Appearance | Self::Desktop | Self::Credits)
    }

    pub fn downloads(self) -> bool {
        matches!(self, Self::Torrents | Self::Sources | Self::Profiles | Self::Automation | Self::Renames)
    }

    fn queries(self) -> &'static [&'static str] {
        match self {
            Self::Account => &["Passkeys", "MyEmail", "AppPasswords"],
            Self::Appearance => &["ColorSchemes", "AppearanceSettings"],
            Self::Libraries
            | Self::Music
            | Self::Sources
            | Self::Profiles
            | Self::Automation
            | Self::General
            | Self::File => &["Settings"],
            Self::Clips => &["Settings", "ClipStorage"],
            Self::Skipped => &["SkippedFiles"],
            Self::Torrents => &["Settings", "SettingsEngine"],
            Self::Renames => &["RenameSuggestions", "FileHistory"],
            Self::Users => &["Settings", "Users", "PermissionDefaults", "Invites"],
            _ => &[],
        }
    }
}

#[derive(Clone)]
pub struct Action {
    pub operation: &'static str,
    pub variables: Value,
    pub confirm: Option<String>,
}
impl Action {
    pub fn new(operation: &'static str, variables: Value) -> Self {
        Self { operation, variables, confirm: None }
    }

    pub fn confirm(mut self, message: impl Into<String>) -> Self {
        self.confirm = Some(message.into());
        self
    }
}

#[derive(Clone)]
pub struct Editor {
    pub title: String,
    pub operation: &'static str,
    pub kind: &'static str,
    pub path: String,
    pub variables: Value,
    pub original: Value,
    pub text: BTreeMap<String, String>,
    pub invalid: BTreeMap<String, String>,
    pub confirmation: Option<String>,
    pub choices: BTreeMap<String, Vec<(String, String)>>,
    pub downloads: bool,
}
impl Editor {
    pub fn new(
        title: impl Into<String>,
        operation: &'static str,
        kind: &'static str,
        path: &str,
        variables: Value,
    ) -> Self {
        Self {
            title: title.into(),
            operation,
            kind,
            path: path.into(),
            original: variables.clone(),
            variables,
            text: BTreeMap::new(),
            invalid: BTreeMap::new(),
            confirmation: None,
            choices: BTreeMap::new(),
            downloads: true,
        }
    }

    pub fn value(&self) -> &Value {
        self.variables.pointer(&self.path).unwrap_or(&Value::Null)
    }

    fn change(&mut self, path: &str, value: Value) {
        if let Some(slot) = self.variables.pointer_mut(path) {
            *slot = value;
        }
    }
}

#[derive(Clone)]
pub enum Message {
    Tab(Tab),
    Loaded(u64, &'static str, u64, Result<Arc<Value>, api::Error>),
    Done(u64, Result<Arc<Value>, api::Error>),
    Edit(Editor),
    Close,
    Set(String, Value),
    Text(String, String, &'static str),
    Add(String, &'static str),
    Remove(String, usize),
    Move(String, usize, i32),
    Save,
    Act(Action),
    Confirm,
    Cancel,
    Retry,
    Copy(String),
    Raw(iced_widget::text_editor::Action),
    BrowseFolder(String),
    SelectRename(i64),
    SelectAllRenames,
    ShowLow(bool),
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SettingsMessage")
    }
}

#[derive(Default)]
struct Entry {
    value: Option<Arc<Value>>,
    pending: bool,
    serial: u64,
    error: Option<String>,
    dirty: bool,
}

pub struct Settings {
    pub tab: Tab,
    cache: BTreeMap<&'static str, Entry>,
    pub editor: Option<Editor>,
    pub confirm: Option<Action>,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub result: Option<Arc<Value>>,
    pub raw: iced_widget::text_editor::Content,
    pub selected_renames: std::collections::BTreeSet<i64>,
    pub show_low: bool,
    downloads: bool,
    generation: u64,
    revision: u64,
    serial: u64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            tab: Tab::Account,
            cache: BTreeMap::new(),
            editor: None,
            confirm: None,
            busy: false,
            error: None,
            notice: None,
            result: None,
            raw: iced_widget::text_editor::Content::new(),
            selected_renames: Default::default(),
            show_low: false,
            downloads: false,
            generation: 0,
            revision: 0,
            serial: 0,
        }
    }
}
impl Settings {
    pub fn data(&self, query: &str) -> Option<&Value> {
        self.cache.get(query)?.value.as_deref()
    }

    pub fn invalidate(&mut self) {
        for entry in self.cache.values_mut() {
            entry.dirty = true;
        }
    }

    pub fn follow(
        &mut self,
        generation: u64,
        revision: u64,
        admin: bool,
        downloads: bool,
        api: &Api,
        effects: &mut Vec<Effect>,
    ) {
        if generation != self.generation {
            *self = Self { generation, ..Self::default() };
        }
        self.downloads = downloads;
        if revision != self.revision {
            self.revision = revision;
        }
        if !admin {
            self.cache.retain(|key, _| {
                ["Passkeys", "MyEmail", "AppPasswords", "ColorSchemes", "AppearanceSettings"].contains(key)
            });
        }
        if (self.tab.admin() && !admin) || (self.tab.downloads() && !downloads) {
            self.tab = Tab::Account;
            self.editor = None;
            self.confirm = None;
        }
        let mut queries = self.tab.queries().to_vec();
        if admin && self.tab == Tab::Appearance {
            queries.push("ServerAppearance");
        }
        for query in queries {
            let entry = self.cache.entry(query).or_default();
            if entry.pending || (!entry.dirty && (entry.value.is_some() || entry.error.is_some())) {
                continue;
            }
            self.serial += 1;
            let serial = self.serial;
            entry.serial = serial;
            entry.pending = true;
            entry.dirty = false;
            let api = api.clone();
            effects.push(Effect::Request(Box::pin(async move {
                AppMessage::Settings(Message::Loaded(
                    generation,
                    query,
                    serial,
                    requests::request(&api, query, json!({})).await.map(Arc::new),
                ))
            })));
        }
    }

    pub fn update(&mut self, message: Message, api: &Api, effects: &mut Vec<Effect>) {
        match message {
            Message::Tab(tab) => {
                if !self.busy {
                    self.tab = tab;
                    self.editor = None;
                    self.result = None;
                    self.error = None;
                    self.notice = None;
                }
            },
            Message::Loaded(g, key, serial, result) => {
                if g != self.generation {
                    return;
                }
                let Some(entry) = self.cache.get_mut(key).filter(|e| e.serial == serial) else { return };
                entry.pending = false;
                match result {
                    Ok(value) => {
                        entry.value = Some(value);
                        entry.error = None;
                    },
                    Err(error) => entry.error = Some(error.message),
                }
            },
            Message::Done(g, result) => {
                if g != self.generation {
                    return;
                }
                self.busy = false;
                match result {
                    Ok(value) => {
                        if let Some(detected) = value.get("detectSource") {
                            let mut input = fields::default_value("SourceInput");
                            for key in ["name", "kind", "url", "feed"] {
                                if let Some(v) = detected.get(key) {
                                    input[key] = v.clone();
                                }
                            }
                            input["enabled"] = json!(true);
                            input["apiKey"] = self
                                .editor
                                .as_ref()
                                .and_then(|e| e.variables.get("apiKey"))
                                .cloned()
                                .unwrap_or(Value::Null);
                            self.editor = Some(Editor::new(
                                "Add source",
                                "AddSource",
                                "SourceInput",
                                "/input",
                                json!({"input":input}),
                            ));
                            self.error = None;
                            self.notice = Some("Source detected. Review it before saving.".into());
                            return;
                        }
                        self.selected_renames.clear();
                        self.result = Some(value);
                        self.notice = Some("Saved".into());
                        self.editor = None;
                        self.error = None;
                        self.invalidate();
                    },
                    Err(error) => self.error = Some(error.message),
                }
            },
            Message::Edit(mut editor) => {
                editor.downloads = self.downloads;
                self.cache.remove("Folders");
                if let Some(data) = self.data("ColorSchemes") {
                    let schemes: Vec<_> = data["colorSchemes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|s| Some((s["id"].as_str()?.to_owned(), s["name"].as_str()?.to_owned())))
                        .collect();
                    for key in ["single", "light", "dark"] {
                        editor.choices.insert(key.into(), schemes.clone());
                    }
                }
                if let Some(data) = self.data("Settings") {
                    for (key, array) in [("profile", "profiles"), ("libraries", "libraries")] {
                        let choices = data["settings"][array]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|s| s["name"].as_str().map(|n| (n.to_owned(), n.to_owned())))
                            .collect();
                        editor.choices.insert(key.into(), choices);
                    }
                }
                if !self.busy {
                    self.raw = iced_widget::text_editor::Content::with_text(
                        editor.variables.get("text").and_then(Value::as_str).unwrap_or_default(),
                    );
                    self.editor = Some(editor);
                    self.error = None;
                    self.result = None;
                }
            },
            Message::Close => {
                if !self.busy {
                    self.editor = None;
                    self.error = None;
                }
            },
            Message::Set(path, value) => {
                if !self.busy
                    && let Some(editor) = &mut self.editor
                {
                    editor.change(&path, value);
                    editor.text.remove(&path);
                    editor.invalid.remove(&path);
                }
            },
            Message::Text(path, value, kind) => {
                if self.busy {
                    return;
                }
                let Some(editor) = &mut self.editor else { return };
                let parsed = match kind {
                    "Int" => value.parse::<i32>().ok().map(|n| json!(n)),
                    "Color" => tinystream_theme::Color::parse(&value).filter(|c| c.opaque()).map(|_| json!(value)),
                    "Float" => value.parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| json!(n)),
                    _ => Some(json!(value)),
                };
                editor.text.insert(path.clone(), value);
                if let Some(parsed) = parsed {
                    editor.change(&path, parsed);
                    editor.invalid.remove(&path);
                } else {
                    editor.invalid.insert(
                        path,
                        if kind == "Color" {
                            "Enter an opaque color, such as #202020."
                        } else {
                            "Enter a valid number."
                        }
                        .into(),
                    );
                }
            },
            Message::Add(path, kind) => {
                if self.busy {
                    return;
                }
                if let Some(editor) = &mut self.editor
                    && let Some(list) = editor.variables.pointer_mut(&path).and_then(Value::as_array_mut)
                {
                    list.push(fields::default_value(kind));
                }
            },
            Message::Remove(path, index) | Message::Move(path, index, 0) => {
                if self.busy {
                    return;
                }
                if let Some(editor) = &mut self.editor
                    && let Some(list) = editor.variables.pointer_mut(&path).and_then(Value::as_array_mut)
                    && index < list.len()
                {
                    list.remove(index);
                    editor.text.clear();
                    editor.invalid.clear();
                }
            },
            Message::Move(path, index, delta) => {
                if self.busy {
                    return;
                }
                if let Some(editor) = &mut self.editor
                    && let Some(list) = editor.variables.pointer_mut(&path).and_then(Value::as_array_mut)
                {
                    let target = index as i32 + delta;
                    if target >= 0 && (target as usize) < list.len() && index < list.len() {
                        list.swap(index, target as usize);
                        editor.text.clear();
                        editor.invalid.clear();
                    }
                }
            },
            Message::Save => {
                let Some(editor) = &self.editor else { return };
                if self.busy || !editor.invalid.is_empty() {
                    return;
                }
                let mut action = Action::new(editor.operation, editor.variables.clone());
                action.confirm = editor.confirmation.clone();
                self.update(Message::Act(action), api, effects);
            },
            Message::Act(mut action) => {
                if self.busy {
                    return;
                }
                if action.confirm.is_some() {
                    self.confirm = Some(action);
                    return;
                }
                self.busy = true;
                self.error = None;
                self.notice = None;
                let generation = self.generation;
                let api = api.clone();
                effects.push(Effect::Request(Box::pin(async move {
                    AppMessage::Settings(Message::Done(
                        generation,
                        if action.operation == "Avatar" {
                            api.avatar(action.variables["userId"].as_i64(), action.variables["reset"] == true)
                                .await
                                .map(|_| Arc::new(json!({})))
                        } else {
                            requests::request(&api, action.operation, std::mem::take(&mut action.variables))
                                .await
                                .map(Arc::new)
                        },
                    ))
                })));
            },
            Message::Confirm => {
                if let Some(mut action) = self.confirm.take() {
                    action.confirm = None;
                    self.update(Message::Act(action), api, effects);
                }
            },
            Message::Cancel => self.confirm = None,
            Message::Retry => {
                self.invalidate();
                for entry in self.cache.values_mut() {
                    entry.error = None;
                }
            },
            Message::Raw(action) => {
                if self.busy {
                    return;
                }
                self.raw.perform(action);
                if let Some(editor) = &mut self.editor {
                    editor.variables["text"] = json!(self.raw.text());
                }
            },
            Message::BrowseFolder(path) => {
                self.serial += 1;
                let serial = self.serial;
                self.cache.insert("Folders", Entry { pending: true, serial, ..Entry::default() });
                let generation = self.generation;
                let api = api.clone();
                effects.push(Effect::Request(Box::pin(async move {
                    AppMessage::Settings(Message::Loaded(
                        generation,
                        "Folders",
                        serial,
                        requests::request(&api, "Folders", json!({"path":if path.is_empty(){None}else{Some(path)}}))
                            .await
                            .map(Arc::new),
                    ))
                })));
            },
            Message::SelectRename(id) => {
                if !self.selected_renames.remove(&id) {
                    self.selected_renames.insert(id);
                }
            },
            Message::SelectAllRenames => {
                let ids: std::collections::BTreeSet<_> = self
                    .data("RenameSuggestions")
                    .and_then(|v| v["renameSuggestions"].as_array())
                    .into_iter()
                    .flatten()
                    .filter(|r| self.show_low || r["confidence"] == "HIGH")
                    .filter_map(|r| r["id"].as_i64())
                    .collect();
                self.selected_renames = if ids == self.selected_renames { Default::default() } else { ids };
            },
            Message::ShowLow(show) => self.show_low = show,
            Message::Copy(value) => {
                effects.push(Effect::Copy(value));
                self.notice = Some("Copied".into());
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{self, run};
    fn api() -> Api {
        Api::new(url::Url::parse("http://localhost:9876").unwrap(), None).unwrap()
    }
    #[test]
    fn regular_users_never_fetch_admin_settings() {
        let mut state = Settings { tab: Tab::General, ..Settings::default() };
        let mut effects = vec![];
        state.follow(0, 0, false, false, &api(), &mut effects);
        assert_eq!(state.tab, Tab::Account);
        assert!(state.cache.keys().all(|key| ["Passkeys", "MyEmail", "AppPasswords"].contains(key)));
        state.tab = Tab::Sources;
        state.follow(0, 0, true, false, &api(), &mut effects);
        assert_eq!(state.tab, Tab::Account);
    }
    #[test]
    fn failed_save_retains_draft_and_invalid_numbers_block_save() {
        let mut state = Settings::default();
        let mut effects = vec![];
        state.update(
            Message::Edit(Editor::new(
                "Invite",
                "CreateInvite",
                "InviteFields",
                "",
                json!({"label":"Friends","maxUses":1,"expiresInHours":168,"email":null}),
            )),
            &api(),
            &mut effects,
        );
        state.update(Message::Text("/maxUses".into(), "-".into(), "Int"), &api(), &mut effects);
        state.update(Message::Save, &api(), &mut effects);
        assert!(effects.is_empty());
        state.update(Message::Text("/maxUses".into(), "2".into(), "Int"), &api(), &mut effects);
        state.update(Message::Save, &api(), &mut effects);
        assert_eq!(effects.len(), 1);
        state.update(Message::Done(0, Err(api::Error::message("Not allowed"))), &api(), &mut effects);
        assert!(!state.busy);
        assert_eq!(state.editor.as_ref().unwrap().variables["maxUses"], 2);
        assert_eq!(state.error.as_deref(), Some("Not allowed"));
    }
    #[test]
    fn invalidation_during_load_and_old_sessions_do_not_lose_updates() {
        let mut state = Settings::default();
        let mut effects = vec![];
        state.follow(0, 0, false, false, &api(), &mut effects);
        let serial = state.cache["Passkeys"].serial;
        state.invalidate();
        state.update(
            Message::Loaded(0, "Passkeys", serial, Ok(Arc::new(json!({"viewer":{"passkeys":[]}})))),
            &api(),
            &mut effects,
        );
        assert!(state.cache["Passkeys"].dirty);
        effects.clear();
        state.follow(0, 0, false, false, &api(), &mut effects);
        assert_eq!(effects.len(), 1);
        state.follow(2, 0, false, false, &api(), &mut effects);
        state.update(
            Message::Loaded(0, "Passkeys", serial, Ok(Arc::new(json!({"secret":"old"})))),
            &api(),
            &mut effects,
        );
        assert!(state.data("Passkeys").is_none());
    }
    #[test]
    fn destructive_actions_wait_for_confirmation() {
        let mut state = Settings::default();
        let mut effects = vec![];
        state.update(
            Message::Act(Action::new("DeleteUser", json!({"id":8})).confirm("Remove user?")),
            &api(),
            &mut effects,
        );
        assert!(effects.is_empty());
        assert!(state.confirm.is_some());
        state.update(Message::Cancel, &api(), &mut effects);
        assert!(state.confirm.is_none());
        assert!(!state.busy);
    }
    #[test]
    fn typed_settings_requests_use_shared_server_mutations_and_report_validation() {
        run(async {
            let (origin, server) = test_support::http(2, |request| {
                if request["operationName"] == "SettingsSaveSection" {
                    json!({"data":{"updateSettings":{"raw":"saved"}}})
                } else {
                    json!({"errors":[{"message":"name is already in use"}]})
                }
            })
            .await;
            let api = Api::new(origin, Some(api::Token("token".into()))).unwrap();
            requests::request(&api,"SaveSection",json!({"patch":{"music":{"onlineLyrics":false,"lyricsUrl":"https://lyrics.example","analyzeLoudness":true}}})).await.unwrap();
            let error = requests::request(
                &api,
                "CreateUser",
                json!({"input":{"username":"existing","password":"password","isAdmin":false,"permissions":{}}}),
            )
            .await
            .unwrap_err();
            assert_eq!(error.message, "name is already in use");
            let received = server.await.unwrap();
            assert!(received[0].0.to_lowercase().contains("authorization: bearer token"));
            assert_eq!(received[0].1["variables"]["patch"]["music"]["onlineLyrics"], false);
            assert!(received[0].1["query"].as_str().unwrap().contains("updateSettings"));
        });
    }
}

#[cfg(all(test, debug_assertions))]
mod render_tests {
    use super::*;
    #[test]
    #[ignore = "requires Vulkan; renders settings and verifies the save control"]
    fn settings_editor_renders_and_saves() {
        use iced_core::{Event, Point, Size, mouse};
        let output = std::env::var_os("TINYSTREAM_TEST_ARTIFACT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("tinystream-settings-qa"));
        std::fs::create_dir_all(&output).unwrap();
        let mut app = crate::app::App::new(crate::route::Route::Settings, crate::window_state::WindowState::default());
        crate::browsing::preview::prepare(&mut app, "home", crate::ui::Style::Layered, false, Some(true));
        app.history = crate::route::History::new(crate::route::Route::Settings);
        let draw = |app: &mut crate::app::App, name: &str, cursor, input: &[Event]| {
            crate::snapshot::snapshot(app, Size::new(1360, 1000), 1.0, cursor, &output.join(name), input).unwrap()
        };
        draw(&mut app, "account.png", None, &[]);
        app.settings.tab = Tab::Music;
        let mut editor = Editor::new(
            "Music",
            "SaveSection",
            "MusicConfigInput",
            "/patch/music",
            json!({"patch":{"music":{"onlineLyrics":true,"lyricsUrl":"https://lyrics.example","analyzeLoudness":true}}}),
        );
        editor.original["patch"]["music"]["onlineLyrics"] = json!(false);
        app.settings.editor = Some(editor);
        let messages = draw(
            &mut app,
            "music.png",
            Some(Point::new(626.0, 447.0)),
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
        );
        assert!(messages.iter().any(|m| matches!(m, AppMessage::Settings(Message::Save))));
        let grey = tinystream_theme::find_builtin("grey").unwrap();
        let seeds: Vec<_> = tinystream_theme::SEEDS
            .iter()
            .zip(grey.scheme.seeds)
            .map(|(name, c)| json!({"name":name,"value":format!("#{:02x}{:02x}{:02x}",c.r,c.g,c.b)}))
            .collect();
        app.settings.tab = Tab::Appearance;
        app.settings.editor = Some(Editor::new(
            "Color scheme",
            "SaveScheme",
            "SchemeInput",
            "/input",
            json!({"id":null,"input":{"name":"Night","seeds":seeds,"overrides":[]}}),
        ));
        draw(&mut app, "scheme.png", None, &[]);
        crate::snapshot::snapshot(&mut app, Size::new(768, 800), 1.0, None, &output.join("scheme-narrow.png"), &[])
            .unwrap();
    }
}
