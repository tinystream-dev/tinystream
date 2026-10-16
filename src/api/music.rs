// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use async_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rand::RngCore;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::media::tokio_stream_shim::ReceiverStream;
use super::schema::Ctx;
use crate::auth::User;
use crate::db::now;
use crate::error::{ApiError, ApiResult};
use crate::events::Event;
use crate::media::audio::{self, Format, Transcode};
use crate::music::catalog::{self, Album, AlbumFilter, AlbumOrder, Artist, Kind, Playlist, Track, UserData};
use crate::music::queue::{self, Playing, Repeat};
use crate::music::{art, lyrics};
use crate::state::AppState;
use crate::tint::Source;

/// Where a track's files and pictures come from: the music API, or a room's.
#[derive(Clone)]
pub(super) struct Base(pub Arc<str>);

impl Base {
    pub fn api() -> Self {
        Base("/api/music".into())
    }

    pub fn room(code: &str) -> Self {
        Base(format!("/api/listen/{code}").into())
    }
}

#[derive(SimpleObject, Clone)]
pub struct ArtistRef {
    id: i64,
    name: String,
}

#[derive(SimpleObject, Clone, Copy)]
pub struct Gains {
    /// dB to bring the track to ReplayGain's reference loudness.
    track_gain: Option<f64>,
    track_peak: Option<f64>,
    album_gain: Option<f64>,
    album_peak: Option<f64>,

    /// Neither tagged nor measured yet; `measureLoudness` sorts that out.
    pending: bool,
}

pub struct TrackView {
    t: Track,
    data: UserData,
    artists: Vec<(i64, String, String)>,
    base: Base,
}

#[Object(name = "Track")]
impl TrackView {
    async fn id(&self) -> i64 {
        self.t.id
    }

    async fn title(&self) -> &str {
        &self.t.title
    }

    /// The artists as tagged, for display.
    async fn artist(&self) -> &str {
        &self.t.artist
    }

    async fn artists(&self) -> Vec<ArtistRef> {
        self.artists.iter().filter(|a| a.2 == "artist").map(|a| ArtistRef { id: a.0, name: a.1.clone() }).collect()
    }

    async fn composers(&self) -> Vec<ArtistRef> {
        self.artists.iter().filter(|a| a.2 == "composer").map(|a| ArtistRef { id: a.0, name: a.1.clone() }).collect()
    }

    async fn album(&self) -> &str {
        &self.t.album
    }

    async fn album_id(&self) -> Option<i64> {
        self.t.album_id
    }

    async fn album_artist(&self) -> Option<&str> {
        self.t.album_artist.as_deref()
    }

    async fn library(&self) -> &str {
        &self.t.library
    }

    async fn disc(&self) -> Option<i64> {
        self.t.disc
    }

    async fn number(&self) -> Option<i64> {
        self.t.number
    }

    async fn year(&self) -> Option<i64> {
        self.t.year
    }

    async fn genres(&self) -> Vec<String> {
        self.t.genres()
    }

    async fn duration(&self) -> f64 {
        self.t.duration
    }

    /// flac, alac, mp3, aac, opus, vorbis, pcm, ape, wavpack, musepack…
    async fn codec(&self) -> &str {
        &self.t.codec
    }

    async fn suffix(&self) -> &str {
        &self.t.suffix
    }

    async fn lossless(&self) -> bool {
        self.t.lossless()
    }

    /// kbit/s.
    async fn bitrate(&self) -> Option<i64> {
        self.t.bitrate
    }

    async fn sample_rate(&self) -> Option<i64> {
        self.t.sample_rate
    }

    async fn bit_depth(&self) -> Option<i64> {
        self.t.bit_depth
    }

    async fn channels(&self) -> Option<i64> {
        self.t.channels
    }

    async fn size(&self) -> i64 {
        self.t.size
    }

    async fn mbid(&self) -> Option<&str> {
        self.t.mbid.as_deref()
    }

    /// The file as it is.
    async fn file(&self) -> String {
        format!("{}/tracks/{}/file", self.base.0, self.t.id)
    }

    /// The same as FLAC, made once and kept, for players that can't read the original.
    async fn flac(&self) -> String {
        format!("{}/tracks/{}/flac", self.base.0, self.t.id)
    }

    /// The same, converted; add `?format=flac|opus|mp3|aac&start=`.
    async fn stream(&self) -> String {
        format!("{}/tracks/{}/stream", self.base.0, self.t.id)
    }

    async fn cover(&self) -> Option<String> {
        self.t.has_cover().then(|| format!("{}/tracks/{}/cover", self.base.0, self.t.id))
    }

    /// The most vivid colour of the cover as "r g b", empty until it's been worked out.
    async fn cover_tint(&self, ctx: &Context<'_>) -> Option<String> {
        if !self.t.has_cover() {
            return None;
        }
        tint(ctx.state(), track_cover_file(ctx.state(), &self.t).await).await
    }

    async fn gains(&self) -> Gains {
        let g = self.t.gains();

        Gains {
            track_gain: g.track_gain,
            track_peak: g.track_peak,
            album_gain: g.album_gain,
            album_peak: g.album_peak,
            pending: self.t.unmeasured(),
        }
    }

    async fn starred(&self) -> bool {
        self.data.starred.is_some()
    }

    async fn rating(&self) -> Option<i64> {
        self.data.rating
    }

    async fn play_count(&self) -> i64 {
        self.data.play_count
    }

