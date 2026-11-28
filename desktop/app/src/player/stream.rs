// SPDX-License-Identifier: AGPL-3.0-or-later

//! What mpv plays, read by the app's own HTTP client through `ts-media://` URLs, so there's one TLS
//! stack and the bearer token, and ffmpeg has no networking of its own.
//!
//! The original file (direct play) is read with Range requests: seeking moves where the next one
//! starts, and a connection that drops is picked up again where it stopped. A transcode can't be
//! resumed like that; it ends with an error, and the player starts a new one where it was.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{self, Either};
use reqwest::{StatusCode, header};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch};

use crate::api::Api;

pub const PROTOCOL: &str = "ts-media";

/// Chunks read ahead of mpv, so a slow round trip doesn't stall it.
const READ_AHEAD: usize = 256;
/// Skipping this far forward reads through instead of making a new request.
const SKIP: u64 = 1 << 20;
/// Tries after a dropped connection before giving up, waiting 1, 2, 4… seconds between them.
const RETRIES: u32 = 5;

/// The `ts-media://` URL that plays `path` (like `api/media/12/file`) from the server.
pub fn url(path: &str) -> String {
    format!("{PROTOCOL}://{path}")
}

/// Opens `ts-media://` URLs against one server.
#[derive(Clone)]
pub struct Opener {
    api: Api,
    runtime: Handle,
    /// Why the last stream couldn't be opened or read, in the server's words when it said.
    pub failure: Arc<Mutex<Option<String>>>,
}

impl Opener {
    pub fn new(api: Api, runtime: Handle) -> Self {
        Self { api, runtime, failure: Arc::default() }
    }

    /// Opens `uri`, blocking (on one of mpv's threads) until the server answers.
    pub fn open(&self, uri: &str) -> Option<Box<dyn mpv::Stream>> {
        let path = uri.strip_prefix(PROTOCOL)?.strip_prefix("://")?.to_owned();
        match self.runtime.block_on(first(&self.api, &path)) {
            Ok((response, size, seekable)) => {
                let (cancel, cancelled) = watch::channel(false);
                let mut stream = Stream {
                    api: self.api.clone(),
                    path,
                    runtime: self.runtime.clone(),
                    size,
                    seekable,
                    position: 0,
                    reader: None,
                    pending: Vec::new(),
                    consumed: 0,
                    cancel: Arc::new(cancel),
                    cancelled,
                    failure: self.failure.clone(),
                };
                stream.reader = Some(stream.read_from(0, Some(response)));
                Some(Box::new(stream))
            },
            Err(error) => {
                tracing::warn!("can't open {path}: {error}");
                *self.failure.lock().unwrap() = Some(error);
                None
            },
        }
    }
}

/// Asks for the start of `path`, learning its size and whether it can be read from anywhere.
async fn first(api: &Api, path: &str) -> Result<(reqwest::Response, Option<u64>, bool), String> {
    let request = api.stream(path).map_err(|error| error.message)?.header(header::RANGE, "bytes=0-");
    let response = request.send().await.map_err(describe)?;
    let response = checked(response).await?;
    let ranges = response.status() == StatusCode::PARTIAL_CONTENT
        || response.headers().get(header::ACCEPT_RANGES).is_some_and(|value| value == "bytes");
    let size = match response.status() {
        StatusCode::PARTIAL_CONTENT => response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.rsplit('/').next())
            .and_then(|total| total.parse().ok()),
        _ => response.content_length(),
    };
    Ok((response, size.filter(|_| ranges), ranges && size.is_some()))
}

/// The response, or what went wrong in the web player's words (`engine.ts`).
async fn checked(response: reqwest::Response) -> Result<reqwest::Response, String> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let message = response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|body| body["error"].as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("the server said {status}"));
    Err(message)
}

fn describe(error: reqwest::Error) -> String {
    if error.is_timeout() { "the server stopped sending the video".into() } else { "couldn't reach the server".into() }
}

struct Stream {
    api: Api,
    path: String,
    runtime: Handle,
    size: Option<u64>,
    seekable: bool,
    position: u64,
    reader: Option<Reader>,
    /// The chunk being handed to mpv, and how much of it it has had.
    pending: Vec<u8>,
    consumed: usize,
    cancel: Arc<watch::Sender<bool>>,
    cancelled: watch::Receiver<bool>,
    failure: Arc<Mutex<Option<String>>>,
}

