// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::sync::Arc;

use crate::api::{self, Api, Token};
use crate::app::{Effect, Message as AppMessage};
use crate::images::{self, Images};
use crate::platform::{self, Desktop};
use crate::servers::{Config, Release, Store, User};
use crate::updates;

mod view;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Connect,
    Login,
    Ready,
    Invite,
    Reset,
    NewPassword,
    Servers,
    Link,
}

pub struct Session {
    pub config: Config,
    pub api: Option<Api>,
    pub connection: Option<api::connection::ResponseData>,
    pub screen: Screen,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub online: bool,
    pub generation: u64,
    pub revision: u64,
    pub last_event: Option<Arc<api::events::ResponseData>>,
    pub fallback: bool,
    pub address: String,
    pub system_dark: bool,
    /// Whether the desktop asks for less motion; Settings can override it.
    pub reduce_motion: bool,
    pub images: Images,
    /// The server waiting for "Remove" to be confirmed.
    pub removing: Option<u64>,
    username: String,
    password: String,
    profile: Option<String>,
    by_name: bool,
    link_token: String,
    link_valid: bool,
    pending_link: Option<String>,
    store: Option<Arc<Store>>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            config: Config::default(),
            api: None,
            connection: None,
            screen: Screen::Connect,
            busy: true,
            error: None,
            notice: None,
            online: false,
            generation: 0,
            revision: 0,
            last_event: None,
            fallback: false,
            address: String::new(),
            system_dark: true,
            reduce_motion: false,
            images: Images::default(),
            removing: None,
            username: String::new(),
            password: String::new(),
            profile: None,
            by_name: false,
            link_token: String::new(),
            link_valid: false,
            pending_link: None,
            store: None,
        }
    }
}

#[derive(Clone)]
pub enum Message {
    Start,
    Address(String),
    Username(String),
    Password(String),
    Profile(String),
    ByName,
    Connect,
    Add,
    Manage,
    EnterLink,
    Cancel,
    Submit,
    Switch(u64),
    SignOut(u64),
    Remove(u64),
    Reset,
    Login,
    Link(String),
    Updates(bool),
    ReduceMotion(Option<bool>),
    AskRemove(Option<u64>),
    DismissMismatch,
    AcknowledgeFallback,
    OpenRelease,
    Done(u64, std::result::Result<Arc<Outcome>, api::Error>),
    Live(u64, api::Live),
    UpdateResult(Option<Release>),
    Tick,
}

/// Where a sign-in profile's picture is.
pub fn profile_avatar(key: &str) -> String {
    let mut url = url::Url::parse("http://server/api/sign-in/").expect("a URL");
    url.path_segments_mut().expect("a path").pop_if_empty().push(key).push("avatar");
    url.path().trim_start_matches('/').to_owned()
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionMessage([redacted])")
    }
}

pub enum Outcome {
    Loaded(Arc<Store>, Config, Option<Api>, bool),
    Connected(Config, Api, Box<api::connection::ResponseData>),
    Activated(Config, Api, bool),
    SignedIn(Config, Api, bool),
    SignedOut(Config),
    Refreshed(Box<api::connection::ResponseData>),
    /// Pictures that arrived: their path, their bytes (or nothing) and whether they're from the
    /// network rather than the disk.
    Pictures(Vec<(String, Option<iced_widget::image::Handle>, bool)>),
    LinkValid(Option<String>),
    Sent,
    Saved(Config),
    Opened,
}