    async fn last_played(&self) -> Option<i64> {
        self.data.played
    }

    async fn added_at(&self) -> i64 {
        self.t.added_at
    }
}

pub(super) async fn track_views(
    state: &AppState,
    user_id: Option<i64>,
    tracks: Vec<Track>,
    base: &Base,
) -> ApiResult<Vec<TrackView>> {
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    let artists = catalog::track_artists(&state.db, &ids).await?;

    let data = match user_id {
        Some(u) => {
            let paths: Vec<String> = tracks.iter().map(|t| t.path.clone()).collect();
            catalog::user_data(&state.db, u, Kind::Track, &paths).await?
        },
        None => HashMap::new(),
    };

    Ok(tracks
        .into_iter()
        .map(|t| TrackView {
            data: data.get(&t.path).copied().unwrap_or_default(),
            artists: artists.get(&t.id).cloned().unwrap_or_default(),
            base: base.clone(),
            t,
        })
        .collect())
}

pub struct AlbumView {
    a: Album,
    data: UserData,
    plays: (i64, Option<i64>),
    artists: Vec<(i64, String)>,
    user_id: i64,
}

#[derive(SimpleObject)]
pub struct DiscTitle {
    disc: i64,
    title: String,
}

#[Object(name = "Album")]
impl AlbumView {
    async fn id(&self) -> i64 {
        self.a.id
    }

    async fn name(&self) -> &str {
        &self.a.title
    }

    async fn artist(&self) -> &str {
        &self.a.artist
    }

    async fn artists(&self) -> Vec<ArtistRef> {
        self.artists.iter().map(|(id, name)| ArtistRef { id: *id, name: name.clone() }).collect()
    }

    async fn library(&self) -> &str {
        &self.a.library
    }

    async fn year(&self) -> Option<i64> {
        self.a.year
    }

    async fn release_date(&self) -> Option<&str> {
        self.a.release_date.as_deref()
    }

    async fn original_date(&self) -> Option<&str> {
        self.a.original_date.as_deref()
    }

    async fn genres(&self) -> Vec<String> {
        self.a.genres()
    }

    async fn release_types(&self) -> Vec<String> {
        catalog::list(&self.a.release_types)
    }

    async fn labels(&self) -> Vec<String> {
        catalog::list(&self.a.labels)
    }

    async fn compilation(&self) -> bool {
        self.a.compilation
    }

    async fn disc_titles(&self) -> Vec<DiscTitle> {
        self.a.disc_titles().into_iter().map(|(disc, title)| DiscTitle { disc, title }).collect()
    }

    async fn track_count(&self) -> i64 {
        self.a.track_count
    }

    async fn duration(&self) -> f64 {
        self.a.duration
    }

    async fn size(&self) -> i64 {
        self.a.size
    }

    /// Missing when there's no picture in its folder or its files.
    async fn cover(&self) -> Option<String> {
        let found = self.a.cover_track.is_some()
            || crate::library::local_art(FsPath::new(&self.a.dir), crate::library::music::COVER_NAMES).is_some();

        found.then(|| format!("/api/music/albums/{}/cover", self.a.id))
    }

    /// The most vivid colour of the cover as "r g b", empty until it's been worked out.
    async fn cover_tint(&self, ctx: &Context<'_>) -> Option<String> {
        tint(ctx.state(), art::album_cover(ctx.state(), &self.a).await).await
    }

    async fn mbid(&self) -> Option<&str> {
        self.a.mbid.as_deref()
    }

    async fn added_at(&self) -> i64 {
        self.a.added_at
    }

    async fn starred(&self) -> bool {
        self.data.starred.is_some()
    }

    async fn rating(&self) -> Option<i64> {
        self.data.rating
    }

    async fn play_count(&self) -> i64 {
        self.plays.0
    }

    async fn last_played(&self) -> Option<i64> {
        self.plays.1
    }

    async fn tracks(&self, ctx: &Context<'_>) -> ApiResult<Vec<TrackView>> {
        let state = ctx.state();
        let tracks = catalog::album_tracks(&state.db, self.a.id).await?;
        track_views(state, Some(self.user_id), tracks, &Base::api()).await
    }
}

pub(super) async fn album_views(state: &AppState, user_id: i64, albums: Vec<Album>) -> ApiResult<Vec<AlbumView>> {
    let ids: Vec<i64> = albums.iter().map(|a| a.id).collect();
    let targets: Vec<String> = albums.iter().map(Album::target).collect();
    let mut data = catalog::user_data(&state.db, user_id, Kind::Album, &targets).await?;
    let plays = catalog::album_plays(&state.db, user_id, &ids).await?;
    let mut artists = catalog::album_artists(&state.db, &ids).await?;

    Ok(albums
        .into_iter()
        .map(|a| AlbumView {
            data: data.remove(&a.target()).unwrap_or_default(),
            plays: plays.get(&a.id).map(|(n, at)| (*n, Some(*at))).unwrap_or((0, None)),
            artists: artists.remove(&a.id).unwrap_or_default(),
            user_id,
            a,
        })
        .collect())
}

pub struct ArtistView {
    r: Artist,
    data: UserData,
    user_id: i64,
}

#[Object(name = "Artist")]
impl ArtistView {
    async fn id(&self) -> i64 {
        self.r.id
    }

    async fn name(&self) -> &str {
        &self.r.name
    }

