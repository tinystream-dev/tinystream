// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use serde_json::{Value, json};

use super::browse::{Id, folders, iso, parse_id, songs, visible_album, visible_artist, visible_track};
use super::{Call, Failure, Reply, Result};
use crate::db::now;
use crate::events::Event;
use crate::music::catalog::{self, Kind, Playlist};
use crate::music::queue::{self, Playing};
use crate::state::AppState;

fn playlist_json(p: &Playlist) -> Value {
    json!({
        "id": format!("pl-{}", p.id),
        "name": p.name,
        "comment": p.comment.clone().unwrap_or_default(),
        "owner": p.owner,
        "public": p.public,
        "songCount": p.track_count,
        "duration": p.duration.round() as i64,
        "created": iso(p.created_at),
        "changed": iso(p.updated_at),
        "coverArt": format!("pl-{}", p.id),
    })
}

fn playlist_id(call: &Call, key: &str) -> Result<i64> {
    match parse_id(call.params.require(key)?) {
        Some(Id::Playlist(n)) => Ok(n),
        _ => call.params.int(key).ok_or_else(|| Failure::not_found("playlist")),
    }
}

async fn track_paths(state: &AppState, call: &Call, ids: Vec<&str>) -> Result<Vec<String>> {
    let ids: Vec<i64> = ids
        .into_iter()
        .filter_map(|id| match parse_id(id) {
            Some(Id::Track(n)) => Some(n),
            _ => None,
        })
        .collect();

    let libs = crate::music::libraries(state, &call.user);
    Ok(catalog::tracks(&state.db, &libs, &ids).await?.into_iter().map(|t| t.path).collect())
}

pub async fn playlists(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let lists = catalog::playlists(&state.db, call.user.id).await?;
    Ok(Reply::one("playlists", json!({ "playlist": lists.iter().map(playlist_json).collect::<Vec<_>>() })))
}

async fn full(state: &AppState, call: &Call, id: i64) -> Result<Value> {
    let p = catalog::playlist(&state.db, call.user.id, id).await?.ok_or_else(|| Failure::not_found("playlist"))?;
    let paths = catalog::playlist_paths(&state.db, id).await?;
    let tracks = catalog::tracks_by_path(&state.db, &crate::music::libraries(state, &call.user), &paths).await?;
    let mut v = playlist_json(&p);
    v["entry"] = json!(songs(state, call.user.id, &tracks).await?);
    Ok(v)
}

pub async fn playlist(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let id = playlist_id(call, "id")?;
    Ok(Reply::one("playlist", full(state, call, id).await?))
}

async fn own(state: &AppState, call: &Call, id: i64) -> Result<Playlist> {
    let p = catalog::playlist(&state.db, call.user.id, id).await?.ok_or_else(|| Failure::not_found("playlist"))?;

    if p.owner_id != call.user.id {
        return Err(Failure::forbidden());
    }

    Ok(p)
}

pub async fn create_playlist(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let paths = track_paths(state, call, call.params.all("songId")).await?;

    let id = if call.params.get("playlistId").is_some() {
        let id = playlist_id(call, "playlistId")?;
        own(state, call, id).await?;
        let mut tx = state.db.begin().await?;
        catalog::set_playlist_paths(&mut tx, id, &paths).await?;
        tx.commit().await?;
        id
    } else {
        let name = call.params.require("name")?.trim();

        if name.is_empty() {
            return Err(Failure::missing("name"));
        }

        catalog::create_playlist(&state.db, call.user.id, name, &paths).await?
    };

    state.events.send(Event::PlaylistsChanged);
    Ok(Reply::one("playlist", full(state, call, id).await?))
}

