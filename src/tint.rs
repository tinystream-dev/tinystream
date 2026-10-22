// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::library::local_version;
use crate::media::thumb;
use crate::state::AppState;

const SIZE: i32 = 24;

/// How long a request waits for a picture that's quick to read, so pages don't need a second visit.
const GRACE: Duration = Duration::from_millis(300);

/// Where a picture comes from, which says how to tell when it has changed.
pub enum Source {
    File(PathBuf),
    Remote(String),
    Upload { id: i64, kind: &'static str, version: i64 },
}

impl Source {
    fn key(&self) -> String {
        match self {
            Source::File(path) => {
                let id = format!("{}\0{}", path.display(), local_version(Some(path)));
                format!("file:{}", hex::encode(&Sha256::digest(id)[..16]))
            },
            Source::Remote(url) => hex::encode(&Sha256::digest(url.as_bytes())[..16]),
            Source::Upload { id, kind, version } => format!("upload:{id}:{kind}:{version}"),
        }
    }

    async fn file(&self, state: &AppState) -> anyhow::Result<(PathBuf, bool)> {
        match self {
            Source::File(path) => Ok((path.clone(), false)),
            Source::Remote(url) => {
                let dir = state.paths.cache_dir().join("images");
                Ok((crate::api::image_cache::remote(&state.http, &dir, url).await?, false))
            },
            Source::Upload { id, kind, .. } => {
                let sql = match *kind {
                    "poster" => "SELECT poster_override FROM items WHERE id = ?",
                    _ => "SELECT backdrop_override FROM items WHERE id = ?",
                };

                let data: Option<Vec<u8>> = sqlx::query_scalar(sql).bind(id).fetch_optional(&state.db).await?.flatten();
                let data = data.ok_or_else(|| anyhow::anyhow!("the picture is gone"))?;
                let dir = state.paths.cache_dir().join("tints");
                tokio::fs::create_dir_all(&dir).await?;
                let file = dir.join(format!("{}.img", hex::encode(&Sha256::digest(self.key())[..8])));
                tokio::fs::write(&file, data).await?;
                Ok((file, true))
            },
        }
    }
}

/// The most vivid colour of each picture, worked out in the background the first time it's asked
/// for.
#[derive(Default)]
pub struct Tints {
    pending: Mutex<HashSet<String>>,
}

impl Tints {
    /// `"r g b"`, or nothing while it's being worked out.
    pub async fn get(&self, state: &Arc<AppState>, source: Source) -> Option<String> {
        let key = source.key();

        match sqlx::query_scalar("SELECT tint FROM image_tints WHERE key = ?")
            .bind(&key)
            .fetch_optional(&state.db)
            .await
        {
            Ok(Some(tint)) => return Some(tint),
            Ok(None) => {},
            Err(e) => {
                tracing::debug!("tint {key}: {e:#}");
                return None;
            },
        }

        if !self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(key.clone()) {
            return None;
        }

        let task = tokio::spawn({
            let state = state.clone();

            async move {
                let tint = work(&state, &source, &key).await;
                state.tints.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&key);
                tint.map_err(|e| tracing::debug!("tint {key}: {e:#}")).ok()
            }
        });

        tokio::time::timeout(GRACE, task).await.ok()?.ok()?
    }

    /// Uploads of the item are about to change, so what was worked out for its old ones is of no
    /// use.
    pub async fn forget_uploads(&self, state: &AppState, id: i64) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM image_tints WHERE key LIKE ?")
            .bind(format!("upload:{id}:%"))
            .execute(&state.db)
            .await?;

        Ok(())
    }
}

async fn work(state: &AppState, source: &Source, key: &str) -> anyhow::Result<String> {
    let (file, temporary) = source.file(state).await?;

    let tint = tokio::task::spawn_blocking({
        let file = file.clone();
        move || compute(&file)
    })
    .await?;

    if temporary {
        let _ = tokio::fs::remove_file(&file).await;
    }

    let tint = tint?;

    sqlx::query("INSERT OR REPLACE INTO image_tints (key, tint) VALUES (?, ?)")
        .bind(key)
        .bind(&tint)
        .execute(&state.db)
        .await?;

    Ok(tint)
}

fn compute(file: &Path) -> anyhow::Result<String> {
    Ok(average(&thumb::pixels(file, SIZE)?))
}

/// The weighted mean of the pixels, favouring saturated, mid-bright ones over greys, blacks and
/// whites.
fn average(rgb: &[u8]) -> String {
    let (mut r, mut g, mut b, mut total) = (0.0, 0.0, 0.0, 0.0);

    for px in rgb.chunks_exact(3) {
        let (pr, pg, pb) = (px[0] as f64, px[1] as f64, px[2] as f64);
        let max = pr.max(pg).max(pb);
        let min = pr.min(pg).min(pb);
        let sat = if max == 0.0 { 0.0 } else { (max - min) / max };
        let light = max / 255.0;
        let w = sat * sat * if light > 0.15 && light < 0.95 { 1.0 } else { 0.1 } + 0.002;
        r += pr * w;
        g += pg * w;
        b += pb * w;
        total += w;
    }

    format!("{} {} {}", (r / total).round(), (g / total).round(), (b / total).round())
}

#[cfg(test)]
mod tests {
    use super::{average, compute};

    #[test]
    fn saturated_pixels_win_over_greys() {
        let mut px = vec![128; 3 * 100];
        px.extend([200, 40, 40].repeat(10));
        let tint = average(&px);
        let c: Vec<u32> = tint.split(' ').map(|n| n.parse().unwrap()).collect();
        assert!(c[0] > 190 && c[1] < 60 && c[2] < 60, "{tint}");
    }

    #[test]
    fn a_plain_grey_stays_grey() {
        assert_eq!(average(&[90; 3 * 576]), "90 90 90");
    }

    #[test]
    fn black_is_not_a_division_by_zero() {
        assert_eq!(average(&[0; 3 * 576]), "0 0 0");
    }

    #[test]
    fn reads_a_picture_from_disk() {
        let file = std::env::temp_dir().join(format!("tinystream-tint-{}.png", std::process::id()));
        image::RgbImage::from_pixel(64, 96, image::Rgb([30, 120, 200])).save(&file).unwrap();
        let tint = compute(&file).unwrap();
        std::fs::remove_file(&file).unwrap();
        assert_eq!(tint, "30 120 200");
    }
}
