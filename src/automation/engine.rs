// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::Context;
use libtorrent_sys as lt;
use serde::Serialize;

use crate::config::{Config, SeedAction, Seeding, parse_clock};
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

pub struct Engine {
    session: lt::UniquePtr<lt::Session>,
    applied: Mutex<lt::Settings>,
    statuses: RwLock<HashMap<String, lt::Status>>,

    killed: AtomicBool,
    listen: Mutex<Option<String>>,
    listen_error: Mutex<Option<String>>,

    turn: tokio::sync::Mutex<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub version: String,
    pub download_rate: i64,
    pub upload_rate: i64,
    pub active: usize,

    pub kill_switch: Option<String>,
    pub listening: Option<String>,
    pub listen_error: Option<String>,
    pub slow_hours: bool,
    pub download_path: PathBuf,
}

fn in_slow_hours(config: &Config) -> bool {
    let d = &config.downloads;

    let (Some(from), Some(to)) =
        (d.slow_from.as_deref().and_then(parse_clock), d.slow_to.as_deref().and_then(parse_clock))
    else {
        return false;
    };

    let now = time::OffsetDateTime::now_utc().to_offset(super::local_offset());
    let minute = now.hour() as u32 * 60 + now.minute() as u32;
    if from <= to { (from..to).contains(&minute) } else { minute >= from || minute < to }
}

fn settings(config: &Config) -> lt::Settings {
    let d = &config.downloads;
    let slow = in_slow_hours(config);
    let kib = |v: u32| v as i64 * 1024;
    let iface = d.bind_interface.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let listen = match iface {
        Some(i) => format!("{i}:{}", d.port),
        None => format!("0.0.0.0:{p},[::]:{p}", p = d.port),
    };

    let (mut proxy_type, mut proxy_host, mut proxy_port, mut proxy_user, mut proxy_pass) =
        (0, String::new(), 0, String::new(), String::new());

    if let Some(p) = d.proxy.as_deref().filter(|p| !p.trim().is_empty())
        && let Ok(u) = url::Url::parse(p)
    {
        proxy_type = if u.scheme() == "http" { 2 } else { 1 };
        proxy_host = u.host_str().unwrap_or_default().to_string();
        proxy_port = u.port().unwrap_or(if proxy_type == 2 { 8080 } else { 1080 });
        proxy_user = u.username().to_string();
        proxy_pass = u.password().unwrap_or_default().to_string();
    }

    lt::Settings {
        listen_interfaces: listen,
        outgoing_interface: iface.unwrap_or_default().to_string(),
        proxy_type,
        proxy_host,
        proxy_port,
        proxy_user,
        proxy_pass,
        download_rate_limit: kib(if slow && d.slow_download_limit > 0 {
            d.slow_download_limit
        } else {
            d.download_limit
        }),
        upload_rate_limit: kib(if slow && d.slow_upload_limit > 0 { d.slow_upload_limit } else { d.upload_limit }),
        enable_upnp: d.upnp && iface.is_none(),
        enable_natpmp: d.upnp && iface.is_none(),
        enable_dht: d.dht,
        enable_lsd: iface.is_none(),
        active_downloads: if d.max_active > 0 { d.max_active } else { -1 },
        active_seeds: -1,
        active_limit: -1,
        user_agent: format!("tinystream/{}", env!("CARGO_PKG_VERSION")),
    }
}

#[cfg(target_vendor = "apple")]
fn interface_up(name: &str) -> bool {
    let mut list = std::ptr::null_mut();

    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return false;
    }

    let mut up = false;
    let mut cur = list;

    while !cur.is_null() {
        let i = unsafe { &*cur };

        if unsafe { std::ffi::CStr::from_ptr(i.ifa_name) }.to_bytes() == name.as_bytes()
            && i.ifa_flags & libc::IFF_UP as u32 != 0
        {
            up = true;
            break;
        }

        cur = i.ifa_next;
    }

    unsafe { libc::freeifaddrs(list) };
    up
}

#[cfg(not(target_vendor = "apple"))]
fn interface_up(name: &str) -> bool {
    std::fs::read_to_string(format!("/sys/class/net/{name}/flags"))
        .ok()
        .and_then(|f| u32::from_str_radix(f.trim().trim_start_matches("0x"), 16).ok())
        .is_some_and(|flags| flags & 1 == 1)
}