    async fn sort_name(&self) -> &str {
        &self.r.sort_name
    }

    async fn library(&self) -> &str {
        &self.r.library
    }

    async fn mbid(&self) -> Option<&str> {
        self.r.mbid.as_deref()
    }

    async fn album_count(&self) -> i64 {
        self.r.album_count
    }

    async fn track_count(&self) -> i64 {
        self.r.track_count
    }

    async fn cover(&self) -> Option<String> {
        self.r.cover_album.map(|_| format!("/api/music/artists/{}/cover", self.r.id))
    }

    /// The most vivid colour of the cover as "r g b", empty until it's been worked out.
    async fn cover_tint(&self, ctx: &Context<'_>) -> ApiResult<Option<String>> {
        Ok(tint(ctx.state(), artist_cover_file(ctx.state(), &self.r).await?).await)
    }

    async fn starred(&self) -> bool {
        self.data.starred.is_some()
    }

    async fn rating(&self) -> Option<i64> {
        self.data.rating
    }

    async fn albums(&self, ctx: &Context<'_>) -> ApiResult<Vec<AlbumView>> {
        let state = ctx.state();
        let (own, _) = catalog::artist_albums(&state.db, self.r.id).await?;
        album_views(state, self.user_id, own).await
    }

    /// Albums by others that this artist is on.
    async fn appears_on(&self, ctx: &Context<'_>) -> ApiResult<Vec<AlbumView>> {
        let state = ctx.state();
        let (_, on) = catalog::artist_albums(&state.db, self.r.id).await?;
        album_views(state, self.user_id, on).await
    }

    async fn top_tracks(&self, ctx: &Context<'_>, #[graphql(default = 10)] count: i64) -> ApiResult<Vec<TrackView>> {
        let state = ctx.state();
        let libs = vec![self.r.library.clone()];
        let tracks = catalog::top_tracks(&state.db, &libs, self.r.id, count.clamp(1, 100)).await?;
        track_views(state, Some(self.user_id), tracks, &Base::api()).await
    }
}

pub(super) async fn artist_views(state: &AppState, user_id: i64, artists: Vec<Artist>) -> ApiResult<Vec<ArtistView>> {
    let targets: Vec<String> = artists.iter().map(Artist::target).collect();
    let mut data = catalog::user_data(&state.db, user_id, Kind::Artist, &targets).await?;

    Ok(artists
        .into_iter()
        .map(|r| ArtistView { data: data.remove(&r.target()).unwrap_or_default(), user_id, r })
        .collect())
}

pub struct PlaylistView {
    p: Playlist,
    user_id: i64,
}

#[derive(SimpleObject)]
pub struct PlaylistOwner {
    id: i64,
    username: String,
}

#[Object(name = "Playlist")]
impl PlaylistView {
    async fn id(&self) -> i64 {
        self.p.id
    }

    async fn name(&self) -> &str {
        &self.p.name
    }

    async fn comment(&self) -> Option<&str> {
        self.p.comment.as_deref()
    }

    async fn public(&self) -> bool {
        self.p.public
    }

    async fn owner(&self) -> PlaylistOwner {
        PlaylistOwner { id: self.p.owner_id, username: self.p.owner.clone() }
    }

    async fn mine(&self) -> bool {
        self.p.owner_id == self.user_id
    }

    async fn track_count(&self) -> i64 {
        self.p.track_count
    }

    async fn duration(&self) -> f64 {
        self.p.duration
    }

    async fn updated_at(&self) -> i64 {
        self.p.updated_at
    }

    /// Up to four album covers from it, for a mosaic.
    async fn covers(&self, ctx: &Context<'_>) -> ApiResult<Vec<String>> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT t.album_id FROM playlist_tracks pt JOIN tracks t ON t.path = pt.track_path
             WHERE pt.playlist_id = ? AND t.album_id IS NOT NULL GROUP BY t.album_id ORDER BY MIN(pt.position) LIMIT 4",
        )
        .bind(self.p.id)
        .fetch_all(&ctx.state().db)
        .await?;

