// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::auth::passkeys::Challenges;
use crate::config::ConfigStore;
use crate::events::Events;
use crate::library::Scanner;
use crate::logging::LogHandle;
use crate::media::MediaService;
use crate::metadata::MetadataService;
use crate::paths::Paths;
use crate::together::Together;

pub struct AppState {
    #[cfg(feature = "torrent")]
    pub automation: Arc<crate::automation::Automation>,
    pub paths: Paths,
    pub config: Arc<ConfigStore>,
    pub db: SqlitePool,
    pub events: Events,
    pub log: LogHandle,
    pub scanner: Scanner,
    pub metadata: MetadataService,
    pub media: Arc<MediaService>,
    pub passkeys: Challenges,
    pub http: reqwest::Client,
    pub together: Arc<Together>,
    pub clips: Arc<crate::clips::Clips>,
    pub music: crate::music::Music,
    pub books: crate::books::Books,
    pub tints: crate::tint::Tints,
}
