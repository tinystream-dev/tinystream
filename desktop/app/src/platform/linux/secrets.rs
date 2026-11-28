// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use futures::future::BoxFuture;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, Proxy};

use crate::api::Token;
use crate::secrets::{Secrets, Stored};

const SERVICE: &str = "org.freedesktop.secrets";
const ROOT: &str = "/org/freedesktop/secrets";
type Secret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);

#[derive(Default)]
pub struct LinuxSecrets {
    lock: tokio::sync::Mutex<()>,
}

impl Secrets for LinuxSecrets {
    fn get(&self, id: u64) -> BoxFuture<'_, Result<Stored>> {
        Box::pin(async move {
            let _guard = self.lock.lock().await;
            let local = tokio::task::spawn_blocking(move || read_file(id)).await??;
            let service = Service::open().await;
            if let Some(token) = local {
                if let Ok(service) = service {
                    service.set(id, token.clone()).await?;
                    tokio::task::spawn_blocking(move || remove_file(id)).await??;
                    return Ok(Stored { token, fallback: false });
                }
                return Ok(Stored { token, fallback: true });
            }
            match service {
                Ok(service) => Ok(Stored { token: service.get(id).await?, fallback: false }),
                Err(_) => Ok(Stored { token: None, fallback: true }),
            }
        })
    }

    fn set(&self, id: u64, token: Option<Token>) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async move {
            let _guard = self.lock.lock().await;
            match Service::open().await {
                Ok(service) => {
                    service.set(id, token).await?;
                    tokio::task::spawn_blocking(move || remove_file(id)).await??;
                    Ok(false)
                },
                Err(_) => {
                    tokio::task::spawn_blocking(move || write_file(id, token)).await??;
                    Ok(true)
                },
            }
        })
    }
}

fn directory() -> Result<PathBuf> {
    Ok(crate::paths::state_dir().context("No state directory is available.")?.join("tokens"))
}

fn read_file(id: u64) -> Result<Option<Option<Token>>> {
    read_at(&directory()?, id)
}

fn read_at(dir: &Path, id: u64) -> Result<Option<Option<Token>>> {
    let path = dir.join(id.to_string());
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => bail!("Couldn't read saved credentials."),
    };
    let owner = fs::metadata(dir)?.uid();
    if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 || metadata.uid() != owner {
        bail!("Saved credentials must be an owned file with mode 0600.");
    }
    let value: Option<String> = serde_json::from_slice(&fs::read(path).context("Couldn't read saved credentials.")?)
        .map_err(|_| anyhow::anyhow!("Couldn't read saved credentials."))?;
    Ok(Some(value.map(Token)))
}

fn write_file(id: u64, token: Option<Token>) -> Result<()> {
    write_at(&directory()?, id, token)
}