        Ok(ids.into_iter().map(|id| format!("/api/music/albums/{id}/cover")).collect())
    }

    async fn tracks(&self, ctx: &Context<'_>) -> ApiResult<Vec<TrackView>> {
        let state = ctx.state();
        let user = ctx.user()?;
        let paths = catalog::playlist_paths(&state.db, self.p.id).await?;
        let tracks = catalog::tracks_by_path(&state.db, &crate::music::libraries(state, user), &paths).await?;
        track_views(state, Some(user.id), tracks, &Base::api()).await
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumSort {
    Newest,
    Name,
    Artist,
    Year,
    Random,
    Recent,
    Frequent,
    Starred,
    Rated,
}

impl From<AlbumSort> for AlbumOrder {
    fn from(s: AlbumSort) -> Self {
        match s {
            AlbumSort::Newest => AlbumOrder::Newest,
            AlbumSort::Name => AlbumOrder::Name,
            AlbumSort::Artist => AlbumOrder::Artist,
            AlbumSort::Year => AlbumOrder::Year,
            AlbumSort::Random => AlbumOrder::Random,
            AlbumSort::Recent => AlbumOrder::Recent,
            AlbumSort::Frequent => AlbumOrder::Frequent,
            AlbumSort::Starred => AlbumOrder::Starred,
            AlbumSort::Rated => AlbumOrder::Rated,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicKind {
    Track,
    Album,
    Artist,
}

#[derive(SimpleObject)]
pub struct Genre {
    name: String,
    track_count: i64,
    album_count: i64,
}

#[derive(SimpleObject)]
pub struct MusicSearch {
    artists: Vec<ArtistView>,
    albums: Vec<AlbumView>,
    tracks: Vec<TrackView>,
}

#[derive(SimpleObject)]
pub struct LyricLine {
    /// Milliseconds in; missing for plain lyrics.
    start: Option<i64>,
    text: String,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsSource {
    File,
    Embedded,
    Online,
}

#[derive(SimpleObject)]
pub struct Lyrics {
    synced: bool,
    source: LyricsSource,
    lines: Vec<LyricLine>,
}

impl From<lyrics::Lyrics> for Lyrics {
    fn from(l: lyrics::Lyrics) -> Self {
        Lyrics {
            synced: l.synced,
            source: match l.source {
                lyrics::Source::File => LyricsSource::File,
                lyrics::Source::Embedded => LyricsSource::Embedded,
                lyrics::Source::Online => LyricsSource::Online,
            },
            lines: l.lines.into_iter().map(|l| LyricLine { start: l.start, text: l.text }).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct PlayQueue {
    tracks: Vec<TrackView>,
    current: i64,
    position: f64,
    shuffled: bool,
    repeat: Repeat,
    changed_by: Option<String>,
    updated_at: i64,
}

#[derive(SimpleObject)]
pub struct MusicHome {
    recently_added: Vec<AlbumView>,
    recently_played: Vec<AlbumView>,
    rediscover: Vec<AlbumView>,
}

#[derive(SimpleObject)]
pub struct AppPassword {
    id: i64,
    name: String,
    created_at: i64,
    last_used: Option<i64>,
    client: Option<String>,
}

#[derive(SimpleObject)]
pub struct NewAppPassword {
    password: AppPassword,

    /// Shown this once.
    secret: String,
}

pub(super) fn libraries(state: &AppState, user: &User, library: Option<&str>) -> ApiResult<Vec<String>> {
    let all = crate::music::libraries(state, user);

    match library {
        Some(l) if all.iter().any(|x| x == l) => Ok(vec![l.to_string()]),
        Some(_) => Err(ApiError::not_found("library")),
        None => Ok(all),
    }
}

async fn visible_track(state: &AppState, user: &User, id: i64) -> ApiResult<Track> {
    let t = catalog::track(&state.db, id).await?.ok_or_else(|| ApiError::not_found("track"))?;

    if !crate::music::libraries(state, user).contains(&t.library) {
        return Err(ApiError::not_found("track"));
    }

    Ok(t)
}

async fn visible_album(state: &AppState, user: &User, id: i64) -> ApiResult<Album> {
    let a = catalog::album(&state.db, id).await?.ok_or_else(|| ApiError::not_found("album"))?;

    if !crate::music::libraries(state, user).contains(&a.library) {
        return Err(ApiError::not_found("album"));
    }

    Ok(a)
}

async fn visible_artist(state: &AppState, user: &User, id: i64) -> ApiResult<Artist> {
    let r = catalog::artist(&state.db, id).await?.ok_or_else(|| ApiError::not_found("artist"))?;

    if !crate::music::libraries(state, user).contains(&r.library) {
        return Err(ApiError::not_found("artist"));
    }

    Ok(r)
}

pub(super) async fn target(state: &AppState, user: &User, kind: MusicKind, id: i64) -> ApiResult<(Kind, String)> {
    Ok(match kind {
        MusicKind::Track => (Kind::Track, visible_track(state, user, id).await?.path),
        MusicKind::Album => (Kind::Album, visible_album(state, user, id).await?.target()),
        MusicKind::Artist => (Kind::Artist, visible_artist(state, user, id).await?.target()),
    })
}

pub(super) async fn play_queue(state: &AppState, user: &User) -> ApiResult<PlayQueue> {
    let q = queue::load(&state.db, user.id).await?;
    let libs = crate::music::libraries(state, user);
    let found = catalog::tracks(&state.db, &libs, &q.tracks).await?;
    let by_id: HashMap<i64, Track> = found.into_iter().map(|t| (t.id, t)).collect();

    let mut current = 0;
    let mut kept = Vec::new();

    for (i, id) in q.tracks.iter().enumerate() {
        if let Some(t) = by_id.get(id) {
            if i == q.current {
                current = kept.len();
            }

            kept.push(t.clone());
        }
    }

    Ok(PlayQueue {
        tracks: track_views(state, Some(user.id), kept, &Base::api()).await?,
        current: current as i64,
        position: q.position,
        shuffled: q.shuffled,
        repeat: q.repeat,
        changed_by: q.changed_by,
        updated_at: q.updated_at,
    })
}

pub fn new_secret() -> String {
    let mut bytes = [0u8; 18];
    rand::rng().fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

#[derive(Default)]
pub struct MusicQuery;

#[Object]
impl MusicQuery {
    async fn albums(
        &self,
        ctx: &Context<'_>,
        library: Option<String>,
        #[graphql(default_with = "AlbumSort::Name")] sort: AlbumSort,
        genre: Option<String>,
        #[graphql(default)] offset: i64,
        #[graphql(default = 500)] limit: i64,
    ) -> ApiResult<Vec<AlbumView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let libs = libraries(state, user, library.as_deref())?;
        let filter = AlbumFilter { genre, ..Default::default() };

        let albums =
            catalog::albums(&state.db, &libs, user.id, sort.into(), &filter, offset.max(0), limit.clamp(1, 5000))
                .await?;

        album_views(state, user.id, albums).await
    }

    async fn album(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<AlbumView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let Ok(a) = visible_album(state, user, id).await else { return Ok(None) };
        Ok(album_views(state, user.id, vec![a]).await?.pop())
    }

    /// Artists with albums of their own.
    async fn artists(&self, ctx: &Context<'_>, library: Option<String>) -> ApiResult<Vec<ArtistView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let libs = libraries(state, user, library.as_deref())?;
        let artists = catalog::album_artists_index(&state.db, &libs).await?;
        artist_views(state, user.id, artists).await
    }

    async fn artist(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<ArtistView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let Ok(r) = visible_artist(state, user, id).await else { return Ok(None) };
        Ok(artist_views(state, user.id, vec![r]).await?.pop())
    }

    async fn songs(
        &self,
        ctx: &Context<'_>,
        library: Option<String>,
        #[graphql(default)] query: String,
        #[graphql(default)] offset: i64,
        #[graphql(default = 500)] limit: i64,
    ) -> ApiResult<Vec<TrackView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let libs = libraries(state, user, library.as_deref())?;
        let tracks = catalog::search_tracks(&state.db, &libs, &query, offset.max(0), limit.clamp(1, 5000)).await?;
        track_views(state, Some(user.id), tracks, &Base::api()).await
    }

    async fn track(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<TrackView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let Ok(t) = visible_track(state, user, id).await else { return Ok(None) };
        Ok(track_views(state, Some(user.id), vec![t], &Base::api()).await?.pop())
    }

    /// The tracks among `ids` that still exist and are visible, in the given order.
    async fn tracks(&self, ctx: &Context<'_>, ids: Vec<i64>) -> ApiResult<Vec<TrackView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let tracks = catalog::tracks(&state.db, &crate::music::libraries(state, user), &ids).await?;
        track_views(state, Some(user.id), tracks, &Base::api()).await
    }

    async fn genres(&self, ctx: &Context<'_>, library: Option<String>) -> ApiResult<Vec<Genre>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let libs = libraries(state, user, library.as_deref())?;

        Ok(catalog::genres(&state.db, &libs)
            .await?
            .into_iter()
            .map(|(name, track_count, album_count)| Genre { name, track_count, album_count })
            .collect())
    }

    async fn music_search(
        &self,
        ctx: &Context<'_>,
        query: String,
        #[graphql(default = 8)] limit: i64,
    ) -> ApiResult<MusicSearch> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let libs = crate::music::libraries(state, user);
        let limit = limit.clamp(1, 100);

        if query.trim().is_empty() {
            return Ok(MusicSearch { artists: Vec::new(), albums: Vec::new(), tracks: Vec::new() });
        }

        let artists = catalog::search_artists(&state.db, &libs, &query, 0, limit).await?;
        let albums = catalog::search_albums(&state.db, &libs, &query, 0, limit).await?;
        let tracks = catalog::search_tracks(&state.db, &libs, &query, 0, limit).await?;

        Ok(MusicSearch {
            artists: artist_views(state, user.id, artists).await?,
            albums: album_views(state, user.id, albums).await?,
            tracks: track_views(state, Some(user.id), tracks, &Base::api()).await?,
        })
    }

    async fn lyrics(&self, ctx: &Context<'_>, track_id: i64) -> ApiResult<Option<Lyrics>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let t = visible_track(state, user, track_id).await?;
        Ok(lyrics::get(state, &t).await?.map(Into::into))
    }

    async fn play_queue(&self, ctx: &Context<'_>) -> ApiResult<PlayQueue> {
        play_queue(ctx.state(), ctx.user()?).await
    }

    async fn playlists(&self, ctx: &Context<'_>) -> ApiResult<Vec<PlaylistView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let lists = catalog::playlists(&state.db, user.id).await?;
        Ok(lists.into_iter().map(|p| PlaylistView { p, user_id: user.id }).collect())
    }

    async fn playlist(&self, ctx: &Context<'_>, id: i64) -> ApiResult<Option<PlaylistView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        Ok(catalog::playlist(&state.db, user.id, id).await?.map(|p| PlaylistView { p, user_id: user.id }))
    }

    async fn music_home(&self, ctx: &Context<'_>) -> ApiResult<MusicHome> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let libs = crate::music::libraries(state, user);
        let none = AlbumFilter::default();
        let added = catalog::albums(&state.db, &libs, user.id, AlbumOrder::Newest, &none, 0, 24).await?;
        let played = catalog::recently_played_albums(&state.db, &libs, user.id, 24).await?;
        let random = catalog::albums(&state.db, &libs, user.id, AlbumOrder::Random, &none, 0, 12).await?;

        Ok(MusicHome {
            recently_added: album_views(state, user.id, added).await?,
            recently_played: album_views(state, user.id, played).await?,
            rediscover: album_views(state, user.id, random).await?,
        })
    }

    /// More like a track, from what's here: for radio when the queue runs out.
    async fn similar_tracks(
        &self,
        ctx: &Context<'_>,
        track_id: i64,
        #[graphql(default = 25)] count: i64,
        #[graphql(default)] exclude: Vec<i64>,
    ) -> ApiResult<Vec<TrackView>> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let t = visible_track(state, user, track_id).await?;
        let libs = crate::music::libraries(state, user);

        let artists: Vec<i64> = catalog::track_artists(&state.db, &[t.id])
            .await?
            .remove(&t.id)
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.0)
            .collect();

        let mut exclude = exclude;
        exclude.push(t.id);

        let tracks =
            catalog::similar_tracks(&state.db, &libs, &artists, &t.genres(), &exclude, count.clamp(1, 200)).await?;

        track_views(state, Some(user.id), tracks, &Base::api()).await
    }

    async fn app_passwords(&self, ctx: &Context<'_>) -> ApiResult<Vec<AppPassword>> {
        let (state, user) = (ctx.state(), ctx.user()?);

        let rows: Vec<(i64, String, i64, Option<i64>, Option<String>)> = sqlx::query_as(
            "SELECT id, name, created_at, last_used, client FROM app_passwords WHERE user_id = ? ORDER BY created_at DESC",
        )
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, created_at, last_used, client)| AppPassword { id, name, created_at, last_used, client })
            .collect())
    }
}

#[derive(InputObject)]
pub struct QueueInput {
    tracks: Vec<i64>,
    current: i64,
    position: f64,
    #[graphql(default)]
    shuffled: bool,
    #[graphql(default_with = "Repeat::Off")]
    repeat: Repeat,
}

#[derive(InputObject)]
pub struct PlaylistInput {
    name: Option<String>,
    comment: Option<String>,
    public: Option<bool>,

    /// Replaces what's in it.
    tracks: Option<Vec<i64>>,
}

async fn own_playlist(state: &AppState, user: &User, id: i64) -> ApiResult<Playlist> {
    let p = catalog::playlist(&state.db, user.id, id).await?.ok_or_else(|| ApiError::not_found("playlist"))?;

    if p.owner_id != user.id {
        return Err(ApiError::forbidden());
    }

    Ok(p)
}

async fn paths_of(state: &AppState, user: &User, ids: &[i64]) -> ApiResult<Vec<String>> {
    Ok(catalog::tracks(&state.db, &crate::music::libraries(state, user), ids)
        .await?
        .into_iter()
        .map(|t| t.path)
        .collect())
}

#[derive(Default)]
pub struct MusicMutation;

#[Object]
impl MusicMutation {
    async fn star(&self, ctx: &Context<'_>, kind: MusicKind, id: i64, starred: bool) -> ApiResult<bool> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let (kind, target) = target(state, user, kind, id).await?;
        catalog::set_star(&state.db, user.id, kind, &target, starred).await?;
        Ok(starred)
    }

    /// 1 to 5 stars; 0 takes the rating away.
    async fn rate(&self, ctx: &Context<'_>, kind: MusicKind, id: i64, rating: i64) -> ApiResult<bool> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let (kind, target) = target(state, user, kind, id).await?;
        catalog::set_rating(&state.db, user.id, kind, &target, rating).await?;
        Ok(true)
    }

    /// Someone listened to (most of) a track.
    async fn played(&self, ctx: &Context<'_>, track_id: i64) -> ApiResult<bool> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let t = visible_track(state, user, track_id).await?;
        catalog::record_play(&state.db, user.id, &t.path, now()).await?;
        Ok(true)
    }

    /// What's playing in tinystream's own player right now (the web's, or `client`, the app's), so
    /// other apps see it; no track means it stopped.
    async fn now_playing(
        &self,
        ctx: &Context<'_>,
        track_id: Option<i64>,
        #[graphql(default)] position: f64,
        #[graphql(default)] paused: bool,
        client: Option<String>,
    ) -> ApiResult<bool> {
        let (state, user) = (ctx.state(), ctx.user()?);

        let playing = match track_id {
            Some(id) => {
                let t = visible_track(state, user, id).await?;

                Some(Playing {
                    track_id: id,
                    client: client.unwrap_or_else(|| "tinystream".into()),
                    since: now(),
                    position,
                    duration: t.duration,
                    paused,
                })
            },
            None => None,
        };

        state.music.playing.set(user.id, playing);
        Ok(true)
    }

    /// Saves the queue of tinystream's own player: the web's, or `client`, the app's.
    async fn save_play_queue(
        &self,
        ctx: &Context<'_>,
        input: QueueInput,
        client: Option<String>,
    ) -> ApiResult<PlayQueue> {
        let (state, user) = (ctx.state(), ctx.user()?);

        let q = queue::Queue {
            tracks: input.tracks,
            current: input.current.max(0) as usize,
            position: input.position,
            shuffled: input.shuffled,
            repeat: input.repeat,
            changed_by: Some(client.unwrap_or_else(|| "tinystream".into())),
            updated_at: 0,
        };

        queue::save(state, user.id, q).await?;
        play_queue(state, user).await
    }

    /// Measures a track's loudness now, if that's still to be done, for whoever's about to hear it.
    async fn measure_loudness(&self, ctx: &Context<'_>, track_id: i64) -> ApiResult<TrackView> {
        let (state, user) = (ctx.state(), ctx.user()?);
        visible_track(state, user, track_id).await?;
        crate::music::loudness::ensure(state, track_id).await;
        let t = visible_track(state, user, track_id).await?;
        Ok(track_views(state, Some(user.id), vec![t], &Base::api()).await?.remove(0))
    }

    async fn create_playlist(
        &self,
        ctx: &Context<'_>,
        name: String,
        #[graphql(default)] tracks: Vec<i64>,
    ) -> ApiResult<PlaylistView> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let name = name.trim();

        if name.is_empty() || name.chars().count() > 200 {
            return Err(ApiError::bad_request("give the playlist a name"));
        }

        let paths = paths_of(state, user, &tracks).await?;
        let id = catalog::create_playlist(&state.db, user.id, name, &paths).await?;
        state.events.send(Event::PlaylistsChanged);
        let p = catalog::playlist(&state.db, user.id, id).await?.ok_or_else(|| ApiError::not_found("playlist"))?;
        Ok(PlaylistView { p, user_id: user.id })
    }

    async fn update_playlist(&self, ctx: &Context<'_>, id: i64, input: PlaylistInput) -> ApiResult<PlaylistView> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let p = own_playlist(state, user, id).await?;
        let name = input.name.as_deref().map(str::trim).unwrap_or(&p.name).to_string();

        if name.is_empty() {
            return Err(ApiError::bad_request("give the playlist a name"));
        }

        let mut tx = state.db.begin().await?;

        sqlx::query("UPDATE playlists SET name = ?, comment = ?, public = ?, updated_at = ? WHERE id = ?")
            .bind(&name)
            .bind(input.comment.or(p.comment).filter(|c| !c.trim().is_empty()))
            .bind(input.public.unwrap_or(p.public))
            .bind(now())
            .bind(id)
            .execute(&mut *tx)
            .await?;

        if let Some(ids) = input.tracks {
            let paths = paths_of(state, user, &ids).await?;
            catalog::set_playlist_paths(&mut tx, id, &paths).await?;
        }

        tx.commit().await?;
        state.events.send(Event::PlaylistsChanged);
        let p = catalog::playlist(&state.db, user.id, id).await?.ok_or_else(|| ApiError::not_found("playlist"))?;
        Ok(PlaylistView { p, user_id: user.id })
    }

    async fn add_to_playlist(&self, ctx: &Context<'_>, id: i64, tracks: Vec<i64>) -> ApiResult<PlaylistView> {
        let (state, user) = (ctx.state(), ctx.user()?);
        own_playlist(state, user, id).await?;
        let mut paths = catalog::playlist_paths(&state.db, id).await?;
        paths.extend(paths_of(state, user, &tracks).await?);
        let mut tx = state.db.begin().await?;
        catalog::set_playlist_paths(&mut tx, id, &paths).await?;
        tx.commit().await?;
        state.events.send(Event::PlaylistsChanged);
        let p = catalog::playlist(&state.db, user.id, id).await?.ok_or_else(|| ApiError::not_found("playlist"))?;
        Ok(PlaylistView { p, user_id: user.id })
    }

    async fn delete_playlist(&self, ctx: &Context<'_>, id: i64) -> ApiResult<bool> {
        let (state, user) = (ctx.state(), ctx.user()?);
        own_playlist(state, user, id).await?;
        sqlx::query("DELETE FROM playlists WHERE id = ?").bind(id).execute(&state.db).await?;
        state.events.send(Event::PlaylistsChanged);
        Ok(true)
    }

    async fn create_app_password(&self, ctx: &Context<'_>, name: String) -> ApiResult<NewAppPassword> {
        let (state, user) = (ctx.state(), ctx.user()?);
        let name = name.trim();

        if name.is_empty() || name.chars().count() > 80 {
            return Err(ApiError::bad_request("name it after the app or device it's for"));
        }

        let secret = new_secret();
        let created_at = now();

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO app_passwords (user_id, name, secret, created_at) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(user.id)
        .bind(name)
        .bind(&secret)
        .bind(created_at)
        .fetch_one(&state.db)
        .await?;

        tracing::info!("{} made an app password for {name:?}", user.username);

        Ok(NewAppPassword {
            password: AppPassword { id, name: name.to_string(), created_at, last_used: None, client: None },
            secret,
        })
    }

    async fn delete_app_password(&self, ctx: &Context<'_>, id: i64) -> ApiResult<bool> {
        let (state, user) = (ctx.state(), ctx.user()?);

        let n = sqlx::query("DELETE FROM app_passwords WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user.id)
            .execute(&state.db)
            .await?
            .rows_affected();

        Ok(n > 0)
    }
}

pub(super) fn file_response(res: Response, track: &Track) -> Response {
    let mut res = res;
    let h = res.headers_mut();

    if let Ok(v) = HeaderValue::from_str(track.content_type()) {
        h.insert(header::CONTENT_TYPE, v);
    }

    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));
    res
}

