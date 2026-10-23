// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

use super::{
    Automation, Clips, Config, Downloads, LibraryKind, Log, Metadata, Music, Network, Provider, Requests, Scan, SignIn,
    Smtp, TEMPLATE, Tailscale, Transcode,
};
use crate::events::{Event, Events};

pub struct ConfigStore {
    path: PathBuf,
    tx: watch::Sender<Arc<Config>>,

    applied_hash: Mutex<[u8; 32]>,
    last_error: Mutex<Option<String>>,
    write_lock: tokio::sync::Mutex<()>,
    events: Events,
}

#[derive(Debug, Deserialize, async_graphql::InputObject)]
#[graphql(name = "ConfigPatch")]
pub struct SettingsPatch {
    pub network: Option<Network>,
    pub log: Option<Log>,
    pub scan: Option<Scan>,
    pub metadata: Option<Metadata>,
    pub transcode: Option<Transcode>,
    pub clips: Option<Clips>,
    pub music: Option<Music>,
    pub downloads: Option<Downloads>,
    pub automation: Option<Automation>,
    pub requests: Option<Requests>,
    pub sign_in: Option<SignIn>,
    pub smtp: Option<Smtp>,
    pub tailscale: Option<Tailscale>,
}

#[derive(Debug, Clone, Deserialize, Serialize, async_graphql::InputObject)]
#[serde(rename_all = "kebab-case")]
pub struct LibraryInput {
    pub name: String,
    pub path: String,
    #[serde(default)]
    #[graphql(default)]
    pub kind: LibraryKind,
    #[serde(default)]
    pub metadata_provider: Option<Provider>,
    #[serde(default)]
    #[graphql(default)]
    pub managed: bool,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub download_path: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum List {
    Sources,
    Profiles,
}

impl List {
    fn key(self) -> &'static str {
        match self {
            List::Sources => "source",
            List::Profiles => "profile",
        }
    }
}

fn hash(text: &str) -> [u8; 32] {
    Sha256::digest(text.as_bytes()).into()
}

