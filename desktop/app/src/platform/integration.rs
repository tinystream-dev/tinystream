// SPDX-License-Identifier: AGPL-3.0-or-later

//! Platform-neutral state and commands for desktop services. Music can publish the same state
//! when its player lands; only the active player is exported.

#[derive(Debug, Clone)]
pub enum Command {
    Open(Vec<String>),
    Show,
    ToggleWindow,
    Quit,
    Play,
    Pause,
    Stop,
    Toggle,
    Next,
    Previous,
    Seek(f64),
    Position(String, f64),
    Rate(f64),
    Volume(f64),
    Notification(String),
    Tray(bool),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Media {
    pub track: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: String,
    pub length: f64,
    pub position: f64,
    pub playing: bool,
    pub rate: f64,
    pub volume: f64,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
    pub discontinuities: u64,
    pub tray: bool,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub link: String,
}

pub struct Integration {
    pub media: tokio::sync::watch::Sender<Media>,
    pub notifications: tokio::sync::mpsc::UnboundedSender<Notification>,
    pub task: tokio::task::JoinHandle<()>,
}

impl Drop for Integration {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// IDs are scoped to the server and account, and survive reconnects and app restarts.
#[derive(Default)]
pub struct NotificationHistory(std::collections::BTreeSet<(String, i64, i64)>);

impl NotificationHistory {
    pub fn load() -> Self {
        let entries = crate::paths::state_dir()
            .and_then(|dir| std::fs::read(dir.join("notifications.json")).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self(entries)
    }

    pub fn receive(&mut self, server: String, user: i64, id: i64) -> bool {
        if !self.0.insert((server, user, id)) {
            return false;
        }
        if let Some(dir) = crate::paths::state_dir() {
            let result = (|| -> anyhow::Result<()> {
                std::fs::create_dir_all(&dir)?;
                let temporary = dir.join("notifications.json.tmp");
                std::fs::write(&temporary, serde_json::to_vec(&self.0)?)?;
                std::fs::rename(temporary, dir.join("notifications.json"))?;
                Ok(())
            })();
            if let Err(error) = result {
                tracing::warn!("can't remember desktop notifications: {error}");
            }
        }
        true
    }
}
