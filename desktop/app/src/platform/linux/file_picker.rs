// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;

use futures::StreamExt;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

/// Asks the portal's file chooser to `method` (`OpenFile` or `SaveFile`) with `options`, and
/// returns the file chosen, or `None` when the person cancelled.
async fn choose(method: &str, title: &str, mut options: HashMap<&str, OwnedValue>) -> anyhow::Result<Option<PathBuf>> {
    let connection = zbus::Connection::session().await?;
    let token =
        format!("tinystream_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos());
    let sender = connection
        .unique_name()
        .ok_or_else(|| anyhow::anyhow!("No portal connection"))?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let request = zbus::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await?;
    let mut response = request.receive_signal("Response").await?;
    let chooser = zbus::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.FileChooser",
    )
    .await?;
    options.insert("handle_token", Value::from(token.as_str()).try_to_owned()?);
    options.insert("modal", OwnedValue::from(true));
    let _: OwnedObjectPath = chooser.call(method, &("", title, options)).await?;
    let response = response.next().await.ok_or_else(|| anyhow::anyhow!("The file chooser closed"))?;
    let (code, result): (u32, HashMap<String, OwnedValue>) = response.body().deserialize()?;
    if code == 1 {
        return Ok(None);
    }
    anyhow::ensure!(code == 0, "The file chooser couldn't open");
    let uris = result.get("uris").ok_or_else(|| anyhow::anyhow!("No file chosen"))?.try_clone()?;
    let uris: Vec<String> = uris.try_into()?;
    let Some(uri) = uris.first() else { return Ok(None) };
    let path = url::Url::parse(uri)?.to_file_path().map_err(|_| anyhow::anyhow!("Choose a local file"))?;
    Ok(Some(path))
}

pub async fn image() -> Result<Option<PathBuf>, String> {
    async fn pick() -> anyhow::Result<Option<PathBuf>> {
        let mut options: HashMap<&str, OwnedValue> = HashMap::new();
        options.insert("multiple", OwnedValue::from(false));
        let filters =
            vec![("Images", vec![(1u32, "image/png"), (1, "image/jpeg"), (1, "image/webp"), (1, "image/gif")])];
        options.insert("filters", Value::from(filters).try_to_owned()?);
        choose("OpenFile", "Choose artwork", options).await
    }
    pick().await.map_err(|e| format!("Couldn't choose artwork: {e}"))
}

pub async fn save(name: String, title: String) -> Result<Option<PathBuf>, String> {
    async fn pick(name: &str, title: &str) -> anyhow::Result<Option<PathBuf>> {
        let mut options: HashMap<&str, OwnedValue> = HashMap::new();
        options.insert("current_name", Value::from(name).try_to_owned()?);
        if let Some(downloads) = dirs::download_dir() {
            let mut folder = downloads.into_os_string().into_encoded_bytes();
            folder.push(0);
            options.insert("current_folder", Value::from(folder).try_to_owned()?);
        }
        choose("SaveFile", title, options).await
    }
    pick(&name, &title).await.map_err(|e| format!("Couldn't choose where to save it: {e}"))
}
