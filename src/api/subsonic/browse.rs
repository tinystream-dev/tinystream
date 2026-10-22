// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde_json::{Value, json};

use super::{Call, Failure, Reply, Result};
use crate::music::catalog::{self, Album, AlbumFilter, AlbumOrder, Artist, Kind, Track, UserData};
use crate::state::AppState;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

pub fn iso(ts: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(ts)
        .ok()
        .and_then(|t| t.format(&time::format_description::well_known::Rfc3339).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Id {
    Track(i64),
    Album(i64),
    Artist(i64),
    Playlist(i64),
    Folder(String, PathBuf),
}

pub fn parse_id(s: &str) -> Option<Id> {
    let (prefix, rest) = s.split_once('-')?;

    match prefix {
        "tr" => rest.parse().ok().map(Id::Track),
        "al" => rest.parse().ok().map(Id::Album),
        "ar" => rest.parse().ok().map(Id::Artist),
        "pl" => rest.parse().ok().map(Id::Playlist),
        "fo" => {
            let raw = String::from_utf8(B64.decode(rest).ok()?).ok()?;
            let (lib, rel) = raw.split_once('\0')?;
            Some(Id::Folder(lib.to_string(), PathBuf::from(rel)))
        },
        _ => None,
    }
}

pub fn folder_id(library: &str, rel: &Path) -> String {
    format!("fo-{}", B64.encode(format!("{library}\0{}", rel.display())))
}

fn date(s: Option<&str>) -> Option<Value> {
    let parts: Vec<i64> = s?
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .take(3)
        .filter_map(|p| p.parse().ok())
        .collect();

    let year = *parts.first().filter(|y| **y >= 1000)?;
    let mut v = json!({ "year": year });
    put(&mut v, "month", parts.get(1).filter(|m| (1..=12).contains(*m)).map(|m| json!(m)));
    put(&mut v, "day", parts.get(2).filter(|d| (1..=31).contains(*d)).map(|d| json!(d)));
    Some(v)
}

fn put(v: &mut Value, key: &str, value: Option<Value>) {
    if let Some(x) = value {
        v[key] = x;
    }
}

fn user_fields(v: &mut Value, d: &UserData) {
    put(v, "starred", d.starred.map(|t| json!(iso(t))));
    put(v, "userRating", d.rating.map(|r| json!(r)));
    put(v, "played", d.played.map(|t| json!(iso(t))));
}

/// The music libraries someone sees, narrowed to one when `musicFolderId` says so.
pub fn folders(state: &AppState, call: &Call) -> Result<Vec<String>> {
    let all = crate::music::libraries(state, &call.user);

    match call.params.int("musicFolderId") {
        Some(n) => {
            let config = state.config.current();

            let name =
                config.libraries.iter().filter(|l| l.is_music()).nth((n - 1).max(0) as usize).map(|l| l.name.clone());

            match name {
                Some(n) if all.contains(&n) => Ok(vec![n]),
                _ => Err(Failure::not_found("music folder")),
            }
        },
        None => Ok(all),
    }
}

fn roots(state: &AppState) -> HashMap<String, PathBuf> {
    let config = state.config.current();

    config
        .libraries
        .iter()
        .filter(|l| l.is_music())
        .filter_map(|l| Some((l.name.clone(), l.resolved_path(state.config.config_dir()).ok()?)))
        .collect()
}

/// Songs as apps want them, with everyone's ids and the asker's stars and plays.
pub async fn songs(state: &AppState, user_id: i64, tracks: &[Track]) -> Result<Vec<Value>> {
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    let paths: Vec<String> = tracks.iter().map(|t| t.path.clone()).collect();
    let artists = catalog::track_artists(&state.db, &ids).await?;
    let mut album_ids: Vec<i64> = tracks.iter().filter_map(|t| t.album_id).collect();
    album_ids.sort_unstable();
    album_ids.dedup();
    let album_artists = catalog::album_artists(&state.db, &album_ids).await?;
    let data = catalog::user_data(&state.db, user_id, Kind::Track, &paths).await?;
    let roots = roots(state);

    Ok(tracks
        .iter()
        .map(|t| {
            let credits = artists.get(&t.id).cloned().unwrap_or_default();

            let performers: Vec<Value> = credits
                .iter()
                .filter(|a| a.2 == "artist")
                .map(|a| json!({ "id": format!("ar-{}", a.0), "name": a.1 }))
                .collect();

            let composers: Vec<&(i64, String, String)> = credits.iter().filter(|a| a.2 == "composer").collect();

            let album_artists: Vec<Value> = t
                .album_id
                .and_then(|a| album_artists.get(&a))
                .map(|l| l.iter().map(|(id, name)| json!({ "id": format!("ar-{id}"), "name": name })).collect())
                .unwrap_or_default();

            let genres = t.genres();

            let rel = roots
                .get(&t.library)
                .and_then(|r| Path::new(&t.path).strip_prefix(r).ok())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| Path::new(&t.path).file_name().unwrap_or_default().to_string_lossy().to_string());

            let g = t.gains();

            let mut v = json!({
                "id": format!("tr-{}", t.id),
                "isDir": false,
                "title": t.title,
                "album": t.album,
                "artist": t.artist,
                "size": t.size,
                "contentType": t.content_type(),
                "suffix": t.suffix,
                "duration": t.duration.round() as i64,
                "path": rel,
                "isVideo": false,
                "created": iso(t.added_at),
                "type": "music",
                "mediaType": "song",
                "sortName": t.title.to_lowercase(),
                "genres": genres.iter().map(|g| json!({ "name": g })).collect::<Vec<_>>(),
                "artists": performers,
                "displayArtist": t.artist,
                "albumArtists": album_artists,
                "displayAlbumArtist": t.album_artist.clone().unwrap_or_default(),
                "contributors": composers.iter().map(|c| json!({
                    "role": "composer",
                    "artist": { "id": format!("ar-{}", c.0), "name": c.1 },
                })).collect::<Vec<_>>(),
                "displayComposer": composers.iter().map(|c| c.1.as_str()).collect::<Vec<_>>().join(", "),
                "moods": [],
                "isrc": [],
                "explicitStatus": "",
                "playCount": 0,
                "replayGain": {},
            });

            put(&mut v, "parent", t.album_id.map(|a| json!(format!("al-{a}"))));
            put(&mut v, "albumId", t.album_id.map(|a| json!(format!("al-{a}"))));

            put(
                &mut v,
                "coverArt",
                Some(json!(match t.album_id {
                    Some(a) if !t.embedded_art => format!("al-{a}"),
                    _ => format!("tr-{}", t.id),
                })),
            );

            put(&mut v, "artistId", credits.iter().find(|a| a.2 == "artist").map(|a| json!(format!("ar-{}", a.0))));
            put(&mut v, "track", t.number.map(|n| json!(n)));
            put(&mut v, "discNumber", t.disc.map(|n| json!(n)));
            put(&mut v, "year", t.year.map(|n| json!(n)));
            put(&mut v, "genre", genres.first().map(|g| json!(g)));
            put(&mut v, "bitRate", t.bitrate.map(|n| json!(n)));
            put(&mut v, "bitDepth", t.bit_depth.map(|n| json!(n)));
            put(&mut v, "samplingRate", t.sample_rate.map(|n| json!(n)));
            put(&mut v, "channelCount", t.channels.map(|n| json!(n)));
            put(&mut v, "bpm", t.bpm.map(|n| json!(n)));
            put(&mut v, "comment", t.comment.as_ref().map(|c| json!(c)));
            put(&mut v, "musicBrainzId", t.mbid.as_ref().map(|c| json!(c)));
            let mut rg = json!({});
            put(&mut rg, "trackGain", g.track_gain.map(|x| json!(x)));
            put(&mut rg, "trackPeak", g.track_peak.map(|x| json!(x)));
            put(&mut rg, "albumGain", g.album_gain.map(|x| json!(x)));
            put(&mut rg, "albumPeak", g.album_peak.map(|x| json!(x)));
            v["replayGain"] = rg;

            if let Some(d) = data.get(&t.path) {
                v["playCount"] = json!(d.play_count);
                user_fields(&mut v, d);
            }

            v
        })
        .collect())
}

/// Albums, as ID3 albums or as folders (`child`) for the older endpoints.
pub async fn albums(state: &AppState, user_id: i64, albums: &[Album], as_child: bool) -> Result<Vec<Value>> {
    let ids: Vec<i64> = albums.iter().map(|a| a.id).collect();
    let targets: Vec<String> = albums.iter().map(Album::target).collect();
    let data = catalog::user_data(&state.db, user_id, Kind::Album, &targets).await?;
    let plays = catalog::album_plays(&state.db, user_id, &ids).await?;
    let artists = catalog::album_artists(&state.db, &ids).await?;

    Ok(albums
        .iter()
        .map(|a| {
            let credits = artists.get(&a.id).cloned().unwrap_or_default();
            let genres = a.genres();

            let mut v = if as_child {
                json!({
                    "id": format!("al-{}", a.id),
                    "isDir": true,
                    "title": a.title,
                    "album": a.title,
                    "name": a.title,
                    "artist": a.artist,
                    "coverArt": format!("al-{}", a.id),
                    "created": iso(a.added_at),
                    "songCount": a.track_count,
                    "duration": a.duration.round() as i64,
                })
            } else {
                json!({
                    "id": format!("al-{}", a.id),
                    "name": a.title,
                    "artist": a.artist,
                    "coverArt": format!("al-{}", a.id),
                    "songCount": a.track_count,
                    "duration": a.duration.round() as i64,
                    "created": iso(a.added_at),
                    "genres": genres.iter().map(|g| json!({ "name": g })).collect::<Vec<_>>(),
                    "artists": credits.iter().map(|(id, name)| json!({ "id": format!("ar-{id}"), "name": name })).collect::<Vec<_>>(),
                    "displayArtist": a.artist,
                    "releaseTypes": catalog::list(&a.release_types),
                    "recordLabels": catalog::list(&a.labels).iter().map(|l| json!({ "name": l })).collect::<Vec<_>>(),
                    "isCompilation": a.compilation,
                    "sortName": a.sort_title,
                    "discTitles": a.disc_titles().into_iter().map(|(disc, title)| json!({ "disc": disc, "title": title })).collect::<Vec<_>>(),
                    "moods": [],
                    "explicitStatus": "",
                    "playCount": 0,
                })
            };

            put(&mut v, "artistId", credits.first().map(|(id, _)| json!(format!("ar-{id}"))));

            if as_child {
                put(&mut v, "parent", credits.first().map(|(id, _)| json!(format!("ar-{id}"))));
            }

            put(&mut v, "year", a.year.map(|y| json!(y)));
            put(&mut v, "genre", genres.first().map(|g| json!(g)));
            put(&mut v, "musicBrainzId", a.mbid.as_ref().map(|m| json!(m)));

            if !as_child {
                put(&mut v, "releaseDate", date(a.release_date.as_deref()));
                put(&mut v, "originalReleaseDate", date(a.original_date.as_deref()));
            }

            if let Some((n, at)) = plays.get(&a.id) {
                v["playCount"] = json!(n);
                v["played"] = json!(iso(*at));
            }

            if let Some(d) = data.get(&a.target()) {
                put(&mut v, "starred", d.starred.map(|t| json!(iso(t))));
                put(&mut v, "userRating", d.rating.map(|r| json!(r)));
            }

            v
        })
        .collect())
}

pub async fn artist_list(state: &AppState, user_id: i64, artists: &[Artist]) -> Result<Vec<Value>> {
    let targets: Vec<String> = artists.iter().map(Artist::target).collect();
    let data = catalog::user_data(&state.db, user_id, Kind::Artist, &targets).await?;

    Ok(artists
        .iter()
        .map(|r| {
            let mut roles = Vec::new();

            if r.album_count > 0 {
                roles.push("albumartist");
            }

            if r.track_count > 0 {
                roles.push("artist");
            }

            let mut v = json!({
                "id": format!("ar-{}", r.id),
                "name": r.name,
                "albumCount": r.album_count,
                "sortName": r.sort_name,
                "roles": roles,
            });

            put(&mut v, "coverArt", r.cover_album.map(|_| json!(format!("ar-{}", r.id))));
            put(&mut v, "musicBrainzId", r.mbid.as_ref().map(|m| json!(m)));

            if let Some(d) = data.get(&r.target()) {
                put(&mut v, "starred", d.starred.map(|t| json!(iso(t))));
                put(&mut v, "userRating", d.rating.map(|x| json!(x)));
            }

            v
        })
        .collect())
}

pub async fn visible_track(state: &AppState, call: &Call, id: &str) -> Result<Track> {
    let Some(Id::Track(n)) = parse_id(id) else { return Err(Failure::not_found("song")) };
    let t = catalog::track(&state.db, n).await?.ok_or_else(|| Failure::not_found("song"))?;

    if !crate::music::libraries(state, &call.user).contains(&t.library) {
        return Err(Failure::not_found("song"));
    }

    Ok(t)
}

pub async fn visible_album(state: &AppState, call: &Call, n: i64) -> Result<Album> {
    let a = catalog::album(&state.db, n).await?.ok_or_else(|| Failure::not_found("album"))?;

    if !crate::music::libraries(state, &call.user).contains(&a.library) {
        return Err(Failure::not_found("album"));
    }

    Ok(a)
}

pub async fn visible_artist(state: &AppState, call: &Call, n: i64) -> Result<Artist> {
    let r = catalog::artist(&state.db, n).await?.ok_or_else(|| Failure::not_found("artist"))?;

    if !crate::music::libraries(state, &call.user).contains(&r.library) {
        return Err(Failure::not_found("artist"));
    }

    Ok(r)
}

pub async fn music_folders(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let visible = crate::music::libraries(state, &call.user);
    let config = state.config.current();

    let list: Vec<Value> = config
        .libraries
        .iter()
        .filter(|l| l.is_music())
        .enumerate()
        .filter(|(_, l)| visible.contains(&l.name))
        .map(|(i, l)| json!({ "id": i + 1, "name": l.name }))
        .collect();

    Ok(Reply::one("musicFolders", json!({ "musicFolder": list })))
}

fn letter(sort_name: &str) -> String {
    match sort_name.chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
        _ => "#".into(),
    }
}