/// The file itself, in whatever range was asked for.
pub(super) async fn serve_track(track: &Track, req: Request) -> ApiResult<Response> {
    let Ok(res) = ServeFile::new(&track.path).oneshot(req).await;

    if res.status() == StatusCode::NOT_FOUND {
        return Err(ApiError::not_found("file"));
    }

    Ok(file_response(res.map(Body::new), track))
}

#[derive(Deserialize)]
pub(super) struct StreamQuery {
    pub format: Option<String>,
    #[serde(alias = "maxBitRate")]
    pub bitrate: Option<u32>,
    #[serde(default, alias = "timeOffset")]
    pub start: f64,
}

pub(super) fn transcoded(
    state: &AppState,
    who: &str,
    track: &Track,
    format: Format,
    bitrate: Option<u32>,
    start: f64,
) -> Response {
    tracing::info!(
        "{who} is playing {} as {}{}",
        FsPath::new(&track.path).file_name().unwrap_or_default().to_string_lossy(),
        format.suffix(),
        bitrate.filter(|_| !format.lossless()).map(|b| format!(" at {b} kbit/s")).unwrap_or_default(),
    );

    let rx = audio::spawn(
        Transcode { path: PathBuf::from(&track.path), start: start.max(0.0), format, bitrate },
        state.media.busy.hold(),
    );

    let mut res = Body::from_stream(ReceiverStream(rx)).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(format.mime()));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert("x-accel-buffering", HeaderValue::from_static("no"));
    res
}