impl ConfigStore {
    pub fn load(path: &Path, events: Events) -> anyhow::Result<Arc<Self>> {
        if !path.exists() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).with_context(|| format!("can't create {}", dir.display()))?;
            }

            std::fs::write(path, TEMPLATE).with_context(|| format!("can't write {}", path.display()))?;
            tracing::info!("created a fresh config at {}", path.display());
        }

        let text = std::fs::read_to_string(path).with_context(|| format!("can't read {}", path.display()))?;
        let config = Config::parse(&text).map_err(|e| anyhow::anyhow!("{} is invalid:\n{e}", path.display()))?;
        let (tx, _) = watch::channel(Arc::new(config));

        Ok(Arc::new(Self {
            path: path.to_path_buf(),
            tx,
            applied_hash: Mutex::new(hash(&text)),
            last_error: Mutex::new(None),
            write_lock: tokio::sync::Mutex::new(()),
            events,
        }))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config_dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("/"))
    }

    pub fn current(&self) -> Arc<Config> {
        self.tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<Config>> {
        self.tx.subscribe()
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    pub fn raw(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.path)
    }

    async fn edit(&self, f: impl FnOnce(&mut DocumentMut) -> Result<(), String>) -> Result<Arc<Config>, String> {
        let _guard = self.write_lock.lock().await;
        let text = self.raw().map_err(|e| format!("can't read config.toml: {e}"))?;

        let mut doc: DocumentMut =
            text.parse().map_err(|e| format!("config.toml has a syntax error, fix it first:\n{e}"))?;

        move_public_url(&mut doc)?;
        f(&mut doc)?;
        self.write(doc.to_string())
    }

    fn write(&self, text: String) -> Result<Arc<Config>, String> {
        let config = Arc::new(Config::parse(&text)?);

        *self.applied_hash.lock().unwrap() = hash(&text);
        let tmp = self.path.with_extension("toml.tmp");

        std::fs::write(&tmp, &text)
            .and_then(|_| std::fs::rename(&tmp, &self.path))
            .map_err(|e| format!("can't write config.toml: {e}"))?;

        *self.last_error.lock().unwrap() = None;
        self.tx.send_replace(config.clone());
        self.events.send(Event::ConfigChanged);
        tracing::info!("saved {}", self.path.display());
        Ok(config)
    }

    pub async fn replace_raw(&self, text: String) -> Result<Arc<Config>, String> {
        let _guard = self.write_lock.lock().await;
        text.parse::<DocumentMut>().map_err(|e| e.to_string())?;
        self.write(text)
    }

    pub async fn patch(&self, patch: SettingsPatch) -> Result<Arc<Config>, String> {
        self.edit(|doc| {
            if let Some(v) = patch.network {
                sync_section(doc, "network", &v, &Network::default())?;
            }
            if let Some(v) = patch.log {
                sync_section(doc, "log", &v, &Log::default())?;
            }
            if let Some(v) = patch.scan {
                sync_section(doc, "scan", &v, &Scan::default())?;
            }
            if let Some(v) = patch.metadata {
                sync_section(doc, "metadata", &v, &Metadata::default())?;
            }
            if let Some(v) = patch.transcode {
                sync_section(doc, "transcode", &v, &Transcode::default())?;
            }
            if let Some(v) = patch.clips {
                sync_section(doc, "clips", &v, &Clips::default())?;
            }
            if let Some(v) = patch.music {
                sync_section(doc, "music", &v, &Music::default())?;
            }
            if let Some(v) = patch.downloads {
                sync_section(doc, "downloads", &v, &Downloads::default())?;
            }
            if let Some(v) = patch.automation {
                sync_section(doc, "automation", &v, &Automation::default())?;
            }
            if let Some(v) = patch.requests {
                sync_section(doc, "requests", &v, &Requests::default())?;
            }
            if let Some(v) = patch.sign_in {
                sync_section(doc, "sign-in", &v, &SignIn::default())?;
            }
            if let Some(v) = patch.smtp {
                sync_section(doc, "smtp", &v, &Smtp::default())?;
            }
            if let Some(v) = patch.tailscale {
                sync_section(doc, "tailscale", &v, &Tailscale::default())?;
            }
            Ok(())
        })
        .await
    }

    pub async fn add_library(&self, input: LibraryInput) -> Result<Arc<Config>, String> {
        self.edit(|doc| {
            let libs = libraries_mut(doc)?;
            let mut table = Table::new();
            write_library(&mut table, &input)?;
            libs.push(table);
            Ok(())
        })
        .await
    }

    pub async fn update_library(&self, index: usize, input: LibraryInput) -> Result<Arc<Config>, String> {
        self.edit(|doc| {
            let table = libraries_mut(doc)?.get_mut(index).ok_or_else(|| format!("there is no library #{index}"))?;
            write_library(table, &input)
        })
        .await
    }

    pub async fn remove_library(&self, index: usize) -> Result<Arc<Config>, String> {
        self.edit(|doc| {
            let libs = libraries_mut(doc)?;

            if index >= libs.len() {
                return Err(format!("there is no library #{index}"));
            }

            libs.remove(index);
            Ok(())
        })
        .await
    }

    pub async fn put_entry<T: Serialize>(
        &self,
        list: List,
        index: Option<usize>,
        value: &T,
    ) -> Result<Arc<Config>, String> {
        let new = to_table(value)?;

        self.edit(|doc| {
            let tables = array_mut(doc, list.key())?;

            match index {
                None => {
                    let mut table = Table::new();
                    sync_table(&mut table, &new, &toml::Table::new(), true)?;
                    tables.push(table);
                },
                Some(i) => {
                    let table = tables.get_mut(i).ok_or_else(|| format!("there is no {} #{i}", list.key()))?;
                    sync_table(table, &new, &toml::Table::new(), true)?;
                },
            }

            Ok(())
        })
        .await
    }

    pub async fn remove_entry(&self, list: List, index: usize) -> Result<Arc<Config>, String> {
        self.edit(|doc| {
            let tables = array_mut(doc, list.key())?;

            if index >= tables.len() {
                return Err(format!("there is no {} #{index}", list.key()));
            }

            tables.remove(index);
            Ok(())
        })
        .await
    }

    pub fn watch(self: &Arc<Self>) -> anyhow::Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let file_name = self.path.file_name().map(|n| n.to_owned());

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res
                && event.paths.iter().any(|p| p.file_name() == file_name.as_deref())
            {
                let _ = tx.send(());
            }
        })?;

        watcher.watch(self.config_dir(), RecursiveMode::NonRecursive)?;

        let this = self.clone();

        tokio::spawn(async move {
            let _watcher = watcher;

            while rx.recv().await.is_some() {
                tokio::time::sleep(Duration::from_millis(150)).await;
                while rx.try_recv().is_ok() {}
                this.reload_from_disk();
            }
        });

        Ok(())
    }

    fn reload_from_disk(&self) {
        let text = match self.raw() {
            Ok(t) => t,
            Err(_) if !self.path.exists() => {
                tracing::warn!("config.toml was removed; keeping the settings already loaded");
                return;
            },
            Err(e) => return tracing::warn!("can't read config.toml: {e}"),
        };

        let h = hash(&text);

        {
            let mut applied = self.applied_hash.lock().unwrap();

            if *applied == h {
                return;
            }

            *applied = h;
        }

        match Config::parse(&text) {
            Ok(config) => {
                *self.last_error.lock().unwrap() = None;

                if *self.current() != config {
                    tracing::info!("config.toml changed on disk; applied");
                    self.tx.send_replace(Arc::new(config));
                }

                self.events.send(Event::ConfigChanged);
            },
            Err(e) => {
                tracing::error!("config.toml changed but is invalid, keeping the previous settings:\n{e}");
                *self.last_error.lock().unwrap() = Some(e.clone());
                self.events.send(Event::ConfigError { message: e });
            },
        }
    }
}