impl Session {
    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) {
        match message {
            Message::Start => {
                self.busy = true;
                self.error = None;
                self.task(effects, async {
                    let store = Store::load(platform::Current::secrets()).await.map_err(storage_error)?;
                    let config = store.config.lock().await.clone();
                    let (config, api, fallback) = match config.active {
                        Some(id) => {
                            let (config, api, fallback) = store.activate(id).await.map_err(storage_error)?;
                            (config, Some(api), fallback)
                        },
                        None => (config, None, false),
                    };
                    Ok(Outcome::Loaded(store, config, api, fallback))
                });
            },
            Message::Address(value) => self.address = value,
            Message::Username(value) => self.username = value,
            Message::Password(value) => self.password = value,
            Message::Profile(key) if !self.busy => {
                self.profile = Some(key);
                self.password.clear();
                self.error = None;
            },
            Message::ByName if !self.busy => {
                self.by_name = true;
                self.profile = None;
                self.error = None;
            },
            Message::Connect if !self.busy => {
                let Some(store) = self.store.clone() else { return };
                let input = self.address.clone();
                self.busy = true;
                self.error = None;
                self.task(effects, async move {
                    let (api, data) = api::probe(&input).await?;
                    let config = store.add(&api).await.map_err(storage_error)?;
                    Ok(Outcome::Connected(config, api, Box::new(data)))
                });
            },
            Message::Add if !self.busy => {
                self.reset(effects);
                self.screen = Screen::Connect;
                self.address.clear();
            },
            Message::Manage if !self.busy => {
                self.screen = Screen::Servers;
                self.clear_form();
            },
            Message::EnterLink if !self.busy => {
                self.screen = Screen::Link;
                self.address.clear();
                self.clear_form();
            },
            Message::Cancel if !self.busy => {
                if let Some(id) = self.config.active {
                    self.update(Message::Switch(id), effects);
                } else {
                    self.screen = Screen::Connect;
                }
            },
            Message::Switch(id) if !self.busy => {
                let Some(store) = self.store.clone() else { return };
                self.reset(effects);
                self.busy = true;
                self.task(effects, async move {
                    let (config, api, fallback) = store.activate(id).await.map_err(storage_error)?;
                    Ok(Outcome::Activated(config, api, fallback))
                });
            },
            Message::SignOut(id) | Message::Remove(id) if !self.busy => {
                let remove = matches!(message, Message::Remove(_));
                self.removing = None;
                self.sign_out(id, remove, effects);
            },
            Message::AskRemove(id) => self.removing = id,
            Message::Submit if !self.busy && self.screen == Screen::Connect => self.update(Message::Connect, effects),
            Message::Submit if !self.busy && self.screen == Screen::Link => {
                self.update(Message::Link(self.address.clone()), effects);
            },
            Message::Submit if !self.busy => self.submit(effects),
            Message::Reset if !self.busy => {
                self.screen = Screen::Reset;
                self.profile = None;
                self.clear_form();
            },
            Message::Login if !self.busy => {
                self.screen = Screen::Login;
                self.clear_form();
                self.profile = None;
                self.by_name = false;
                self.link_token.clear();
            },
            Message::Link(link) if !self.busy => self.open_link(&link, effects),
            Message::Updates(enabled) => {
                let Some(store) = self.store.clone() else { return };
                self.config.check_updates = enabled;
                self.task(effects, async move {
                    Ok(Outcome::Saved(
                        store.save(|config| config.check_updates = enabled).await.map_err(storage_error)?,
                    ))
                });
            },
            Message::ReduceMotion(reduce) => {
                let Some(store) = self.store.clone() else { return };
                self.config.reduce_motion = reduce;
                self.task(effects, async move {
                    Ok(Outcome::Saved(store.save(|config| config.reduce_motion = reduce).await.map_err(storage_error)?))
                });
            },
            Message::DismissMismatch => {
                let Some(key) = self.mismatch_key() else { return };
                let Some(store) = self.store.clone() else { return };
                self.config.dismissed.push(key.clone());
                self.task(effects, async move {
                    Ok(Outcome::Saved(
                        store
                            .save(|config| {
                                if !config.dismissed.contains(&key) {
                                    config.dismissed.push(key)
                                }
                            })
                            .await
                            .map_err(storage_error)?,
                    ))
                });
            },
            Message::AcknowledgeFallback => {
                let Some(store) = self.store.clone() else { return };
                self.config.fallback_seen = true;
                self.task(effects, async move {
                    Ok(Outcome::Saved(store.save(|config| config.fallback_seen = true).await.map_err(storage_error)?))
                });
            },
            Message::OpenRelease => {
                if let Some(release) = &self.config.release {
                    let url = release.url.clone();
                    self.task(effects, async move {
                        platform::Current::open_url(url).await.map_err(api::Error::message)?;
                        Ok(Outcome::Opened)
                    });
                }
            },
            Message::Done(generation, _) if generation != self.generation => {},
            Message::Done(_, result) => match result {
                Ok(outcome) => self.completed(outcome.as_ref(), effects),
                Err(error) => {
                    self.busy = false;
                    self.password.clear();
                    self.error = Some(error.message);
                    if error.unauthorized
                        && self.api.as_ref().is_some_and(|api| api.token.is_some())
                        && let Some(id) = self.config.active
                    {
                        self.sign_out(id, false, effects);
                    }
                },
            },
            Message::Live(generation, _) if generation != self.generation => {},
            Message::Live(_, event) => match event {
                api::Live::Connected => {
                    self.images.clear();
                    self.online = true;
                    self.revision += 1;
                    self.refresh(effects);
                },
                api::Live::Offline => self.online = false,
                api::Live::Unauthorized => {
                    if let Some(id) = self.config.active {
                        self.sign_out(id, false, effects);
                    }
                },
                api::Live::Event(event) => {
                    let refresh = match &event.events {
                        api::events::EventsEvents::ConfigChanged(_) => true,
                        api::events::EventsEvents::ListChanged(change) => matches!(
                            change.list,
                            api::events::ChangedList::USERS | api::events::ChangedList::APPEARANCE
                        ),
                        _ => false,
                    };
                    if matches!(event.events, api::events::EventsEvents::ListChanged(ref change) if matches!(change.list, api::events::ChangedList::USERS))
                    {
                        self.images.clear();
                    }
                    self.last_event = Some(event);
                    self.revision += 1;
                    if refresh {
                        self.refresh(effects);
                    }
                },
            },
            Message::UpdateResult(release) => self.config.release = release,
            Message::Tick => {
                if let Some(store) = self.store.clone() {
                    effects.push(Effect::Task(Box::pin(async move {
                        AppMessage::Session(Message::UpdateResult(updates::check(store).await.unwrap_or(None)))
                    })));
                }
            },
            _ => {},
        }
    }

    fn task(
        &self,
        effects: &mut Vec<Effect>,
        future: impl Future<Output = std::result::Result<Outcome, api::Error>> + Send + 'static,
    ) {
        let generation = self.generation;
        effects.push(Effect::Task(Box::pin(async move {
            AppMessage::Session(Message::Done(generation, future.await.map(Arc::new)))
        })));
    }

    fn request(
        &self,
        effects: &mut Vec<Effect>,
        future: impl Future<Output = std::result::Result<Outcome, api::Error>> + Send + 'static,
    ) {
        let generation = self.generation;
        effects.push(Effect::Request(Box::pin(async move {
            AppMessage::Session(Message::Done(generation, future.await.map(Arc::new)))
        })));
    }

    fn reset(&mut self, effects: &mut Vec<Effect>) {
        self.generation += 1;
        self.revision += 1;
        effects.push(Effect::Live(None, self.generation));
        self.api = None;
        self.last_event = None;
        self.screen = Screen::Login;
        self.connection = None;
        self.images.clear();
        self.online = false;
        self.profile = None;
        self.by_name = false;
        self.link_token.clear();
        self.clear_form();
    }

    fn clear_form(&mut self) {
        self.username.clear();
        self.password.clear();
        self.error = None;
        self.notice = None;
        self.link_valid = false;
    }

    fn completed(&mut self, outcome: &Outcome, effects: &mut Vec<Effect>) {
        match outcome {
            Outcome::Loaded(store, config, api, fallback) => {
                self.store = Some(store.clone());
                self.config = config.clone();
                self.fallback = *fallback;
                self.busy = false;
                self.update(Message::Tick, effects);
                if let Some(link) = self.pending_link.take() {
                    self.open_link(&link, effects);
                } else if let Some(api) = api {
                    self.api = Some(api.clone());
                    self.screen = Screen::Login;
                    self.busy = true;
                    self.refresh(effects);
                }
            },
            Outcome::Activated(config, api, fallback) | Outcome::SignedIn(config, api, fallback) => {
                if matches!(outcome, Outcome::SignedIn(..)) {
                    self.reset(effects);
                }
                self.config = config.clone();
                self.api = Some(api.clone());
                self.fallback = *fallback;
                self.busy = true;
                self.screen = Screen::Login;
                self.password.clear();
                self.link_token.clear();
                self.refresh(effects);
            },
            Outcome::Connected(config, api, data) => {
                self.reset(effects);
                self.config = config.clone();
                self.api = Some(api.clone());
                self.address.clear();
                self.accept_connection(data.as_ref().clone(), effects);
            },
            Outcome::Refreshed(data) => self.accept_connection(data.as_ref().clone(), effects),
            Outcome::SignedOut(config) => {
                self.config = config.clone();
                self.busy = false;
                if let Some(id) = config.active {
                    self.update(Message::Switch(id), effects);
                } else {
                    self.screen = Screen::Connect;
                }
            },
            Outcome::Pictures(pictures) => {
                for (path, bytes, fresh) in pictures {
                    self.images.arrived(path.clone(), bytes.clone(), *fresh);
                }
            },
            Outcome::LinkValid(who) => {
                self.busy = false;
                self.link_valid = who.is_some();
                self.notice = who.clone();
                if who.is_none() {
                    self.error = Some("This link doesn't work anymore. Ask for a new one.".into());
                }
            },
            Outcome::Sent => {
                self.busy = false;
                self.notice =
                    Some("If that account has an email address, a link is on its way. It works for an hour.".into());
            },
            Outcome::Saved(config) => {
                self.config = config.clone();
            },
            Outcome::Opened => {},
        }
    }

    fn accept_connection(&mut self, data: api::connection::ResponseData, effects: &mut Vec<Effect>) {
        self.busy = false;
        self.error = None;
        let ready = data.viewer.is_some();
        if let Some(user) = &data.viewer
            && let Some(server) = self.config.servers.iter_mut().find(|s| Some(s.id) == self.config.active)
        {
            server.user = Some(User { id: user.id, username: user.username.clone(), avatar: user.avatar.clone() });
        }
        if !matches!(self.screen, Screen::Invite | Screen::Reset | Screen::NewPassword | Screen::Servers | Screen::Link)
        {
            self.screen = if ready { Screen::Ready } else { Screen::Login };
        }
        let mut wanted: Vec<String> = data
            .sign_in_profiles
            .iter()
            .filter(|p| p.avatar.is_some())
            .map(|p| profile_avatar(&p.key))
            .chain(data.viewer.as_ref().and_then(|v| v.avatar.clone()))
            .collect();
        wanted.retain(|path| self.images.want(path));
        self.connection = Some(data);
        if ready
            && !self.online
            && let Some(api) = self.api.clone()
        {
            effects.push(Effect::Live(Some(api), self.generation));
        }
        if !wanted.is_empty()
            && let Some(api) = self.api.clone()
        {
            self.pictures(effects, api, wanted);
        }
        if let Some(link) = self.pending_link.take() {
            self.open_link(&link, effects);
        }
    }

    /// Shows what's on disk for `paths` at once, then what the server has.
    fn pictures(&self, effects: &mut Vec<Effect>, api: Api, paths: Vec<String>) {
        let cached = api.clone();
        let from_disk = paths.clone();
        self.request(effects, async move {
            let mut pictures = vec![];
            for path in from_disk {
                if let Some(bytes) = images::cached(&cached, &path).await {
                    pictures.push((path, images::decode(bytes).await, false));
                }
            }
            Ok(Outcome::Pictures(pictures))
        });
        self.request(effects, async move {
            let mut pictures = vec![];
            for path in paths {
                let handle = match images::fetch(&api, &path).await {
                    Some(bytes) => images::decode(bytes).await,
                    None => None,
                };
                pictures.push((path, handle, true));
            }
            Ok(Outcome::Pictures(pictures))
        });
    }

    fn refresh(&self, effects: &mut Vec<Effect>) {
        if let Some(api) = self.api.clone() {
            self.request(effects, async move { api.connection().await.map(|data| Outcome::Refreshed(Box::new(data))) });
        }
    }

    fn sign_out(&mut self, id: u64, remove: bool, effects: &mut Vec<Effect>) {
        let Some(store) = self.store.clone() else { return };
        let api = (self.config.active == Some(id)).then(|| self.api.clone()).flatten();
        self.reset(effects);
        self.busy = true;
        self.screen = Screen::Login;
        self.task(effects, async move {
            let api = match api {
                Some(api) => Some(api),
                None => store.client(id).await.ok().map(|(api, _)| api),
            };
            let config = store.sign_out(id, remove).await.map_err(storage_error)?;
            if let Some(api) = api
                && api.token.is_some()
            {
                let _ = api.request::<api::SignOut>(api::sign_out::Variables).await;
            }
            Ok(Outcome::SignedOut(config))
        });
    }

    fn submit(&mut self, effects: &mut Vec<Effect>) {
        let (Some(api), Some(store), Some(id)) = (self.api.clone(), self.store.clone(), self.config.active) else {
            return;
        };
        let screen = self.screen.clone();
        let username = self.username.trim().to_owned();
        let password = std::mem::take(&mut self.password);
        let profile = self.profile.clone();
        let token = self.link_token.clone();
        let setup = self.connection.as_ref().is_some_and(|c| c.server.setup_required);
        if (screen == Screen::Reset && username.is_empty())
            || (screen != Screen::Reset
                && (password.is_empty() || (profile.is_none() && screen != Screen::NewPassword && username.is_empty())))
            || (matches!(screen, Screen::Invite | Screen::NewPassword) && !self.link_valid)
        {
            return;
        }
        self.busy = true;
        self.error = None;
        self.task(effects, async move {
            if screen == Screen::Reset {
                api.request::<api::RequestPasswordReset>(api::request_password_reset::Variables { who: username })
                    .await?;
                return Ok(Outcome::Sent);
            }
            macro_rules! signed {
                ($op:ty, $variables:expr, $field:ident) => {{
                    let signed = api.request::<$op>($variables).await?.$field;
                    (
                        Token(signed.token),
                        User { id: signed.user.id, username: signed.user.username, avatar: signed.user.avatar },
                    )
                }};
            }
            let (token, user) = match screen {
                Screen::Invite => signed!(
                    api::AcceptInvite,
                    api::accept_invite::Variables { token, username, password },
                    accept_invite
                ),
                Screen::NewPassword => {
                    signed!(api::ResetPassword, api::reset_password::Variables { token, password }, reset_password)
                },
                _ if setup => signed!(api::Setup, api::setup::Variables { username, password }, setup),
                _ => signed!(
                    api::SignIn,
                    api::sign_in::Variables { username: profile.is_none().then_some(username), profile, password },
                    sign_in
                ),
            };
            let (config, fallback) = store.signed_in(id, token.clone(), user).await.map_err(storage_error)?;
            let api = Api::new(api.origin, Some(token))?;
            Ok(Outcome::SignedIn(config, api, fallback))
        });
    }

    /// #92 can dispatch incoming web and tinystream:// links through the same message.
    fn open_link(&mut self, link: &str, effects: &mut Vec<Effect>) {
        let Ok(url) = url::Url::parse(link) else {
            self.error = Some("Paste the full invite or reset link.".into());
            return;
        };
        if !matches!(url.scheme(), "http" | "https" | "tinystream")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
        {
            self.error = Some("Use an invite or reset link without credentials or a query.".into());
            return;
        }
        let target = match url.path() {
            "/invite" => Screen::Invite,
            "/reset" => {
                if url.fragment().is_some_and(|s| !s.is_empty()) {
                    Screen::NewPassword
                } else {
                    Screen::Reset
                }
            },
            _ => {
                self.error = Some("Use an invite or reset link.".into());
                return;
            },
        };
        let matches = |origin: &url::Url| {
            if url.scheme() == "tinystream" {
                origin.host_str() == url.host_str() && origin.port() == url.port()
            } else {
                origin.origin() == url.origin()
            }
        };
        if self.api.as_ref().is_none_or(|api| !matches(&api.origin)) {
            let saved = self
                .config
                .servers
                .iter()
                .find(|server| url::Url::parse(&server.url).is_ok_and(|origin| matches(&origin)))
                .map(|server| server.id);
            self.pending_link = Some(link.to_string());
            if let Some(id) = saved {
                self.update(Message::Switch(id), effects);
            } else {
                self.reset(effects);
                self.screen = Screen::Connect;
                self.address = if url.scheme() == "tinystream" {
                    url.authority().to_string()
                } else {
                    url.origin().ascii_serialization()
                };
            }
            return;
        }
        if self.api.as_ref().is_some_and(|api| api.token.is_some()) {
            self.error = Some("Sign out to use this link.".into());
            self.pending_link = Some(link.to_string());
            return;
        }
        self.clear_form();
        self.address.clear();
        self.screen = target;
        self.link_token = url.fragment().unwrap_or_default().to_string();
        if matches!(self.screen, Screen::Invite | Screen::NewPassword) {
            let api = self.api.clone().expect("connected");
            let token = self.link_token.clone();
            let invite = self.screen == Screen::Invite;
            self.busy = true;
            self.task(effects, async move {
                let who = if invite {
                    api.request::<api::Invite>(api::invite::Variables { token })
                        .await?
                        .invite
                        .map(|_| "Pick a username and password.".to_string())
                } else {
                    api.request::<api::PasswordReset>(api::password_reset::Variables { token })
                        .await?
                        .password_reset
                        .map(|name| format!("For {name}. You'll be signed out everywhere else."))
                };
                Ok(Outcome::LinkValid(who))
            });
        }
    }

    pub fn focus_target(&self) -> Option<&'static str> {
        match self.screen {
            Screen::Connect if self.store.is_none() => None,
            Screen::Invite | Screen::NewPassword if !self.link_valid => None,
            Screen::Connect | Screen::Link => Some("auth-address"),
            Screen::Login if self.profile.is_some() => Some("auth-password"),
            Screen::Login if self.show_profiles() => None,
            Screen::Login | Screen::Invite | Screen::Reset => Some("auth-username"),
            Screen::NewPassword => Some("auth-password"),
            _ => None,
        }
    }

    pub fn initial_link(&mut self, link: String) {
        self.pending_link = Some(link);
    }

    fn mismatch_key(&self) -> Option<String> {
        let server = self.connection.as_ref()?;
        let version = semver::Version::parse(server.server.version.trim_start_matches('v')).ok()?;
        Some(format!("{}@{}.{}", self.config.active?, version.major, version.minor))
    }

    pub fn mismatch(&self) -> Option<&str> {
        let version = &self.connection.as_ref()?.server.version;
        (updates::mismatch(version) && !self.config.dismissed.contains(&self.mismatch_key()?)).then_some(version)
    }
}