pub async fn file(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    req: Request,
) -> ApiResult<Response> {
    let t = visible_track(&state, &user, id).await?;
    serve_track(&t, req).await
}

/// A lossless copy of a track as FLAC, converted the first time it's asked for.
pub(super) async fn flac_copy(state: &AppState, track: &Track) -> ApiResult<Track> {
    use sha2::{Digest, Sha256};
    let key = hex::encode(&Sha256::digest(format!("{}\0{}\0{}", track.path, track.size, track.mtime))[..12]);
    let file = state.paths.cache_dir().join("music").join(format!("{key}.flac"));
    let cell = state.music.copies.lock().unwrap().entry(file.clone()).or_default().clone();

    let made = cell
        .get_or_init(|| async {
            if file.exists() {
                return Ok(());
            }

            let rx = audio::spawn(
                Transcode { path: PathBuf::from(&track.path), start: 0.0, format: Format::Flac, bitrate: None },
                state.media.busy.hold(),
            );

            let tmp = file.with_extension("part");

            let write = async {
                tokio::fs::create_dir_all(file.parent().unwrap()).await?;
                let mut out = tokio::fs::File::create(&tmp).await?;
                let mut rx = rx;

                while let Some(chunk) = rx.recv().await {
                    tokio::io::AsyncWriteExt::write_all(&mut out, &chunk?).await?;
                }

                tokio::io::AsyncWriteExt::flush(&mut out).await?;
                tokio::fs::rename(&tmp, &file).await
            };

            write.await.map_err(|e: std::io::Error| {
                let _ = std::fs::remove_file(&tmp);
                format!("{e}")
            })
        })
        .await
        .clone();

    state.music.copies.lock().unwrap().remove(&file);
    made.map_err(|e| ApiError::from(anyhow::anyhow!("can't convert {}: {e}", track.path)))?;
    let size = tokio::fs::metadata(&file).await.map(|m| m.len() as i64).unwrap_or(0);
    Ok(Track { path: file.to_string_lossy().to_string(), size, suffix: "flac".into(), ..track.clone() })
}

