// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use lofty::file::FileType;
use lofty::prelude::*;
use lofty::probe::Probe;
use regex::Regex;
use serde::Serialize;

use super::parse;
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

pub const AUDIO_EXTENSIONS: &[&str] =
    &["flac", "mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "aif", "aiff", "ape", "wv", "mpc"];

pub const COVER_NAMES: &[&str] = &["cover", "folder", "front", "album", "albumart", "poster"];

pub const VARIOUS_ARTISTS: &str = "Various Artists";

static DISC_FOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:cd|dis[ck]|side)\s*[-_.]?\s*(?P<n>\d{1,2})\b").unwrap());

static NUMBERED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(?P<d>\d)[-.])?(?P<n>\d{1,3})(?:\s*[-._)]\s*|\s+)(?P<t>.+)$").unwrap());

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// The folder a release lives in: the track's own, or its parent when the
/// track sits in a `CD1`/`Disc 2` folder.
pub fn release_dir(path: &Path) -> (PathBuf, Option<i64>) {
    let dir = path.parent().unwrap_or(Path::new("/"));
    let name = dir.file_name().unwrap_or_default().to_string_lossy();

    match DISC_FOLDER.captures(&name) {
        Some(c) => (dir.parent().unwrap_or(dir).to_path_buf(), c["n"].parse().ok()),
        None => (dir.to_path_buf(), None),
    }
}

pub fn artist_sort(name: &str) -> String {
    parse::sort_title(name)
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub artists: Vec<String>,
    pub artist_sort: Option<String>,
    pub album: String,
    pub album_artist: Option<String>,
    pub album_artists: Vec<String>,
    pub album_artist_sort: Option<String>,
    pub album_sort: Option<String>,
    pub composers: Vec<String>,
    pub compilation: bool,
    pub disc: Option<i64>,
    pub disc_subtitle: Option<String>,
    pub number: Option<i64>,
    pub year: Option<i64>,
    pub release_date: Option<String>,
    pub original_date: Option<String>,
    pub genres: Vec<String>,
    pub release_types: Vec<String>,
    pub labels: Vec<String>,
    pub mbid: Option<String>,
    pub album_mbid: Option<String>,
    pub artist_mbids: Vec<String>,
    pub album_artist_mbids: Vec<String>,
    pub bpm: Option<i64>,
    pub comment: Option<String>,
    pub lyrics: Option<String>,
    pub duration: f64,
    pub codec: &'static str,
    pub suffix: String,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub embedded_art: bool,
    pub rg_track_gain: Option<f64>,
    pub rg_track_peak: Option<f64>,
    pub rg_album_gain: Option<f64>,
    pub rg_album_peak: Option<f64>,
}

impl Tags {
    pub fn album_key(&self, path: &Path) -> String {
        match &self.album_mbid {
            Some(mb) => format!("mb:{}", mb.to_lowercase()),
            None => format!("{}\0{}", self.album.to_lowercase(), release_dir(path).0.display()),
        }
    }
}

fn clean(s: &str) -> Option<String> {
    let s = s.trim().trim_matches('\0').trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn leading_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(s.len());
    s[..end].parse().ok()
}

fn year_of(date: &str) -> Option<i64> {
    let digits: String = date.chars().skip_while(|c| !c.is_ascii_digit()).take(4).collect();
    digits.parse().ok().filter(|y| (1000..=9999).contains(y))
}

fn codec_of(ty: FileType, bit_depth: Option<u8>) -> &'static str {
    match ty {
        FileType::Aac => "aac",
        FileType::Aiff | FileType::Wav => "pcm",
        FileType::Ape => "ape",
        FileType::Flac => "flac",
        FileType::Mpeg => "mp3",
        FileType::Mp4 if bit_depth.is_some() => "alac",
        FileType::Mp4 => "aac",
        FileType::Mpc => "musepack",
        FileType::Opus => "opus",
        FileType::Vorbis => "vorbis",
        FileType::Speex => "speex",
        FileType::WavPack => "wavpack",
        _ => "unknown",
    }
}

fn unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values.into_iter().filter(|v| seen.insert(v.to_lowercase())).collect()
}

