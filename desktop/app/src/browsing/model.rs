// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared view models for selections used by multiple browsing operations.

use serde::{Deserialize, Deserializer};

/// `null` read as the type's default, for fields the schema lets be null where the web treats
/// null as false.
fn null_as_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(deserializer: D) -> Result<T, D::Error> {
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Document {
    pub viewer: Option<Viewer>,
    pub libraries: Vec<Library>,
    pub server: Option<Features>,
    pub home: Option<Home>,
    pub library: Option<LibraryTitles>,
    pub title: Option<Title>,
    pub calendar: Vec<CalendarEntry>,
    pub wanted: Vec<Wanted>,
    pub requests: Vec<Request>,
    pub downloads: Vec<Download>,
    pub download_engine: Option<Engine>,
    pub notifications: Option<Inbox>,
    pub discover: Option<Discovery>,
    pub for_you: Option<Recommendations>,
    pub search: Option<Search>,
    pub music_search: Option<MusicSearch>,
    pub music_home: Option<MusicHome>,
    pub series: Option<Series>,
    pub settings: Option<Options>,
    pub aired_episodes: Option<i64>,
    pub titles: Vec<Card>,
    pub books: Vec<Book>,
    pub reading_now: Vec<Book>,
    pub new_books: Vec<BookShelf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Viewer {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub permissions: Permissions,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Permissions {
    pub request: bool,
    pub manage_shows: bool,
    pub manage_requests: bool,
    pub downloads: bool,
    pub edit_metadata: bool,
    pub watch_together: bool,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Features {
    pub downloads: bool,
    pub clips: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    pub name: String,
    pub kind: String,
    pub show_count: i64,
    pub movie_count: i64,
    pub album_count: i64,
    pub track_count: i64,
    pub book_count: i64,
    pub metadata_provider: Option<String>,
    pub managed: bool,
    pub profile: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Card {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub library: String,
    pub year: Option<i64>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub poster_tint: Option<String>,
    pub backdrop_tint: Option<String>,
    pub watched_count: i64,
    pub video_count: i64,
    pub progress: Option<f64>,
    pub fresh_count: i64,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Home {
    pub continue_watching: Vec<Continue>,
    pub recently_added: Vec<Shelf>,
    pub popular_here: Vec<Popular>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Continue {
    pub position: f64,
    pub up_next: bool,
    pub new_episode: bool,
    pub video: Video,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Shelf {
    pub library: String,
    pub titles: Vec<Card>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Popular {
    pub people: i64,
    pub title: Card,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LibraryTitles {
    pub titles: Vec<Card>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Title {
    #[serde(flatten)]
    pub card: Card,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub rating: Option<f64>,
    pub custom_poster: bool,
    pub custom_backdrop: bool,
    pub match_state: String,
    pub library_provider: Option<String>,
    pub seasons: Vec<Season>,
    pub movie: Option<Video>,
    pub next_up: Option<Next>,
    pub series: Option<Series>,
    pub similar: Option<Similar>,
    pub match_candidates: Option<Candidates>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Season {
    pub number: i64,
    pub name: String,
    pub title: Option<String>,
    pub poster: Option<String>,
    pub episodes: Vec<Video>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Video {
    pub id: i64,
    pub label: Option<String>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub still: Option<String>,
    pub custom_still: bool,
    pub duration: Option<f64>,
    pub position: Option<f64>,
    #[serde(deserialize_with = "null_as_default")]
    pub finished: bool,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub air_date: Option<String>,
    pub title: Card,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Next {
    pub resuming: bool,
    pub video: Video,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Similar {
    pub recommendations: Vec<Discover>,
    pub also_watched: Vec<Card>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Candidates {
    pub query: String,
    pub results: Vec<Discover>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Discover {
    pub category: String,
    pub provider: String,
    pub id: String,
    pub name: String,
    pub romaji: Option<String>,
    pub year: Option<i64>,
    pub poster: Option<String>,
    pub overview: Option<String>,
    pub library: String,
    pub title_id: Option<i64>,
    pub series_id: Option<i64>,
    pub monitor: Option<String>,
    pub request_state: Option<String>,
    pub because: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Discovery {
    pub library: Option<String>,
    pub results: Vec<Discover>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Recommendations {
    pub shelves: Vec<DiscoverShelf>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DiscoverShelf {
    pub key: String,
    pub name: String,
    pub results: Vec<Discover>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Search {
    pub titles: Vec<Card>,
    pub videos: Vec<Video>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MusicSearch {
    pub artists: Vec<Album>,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Album {
    pub id: i64,
    pub name: String,
    pub artist: String,
    pub year: Option<i64>,
    pub cover: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Track {
    pub album_id: Option<i64>,
    pub cover: Option<String>,
    pub id: i64,
    pub title: String,
    pub artist: String,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MusicHome {
    pub recently_played: Vec<Album>,
    pub recently_added: Vec<Album>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Book {
    pub id: i64,
    pub library: String,
    pub title: String,
    pub authors: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub year: Option<i64>,
    pub cover: Option<String>,
    pub reading: Option<Reading>,
    pub added_at: i64,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Reading {
    pub updated_at: i64,
    pub position: f64,
    pub progress: f64,
    pub finished: bool,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalendarEntry {
    pub series_id: i64,
    pub title: Option<Id>,
    pub library: String,
    pub show: String,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub backdrop_tint: Option<String>,
    pub monitor: String,
    pub season: i64,
    pub episode: i64,
    pub name: Option<String>,
    pub air_at: i64,
    pub state: String,
    pub video: Option<Id>,
    pub download: Option<Live>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Id {
    pub id: i64,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Wanted {
    pub series_id: i64,
    pub title: Option<Id>,
    pub show: String,
    pub season: i64,
    pub episode: i64,
    pub name: Option<String>,
    pub air_at: Option<i64>,
    pub aired: bool,
    pub state: String,
    pub attempts: i64,
    pub searched_at: Option<i64>,
    pub next_search: Option<i64>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Request {
    pub id: i64,
    pub user: Option<Person>,
    pub name: String,
    pub year: Option<i64>,
    pub poster: Option<String>,
    pub library: Option<String>,
    pub state: String,
    pub title: Option<Id>,
    pub note: Option<String>,
    pub created_at: i64,
    pub have: Option<i64>,
    pub aired: Option<i64>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Person {
    pub id: i64,
    pub username: String,
    pub avatar: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Download {
    pub id: i64,
    pub name: String,
    pub series_id: Option<i64>,
    pub series_name: Option<String>,
    pub title: Option<Id>,
    pub poster: Option<String>,
    pub source: Option<String>,
    pub size: Option<i64>,
    pub state: String,
    pub import_state: String,
    pub import_error: Option<String>,
    pub error: Option<String>,
    pub live: Option<Live>,
    pub save_path: Option<String>,
    pub finished_at: Option<i64>,
    pub episodes: Vec<EpisodeNumber>,
    pub category: String,
    pub seed_goal: SeedGoal,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Live {
    pub stage: String,
    pub paused: bool,
    pub progress: f64,
    pub download_rate: i64,
    pub upload_rate: i64,
    pub done: i64,
    pub uploaded: i64,
    pub ratio: f64,
    pub peers: i64,
    pub seeds: i64,
    pub eta: Option<i64>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Engine {
    pub version: String,
    pub download_path: String,
    pub download_rate: i64,
    pub upload_rate: i64,
    pub active: i64,
    pub kill_switch: Option<String>,
    pub listen_error: Option<String>,
    pub slow_hours: bool,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Inbox {
    pub items: Vec<Notice>,
    pub unread: i64,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Notice {
    pub id: i64,
    pub kind: String,
    pub priority: bool,
    pub title: String,
    pub body: Option<String>,
    pub image: Option<String>,
    pub link: Option<String>,
    pub actor: Option<Person>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub read_at: Option<i64>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Series {
    pub id: i64,
    pub monitor: String,
    pub status: Option<String>,
    pub name: String,
    pub library: String,
    pub managed: bool,
    pub profile: Option<String>,
    pub effective_profile: String,
    pub sources: Vec<String>,
    pub groups: Vec<String>,
    pub aliases: Vec<String>,
    pub numbering: String,
    pub naming: Option<String>,
    pub style: NamingStyle,
    pub next: Option<SeriesEpisode>,
    pub seeding: Option<Seeding>,
    pub counts: Counts,
    pub naming_preview: Option<NamingPreview>,
    pub episodes: Vec<SeriesEpisode>,
    pub releases: Vec<ReleaseCandidate>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SeriesEpisode {
    pub season: i64,
    pub episode: i64,
    pub name: Option<String>,
    pub air_at: Option<i64>,
    pub aired: bool,
    pub state: String,
    pub video: Option<Id>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReleaseCandidate {
    pub release: Release,
    pub verdict: Verdict,
    pub episodes: Vec<EpisodeNumber>,
    pub batch: bool,
    pub attributes: Attributes,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Release {
    pub title: String,
    pub source: String,
    pub link: String,
    pub info_hash: Option<String>,
    pub size: Option<i64>,
    pub seeders: Option<i64>,
    pub leechers: Option<i64>,
    pub published: Option<i64>,
    pub page: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Verdict {
    pub accepted: bool,
    pub score: i64,
    pub rejections: Vec<String>,
    pub warnings: Vec<String>,
    pub nonstandard: bool,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EpisodeNumber {
    pub season: i64,
    pub episode: i64,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Options {
    pub sources: Vec<Named>,
    pub libraries: Vec<Library>,
    pub profiles: Vec<Named>,
    pub automation: Automation,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Named {
    pub name: String,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Automation {
    pub default_monitor: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct NamingStyle {
    pub file: String,
    pub samples: i64,
    pub agreement: Option<f64>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Seeding {
    pub ratio: Option<f64>,
    pub time: Option<String>,
    pub idle: Option<String>,
    pub then: String,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Counts {
    pub have: i64,
    pub wanted: i64,
    pub missing: i64,
    pub grabbed: i64,
    pub total: i64,
    pub upcoming: i64,
    pub skipped: i64,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NamingPreview {
    pub samples: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SeedGoal {
    pub ratio: Option<f64>,
    pub seconds: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BookShelf {
    pub library: String,
    pub books: Vec<Book>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Attributes {
    pub group: Option<String>,
    pub resolution: Option<i64>,
    pub codec: Option<String>,
    pub source: Option<String>,
    pub dual_audio: bool,
    pub version: i64,
}