const IGNORED: &str = "The A An";

pub async fn artists(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = folders(state, call)?;
    let list = catalog::album_artists_index(&state.db, &libs).await?;
    let json = artist_list(state, call.user.id, &list).await?;
    let mut index: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    for (r, v) in list.iter().zip(json) {
        index.entry(letter(&r.sort_name)).or_default().push(v);
    }

    let index: Vec<Value> = index.into_iter().map(|(name, artist)| json!({ "name": name, "artist": artist })).collect();
    Ok(Reply::one("artists", json!({ "ignoredArticles": IGNORED, "index": index })))
}

/// Folders straight under each library: what file browsers start with.
pub async fn indexes(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = folders(state, call)?;
    let roots = roots(state);
    let mut index: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut loose = Vec::new();

    for lib in &libs {
        let Some(root) = roots.get(lib) else { continue };
        let (dirs, tracks) = children(state, lib, root, Path::new("")).await?;

        for (name, rel) in dirs {
            let sort = crate::library::music::artist_sort(&name);
            index.entry(letter(&sort)).or_default().push(json!({ "id": folder_id(lib, &rel), "name": name }));
        }

        loose.extend(tracks);
    }

    let index: Vec<Value> = index.into_iter().map(|(name, artist)| json!({ "name": name, "artist": artist })).collect();
    let changed: Option<i64> = sqlx::query_scalar("SELECT MAX(added_at) FROM tracks").fetch_one(&state.db).await?;

    Ok(Reply::one(
        "indexes",
        json!({
            "lastModified": changed.unwrap_or(0) * 1000,
            "ignoredArticles": IGNORED,
            "index": index,
            "child": songs(state, call.user.id, &loose).await?,
        }),
    ))
}