fn write_at(dir: &Path, id: u64, token: Option<Token>) -> Result<()> {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let metadata = fs::symlink_metadata(dir)?;
    if !metadata.is_dir() {
        bail!("Invalid credential directory.");
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let target = dir.join(id.to_string());
    let temporary = dir.join(format!(".{id}.{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .context("Couldn't save credentials.")?;
    let result = (|| -> Result<()> {
        let bytes = serde_json::to_vec(&token.map(|t| t.0))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result.context("Couldn't save credentials.")
}

fn remove_file(id: u64) -> Result<()> {
    match fs::remove_file(directory()?.join(id.to_string())) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => bail!("Couldn't remove saved credentials."),
    }
}

struct Service {
    connection: Connection,
    session: OwnedObjectPath,
}

impl Service {
    async fn open() -> Result<Self> {
        let connection = Connection::session().await?;
        let proxy = Proxy::new(&connection, SERVICE, ROOT, "org.freedesktop.Secret.Service").await?;
        let (_, session): (OwnedValue, OwnedObjectPath) =
            proxy.call("OpenSession", &("plain", Value::from(""))).await?;
        Ok(Self { connection, session })
    }

    async fn proxy<'a>(&'a self, path: &'a str, interface: &'a str) -> Result<Proxy<'a>> {
        Ok(Proxy::new(&self.connection, SERVICE, path, interface).await?)
    }

    async fn prompt(&self, path: OwnedObjectPath) -> Result<OwnedValue> {
        let proxy = self.proxy(path.as_str(), "org.freedesktop.Secret.Prompt").await?;
        let mut completed = proxy.receive_signal("Completed").await?;
        proxy.call::<_, _, ()>("Prompt", &("",)).await?;
        let signal = tokio::time::timeout(std::time::Duration::from_secs(120), completed.next())
            .await?
            .context("Credential prompt closed.")?;
        let (dismissed, result): (bool, OwnedValue) = signal.body().deserialize()?;
        if dismissed {
            bail!("Unlock your keyring to save this sign-in.");
        }
        Ok(result)
    }

    async fn items(&self, id: u64) -> Result<Vec<OwnedObjectPath>> {
        let proxy = self.proxy(ROOT, "org.freedesktop.Secret.Service").await?;
        let attributes =
            HashMap::from([("application", "dev.tinystream.Tinystream".to_string()), ("server", id.to_string())]);
        let (mut unlocked, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) =
            proxy.call("SearchItems", &(attributes,)).await?;
        if !locked.is_empty() {
            let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = proxy.call("Unlock", &(&locked,)).await?;
            if prompt.as_str() != "/" {
                self.prompt(prompt).await?;
            }
            unlocked.extend(locked);
        }
        Ok(unlocked)
    }

    async fn get(&self, id: u64) -> Result<Option<Token>> {
        let Some(item) = self.items(id).await?.into_iter().next() else { return Ok(None) };
        let proxy = self.proxy(item.as_str(), "org.freedesktop.Secret.Item").await?;
        let (_, _, bytes, _): Secret = proxy.call("GetSecret", &(&self.session,)).await?;
        Ok(Some(Token(String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("Invalid saved credential."))?)))
    }

    async fn set(&self, id: u64, token: Option<Token>) -> Result<()> {
        let Some(token) = token else {
            for item in self.items(id).await? {
                let proxy = self.proxy(item.as_str(), "org.freedesktop.Secret.Item").await?;
                let prompt: OwnedObjectPath = proxy.call("Delete", &()).await?;
                if prompt.as_str() != "/" {
                    self.prompt(prompt).await?;
                }
            }
            return Ok(());
        };
        let service = self.proxy(ROOT, "org.freedesktop.Secret.Service").await?;
        let mut collection: OwnedObjectPath = service.call("ReadAlias", &("default",)).await?;
        if collection.as_str() == "/" {
            let properties = HashMap::from([("org.freedesktop.Secret.Collection.Label", Value::from("tinystream"))]);
            let (path, prompt): (OwnedObjectPath, OwnedObjectPath) =
                service.call("CreateCollection", &(properties, "default")).await?;
            collection = if prompt.as_str() == "/" { path } else { self.prompt(prompt).await?.try_into()? };
        }
        let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) =
            service.call("Unlock", &(vec![collection.clone()],)).await?;
        if prompt.as_str() != "/" {
            self.prompt(prompt).await?;
        }
        let proxy = self.proxy(collection.as_str(), "org.freedesktop.Secret.Collection").await?;
        let attributes =
            HashMap::from([("application", "dev.tinystream.Tinystream".to_string()), ("server", id.to_string())]);
        let properties = HashMap::from([
            ("org.freedesktop.Secret.Item.Label", Value::from("tinystream sign-in")),
            ("org.freedesktop.Secret.Item.Attributes", Value::from(attributes)),
        ]);
        let secret: Secret = (self.session.clone(), vec![], token.0.into_bytes(), "text/plain; charset=utf8".into());
        let (_, prompt): (OwnedObjectPath, OwnedObjectPath) =
            proxy.call("CreateItem", &(properties, secret, true)).await?;
        if prompt.as_str() != "/" {
            self.prompt(prompt).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Directory, run};

    #[test]
    fn fallback_is_private_and_sign_out_leaves_a_tombstone() {
        let dir = Directory::new("fallback");
        write_at(&dir.0, 1, Some(Token("test-credential".into()))).unwrap();
        assert_eq!(fs::metadata(dir.0.join("1")).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(read_at(&dir.0, 1).unwrap().unwrap().unwrap().0, "test-credential");
        write_at(&dir.0, 1, None).unwrap();
        assert!(read_at(&dir.0, 1).unwrap().unwrap().is_none());
        fs::set_permissions(dir.0.join("1"), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_at(&dir.0, 1).is_err());
        fs::remove_file(dir.0.join("1")).unwrap();
        std::os::unix::fs::symlink("missing", dir.0.join("1")).unwrap();
        assert!(read_at(&dir.0, 1).is_err());
    }

    #[test]
    #[ignore = "requires isolated XDG directories; run with a private keyring or TINYSTREAM_TEST_FILE_SECRETS=1 and no session bus"]
    fn secret_service_round_trip() {
        run(async {
            let store = LinuxSecrets::default();
            let id = u64::MAX;
            let fallback = store.set(id, Some(Token("first-test-credential".into()))).await.unwrap();
            assert_eq!(fallback, std::env::var("TINYSTREAM_TEST_FILE_SECRETS").is_ok());
            let restarted = LinuxSecrets::default();
            assert_eq!(restarted.get(id).await.unwrap().token.unwrap().0, "first-test-credential");
            restarted.set(id, Some(Token("replacement-test-credential".into()))).await.unwrap();
            assert_eq!(store.get(id).await.unwrap().token.unwrap().0, "replacement-test-credential");
            store.set(id, None).await.unwrap();
            assert!(restarted.get(id).await.unwrap().token.is_none());
        });
    }
}