/// `public-url` used to sit under [smtp], when only emails had links in them.
fn move_public_url(doc: &mut DocumentMut) -> Result<(), String> {
    let Some(url) = doc.get_mut("smtp").and_then(Item::as_table_like_mut).and_then(|t| t.remove("public-url")) else {
        return Ok(());
    };

    if !doc.contains_key("network") {
        insert_before_libraries(doc, "network");
    }

    let network = doc["network"].as_table_mut().ok_or("`network` in config.toml must be a [network] table")?;

    if !network.contains_key("public-url") {
        network.insert("public-url", url);
    }

    Ok(())
}

fn libraries_mut(doc: &mut DocumentMut) -> Result<&mut ArrayOfTables, String> {
    array_mut(doc, "library")
}

fn array_mut<'a>(doc: &'a mut DocumentMut, key: &str) -> Result<&'a mut ArrayOfTables, String> {
    if !doc.contains_key(key) {
        doc.insert(key, Item::ArrayOfTables(ArrayOfTables::new()));
    }
    doc[key]
        .as_array_of_tables_mut()
        .ok_or_else(|| format!("`{key}` in config.toml must be written as [[{key}]] tables"))
}

fn write_library(table: &mut Table, input: &LibraryInput) -> Result<(), String> {
    let text = |s: &Option<String>| {
        s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| toml::Value::String(s.to_string()))
    };

    set_value(table, "name", Some(toml::Value::String(input.name.trim().to_string())))?;
    set_value(table, "path", Some(toml::Value::String(input.path.trim().to_string())))?;
    set_value(table, "kind", (!input.kind.is_video()).then(|| toml::Value::String(input.kind.as_str().into())))?;
    set_value(table, "metadata-provider", input.metadata_provider.map(|p| toml::Value::String(p.as_str().into())))?;
    set_value(table, "managed", input.managed.then_some(toml::Value::Boolean(true)))?;
    set_value(table, "profile", text(&input.profile))?;
    set_value(table, "download-path", text(&input.download_path))
}

