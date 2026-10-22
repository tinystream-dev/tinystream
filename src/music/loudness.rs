// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{Notify, OnceCell};

use crate::state::AppState;

/// Measures tracks whose tags carry no ReplayGain: in the background while
/// nothing else is going on, or straight away for whatever's about to play.
#[derive(Default)]
pub struct Analyzer {
    wake: Notify,
    running: Mutex<HashMap<i64, Job>>,
}

#[derive(Clone, Default)]
struct Job {
    done: Arc<OnceCell<bool>>,

    /// Someone's waiting to play it, so it no longer waits for anything else.
    urgent: Arc<AtomicBool>,
}

pub fn wake(state: &AppState) {
    state.music.loudness.wake.notify_one();
}

/// Measures a track now, unless it's been measured or doesn't need it.
/// Returns whether there's anything new to read.
pub async fn ensure(state: &Arc<AppState>, track_id: i64) -> bool {
    analyze(state, track_id, true).await
}

async fn analyze(state: &Arc<AppState>, track_id: i64, urgent: bool) -> bool {
    let job = state.music.loudness.running.lock().unwrap().entry(track_id).or_default().clone();

    if urgent {
        job.urgent.store(true, Ordering::Relaxed);
    }

    let done = job
        .done
        .get_or_init(|| async {
            match measure(state, track_id, job.urgent.clone()).await {
                Ok(changed) => changed,
                Err(e) => {
                    tracing::warn!("can't measure the loudness of track {track_id}: {e:#}");

                    let _ = sqlx::query("UPDATE tracks SET analyzed = -1 WHERE id = ?")
                        .bind(track_id)
                        .execute(&state.db)
                        .await;

                    false
                },
            }
        })
        .await;

    state.music.loudness.running.lock().unwrap().remove(&track_id);
    *done
}

async fn measure(state: &Arc<AppState>, track_id: i64, urgent: Arc<AtomicBool>) -> anyhow::Result<bool> {
    let row: Option<(String, i64, Option<f64>, Option<i64>)> =
        sqlx::query_as("SELECT path, analyzed, rg_track_gain, album_id FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_optional(&state.db)
            .await?;

    let Some((path, analyzed, tagged, album_id)) = row else { return Ok(false) };

    if analyzed != 0 || tagged.is_some() {
        return Ok(false);
    }

    let started = std::time::Instant::now();
    let busy = state.media.busy.clone();
    let p = PathBuf::from(&path);

    let (loudness, peak) = tokio::task::spawn_blocking(move || {
        let wait = || {
            while !urgent.load(Ordering::Relaxed) && busy.is_busy() {
                std::thread::sleep(Duration::from_millis(250));
            }
        };

        crate::media::audio::measure(&p, &wait)
    })
    .await??;

    sqlx::query("UPDATE tracks SET loudness = ?, peak = ?, analyzed = 1 WHERE id = ?")
        .bind(loudness)
        .bind(peak)
        .bind(track_id)
        .execute(&state.db)
        .await?;

    tracing::debug!(
        "measured {} in {:.1?}: {}",
        path,
        started.elapsed(),
        loudness.map(|l| format!("{l:.1} LUFS")).unwrap_or_else(|| "silent".into())
    );

    if let Some(album) = album_id {
        album_loudness(state, album).await?;
    }

    Ok(true)
}

/// Once every track's measured, the album's loudness is all of them together,
/// weighted by how long each one is.
async fn album_loudness(state: &AppState, album_id: i64) -> anyhow::Result<()> {
    let rows: Vec<(Option<f64>, f64, i64, Option<f64>)> =
        sqlx::query_as("SELECT loudness, duration, analyzed, rg_track_gain FROM tracks WHERE album_id = ?")
            .bind(album_id)
            .fetch_all(&state.db)
            .await?;

    if rows.iter().any(|(_, _, analyzed, tagged)| *analyzed == 0 && tagged.is_none()) {
        return Ok(());
    }

    let (energy, time) = rows
        .iter()
        .filter_map(|(l, d, ..)| Some((10f64.powf(l.as_ref()? / 10.0) * d, *d)))
        .fold((0.0, 0.0), |(e, t), (de, dt)| (e + de, t + dt));

    let loudness = (time > 0.0).then(|| 10.0 * (energy / time).log10());
    sqlx::query("UPDATE albums SET loudness = ? WHERE id = ?").bind(loudness).bind(album_id).execute(&state.db).await?;
    Ok(())
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            let enabled = state.config.current().music.analyze_loudness;

            let next: Option<i64> = if enabled {
                sqlx::query_scalar(
                    "SELECT id FROM tracks WHERE analyzed = 0 AND rg_track_gain IS NULL ORDER BY added_at DESC, id LIMIT 1",
                )
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None)
            } else {
                None
            };

            let Some(id) = next else {
                tokio::select! {
                    _ = state.music.loudness.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(600)) => {}
                }

                continue;
            };

            state.media.busy.until_idle().await;
            analyze(&state, id, false).await;
        }
    });
}
