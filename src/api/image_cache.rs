// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, Weak};

use sha2::{Digest, Sha256};
use tokio::sync::Mutex as AsyncMutex;

static DOWNLOADS: LazyLock<Mutex<HashMap<PathBuf, Weak<AsyncMutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn download_lock(file: &Path) -> Arc<AsyncMutex<()>> {
    let mut downloads = DOWNLOADS.lock().unwrap_or_else(|e| e.into_inner());
    downloads.retain(|_, lock| lock.strong_count() > 0);

    if let Some(lock) = downloads.get(file).and_then(Weak::upgrade) {
        return lock;
    }

    let lock = Arc::new(AsyncMutex::new(()));
    downloads.insert(file.to_path_buf(), Arc::downgrade(&lock));
    lock
}

pub(crate) async fn remote(http: &reqwest::Client, dir: &Path, url: &str) -> anyhow::Result<PathBuf> {
    let key = hex::encode(&Sha256::digest(url.as_bytes())[..16]);
    let ext = url.rsplit('.').next().filter(|e| e.len() <= 4).unwrap_or("jpg");
    let file = dir.join(format!("{key}.{ext}"));

    if tokio::fs::try_exists(&file).await? {
        return Ok(file);
    }

    let lock = download_lock(&file);
    let _guard = lock.lock().await;

    if tokio::fs::try_exists(&file).await? {
        return Ok(file);
    }

    let bytes = http.get(url).send().await?.error_for_status()?.bytes().await?;
    tokio::fs::create_dir_all(dir).await?;
    let tmp = file.with_extension("part");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &file).await?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Notify;
    use tokio::task::{JoinHandle, JoinSet};

    use super::remote;

    const BODY: &[u8] = b"original-unmodified-image-data";

    struct Server {
        url: String,
        count: Arc<AtomicUsize>,
        task: JoinHandle<()>,
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn server(fail_first: bool, gate: Option<Arc<Notify>>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();

        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let c = c.clone();
                let gate = gate.clone();

                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    stream.read(&mut buf).await.unwrap();
                    let n = c.fetch_add(1, Ordering::SeqCst);

                    if let Some(gate) = gate {
                        gate.notified().await;
                    } else {
                        tokio::time::sleep(Duration::from_millis(60)).await;
                    }

                    let status = if fail_first && n == 0 { "500 Internal Server Error" } else { "200 OK" };

                    let headers =
                        format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", BODY.len());

                    stream.write_all(headers.as_bytes()).await.unwrap();
                    stream.write_all(BODY).await.unwrap();
                });
            }
        });

        Server { url, count, task }
    }

    fn dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "tinystream-image-cache-{name}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_download_and_cache_original_bytes() {
        let server = server(false, None).await;
        let dir = dir("shared");
        let client = reqwest::Client::new();
        let mut jobs = JoinSet::new();

        for _ in 0..20 {
            let client = client.clone();
            let dir = dir.clone();
            let url = format!("{}/poster.jpg", server.url);
            jobs.spawn(async move { remote(&client, &dir, &url).await.unwrap() });
        }

        let mut files = vec![];

        while let Some(file) = jobs.join_next().await {
            files.push(file.unwrap());
        }

        assert!(files.iter().all(|f| f == &files[0]));
        assert_eq!(server.count.load(Ordering::SeqCst), 1);
        assert_eq!(tokio::fs::read(&files[0]).await.unwrap(), BODY);
        server.task.abort();
        assert_eq!(remote(&client, &dir, &format!("{}/poster.jpg", server.url)).await.unwrap(), files[0]);
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[tokio::test]
    async fn failed_download_can_retry_without_poisoning_cache() {
        let server = server(true, None).await;
        let dir = dir("retry");
        let client = reqwest::Client::new();
        let url = format!("{}/poster.jpg", server.url);
        assert!(remote(&client, &dir, &url).await.is_err());
        let file = remote(&client, &dir, &url).await.unwrap();
        assert_eq!(tokio::fs::read(file).await.unwrap(), BODY);
        assert_eq!(server.count.load(Ordering::SeqCst), 2);
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[tokio::test]
    async fn different_images_download_in_parallel() {
        let gate = Arc::new(Notify::new());
        let server = server(false, Some(gate.clone())).await;
        let dir = dir("parallel");
        let client = reqwest::Client::new();
        let mut jobs = JoinSet::new();

        for image in ["one", "two"] {
            let dir = dir.clone();
            let client = client.clone();
            let url = format!("{}/{image}.jpg", server.url);
            jobs.spawn(async move { remote(&client, &dir, &url).await.unwrap() });
        }

        tokio::time::timeout(Duration::from_secs(3), async {
            while server.count.load(Ordering::SeqCst) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        gate.notify_waiters();

        while let Some(result) = jobs.join_next().await {
            assert!(result.is_ok());
        }

        tokio::fs::remove_dir_all(dir).await.unwrap();
    }
}