fn sync_section<T: Serialize + serde::de::DeserializeOwned>(
    doc: &mut DocumentMut,
    name: &str,
    new: &T,
    default: &T,
) -> Result<(), String> {
    let new = to_table(new)?;
    let default = to_table(default)?;

    if !doc.contains_key(name) {
        let differs =
            new.iter().any(|(k, v)| default.get(k) != Some(v)) || default.keys().any(|k| !new.contains_key(k));

        if !differs {
            return Ok(());
        }

        insert_before_libraries(doc, name);
    }

    let table = doc[name].as_table_mut().ok_or_else(|| format!("`{name}` in config.toml must be a [{name}] table"))?;

    // What's written now and still would be, but isn't in the new settings, was cleared (an emptied
    // Option). Keys that are read but never written back, kept for old configs, don't come through.
    let written = toml::from_str::<T>(&table.to_string()).ok().map(|t| to_table(&t)).transpose()?.unwrap_or_default();
    let cleared: Vec<String> =
        written.keys().filter(|k| !new.contains_key(*k) && !default.contains_key(*k)).cloned().collect();

    sync_table(table, &new, &default, false)?;

    for key in cleared {
        table.remove(&key);
    }

    Ok(())
}

fn sync_table(table: &mut Table, new: &toml::Table, default: &toml::Table, exact: bool) -> Result<(), String> {
    let mut keys: Vec<String> = new.keys().chain(default.keys().filter(|k| !new.contains_key(*k))).cloned().collect();

    if exact {
        keys.extend(
            table.iter().map(|(k, _)| k.to_string()).filter(|k| !new.contains_key(k) && !default.contains_key(k)),
        );
    }

    for key in &keys {
        let value = new.get(key).cloned();

        if !table.contains_key(key) && value.as_ref() == default.get(key) {
            continue;
        }

        if let (Some(toml::Value::Table(sub)), Some(Item::Table(existing))) = (&value, table.get_mut(key)) {
            let sub_default = match default.get(key) {
                Some(toml::Value::Table(t)) => t.clone(),
                _ => toml::Table::new(),
            };

            sync_table(existing, sub, &sub_default, true)?;
            continue;
        }

        let current = table.get(key).and_then(|i| i.as_value()).map(|v| v.to_string());
        let wanted = value.as_ref().map(|v| edit_value(v).to_string());

        if current.as_deref().map(str::trim) == wanted.as_deref().map(str::trim) {
            continue;
        }

        set_value(table, key, value)?;
    }

    Ok(())
}

fn insert_before_libraries(doc: &mut DocumentMut, name: &str) {
    let mut table = Table::new();

    if let Some(libs) = doc.get_mut("library").and_then(Item::as_array_of_tables_mut) {
        let first = libs.iter().filter_map(Table::position).min();

        if let Some(pos) = first {
            for t in libs.iter_mut() {
                if let Some(p) = t.position() {
                    t.set_position(Some(p + 1));
                }
            }

            table.set_position(Some(pos));
        }
    }

    doc.insert(name, Item::Table(table));
}

fn to_table<T: Serialize>(v: &T) -> Result<toml::Table, String> {
    toml::Table::try_from(v).map_err(|e| e.to_string())
}

fn set_value(table: &mut Table, key: &str, value: Option<toml::Value>) -> Result<(), String> {
    let Some(value) = value else {
        table.remove(key);
        return Ok(());
    };

    let mut new = edit_value(&value);

    if let Some(old) = table.get(key).and_then(|i| i.as_value()) {
        *new.decor_mut() = old.decor().clone();
    }

    table.insert(key, Item::Value(new));
    Ok(())
}