/// What's directly inside a folder of a library: folders with music somewhere in them, and tracks.
async fn children(
    state: &AppState,
    library: &str,
    root: &Path,
    rel: &Path,
) -> Result<(Vec<(String, PathBuf)>, Vec<Track>)> {
    let dir = root.join(rel);
    let prefix = format!("{}/", dir.display().to_string().trim_end_matches('/'));

    let paths: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, path FROM tracks WHERE library = ? AND substr(path, 1, length(?2)) = ?2 ORDER BY path",
    )
    .bind(library)
    .bind(&prefix)
    .fetch_all(&state.db)
    .await?;

    let mut dirs: Vec<(String, PathBuf)> = Vec::new();
    let mut here = Vec::new();

    for (id, p) in paths {
        let rest = &p[prefix.len()..];

        match rest.split_once('/') {
            Some((first, _)) => {
                if dirs.last().is_none_or(|(n, _)| n != first) {
                    dirs.push((first.to_string(), rel.join(first)));
                }
            },
            None => here.push(id),
        }
    }

    let tracks = catalog::tracks(&state.db, &[library.to_string()], &here).await?;
    Ok((dirs, tracks))
}

pub async fn music_directory(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let id = call.params.require("id")?;
    let libs = crate::music::libraries(state, &call.user);

    let dir = match parse_id(id) {
        Some(Id::Folder(lib, rel)) => {
            if !libs.contains(&lib) || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(Failure::not_found("folder"));
            }

            let root = roots(state).remove(&lib).ok_or_else(|| Failure::not_found("folder"))?;
            let (dirs, tracks) = children(state, &lib, &root, &rel).await?;
            let me = folder_id(&lib, &rel);
            let mut child: Vec<Value> = Vec::new();

            for (name, sub) in dirs {
                let album: Option<i64> = sqlx::query_scalar("SELECT id FROM albums WHERE library = ? AND dir = ?")
                    .bind(&lib)
                    .bind(root.join(&sub).display().to_string())
                    .fetch_optional(&state.db)
                    .await?;

                let mut v = json!({ "id": folder_id(&lib, &sub), "parent": me, "isDir": true, "title": name });
                put(&mut v, "coverArt", album.map(|a| json!(format!("al-{a}"))));
                child.push(v);
            }

            let mut songs = songs(state, call.user.id, &tracks).await?;

            for s in &mut songs {
                s["parent"] = json!(me);
            }

            child.extend(songs);

            let mut v = json!({
                "id": me,
                "name": rel.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(lib.clone()),
                "child": child,
            });

            if let Some(parent) = rel.parent().filter(|_| !rel.as_os_str().is_empty()) {
                v["parent"] = json!(folder_id(&lib, parent));
            }

            v
        },
        Some(Id::Album(n)) => {
            let a = visible_album(state, call, n).await?;
            let tracks = catalog::album_tracks(&state.db, a.id).await?;
            let mut v = json!({ "id": id, "name": a.title, "child": songs(state, call.user.id, &tracks).await? });
            let artists = catalog::album_artists(&state.db, &[a.id]).await?;
            put(&mut v, "parent", artists.get(&a.id).and_then(|l| l.first()).map(|(r, _)| json!(format!("ar-{r}"))));
            v
        },
        Some(Id::Artist(n)) => {
            let r = visible_artist(state, call, n).await?;
            let (own, _) = catalog::artist_albums(&state.db, r.id).await?;
            json!({ "id": id, "name": r.name, "child": albums(state, call.user.id, &own, true).await? })
        },
        _ => return Err(Failure::not_found("folder")),
    };

    Ok(Reply::one("directory", dir))
}