pub fn download_root(state: &AppState, config: &Config) -> PathBuf {
    config
        .downloads
        .path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .and_then(|p| crate::paths::resolve_config_path(p, state.config.config_dir()).ok())
        .unwrap_or_else(|| state.paths.data_dir.join("downloads"))
}

pub fn download_dirs(state: &AppState, config: &Config) -> Vec<PathBuf> {
    let config_dir = state.config.config_dir();

    let overrides = config
        .libraries
        .iter()
        .filter_map(|l| l.download_path.as_deref())
        .chain(config.sources.iter().filter_map(|s| s.download_path.as_deref()))
        .filter(|p| !p.trim().is_empty())
        .filter_map(|p| crate::paths::resolve_config_path(p, config_dir).ok());

    let mut dirs: Vec<PathBuf> =
        std::iter::once(download_root(state, config)).chain(overrides).map(|p| p.canonicalize().unwrap_or(p)).collect();

    dirs.sort();
    dirs.dedup();
    dirs
}

impl Engine {
    pub fn start(config: &Config) -> anyhow::Result<Self> {
        let s = settings(config);
        let session = lt::new_session(&s).map_err(|e| anyhow::anyhow!("can't start the torrent engine: {e}"))?;
        tracing::info!("torrent engine ready (libtorrent {})", lt::version());

        Ok(Self {
            session,
            applied: Mutex::new(s),
            statuses: RwLock::new(HashMap::new()),
            killed: AtomicBool::new(false),
            listen: Mutex::new(None),
            listen_error: Mutex::new(None),
            turn: tokio::sync::Mutex::new(false),
        })
    }

    pub fn add(&self, params: &lt::AddParams) -> anyhow::Result<String> {
        self.session.add(params).map_err(|e| anyhow::anyhow!("{}", e.what()))
    }

    pub fn remove(&self, hash: &str, delete_files: bool) {
        self.session.remove(hash, delete_files);
        self.statuses.write().unwrap().remove(hash);
    }

    pub fn pause(&self, hash: &str) {
        self.session.pause_torrent(hash);
    }

    pub fn resume(&self, hash: &str) {
        self.session.resume_torrent(hash);
    }

    pub fn recheck(&self, hash: &str) {
        self.session.recheck(hash);
    }

    pub fn files(&self, hash: &str) -> Vec<lt::FileEntry> {
        self.session.files(hash)
    }

    pub fn status(&self, hash: &str) -> Option<lt::Status> {
        self.statuses.read().unwrap().get(hash).cloned()
    }

    pub fn overview(&self, state: &AppState) -> Overview {
        let config = state.config.current();
        let statuses = self.statuses.read().unwrap();

        Overview {
            version: lt::version(),
            download_rate: statuses.values().map(|s| s.download_rate).sum(),
            upload_rate: statuses.values().map(|s| s.upload_rate).sum(),
            active: statuses.values().filter(|s| !s.paused && (s.download_rate > 0 || s.upload_rate > 0)).count(),
            kill_switch: self
                .killed
                .load(Ordering::Relaxed)
                .then(|| config.downloads.bind_interface.clone().unwrap_or_default()),
            listening: self.listen.lock().unwrap().clone(),
            listen_error: self.listen_error.lock().unwrap().clone(),
            slow_hours: in_slow_hours(&config),
            download_path: download_root(state, &config),
        }
    }

    fn sync_settings(&self, config: &Config) {
        let wanted = settings(config);
        let iface = config.downloads.bind_interface.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let down = iface.is_some_and(|i| !interface_up(i));
        let was_killed = self.killed.swap(down, Ordering::Relaxed);
        let mut applied = self.applied.lock().unwrap();

        if down && !was_killed {
            tracing::warn!("{} is down; pausing every torrent until it's back", iface.unwrap_or_default());
            self.session.set_paused(true);
        }

        if *applied != wanted || (was_killed && !down) {
            self.session.apply_settings(&wanted);
            *applied = wanted;
        }

        if was_killed && !down {
            tracing::info!("{} is back; resuming torrents", iface.unwrap_or_default());
            self.session.set_paused(false);
        }
    }
}

#[derive(sqlx::FromRow)]
struct Stored {
    id: i64,
    hash: Option<String>,
    link: String,
    save_path: String,
    state: String,
    torrent: Option<Vec<u8>>,
    resume: Option<Vec<u8>>,
}