pub fn read(path: &Path) -> anyhow::Result<Tags> {
    let file = Probe::open(path)?.guess_file_type()?.read()?;
    let props = file.properties();
    let tag = file.primary_tag().or_else(|| file.first_tag());
    let one = |key: ItemKey| tag.and_then(|t| t.get_string(key)).and_then(clean);

    let many = |key: ItemKey| -> Vec<String> {
        tag.map(|t| t.get_strings(key).filter_map(clean).collect()).unwrap_or_default()
    };

    let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let named = NUMBERED.captures(&stem);
    let (dir, folder_disc) = release_dir(path);

    let artist_values = many(ItemKey::TrackArtist);
    let mut artists = many(ItemKey::TrackArtists);

    if artists.is_empty() {
        artists = artist_values.clone();
    }

    let artist = match artist_values.len() {
        0 => artists.join(", "),
        _ => artist_values.join(", "),
    };

    let album_artist_values = many(ItemKey::AlbumArtist);
    let mut album_artists = many(ItemKey::AlbumArtists);

    if album_artists.is_empty() {
        album_artists = album_artist_values.clone();
    }

    let album_artist = (!album_artist_values.is_empty()).then(|| album_artist_values.join(", "));

    let recorded = one(ItemKey::RecordingDate).or_else(|| one(ItemKey::Year));
    let released = one(ItemKey::ReleaseDate);
    let original = one(ItemKey::OriginalReleaseDate);
    let year = recorded.as_deref().or(released.as_deref()).or(original.as_deref()).and_then(year_of);

    let gain = |key| one(key).as_deref().and_then(leading_number);
    let r128 = |key| one(key).and_then(|v| v.trim().parse::<f64>().ok()).map(|q| q / 256.0 + 5.0);
    let bit_depth = props.bit_depth();

    Ok(Tags {
        title: one(ItemKey::TrackTitle)
            .or_else(|| named.as_ref().map(|c| c["t"].trim().to_string()))
            .unwrap_or_else(|| stem.clone()),
        artist: if artist.is_empty() { album_artist.clone().unwrap_or_default() } else { artist },
        artists: unique(artists),
        artist_sort: one(ItemKey::TrackArtistSortOrder),
        album: one(ItemKey::AlbumTitle)
            .unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().to_string()),
        album_artist,
        album_artists: unique(album_artists),
        album_artist_sort: one(ItemKey::AlbumArtistSortOrder),
        album_sort: one(ItemKey::AlbumTitleSortOrder),
        composers: unique(many(ItemKey::Composer)),
        compilation: one(ItemKey::FlagCompilation).is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true")),
        disc: tag.and_then(|t| t.disk()).map(i64::from).filter(|&d| d > 0).or(folder_disc),
        disc_subtitle: one(ItemKey::SetSubtitle),
        number: tag.and_then(|t| t.track()).map(i64::from).or_else(|| named.as_ref().and_then(|c| c["n"].parse().ok())),
        year,
        release_date: released.or(recorded),
        original_date: original,
        genres: unique(many(ItemKey::Genre).iter().flat_map(|g| g.split(';').filter_map(clean)).collect::<Vec<_>>()),
        release_types: unique(
            many(ItemKey::MusicBrainzReleaseType)
                .iter()
                .flat_map(|g| g.split(';').filter_map(clean))
                .collect::<Vec<_>>(),
        ),
        labels: unique(many(ItemKey::Label)),
        mbid: one(ItemKey::MusicBrainzRecordingId),
        album_mbid: one(ItemKey::MusicBrainzReleaseId),
        artist_mbids: many(ItemKey::MusicBrainzArtistId),
        album_artist_mbids: many(ItemKey::MusicBrainzReleaseArtistId),
        bpm: one(ItemKey::IntegerBpm)
            .or_else(|| one(ItemKey::Bpm))
            .as_deref()
            .and_then(leading_number)
            .map(|b| b as i64),
        comment: one(ItemKey::Comment),
        lyrics: one(ItemKey::Lyrics).or_else(|| one(ItemKey::UnsyncLyrics)),
        duration: props.duration().as_secs_f64(),
        codec: codec_of(file.file_type(), bit_depth),
        suffix: path.extension().unwrap_or_default().to_string_lossy().to_lowercase(),
        bitrate: props.audio_bitrate().or(props.overall_bitrate()).map(i64::from),
        sample_rate: props.sample_rate().map(i64::from),
        bit_depth: bit_depth.map(i64::from),
        channels: props.channels().map(i64::from),
        embedded_art: file.tags().iter().any(|t| !t.pictures().is_empty()),
        rg_track_gain: gain(ItemKey::ReplayGainTrackGain).or_else(|| r128(ItemKey::R128TrackGain)),
        rg_track_peak: gain(ItemKey::ReplayGainTrackPeak),
        rg_album_gain: gain(ItemKey::ReplayGainAlbumGain).or_else(|| r128(ItemKey::R128AlbumGain)),
        rg_album_peak: gain(ItemKey::ReplayGainAlbumPeak),
    })
}

