// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use axum::extract::Request;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::music::{image, serve_track, track_cover_file, transcoded};
use super::super::users;
use super::browse::{Id, parse_id, visible_album, visible_artist, visible_track};
use super::{Call, Failure, Reply, Result};
use crate::media::audio::Format;
use crate::music::catalog::{self, Track};
use crate::music::{art, lyrics};
use crate::state::AppState;

/// The original, unless the app asked for something else: a format, or fewer kbit/s than the file
/// has.
fn wanted(track: &Track, format: Option<&str>, max_kbps: Option<i64>) -> Option<(Format, Option<u32>)> {
    let max = max_kbps.filter(|&b| b > 0).map(|b| b as u32);

    match format.map(str::trim).filter(|f| !f.is_empty()) {
        Some("raw") => None,
        Some(f)
            if f.eq_ignore_ascii_case(&track.suffix)
                && max.is_none_or(|m| track.bitrate.is_none_or(|b| b <= m as i64)) =>
        {
            None
        },
        Some(f) => Format::parse(f).map(|fmt| (fmt, max)),
        None => match max {
            Some(m) if track.bitrate.is_some_and(|b| b > m as i64) => Some((Format::Mp3, Some(m))),
            _ => None,
        },
    }
}

pub async fn stream(state: &Arc<AppState>, call: &Call, req: Option<Request>) -> Result<Reply> {
    let t = visible_track(state, call, call.params.require("id")?).await?;
    let p = &call.params;
    let offset = p.get("timeOffset").and_then(|o| o.parse::<f64>().ok()).unwrap_or(0.0);

    match wanted(&t, p.get("format"), p.int("maxBitRate")) {
        Some((format, bitrate)) => Ok(Reply::Raw(transcoded(
            state,
            &format!("{} ({})", call.user.username, call.client),
            &t,
            format,
            bitrate,
            offset,
        ))),
        None if offset > 0.0 => Ok(Reply::Raw(transcoded(
            state,
            &format!("{} ({})", call.user.username, call.client),
            &t,
            Format::Flac,
            None,
            offset,
        ))),
        None => {
            let req = req.ok_or_else(|| Failure::new(0, "no request"))?;
            Ok(Reply::Raw(serve_track(&t, req).await?))
        },
    }
}

pub async fn download(state: &Arc<AppState>, call: &Call, req: Option<Request>) -> Result<Reply> {
    let t = visible_track(state, call, call.params.require("id")?).await?;
    let req = req.ok_or_else(|| Failure::new(0, "no request"))?;
    let mut res = serve_track(&t, req).await?;
    let name = std::path::Path::new(&t.path).file_name().unwrap_or_default().to_string_lossy().replace('"', "");

    if let Ok(v) = format!("attachment; filename=\"{name}\"").parse() {
        res.headers_mut().insert(axum::http::header::CONTENT_DISPOSITION, v);
    }

    Ok(Reply::Raw(res))
}

pub async fn cover_art(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let id = call.params.require("id")?;
    let size = call.params.int("size").map(|s| s.clamp(16, 2048) as u32);

    let file = match parse_id(id) {
        Some(Id::Album(n)) => art::album_cover(state, &visible_album(state, call, n).await?).await,
        Some(Id::Track(_)) => {
            let t = visible_track(state, call, id).await?;
            track_cover_file(state, &t).await
        },
        Some(Id::Artist(n)) => {
            let r = visible_artist(state, call, n).await?;

            match r.cover_album {
                Some(a) => match catalog::album(&state.db, a).await? {
                    Some(a) => art::album_cover(state, &a).await,
                    None => None,
                },
                None => None,
            }
        },
        Some(Id::Playlist(n)) => {
            let album: Option<i64> = sqlx::query_scalar(
                "SELECT t.album_id FROM playlist_tracks pt JOIN tracks t ON t.path = pt.track_path
                 JOIN playlists p ON p.id = pt.playlist_id
                 WHERE pt.playlist_id = ? AND (p.owner_id = ? OR p.public = 1) ORDER BY pt.position LIMIT 1",
            )
            .bind(n)
            .bind(call.user.id)
            .fetch_optional(&state.db)
            .await?
            .flatten();

            match album {
                Some(a) => art::album_cover(state, &visible_album(state, call, a).await?).await,
                None => None,
            }
        },
        Some(Id::Folder(lib, rel)) => {
            let root =
                state.config.current().library(&lib).and_then(|l| l.resolved_path(state.config.config_dir()).ok());

            let album: Option<i64> = match root {
                Some(root) if crate::music::libraries(state, &call.user).contains(&lib) => {
                    sqlx::query_scalar("SELECT id FROM albums WHERE library = ? AND dir = ?")
                        .bind(&lib)
                        .bind(root.join(rel).display().to_string())
                        .fetch_optional(&state.db)
                        .await?
                },
                _ => None,
            };

            match album {
                Some(a) => art::album_cover(state, &visible_album(state, call, a).await?).await,
                None => None,
            }
        },
        None => None,
    };

    let res = image(state, file, size).await.map_err(|_| Failure::not_found("cover art"))?;
    Ok(Reply::Raw(res))
}