pub async fn genres(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = folders(state, call)?;

    let list: Vec<Value> = catalog::genres(&state.db, &libs)
        .await?
        .into_iter()
        .map(|(name, songs, albums)| json!({ "value": name, "songCount": songs, "albumCount": albums }))
        .collect();

    Ok(Reply::one("genres", json!({ "genre": list })))
}

pub async fn artist(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let Some(Id::Artist(n)) = parse_id(call.params.require("id")?) else { return Err(Failure::not_found("artist")) };
    let r = visible_artist(state, call, n).await?;
    let (own, _) = catalog::artist_albums(&state.db, r.id).await?;
    let mut v = artist_list(state, call.user.id, std::slice::from_ref(&r)).await?.remove(0);
    v["album"] = json!(albums(state, call.user.id, &own, false).await?);
    Ok(Reply::one("artist", v))
}

pub async fn album(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let Some(Id::Album(n)) = parse_id(call.params.require("id")?) else { return Err(Failure::not_found("album")) };
    let a = visible_album(state, call, n).await?;
    let tracks = catalog::album_tracks(&state.db, a.id).await?;
    let mut v = albums(state, call.user.id, std::slice::from_ref(&a), false).await?.remove(0);
    v["song"] = json!(songs(state, call.user.id, &tracks).await?);
    Ok(Reply::one("album", v))
}