/// Reads one stretch of the file ahead of mpv, from where it started until it's dropped.
struct Reader {
    chunks: mpsc::Receiver<Result<Vec<u8>, String>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Stream {
    fn read_from(&self, start: u64, response: Option<reqwest::Response>) -> Reader {
        let (send, chunks) = mpsc::channel(READ_AHEAD);
        let (api, path, size, seekable) = (self.api.clone(), self.path.clone(), self.size, self.seekable);
        let task = self.runtime.spawn(fetch(api, path, start, size, seekable, response, send));
        Reader { chunks, task }
    }

    /// The next chunk from the reader, or `None` at the end; `Err` if mpv gave up on the stream.
    fn next(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.reader.is_none() {
            self.reader = Some(self.read_from(self.position, None));
        }
        let reader = self.reader.as_mut().expect("a reader");
        let mut cancelled = self.cancelled.clone();
        let chunk = self.runtime.block_on(async {
            let cancel = Box::pin(cancelled.wait_for(|cancelled| *cancelled));
            match future::select(Box::pin(reader.chunks.recv()), cancel).await {
                Either::Left((chunk, _)) => Ok(chunk),
                Either::Right(_) => Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled")),
            }
        })?;
        match chunk {
            Some(Ok(chunk)) => Ok(Some(chunk)),
            Some(Err(error)) => {
                *self.failure.lock().unwrap() = Some(error.clone());
                Err(io::Error::other(error))
            },
            None => Ok(None),
        }
    }
}

impl mpv::Stream for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.consumed == self.pending.len() {
            match self.next()? {
                Some(chunk) => {
                    self.pending = chunk;
                    self.consumed = 0;
                },
                None => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len() - self.consumed);
        buf[..n].copy_from_slice(&self.pending[self.consumed..self.consumed + n]);
        self.consumed += n;
        self.position += n as u64;
        Ok(n)
    }

    fn seek(&mut self, offset: u64) -> io::Result<u64> {
        if !self.seekable {
            return Err(io::ErrorKind::Unsupported.into());
        }
        if offset == self.position {
            return Ok(offset);
        }
        if offset > self.position && offset - self.position <= SKIP && self.reader.is_some() {
            while self.position < offset {
                if self.consumed == self.pending.len() {
                    match self.next()? {
                        Some(chunk) => (self.pending, self.consumed) = (chunk, 0),
                        None => break,
                    }
                    continue;
                }
                let n = (self.pending.len() - self.consumed).min((offset - self.position) as usize);
                self.consumed += n;
                self.position += n as u64;
            }
            if self.position == offset {
                return Ok(offset);
            }
        }
        self.reader = None;
        self.pending.clear();
        self.consumed = 0;
        self.position = offset;
        Ok(offset)
    }

    fn size(&mut self) -> Option<u64> {
        self.size
    }

    fn seekable(&self) -> bool {
        self.seekable
    }

    fn canceler(&self) -> Option<Box<dyn Fn() + Send + Sync>> {
        let cancel = self.cancel.clone();
        Some(Box::new(move || {
            let _ = cancel.send(true);
        }))
    }
}

/// Sends `path` from `start` to `send` until the end, or until nobody's listening. A ranged read
/// that breaks off, or ends before the file does, carries on from where it got to.
async fn fetch(
    api: Api,
    path: String,
    start: u64,
    size: Option<u64>,
    ranged: bool,
    mut response: Option<reqwest::Response>,
    send: mpsc::Sender<Result<Vec<u8>, String>>,
) {
    let mut at = start;
    let mut failures = 0;
    loop {
        if size.is_some_and(|size| at >= size) {
            return;
        }
        let failure = match read(&api, &path, &mut at, ranged, response.take(), &send).await {
            Ok(()) if !ranged || size.is_none_or(|size| at >= size) => return,
            Ok(()) => "the server stopped sending the video".to_owned(),
            Err(Failure::Gone) => return,
            Err(Failure::Refused(message)) => {
                let _ = send.send(Err(message)).await;
                return;
            },
            Err(Failure::Broken(message)) => message,
        };
        failures += 1;
        if !ranged || failures > RETRIES {
            let _ = send.send(Err(failure)).await;
            return;
        }
        tracing::info!("{path} broke off at byte {at} ({failure}), trying again");
        tokio::time::sleep(Duration::from_secs(1 << (failures - 1))).await;
    }
}

enum Failure {
    /// Nobody reads the stream anymore.
    Gone,
    /// The server said no; asking again won't help.
    Refused(String),
    /// The connection broke.
    Broken(String),
}

