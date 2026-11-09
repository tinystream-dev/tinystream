// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use tinystream_decoder_core::RangeSource;
use tinystream_decoder_core::sequence::{Event, Item, Sequencer};

use super::model::{Entry, Repeat, Settings, fade, gain};
use super::output::Output;
use crate::api::Api;
use crate::platform::{Current, Desktop};

#[derive(Debug, Clone)]
pub enum Update {
    Reload,
    Position { key: String, time: f64, rate: u32 },
    Started(String),
    Waiting(bool),
    Ended,
    Error(String, String),
}

enum Command {
    Load(Vec<Entry>, Settings, f64, bool, Arc<AtomicBool>, Repeat),
    Upcoming(String, Vec<Entry>, Settings),
    Playing(bool),
    Settings(Settings),
    Gain(String, f32),
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

    pub fn load(&mut self, entries: Vec<Entry>, settings: Settings, time: f64, playing: bool, repeat: Repeat) {
        self.cancel.store(true, Ordering::Release);
        self.cancel = Arc::new(AtomicBool::new(false));
        let _ = self.commands.send(Command::Load(entries, settings, time, playing, self.cancel.clone(), repeat));
    }

    pub fn upcoming(&self, after: String, entries: Vec<Entry>, settings: Settings) {
        let _ = self.commands.send(Command::Upcoming(after, entries, settings));
    }

    pub fn playing(&self, playing: bool) {
        let _ = self.commands.send(Command::Playing(playing));
    }

    pub fn gain(&self, key: String, gain: f32) {
        let _ = self.commands.send(Command::Gain(key, gain));
    }