async fn restore(state: &Arc<AppState>, engine: &Engine) -> anyhow::Result<()> {
    let rows: Vec<Stored> = sqlx::query_as(
        "SELECT id, hash, link, save_path, state, torrent, resume FROM downloads WHERE state IN ('downloading', 'seeding', 'paused')",
    )
    .fetch_all(&state.db)
    .await?;

    for row in rows {
        let mut params = lt::AddParams {
            magnet: if row.link.starts_with("magnet:") { row.link.clone() } else { String::new() },
            torrent: row.torrent.clone().unwrap_or_default(),
            resume: row.resume.clone().unwrap_or_default(),
            save_path: row.save_path.clone(),
            paused: row.state == "paused",
        };

        let mut added = engine.add(&params);

        if added.is_err() && !params.resume.is_empty() {
            params.resume.clear();
            added = engine.add(&params);
        }

        match added {
            Ok(hash) => {
                if row.hash.as_deref() != Some(hash.as_str()) {
                    sqlx::query("UPDATE downloads SET hash = ? WHERE id = ?")
                        .bind(&hash)
                        .bind(row.id)
                        .execute(&state.db)
                        .await?;
                }
            },
            Err(e) => {
                tracing::warn!("can't restore download {}: {e:#}", row.id);

                sqlx::query("UPDATE downloads SET state = 'failed', error = ? WHERE id = ?")
                    .bind(format!("couldn't be restored after a restart: {e:#}"))
                    .bind(row.id)
                    .execute(&state.db)
                    .await?;
            },
        }
    }

    Ok(())
}

fn rules(json: Option<&str>, config: &Config) -> Seeding {
    json.and_then(|j| serde_json::from_str(j).ok()).unwrap_or_else(|| config.downloads.seeding.clone())
}

fn seeding_done(rules: &Seeding, s: &lt::Status) -> Option<String> {
    let ratio = s.all_time_upload as f64 / s.total_wanted.max(1) as f64;

    if let Some(r) = rules.ratio
        && ratio >= r
    {
        return Some(format!("reached ratio {ratio:.2}"));
    }

    if let Some(t) = rules.time
        && s.seeding_seconds as u64 >= t.as_secs()
    {
        return Some(format!(
            "seeded for {}",
            humantime::format_duration(Duration::from_secs(s.seeding_seconds as u64))
        ));
    }

    if let Some(idle) = rules.idle {
        let quiet = if s.last_upload_ago < 0 { s.seeding_seconds } else { s.last_upload_ago.min(s.seeding_seconds) };

        if quiet as u64 >= idle.as_secs() {
            return Some("nobody's downloading it anymore".into());
        }
    }

    None
}

