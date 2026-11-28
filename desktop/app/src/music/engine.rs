// SPDX-License-Identifier: AGPL-3.0-or-later
//! The music player's sound: a thread decodes the queue with the sequencer (decoder/core) into the
//! output's ring, and the output plays it. Where we are is worked out from the frames played and
//! where each track began, as on web (`web/src/music/engine.ts`).

use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use tinystream_decoder_core::RangeSource;
use tinystream_decoder_core::sequence::{self, Event, Sequencer};

use super::model::Item;
use super::output::Output;
use crate::api::Api;
use crate::platform::{Current, Desktop};

/// Frames decoded at a time.
const CHUNK: usize = 4096;
/// Frames past what's been played that may be on their way out already, so they're never redone.
const PLAYING: u64 = 8192;

#[derive(Debug, Clone)]
pub enum Update {
    /// The output went away or changed: whatever was playing has to be loaded again.
    Reload,
    /// Where in the track you hear now, and the rate sound goes out at.
    Position {
        key: String,
        time: f64,
        rate: u32,
    },
    /// A track began playing (it's audible now, not just decoded).
    Track(String),
    /// Everything queued has been played.
    Ended,
    Starved(bool),
    Error(String, String),
}

enum Command {
    Load(Vec<Item>, f64, Arc<AtomicBool>),
    Upcoming(String, Vec<Item>),
    Gain(String, f32),
    Playing(bool),
    Volume(f32),
    Stop,
}

pub struct Engine {
    commands: mpsc::Sender<Command>,
    cancel: Arc<AtomicBool>,
}
impl Engine {
    pub fn new(api: Api, runtime: tokio::runtime::Handle, emit: Arc<dyn Fn(Update) + Send + Sync>) -> Self {
        let (commands, receive) = mpsc::channel();
        std::thread::spawn(move || run(api, runtime, receive, emit));
        Self { commands, cancel: Arc::new(AtomicBool::new(false)) }
    }

    /// Plays `items` in order, from `start` seconds into the first, throwing away whatever was
    /// lined up.
    pub fn load(&mut self, items: Vec<Item>, start: f64) {
        self.cancel.store(true, Ordering::Release);
        self.cancel = Arc::new(AtomicBool::new(false));
        let _ = self.commands.send(Command::Load(items, start, self.cancel.clone()));
    }

    /// What comes after `after` changed.
    pub fn upcoming(&self, after: String, items: Vec<Item>) {
        let _ = self.commands.send(Command::Upcoming(after, items));
    }

    pub fn gain(&self, key: String, gain: f32) {
        let _ = self.commands.send(Command::Gain(key, gain));
    }

    pub fn play(&self) {
        let _ = self.commands.send(Command::Playing(true));
    }

    pub fn pause(&self) {
        let _ = self.commands.send(Command::Playing(false));
    }

    pub fn volume(&self, volume: f32) {
        let _ = self.commands.send(Command::Volume(volume));
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Source {
    api: Api,
    path: String,
    size: u64,
    runtime: tokio::runtime::Handle,
    cancel: Arc<AtomicBool>,
}
impl Source {
    fn request<T>(&self, future: impl Future<Output = io::Result<T>>) -> io::Result<T> {
        self.runtime.block_on(async {
            tokio::pin!(future);
            loop {
                if self.cancel.load(Ordering::Acquire) {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "Playback cancelled"));
                }
                if let futures::future::Either::Left((result, _)) =
                    futures::future::select(future.as_mut(), Box::pin(tokio::time::sleep(Duration::from_millis(20))))
                        .await
                {
                    return result;
                }
            }
        })
    }
}
impl RangeSource for Source {
    fn len(&self) -> io::Result<u64> {
        if self.size > 0 {
            return Ok(self.size);
        }
        self.request(async {
            let response = self
                .api
                .head(&self.path)
                .map_err(|e| io::Error::other(e.message))?
                .send()
                .await
                .map_err(io::Error::other)?;
            if !response.status().is_success() {
                return Err(io::Error::other("Couldn't get the audio size"));
            }
            response.content_length().filter(|n| *n > 0).ok_or_else(|| io::Error::other("Missing audio size"))
        })
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        if end < start || end - start >= 512 * 1024 {
            return Err(io::Error::other("Invalid audio range"));
        }
        self.request(async {
            let mut response = self
                .api
                .media(&self.path)
                .map_err(|e| io::Error::other(e.message))?
                .header("Range", format!("bytes={start}-{end}"))
                .send()
                .await
                .map_err(io::Error::other)?;
            if response.status() != reqwest::StatusCode::PARTIAL_CONTENT
                || !response
                    .headers()
                    .get("Content-Range")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.starts_with(&format!("bytes {start}-{end}/")))
            {
                return Err(io::Error::other("The server returned the wrong audio range"));
            }
            let count = (end - start + 1) as usize;
            let mut bytes = Vec::with_capacity(count);
            while let Some(chunk) = response.chunk().await.map_err(io::Error::other)? {
                if bytes.len() + chunk.len() > count {
                    return Err(io::Error::other("Oversized audio response"));
                }
                bytes.extend_from_slice(&chunk);
            }
            if bytes.len() != count {
                return Err(io::Error::other("The audio download stopped"));
            }
            Ok(bytes)
        })
    }
}