    pub fn settings(&self, settings: Settings) {
        let _ = self.commands.send(Command::Settings(settings));
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

fn run(
    api: Api,
    runtime: tokio::runtime::Handle,
    commands: mpsc::Receiver<Command>,
    emit: Arc<dyn Fn(Update) + Send + Sync>,
) {
    let mut output: Option<Output> = None;
    let mut sequencer = Sequencer::new(48000);
    let mut cancel = Arc::new(AtomicBool::new(false));
    let mut marks = std::collections::VecDeque::new();
    let mut current: Option<(String, u64, f64, f64)> = None;
    let mut end = None;
    let mut pending = Vec::new();
    let mut at = 0;
    let mut playing = false;
    let mut loop_entries: Vec<Entry> = vec![];
    let mut loop_last = String::new();
    let mut loop_cycle = 0u64;
    let mut looping = false;
    let mut settings = Settings::default();
    let mut progress = Instant::now();
    let mut device_check = Instant::now();
    let mut waiting = false;
    let items = |entries: &[Entry], settings: &Settings, cancel: &Arc<AtomicBool>| {
        entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let direct =
                    !e.fallback && ["flac", "mp3", "aac", "alac", "vorbis", "pcm"].contains(&e.track.codec.as_str());
                Item {
                    key: e.key.clone(),
                    size: if direct { e.track.size } else { 0 },
                    ext: if direct { e.track.suffix.clone() } else { "flac".into() },
                    gain: gain(i, entries, settings, false),
                    crossfade: fade(i, entries, settings),
                    source: Arc::new(Source {
                        api: api.clone(),
                        runtime: runtime.clone(),
                        path: if direct { e.track.file.clone() } else { e.track.flac.clone() },
                        size: if direct { e.track.size } else { 0 },
                        cancel: cancel.clone(),
                    }),
                }
            })
            .collect::<Vec<_>>()
    };
    loop {
        match commands.recv_timeout(Duration::from_millis(if playing { 10 } else { 200 })) {
            Ok(Command::Load(mut entries, next, time, play, token, repeat)) => {
                output = None;
                cancel = token;
                if cancel.load(Ordering::Acquire) {
                    continue;
                }
                match Current::audio_output() {
                    Ok(audio) => {
                        sequencer = Sequencer::new(audio.rate);
                        settings = next;
                        audio.volume(if settings.muted { 0.0 } else { settings.volume });
                        marks.clear();
                        current = None;
                        end = None;
                        pending.clear();
                        at = 0;
                        looping = repeat != Repeat::Off;
                        if repeat == Repeat::One {
                            entries.truncate(1);
                        }
                        loop_entries = entries.clone();
                        loop_last = entries.last().map_or(String::new(), |e| e.key.clone());
                        loop_cycle = 0;
                        sequencer.play(items(&entries, &settings, &cancel), time, 0);
                        playing = play;
                        audio.playing(play);
                        output = Some(audio);
                    },
                    Err(error) => {
                        emit(Update::Error(entries.first().map_or(String::new(), |e| e.key.clone()), error.to_string()))
                    },
                }
            },
            Ok(Command::Upcoming(after, entries, next)) => {
                if let Some(audio) = &output
                    && sequencer.upcoming(&after, items(&entries, &next, &cancel), audio.ring.heard()).is_some()
                {
                    emit(Update::Reload);
                }
            },
            Ok(Command::Playing(value)) => {
                playing = value;
                if let Some(audio) = &output {
                    audio.playing(value);
                }
            },
            Ok(Command::Gain(key, gain)) => sequencer.gain(&key, gain),
            Ok(Command::Settings(next)) => {
                settings = next;
                if let Some(audio) = &output {
                    audio.volume(if settings.muted { 0.0 } else { settings.volume });
                }
            },
            Ok(Command::Stop) => {
                output = None;
                sequencer.stop();
                current = None;
                marks.clear();
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {},
        }
        let Some(audio) = &output else { continue };
        if cancel.load(Ordering::Acquire) {
            continue;
        }
        if device_check.elapsed() >= Duration::from_secs(2) {
            device_check = Instant::now();
            if audio.device_changed() || audio.failed.load(Ordering::Acquire) {
                emit(Update::Reload);
                output = None;
                continue;
            }
        }
        let heard = audio.ring.heard();
        while marks.front().is_some_and(|(_, frame, _, _)| *frame <= heard) {
            current = marks.pop_front();
            emit(Update::Started(current.as_ref().unwrap().0.clone()));
        }
        if progress.elapsed() >= Duration::from_millis(200) {
            progress = Instant::now();
            if let Some((key, frame, offset, duration)) = &current {
                emit(Update::Position {
                    key: key.clone(),
                    time: (offset + heard.saturating_sub(*frame) as f64 / audio.rate as f64).min(*duration),
                    rate: audio.rate,
                });
            }
        }
        if end.is_some_and(|frame| heard >= frame) {
            emit(Update::Ended);
            output = None;
            continue;
        }
        if at >= pending.len()
            && !sequencer.finished()
            && audio.ring.written().saturating_sub(heard) < audio.ring.capacity() / 2
        {
            pending.resize(8192, 0.0);
            let frames = sequencer.read(&mut pending);
            if cancel.load(Ordering::Acquire) {
                pending.clear();
                at = 0;
                continue;
            }
            pending.truncate(frames * 2);
            at = 0;
            for event in sequencer.events() {
                match event {
                    Event::Mark { key, frame, offset, duration } => {
                        if looping && key == loop_last {
                            loop_cycle += 1;
                            let next: Vec<_> = loop_entries
                                .iter()
                                .cloned()
                                .map(|mut e| {
                                    e.key = format!("{}@{}", e.key, loop_cycle);
                                    e
                                })
                                .collect();
                            loop_last = next.last().map_or(String::new(), |e| e.key.clone());
                            sequencer.upcoming(&key, items(&next, &settings, &cancel), audio.ring.heard());
                        }
                        marks.push_back((key, frame, offset, duration));
                    },
                    Event::End { frame } => {
                        if !looping {
                            end = Some(frame);
                        }
                    },
                    Event::Error { key, message } => emit(Update::Error(key, message)),
                    Event::Rewind { .. } => {},
                }
            }
        }
        if at < pending.len() {
            at += audio.ring.write(&pending[at..]) * 2;
        }
        let starved = playing && end.is_none() && audio.ring.written() == heard;
        if waiting != starved {
            waiting = starved;
            emit(Update::Waiting(waiting));
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
        let entries = (0..2)
            .map(|i| Entry {
                key: format!("e{i}"),
                fallback: false,
                track: super::super::model::Track {
                    id: i,
                    title: format!("Track {i}"),
                    codec: "flac".into(),
                    suffix: "flac".into(),
                    size,
                    file: "/fixture".into(),
                    ..Default::default()
                },
            })
            .collect();
        engine.load(entries, Settings { volume: 0.0, ..Settings::default() }, 0.0, true, Repeat::Off);
        let mut started = vec![];
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut ended = false;
        let mut error = None;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Update::Started(key)) => started.push(key),
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
