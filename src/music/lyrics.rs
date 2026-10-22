// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use tokio::sync::OnceCell;

use super::catalog::Track;
use crate::db::now;
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Milliseconds in, for synced lyrics.
    pub start: Option<i64>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    File,
    Embedded,
    Online,
}

#[derive(Debug, Clone)]
pub struct Lyrics {
    pub synced: bool,
    pub lines: Vec<Line>,
    pub source: Source,
}

#[derive(Default)]
pub struct Fetches(Mutex<HashMap<String, Arc<OnceCell<()>>>>);

fn timestamp(tag: &str) -> Option<i64> {
    let (m, rest) = tag.split_once(':')?;
    let m: i64 = m.trim().parse().ok()?;
    let secs: f64 = rest.trim().replace(',', ".").parse().ok()?;
    (secs >= 0.0).then(|| m * 60_000 + (secs * 1000.0).round() as i64)
}

/// LRC, or plain text when it has no timestamps.
pub fn parse(text: &str) -> (bool, Vec<Line>) {
    let mut offset = 0i64;
    let mut timed: Vec<Line> = Vec::new();
    let mut plain = Vec::new();

    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut starts = Vec::new();

        while let Some(body) = rest.strip_prefix('[') {
            let Some(end) = body.find(']') else { break };
            let tag = &body[..end];

            if let Some(ms) = timestamp(tag) {
                starts.push(ms);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
            }

            rest = body[end + 1..].trim_start();
        }

        let words = strip_word_times(rest);

        if starts.is_empty() {
            if !raw.trim_start().starts_with('[') {
                plain.push(Line { start: None, text: words });
            }
        } else {
            for s in starts {
                timed.push(Line { start: Some(s), text: words.clone() });
            }
        }
    }

    if timed.is_empty() {
        while plain.last().is_some_and(|l| l.text.is_empty()) {
            plain.pop();
        }

        return (false, plain);
    }

    for l in &mut timed {
        l.start = l.start.map(|s| (s - offset).max(0));
    }

    timed.sort_by_key(|l| l.start);
    (true, timed)
}

/// Drops the per-word `<mm:ss.xx>` marks of enhanced LRC.
fn strip_word_times(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;

    while let Some(i) = rest.find('<') {
        match rest[i..].find('>') {
            Some(j) if timestamp(&rest[i + 1..i + j]).is_some() => {
                out.push_str(&rest[..i]);
                rest = &rest[i + j + 1..];
            },
            _ => {
                out.push_str(&rest[..=i]);
                rest = &rest[i + 1..];
            },
        }
    }

    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn usable(text: &str, source: Source) -> Option<Lyrics> {
    let (synced, lines) = parse(text);
    lines.iter().any(|l| !l.text.is_empty()).then_some(Lyrics { synced, lines, source })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Found {
    synced_lyrics: Option<String>,
    plain_lyrics: Option<String>,
}

async fn look_up(state: &AppState, track: &Track) -> anyhow::Result<(Option<String>, Option<String>)> {
    let base = state.config.current().music.lyrics_url.trim().trim_end_matches('/').to_string();

    let artist =
        track.album_artist.as_deref().filter(|a| !a.is_empty() && *a != crate::library::music::VARIOUS_ARTISTS);

    let res = state
        .http
        .get(format!("{base}/api/get"))
        .query(&[
            ("track_name", track.title.as_str()),
            ("artist_name", track.artists().first().map(String::as_str).or(artist).unwrap_or(&track.artist)),
            ("album_name", track.album.as_str()),
            ("duration", &format!("{}", track.duration.round() as i64)),
        ])
        .send()
        .await?;

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok((None, None));
    }

    let found: Found = res.error_for_status()?.json().await?;
    Ok((found.synced_lyrics.filter(|s| !s.trim().is_empty()), found.plain_lyrics.filter(|s| !s.trim().is_empty())))
}

/// A track's lyrics: a `.lrc` next to it, then its tags, then online.
/// Synced lyrics win over plain ones wherever they come from.
pub async fn get(state: &AppState, track: &Track) -> anyhow::Result<Option<Lyrics>> {
    let sidecar = Path::new(&track.path).with_extension("lrc");
    let file = tokio::fs::read_to_string(&sidecar).await.ok().and_then(|t| usable(&t, Source::File));

    if file.as_ref().is_some_and(|l| l.synced) {
        return Ok(file);
    }

    let embedded = if track.has_lyrics {
        let text: Option<String> =
            sqlx::query_scalar("SELECT lyrics FROM tracks WHERE id = ?").bind(track.id).fetch_one(&state.db).await?;

        text.and_then(|t| usable(&t, Source::Embedded))
    } else {
        None
    };

    if embedded.as_ref().is_some_and(|l| l.synced) {
        return Ok(embedded);
    }

    let local = file.or(embedded);
    let config = state.config.current();

    if !config.music.online_lyrics {
        return Ok(local);
    }

    let cell = state.music.lyrics.0.lock().unwrap().entry(track.path.clone()).or_default().clone();

    cell.get_or_init(|| async {
        let cached: Option<i64> = sqlx::query_scalar("SELECT fetched_at FROM lyrics WHERE track_path = ?")
            .bind(&track.path)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

        if cached.is_some_and(|at| at > now() - 30 * 86400) {
            return;
        }

        match look_up(state, track).await {
            Ok((synced, plain)) => {
                let _ = sqlx::query(
                    "INSERT INTO lyrics (track_path, synced, plain, fetched_at) VALUES (?, ?, ?, ?)
                     ON CONFLICT(track_path) DO UPDATE SET synced = excluded.synced, plain = excluded.plain,
                        fetched_at = excluded.fetched_at",
                )
                .bind(&track.path)
                .bind(synced)
                .bind(plain)
                .bind(now())
                .execute(&state.db)
                .await;
            },
            Err(e) => tracing::debug!("looking up lyrics for {}: {e:#}", track.path),
        }
    })
    .await;

    state.music.lyrics.0.lock().unwrap().remove(&track.path);

    let online: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT synced, plain FROM lyrics WHERE track_path = ?")
            .bind(&track.path)
            .fetch_optional(&state.db)
            .await?;

    let (synced, plain) = online.unwrap_or_default();

    if let Some(l) = synced.as_deref().and_then(|t| usable(t, Source::Online)) {
        return Ok(Some(l));
    }

    Ok(local.or_else(|| plain.as_deref().and_then(|t| usable(t, Source::Online))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc() {
        let (synced, lines) = parse("[ar:Someone]\n[00:12.50]First\n[00:05.00][00:20.00]Chorus\n[offset:+500]\n");
        assert!(synced);
        let starts: Vec<_> = lines.iter().map(|l| (l.start.unwrap(), l.text.as_str())).collect();
        assert_eq!(starts, [(4500, "Chorus"), (12000, "First"), (19500, "Chorus")]);
    }

    #[test]
    fn enhanced_lrc_and_plain() {
        let (_, lines) = parse("[00:01.00]<00:01.00>Hello <00:01.50>there");
        assert_eq!(lines[0].text, "Hello there");
        let (synced, lines) = parse("Just\nwords\n\n");
        assert!(!synced);
        assert_eq!(lines.len(), 2);
    }
}