/// The front cover inside a file, or any picture when there's no front.
pub fn embedded_cover(path: &Path) -> Option<(Vec<u8>, String)> {
    let file = Probe::open(path).ok()?.guess_file_type().ok()?.read().ok()?;
    let pictures: Vec<_> = file.tags().iter().flat_map(|t| t.pictures()).collect();

    let pic = pictures
        .iter()
        .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
        .or_else(|| pictures.first())?;

    let mime = pic.mime_type().map(|m| m.as_str().to_string()).unwrap_or_else(|| "image/jpeg".into());
    Some((pic.data().to_vec(), mime))
}

#[derive(Debug)]
struct Found {
    path: PathBuf,
    size: i64,
    mtime: i64,
}

fn walk(root: &Path, download_dirs: &[PathBuf], out: &mut Vec<Found>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        tracing::warn!("can't read {} (permissions?)", root.display());
        return;
    };

    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }

        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };

        if ft.is_dir() || (ft.is_symlink() && path.is_dir()) {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

            if !download_dirs.iter().any(|d| d == &canonical) {
                walk(&path, download_dirs, out);
            }
        } else if is_audio(&path) {
            let (size, mtime) = super::scanner::file_info(&path);
            out.push(Found { path, size, mtime });
        }
    }
}

fn read_all(files: Vec<Found>) -> Vec<(Found, anyhow::Result<Tags>)> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunk = files.len().div_ceil(threads).max(1);
    let mut files = files;
    let mut chunks = Vec::new();

    while !files.is_empty() {
        let rest = files.split_off(chunk.min(files.len()));
        chunks.push(std::mem::replace(&mut files, rest));
    }

    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|c| {
                s.spawn(move || {
                    c.into_iter()
                        .map(|f| {
                            let tags = read(&f.path);
                            (f, tags)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".into())
}

pub async fn scan(state: &AppState, name: &str, root: &Path, download_dirs: Vec<PathBuf>) -> anyhow::Result<()> {
    let started = std::time::Instant::now();

    let found = {
        let root = root.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            walk(&root, &download_dirs, &mut out);
            out
        })
        .await?
    };

    let db = &state.db;

    let known: Vec<(i64, String, i64, i64)> =
        sqlx::query_as("SELECT id, path, size, mtime FROM tracks WHERE library = ?").bind(name).fetch_all(db).await?;

    if found.is_empty() && !known.is_empty() {
        tracing::warn!(
            "library {name:?}: {} has no music; if it's a drive that isn't mounted, that's why. \
             Keeping the {} tracks already scanned",
            root.display(),
            known.len()
        );

        return Ok(());
    }

    let unchanged: HashMap<&str, (i64, i64)> = known.iter().map(|(_, p, s, m)| (p.as_str(), (*s, *m))).collect();
    let present: HashSet<String> = found.iter().map(|f| f.path.to_string_lossy().to_string()).collect();

    let (same, changed): (Vec<Found>, Vec<Found>) =
        found.into_iter().partition(|f| unchanged.get(f.path.to_string_lossy().as_ref()) == Some(&(f.size, f.mtime)));

    let read = tokio::task::spawn_blocking(move || read_all(changed)).await?;
    let gone: Vec<i64> = known.iter().filter(|(_, p, ..)| !present.contains(p)).map(|(id, ..)| *id).collect();

    let ts = now();
    let mut tx = db.begin().await?;
    let mut skipped = Vec::new();
    let mut new_tracks = 0usize;

    for (f, tags) in &read {
        let t = match tags {
            Ok(t) => t,
            Err(e) => {
                skipped.push((f.path.clone(), format!("can't read its tags: {e}")));
                continue;
            },
        };

        let inserted: bool = sqlx::query_scalar(
            "INSERT INTO tracks (library, path, size, mtime, album_key, title, artist, artists, artist_sort, album,
                album_artist, album_artists, album_artist_sort, album_sort, composers, compilation, disc, disc_subtitle,
                number, year, release_date, original_date, genres, release_types, labels, mbid, album_mbid,
                artist_mbids, album_artist_mbids, bpm, comment, lyrics, duration, codec, suffix, bitrate, sample_rate,
                bit_depth, channels, embedded_art, rg_track_gain, rg_track_peak, rg_album_gain, rg_album_peak, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21,
                ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41,
                ?42, ?43, ?44, ?45)
             ON CONFLICT(path) DO UPDATE SET
                library = excluded.library, size = excluded.size, mtime = excluded.mtime,
                album_key = excluded.album_key, title = excluded.title, artist = excluded.artist,
                artists = excluded.artists, artist_sort = excluded.artist_sort, album = excluded.album,
                album_artist = excluded.album_artist, album_artists = excluded.album_artists,
                album_artist_sort = excluded.album_artist_sort, album_sort = excluded.album_sort,
                composers = excluded.composers, compilation = excluded.compilation, disc = excluded.disc,
                disc_subtitle = excluded.disc_subtitle, number = excluded.number, year = excluded.year,
                release_date = excluded.release_date, original_date = excluded.original_date,
                genres = excluded.genres, release_types = excluded.release_types, labels = excluded.labels,
                mbid = excluded.mbid, album_mbid = excluded.album_mbid, artist_mbids = excluded.artist_mbids,
                album_artist_mbids = excluded.album_artist_mbids, bpm = excluded.bpm, comment = excluded.comment,
                lyrics = excluded.lyrics, duration = excluded.duration, codec = excluded.codec,
                suffix = excluded.suffix, bitrate = excluded.bitrate, sample_rate = excluded.sample_rate,
                bit_depth = excluded.bit_depth, channels = excluded.channels, embedded_art = excluded.embedded_art,
                rg_track_gain = excluded.rg_track_gain, rg_track_peak = excluded.rg_track_peak,
                rg_album_gain = excluded.rg_album_gain, rg_album_peak = excluded.rg_album_peak,
                loudness = NULL, peak = NULL, analyzed = 0
             RETURNING added_at = ?45",
        )
        .bind(name)
        .bind(f.path.to_string_lossy().to_string())
        .bind(f.size)
        .bind(f.mtime)
        .bind(t.album_key(&f.path))
        .bind(&t.title)
        .bind(&t.artist)
        .bind(json(&t.artists))
        .bind(&t.artist_sort)
        .bind(&t.album)
        .bind(&t.album_artist)
        .bind(json(&t.album_artists))
        .bind(&t.album_artist_sort)
        .bind(&t.album_sort)
        .bind(json(&t.composers))
        .bind(t.compilation)
        .bind(t.disc)
        .bind(&t.disc_subtitle)
        .bind(t.number)
        .bind(t.year)
        .bind(&t.release_date)
        .bind(&t.original_date)
        .bind(json(&t.genres))
        .bind(json(&t.release_types))
        .bind(json(&t.labels))
        .bind(&t.mbid)
        .bind(&t.album_mbid)
        .bind(json(&t.artist_mbids))
        .bind(json(&t.album_artist_mbids))
        .bind(t.bpm)
        .bind(&t.comment)
        .bind(&t.lyrics)
        .bind(t.duration)
        .bind(t.codec)
        .bind(&t.suffix)
        .bind(t.bitrate)
        .bind(t.sample_rate)
        .bind(t.bit_depth)
        .bind(t.channels)
        .bind(t.embedded_art)
        .bind(t.rg_track_gain)
        .bind(t.rg_track_peak)
        .bind(t.rg_album_gain)
        .bind(t.rg_album_peak)
        .bind(ts)
        .fetch_one(&mut *tx)
        .await?;

        if inserted {
            new_tracks += 1;
        }
    }

    for chunk in gone.chunks(500) {
        let ids = chunk.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM tracks WHERE id IN ({ids})"))).execute(&mut *tx).await?;
    }

    let changed = !read.is_empty() || !gone.is_empty();

    if changed {
        derive(&mut tx, name, ts).await?;
    }

    sqlx::query("DELETE FROM skipped WHERE library = ? AND path NOT IN (SELECT path FROM tracks)")
        .bind(name)
        .execute(&mut *tx)
        .await?;

    for (path, reason) in &skipped {
        tracing::warn!("skipped {}: {reason}", path.display());

        sqlx::query("INSERT OR REPLACE INTO skipped (path, library, reason, seen_at) VALUES (?, ?, ?, ?)")
            .bind(path.to_string_lossy().to_string())
            .bind(name)
            .bind(reason)
            .bind(ts)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    let (tracks, albums): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM tracks WHERE library = ?1), (SELECT COUNT(*) FROM albums WHERE library = ?1)",
    )
    .bind(name)
    .fetch_one(db)
    .await?;

    tracing::info!(
        "scanned {name:?} in {:.1?}: {albums} albums, {tracks} tracks ({} read, {new_tracks} new, {} removed, {} unchanged), {} skipped",
        started.elapsed(),
        read.len(),
        gone.len(),
        same.len(),
        skipped.len(),
    );

    state.events.send(Event::ScanFinished {
        library: name.to_string(),
        items: albums as usize,
        media: tracks as usize,
        skipped: skipped.len(),
    });

    if changed {
        state.events.send(Event::LibraryChanged { library: name.to_string() });
    }

    crate::music::loudness::wake(state);
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    path: String,
    album_key: String,
    artist: String,
    artists: String,
    artist_sort: Option<String>,
    album: String,
    album_artist: Option<String>,
    album_artists: String,
    album_artist_sort: Option<String>,
    album_sort: Option<String>,
    composers: String,
    compilation: bool,
    disc: Option<i64>,
    disc_subtitle: Option<String>,
    year: Option<i64>,
    release_date: Option<String>,
    original_date: Option<String>,
    genres: String,
    release_types: String,
    labels: String,
    album_mbid: Option<String>,
    artist_mbids: String,
    album_artist_mbids: String,
    embedded_art: bool,
    added_at: i64,
}

