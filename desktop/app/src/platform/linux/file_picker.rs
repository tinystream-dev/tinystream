// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;

use futures::StreamExt;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub async fn image() -> Result<Option<PathBuf>, String> {
    async fn choose() -> anyhow::Result<Option<PathBuf>> {
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
        let mut options: HashMap<&str, OwnedValue> = HashMap::new();
        options.insert("handle_token", Value::from(token.as_str()).try_to_owned()?);
        options.insert("modal", OwnedValue::from(true));
        options.insert("multiple", OwnedValue::from(false));
        let filters =
            vec![("Images", vec![(1u32, "image/png"), (1, "image/jpeg"), (1, "image/webp"), (1, "image/gif")])];
        options.insert("filters", Value::from(filters).try_to_owned()?);
        let _: OwnedObjectPath = chooser.call("OpenFile", &("", "Choose artwork", options)).await?;
        let response = response.next().await.ok_or_else(|| anyhow::anyhow!("The file chooser closed"))?;
        let (code, result): (u32, HashMap<String, OwnedValue>) = response.body().deserialize()?;
        if code == 1 {
            return Ok(None);
        }
        anyhow::ensure!(code == 0, "The file chooser couldn't open a picture");
        let uris = result.get("uris").ok_or_else(|| anyhow::anyhow!("No picture selected"))?.try_clone()?;
        let uris: Vec<String> = uris.try_into()?;
        let Some(uri) = uris.first() else { return Ok(None) };
        let path = url::Url::parse(uri)?.to_file_path().map_err(|_| anyhow::anyhow!("Choose a local picture"))?;
        Ok(Some(path))
    }
    choose().await.map_err(|e| format!("Couldn't choose artwork: {e}"))
}