pub async fn flac(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    req: Request,
) -> ApiResult<Response> {
    let t = visible_track(&state, &user, id).await?;
    let copy = flac_copy(&state, &t).await?;
    serve_track(&copy, req).await
}

pub async fn stream(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<StreamQuery>,
) -> ApiResult<Response> {
    let t = visible_track(&state, &user, id).await?;
    let format = q.format.as_deref().and_then(Format::parse).unwrap_or(Format::Flac);
    Ok(transcoded(&state, &user.username, &t, format, q.bitrate, q.start))
}

#[derive(Deserialize)]
pub(super) struct CoverQuery {
    pub size: Option<u32>,
}

pub(super) async fn image(state: &AppState, file: Option<PathBuf>, size: Option<u32>) -> ApiResult<Response> {
    let file = file.ok_or_else(|| ApiError::not_found("cover"))?;
    let file = art::scaled(state, file, size).await;
    let data = tokio::fs::read(&file).await.map_err(|_| ApiError::not_found("cover"))?;
    let mime = mime_guess::from_path(&file).first_or_octet_stream().to_string();
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=86400".into())], data).into_response())
}

async fn tint(state: &Arc<AppState>, file: Option<PathBuf>) -> Option<String> {
    state.tints.get(state, Source::File(file?)).await
}