pub async fn song(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let t = visible_track(state, call, call.params.require("id")?).await?;
    Ok(Reply::one("song", songs(state, call.user.id, &[t]).await?.remove(0)))
}

pub async fn artist_info(state: &Arc<AppState>, call: &Call, name: &str) -> Result<Reply> {
    let id = call.params.require("id")?;

    let mbid = match parse_id(id) {
        Some(Id::Artist(n)) => visible_artist(state, call, n).await?.mbid,
        _ => None,
    };

    let mut v = json!({ "biography": "" });
    put(&mut v, "musicBrainzId", mbid.map(|m| json!(m)));
    Ok(Reply::one(if name == "getArtistInfo2" { "artistInfo2" } else { "artistInfo" }, v))
}

pub async fn album_info(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let Some(Id::Album(n)) = parse_id(call.params.require("id")?) else { return Err(Failure::not_found("album")) };
    let a = visible_album(state, call, n).await?;
    let mut v = json!({ "notes": "" });
    put(&mut v, "musicBrainzId", a.mbid.map(|m| json!(m)));
    Ok(Reply::one("albumInfo", v))
}

pub async fn similar_songs(state: &Arc<AppState>, call: &Call, name: &str) -> Result<Reply> {
    let libs = crate::music::libraries(state, &call.user);
    let count = call.params.int_or("count", 50).clamp(1, 500);
    let id = call.params.require("id")?;

    let (artists, genres, exclude) = match parse_id(id) {
        Some(Id::Track(_)) => {
            let t = visible_track(state, call, id).await?;
            let a = catalog::track_artists(&state.db, &[t.id]).await?.remove(&t.id).unwrap_or_default();
            (a.into_iter().map(|a| a.0).collect::<Vec<_>>(), t.genres(), vec![t.id])
        },
        Some(Id::Album(n)) => {
            let a = visible_album(state, call, n).await?;

            let ids: Vec<i64> = catalog::album_artists(&state.db, &[a.id])
                .await?
                .remove(&a.id)
                .unwrap_or_default()
                .into_iter()
                .map(|x| x.0)
                .collect();

            (ids, a.genres(), Vec::new())
        },
        Some(Id::Artist(n)) => {
            let r = visible_artist(state, call, n).await?;

            let genres: Vec<String> = sqlx::query_scalar(
                "SELECT DISTINCT g.value FROM track_artists ta JOIN tracks t ON t.id = ta.track_id, json_each(t.genres) g
                 WHERE ta.artist_id = ? LIMIT 5",
            )
            .bind(r.id)
            .fetch_all(&state.db)
            .await?;

            (vec![r.id], genres, Vec::new())
        },
        _ => return Err(Failure::not_found("song")),
    };

    let tracks = catalog::similar_tracks(&state.db, &libs, &artists, &genres, &exclude, count).await?;
    let key = if name == "getSimilarSongs2" { "similarSongs2" } else { "similarSongs" };
    Ok(Reply::one(key, json!({ "song": songs(state, call.user.id, &tracks).await? })))
}

