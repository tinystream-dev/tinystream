// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use async_graphql::{Context, InputObject, MergedObject, MergedSubscription, Schema, SimpleObject};
use axum::http::HeaderMap;

use super::appearance::{AppearanceMutation, AppearanceQuery};
use super::auth::{AuthMutation, AuthQuery};
#[cfg(feature = "torrent")]
use super::automation::{AutomationMutation, AutomationQuery};
use super::clips::{ClipMutation, ClipQuery};
use super::discovery::DiscoveryQuery;
use super::events::EventSubscription;
use super::invites::{InviteMutation, InviteQuery};
use super::library::{LibraryMutation, LibraryQuery};
use super::listen::{ListenMutation, ListenQuery};
use super::music::{MusicMutation, MusicQuery};
use super::notifications::{NotificationMutation, NotificationQuery};
use super::settings::{SettingsMutation, SettingsQuery};
use super::together::{RoomMutation, RoomQuery};
use super::users::{UserMutation, UserQuery};
use crate::auth::User;
use crate::auth::permissions::Permissions;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[cfg(feature = "torrent")]
#[derive(MergedObject, Default)]
pub struct Query(
    AuthQuery,
    InviteQuery,
    UserQuery,
    LibraryQuery,
    MusicQuery,
    DiscoveryQuery,
    RoomQuery,
    ListenQuery,
    ClipQuery,
    NotificationQuery,
    SettingsQuery,
    AppearanceQuery,
    AutomationQuery,
);

#[cfg(not(feature = "torrent"))]
#[derive(MergedObject, Default)]
pub struct Query(
    AuthQuery,
    InviteQuery,
    UserQuery,
    LibraryQuery,
    MusicQuery,
    DiscoveryQuery,
    RoomQuery,
    ListenQuery,
    ClipQuery,
    NotificationQuery,
    SettingsQuery,
    AppearanceQuery,
);

#[cfg(feature = "torrent")]
#[derive(MergedObject, Default)]
pub struct Mutation(
    AuthMutation,
    InviteMutation,
    UserMutation,
    LibraryMutation,
    MusicMutation,
    RoomMutation,
    ListenMutation,
    ClipMutation,
    NotificationMutation,
    SettingsMutation,
    AppearanceMutation,
    AutomationMutation,
);

#[cfg(not(feature = "torrent"))]
#[derive(MergedObject, Default)]
pub struct Mutation(
    AuthMutation,
    InviteMutation,
    UserMutation,
    LibraryMutation,
    MusicMutation,
    RoomMutation,
    ListenMutation,
    ClipMutation,
    NotificationMutation,
    SettingsMutation,
    AppearanceMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(EventSubscription);

pub type AppSchema = Schema<Query, Mutation, Subscription>;

pub fn build(state: Arc<AppState>) -> AppSchema {
    Schema::build(Query::default(), Mutation::default(), Subscription::default()).data(state).limit_depth(16).finish()
}

#[cfg(test)]
pub fn sdl() -> String {
    Schema::build(Query::default(), Mutation::default(), Subscription::default()).finish().sdl()
}

pub struct Session {
    pub user: Option<User>,
    pub token: Option<String>,
    pub headers: HeaderMap,
}

pub trait Ctx {
    fn state(&self) -> &Arc<AppState>;
    fn session(&self) -> &Session;

    fn maybe_user(&self) -> Option<&User> {
        self.session().user.as_ref()
    }

    fn user(&self) -> ApiResult<&User> {
        self.maybe_user().ok_or_else(ApiError::unauthorized)
    }
    fn admin(&self) -> ApiResult<&User> {
        let user = self.user()?;
        if user.is_admin { Ok(user) } else { Err(ApiError::forbidden()) }
    }

    fn allowed(&self, can: impl FnOnce(&Permissions) -> bool) -> ApiResult<&User> {
        let user = self.user()?;
        if can(&user.permissions) { Ok(user) } else { Err(ApiError::forbidden()) }
    }

    fn access(&self) -> ApiResult<Arc<Access>> {
        Ok(Arc::new(Access::User(self.user()?.clone())))
    }
}

impl Ctx for Context<'_> {
    fn state(&self) -> &Arc<AppState> {
        self.data_unchecked::<Arc<AppState>>()
    }

    fn session(&self) -> &Session {
        self.data_unchecked::<Session>()
    }
}

#[derive(Debug, Clone)]
pub enum Access {
    User(User),

    Room { code: String, host: User, item_id: i64 },
}

impl Access {
    pub fn person(&self) -> &User {
        match self {
            Access::User(u) => u,
            Access::Room { host, .. } => host,
        }
    }

    pub fn user(&self) -> Option<&User> {
        match self {
            Access::User(u) => Some(u),
            Access::Room { .. } => None,
        }
    }

    pub fn room(&self) -> Option<&str> {
        match self {
            Access::User(_) => None,
            Access::Room { code, .. } => Some(code),
        }
    }

    pub fn sees(&self, item_id: i64, library: &str) -> bool {
        match self {
            Access::User(u) => u.permissions.can_see(library),
            Access::Room { item_id: room_item, .. } => *room_item == item_id,
        }
    }
}

#[derive(SimpleObject, InputObject, Debug, Clone, Copy, PartialEq, Eq)]
#[graphql(input_name = "EpisodeNumberInput")]
pub struct EpisodeNumber {
    pub season: u32,
    pub episode: u32,
}

impl From<(u32, u32)> for EpisodeNumber {
    fn from((season, episode): (u32, u32)) -> Self {
        Self { season, episode }
    }
}

impl From<EpisodeNumber> for (u32, u32) {
    fn from(n: EpisodeNumber) -> Self {
        (n.season, n.episode)
    }
}

#[cfg(test)]
mod tests {

    #[cfg(all(feature = "torrent", feature = "metadata"))]
    #[test]
    fn schema_file_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/web/schema.graphql");
        let sdl = super::sdl();
        if std::env::var_os("TINYSTREAM_WRITE_SCHEMA").is_some() {
            std::fs::write(path, &sdl).unwrap();
            return;
        }
        let current = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            current == sdl,
            "web/schema.graphql is out of date; run `TINYSTREAM_WRITE_SCHEMA=1 cargo test schema_file_is_current`, \
             then `bun run codegen` in web/"
        );
    }
}