fn list(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

fn most_common<'a>(values: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut counts: Vec<(&str, usize)> = Vec::new();

    for v in values {
        match counts.iter_mut().find(|(k, _)| *k == v) {
            Some((_, n)) => *n += 1,
            None => counts.push((v, 1)),
        }
    }

    counts.into_iter().rev().max_by_key(|(_, n)| *n).map(|(v, _)| v)
}

#[derive(Default)]
struct ArtistInfo {
    name: String,
    sort: Option<String>,
    mbid: Option<String>,
}

/// Works out albums and artists from the tracks' tags, and links them up.
async fn derive(tx: &mut sqlx::SqliteConnection, library: &str, ts: i64) -> anyhow::Result<()> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, path, album_key, artist, artists, artist_sort, album, album_artist, album_artists,
                album_artist_sort, album_sort, composers, compilation, disc, disc_subtitle, year, release_date,
                original_date, genres, release_types, labels, album_mbid, artist_mbids, album_artist_mbids,
                embedded_art, added_at
         FROM tracks WHERE library = ? ORDER BY album_key, disc, number, path",
    )
    .bind(library)
    .fetch_all(&mut *tx)
    .await?;

    let mut groups: Vec<(&str, Vec<&Row>)> = Vec::new();

    for r in &rows {
        match groups.last_mut() {
            Some((k, g)) if *k == r.album_key => g.push(r),
            _ => groups.push((&r.album_key, vec![r])),
        }
    }

    let mut artists: HashMap<String, ArtistInfo> = HashMap::new();

    let mut note = |name: &str, sort: Option<&String>, mbid: Option<&String>| {
        let key = name.to_lowercase();
        let a = artists.entry(key).or_default();

        if a.name.is_empty() {
            a.name = name.to_string();
        }

        if a.sort.is_none() {
            a.sort = sort.cloned();
        }

        if a.mbid.is_none() {
            a.mbid = mbid.cloned();
        }
    };

    let mut album_links: Vec<(i64, Vec<String>)> = Vec::new();
    let mut track_links: Vec<(i64, Vec<String>, Vec<String>)> = Vec::new();
    let mut keep_albums = Vec::new();

    for (key, tracks) in &groups {
        let first = tracks[0];
        let title = most_common(tracks.iter().map(|t| t.album.as_str())).unwrap_or(&first.album).to_string();
        let tagged_artist = most_common(tracks.iter().filter_map(|t| t.album_artist.as_deref()));
        let flagged = tracks.iter().any(|t| t.compilation);
        let one_artist = tracks.iter().all(|t| t.artist == first.artist);

        let (artist, mut album_artists, compilation) = match tagged_artist {
            Some(a) => {
                let names = tracks
                    .iter()
                    .find(|t| t.album_artist.as_deref() == Some(a))
                    .map(|t| list(&t.album_artists))
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| vec![a.to_string()]);

                (a.to_string(), names, flagged)
            },
            None if !flagged && one_artist && !first.artist.is_empty() => {
                let names = list(&first.artists);
                (first.artist.clone(), if names.is_empty() { vec![first.artist.clone()] } else { names }, false)
            },
            None => (VARIOUS_ARTISTS.to_string(), vec![VARIOUS_ARTISTS.to_string()], true),
        };

        if album_artists.is_empty() {
            album_artists.push(artist.clone());
        }

        let source = tracks.iter().find(|t| t.album_artist.as_deref() == Some(artist.as_str())).unwrap_or(&first);
        let mbids = list(&source.album_artist_mbids);

        for (i, a) in album_artists.iter().enumerate() {
            let sort = (album_artists.len() == 1).then_some(source.album_artist_sort.as_ref()).flatten();
            note(a, sort, mbids.get(i));
        }

        let mut genres = Vec::new();
        let mut discs: Vec<(i64, String)> = Vec::new();

        for t in tracks.iter() {
            for g in list(&t.genres) {
                if !genres.contains(&g) {
                    genres.push(g);
                }
            }
            if let (Some(d), Some(s)) = (t.disc, &t.disc_subtitle)
                && !discs.iter().any(|(n, _)| *n == d)
            {
                discs.push((d, s.clone()));
            }
        }

        let dir = release_dir(Path::new(&first.path)).0;
        let cover = tracks.iter().find(|t| t.embedded_art).map(|t| t.id);
        let sort_title = first.album_sort.clone().unwrap_or_else(|| parse::sort_title(&title));
        let added = tracks.iter().map(|t| t.added_at).min().unwrap_or(ts);

        let album_id: i64 = sqlx::query_scalar(
            "INSERT INTO albums (library, key, title, sort_title, artist, year, release_date, original_date, genres,
                release_types, labels, disc_titles, compilation, mbid, dir, cover_track, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(library, key) DO UPDATE SET
                title = excluded.title, sort_title = excluded.sort_title, artist = excluded.artist,
                year = excluded.year, release_date = excluded.release_date, original_date = excluded.original_date,
                genres = excluded.genres, release_types = excluded.release_types, labels = excluded.labels,
                disc_titles = excluded.disc_titles, compilation = excluded.compilation, mbid = excluded.mbid,
                dir = excluded.dir, cover_track = excluded.cover_track,
                added_at = MIN(albums.added_at, excluded.added_at)
             RETURNING id",
        )
        .bind(library)
        .bind(key)
        .bind(&title)
        .bind(&sort_title)
        .bind(&artist)
        .bind(tracks.iter().filter_map(|t| t.year).min())
        .bind(tracks.iter().find_map(|t| t.release_date.clone()))
        .bind(tracks.iter().find_map(|t| t.original_date.clone()))
        .bind(json(&genres))
        .bind(
            tracks
                .iter()
                .map(|t| list(&t.release_types))
                .find(|l| !l.is_empty())
                .map(|l| json(&l))
                .unwrap_or("[]".into()),
        )
        .bind(tracks.iter().map(|t| list(&t.labels)).find(|l| !l.is_empty()).map(|l| json(&l)).unwrap_or("[]".into()))
        .bind(json(&discs))
        .bind(compilation)
        .bind(&first.album_mbid)
        .bind(dir.to_string_lossy().to_string())
        .bind(cover)
        .bind(added)
        .fetch_one(&mut *tx)
        .await?;

        keep_albums.push(album_id);
        album_links.push((album_id, album_artists));

        for t in tracks.iter() {
            let mut names = list(&t.artists);

            if names.is_empty() && !t.artist.is_empty() {
                names.push(t.artist.clone());
            }

            let mbids = list(&t.artist_mbids);

            for (i, a) in names.iter().enumerate() {
                let sort = (names.len() == 1).then_some(t.artist_sort.as_ref()).flatten();
                note(a, sort, mbids.get(i));
            }

            let composers = list(&t.composers);

            for c in &composers {
                note(c, None, None);
            }

            sqlx::query("UPDATE tracks SET album_id = ? WHERE id = ?")
                .bind(album_id)
                .bind(t.id)
                .execute(&mut *tx)
                .await?;

            track_links.push((t.id, names, composers));
        }
    }

    let ids = keep_albums.iter().map(i64::to_string).collect::<Vec<_>>().join(",");

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM albums WHERE library = ? AND id NOT IN ({})",
        if ids.is_empty() { "-1".to_string() } else { ids }
    )))
    .bind(library)
    .execute(&mut *tx)
    .await?;

    let mut artist_ids: HashMap<String, i64> = HashMap::new();

    for (key, a) in &artists {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO artists (library, key, name, sort_name, mbid, added_at) VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(library, key) DO UPDATE SET name = excluded.name, sort_name = excluded.sort_name,
                mbid = excluded.mbid
             RETURNING id",
        )
        .bind(library)
        .bind(key)
        .bind(&a.name)
        .bind(a.sort.as_deref().map(str::to_lowercase).unwrap_or_else(|| artist_sort(&a.name)))
        .bind(&a.mbid)
        .bind(ts)
        .fetch_one(&mut *tx)
        .await?;

        artist_ids.insert(key.clone(), id);
    }

    let ids = artist_ids.values().map(i64::to_string).collect::<Vec<_>>().join(",");

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM artists WHERE library = ? AND id NOT IN ({})",
        if ids.is_empty() { "-1".to_string() } else { ids }
    )))
    .bind(library)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM album_artists WHERE album_id IN (SELECT id FROM albums WHERE library = ?)")
        .bind(library)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM track_artists WHERE track_id IN (SELECT id FROM tracks WHERE library = ?)")
        .bind(library)
        .execute(&mut *tx)
        .await?;

    for (album_id, names) in &album_links {
        for (i, n) in names.iter().enumerate() {
            sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id, position) VALUES (?, ?, ?)")
                .bind(album_id)
                .bind(artist_ids[&n.to_lowercase()])
                .bind(i as i64)
                .execute(&mut *tx)
                .await?;
        }
    }

    for (track_id, names, composers) in &track_links {
        for (role, list) in [("artist", names), ("composer", composers)] {
            for (i, n) in list.iter().enumerate() {
                sqlx::query(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role, position) VALUES (?, ?, ?, ?)",
                )
                .bind(track_id)
                .bind(artist_ids[&n.to_lowercase()])
                .bind(role)
                .bind(i as i64)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_folders_fold_into_the_release() {
        let (dir, disc) = release_dir(Path::new("/m/Artist/Album (2001)/CD2/01 Song.flac"));
        assert_eq!(dir, Path::new("/m/Artist/Album (2001)"));
        assert_eq!(disc, Some(2));
        let (dir, disc) = release_dir(Path::new("/m/Artist/Album/Disc 1/01.flac"));
        assert_eq!(dir, Path::new("/m/Artist/Album"));
        assert_eq!(disc, Some(1));
        let (dir, disc) = release_dir(Path::new("/m/Artist/Discovery/01.flac"));
        assert_eq!(dir, Path::new("/m/Artist/Discovery"));
        assert_eq!(disc, None);
    }

    #[test]
    fn numbered_file_names() {
        let c = NUMBERED.captures("03 - Song Name").unwrap();
        assert_eq!((&c["n"], &c["t"]), ("03", "Song Name"));
        let c = NUMBERED.captures("1-07 Song").unwrap();
        assert_eq!((&c["d"], &c["n"], &c["t"]), ("1", "07", "Song"));
        assert!(NUMBERED.captures("Song").is_none());
    }

    #[test]
    fn replaygain_values() {
        assert_eq!(leading_number("-6.54 dB"), Some(-6.54));
        assert_eq!(leading_number("+1.20 dB"), Some(1.2));
        assert_eq!(leading_number("0.988"), Some(0.988));
        assert_eq!(year_of("2004-05-01"), Some(2004));
        assert_eq!(year_of("abc"), None);
    }
}
