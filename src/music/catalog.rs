// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use super::gain_for;
use crate::db::now;

pub const TRACK_COLUMNS: &str = "t.id, t.library, t.album_id, t.path, t.size, t.mtime, t.title, t.artist, t.artists,
    t.album, a.artist AS album_artist, t.disc, t.number, t.year, t.genres, t.mbid, t.bpm, t.comment,
    t.duration, t.codec, t.suffix, t.bitrate, t.sample_rate, t.bit_depth, t.channels, t.rg_track_gain,
    t.rg_track_peak, t.rg_album_gain, t.rg_album_peak, t.loudness, t.peak, a.loudness AS album_loudness,
    t.embedded_art, t.lyrics IS NOT NULL AS has_lyrics, t.analyzed, t.added_at, a.dir AS album_dir,
    a.cover_track IS NOT NULL AS album_art";

const TRACKS_FROM: &str = "FROM tracks t LEFT JOIN albums a ON a.id = t.album_id";

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Track {
    pub id: i64,
    pub library: String,
    pub album_id: Option<i64>,
    pub path: String,
    pub size: i64,
    pub mtime: i64,
    pub title: String,
    pub artist: String,
    pub artists: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub disc: Option<i64>,
    pub number: Option<i64>,
    pub year: Option<i64>,
    pub genres: String,
    pub mbid: Option<String>,
    pub bpm: Option<i64>,
    pub comment: Option<String>,
    pub duration: f64,
    pub codec: String,
    pub suffix: String,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub rg_track_gain: Option<f64>,
    pub rg_track_peak: Option<f64>,
    pub rg_album_gain: Option<f64>,
    pub rg_album_peak: Option<f64>,
    pub loudness: Option<f64>,
    pub peak: Option<f64>,
    pub album_loudness: Option<f64>,
    pub embedded_art: bool,
    pub has_lyrics: bool,
    pub analyzed: i64,
    pub added_at: i64,
    pub album_dir: Option<String>,
    pub album_art: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Gains {
    pub track_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
}

pub fn list(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

impl Track {
    pub fn artists(&self) -> Vec<String> {
        let l = list(&self.artists);
        if l.is_empty() && !self.artist.is_empty() { vec![self.artist.clone()] } else { l }
    }

    pub fn genres(&self) -> Vec<String> {
        list(&self.genres)
    }

    /// What the tags say, or what was measured here when they don't.
    pub fn gains(&self) -> Gains {
        let track_gain = self.rg_track_gain.or_else(|| gain_for(self.loudness));

        Gains {
            track_gain,
            track_peak: self.rg_track_peak.or(self.peak),
            album_gain: self.rg_album_gain.or_else(|| gain_for(self.album_loudness)),
            album_peak: self.rg_album_peak,
        }
    }

    /// Whether there's a picture for it anywhere: in it, in its album's files, or its album's
    /// folder.
    pub fn has_cover(&self) -> bool {
        self.embedded_art
            || self.album_art == Some(true)
            || self.album_dir.as_deref().is_some_and(|d| {
                crate::library::local_art(std::path::Path::new(d), crate::library::music::COVER_NAMES).is_some()
            })
    }

    /// Nothing to go on yet: no ReplayGain in its tags, and not measured.
    pub fn unmeasured(&self) -> bool {
        self.rg_track_gain.is_none() && self.analyzed == 0
    }

    pub fn lossless(&self) -> bool {
        matches!(self.codec.as_str(), "flac" | "alac" | "pcm" | "ape" | "wavpack")
    }

    /// Whether a browser's own decoders, or the player's, can play the file as it is.
    pub fn content_type(&self) -> &'static str {
        match self.suffix.as_str() {
            "flac" => "audio/flac",
            "mp3" => "audio/mpeg",
            "m4a" => "audio/mp4",
            "aac" => "audio/aac",
            "ogg" | "oga" => "audio/ogg",
            "opus" => "audio/ogg",
            "wav" => "audio/wav",
            "aif" | "aiff" => "audio/aiff",
            "ape" => "audio/x-ape",
            "wv" => "audio/x-wavpack",
            "mpc" => "audio/x-musepack",
            _ => "application/octet-stream",
        }
    }
}

pub const ALBUM_COLUMNS: &str = "a.id, a.library, a.key, a.title, a.sort_title, a.artist, a.year, a.release_date,
    a.original_date, a.genres, a.release_types, a.labels, a.disc_titles, a.compilation, a.mbid, a.dir, a.cover_track,
    a.added_at,
    (SELECT COUNT(*) FROM tracks WHERE album_id = a.id) AS track_count,
    (SELECT COALESCE(SUM(duration), 0.0) FROM tracks WHERE album_id = a.id) AS duration,
    (SELECT COALESCE(SUM(size), 0) FROM tracks WHERE album_id = a.id) AS size";

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Album {
    pub id: i64,
    pub library: String,
    pub key: String,
    pub title: String,
    pub sort_title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub release_date: Option<String>,
    pub original_date: Option<String>,
    pub genres: String,
    pub release_types: String,
    pub labels: String,
    pub disc_titles: String,
    pub compilation: bool,
    pub mbid: Option<String>,
    pub dir: String,
    pub cover_track: Option<i64>,
    pub added_at: i64,
    pub track_count: i64,
    pub duration: f64,
    pub size: i64,
}

impl Album {
    pub fn target(&self) -> String {
        format!("{}/{}", self.library, self.key)
    }

    pub fn genres(&self) -> Vec<String> {
        list(&self.genres)
    }

    pub fn disc_titles(&self) -> Vec<(i64, String)> {
        serde_json::from_str(&self.disc_titles).unwrap_or_default()
    }
}

pub const ARTIST_COLUMNS: &str = "r.id, r.library, r.key, r.name, r.sort_name, r.mbid,
    (SELECT COUNT(*) FROM album_artists WHERE artist_id = r.id) AS album_count,
    (SELECT COUNT(*) FROM track_artists WHERE artist_id = r.id AND role = 'artist') AS track_count,
    (SELECT a.id FROM album_artists aa JOIN albums a ON a.id = aa.album_id WHERE aa.artist_id = r.id
        ORDER BY a.year DESC LIMIT 1) AS cover_album";

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Artist {
    pub id: i64,
    pub library: String,
    pub key: String,
    pub name: String,
    pub sort_name: String,
    pub mbid: Option<String>,
    pub album_count: i64,
    pub track_count: i64,
    pub cover_album: Option<i64>,
}

impl Artist {
    pub fn target(&self) -> String {
        format!("{}/{}", self.library, self.key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Track,
    Album,
    Artist,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Track => "track",
            Kind::Album => "album",
            Kind::Artist => "artist",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UserData {
    pub starred: Option<i64>,
    pub rating: Option<i64>,
    pub play_count: i64,
    pub played: Option<i64>,
}

fn push_list<T>(q: &mut QueryBuilder<Sqlite>, values: &[T])
where
    T: Clone + for<'t> sqlx::Encode<'t, Sqlite> + sqlx::Type<Sqlite>,
{
    q.push("(");

    if values.is_empty() {
        q.push("NULL");
    }

    let mut sep = q.separated(", ");

    for v in values {
        sep.push_bind(v.clone());
    }

    q.push(")");
}

/// Stars, ratings and plays of someone's for these targets (paths for tracks).
pub async fn user_data(
    db: &SqlitePool,
    user_id: i64,
    kind: Kind,
    targets: &[String],
) -> sqlx::Result<HashMap<String, UserData>> {
    let mut out: HashMap<String, UserData> = HashMap::new();

    for chunk in targets.chunks(500) {
        let mut q = QueryBuilder::new("SELECT target, starred_at FROM stars WHERE user_id = ");
        q.push_bind(user_id).push(" AND kind = ").push_bind(kind.as_str()).push(" AND target IN ");
        push_list(&mut q, chunk);

        for (t, at) in q.build_query_as::<(String, i64)>().fetch_all(db).await? {
            out.entry(t).or_default().starred = Some(at);
        }

        let mut q = QueryBuilder::new("SELECT target, rating FROM ratings WHERE user_id = ");
        q.push_bind(user_id).push(" AND kind = ").push_bind(kind.as_str()).push(" AND target IN ");
        push_list(&mut q, chunk);

        for (t, r) in q.build_query_as::<(String, i64)>().fetch_all(db).await? {
            out.entry(t).or_default().rating = Some(r);
        }

        if kind == Kind::Track {
            let mut q = QueryBuilder::new("SELECT track_path, COUNT(*), MAX(played_at) FROM plays WHERE user_id = ");
            q.push_bind(user_id).push(" AND track_path IN ");
            push_list(&mut q, chunk);
            q.push(" GROUP BY track_path");

            for (t, n, at) in q.build_query_as::<(String, i64, i64)>().fetch_all(db).await? {
                let d = out.entry(t).or_default();
                d.play_count = n;
                d.played = Some(at);
            }
        }
    }

    Ok(out)
}

/// Plays of whole albums: how many times any of their tracks were played, and when last.
pub async fn album_plays(db: &SqlitePool, user_id: i64, ids: &[i64]) -> sqlx::Result<HashMap<i64, (i64, i64)>> {
    let mut out = HashMap::new();

    for chunk in ids.chunks(500) {
        let mut q = QueryBuilder::new(
            "SELECT t.album_id, COUNT(*), MAX(p.played_at) FROM plays p JOIN tracks t ON t.path = p.track_path WHERE p.user_id = ",
        );

        q.push_bind(user_id).push(" AND t.album_id IN ");
        push_list(&mut q, chunk);
        q.push(" GROUP BY t.album_id");

        for (id, n, at) in q.build_query_as::<(i64, i64, i64)>().fetch_all(db).await? {
            out.insert(id, (n, at));
        }
    }

    Ok(out)
}

fn in_libraries(q: &mut QueryBuilder<Sqlite>, column: &str, libraries: &[String]) {
    q.push(format!("{column} IN "));
    push_list(q, libraries);
}

pub async fn track(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Track>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE t.id = ?")))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// These tracks, in this order, leaving out what's gone or hidden.
pub async fn tracks(db: &SqlitePool, libraries: &[String], ids: &[i64]) -> sqlx::Result<Vec<Track>> {
    let mut found: HashMap<i64, Track> = HashMap::new();

    for chunk in ids.chunks(500) {
        let mut q = QueryBuilder::new(format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE "));
        in_libraries(&mut q, "t.library", libraries);
        q.push(" AND t.id IN ");
        push_list(&mut q, chunk);

        for t in q.build_query_as::<Track>().fetch_all(db).await? {
            found.insert(t.id, t);
        }
    }

    Ok(ids.iter().filter_map(|id| found.get(id).cloned()).collect())
}

pub async fn tracks_by_path(db: &SqlitePool, libraries: &[String], paths: &[String]) -> sqlx::Result<Vec<Track>> {
    let mut found: HashMap<String, Track> = HashMap::new();

    for chunk in paths.chunks(500) {
        let mut q = QueryBuilder::new(format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE "));
        in_libraries(&mut q, "t.library", libraries);
        q.push(" AND t.path IN ");
        push_list(&mut q, chunk);

        for t in q.build_query_as::<Track>().fetch_all(db).await? {
            found.insert(t.path.clone(), t);
        }
    }

    Ok(paths.iter().filter_map(|p| found.get(p).cloned()).collect())
}

pub async fn album_tracks(db: &SqlitePool, album_id: i64) -> sqlx::Result<Vec<Track>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE t.album_id = ? ORDER BY COALESCE(t.disc, 1), t.number, t.path"
    )))
    .bind(album_id)
    .fetch_all(db)
    .await
}

pub async fn album(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Album>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {ALBUM_COLUMNS} FROM albums a WHERE a.id = ?")))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn artist(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Artist>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {ARTIST_COLUMNS} FROM artists r WHERE r.id = ?")))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Albums an artist made, then the ones they're only on.
pub async fn artist_albums(db: &SqlitePool, artist_id: i64) -> sqlx::Result<(Vec<Album>, Vec<Album>)> {
    let own: Vec<Album> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ALBUM_COLUMNS} FROM albums a JOIN album_artists aa ON aa.album_id = a.id
         WHERE aa.artist_id = ? ORDER BY COALESCE(a.original_date, a.release_date, a.year) DESC, a.sort_title"
    )))
    .bind(artist_id)
    .fetch_all(db)
    .await?;

    let on: Vec<Album> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ALBUM_COLUMNS} FROM albums a
         WHERE a.id IN (SELECT t.album_id FROM track_artists ta JOIN tracks t ON t.id = ta.track_id WHERE ta.artist_id = ?1)
           AND a.id NOT IN (SELECT album_id FROM album_artists WHERE artist_id = ?1)
         ORDER BY COALESCE(a.original_date, a.release_date, a.year) DESC, a.sort_title"
    )))
    .bind(artist_id)
    .fetch_all(db)
    .await?;

    Ok((own, on))
}