pub async fn update_playlist(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let id = playlist_id(call, "playlistId")?;
    let p = own(state, call, id).await?;
    let mut paths = catalog::playlist_paths(&state.db, id).await?;
    let mut remove: Vec<usize> = call.params.all("songIndexToRemove").iter().filter_map(|i| i.parse().ok()).collect();
    remove.sort_unstable_by(|a, b| b.cmp(a));
    remove.dedup();

    for i in remove {
        if i < paths.len() {
            paths.remove(i);
        }
    }

    paths.extend(track_paths(state, call, call.params.all("songIdToAdd")).await?);
    let mut tx = state.db.begin().await?;

    sqlx::query("UPDATE playlists SET name = ?, comment = ?, public = ? WHERE id = ?")
        .bind(call.params.get("name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&p.name))
        .bind(call.params.get("comment").map(str::to_string).or(p.comment))
        .bind(call.params.flag("public").unwrap_or(p.public))
        .bind(id)
        .execute(&mut *tx)
        .await?;

    catalog::set_playlist_paths(&mut tx, id, &paths).await?;
    tx.commit().await?;
    state.events.send(Event::PlaylistsChanged);
    Ok(Reply::empty())
}

pub async fn delete_playlist(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let id = playlist_id(call, "id")?;
    own(state, call, id).await?;
    sqlx::query("DELETE FROM playlists WHERE id = ?").bind(id).execute(&state.db).await?;
    state.events.send(Event::PlaylistsChanged);
    Ok(Reply::empty())
}

/// What a star or rating is about; old apps send albums and artists as plain `id`s too.
async fn targets(state: &AppState, call: &Call) -> Result<Vec<(Kind, String)>> {
    let mut out = Vec::new();
    let ids = call.params.all("id").into_iter().chain(call.params.all("albumId")).chain(call.params.all("artistId"));

    for id in ids {
        match parse_id(id) {
            Some(Id::Track(_)) => out.push((Kind::Track, visible_track(state, call, id).await?.path)),
            Some(Id::Album(n)) => out.push((Kind::Album, visible_album(state, call, n).await?.target())),
            Some(Id::Artist(n)) => out.push((Kind::Artist, visible_artist(state, call, n).await?.target())),
            _ => return Err(Failure::not_found("item")),
        }
    }

    Ok(out)
}

pub async fn star(state: &Arc<AppState>, call: &Call, on: bool) -> Result<Reply> {
    for (kind, target) in targets(state, call).await? {
        catalog::set_star(&state.db, call.user.id, kind, &target, on).await?;
    }
    Ok(Reply::empty())
}

pub async fn set_rating(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let rating = call.params.int("rating").ok_or_else(|| Failure::missing("rating"))?;

    for (kind, target) in targets(state, call).await? {
        catalog::set_rating(&state.db, call.user.id, kind, &target, rating).await?;
    }

    Ok(Reply::empty())
}

pub async fn scrobble(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let submission = call.params.flag("submission").unwrap_or(true);
    let times: Vec<i64> = call.params.all("time").iter().filter_map(|t| t.parse::<i64>().ok()).collect();

    for (i, id) in call.params.all("id").into_iter().enumerate() {
        let t = visible_track(state, call, id).await?;

        if submission {
            let at = times.get(i).map(|ms| ms / 1000).unwrap_or_else(now);
            catalog::record_play(&state.db, call.user.id, &t.path, at).await?;
        } else {
            let playing = Playing {
                track_id: t.id,
                client: call.client.clone(),
                since: now(),
                position: 0.0,
                duration: t.duration,
                paused: false,
            };

            playback(state, call, Some(playing));
        }
    }

    Ok(Reply::empty())
}

pub async fn report_playback(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let p = &call.params;
    let t = visible_track(state, call, p.require("mediaId")?).await?;
    let position = p.int_or("positionMs", 0).max(0) as f64 / 1000.0;

    match p.require("state")? {
        "starting" | "playing" | "paused" => {
            let playing = Playing {
                track_id: t.id,
                client: call.client.clone(),
                since: now() - position as i64,
                position,
                duration: t.duration,
                paused: p.get("state") == Some("paused"),
            };

            playback(state, call, Some(playing));
        },
        "stopped" => {
            playback(state, call, None);

            let enough = position >= (t.duration / 2.0).min(240.0);

            if enough && !p.flag("ignoreScrobble").unwrap_or(false) {
                catalog::record_play(&state.db, call.user.id, &t.path, now() - position as i64).await?;
            }
        },
        other => return Err(Failure::new(10, format!("{other:?} isn't a playback state"))),
    }

    Ok(Reply::empty())
}

/// Records what an app is playing, and tells the web player so it can follow along.
fn playback(state: &AppState, call: &Call, playing: Option<Playing>) {
    state.events.send(Event::PlaybackChanged {
        user_id: call.user.id,
        client: call.client.clone(),
        track_id: playing.as_ref().map(|p| p.track_id),
        position: playing.as_ref().map_or(0.0, |p| p.position),
        paused: playing.as_ref().is_none_or(|p| p.paused),
    });

    state.music.playing.set(call.user.id, playing);
}

pub async fn play_queue(state: &Arc<AppState>, call: &Call, name: &str) -> Result<Reply> {
    let q = queue::load(&state.db, call.user.id).await?;
    let libs = crate::music::libraries(state, &call.user);
    let mut unique = q.tracks.clone();
    unique.sort_unstable();
    unique.dedup();
    let found = catalog::tracks(&state.db, &libs, &unique).await?;
    let mut kept = Vec::new();
    let mut current = 0usize;

    for (i, id) in q.tracks.iter().enumerate() {
        if let Some(t) = found.iter().find(|t| t.id == *id) {
            if i == q.current {
                current = kept.len();
            }

            kept.push(t.clone());
        }
    }

    if kept.is_empty() {
        return Ok(Reply::empty());
    }

    let mut v = json!({
        "position": (q.position * 1000.0) as i64,
        "username": call.user.username,
        "changed": iso(q.updated_at),
        "changedBy": q.changed_by.unwrap_or_default(),
        "entry": songs(state, call.user.id, &kept).await?,
    });

    if name == "getPlayQueueByIndex" {
        v["currentIndex"] = json!(current);
        Ok(Reply::one("playQueueByIndex", v))
    } else {
        v["current"] = json!(format!("tr-{}", kept[current].id));
        Ok(Reply::one("playQueue", v))
    }
}

pub async fn save_play_queue(state: &Arc<AppState>, call: &Call, name: &str) -> Result<Reply> {
    let p = &call.params;

    let ids: Vec<i64> = p
        .all("id")
        .into_iter()
        .filter_map(|id| match parse_id(id) {
            Some(Id::Track(n)) => Some(n),
            _ => None,
        })
        .collect();

    let current = if name == "savePlayQueueByIndex" {
        p.int_or("currentIndex", 0).max(0) as usize
    } else {
        match p.get("current").and_then(parse_id) {
            Some(Id::Track(n)) => ids.iter().position(|id| *id == n).unwrap_or(0),
            _ => 0,
        }
    };

    let old = queue::load(&state.db, call.user.id).await?;

    let q = queue::Queue {
        tracks: ids,
        current,
        position: p.int_or("position", 0).max(0) as f64 / 1000.0,
        shuffled: old.shuffled,
        repeat: old.repeat,
        changed_by: Some(call.client.clone()),
        updated_at: 0,
    };

    queue::save(state, call.user.id, q).await?;
    Ok(Reply::empty())
}

fn user_json(state: &AppState, user: &crate::auth::User) -> Value {
    let config = state.config.current();

    let folders: Vec<usize> = config
        .libraries
        .iter()
        .filter(|l| l.is_music())
        .enumerate()
        .filter(|(_, l)| user.permissions.can_see(&l.name))
        .map(|(i, _)| i + 1)
        .collect();

    json!({
        "username": user.username,
        "email": "",
        "scrobblingEnabled": true,
        "adminRole": user.is_admin,
        "settingsRole": false,
        "downloadRole": true,
        "uploadRole": false,
        "playlistRole": true,
        "coverArtRole": true,
        "commentRole": false,
        "podcastRole": false,
        "streamRole": true,
        "jukeboxRole": false,
        "shareRole": false,
        "videoConversionRole": false,
        "folder": folders,
    })
}

pub async fn user(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let name = call.params.get("username").unwrap_or(&call.user.username);

    if !name.eq_ignore_ascii_case(&call.user.username) {
        if !call.user.is_admin {
            return Err(Failure::forbidden());
        }

        let id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
            .bind(name)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| Failure::not_found("user"))?;

        let u = crate::auth::load_user(state, id).await?.ok_or_else(|| Failure::not_found("user"))?;
        return Ok(Reply::one("user", user_json(state, &u)));
    }

    Ok(Reply::one("user", user_json(state, &call.user)))
}

pub async fn users(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    if !call.user.is_admin {
        return Err(Failure::forbidden());
    }

    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM users ORDER BY username").fetch_all(&state.db).await?;
    let mut list = Vec::new();

    for id in ids {
        if let Some(u) = crate::auth::load_user(state, id).await? {
            list.push(user_json(state, &u));
        }
    }

    Ok(Reply::one("users", json!({ "user": list })))
}

pub async fn scan_status(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = folders(state, call)?;
    let mut count = 0i64;

    for l in &libs {
        count += sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tracks WHERE library = ?")
            .bind(l)
            .fetch_one(&state.db)
            .await?;
    }

    Ok(Reply::one("scanStatus", json!({ "scanning": false, "count": count })))
}

pub async fn start_scan(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    if !call.user.is_admin {
        return Err(Failure::forbidden());
    }
    for l in crate::music::libraries(state, &call.user) {
        state.scanner.request(&l);
    }
    Ok(Reply::one("scanStatus", json!({ "scanning": true, "count": 0 })))
}
