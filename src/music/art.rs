// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::catalog::Album;
use crate::library::local_art;
use crate::library::music::{COVER_NAMES, embedded_cover};
use crate::state::AppState;

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn key(path: &Path) -> String {
    hex::encode(&Sha256::digest(format!("{}\0{}", path.display(), mtime(path)))[..12])
}

/// The picture inside a track, written out once so it can be served and scaled like any other.
async fn embedded(state: &AppState, track: &Path) -> Option<PathBuf> {
    let dir = state.paths.cache_dir().join("covers");
    let k = key(track);

    for ext in ["jpg", "png", "webp"] {
        let f = dir.join(format!("{k}.{ext}"));

        if f.exists() {
            return Some(f);
        }
    }

    let p = track.to_path_buf();
    let (data, mime) = tokio::task::spawn_blocking(move || embedded_cover(&p)).await.ok()??;

    let ext = match mime.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "jpg",
    };

    let file = dir.join(format!("{k}.{ext}"));
    tokio::fs::create_dir_all(&dir).await.ok()?;
    let tmp = file.with_extension("part");
    tokio::fs::write(&tmp, data).await.ok()?;
    tokio::fs::rename(&tmp, &file).await.ok()?;
    Some(file)
}

/// The album's cover: a picture in its folder, or the one inside its tracks.
pub async fn album_cover(state: &AppState, album: &Album) -> Option<PathBuf> {
    if let Some(local) = local_art(Path::new(&album.dir), COVER_NAMES) {
        return Some(local);
    }

    let track: String = sqlx::query_scalar("SELECT path FROM tracks WHERE id = ?")
        .bind(album.cover_track?)
        .fetch_optional(&state.db)
        .await
        .ok()??;

    embedded(state, Path::new(&track)).await
}

pub async fn track_cover(state: &AppState, path: &Path, embedded_art: bool, album: Option<&Album>) -> Option<PathBuf> {
    if embedded_art && let Some(f) = embedded(state, path).await {
        return Some(f);
    }
    album_cover(state, album?).await
}

/// `original` made `size` pixels wide, unless it's that small already.
pub async fn scaled(state: &AppState, original: PathBuf, size: Option<u32>) -> PathBuf {
    let Some(size) = size.filter(|&s| (16..2048).contains(&s)) else { return original };
    let size = size.next_multiple_of(8);
    let file = state.paths.cache_dir().join("covers").join(format!("{}-{size}.jpg", key(&original)));

    if file.exists() {
        return file;
    }

    let o = original.clone();
    let jpeg = tokio::task::spawn_blocking(move || crate::media::thumb::picture(&o, size as i32)).await;

    match jpeg {
        Ok(Ok(bytes)) => {
            let tmp = file.with_extension("part");
            let _ = tokio::fs::create_dir_all(file.parent().unwrap_or(&file)).await;

            if tokio::fs::write(&tmp, bytes).await.is_ok() && tokio::fs::rename(&tmp, &file).await.is_ok() {
                return file;
            }

            original
        },
        Ok(Err(e)) => {
            tracing::debug!("scaling {}: {e:#}", original.display());
            original
        },
        Err(_) => original,
    }
}
