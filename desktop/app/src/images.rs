// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pictures from the server: kept in memory for the session and on disk between runs. One on
//! disk is shown at once and fetched again behind it, so a changed picture still turns up.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::time::Instant;

use iced_widget::image::Handle;

use crate::api::Api;

/// The largest picture fetched, so a broken server can't fill memory.
const LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub enum Picture {
    Loading,
    /// When it arrived from the network, for developing it in; `None` if it came from disk.
    Ready(Handle, Option<Instant>),
    Missing,
}

#[derive(Default)]
pub struct Images {
    pictures: HashMap<String, Picture>,
}

impl Images {
    pub fn get(&self, path: &str) -> Option<&Handle> {
        match self.pictures.get(path) {
            Some(Picture::Ready(handle, _)) => Some(handle),
            _ => None,
        }
    }

    pub fn picture(&self, path: &str) -> Option<&Picture> {
        self.pictures.get(path)
    }

    /// Whether `path` still has to be asked for; marks it as on its way.
    pub fn want(&mut self, path: &str) -> bool {
        if self.pictures.contains_key(path) {
            return false;
        }
        self.pictures.insert(path.to_owned(), Picture::Loading);
        true
    }

    pub fn arrived(&mut self, path: String, handle: Option<Handle>, fresh: bool) {
        if !fresh && matches!(self.pictures.get(&path), Some(Picture::Ready(_, Some(_)))) {
            return;
        }
        let picture = match handle {
            Some(handle) => Picture::Ready(handle, fresh.then(Instant::now)),
            None if matches!(self.pictures.get(&path), Some(Picture::Ready(..))) => return,
            None => Picture::Missing,
        };
        self.pictures.insert(path, picture);
    }

    /// Forgets everything, for another server or when pictures may have changed.
    pub fn clear(&mut self) {
        self.pictures.clear();
    }
}

/// Where a picture from `api`'s server is kept on disk.
fn file(api: &Api, path: &str) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    (api.origin.as_str(), path).hash(&mut hasher);
    Some(crate::paths::cache_dir()?.join("images").join(format!("{:016x}", hasher.finish())))
}

/// What's on disk for `path`, if anything.
pub async fn cached(api: &Api, path: &str) -> Option<Vec<u8>> {
    tokio::fs::read(file(api, path)?).await.ok()
}

/// `path` from the server, saved to disk for next time.
pub async fn fetch(api: &Api, path: &str) -> Option<Vec<u8>> {
    let mut response = api.media(path).ok()?.send().await.ok()?.error_for_status().ok()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > LIMIT {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    if let Some(file) = file(api, path) {
        if let Some(dir) = file.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let _ = tokio::fs::write(&file, &bytes).await;
    }
    Some(bytes)
}

/// Decode away from the event loop, with a bound on decompressed memory as well as download size.
pub async fn decode(bytes: Vec<u8>) -> Option<Handle> {
    tokio::task::spawn_blocking(move || {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let rgba = reader.decode().ok()?.into_rgba8();
        Some(Handle::from_rgba(rgba.width(), rgba.height(), rgba.into_raw()))
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_results_cannot_replace_a_fresher_picture() {
        let mut images = Images::default();
        let fresh = Handle::from_rgba(1, 1, vec![255, 0, 0, 255]);
        let old = Handle::from_rgba(1, 1, vec![0, 0, 255, 255]);
        images.arrived("avatar".into(), Some(fresh.clone()), true);
        images.arrived("avatar".into(), Some(old), false);
        images.arrived("avatar".into(), None, true);
        assert_eq!(images.get("avatar").unwrap().id(), fresh.id());
        images.clear();
        assert!(images.want("avatar"));
        assert!(!images.want("avatar"));
    }

    #[test]
    fn invalid_images_are_missing() {
        assert!(crate::test_support::run(decode(b"not an image".to_vec())).is_none());
    }
}