pub async fn top_songs(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = crate::music::libraries(state, &call.user);
    let count = call.params.int_or("count", 50).clamp(1, 500);

    let artist = match call.params.get("id").and_then(parse_id) {
        Some(Id::Artist(n)) => Some(visible_artist(state, call, n).await?.id),
        _ => match call.params.get("artist") {
            Some(name) => catalog::artist_by_name(&state.db, &libs, name).await?,
            None => return Err(Failure::missing("artist")),
        },
    };

    let tracks = match artist {
        Some(id) => catalog::top_tracks(&state.db, &libs, id, count).await?,
        None => Vec::new(),
    };

    Ok(Reply::one("topSongs", json!({ "song": songs(state, call.user.id, &tracks).await? })))
}

pub async fn album_list(state: &Arc<AppState>, call: &Call, name: &str) -> Result<Reply> {
    let libs = folders(state, call)?;
    let p = &call.params;
    let kind = p.require("type")?;
    let size = p.int_or("size", 10).clamp(1, 500);
    let offset = p.int_or("offset", 0).max(0);
    let mut filter = AlbumFilter::default();

    let order = match kind {
        "random" => AlbumOrder::Random,
        "newest" => AlbumOrder::Newest,
        "highest" => AlbumOrder::Rated,
        "frequent" => AlbumOrder::Frequent,
        "recent" => AlbumOrder::Recent,
        "alphabeticalByName" => AlbumOrder::Name,
        "alphabeticalByArtist" => AlbumOrder::Artist,
        "starred" => AlbumOrder::Starred,
        "byYear" => {
            filter.years = Some((p.int_or("fromYear", 0), p.int_or("toYear", 9999)));
            AlbumOrder::Year
        },
        "byGenre" => {
            filter.genre = Some(p.require("genre")?.to_string());
            AlbumOrder::Name
        },
        other => return Err(Failure::new(0, format!("{other:?} isn't a kind of album list"))),
    };

    let list = catalog::albums(&state.db, &libs, call.user.id, order, &filter, offset, size).await?;
    let id3 = name == "getAlbumList2";
    let key = if id3 { "albumList2" } else { "albumList" };
    Ok(Reply::one(key, json!({ "album": albums(state, call.user.id, &list, !id3).await? })))
}

