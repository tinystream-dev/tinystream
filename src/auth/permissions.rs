// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::config::{Config, RequestMode};
use crate::error::ApiResult;
use crate::state::AppState;

const DEFAULTS_KEY: &str = "permission-defaults";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(input_name = "PermissionsInput")]
#[serde(rename_all = "camelCase", default)]
pub struct Permissions {
    pub all_libraries: bool,

    pub libraries: Vec<String>,

    pub request: bool,

    pub auto_approve: bool,

    pub request_limit: u32,

    pub manage_requests: bool,

    pub manage_shows: bool,

    pub downloads: bool,

    pub edit_metadata: bool,

    pub watch_together: bool,

    pub share_links: bool,

    pub clip: bool,

    pub clip_max_length: u32,

    pub clip_limit: u32,

    pub clip_storage: u32,

    pub clip_links: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            all_libraries: true,
            libraries: Vec::new(),
            request: false,
            auto_approve: false,
            request_limit: 0,
            manage_requests: false,
            manage_shows: false,
            downloads: false,
            edit_metadata: false,
            watch_together: true,
            share_links: false,
            clip: true,
            clip_max_length: 60,
            clip_limit: 50,
            clip_storage: 2048,
            clip_links: false,
        }
    }
}

impl Permissions {
    pub fn everything() -> Self {
        Self {
            all_libraries: true,
            libraries: Vec::new(),
            request: true,
            auto_approve: true,
            request_limit: 0,
            manage_requests: true,
            manage_shows: true,
            downloads: true,
            edit_metadata: true,
            watch_together: true,
            share_links: true,
            clip: true,
            clip_max_length: 0,
            clip_limit: 0,
            clip_storage: 0,
            clip_links: true,
        }
    }

    fn from_config(config: &Config) -> Self {
        let mode = config.requests.mode.unwrap_or_default();

        Self {
            request: mode != RequestMode::Off,
            auto_approve: mode == RequestMode::Auto,
            request_limit: config.requests.max_open.unwrap_or(0),
            ..Self::default()
        }
    }

    pub fn can_see(&self, library: &str) -> bool {
        self.all_libraries || self.libraries.iter().any(|l| l == library)
    }
}

#[derive(
    Debug, Clone, Default, PartialEq, Serialize, Deserialize, async_graphql::SimpleObject, async_graphql::InputObject,
)]
#[graphql(name = "PermissionOverrides", input_name = "PermissionOverridesInput")]
#[serde(rename_all = "camelCase")]
pub struct Overrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_libraries: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libraries: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manage_requests: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manage_shows: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_metadata: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_together: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_links: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_max_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_storage: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_links: Option<bool>,
}

impl Overrides {
    pub fn parse(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    pub fn apply(&self, d: &Permissions) -> Permissions {
        Permissions {
            all_libraries: self.all_libraries.unwrap_or(d.all_libraries),
            libraries: self.libraries.clone().unwrap_or_else(|| d.libraries.clone()),
            request: self.request.unwrap_or(d.request),
            auto_approve: self.auto_approve.unwrap_or(d.auto_approve),
            request_limit: self.request_limit.unwrap_or(d.request_limit),
            manage_requests: self.manage_requests.unwrap_or(d.manage_requests),
            manage_shows: self.manage_shows.unwrap_or(d.manage_shows),
            downloads: self.downloads.unwrap_or(d.downloads),
            edit_metadata: self.edit_metadata.unwrap_or(d.edit_metadata),
            watch_together: self.watch_together.unwrap_or(d.watch_together),
            share_links: self.share_links.unwrap_or(d.share_links),
            clip: self.clip.unwrap_or(d.clip),
            clip_max_length: self.clip_max_length.unwrap_or(d.clip_max_length),
            clip_limit: self.clip_limit.unwrap_or(d.clip_limit),
            clip_storage: self.clip_storage.unwrap_or(d.clip_storage),
            clip_links: self.clip_links.unwrap_or(d.clip_links),
        }
    }
}

pub fn effective(is_admin: bool, overrides: &Overrides, defaults: &Permissions) -> Permissions {
    if is_admin { Permissions::everything() } else { overrides.apply(defaults) }
}

pub async fn defaults(state: &AppState) -> ApiResult<Permissions> {
    let stored: Option<String> = sqlx::query_scalar("SELECT value FROM server_settings WHERE key = ?")
        .bind(DEFAULTS_KEY)
        .fetch_optional(&state.db)
        .await?;

    if let Some(json) = stored {
        return Ok(serde_json::from_str(&json).unwrap_or_default());
    }

    let seeded = Permissions::from_config(&state.config.current());
    set_defaults(state, &seeded).await?;
    Ok(seeded)
}

pub async fn set_defaults(state: &AppState, p: &Permissions) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO server_settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(DEFAULTS_KEY)
    .bind(serde_json::to_string(p).map_err(anyhow::Error::from)?)
    .execute(&state.db)
    .await?;

    Ok(())
}

pub async fn rename_library(state: &AppState, old: &str, new: &str) -> ApiResult<()> {
    let rename = |list: &mut Vec<String>| {
        let mut changed = false;

        for l in list.iter_mut().filter(|l| *l == old) {
            *l = new.to_string();
            changed = true;
        }

        changed
    };

    let mut d = defaults(state).await?;

    if rename(&mut d.libraries) {
        set_defaults(state, &d).await?;
    }

    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, permissions FROM users").fetch_all(&state.db).await?;

    for (id, json) in rows {
        let mut o = Overrides::parse(&json);

        if o.libraries.as_mut().is_some_and(rename) {
            sqlx::query("UPDATE users SET permissions = ? WHERE id = ?")
                .bind(o.to_json())
                .bind(id)
                .execute(&state.db)
                .await?;
        }
    }

    Ok(())
}