/// Each track's artists, with their ids, in credit order.
pub async fn track_artists(db: &SqlitePool, ids: &[i64]) -> sqlx::Result<HashMap<i64, Vec<(i64, String, String)>>> {
    let mut out: HashMap<i64, Vec<(i64, String, String)>> = HashMap::new();

    for chunk in ids.chunks(500) {
        let mut q = QueryBuilder::new(
            "SELECT ta.track_id, r.id, r.name, ta.role FROM track_artists ta JOIN artists r ON r.id = ta.artist_id WHERE ta.track_id IN ",
        );

        push_list(&mut q, chunk);
        q.push(" ORDER BY ta.track_id, ta.role, ta.position");

        for (track, id, name, role) in q.build_query_as::<(i64, i64, String, String)>().fetch_all(db).await? {
            out.entry(track).or_default().push((id, name, role));
        }
    }

    Ok(out)
}

pub async fn album_artists(db: &SqlitePool, ids: &[i64]) -> sqlx::Result<HashMap<i64, Vec<(i64, String)>>> {
    let mut out: HashMap<i64, Vec<(i64, String)>> = HashMap::new();

    for chunk in ids.chunks(500) {
        let mut q = QueryBuilder::new(
            "SELECT aa.album_id, r.id, r.name FROM album_artists aa JOIN artists r ON r.id = aa.artist_id WHERE aa.album_id IN ",
        );

        push_list(&mut q, chunk);
        q.push(" ORDER BY aa.album_id, aa.position");

        for (album, id, name) in q.build_query_as::<(i64, i64, String)>().fetch_all(db).await? {
            out.entry(album).or_default().push((id, name));
        }
    }

    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumOrder {
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

#[derive(Debug, Clone, Default)]
pub struct AlbumFilter {
    pub genre: Option<String>,
    pub years: Option<(i64, i64)>,
    pub artist_id: Option<i64>,
}

pub async fn albums(
    db: &SqlitePool,
    libraries: &[String],
    user_id: i64,
    order: AlbumOrder,
    filter: &AlbumFilter,
    offset: i64,
    limit: i64,
) -> sqlx::Result<Vec<Album>> {
    let mut q = QueryBuilder::new(format!("SELECT {ALBUM_COLUMNS} FROM albums a WHERE "));
    in_libraries(&mut q, "a.library", libraries);

    if let Some(g) = &filter.genre {
        q.push(" AND EXISTS (SELECT 1 FROM json_each(a.genres) WHERE lower(value) = lower(").push_bind(g).push("))");
    }

    if let Some((from, to)) = filter.years {
        q.push(" AND a.year BETWEEN ").push_bind(from.min(to)).push(" AND ").push_bind(from.max(to));
    }

    if let Some(id) = filter.artist_id {
        q.push(" AND a.id IN (SELECT album_id FROM album_artists WHERE artist_id = ").push_bind(id).push(")");
    }

    let played = "(SELECT MAX(p.played_at) FROM plays p JOIN tracks t ON t.path = p.track_path WHERE t.album_id = a.id AND p.user_id = ";

    match order {
        AlbumOrder::Newest => {
            q.push(" ORDER BY a.added_at DESC, a.id DESC");
        },
        AlbumOrder::Name => {
            q.push(" ORDER BY a.sort_title, a.id");
        },
        AlbumOrder::Artist => {
            q.push(" ORDER BY lower(a.artist), a.year, a.sort_title");
        },
        AlbumOrder::Year => {
            let desc = filter.years.is_some_and(|(from, to)| from > to);
            q.push(if desc { " ORDER BY a.year DESC, a.sort_title" } else { " ORDER BY a.year, a.sort_title" });
        },
        AlbumOrder::Random => {
            q.push(" ORDER BY random()");
        },
        AlbumOrder::Recent => {
            q.push(format!(" AND {played}")).push_bind(user_id).push(") IS NOT NULL");
            q.push(format!(" ORDER BY {played}")).push_bind(user_id).push(") DESC");
        },
        AlbumOrder::Frequent => {
            let count = "(SELECT COUNT(*) FROM plays p JOIN tracks t ON t.path = p.track_path WHERE t.album_id = a.id AND p.user_id = ";
            q.push(format!(" AND {count}")).push_bind(user_id).push(") > 0");
            q.push(format!(" ORDER BY {count}")).push_bind(user_id).push(") DESC");
        },
        AlbumOrder::Rated => {
            let rating = "(SELECT rating FROM ratings r WHERE r.kind = 'album' AND r.target = a.library || '/' || a.key AND r.user_id = ";
            q.push(format!(" AND {rating}")).push_bind(user_id).push(") IS NOT NULL");
            q.push(format!(" ORDER BY {rating}")).push_bind(user_id).push(") DESC");
        },
        AlbumOrder::Starred => {
            q.push(" AND EXISTS (SELECT 1 FROM stars s WHERE s.user_id = ").push_bind(user_id);
            q.push(" AND s.kind = 'album' AND s.target = a.library || '/' || a.key)");
            q.push(" ORDER BY (SELECT starred_at FROM stars s WHERE s.user_id = ").push_bind(user_id);
            q.push(" AND s.kind = 'album' AND s.target = a.library || '/' || a.key) DESC");
        },
    }

    q.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    q.build_query_as().fetch_all(db).await
}

/// Artists with albums of their own, by name.
pub async fn album_artists_index(db: &SqlitePool, libraries: &[String]) -> sqlx::Result<Vec<Artist>> {
    let mut q = QueryBuilder::new(format!("SELECT {ARTIST_COLUMNS} FROM artists r WHERE "));
    in_libraries(&mut q, "r.library", libraries);
    q.push(" AND EXISTS (SELECT 1 FROM album_artists WHERE artist_id = r.id) ORDER BY r.sort_name");
    q.build_query_as().fetch_all(db).await
}

fn words(query: &str) -> Vec<String> {
    let q = query.trim().trim_matches('"').trim();
    q.split_whitespace().map(|w| format!("%{}%", w.to_lowercase().replace(['%', '_'], ""))).collect()
}

fn push_words(q: &mut QueryBuilder<Sqlite>, columns: &str, words: &[String]) {
    for w in words {
        q.push(format!(" AND lower({columns}) LIKE ")).push_bind(w.clone());
    }
}

/// Everything when `query` is empty, so apps can page through the whole library.
pub async fn search_artists(
    db: &SqlitePool,
    libraries: &[String],
    query: &str,
    offset: i64,
    limit: i64,
) -> sqlx::Result<Vec<Artist>> {
    let w = words(query);
    let mut q = QueryBuilder::new(format!("SELECT {ARTIST_COLUMNS} FROM artists r WHERE "));
    in_libraries(&mut q, "r.library", libraries);
    push_words(&mut q, "r.name", &w);
    q.push(" ORDER BY r.sort_name, r.id LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    q.build_query_as().fetch_all(db).await
}

pub async fn search_albums(
    db: &SqlitePool,
    libraries: &[String],
    query: &str,
    offset: i64,
    limit: i64,
) -> sqlx::Result<Vec<Album>> {
    let w = words(query);
    let mut q = QueryBuilder::new(format!("SELECT {ALBUM_COLUMNS} FROM albums a WHERE "));
    in_libraries(&mut q, "a.library", libraries);
    push_words(&mut q, "a.title || ' ' || a.artist", &w);
    q.push(" ORDER BY a.sort_title, a.id LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    q.build_query_as().fetch_all(db).await
}

pub async fn search_tracks(
    db: &SqlitePool,
    libraries: &[String],
    query: &str,
    offset: i64,
    limit: i64,
) -> sqlx::Result<Vec<Track>> {
    let w = words(query);
    let mut q = QueryBuilder::new(format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE "));
    in_libraries(&mut q, "t.library", libraries);
    push_words(&mut q, "t.title || ' ' || t.artist || ' ' || t.album", &w);

    if w.is_empty() {
        q.push(" ORDER BY t.id");
    } else {
        q.push(" ORDER BY (lower(t.title) = ").push_bind(query.trim().to_lowercase()).push(") DESC, t.title");
    }

    q.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    q.build_query_as().fetch_all(db).await
}

pub async fn genres(db: &SqlitePool, libraries: &[String]) -> sqlx::Result<Vec<(String, i64, i64)>> {
    let mut q = QueryBuilder::new(
        "SELECT g.value, COUNT(DISTINCT t.id), COUNT(DISTINCT t.album_id) FROM tracks t, json_each(t.genres) g WHERE ",
    );

    in_libraries(&mut q, "t.library", libraries);
    q.push(" GROUP BY lower(g.value) ORDER BY lower(g.value)");
    q.build_query_as().fetch_all(db).await
}

pub async fn random_tracks(
    db: &SqlitePool,
    libraries: &[String],
    count: i64,
    genre: Option<&str>,
    years: (Option<i64>, Option<i64>),
) -> sqlx::Result<Vec<Track>> {
    let mut q = QueryBuilder::new(format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE "));
    in_libraries(&mut q, "t.library", libraries);

    if let Some(g) = genre {
        q.push(" AND EXISTS (SELECT 1 FROM json_each(t.genres) WHERE lower(value) = lower(").push_bind(g).push("))");
    }

    if let Some(from) = years.0 {
        q.push(" AND t.year >= ").push_bind(from);
    }

    if let Some(to) = years.1 {
        q.push(" AND t.year <= ").push_bind(to);
    }

    q.push(" ORDER BY random() LIMIT ").push_bind(count);
    q.build_query_as().fetch_all(db).await
}

pub async fn genre_tracks(
    db: &SqlitePool,
    libraries: &[String],
    genre: &str,
    offset: i64,
    limit: i64,
) -> sqlx::Result<Vec<Track>> {
    let mut q = QueryBuilder::new(format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE "));
    in_libraries(&mut q, "t.library", libraries);
    q.push(" AND EXISTS (SELECT 1 FROM json_each(t.genres) WHERE lower(value) = lower(").push_bind(genre).push("))");
    q.push(" ORDER BY t.album_id, t.disc, t.number LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    q.build_query_as().fetch_all(db).await
}

pub async fn starred(
    db: &SqlitePool,
    libraries: &[String],
    user_id: i64,
) -> sqlx::Result<(Vec<Artist>, Vec<Album>, Vec<Track>)> {
    let mut q = QueryBuilder::new(format!(
        "SELECT {ARTIST_COLUMNS} FROM artists r JOIN stars s ON s.kind = 'artist' AND s.target = r.library || '/' || r.key WHERE s.user_id = "
    ));

    q.push_bind(user_id).push(" AND ");
    in_libraries(&mut q, "r.library", libraries);
    q.push(" ORDER BY s.starred_at DESC");
    let artists = q.build_query_as().fetch_all(db).await?;

    let mut q = QueryBuilder::new(format!(
        "SELECT {ALBUM_COLUMNS} FROM albums a JOIN stars s ON s.kind = 'album' AND s.target = a.library || '/' || a.key WHERE s.user_id = "
    ));

    q.push_bind(user_id).push(" AND ");
    in_libraries(&mut q, "a.library", libraries);
    q.push(" ORDER BY s.starred_at DESC");
    let albums = q.build_query_as().fetch_all(db).await?;

    let mut q = QueryBuilder::new(format!(
        "SELECT {TRACK_COLUMNS} {TRACKS_FROM} JOIN stars s ON s.kind = 'track' AND s.target = t.path WHERE s.user_id = "
    ));

    q.push_bind(user_id).push(" AND ");
    in_libraries(&mut q, "t.library", libraries);
    q.push(" ORDER BY s.starred_at DESC");
    let tracks = q.build_query_as().fetch_all(db).await?;
    Ok((artists, albums, tracks))
}

/// An artist's most played tracks here, by everyone; their albums in order when nobody's played
/// them yet.
pub async fn top_tracks(db: &SqlitePool, libraries: &[String], artist_id: i64, count: i64) -> sqlx::Result<Vec<Track>> {
    let mut q = QueryBuilder::new(format!(
        "SELECT {TRACK_COLUMNS} {TRACKS_FROM} JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'artist'
         WHERE ta.artist_id = "
    ));

    q.push_bind(artist_id).push(" AND ");
    in_libraries(&mut q, "t.library", libraries);
    q.push(" ORDER BY (SELECT COUNT(*) FROM plays p WHERE p.track_path = t.path) DESC, a.year DESC, t.disc, t.number LIMIT ");
    q.push_bind(count);
    q.build_query_as().fetch_all(db).await
}

pub async fn artist_by_name(db: &SqlitePool, libraries: &[String], name: &str) -> sqlx::Result<Option<i64>> {
    let mut q = QueryBuilder::new("SELECT id FROM artists r WHERE r.key = ");
    q.push_bind(name.to_lowercase()).push(" AND ");
    in_libraries(&mut q, "r.library", libraries);
    q.push(" LIMIT 1");
    q.build_query_scalar().fetch_optional(db).await
}

/// Tracks like these, without anything from outside: the same artists, then
/// artists sharing their genres, shuffled.
pub async fn similar_tracks(
    db: &SqlitePool,
    libraries: &[String],
    artist_ids: &[i64],
    genres: &[String],
    exclude: &[i64],
    count: i64,
) -> sqlx::Result<Vec<Track>> {
    let mut q = QueryBuilder::new(format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} WHERE "));
    in_libraries(&mut q, "t.library", libraries);
    q.push(" AND t.id NOT IN ");
    push_list(&mut q, exclude);
    q.push(" AND (t.id IN (SELECT track_id FROM track_artists WHERE artist_id IN ");
    push_list(&mut q, artist_ids);
    q.push(")");

    if !genres.is_empty() {
        q.push(" OR EXISTS (SELECT 1 FROM json_each(t.genres) g WHERE lower(g.value) IN ");
        let lower: Vec<String> = genres.iter().map(|g| g.to_lowercase()).collect();
        q.push("(");
        let mut sep = q.separated(", ");

        for g in lower {
            sep.push_bind(g);
        }

        q.push("))");
    }

    q.push(") ORDER BY random() LIMIT ").push_bind(count);
    q.build_query_as().fetch_all(db).await
}

pub async fn set_star(db: &SqlitePool, user_id: i64, kind: Kind, target: &str, on: bool) -> sqlx::Result<()> {
    if on {
        sqlx::query("INSERT OR IGNORE INTO stars (user_id, kind, target, starred_at) VALUES (?, ?, ?, ?)")
            .bind(user_id)
            .bind(kind.as_str())
            .bind(target)
            .bind(now())
            .execute(db)
            .await?;
    } else {
        sqlx::query("DELETE FROM stars WHERE user_id = ? AND kind = ? AND target = ?")
            .bind(user_id)
            .bind(kind.as_str())
            .bind(target)
            .execute(db)
            .await?;
    }
    Ok(())
}

pub async fn set_rating(db: &SqlitePool, user_id: i64, kind: Kind, target: &str, rating: i64) -> sqlx::Result<()> {
    if (1..=5).contains(&rating) {
        sqlx::query(
            "INSERT INTO ratings (user_id, kind, target, rating) VALUES (?, ?, ?, ?)
             ON CONFLICT(user_id, kind, target) DO UPDATE SET rating = excluded.rating",
        )
        .bind(user_id)
        .bind(kind.as_str())
        .bind(target)
        .bind(rating)
        .execute(db)
        .await?;
    } else {
        sqlx::query("DELETE FROM ratings WHERE user_id = ? AND kind = ? AND target = ?")
            .bind(user_id)
            .bind(kind.as_str())
            .bind(target)
            .execute(db)
            .await?;
    }
    Ok(())
}

pub async fn record_play(db: &SqlitePool, user_id: i64, path: &str, at: i64) -> sqlx::Result<()> {
    let recent: Option<i64> =
        sqlx::query_scalar("SELECT id FROM plays WHERE user_id = ? AND track_path = ? AND played_at BETWEEN ? AND ?")
            .bind(user_id)
            .bind(path)
            .bind(at - 30)
            .bind(at + 30)
            .fetch_optional(db)
            .await?;

    if recent.is_none() {
        sqlx::query("INSERT INTO plays (user_id, track_path, played_at) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind(path)
            .bind(at)
            .execute(db)
            .await?;
    }

    Ok(())
}

/// Albums someone played lately, most recent first.
pub async fn recently_played_albums(
    db: &SqlitePool,
    libraries: &[String],
    user_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<Album>> {
    albums(db, libraries, user_id, AlbumOrder::Recent, &AlbumFilter::default(), 0, limit).await
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Playlist {
    pub id: i64,
    pub owner_id: i64,
    pub owner: String,
    pub name: String,
    pub comment: Option<String>,
    pub public: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub track_count: i64,
    pub duration: f64,
}

const PLAYLIST_COLUMNS: &str = "p.id, p.owner_id, u.username AS owner, p.name, p.comment, p.public, p.created_at, p.updated_at,
    (SELECT COUNT(*) FROM playlist_tracks pt JOIN tracks t ON t.path = pt.track_path WHERE pt.playlist_id = p.id) AS track_count,
    (SELECT COALESCE(SUM(t.duration), 0.0) FROM playlist_tracks pt JOIN tracks t ON t.path = pt.track_path WHERE pt.playlist_id = p.id) AS duration";

pub async fn playlists(db: &SqlitePool, user_id: i64) -> sqlx::Result<Vec<Playlist>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {PLAYLIST_COLUMNS} FROM playlists p JOIN users u ON u.id = p.owner_id
         WHERE p.owner_id = ?1 OR p.public = 1 ORDER BY p.owner_id != ?1, lower(p.name)"
    )))
    .bind(user_id)
    .fetch_all(db)
    .await
}

/// A playlist someone may see: their own, or anyone's public one.
pub async fn playlist(db: &SqlitePool, user_id: i64, id: i64) -> sqlx::Result<Option<Playlist>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {PLAYLIST_COLUMNS} FROM playlists p JOIN users u ON u.id = p.owner_id
         WHERE p.id = ?2 AND (p.owner_id = ?1 OR p.public = 1)"
    )))
    .bind(user_id)
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn playlist_paths(db: &SqlitePool, id: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT track_path FROM playlist_tracks WHERE playlist_id = ? ORDER BY position")
        .bind(id)
        .fetch_all(db)
        .await
}

pub async fn set_playlist_paths(tx: &mut sqlx::SqliteConnection, id: i64, paths: &[String]) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?").bind(id).execute(&mut *tx).await?;

    for (i, p) in paths.iter().enumerate() {
        sqlx::query("INSERT INTO playlist_tracks (playlist_id, position, track_path) VALUES (?, ?, ?)")
            .bind(id)
            .bind(i as i64)
            .bind(p)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("UPDATE playlists SET updated_at = ? WHERE id = ?").bind(now()).bind(id).execute(&mut *tx).await?;
    Ok(())
}

pub async fn create_playlist(db: &SqlitePool, owner: i64, name: &str, paths: &[String]) -> sqlx::Result<i64> {
    let mut tx = db.begin().await?;

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (owner_id, name, created_at, updated_at) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(owner)
    .bind(name)
    .bind(now())
    .bind(now())
    .fetch_one(&mut *tx)
    .await?;

    set_playlist_paths(&mut tx, id, paths).await?;
    tx.commit().await?;
    Ok(id)
}