pub async fn random_songs(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = folders(state, call)?;
    let p = &call.params;

    let tracks = catalog::random_tracks(
        &state.db,
        &libs,
        p.int_or("size", 10).clamp(1, 500),
        p.get("genre"),
        (p.int("fromYear"), p.int("toYear")),
    )
    .await?;

    Ok(Reply::one("randomSongs", json!({ "song": songs(state, call.user.id, &tracks).await? })))
}

pub async fn songs_by_genre(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = folders(state, call)?;
    let p = &call.params;

    let tracks = catalog::genre_tracks(
        &state.db,
        &libs,
        p.require("genre")?,
        p.int_or("offset", 0).max(0),
        p.int_or("count", 10).clamp(1, 500),
    )
    .await?;

    Ok(Reply::one("songsByGenre", json!({ "song": songs(state, call.user.id, &tracks).await? })))
}

pub async fn starred(state: &Arc<AppState>, call: &Call, name: &str) -> Result<Reply> {
    let libs = folders(state, call)?;
    let (a, b, t) = catalog::starred(&state.db, &libs, call.user.id).await?;
    let id3 = name == "getStarred2";

    let v = json!({
        "artist": artist_list(state, call.user.id, &a).await?,
        "album": albums(state, call.user.id, &b, !id3).await?,
        "song": songs(state, call.user.id, &t).await?,
    });

    Ok(Reply::one(if id3 { "starred2" } else { "starred" }, v))
}

pub async fn search(state: &Arc<AppState>, call: &Call, name: &str) -> Result<Reply> {
    let libs = folders(state, call)?;
    let p = &call.params;
    let query = p.get("query").unwrap_or("");
    let query = if query.trim() == "\"\"" { "" } else { query };
    let count = |k: &str| p.int_or(k, 20).clamp(0, 5000);
    let offset = |k: &str| p.int_or(k, 0).max(0);
    let id3 = name == "search3";
    let mut v = json!({});
    let n = count("artistCount");

    if n > 0 {
        let list = catalog::search_artists(&state.db, &libs, query, offset("artistOffset"), n).await?;
        v["artist"] = json!(artist_list(state, call.user.id, &list).await?);
    }

    let n = count("albumCount");

    if n > 0 {
        let list = catalog::search_albums(&state.db, &libs, query, offset("albumOffset"), n).await?;
        v["album"] = json!(albums(state, call.user.id, &list, !id3).await?);
    }

    let n = count("songCount");

    if n > 0 {
        let list = catalog::search_tracks(&state.db, &libs, query, offset("songOffset"), n).await?;
        v["song"] = json!(songs(state, call.user.id, &list).await?);
    }

    Ok(Reply::one(if id3 { "searchResult3" } else { "searchResult2" }, v))
}

pub async fn now_playing(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = crate::music::libraries(state, &call.user);
    let mut entries = Vec::new();

    for (user_id, playing) in state.music.playing.all().into_iter().filter(|(_, p)| !p.paused) {
        let Some(t) = catalog::tracks(&state.db, &libs, &[playing.track_id]).await?.pop() else { continue };

        let username: Option<String> = sqlx::query_scalar("SELECT username FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;

        let mut v = songs(state, call.user.id, &[t]).await?.remove(0);
        v["username"] = json!(username.unwrap_or_default());
        v["minutesAgo"] = json!((crate::db::now() - playing.since) / 60);
        v["playerId"] = json!(user_id);
        v["playerName"] = json!(playing.client);
        entries.push(v);
    }

    Ok(Reply::one("nowPlaying", json!({ "entry": entries })))
}
