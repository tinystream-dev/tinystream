// SPDX-License-Identifier: AGPL-3.0-or-later

//! Downloads one file from the server into the store, picking up where an earlier try stopped
//! with a Range request.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use reqwest::{StatusCode, header};
use tokio::io::AsyncWriteExt;

use crate::api::Api;

/// How often progress is told while a file comes in.
const TELL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Paused or cancelled; what came in so far stays for next time.
    Stopped,
    /// The server can't be reached; try again once it can.
    Unreachable,
    Failed(String),
}

impl From<crate::api::Error> for Error {
    fn from(error: crate::api::Error) -> Self {
        if error.unreachable() { Self::Unreachable } else { Self::Failed(error.message) }
    }
}

/// Fetches `url` (a server path) into `file`, by way of `part`, telling `progress` the bytes in so
/// far and in all. Returns how big it is.
pub async fn fetch(
    api: &Api,
    url: &str,
    part: &Path,
    file: &Path,
    stop: &AtomicBool,
    progress: impl Fn(u64, Option<u64>),
) -> Result<u64, Error> {
    if let Ok(meta) = tokio::fs::metadata(file).await {
        return Ok(meta.len());
    }
    if let Some(dir) = file.parent() {
        tokio::fs::create_dir_all(dir).await.map_err(|e| Error::Failed(format!("Couldn't save the download: {e}")))?;
    }
    let have = tokio::fs::metadata(part).await.map(|meta| meta.len()).unwrap_or(0);
    let mut request = api.stream(url)?;
    if have > 0 {
        request = request.header(header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request.send().await.map_err(|_| Error::Unreachable)?;
    let range = response.headers().get(header::CONTENT_RANGE).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let (mut done, total) = match response.status() {
        StatusCode::PARTIAL_CONTENT => match range.as_deref().and_then(content_range) {
            Some((start, total)) if start == have => (have, total),
            _ => return Err(Error::Failed("The server sent the wrong part of the file.".into())),
        },
        StatusCode::OK => (0, response.content_length()),
        StatusCode::RANGE_NOT_SATISFIABLE
            if range.as_deref().and_then(|r| r.strip_prefix("bytes */")).and_then(|t| t.parse().ok()) == Some(have) =>
        {
            return finish(part, file, have).await;
        },
        StatusCode::FORBIDDEN => return Err(Error::Failed("The server doesn't let you download this.".into())),
        StatusCode::NOT_FOUND => return Err(Error::Failed("It isn't on the server anymore.".into())),
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => {
            return Err(Error::Unreachable);
        },
        status => return Err(Error::Failed(format!("The server returned HTTP {}.", status.as_u16()))),
    };
    let mut out = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(done > 0)
        .truncate(done == 0)
        .open(part)
        .await
        .map_err(|e| Error::Failed(format!("Couldn't save the download: {e}")))?;
    progress(done, total);
    let mut told = Instant::now();
    loop {
        if stop.load(Ordering::Acquire) {
            let _ = out.flush().await;
            return Err(Error::Stopped);
        }
        let Some(chunk) = response.chunk().await.map_err(|_| Error::Unreachable)? else { break };
        out.write_all(&chunk).await.map_err(|e| Error::Failed(format!("Couldn't save the download: {e}")))?;
        done += chunk.len() as u64;
        if told.elapsed() >= TELL {
            told = Instant::now();
            progress(done, total);
        }
    }
    out.flush().await.map_err(|e| Error::Failed(format!("Couldn't save the download: {e}")))?;
    drop(out);
    if total.is_some_and(|total| total != done) {
        return Err(Error::Unreachable);
    }
    progress(done, Some(done));
    finish(part, file, done).await
}

async fn finish(part: &Path, file: &Path, size: u64) -> Result<u64, Error> {
    tokio::fs::rename(part, file).await.map_err(|e| Error::Failed(format!("Couldn't save the download: {e}")))?;
    Ok(size)
}

/// `bytes 100-199/1000` as (100, Some(1000)).
fn content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let start = range.split_once('-')?.0.parse().ok()?;
    Some((start, total.parse().ok()))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    use super::*;

    /// Answers one request per response, handing back what was asked.
    fn server(responses: Vec<String>) -> (url::Url, std::thread::JoinHandle<Vec<String>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let mut asked = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut buffer = [0; 4096];
                let n = socket.read(&mut buffer).unwrap();
                asked.push(String::from_utf8_lossy(&buffer[..n]).to_ascii_lowercase());
                socket.write_all(response.as_bytes()).unwrap();
            }
            asked
        });
        (url::Url::parse(&format!("http://{address}/")).unwrap(), handle)
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tinystream-fetch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resumes_where_it_stopped_and_keeps_the_whole_file() {
        let dir = dir("resume");
        let (part, file) = (dir.join("a.part"), dir.join("a"));
        std::fs::write(&part, b"hello ").unwrap();
        let (origin, server) = server(vec![
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 6-10/11\r\nContent-Length: 5\r\nConnection: close\r\n\r\nworld".into(),
        ]);
        let api = Api::new(origin, Some(crate::api::Token("t".into()))).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let tell = seen.clone();
        let size = runtime
            .block_on(fetch(&api, "api/media/1/file?download=1", &part, &file, &AtomicBool::new(false), move |d, t| {
                tell.lock().unwrap().push((d, t))
            }))
            .unwrap();
        assert_eq!(size, 11);
        assert_eq!(std::fs::read(&file).unwrap(), b"hello world");
        assert!(!part.exists());
        assert_eq!(seen.lock().unwrap().last(), Some(&(11, Some(11))));
        let asked = server.join().unwrap();
        assert!(asked[0].starts_with("get /api/media/1/file?download=1 "));
        assert!(asked[0].contains("range: bytes=6-"));
        assert!(asked[0].contains("authorization: bearer t"));

        let again = runtime.block_on(fetch(&api, "unused", &part, &file, &AtomicBool::new(false), |_, _| {}));
        assert_eq!(again, Ok(11));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn starts_over_when_the_server_sends_it_all_and_refuses_what_it_should() {
        let dir = dir("over");
        let (part, file) = (dir.join("a.part"), dir.join("a"));
        std::fs::write(&part, b"stale").unwrap();
        let (origin, server) = server(vec![
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew".into(),
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-2/3\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc"
                .into(),
        ]);
        let api = Api::new(origin, None).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let stop = AtomicBool::new(false);
        assert_eq!(runtime.block_on(fetch(&api, "a", &part, &file, &stop, |_, _| {})), Ok(3));
        assert_eq!(std::fs::read(&file).unwrap(), b"new");

        let (part, file) = (dir.join("b.part"), dir.join("b"));
        assert_eq!(
            runtime.block_on(fetch(&api, "b", &part, &file, &stop, |_, _| {})),
            Err(Error::Failed("The server doesn't let you download this.".into()))
        );
        std::fs::write(&part, b"xy").unwrap();
        assert!(matches!(runtime.block_on(fetch(&api, "b", &part, &file, &stop, |_, _| {})), Err(Error::Failed(_))));
        server.join().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_content_ranges() {
        assert_eq!(content_range("bytes 100-199/1000"), Some((100, Some(1000))));
        assert_eq!(content_range("bytes 0-0/*"), Some((0, None)));
        assert_eq!(content_range("items 0-1/2"), None);
    }
}
