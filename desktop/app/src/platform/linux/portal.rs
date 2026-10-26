// SPDX-License-Identifier: AGPL-3.0-or-later

//! Desktop settings from the xdg-desktop-portal Settings interface, which GNOME, KDE and others
//! back with their own: the window button layout, and whether to reduce motion.

use futures::StreamExt;
use zbus::zvariant::{OwnedValue, Value};

use crate::chrome::ButtonLayout;

const LAYOUT: (&str, &str) = ("org.gnome.desktop.wm.preferences", "button-layout");
/// `1` for "reduce", in newer portals.
const REDUCED_MOTION: (&str, &str) = ("org.freedesktop.appearance", "reduced-motion");
/// GNOME's switch, read where the portal doesn't have the standard key.
const ANIMATIONS: (&str, &str) = ("org.gnome.desktop.interface", "enable-animations");

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
    let parse = |value: &Value<'_>| match value {
        Value::Str(layout) => Some(ButtonLayout::parse(layout.as_str())),
        _ => None,
    };
    if let Err(error) = follow(LAYOUT, parse, &*send).await {
        tracing::info!("no button layout from the settings portal, using the default: {error}");
    }
}

pub async fn watch_reduced_motion(send: Box<dyn Fn(bool) + Send + Sync>) {
    let standard = |value: &Value<'_>| match value {
        Value::U32(preference) => Some(*preference == 1),
        _ => None,
    };
    let gnome = |value: &Value<'_>| match value {
        Value::Bool(animations) => Some(!animations),
        _ => None,
    };
    if follow(REDUCED_MOTION, standard, &*send).await.is_err()
        && let Err(error) = follow(ANIMATIONS, gnome, &*send).await
    {
        tracing::info!("no reduced motion setting from the settings portal: {error}");
    }
}

/// Calls `send` with a setting now and whenever it changes.
async fn follow<T>(
    (namespace, key): (&str, &str),
    parse: impl Fn(&Value<'_>) -> Option<T>,
    send: &(dyn Fn(T) + Send + Sync),
) -> zbus::Result<()> {
    let connection = zbus::Connection::session().await?;
    let settings = SettingsProxy::new(&connection).await?;
    let mut changes = settings.receive_setting_changed_with_args(&[(0, namespace), (1, key)]).await?;

    let value = match settings.read_one(namespace, key).await {
        Ok(value) => value,
        Err(_) => settings.read(namespace, key).await?,
    };
    if let Some(value) = unwrap(&value, &parse) {
        send(value);
    }

    while let Some(change) = changes.next().await {
        if let Some(value) = change.args().ok().and_then(|args| unwrap(args.value(), &parse)) {
            send(value);
        }
    }
    Ok(())
}

/// Version 1 of the interface wraps values in a second variant.
fn unwrap<T>(value: &Value<'_>, parse: &impl Fn(&Value<'_>) -> Option<T>) -> Option<T> {
    match value {
        Value::Value(inner) => unwrap(inner, parse),
        value => parse(value),
    }
}
