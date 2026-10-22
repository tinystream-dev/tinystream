// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod audio;
pub mod burn;
pub mod clip;
pub mod codecs;
pub mod ff;
pub mod hw;
pub mod probe;
pub mod screenshot;
pub mod stream;
pub mod subs;
pub mod thumb;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub use probe::MediaInfo;
use tokio::sync::OnceCell;

/// Work someone's waiting on right now, like a transcode or a clip being
/// rendered. Background jobs hold off while there's any.
#[derive(Default)]
pub struct Busy {
    count: AtomicUsize,
    idle: tokio::sync::Notify,
}

pub struct Hold(Arc<Busy>);

impl Busy {
    pub fn hold(self: &Arc<Self>) -> Hold {
        self.count.fetch_add(1, Ordering::SeqCst);
        Hold(self.clone())
    }

    pub fn is_busy(&self) -> bool {
        self.count.load(Ordering::SeqCst) > 0
    }

    pub async fn until_idle(&self) {
        loop {
            let idle = self.idle.notified();

            if !self.is_busy() {
                return;
            }

            idle.await;
        }
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        if self.0.count.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.0.idle.notify_waiters();
        }
    }
}

pub struct MediaService {
    pub hw: hw::Hw,
    pub busy: Arc<Busy>,
    cache_dir: PathBuf,
    probes: Mutex<HashMap<(PathBuf, i64), Arc<MediaInfo>>>,

    extractions: Mutex<HashMap<PathBuf, Arc<OnceCell<Result<(), String>>>>>,

    thumbs: tokio::sync::Semaphore,
    previews: tokio::sync::Semaphore,
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl MediaService {
    pub fn new(cache_dir: PathBuf) -> Self {
        unsafe { ff::ffi::av_log_set_level(ff::ffi::AV_LOG_ERROR as i32) };

        Self {
            hw: hw::Hw::new(),
            busy: Default::default(),
            cache_dir,
            probes: Mutex::new(HashMap::new()),
            extractions: Mutex::new(HashMap::new()),
            thumbs: tokio::sync::Semaphore::new(3),
            previews: tokio::sync::Semaphore::new(4),
        }
    }

    pub async fn probe(&self, path: &Path) -> anyhow::Result<Arc<MediaInfo>> {
        let key = (path.to_path_buf(), mtime(path));

        if let Some(info) = self.probes.lock().unwrap().get(&key) {
            return Ok(info.clone());
        }

        let p = path.to_path_buf();
        let info = Arc::new(tokio::task::spawn_blocking(move || probe::probe(&p)).await??);
        let mut probes = self.probes.lock().unwrap();

        if probes.len() > 512 {
            probes.clear();
        }

        probes.insert(key, info.clone());
        Ok(info)
    }

    pub async fn thumbnail(&self, path: &Path) -> anyhow::Result<PathBuf> {
        use sha2::{Digest, Sha256};
        let key = hex::encode(&Sha256::digest(format!("{}\0{}", path.display(), mtime(path)))[..12]);
        let file = self.cache_dir.join("thumbnails").join(format!("{key}.jpg"));

        if file.exists() {
            return Ok(file);
        }

        let _permit = self.thumbs.acquire().await?;

        if file.exists() {
            return Ok(file);
        }

        let p = path.to_path_buf();
        let jpeg = tokio::task::spawn_blocking(move || thumb::capture(&p, 640)).await??;
        std::fs::create_dir_all(file.parent().unwrap())?;
        let tmp = file.with_extension("part");
        std::fs::write(&tmp, jpeg)?;
        std::fs::rename(&tmp, &file)?;
        Ok(file)
    }

    pub async fn preview(&self, path: &Path, at: u32) -> anyhow::Result<PathBuf> {
        use sha2::{Digest, Sha256};
        let key = hex::encode(&Sha256::digest(format!("{}\0{}", path.display(), mtime(path)))[..12]);
        let file = self.cache_dir.join("previews").join(&key).join(format!("{at}.jpg"));

        if file.exists() {
            return Ok(file);
        }

        let _permit = self.previews.acquire().await?;

        if file.exists() {
            return Ok(file);
        }

        let p = path.to_path_buf();
        let jpeg = tokio::task::spawn_blocking(move || thumb::preview(&p, at as f64, 256)).await??;
        std::fs::create_dir_all(file.parent().unwrap())?;
        let tmp = file.with_extension("part");
        std::fs::write(&tmp, jpeg)?;
        std::fs::rename(&tmp, &file)?;
        Ok(file)
    }

    pub async fn subtitles(&self, path: &Path, track: &str) -> anyhow::Result<String> {
        if track.starts_with('s') {
            self.ensure_extracted(path).await?;
        }

        let (p, t, cache) = (path.to_path_buf(), track.to_string(), self.cache_dir.clone());
        tokio::task::spawn_blocking(move || subs::load(&p, &t, &cache)).await?
    }

    pub fn prefetch_subtitles(self: &Arc<Self>, path: PathBuf) {
        let this = self.clone();

        tokio::spawn(async move {
            if let Err(e) = this.ensure_extracted(&path).await {
                tracing::debug!("prefetching subtitles of {}: {e:#}", path.display());
            }
        });
    }

    async fn ensure_extracted(&self, path: &Path) -> anyhow::Result<()> {
        let info = self.probe(path).await?;

        let embedded: Vec<usize> = info
            .subtitles
            .iter()
            .filter(|s| s.supported)
            .filter_map(|s| s.id.strip_prefix('s')?.parse().ok())
            .collect();

        if embedded.iter().all(|&i| subs::is_cached(path, i, &self.cache_dir)) {
            return Ok(());
        }

        let cell = self.extractions.lock().unwrap().entry(path.to_path_buf()).or_default().clone();

        let result = cell
            .get_or_init(|| {
                let (p, cache) = (path.to_path_buf(), self.cache_dir.clone());

                async move {
                    let first = embedded.first().copied().unwrap_or(0);

                    tokio::task::spawn_blocking(move || subs::load(&p, &format!("s{first}"), &cache).map(|_| ()))
                        .await
                        .map_err(|e| e.to_string())?
                        .map_err(|e| format!("{e:#}"))
                }
            })
            .await
            .clone();

        self.extractions.lock().unwrap().remove(path);
        result.map_err(anyhow::Error::msg)
    }
}