pub(super) async fn track_cover_file(state: &AppState, t: &Track) -> Option<PathBuf> {
    let album = match t.album_id {
        Some(id) => catalog::album(&state.db, id).await.ok().flatten(),
        None => None,
    };

    art::track_cover(state, FsPath::new(&t.path), t.embedded_art, album.as_ref()).await
}

async fn artist_cover_file(state: &AppState, r: &Artist) -> ApiResult<Option<PathBuf>> {
    let album = match r.cover_album {
        Some(a) => catalog::album(&state.db, a).await?,
        None => None,
    };

    Ok(match album {
        Some(a) => art::album_cover(state, &a).await,
        None => None,
    })
}

pub async fn track_cover(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<CoverQuery>,
) -> ApiResult<Response> {
    let t = visible_track(&state, &user, id).await?;
    image(&state, track_cover_file(&state, &t).await, q.size).await
}

pub async fn album_cover(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<CoverQuery>,
) -> ApiResult<Response> {
    let a = visible_album(&state, &user, id).await?;
    image(&state, art::album_cover(&state, &a).await, q.size).await
}

pub async fn artist_cover(
    State(state): State<Arc<AppState>>,
    user: User,
    Path(id): Path<i64>,
    Query(q): Query<CoverQuery>,
) -> ApiResult<Response> {
    let r = visible_artist(&state, &user, id).await?;
    image(&state, artist_cover_file(&state, &r).await?, q.size).await
}
