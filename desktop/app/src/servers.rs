// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use url::Url;

use crate::api::{Api, Token};
use crate::secrets;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: u64,
    pub url: String,
    pub name: String,
    pub user: Option<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub servers: Vec<Server>,
    pub active: Option<u64>,
    pub check_updates: bool,
    pub last_check: u64,
    pub release: Option<Release>,
    pub dismissed: Vec<String>,
    pub fallback_seen: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            servers: vec![],
            active: None,
            check_updates: true,
            last_check: 0,
            release: None,
            dismissed: vec![],
            fallback_seen: false,
        }
    }
}

impl Config {
    pub fn active(&self) -> Option<&Server> {
        self.servers.iter().find(|s| Some(s.id) == self.active)
    }
}

pub struct Store {
    pub config: Mutex<Config>,
    secrets: secrets::Store,
    path: PathBuf,
}

impl Store {
    pub async fn load(secrets: secrets::Store) -> Result<Arc<Self>> {
        let path = crate::paths::config_dir().context("No configuration directory is available.")?.join("servers.json");
        Self::open(path, secrets).await
    }

    pub async fn open(path: PathBuf, secrets: secrets::Store) -> Result<Arc<Self>> {
        let config = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).context("Couldn't read saved servers.")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(_) => anyhow::bail!("Couldn't read saved servers."),
        };
        Ok(Arc::new(Self { config: Mutex::new(config), secrets, path }))
    }

    pub async fn save(&self, change: impl FnOnce(&mut Config)) -> Result<Config> {
        let mut config = self.config.lock().await;
        let mut next = config.clone();
        change(&mut next);
        tokio::fs::create_dir_all(self.path.parent().context("Invalid config path.")?).await?;
        let temporary = self.path.with_extension("json.tmp");
        tokio::fs::write(&temporary, serde_json::to_vec_pretty(&next)?).await.context("Couldn't save servers.")?;
        tokio::fs::rename(temporary, &self.path).await.context("Couldn't save servers.")?;
        *config = next;
        Ok(config.clone())
    }

    pub async fn activate(&self, id: u64) -> Result<(Config, Api, bool)> {
        let config = self.save(|config| config.active = Some(id)).await?;
        let (api, fallback) = self.client(id).await?;
        Ok((config, api, fallback))
    }

    pub async fn client(&self, id: u64) -> Result<(Api, bool)> {
        let url = self
            .config
            .lock()
            .await
            .servers
            .iter()
            .find(|server| server.id == id)
            .context("Unknown server.")?
            .url
            .clone();
        let stored = self.secrets.get(id).await?;
        let api = Api::new(Url::parse(&url)?, stored.token)?;
        Ok((api, stored.fallback))
    }

    pub async fn add(&self, api: &Api) -> Result<Config> {
        let mut bytes = [0; 8];
        getrandom::fill(&mut bytes).map_err(|_| anyhow::anyhow!("Couldn't create a server identifier."))?;
        let fresh_id = u64::from_le_bytes(bytes);
        self.save(|config| {
            let id = config.servers.iter().find(|s| s.url == api.origin.as_str()).map(|s| s.id).unwrap_or_else(|| {
                let id = fresh_id;
                config.servers.push(Server {
                    id,
                    url: api.origin.to_string(),
                    name: api.origin.authority().to_string(),
                    user: None,
                });
                id
            });
            config.active = Some(id);
        })
        .await
    }

    pub async fn signed_in(&self, id: u64, token: Token, user: User) -> Result<(Config, bool)> {
        let fallback = self.secrets.set(id, Some(token)).await?;
        let config = self
            .save(|config| {
                if let Some(server) = config.servers.iter_mut().find(|s| s.id == id) {
                    server.user = Some(user);
                }
            })
            .await?;
        Ok((config, fallback))
    }

    pub async fn sign_out(&self, id: u64, remove: bool) -> Result<Config> {
        self.secrets.set(id, None).await?;
        self.save(|config| {
            if remove {
                config.servers.retain(|s| s.id != id);
                if config.active == Some(id) {
                    config.active = config.servers.first().map(|s| s.id);
                }
            } else if let Some(server) = config.servers.iter_mut().find(|s| s.id == id) {
                server.user = None;
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Directory, MemorySecrets, run};

    #[test]
    fn two_servers_keep_their_own_credentials_across_restarts() {
        run(async {
            let dir = Directory::new("servers");
            let path = dir.0.join("servers.json");
            let secrets = Arc::new(MemorySecrets::default());
            let store = Store::open(path.clone(), secrets.clone()).await.unwrap();
            let mut ids = vec![];
            for (origin, token) in [("https://one.example", "one-secret"), ("http://two.example:3000", "two-secret")] {
                let api = Api::new(Url::parse(origin).unwrap(), None).unwrap();
                let config = store.add(&api).await.unwrap();
                ids.push(config.active.unwrap());
                store
                    .signed_in(
                        config.active.unwrap(),
                        Token(token.into()),
                        User { id: 1, username: "David".into(), avatar: None },
                    )
                    .await
                    .unwrap();
            }
            let saved = tokio::fs::read_to_string(&path).await.unwrap();
            assert!(!saved.contains("secret"));
            let restarted = Store::open(path, secrets).await.unwrap();
            assert_eq!(restarted.config.lock().await.servers.len(), 2);
            for (id, token) in [(ids[0], "one-secret"), (ids[1], "two-secret")] {
                let (_, api, _) = restarted.activate(id).await.unwrap();
                assert_eq!(api.token.unwrap().0, token);
            }
            restarted.sign_out(ids[0], false).await.unwrap();
            assert!(restarted.activate(ids[0]).await.unwrap().1.token.is_none());
            assert!(restarted.activate(ids[1]).await.unwrap().1.token.is_some());
            let config = restarted.sign_out(ids[1], true).await.unwrap();
            assert_eq!(config.active, Some(ids[0]));
            assert_eq!(config.servers.len(), 1);
        });
    }
}
