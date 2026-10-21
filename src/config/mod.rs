// SPDX-License-Identifier: AGPL-3.0-or-later

mod store;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
pub use store::{ConfigStore, LibraryInput, List, SettingsPatch};

use crate::paths::resolve_config_path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Span(pub Duration);

impl std::ops::Deref for Span {
    type Target = Duration;

    fn deref(&self) -> &Duration {
        &self.0
    }
}

impl Serialize for Span {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        humantime_serde::serialize(&self.0, s)
    }
}

impl<'de> Deserialize<'de> for Span {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        humantime_serde::deserialize(d).map(Span)
    }
}

#[async_graphql::Scalar(name = "Duration")]
impl async_graphql::ScalarType for Span {
    fn parse(value: async_graphql::Value) -> async_graphql::InputValueResult<Self> {
        match &value {
            async_graphql::Value::String(s) => humantime::parse_duration(s.trim()).map(Span).map_err(|_| {
                async_graphql::InputValueError::custom(format!("{s:?} isn't a length of time like \"15m\""))
            }),
            _ => Err(async_graphql::InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> async_graphql::Value {
        async_graphql::Value::String(humantime::format_duration(self.0).to_string())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub log: Log,
    #[serde(default)]
    pub scan: Scan,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default)]
    pub transcode: Transcode,
    #[serde(default)]
    pub clips: Clips,
    #[serde(default)]
    pub music: Music,
    #[serde(default, rename = "library")]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub downloads: Downloads,
    #[serde(default)]
    pub automation: Automation,
    #[serde(default)]
    pub requests: Requests,
    #[serde(default)]
    pub sign_in: SignIn,
    #[serde(default)]
    pub smtp: Smtp,
    #[serde(default, rename = "source")]
    pub sources: Vec<Source>,
    #[serde(default, rename = "profile")]
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "NetworkConfig", input_name = "NetworkConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Network {
    pub host: String,
    pub port: u16,

    pub cors: Vec<String>,
}

impl Default for Network {
    fn default() -> Self {
        Self { host: "0.0.0.0".into(), port: 3000, cors: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "LogConfig", input_name = "LogConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Log {
    pub level: String,
}

impl Default for Log {
    fn default() -> Self {
        Self { level: "info".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "ScanConfig", input_name = "ScanConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Scan {
    pub watch: bool,

    pub interval: Option<Span>,
}

impl Default for Scan {
    fn default() -> Self {
        Self { watch: true, interval: None }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "MetadataConfig", input_name = "MetadataConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Metadata {
    pub tmdb_api_key: Option<String>,

    pub language: String,
}

impl Default for Metadata {
    fn default() -> Self {
        Self { tmdb_api_key: None, language: "en-US".into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum Hardware {
    #[default]
    Auto,
    Vaapi,
    Software,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "TranscodeConfig", input_name = "TranscodeConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Transcode {
    pub hardware: Hardware,
    pub vaapi_device: String,
}

impl Default for Transcode {
    fn default() -> Self {
        Self { hardware: Hardware::Auto, vaapi_device: "/dev/dri/renderD128".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "ClipsConfig", input_name = "ClipsConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Clips {
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    pub public_links: bool,

    pub concurrency: u32,

    pub max_storage: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fonts_dir: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_font: Option<String>,
}

impl Clips {
    pub fn dir(&self, config_dir: &Path, data_dir: &Path) -> anyhow::Result<PathBuf> {
        match self.path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            Some(p) => resolve_config_path(p, config_dir),
            None => Ok(data_dir.join("clips")),
        }
    }

    pub fn fonts_dir_path(&self, config_dir: &Path) -> Option<PathBuf> {
        let p = self.fonts_dir.as_deref().map(str::trim).filter(|p| !p.is_empty())?;
        resolve_config_path(p, config_dir).ok()
    }

    pub fn default_font_path(&self, config_dir: &Path) -> Option<PathBuf> {
        let p = self.default_font.as_deref().map(str::trim).filter(|p| !p.is_empty())?;
        resolve_config_path(p, config_dir).ok()
    }
}

impl Default for Clips {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
            public_links: true,
            concurrency: 2,
            max_storage: 0,
            fonts_dir: None,
            default_font: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "MusicConfig", input_name = "MusicConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Music {
    pub online_lyrics: bool,

    pub lyrics_url: String,

    pub analyze_loudness: bool,
}

impl Default for Music {
    fn default() -> Self {
        Self { online_lyrics: true, lyrics_url: "https://lrclib.net".into(), analyze_loudness: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Anilist,
    Tmdb,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Anilist => "anilist",
            Provider::Tmdb => "tmdb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Library {
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "LibraryKind::is_video")]
    pub kind: LibraryKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_provider: Option<Provider>,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub managed: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum LibraryKind {
    #[default]
    Video,
    Music,
    Books,
}

impl LibraryKind {
    pub fn is_video(&self) -> bool {
        *self == LibraryKind::Video
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LibraryKind::Video => "video",
            LibraryKind::Music => "music",
            LibraryKind::Books => "books",
        }
    }
}

impl Library {
    pub fn is_music(&self) -> bool {
        self.kind == LibraryKind::Music
    }

    pub fn is_books(&self) -> bool {
        self.kind == LibraryKind::Books
    }

    pub fn resolved_path(&self, config_dir: &Path) -> anyhow::Result<PathBuf> {
        resolve_config_path(&self.path, config_dir)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum ImportMode {
    #[default]
    Auto,
    Hardlink,
    Copy,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum SeedAction {
    Pause,

    #[default]
    Remove,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "Seeding", input_name = "SeedingInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Seeding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<Span>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle: Option<Span>,
    #[serde(default)]
    pub then: SeedAction,
}

impl Default for Seeding {
    fn default() -> Self {
        Self { ratio: Some(1.0), time: None, idle: None, then: SeedAction::Remove }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "DownloadsConfig", input_name = "DownloadsConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Downloads {
    pub path: Option<String>,
    pub import: ImportMode,

    pub port: u16,
    pub upnp: bool,
    pub dht: bool,

    pub max_active: i32,

    pub download_limit: u32,
    pub upload_limit: u32,

    pub slow_download_limit: u32,
    pub slow_upload_limit: u32,
    pub slow_from: Option<String>,
    pub slow_to: Option<String>,

    pub bind_interface: Option<String>,

    pub proxy: Option<String>,
    pub seeding: Seeding,
}

impl Default for Downloads {
    fn default() -> Self {
        Self {
            path: None,
            import: ImportMode::Auto,
            port: 6881,
            upnp: true,
            dht: true,
            max_active: 4,
            download_limit: 0,
            upload_limit: 0,
            slow_download_limit: 0,
            slow_upload_limit: 0,
            slow_from: None,
            slow_to: None,
            bind_interface: None,
            proxy: None,
            seeding: Seeding::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum Monitor {
    #[default]
    None,

    Future,

    Missing,
}

impl Monitor {
    pub fn as_str(self) -> &'static str {
        match self {
            Monitor::None => "none",
            Monitor::Future => "future",
            Monitor::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "RetryStep", input_name = "RetryStepInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RetryStep {
    pub every: Span,
    pub until: Span,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "AutomationConfig", input_name = "AutomationConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Automation {
    pub default_monitor: Monitor,

    pub rss_interval: Span,

    pub retry: Vec<RetryStep>,

    pub rename_suggestions: bool,
}

impl Default for Automation {
    fn default() -> Self {
        let step = |every: u64, until: u64| RetryStep {
            every: Span(Duration::from_secs(every)),
            until: Span(Duration::from_secs(until)),
        };

        Self {
            default_monitor: Monitor::None,
            rss_interval: Span(Duration::from_secs(15 * 60)),
            retry: vec![step(120, 1800), step(300, 7200), step(3600, 86400), step(86400, 7 * 86400)],
            rename_suggestions: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RequestMode {
    #[default]
    Off,

    Approval,

    Auto,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "RequestsConfig", input_name = "RequestsConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Requests {
    pub monitor: Monitor,

    #[serde(skip_serializing)]
    #[graphql(skip)]
    pub mode: Option<RequestMode>,
    #[serde(skip_serializing)]
    #[graphql(skip)]
    pub max_open: Option<u32>,
}

impl Default for Requests {
    fn default() -> Self {
        Self { monitor: Monitor::Missing, mode: None, max_open: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum SignInStyle {
    #[default]
    Profiles,

    Username,
}

#[derive(
    Debug, Clone, PartialEq, Default, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject,
)]
#[graphql(name = "SignInConfig", input_name = "SignInConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct SignIn {
    pub style: SignInStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum SmtpSecurity {
    #[default]
    Starttls,
    Tls,
    None,
}

#[derive(
    Debug, Clone, PartialEq, Default, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject,
)]
#[graphql(name = "SmtpConfig", input_name = "SmtpConfigInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Smtp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub security: SmtpSecurity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
}

impl Smtp {
    pub fn relay_host(&self) -> Option<&str> {
        self.host.as_deref().map(str::trim).filter(|h| !h.is_empty())
    }

    #[cfg_attr(not(feature = "smtp"), allow(dead_code))]
    pub fn relay_port(&self) -> u16 {
        self.port.unwrap_or(match self.security {
            SmtpSecurity::Starttls => 587,
            SmtpSecurity::Tls => 465,
            SmtpSecurity::None => 25,
        })
    }

    pub fn link(&self, path: &str) -> Option<String> {
        let base = self.public_url.as_deref()?.trim().trim_end_matches('/');
        Some(format!("{base}/{}", path.trim_start_matches('/')))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, async_graphql::Enum)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    Torznab,

    Rss,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "Source", input_name = "SourceInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Source {
    pub name: String,
    pub kind: SourceKind,

    pub url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<u32>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    #[graphql(default = true)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_path: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeding: Option<Seeding>,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(name = "Profile", input_name = "ProfileInput")]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Profile {
    pub name: String,

    pub resolutions: Vec<String>,

    pub groups: Vec<String>,

    pub require: Vec<String>,

    pub reject: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<u64>,

    pub codecs: Vec<String>,
    pub prefer_dual_audio: bool,

    pub batches: bool,
    pub min_seeders: u32,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Any".into(),
            resolutions: vec!["1080p".into(), "720p".into(), "2160p".into(), "480p".into()],
            groups: Vec::new(),
            require: Vec::new(),
            reject: Vec::new(),
            min_size: None,
            max_size: None,
            codecs: Vec::new(),
            prefer_dual_audio: false,
            batches: true,
            min_seeders: 1,
        }
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();

        for lib in &self.libraries {
            let name = lib.name.trim();

            if name.is_empty() {
                return Err("every [[library]] needs a non-empty `name`".into());
            }

            if !seen.insert(name.to_lowercase()) {
                return Err(format!("two libraries are called {name:?}; names must be unique"));
            }

            if lib.path.trim().is_empty() {
                return Err(format!("library {name:?} needs a `path`"));
            }

            if !lib.kind.is_video() && (lib.managed || lib.metadata_provider.is_some() || lib.profile.is_some()) {
                return Err(format!(
                    "library {name:?} is a {} library; `managed`, `metadata-provider` and `profile` are for video",
                    lib.kind.as_str()
                ));
            }
        }

        let mut seen = std::collections::HashSet::new();

        for source in &self.sources {
            let name = source.name.trim();

            if name.is_empty() {
                return Err("every [[source]] needs a non-empty `name`".into());
            }

            if !seen.insert(name.to_lowercase()) {
                return Err(format!("two sources are called {name:?}; names must be unique"));
            }

            if url::Url::parse(source.url.replace("{query}", "x").trim()).is_err() {
                return Err(format!("source {name:?}: {:?} isn't a URL", source.url));
            }

            if source.kind == SourceKind::Rss && !source.url.contains("{query}") && source.feed.is_none() {
                return Err(format!(
                    "source {name:?}: an RSS source's `url` needs `{{query}}` where the search goes (or set only `feed`)"
                ));
            }
        }

        let mut seen = std::collections::HashSet::new();

        for profile in &self.profiles {
            let name = profile.name.trim();

            if name.is_empty() {
                return Err("every [[profile]] needs a non-empty `name`".into());
            }

            if !seen.insert(name.to_lowercase()) {
                return Err(format!("two profiles are called {name:?}; names must be unique"));
            }

            for pattern in profile.require.iter().chain(&profile.reject) {
                if let Err(e) = regex::RegexBuilder::new(pattern).case_insensitive(true).build() {
                    return Err(format!("profile {name:?}: {pattern:?} isn't a valid pattern: {e}"));
                }
            }
        }

        for lib in &self.libraries {
            if let Some(p) = &lib.profile
                && self.profile(p).is_none()
            {
                return Err(format!(
                    "library {:?} uses profile {p:?}, but there's no [[profile]] called that",
                    lib.name
                ));
            }
        }

        for t in [&self.downloads.slow_from, &self.downloads.slow_to].into_iter().flatten() {
            if parse_clock(t).is_none() {
                return Err(format!("{t:?} isn't a time of day; write it like \"08:00\""));
            }
        }

        if let Some(proxy) = &self.downloads.proxy
            && !proxy.trim().is_empty()
        {
            let url =
                url::Url::parse(proxy).map_err(|_| format!("proxy {proxy:?} isn't a URL like socks5://host:1080"))?;

            if !matches!(url.scheme(), "socks5" | "http") || url.host_str().is_none() {
                return Err(format!("proxy {proxy:?} must look like socks5://host:1080 or http://host:8080"));
            }
        }

        if self.music.online_lyrics && url::Url::parse(self.music.lyrics_url.trim()).is_err() {
            return Err(format!("`[music] lyrics-url = {:?}` isn't a URL", self.music.lyrics_url));
        }

        if self.clips.concurrency == 0 {
            return Err("`[clips] concurrency` has to be at least 1".into());
        }

        if self.automation.retry.iter().any(|s| s.every.is_zero()) {
            return Err("every [automation] retry step needs a non-zero `every`".into());
        }

        if self.smtp.relay_host().is_some() {
            if !self.smtp.from.as_deref().is_some_and(|f| f.contains('@')) {
                return Err("[smtp] needs a `from` address, like \"tinystream <tinystream@example.com>\"".into());
            }

            let url = self.smtp.public_url.as_deref().map(str::trim).unwrap_or_default();

            if !url::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https")) {
                return Err(format!(
                    "`[smtp] public-url = {url:?}` isn't the address people open tinystream at, like https://tv.example.com"
                ));
            }
        }

        if self.log.level.parse::<tracing::Level>().is_err() {
            return Err(format!(
                "`[log] level = {:?}` isn't a level; use one of trace, debug, info, warn, error",
                self.log.level
            ));
        }

        Ok(())
    }

    pub fn library(&self, name: &str) -> Option<&Library> {
        self.libraries.iter().find(|l| l.name == name)
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn source(&self, name: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }
}

pub fn parse_clock(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub const TEMPLATE: &str = r#"# tinystream
#
# Edits to this file are picked up instantly; there's no need to restart.
# Everything is optional except your libraries. `~` means $HOME, and relative
# paths are relative to this file.

[network]
host = "0.0.0.0"
port = 3000

# Add one [[library]] per folder. tinystream figures out on its own whether a
# folder holds a show (it has season folders) or a movie (a video right inside).
#
#   Show Name/Season 01/Anything S01E01.mkv
#   Movie Name (2014)/Movie Name.mkv
#
# metadata-provider is "anilist" or "tmdb" (tmdb needs [metadata] tmdb-api-key).
#
# [[library]]
# name = "Anime"
# path = "~/Videos/Anime"
# metadata-provider = "anilist"
# managed = true   # let tinystream put downloads here and rename files you approve
#
# Music libraries are read from the files' tags, however the folders are laid out:
#
# [[library]]
# name = "Music"
# path = "~/Music"
# kind = "music"
#
# Books (EPUB, FB2 and plain text) are read from the files themselves, with
# what's missing looked up on Google Books and Open Library:
#
# [[library]]
# name = "Novels"
# path = "~/Books"
# kind = "books"

# Downloads (Settings → Downloads and Sources in the UI). tinystream ships
# with no sources; add the ones you use:
#
# [[source]]
# name = "My tracker"
# kind = "torznab"   # or "rss", with {query} in the url where the search goes
# url = "https://tracker.example/api"
# api-key = "…"
#
# [downloads]
# path = "~/Downloads/tinystream"
# bind-interface = "wg0"   # pause every torrent whenever your VPN is down

# Email, for sending invite links and password resets (only in builds with the
# `smtp` feature). tinystream sends through your mail provider's SMTP server:
#
# [smtp]
# host = "smtp.example.com"
# security = "starttls"   # or "tls" (port 465), or "none"
# username = "me@example.com"
# password = "…"
# from = "tinystream <me@example.com>"
# public-url = "https://tv.example.com"   # where links in emails point
"#;