pub async fn avatar(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let name = call.params.get("username").unwrap_or(&call.user.username);

    let id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(name)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| Failure::not_found("user"))?;

    let res = users::avatar_image(state, id).await.map_err(|_| Failure::not_found("avatar"))?;
    Ok(Reply::Raw(res.into_response()))
}

pub async fn lyrics(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let libs = crate::music::libraries(state, &call.user);
    let title = call.params.get("title").unwrap_or("");
    let artist = call.params.get("artist").unwrap_or("");

    let found = catalog::search_tracks(&state.db, &libs, &format!("{title} {artist}"), 0, 20)
        .await?
        .into_iter()
        .find(|t| t.title.eq_ignore_ascii_case(title.trim()));

    let mut v = json!({ "artist": artist, "title": title });

    if let Some(t) = found
        && let Some(l) = lyrics::get(state, &t).await?
    {
        v["artist"] = json!(t.artist);
        v["title"] = json!(t.title);
        v["value"] = json!(l.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"));
    }

    Ok(Reply::one("lyrics", v))
}

pub async fn lyrics_by_song(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let t = visible_track(state, call, call.params.require("id")?).await?;

    let list: Vec<Value> = match lyrics::get(state, &t).await? {
        Some(l) => vec![json!({
            "displayArtist": t.artist,
            "displayTitle": t.title,
            "lang": "xxx",
            "offset": 0,
            "synced": l.synced,
            "line": l.lines.iter().map(|line| {
                let mut v = json!({ "value": line.text });

                if let Some(s) = line.start {
                    v["start"] = json!(s);
                }

                v
            }).collect::<Vec<_>>(),
        })],
        None => Vec::new(),
    };

    Ok(Reply::one("lyricsList", json!({ "structuredLyrics": list })))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ClientInfo {
    max_audio_bitrate: Option<f64>,
    max_transcoding_audio_bitrate: Option<f64>,
    direct_play_profiles: Vec<DirectPlay>,
    transcoding_profiles: Vec<TranscodingProfile>,
    codec_profiles: Vec<CodecProfile>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct DirectPlay {
    containers: Vec<String>,
    audio_codecs: Vec<String>,
    max_audio_channels: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct TranscodingProfile {
    container: String,
    audio_codec: String,
    max_audio_channels: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CodecProfile {
    name: String,
    limitations: Vec<Limitation>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Limitation {
    name: String,
    comparison: String,
    values: Vec<Value>,
}

/// Bitrates come in bit/s; anything small enough to be kbit/s already is taken as such.
fn kbps(v: f64) -> i64 {
    if v > 10_000.0 { (v / 1000.0) as i64 } else { v as i64 }
}

fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse().ok())
}

fn same(a: &str, b: &str) -> bool {
    a == "*" || a.eq_ignore_ascii_case(b) || (a.eq_ignore_ascii_case("pcm") && b == "wav")
}

/// What the track measures for a limitation.
fn measure(t: &Track, name: &str) -> Option<f64> {
    match name {
        "audioBitrate" => t.bitrate.map(|b| b as f64 * 1000.0),
        "audioSamplerate" => t.sample_rate.map(|r| r as f64),
        "audioChannels" => t.channels.map(|c| c as f64),
        "audioBitdepth" => t.bit_depth.map(|d| d as f64),
        _ => None,
    }
}

fn allows(t: &Track, l: &Limitation) -> bool {
    let Some(have) = measure(t, &l.name) else { return true };
    let values: Vec<f64> = l.values.iter().filter_map(number).collect();
    let Some(&first) = values.first() else { return true };

    match l.comparison.as_str() {
        "LessThanEqual" => have <= first,
        "GreaterThanEqual" => have >= first,
        "Equals" => values.contains(&have),
        "NotEquals" => !values.contains(&have),
        _ => true,
    }
}

fn stream_info(t: &Track) -> Value {
    json!({
        "protocol": "http",
        "container": t.suffix,
        "codec": t.codec,
        "audioChannels": t.channels.unwrap_or(2),
        "audioBitrate": t.bitrate.unwrap_or(0) * 1000,
        "audioSamplerate": t.sample_rate.unwrap_or(44_100),
        "audioBitdepth": t.bit_depth.unwrap_or(0),
    })
}

pub async fn transcode_decision(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let t = visible_track(state, call, call.params.require("mediaId")?).await?;
    let info: ClientInfo = serde_json::from_slice(&call.body).unwrap_or_default();
    let codec = |c: &str| c.eq_ignore_ascii_case(&t.codec) || (t.codec == "pcm" && c.eq_ignore_ascii_case("wav"));

    let limits_ok =
        info.codec_profiles.iter().filter(|c| codec(&c.name)).all(|c| c.limitations.iter().all(|l| allows(&t, l)));

    let bitrate_ok =
        info.max_audio_bitrate.filter(|b| *b > 0.0).is_none_or(|b| t.bitrate.is_none_or(|have| have <= kbps(b)));

    let direct = info.direct_play_profiles.iter().any(|d| {
        (d.containers.is_empty() || d.containers.iter().any(|c| same(c, &t.suffix)))
            && (d.audio_codecs.is_empty() || d.audio_codecs.iter().any(|c| same(c, &t.codec)))
            && d.max_audio_channels.is_none_or(|m| t.channels.is_none_or(|c| c <= m))
    });

    let mut reasons = Vec::new();

    if !direct {
        reasons.push("ContainerNotSupported");
    }

    if !limits_ok {
        reasons.push("AudioCodecNotSupported");
    }

    if !bitrate_ok {
        reasons.push("AudioBitrateNotSupported");
    }

    let mut v = json!({
        "canDirectPlay": direct && limits_ok && bitrate_ok,
        "canTranscode": false,
        "sourceStream": stream_info(&t),
    });

    if direct && limits_ok && bitrate_ok {
        return Ok(Reply::one("transcodeDecision", v));
    }

    v["transcodeReason"] = json!(reasons);

    let pick = info.transcoding_profiles.iter().find_map(|p| {
        let f = Format::parse(&p.audio_codec).or_else(|| Format::parse(&p.container))?;
        Some((f, p))
    });

    let Some((format, profile)) = pick else {
        v["errorReason"] = json!("none of the app's transcoding profiles is one this server can make");
        return Ok(Reply::one("transcodeDecision", v));
    };

    let cap = info.max_transcoding_audio_bitrate.or(info.max_audio_bitrate).filter(|b| *b > 0.0).map(kbps);
    let bitrate = if format.lossless() { 0 } else { cap.unwrap_or(format.default_bitrate() as i64).clamp(32, 320) };
    let channels = profile.max_audio_channels.unwrap_or(2).min(t.channels.unwrap_or(2)).max(1);

    let (rate, depth) = match format {
        Format::Flac => (t.sample_rate.unwrap_or(44_100), t.bit_depth.unwrap_or(16).min(24)),
        Format::Opus => (48_000, 0),
        _ => (t.sample_rate.filter(|r| *r <= 48_000).unwrap_or(48_000), 0),
    };

    v["canTranscode"] = json!(true);
    v["transcodeParams"] = json!(format!("{}-{bitrate}", format.suffix()));

    v["transcodeStream"] = json!({
        "protocol": "http",
        "container": if profile.container.is_empty() { format.suffix().to_string() } else { profile.container.clone() },
        "codec": format.suffix(),
        "audioChannels": channels,
        "audioBitrate": bitrate * 1000,
        "audioSamplerate": rate,
        "audioBitdepth": depth,
    });

    Ok(Reply::one("transcodeDecision", v))
}

pub async fn transcode_stream(state: &Arc<AppState>, call: &Call) -> Result<Reply> {
    let t = visible_track(state, call, call.params.require("mediaId")?).await?;

    let params = call
        .params
        .get("transcodeParams")
        .or(call.params.get("parameters"))
        .ok_or_else(|| Failure::missing("transcodeParams"))?;

    let (format, bitrate) = params.split_once('-').unwrap_or((params, "0"));

    let format =
        Format::parse(format).ok_or_else(|| Failure::new(10, "those transcodeParams aren't from this server"))?;

    let bitrate = bitrate.parse::<u32>().ok().filter(|b| *b > 0);
    let offset = call.params.get("offset").and_then(|o| o.parse::<f64>().ok()).unwrap_or(0.0);
    Ok(Reply::Raw(transcoded(state, &format!("{} ({})", call.user.username, call.client), &t, format, bitrate, offset)))
}