async fn read(
    api: &Api,
    path: &str,
    at: &mut u64,
    ranged: bool,
    response: Option<reqwest::Response>,
    send: &mpsc::Sender<Result<Vec<u8>, String>>,
) -> Result<(), Failure> {
    let mut response = match response {
        Some(response) => response,
        None => {
            let mut request = api.stream(path).map_err(|error| Failure::Refused(error.message))?;
            if ranged {
                request = request.header(header::RANGE, format!("bytes={at}-"));
            }
            let response = request.send().await.map_err(|error| Failure::Broken(describe(error)))?;
            if response.status().is_server_error() {
                return Err(Failure::Broken(format!("the server said {}", response.status().as_u16())));
            }
            let response = checked(response).await.map_err(Failure::Refused)?;
            if ranged && *at > 0 && response.status() != StatusCode::PARTIAL_CONTENT {
                return Err(Failure::Refused("the server can't seek in this file".into()));
            }
            response
        },
    };
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                *at += chunk.len() as u64;
                if send.send(Ok(chunk.to_vec())).await.is_err() {
                    return Err(Failure::Gone);
                }
            },
            Ok(None) => return Ok(()),
            Err(error) => return Err(Failure::Broken(describe(error))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use url::Url;

    use super::*;

    fn file() -> Vec<u8> {
        (0..300_000u32).map(|i| (i % 251) as u8).collect()
    }

    /// Serves `file()` with Range support. `cut` makes the first response stop after that many
    /// bytes, like a dropped connection.
    async fn server(cut: Option<usize>) -> (Url, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let count = Arc::new(AtomicUsize::new(0));
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let seen = seen.clone();
                let count = count.clone();
                tokio::spawn(async move {
                    let mut head = Vec::new();
                    while !head.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        if socket.read(&mut byte).await.unwrap_or(0) == 0 {
                            return;
                        }
                        head.push(byte[0]);
                    }
                    let head = String::from_utf8(head).unwrap();
                    seen.lock().unwrap().push(head.clone());
                    let data = file();
                    let start = head
                        .lines()
                        .find_map(|line| line.to_lowercase().strip_prefix("range: bytes=").map(str::to_owned))
                        .map(|range| range.trim_end_matches('-').parse::<usize>().unwrap());
                    let first = count.fetch_add(1, Ordering::SeqCst) == 0;
                    let response = match start.filter(|_| !head.contains("/missing/")) {
                        Some(start) => {
                            let body = &data[start..];
                            let mut response = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\r\n",
                                data.len() - 1,
                                data.len(),
                                body.len()
                            )
                            .into_bytes();
                            match cut.filter(|_| first) {
                                Some(cut) => response.extend_from_slice(&body[..cut]),
                                None => response.extend_from_slice(body),
                            }
                            response
                        },
                        None => b"HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: 27\r\n\r\n{\"error\":\"no such episode\"}".to_vec(),
                    };
                    let _ = socket.write_all(&response).await;
                });
            }
        });
        (origin, requests)
    }

    fn opener(origin: Url, runtime: &tokio::runtime::Runtime) -> Opener {
        let api = Api::new(origin, Some(crate::api::Token("media-session".into()))).unwrap();
        Opener::new(api, runtime.handle().clone())
    }

    #[test]
    fn reads_seeks_and_authenticates() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (origin, requests) = runtime.block_on(server(None));
        let opener = opener(origin, &runtime);
        let mut stream = opener.open(&url("api/media/1/file")).unwrap();
        assert_eq!(stream.size(), Some(300_000));
        assert!(stream.seekable());

        let mut start = [0; 1000];
        stream.read_exact_like(&mut start);
        assert_eq!(start[..], file()[..1000]);

        // Ahead: read through. Back: a new request from there.
        stream.seek(250_000).unwrap();
        let mut ahead = [0; 10];
        stream.read_exact_like(&mut ahead);
        assert_eq!(ahead[..], file()[250_000..250_010]);
        stream.seek(100).unwrap();
        let mut back = [0; 10];
        stream.read_exact_like(&mut back);
        assert_eq!(back[..], file()[100..110]);

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.to_lowercase().contains("authorization: bearer media-session")));
        assert!(requests[1].to_lowercase().contains("range: bytes=100-"));
    }

    #[test]
    fn picks_up_a_dropped_connection_where_it_stopped() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (origin, requests) = runtime.block_on(server(Some(120_000)));
        let opener = opener(origin, &runtime);
        let mut stream = opener.open(&url("api/media/1/file")).unwrap();
        let mut all = Vec::new();
        Reader(&mut *stream).read_to_end(&mut all).unwrap();
        assert_eq!(all, file());
        assert!(requests.lock().unwrap()[1].to_lowercase().contains("range: bytes=120000-"));
    }

    #[test]
    fn says_why_it_cannot_open() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (origin, _) = runtime.block_on(server(None));
        let opener = opener(origin, &runtime);
        assert!(opener.open(&url("api/media/missing/file")).is_none());
        assert_eq!(opener.failure.lock().unwrap().as_deref(), Some("no such episode"));
    }

    #[test]
    fn cancelling_interrupts_a_waiting_read() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let listener = runtime.block_on(TcpListener::bind("127.0.0.1:0")).unwrap();
        let origin = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        runtime.spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut head = [0; 1024];
            let _ = socket.read(&mut head).await;
            // A transcode that has started but sends nothing yet.
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        let opener = opener(origin, &runtime);
        let mut stream = opener.open(&url("api/media/1/stream?start=0")).unwrap();
        assert!(!stream.seekable());
        assert_eq!(stream.size(), None);
        let cancel = stream.canceler().unwrap();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            cancel();
        });
        let mut buf = [0; 16];
        assert_eq!(stream.read(&mut buf).unwrap_err().kind(), io::ErrorKind::Interrupted);
    }

    struct Reader<'a>(&'a mut dyn mpv::Stream);

    impl io::Read for Reader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    trait ReadExactLike {
        fn read_exact_like(&mut self, buf: &mut [u8]);
    }

    impl ReadExactLike for Box<dyn mpv::Stream> {
        fn read_exact_like(&mut self, buf: &mut [u8]) {
            Reader(&mut **self).read_exact(buf).unwrap();
        }
    }
}