async fn apply_seeding_rules(state: &Arc<AppState>, engine: &Engine) -> anyhow::Result<()> {
    let config = state.config.current();

    let rows: Vec<(i64, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, hash, seeding, import_state, import_mode FROM downloads WHERE state = 'seeding' AND hash IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await?;

    for (id, hash, seeding, import_state, import_mode) in rows {
        let Some(status) = engine.status(&hash) else { continue };

        if !matches!(status.state, 4 | 5) {
            continue;
        }

        if import_state == "pending" {
            continue;
        }

        let rules = rules(seeding.as_deref(), &config);
        let Some(why) = seeding_done(&rules, &status) else { continue };

        let imported = import_state == "done" && import_mode.as_deref() != Some("move");

        if rules.then == SeedAction::Remove && (imported || import_mode.as_deref() == Some("move")) {
            tracing::info!("done seeding {} ({why}); removing it", status.name);
            engine.remove(&hash, imported);

            sqlx::query("UPDATE downloads SET state = 'done', removed_at = ? WHERE id = ?")
                .bind(now())
                .bind(id)
                .execute(&state.db)
                .await?;
        } else {
            tracing::info!("done seeding {} ({why}); pausing it", status.name);
            engine.pause(&hash);
            sqlx::query("UPDATE downloads SET state = 'paused' WHERE id = ?").bind(id).execute(&state.db).await?;
        }

        state.events.send(Event::DownloadsChanged);
    }

    Ok(())
}

async fn handle_events(state: &Arc<AppState>, engine: &Engine) -> anyhow::Result<()> {
    for e in engine.session.poll() {
        match e.kind {
            1 => {
                let updated = sqlx::query(
                    "UPDATE downloads SET state = 'seeding', finished_at = COALESCE(finished_at, ?) WHERE hash = ? AND state = 'downloading'",
                )
                .bind(now())
                .bind(&e.hash)
                .execute(&state.db)
                .await?
                .rows_affected();

                engine.session.save_resume(&e.hash);

                if updated > 0 {
                    state.events.send(Event::DownloadsChanged);

                    let id: Option<i64> = sqlx::query_scalar("SELECT id FROM downloads WHERE hash = ?")
                        .bind(&e.hash)
                        .fetch_optional(&state.db)
                        .await?;

                    if let Some(id) = id {
                        let state = state.clone();

                        tokio::spawn(async move {
                            if let Err(err) = super::import::run(&state, id).await {
                                tracing::error!("importing download {id}: {err:#}");
                            }
                        });
                    }
                }
            },
            2 => {
                engine.session.save_resume(&e.hash);
                state.events.send(Event::DownloadsChanged);
            },
            3 => {
                sqlx::query("UPDATE downloads SET resume = ? WHERE hash = ?")
                    .bind(e.data.to_vec())
                    .bind(&e.hash)
                    .execute(&state.db)
                    .await?;
            },
            4 => {
                tracing::warn!("torrent {}: {}", e.hash, e.message);

                sqlx::query("UPDATE downloads SET error = ? WHERE hash = ?")
                    .bind(&e.message)
                    .bind(&e.hash)
                    .execute(&state.db)
                    .await?;

                state.events.send(Event::DownloadsChanged);
            },
            6 => {
                tracing::warn!("torrent engine: {}", e.message);
                *engine.listen_error.lock().unwrap() = Some(e.message.clone());
            },
            7 => {
                *engine.listen.lock().unwrap() = Some(e.message.clone());
                *engine.listen_error.lock().unwrap() = None;
            },
            _ => {},
        }
    }
    Ok(())
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let engine = &state.automation.engine;

        if let Err(e) = restore(&state, engine).await {
            tracing::error!("restoring downloads: {e:#}");
        }

        let mut last_rules = Instant::now();
        let mut last_resume = Instant::now();

        loop {
            let stopped = engine.turn.lock().await;

            if *stopped {
                return;
            }

            let config = state.config.current();
            engine.sync_settings(&config);
            let statuses = engine.session.statuses();

            {
                let mut map = engine.statuses.write().unwrap();
                map.clear();

                for s in statuses.iter() {
                    map.insert(s.hash.clone(), s.clone());
                }
            }

            if let Err(e) = handle_events(&state, engine).await {
                tracing::error!("torrent events: {e:#}");
            }

            if last_rules.elapsed() > Duration::from_secs(30) {
                last_rules = Instant::now();

                if let Err(e) = apply_seeding_rules(&state, engine).await {
                    tracing::error!("seeding rules: {e:#}");
                }
            }

            if last_resume.elapsed() > Duration::from_secs(300) {
                last_resume = Instant::now();

                for s in statuses.iter().filter(|s| s.need_save_resume) {
                    engine.session.save_resume(&s.hash);
                }
            }

            drop(stopped);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

pub async fn flush(state: &AppState) {
    let engine = &state.automation.engine;
    let mut stopped = engine.turn.lock().await;
    *stopped = true;
    let mut pending = HashSet::new();

    for s in engine.session.statuses().iter().filter(|s| s.need_save_resume) {
        engine.session.save_resume(&s.hash);
        pending.insert(s.hash.clone());
    }

    let deadline = Instant::now() + Duration::from_secs(3);

    loop {
        for e in engine.session.poll() {
            match e.kind {
                3 => {
                    let _ = sqlx::query("UPDATE downloads SET resume = ? WHERE hash = ?")
                        .bind(e.data.to_vec())
                        .bind(&e.hash)
                        .execute(&state.db)
                        .await;

                    pending.remove(&e.hash);
                },
                5 | 8 => {
                    pending.remove(&e.hash);
                },
                _ => {},
            }
        }

        if pending.is_empty() || Instant::now() >= deadline {
            break;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    if !pending.is_empty() {
        tracing::warn!("gave up waiting for resume data from {} torrent(s)", pending.len());
    }
}

pub fn ensure_dir(path: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("can't create the download folder {}", path.display()))
}