fn storage_error(_: anyhow::Error) -> api::Error {
    api::Error::message("Couldn't access saved servers or credentials. Check your keyring and file permissions.")
}

#[cfg(debug_assertions)]
impl Session {
    /// Local-only scenes for checking layout without opening windows or touching accounts.
    pub async fn preview(&mut self, scene: &str) -> anyhow::Result<()> {
        use serde_json::json;

        self.busy = false;
        self.online = true;
        self.store = Store::open(
            std::env::temp_dir().join(format!("tinystream-preview-{}", std::process::id())).join("servers.json"),
            platform::Current::secrets(),
        )
        .await
        .ok();
        if scene == "connect" {
            return Ok(());
        }
        if scene.starts_with("http://") || scene.starts_with("https://") {
            let api = Api::new(url::Url::parse(scene)?, None)?;
            self.connection = Some(api.connection().await?);
            self.api = Some(api);
            self.screen = Screen::Login;
            return Ok(());
        }
        self.api = Some(
            Api::new(url::Url::parse("https://tv.example.com/").expect("preview URL"), None).expect("preview API"),
        );
        self.config.servers = vec![
            crate::servers::Server {
                id: 1,
                url: "https://tv.example.com/".into(),
                name: "tv.example.com".into(),
                user: Some(User { id: 1, username: "David".into(), avatar: None }),
            },
            crate::servers::Server {
                id: 2,
                url: "http://tinystream.lan:3000/".into(),
                name: "tinystream.lan:3000".into(),
                user: Some(User { id: 2, username: "Mira".into(), avatar: None }),
            },
            crate::servers::Server {
                id: 3,
                url: "https://a-very-long-server-address.example.com/tinystream/".into(),
                name: "a-very-long-server-address.example.com".into(),
                user: None,
            },
        ];
        self.config.active = Some(1);
        self.connection = Some(serde_json::from_value(json!({
            "server": { "version": updates::VERSION, "signInStyle": "PROFILES", "setupRequired": false, "email": true },
            "appearance": { "mode": "SINGLE", "style": "LAYERED", "mediaTint": false, "light": { "palette": { "tokens": [] } }, "dark": { "palette": { "tokens": [] } } },
            "signInProfiles": [ { "key": "one", "avatar": null }, { "key": "two", "avatar": null }, { "key": "three", "avatar": null } ],
            "viewer": null
        })).expect("preview connection"));
        self.screen = match scene {
            "servers" => Screen::Servers,
            "settings" | "credits" | "account" => Screen::Ready,
            "reset" => Screen::Reset,
            "invite" => {
                self.link_valid = true;
                Screen::Invite
            },
            _ => Screen::Login,
        };
        self.by_name = scene == "login";
        if scene == "unlock" {
            self.profile = Some("one".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::{self, Directory, MemorySecrets, run};

    async fn finish(session: &mut Session, mut effects: Vec<Effect>) {
        for _ in 0..12 {
            if effects.is_empty() {
                return;
            }
            let pending = std::mem::take(&mut effects);
            for effect in pending {
                if let Effect::Task(task) | Effect::Request(task) = effect
                    && let AppMessage::Session(message) = task.await
                {
                    session.update(message, &mut effects);
                }
            }
        }
        panic!("session did not settle");
    }

    #[test]
    fn password_profile_setup_invite_and_reset_sign_in() {
        run(async {
            for (operation, profile) in [
                ("SignIn", false),
                ("SignIn", true),
                ("Setup", false),
                ("AcceptInvite", false),
                ("ResetPassword", false),
            ] {
                let (origin, server) = test_support::http(2, |request| {
                    let field = match request["operationName"].as_str().unwrap() {
                        "SignIn" => "signIn",
                        "Setup" => "setup",
                        "AcceptInvite" => "acceptInvite",
                        "ResetPassword" => "resetPassword",
                        "Connection" => return test_support::connection(true),
                        _ => panic!("unexpected operation"),
                    };
                    json!({"data":{field:{"token":"test-token","user":{"id":1,"username":"David","avatar":null}}}})
                })
                .await;
                let dir = Directory::new("auth-flow");
                let store = Store::open(dir.0.join("servers.json"), Arc::new(MemorySecrets::default())).await.unwrap();
                let api = Api::new(origin, None).unwrap();
                let config = store.add(&api).await.unwrap();
                let mut data: api::connection::ResponseData =
                    serde_json::from_value(test_support::connection(false)["data"].clone()).unwrap();
                data.server.setup_required = operation == "Setup";
                let mut session = Session {
                    store: Some(store),
                    config,
                    api: Some(api),
                    busy: false,
                    connection: Some(data),
                    screen: match operation {
                        "AcceptInvite" => Screen::Invite,
                        "ResetPassword" => Screen::NewPassword,
                        _ => Screen::Login,
                    },
                    username: "David".into(),
                    password: "test-password".into(),
                    profile: profile.then(|| "opaque-profile".into()),
                    link_token: "fragment-token".into(),
                    link_valid: true,
                    ..Session::default()
                };
                let mut effects = vec![];
                session.update(Message::Submit, &mut effects);
                finish(&mut session, effects).await;
                assert_eq!(session.screen, Screen::Ready);
                assert!(session.password.is_empty());
                assert!(session.link_token.is_empty());
                assert_eq!(session.api.as_ref().unwrap().token.as_ref().unwrap().0, "test-token");
                let requests = server.await.unwrap();
                assert_eq!(requests[0].1["operationName"], operation);
                if profile {
                    assert_eq!(requests[0].1["variables"]["profile"], "opaque-profile");
                    assert!(requests[0].1["variables"]["username"].is_null());
                }
                assert!(requests[1].0.to_lowercase().contains("authorization: bearer test-token"));
            }
        });
    }

    #[test]
    fn invite_and_reset_fragments_follow_the_right_server() {
        run(async {
            let (origin, server) = test_support::http(1, |request| {
                assert_eq!(request["operationName"], "PasswordReset");
                assert_eq!(request["variables"]["token"], "fragment-token");
                json!({"data":{"passwordReset":"David"}})
            })
            .await;
            let mut session =
                Session { api: Some(Api::new(origin.clone(), None).unwrap()), busy: false, ..Session::default() };
            let link = format!("tinystream://{}/reset#fragment-token", origin.authority());
            let mut effects = vec![];
            session.update(Message::Link(link), &mut effects);
            finish(&mut session, effects).await;
            assert_eq!(session.screen, Screen::NewPassword);
            assert!(session.link_valid);
            assert!(session.address.is_empty());
            let requests = server.await.unwrap();
            assert!(!requests[0].0.contains("fragment-token"));

            let mut unknown = Session { busy: false, ..Session::default() };
            unknown.update(Message::Link("https://other.example/invite#private-token".into()), &mut vec![]);
            assert_eq!(unknown.screen, Screen::Connect);
            assert_eq!(unknown.address, "https://other.example");
            assert!(unknown.pending_link.is_some());
        });
    }

    #[test]
    fn wrong_password_keeps_the_profile_and_username() {
        let mut session = Session {
            api: Some(Api::new(url::Url::parse("https://example.com").unwrap(), None).unwrap()),
            username: "David".into(),
            profile: Some("profile".into()),
            screen: Screen::Login,
            ..Session::default()
        };
        let mut effects = vec![];
        session.update(
            Message::Done(0, Err(api::Error { unauthorized: true, message: "wrong password".into() })),
            &mut effects,
        );
        assert_eq!(session.username, "David");
        assert_eq!(session.profile.as_deref(), Some("profile"));
        assert_eq!(session.error.as_deref(), Some("wrong password"));
        assert!(!session.busy);
        assert!(effects.is_empty());
    }

    #[test]
    fn old_server_replies_cannot_repopulate_a_new_session() {
        let mut session = Session { busy: false, ..Session::default() };
        let mut effects = vec![];
        session.update(Message::Add, &mut effects);
        let data = serde_json::from_value(test_support::connection(true)["data"].clone()).unwrap();
        session.update(Message::Done(0, Ok(Arc::new(Outcome::Refreshed(Box::new(data))))), &mut effects);
        session.update(Message::Live(0, api::Live::Connected), &mut effects);
        assert!(session.connection.is_none());
        assert!(!session.online);
        assert_eq!(session.screen, Screen::Connect);
    }

    #[test]
    fn reconnect_invalidates_and_refetches_the_current_server() {
        let mut session = Session {
            api: Some(Api::new(url::Url::parse("https://example.com").unwrap(), Some(Token("test".into()))).unwrap()),
            ..Session::default()
        };
        let mut effects = vec![];
        session.update(Message::Live(0, api::Live::Connected), &mut effects);
        session.update(Message::Live(0, api::Live::Offline), &mut effects);
        session.update(Message::Live(0, api::Live::Connected), &mut effects);
        assert_eq!(session.revision, 2);
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Request(_))).count(), 2);
    }
}
