// SPDX-License-Identifier: AGPL-3.0-or-later

//! The window button layout from the xdg-desktop-portal Settings interface, which GNOME, KDE and
//! others back with their own settings.

use futures::StreamExt;
use zbus::zvariant::{OwnedValue, Value};

use crate::chrome::ButtonLayout;

const NAMESPACE: &str = "org.gnome.desktop.wm.preferences";
const KEY: &str = "button-layout";

#[zbus::proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Settings {
    /// Version 2 of the interface.
    fn read_one(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;

    /// Version 1, which wraps the value in a second variant.
    fn read(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;

    #[zbus(signal)]
    fn setting_changed(&self, namespace: &str, key: &str, value: Value<'_>) -> zbus::Result<()>;
}

pub async fn watch_button_layout(send: Box<dyn Fn(ButtonLayout) + Send + Sync>) {
    if let Err(error) = follow(&*send).await {
        tracing::info!("no button layout from the settings portal, using the default: {error}");
    }
}

async fn follow(send: &(dyn Fn(ButtonLayout) + Send + Sync)) -> zbus::Result<()> {
    let connection = zbus::Connection::session().await?;
    let settings = SettingsProxy::new(&connection).await?;
    let mut changes = settings.receive_setting_changed_with_args(&[(0, NAMESPACE), (1, KEY)]).await?;

    let value = match settings.read_one(NAMESPACE, KEY).await {
        Ok(value) => value,
        Err(_) => settings.read(NAMESPACE, KEY).await?,
    };
    if let Some(layout) = parse(&value) {
        send(layout);
    }

    while let Some(change) = changes.next().await {
        if let Some(layout) = change.args().ok().and_then(|args| parse(args.value())) {
            send(layout);
        }
    }
    Ok(())
}

fn parse(value: &Value<'_>) -> Option<ButtonLayout> {
    match value {
        Value::Str(layout) => Some(ButtonLayout::parse(layout.as_str())),
        Value::Value(inner) => parse(inner),
        _ => None,
    }
}