/// A track kept on this computer (#97).
struct Kept(std::sync::Mutex<std::fs::File>);

impl RangeSource for Kept {
    fn len(&self) -> io::Result<u64> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).metadata()?.len())
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        if end < start || end - start >= 512 * 1024 {
            return Err(io::Error::other("Invalid audio range"));
        }
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = vec![0; (end - start + 1) as usize];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

/// `key` begins at `frame`, `offset` seconds in.
struct Mark {
    key: String,
    frame: u64,
    offset: f64,
    duration: f64,
}

fn run(
    api: Api,
    runtime: tokio::runtime::Handle,
    commands: mpsc::Receiver<Command>,
    emit: Arc<dyn Fn(Update) + Send + Sync>,
) {
    let mut output: Option<Output> = None;
    let mut sequencer = Sequencer::new(48000);
    let mut cancel = Arc::new(AtomicBool::new(false));
    let mut marks: VecDeque<Mark> = VecDeque::new();
    let mut end: Option<u64> = None;
    let mut current: Option<String> = None;
    let mut playing = false;
    let mut volume = 1.0;
    let mut starved = false;
    let mut progress = Instant::now();
    let mut device_check = Instant::now();
    let mut buffer = vec![0.0; CHUNK * 2];
    let native = |items: Vec<Item>, cancel: &Arc<AtomicBool>| {
        items
            .into_iter()
            .map(|item| sequence::Item {
                source: match crate::offline::local(&api, &item.path).and_then(|file| std::fs::File::open(file).ok()) {
                    Some(file) => Arc::new(Kept(std::sync::Mutex::new(file))) as Arc<dyn RangeSource>,
                    None => Arc::new(Source {
                        api: api.clone(),
                        runtime: runtime.clone(),
                        path: item.path,
                        size: item.size,
                        cancel: cancel.clone(),
                    }),
                },
                key: item.key,
                size: item.size,
                ext: item.ext,
                gain: item.gain,
                crossfade: item.crossfade,
            })
            .collect::<Vec<_>>()
    };
    loop {
        let busy = playing || output.as_ref().is_some_and(|o| !sequencer.finished() && o.ring().room() >= CHUNK as u64);
        let command = match commands.recv_timeout(Duration::from_millis(if busy { 10 } else { 200 })) {
            Ok(command) => Some(command),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        let mut events = vec![];
        match command {
            Some(Command::Load(items, start, token)) => {
                cancel = token;
                if cancel.load(Ordering::Acquire) {
                    continue;
                }
                if output.is_none() {
                    match Current::audio_output() {
                        Ok(audio) => {
                            audio.volume(volume);
                            output = Some(audio);
                        },
                        Err(error) => {
                            let key = items.first().map_or(String::new(), |i| i.key.clone());
                            emit(Update::Error(key, error.to_string()));
                            continue;
                        },
                    }
                }
                let audio = output.as_ref().unwrap();
                if sequencer.rate() != audio.rate {
                    sequencer = Sequencer::new(audio.rate);
                }
                marks.clear();
                end = None;
                current = None;
                let at = audio.ring().flush();
                sequencer.play(native(items, &cancel), start, at);
                audio.playing(playing);
                events = sequencer.events();
            },
            Some(Command::Upcoming(after, items)) => {
                end = None;
                if let Some(audio) = &output {
                    if let Some(back) =
                        sequencer.upcoming(&after, native(items, &cancel), audio.ring().heard() + PLAYING)
                    {
                        audio.ring().rewind(back);
                    }
                    events = sequencer.events();
                }
            },
            Some(Command::Gain(key, gain)) => sequencer.gain(&key, gain),
            Some(Command::Playing(value)) => {
                playing = value;
                if let Some(audio) = &output {
                    audio.playing(value);
                }
            },
            Some(Command::Volume(value)) => {
                volume = value;
                if let Some(audio) = &output {
                    audio.volume(value);
                }
            },
            Some(Command::Stop) => {
                playing = false;
                sequencer.stop();
                marks.clear();
                end = None;
                current = None;
                if let Some(audio) = &output {
                    audio.playing(false);
                    audio.ring().flush();
                }
            },
            None => {},
        }
        let Some(audio) = &output else { continue };
        if device_check.elapsed() >= Duration::from_secs(2) {
            device_check = Instant::now();
            if audio.failed() || audio.device_changed() {
                output = None;
                marks.clear();
                sequencer.stop();
                if current.take().is_some() {
                    emit(Update::Reload);
                }
                continue;
            }
        }
        if cancel.load(Ordering::Acquire) {
            continue;
        }
        let mut guard = 0;
        while audio.ring().room() >= CHUNK as u64 && !sequencer.finished() && guard < 64 {
            guard += 1;
            let frames = sequencer.read(&mut buffer);
            if cancel.load(Ordering::Acquire) {
                break;
            }
            audio.ring().write(&buffer[..frames * 2]);
            events.extend(sequencer.events());
            if frames < CHUNK {
                break;
            }
        }
        if cancel.load(Ordering::Acquire) {
            continue;
        }
        for event in events {
            match event {
                Event::Mark { key, frame, offset, duration } => {
                    let at = marks.iter().position(|m| m.frame > frame).unwrap_or(marks.len());
                    marks.insert(at, Mark { key, frame, offset, duration });
                },
                Event::Rewind { frame } => {
                    marks.retain(|m| m.frame < frame);
                    end = None;
                },
                Event::End { frame } => end = Some(frame),
                Event::Error { key, message } => emit(Update::Error(key, message)),
            }
        }
        let heard = audio.heard();
        let mark = marks.iter().rposition(|m| m.frame <= heard);
        if let Some(i) = mark {
            if current.as_deref() != Some(marks[i].key.as_str()) {
                current = Some(marks[i].key.clone());
                emit(Update::Track(marks[i].key.clone()));
            }
            // Marks long behind are of no more use.
            if i > 4 {
                marks.drain(..i - 1);
            }
        }
        if playing && end.is_some_and(|frame| heard >= frame) {
            end = None;
            emit(Update::Ended);
        }
        if progress.elapsed() >= Duration::from_millis(200)
            && let Some(m) = marks.iter().rev().find(|m| m.frame <= heard)
        {
            progress = Instant::now();
            let time = m.offset + (heard - m.frame) as f64 / audio.rate as f64;
            emit(Update::Position {
                key: m.key.clone(),
                time: if m.duration > 0.0 { time.min(m.duration) } else { time },
                rate: audio.rate,
            });
        }
        let now = playing && audio.starved();
        if now != starved {
            starved = now;
            emit(Update::Starved(starved));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;
    fn response(status: &str, headers: &str, body: &str, read: impl FnOnce(Source)) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!("HTTP/1.1 {status}\r\n{headers}Connection: close\r\n\r\n{body}");
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut buffer = [0; 4096];
            let n = socket.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..n]);
            assert!(request.to_ascii_lowercase().contains("authorization: bearer test-token"));
            assert!(request.to_ascii_lowercase().contains("range: bytes=1-3"));
            socket.write_all(response.as_bytes()).unwrap();
        });
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        read(Source {
            api: Api::new(
                url::Url::parse(&format!("http://{address}/")).unwrap(),
                Some(crate::api::Token("test-token".into())),
            )
            .unwrap(),
            path: "/audio".into(),
            size: 5,
            runtime: runtime.handle().clone(),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        server.join().unwrap();
    }
    #[test]
    fn kept_tracks_read_from_disk() {
        let file = std::env::temp_dir().join(format!("tinystream-kept-{}", std::process::id()));
        std::fs::write(&file, b"abcdef").unwrap();
        let kept = Kept(std::sync::Mutex::new(std::fs::File::open(&file).unwrap()));
        assert_eq!(kept.len().unwrap(), 6);
        assert_eq!(kept.read_range(1, 3).unwrap(), b"bcd");
        assert!(kept.read_range(4, 9).is_err());
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn authenticated_audio_ranges_are_exact_and_bounded() {
        response("206 Partial Content", "Content-Range: bytes 1-3/5\r\nContent-Length: 3\r\n", "bcd", |source| {
            assert_eq!(source.read_range(1, 3).unwrap(), b"bcd")
        });
        response("206 Partial Content", "Content-Range: bytes 0-2/5\r\nContent-Length: 3\r\n", "abc", |source| {
            assert!(source.read_range(1, 3).is_err())
        });
        response("302 Found", "Location: http://127.0.0.1:1/stolen\r\nContent-Length: 0\r\n", "", |source| {
            assert!(source.read_range(1, 3).is_err())
        });
    }
}

#[cfg(test)]
mod output_tests {
    use std::io::{Read, Write};

    use super::*;
    #[test]
    #[ignore = "requires an ALSA output; plays silent decoded fixture audio through CPAL"]
    fn native_output_plays_two_tracks_and_releases_on_stop() {
        let fixture = include_bytes!("../../../../decoder/core/tests/fixtures/a.flac").to_vec();
        let size = fixture.len() as u64;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let stop = done.clone();
        let server = std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                        let mut bytes = vec![];
                        let mut chunk = [0; 4096];
                        loop {
                            let n = socket.read(&mut chunk).unwrap_or(0);
                            if n == 0 {
                                break;
                            }
                            bytes.extend_from_slice(&chunk[..n]);
                            if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        let headers = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
                        let range = headers.lines().find_map(|line| line.strip_prefix("range: bytes=")).unwrap();
                        let (start, end) = range.split_once('-').unwrap();
                        let start: usize = start.trim().parse().unwrap();
                        let end: usize = end.trim().parse().unwrap();
                        let body = &fixture[start..=end];
                        let response = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            fixture.len(),
                            body.len()
                        );
                        let _ = socket.write_all(response.as_bytes());
                        let _ = socket.write_all(body);
                    },
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let api = Api::new(url::Url::parse(&format!("http://{address}/")).unwrap(), None).unwrap();
        let (tx, rx) = mpsc::channel();
        let mut engine = Engine::new(
            api,
            runtime.handle().clone(),
            Arc::new(move |event| {
                let _ = tx.send(event);
            }),
        );
        let items = (0..2)
            .map(|i| Item {
                key: format!("e{i}"),
                path: "/fixture".into(),
                size,
                ext: "flac".into(),
                gain: 1.0,
                crossfade: 0.0,
            })
            .collect();
        engine.volume(0.0);
        engine.load(items, 0.0);
        engine.play();
        let mut started = vec![];
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut ended = false;
        let mut error = None;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Update::Track(key)) => started.push(key),
                Ok(Update::Ended) => {
                    ended = true;
                    break;
                },
                Ok(Update::Error(_, message)) => {
                    error = Some(message);
                    break;
                },
                _ => {},
            }
        }
        engine.stop();
        drop(engine);
        done.store(true, Ordering::Release);
        server.join().unwrap();
        assert!(error.is_none(), "{error:?}");
        assert!(ended, "CPAL output did not finish");
        assert_eq!(started, vec!["e0", "e1"]);
    }
}