fn edit_value(value: &toml::Value) -> toml_edit::Value {
    match value {
        toml::Value::Table(t) => {
            let mut inline = toml_edit::InlineTable::new();

            for (k, v) in t {
                inline.insert(k, edit_value(v));
            }

            toml_edit::Value::InlineTable(inline)
        },
        toml::Value::Array(a) => {
            let mut array = toml_edit::Array::new();

            for v in a {
                array.push(edit_value(v));
            }

            toml_edit::Value::Array(array)
        },
        toml::Value::String(s) => s.as_str().into(),
        toml::Value::Integer(i) => (*i).into(),
        toml::Value::Float(f) => (*f).into(),
        toml::Value::Boolean(b) => (*b).into(),
        toml::Value::Datetime(d) => d.to_string().parse().unwrap_or_else(|_| d.to_string().into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_keeps_comments_and_untouched_keys() {
        let mut doc: DocumentMut =
            "# hello\n[network]\nhost = \"0.0.0.0\" # everywhere\nport = 3000\n".parse().unwrap();

        let net = Network { port: 4000, ..Network::default() };
        sync_section(&mut doc, "network", &net, &Network::default()).unwrap();
        let out = doc.to_string();
        assert!(out.contains("# hello"));
        assert!(out.contains("host = \"0.0.0.0\" # everywhere"));
        assert!(out.contains("port = 4000"));
        assert!(!out.contains("cors"));
    }

    #[test]
    fn new_sections_go_above_libraries() {
        let mut doc: DocumentMut = "[network]\nport = 1\n\n[[library]]\nname = \"A\"\npath = \"/a\"\n".parse().unwrap();

        let scan =
            Scan { interval: Some(crate::config::Span(std::time::Duration::from_secs(3600))), ..Scan::default() };

        sync_section(&mut doc, "scan", &scan, &Scan::default()).unwrap();
        let out = doc.to_string();
        assert!(out.find("[scan]").unwrap() < out.find("[[library]]").unwrap(), "{out}");
    }

    #[test]
    fn cleared_settings_leave_the_file() {
        let mut doc: DocumentMut = "[metadata]\ntmdb-api-key = \"abc\"\nlanguage = \"de-DE\"\n".parse().unwrap();
        let metadata = Metadata { language: "de-DE".into(), ..Metadata::default() };
        sync_section(&mut doc, "metadata", &metadata, &Metadata::default()).unwrap();
        let out = doc.to_string();
        assert!(!out.contains("tmdb-api-key") && out.contains("language = \"de-DE\""), "{out}");
    }

    #[test]
    fn unwritten_keys_stay() {
        let mut doc: DocumentMut = "[requests]\nmax-open = 3\n".parse().unwrap();
        sync_section(&mut doc, "requests", &Requests::default(), &Requests::default()).unwrap();
        assert!(doc.to_string().contains("max-open = 3"), "{doc}");
    }

    #[test]
    fn untouched_defaults_add_nothing() {
        let mut doc: DocumentMut = "".parse().unwrap();
        sync_section(&mut doc, "log", &Log::default(), &Log::default()).unwrap();
        assert_eq!(doc.to_string(), "");
    }

    #[test]
    fn public_url_moves_out_of_smtp() {
        let text = "[network]\nport = 1\n\n[smtp]\nhost = \"mail\"\nfrom = \"a@b.c\"\npublic-url = \"https://tv.example.com\"\n";
        assert_eq!(Config::parse(text).unwrap().network.public_base(), Some("https://tv.example.com"));

        let mut doc: DocumentMut = text.parse().unwrap();
        move_public_url(&mut doc).unwrap();
        let out = doc.to_string();
        let config = Config::parse(&out).unwrap();
        assert_eq!(config.network.public_base(), Some("https://tv.example.com"), "{out}");
        assert_eq!(out.matches("public-url").count(), 1, "{out}");
        assert!(out.find("public-url").unwrap() < out.find("[smtp]").unwrap(), "{out}");
    }

    #[test]
    fn trusted_proxies_must_be_addresses() {
        assert!(Config::parse("[network]\ntrusted-proxies = [\"10.0.0.0/8\", \"::1\"]\n").is_ok());
        assert!(Config::parse("[network]\ntrusted-proxies = [\"proxy.lan\"]\n").is_err());
        assert!(Config::parse("[network]\ntrusted-proxies = [\"10.0.0.0/33\"]\n").is_err());
    }

    #[test]
    fn exposing_rules() {
        let public = Config::parse("[tailscale]\nexpose = \"public\"\n").unwrap();
        assert_eq!(public.sign_in_style(false), super::super::SignInStyle::Username);
        let shown = Config::parse("[tailscale]\nexpose = \"public\"\n\n[sign-in]\npublic-profiles = true\n").unwrap();
        assert_eq!(shown.sign_in_style(false), super::super::SignInStyle::Profiles);
        assert_eq!(Config::default().sign_in_style(true), super::super::SignInStyle::Username);
        assert!(Config::parse("[tailscale]\nexpose = \"public\"\nhttps-port = 444\n").is_err());
        assert!(Config::parse("[tailscale]\nexpose = \"tailnet\"\nhttps-port = 444\n").is_ok());
        assert!(Config::parse("[tailscale]\nexpose = \"tailnet\"\ndetect = false\n").is_err());
    }

    #[test]
    fn library_roundtrip() {
        let mut doc: DocumentMut = TEMPLATE.parse().unwrap();
        let libs = libraries_mut(&mut doc).unwrap();
        let mut t = Table::new();

        write_library(
            &mut t,
            &LibraryInput {
                name: "Anime".into(),
                path: "~/Anime".into(),
                kind: LibraryKind::Video,
                metadata_provider: Some(Provider::Anilist),
                managed: true,
                profile: None,
                download_path: None,
            },
        )
        .unwrap();

        libs.push(t);
        let config = Config::parse(&doc.to_string()).unwrap();
        assert_eq!(config.libraries.len(), 1);
        assert_eq!(config.libraries[0].metadata_provider, Some(Provider::Anilist));
        assert!(config.libraries[0].managed);
    }

    #[test]
    fn nested_settings_are_inline() {
        let mut doc: DocumentMut = "[downloads]\nport = 1\n".parse().unwrap();
        let mut d = Downloads { port: 1, ..Downloads::default() };
        d.seeding.ratio = Some(2.0);
        sync_section(&mut doc, "downloads", &d, &Downloads::default()).unwrap();
        let out = doc.to_string();
        assert!(out.contains("seeding = { ratio = 2.0"), "{out}");
        let config = Config::parse(&out).unwrap();
        assert_eq!(config.downloads.seeding.ratio, Some(2.0));

        let forever: Config = Config::parse("[downloads]\nseeding = { then = \"pause\" }\n").unwrap();
        assert_eq!(forever.downloads.seeding.ratio, None);
    }

    #[test]
    fn entries_roundtrip() {
        let mut doc: DocumentMut = "".parse().unwrap();

        let source = super::super::Source {
            name: "Nyaa".into(),
            kind: super::super::SourceKind::Rss,
            url: "https://example.org/?q={query}".into(),
            feed: None,
            api_key: None,
            categories: vec![],
            enabled: true,
            download_path: None,
            seeding: None,
        };

        let mut t = Table::new();
        sync_table(&mut t, &to_table(&source).unwrap(), &toml::Table::new(), true).unwrap();
        array_mut(&mut doc, "source").unwrap().push(t);
        let config = Config::parse(&doc.to_string()).unwrap();
        assert_eq!(config.sources, vec![source]);
    }
}
