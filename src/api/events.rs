// SPDX-License-Identifier: AGPL-3.0-or-later

use async_graphql::{Context, Enum, SimpleObject, Subscription, Union};
use futures::stream::{BoxStream, StreamExt};
use tokio::sync::broadcast::error::RecvError;

use super::clips::ClipState;
use super::schema::{Ctx, EpisodeNumber};
use crate::events::Event;
use crate::notifications::Notification;

#[derive(SimpleObject)]
pub struct ConfigChanged {
    error: Option<String>,
}

#[derive(SimpleObject)]
pub struct ScanStarted {
    library: String,
}

#[derive(SimpleObject)]
pub struct ScanFinished {
    library: String,
    titles: usize,
    videos: usize,
    skipped: usize,
}

#[derive(SimpleObject)]
pub struct LibraryChanged {
    library: String,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataStatus {
    Fetching,
    Updated,
    Failed,
}

#[derive(SimpleObject)]
pub struct MetadataChanged {
    title_id: i64,
    status: MetadataStatus,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangedList {
    Downloads,
    RenameSuggestions,
    Requests,

    Users,

    Notifications,

    Appearance,

    Playlists,
}

#[derive(SimpleObject)]
pub struct ListChanged {
    list: ChangedList,
}

#[derive(SimpleObject)]
pub struct SeriesChanged {
    series_id: i64,
}

#[derive(SimpleObject)]
pub struct EpisodesImported {
    library: String,
    show: String,
    title_id: Option<i64>,
    episodes: Vec<EpisodeNumber>,
}

#[derive(SimpleObject)]
pub struct NotificationReceived {
    notification: Notification,
}

#[derive(SimpleObject)]
pub struct ClipChanged {
    clip_id: i64,

    state: Option<ClipState>,

    progress: Option<f32>,
}

#[derive(SimpleObject)]
pub struct QueueChanged {
    by: Option<String>,
}

#[derive(SimpleObject)]
pub struct PlaybackChanged {
    client: String,
    track_id: Option<i64>,
    /// Seconds in, as of when it was sent.
    position: f64,
    paused: bool,
}

#[derive(Union)]
#[graphql(name = "Event")]
pub enum ServerEvent {
    ConfigChanged(ConfigChanged),
    ScanStarted(ScanStarted),
    ScanFinished(ScanFinished),
    LibraryChanged(LibraryChanged),
    MetadataChanged(MetadataChanged),
    ListChanged(ListChanged),
    SeriesChanged(SeriesChanged),
    EpisodesImported(EpisodesImported),
    NotificationReceived(NotificationReceived),
    ClipChanged(ClipChanged),
    QueueChanged(QueueChanged),
    PlaybackChanged(PlaybackChanged),
}

fn translate(event: Event, me: i64, libraries: &[String]) -> Option<ServerEvent> {
    let list = |list| Some(ServerEvent::ListChanged(ListChanged { list }));
    let metadata = |title_id, status| Some(ServerEvent::MetadataChanged(MetadataChanged { title_id, status }));

    match event {
        Event::ConfigChanged => Some(ServerEvent::ConfigChanged(ConfigChanged { error: None })),
        Event::ConfigError { message } => Some(ServerEvent::ConfigChanged(ConfigChanged { error: Some(message) })),
        Event::ScanStarted { library } => Some(ServerEvent::ScanStarted(ScanStarted { library })),
        Event::ScanFinished { library, items, media, skipped } => {
            Some(ServerEvent::ScanFinished(ScanFinished { library, titles: items, videos: media, skipped }))
        },
        Event::LibraryChanged { library } => Some(ServerEvent::LibraryChanged(LibraryChanged { library })),
        Event::MetadataFetching { item_id } => metadata(item_id, MetadataStatus::Fetching),
        Event::MetadataUpdated { item_id } => metadata(item_id, MetadataStatus::Updated),
        Event::MetadataFailed { item_id } => metadata(item_id, MetadataStatus::Failed),
        Event::DownloadsChanged => list(ChangedList::Downloads),
        Event::RenamesChanged => list(ChangedList::RenameSuggestions),
        Event::RequestsChanged => list(ChangedList::Requests),
        Event::UsersChanged => list(ChangedList::Users),
        Event::SeriesChanged { series_id } => Some(ServerEvent::SeriesChanged(SeriesChanged { series_id })),
        Event::Imported { library, show, item_id, episodes } => libraries.contains(&library).then(|| {
            ServerEvent::EpisodesImported(EpisodesImported {
                library,
                show,
                title_id: item_id,
                episodes: episodes.into_iter().map(Into::into).collect(),
            })
        }),
        Event::Notified { user_id, notification } => {
            (user_id == me).then_some(ServerEvent::NotificationReceived(NotificationReceived { notification }))
        },
        Event::NotificationsChanged { user_id } => {
            (user_id == me).then_some(ServerEvent::ListChanged(ListChanged { list: ChangedList::Notifications }))
        },
        Event::AppearanceChanged { user_id } => user_id
            .is_none_or(|u| u == me)
            .then_some(ServerEvent::ListChanged(ListChanged { list: ChangedList::Appearance })),
        Event::QueueChanged { user_id, by } => {
            (user_id == me).then_some(ServerEvent::QueueChanged(QueueChanged { by }))
        },
        Event::PlaybackChanged { user_id, client, track_id, position, paused } => (user_id == me)
            .then_some(ServerEvent::PlaybackChanged(PlaybackChanged { client, track_id, position, paused })),
        Event::PlaylistsChanged => list(ChangedList::Playlists),
        Event::ClipChanged { clip_id, users, state, progress } => users
            .contains(&me)
            .then(|| ServerEvent::ClipChanged(ClipChanged { clip_id, state: ClipState::parse(&state), progress })),
    }
}

#[derive(Default)]
pub struct EventSubscription;

#[Subscription]
impl EventSubscription {
    async fn events(&self, ctx: &Context<'_>) -> async_graphql::Result<BoxStream<'static, ServerEvent>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let mut rx = state.events.subscribe();
        let libraries = user.libraries(state).await?;
        let me = user.id;

        Ok(async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if let Some(e) = translate(event, me, &libraries) {
                            yield e;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        }
        .boxed())
    }
}
