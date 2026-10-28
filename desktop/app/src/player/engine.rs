// SPDX-License-Identifier: AGPL-3.0-or-later

//! One mpv client per player: the options the app always sets, the user's `mpv.conf` under them,
//! what to load for direct play or a transcode, and mpv's events as the player's messages.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mpv::{Event, Format, Mpv, Node, Value};

use super::stream::{self, Opener};
use crate::api::Api;

/// What the app controls, set again after `mpv.conf` so it can't be overridden.
const FORCED: &[(&str, &str)] = &[
    ("vo", "libmpv"),
    // VA-API on AMD and Intel, NVDEC on NVIDIA through CUDA interop.
    ("hwdec", "auto-safe"),
    // rustyass draws text subtitles (#87), and there's no OSD.
    ("sid", "no"),
    ("osd-level", "0"),
    ("input-default-bindings", "no"),
    ("input-builtin-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("terminal", "no"),
    ("resume-playback", "no"),
    ("save-position-on-quit", "no"),
    // A transcode's timestamps are the file's own, from wherever it started.
    ("rebase-start-time", "no"),
    ("keep-open", "yes"),
    ("idle", "yes"),
    ("audio-client-name", "tinystream"),
    // About a minute ahead and half a minute behind, like the web's BUFFER_AHEAD/KEEP_BEHIND; the
    // byte limits leave room for that at a 4K remux's bitrate.
    ("cache", "yes"),
    ("cache-secs", "60"),
    ("demuxer-readahead-secs", "60"),
    ("demuxer-max-bytes", "800MiB"),
    ("demuxer-max-back-bytes", "400MiB"),
    ("demuxer-seekable-cache", "yes"),
];

/// What plays when there's no `mpv.conf`: mpv's defaults, which integrated GPUs keep up with, and
/// debanding for the gradients anime is full of.
const DEFAULTS: &[(&str, &str)] = &[("deband", "yes")];

/// Where the user's own mpv options go.
pub fn config_file() -> Option<PathBuf> {
    crate::paths::config_dir().map(|dir| dir.join("mpv.conf"))
}

/// What's played: the file as it is, or a transcode to `height` from `start`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Direct,
    Transcode { height: u32, start: f64 },
}

impl Source {
    /// The server path for media `base` (like `api/media/12`), with `audio` the track to keep.
    pub fn path(self, base: &str, audio: Option<i64>, audio_codec: Option<&str>) -> String {
        match self {
            Source::Direct => format!("{base}/file"),
            Source::Transcode { height, start } => {
                let mut path = format!("{base}/stream?start={start:.3}&video=transcode&height={height}");
                if let Some(audio) = audio {
                    path.push_str(&format!("&audio={audio}"));
                }
                // mpv plays anything, but the transcode's MP4 only carries some codecs as they are.
                let copy = audio_codec
                    .is_some_and(|codec| matches!(codec, "aac" | "ac3" | "eac3" | "opus" | "flac" | "mp3" | "alac"));
                path.push_str(if copy { "&audioMode=copy" } else { "&audioMode=aac" });
                path
            },
        }
    }
}

/// What mpv says, for the player.
#[derive(Debug, Clone)]
pub enum Update {
    /// Where playback is, in the file's own time.
    Time(f64),
    Paused(bool),
    /// Waiting for data (or a seek) rather than playing.
    Waiting(bool),
    /// Playback reached the end of what was loaded.
    Eof(bool),
    /// How far ahead of the playhead is cached, in file time.
    Cached(f64),
    /// The picture's display size changed (`dwidth`/`dheight`).
    Size(i64, i64),
    /// A file is ready, with mpv's audio tracks as (mpv id, FFmpeg stream index).
    Loaded(Vec<(i64, i64)>),
    /// The first frame after loading or seeking is up.
    Restarted,
    /// Loading or playing failed, with why.
    Failed(String),
}

/// Observed property ids.
const TIME: u64 = 1;
const PAUSE: u64 = 2;
const WAITING: u64 = 3;
const SEEKING: u64 = 4;
const EOF: u64 = 5;
const CACHED: u64 = 6;
const WIDTH: u64 = 7;
const HEIGHT: u64 = 8;

pub struct Engine {
    pub mpv: Mpv,
    opener: Opener,
    alive: Arc<AtomicBool>,
}

impl Engine {
    pub fn new(api: Api, runtime: tokio::runtime::Handle) -> Result<Self, String> {
        Self::with_config(api, runtime, config_file())
    }

    fn with_config(api: Api, runtime: tokio::runtime::Handle, config: Option<PathBuf>) -> Result<Self, String> {
        let mut options: Vec<(&str, String)> = FORCED.iter().map(|&(name, value)| (name, value.to_owned())).collect();
        match config.as_ref().and_then(|file| file.parent()) {
            Some(dir) if config.as_ref().is_some_and(|file| file.is_file()) => {
                options.push(("config", "yes".into()));
                options.push(("config-dir", dir.to_string_lossy().into_owned()));
            },
            _ => options.extend(DEFAULTS.iter().map(|&(name, value)| (name, value.to_owned()))),
        }
        let options: Vec<(&str, &str)> = options.iter().map(|(name, value)| (*name, value.as_str())).collect();
        let mpv = Mpv::new(&options).map_err(|error| error.to_string())?;
        for (name, value) in FORCED {
            if let Err(error) = mpv.set_property(name, *value) {
                tracing::debug!("can't set mpv's {name} again: {error}");
            }
        }

        let opener = Opener::new(api, runtime);
        let open = opener.clone();
        mpv.add_protocol(stream::PROTOCOL, move |uri| open.open(uri)).map_err(|error| error.to_string())?;

        for (id, name, format) in [
            (TIME, "time-pos", Format::Double),
            (PAUSE, "pause", Format::Flag),
            (WAITING, "paused-for-cache", Format::Flag),
            (SEEKING, "seeking", Format::Flag),
            (EOF, "eof-reached", Format::Flag),
            (CACHED, "demuxer-cache-time", Format::Double),
            (WIDTH, "dwidth", Format::Int64),
            (HEIGHT, "dheight", Format::Int64),
        ] {
            mpv.observe_property(id, name, format).map_err(|error| error.to_string())?;
        }
        mpv.request_log_messages("warn").map_err(|error| error.to_string())?;

        Ok(Self { mpv, opener, alive: Arc::new(AtomicBool::new(true)) })
    }

    /// Whether the engine is still around, for what runs beside it.
    pub fn alive(&self) -> Arc<AtomicBool> {
        self.alive.clone()
    }

    /// What sends mpv's news to `send`, from a thread of its own, until the engine is dropped.
    pub fn listener(&self) -> impl FnOnce(Box<dyn Fn(Update) + Send>) + Send + 'static + use<> {
        let (mpv, alive, failure) = (self.mpv.clone(), self.alive.clone(), self.opener.failure.clone());
        move |send| listen(mpv, alive, failure, send)
    }

    /// Plays `path` on the server from `start` seconds.
    pub fn load(&self, path: &str, start: f64, paused: bool) {
        let options = format!("start={start:.3},pause={}", if paused { "yes" } else { "no" });
        self.command(&["loadfile", &stream::url(path), "replace", "-1", &options]);
    }

    pub fn frame_step(&self, back: bool) {
        self.command(&[if back { "frame-back-step" } else { "frame-step" }]);
    }

    pub fn seek(&self, to: f64) {
        self.command(&["seek", &format!("{to:.3}"), "absolute"]);
    }

    pub fn bitmap_subtitle(&self, index: Option<i64>) {
        let id = self.mpv.get_property::<Node>("track-list").ok().and_then(|tracks| {
            tracks
                .as_array()
                .iter()
                .find(|track| {
                    track.get("type").and_then(Node::as_str) == Some("sub")
                        && track.get("ff-index").and_then(Node::as_i64) == index
                })
                .and_then(|track| track.get("id").and_then(Node::as_i64))
        });
        match id.filter(|_| index.is_some()) {
            Some(id) => self.set("sid", id),
            None => self.set("sid", "no"),
        }
    }

    pub fn set<T: mpv::SetProperty>(&self, name: &str, value: T) {
        if let Err(error) = self.mpv.set_property(name, value) {
            tracing::debug!("can't set mpv's {name}: {error}");
        }
    }

    /// The cached stretches of file time that can be seeked into without loading anything.
    pub fn seekable(&self) -> Vec<(f64, f64)> {
        let Ok(state) = self.mpv.get_property::<Node>("demuxer-cache-state") else { return Vec::new() };
        let ranges = state.get("seekable-ranges").map(Node::as_array).unwrap_or_default();
        ranges.iter().filter_map(|range| Some((range.get("start")?.as_f64()?, range.get("end")?.as_f64()?))).collect()
    }

    fn command(&self, args: &[&str]) {
        if let Err(error) = self.mpv.command_async(0, args) {
            tracing::warn!("mpv {}: {error}", args[0]);
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
        let _ = self.mpv.command(&["stop"]);
    }
}

fn listen(
    mpv: Mpv,
    alive: Arc<AtomicBool>,
    failure: Arc<std::sync::Mutex<Option<String>>>,
    send: Box<dyn Fn(Update) + Send>,
) {
    let spawned = std::thread::Builder::new().name("mpv events".into()).spawn(move || {
        let (mut width, mut height) = (0, 0);
        let mut time = None;
        let mut sent = Instant::now();
        while alive.load(Ordering::Relaxed) {
            let Some(event) = mpv.wait_event(0.25) else {
                // Time moves on its own; it's sent a few times a second rather than every frame.
                if let Some(at) = time.take() {
                    send(Update::Time(at));
                }
                continue;
            };
            match event {
                Event::Shutdown => break,
                Event::PropertyChange { id: TIME, value: Value::Double(at), .. } => {
                    time = Some(at);
                    if sent.elapsed() >= Duration::from_millis(200) {
                        sent = Instant::now();
                        send(Update::Time(at));
                        time = None;
                    }
                },
                Event::PropertyChange { id: PAUSE, value: Value::Flag(paused), .. } => send(Update::Paused(paused)),
                Event::PropertyChange { id: WAITING | SEEKING, .. } => {
                    let waiting = mpv.get_property::<bool>("paused-for-cache").unwrap_or(false)
                        || mpv.get_property::<bool>("seeking").unwrap_or(false);
                    send(Update::Waiting(waiting));
                },
                Event::PropertyChange { id: EOF, value, .. } => send(Update::Eof(value == Value::Flag(true))),
                Event::PropertyChange { id: CACHED, value: Value::Double(at), .. } => send(Update::Cached(at)),
                Event::PropertyChange { id: WIDTH | HEIGHT, name, value, .. } => {
                    let value = if let Value::Int64(value) = value { value } else { 0 };
                    if name == "dwidth" {
                        width = value
                    } else {
                        height = value
                    }
                    if width > 0 && height > 0 {
                        send(Update::Size(width, height));
                    }
                },
                Event::FileLoaded => {
                    *failure.lock().unwrap() = None;
                    send(Update::Loaded(audio_tracks(&mpv)));
                },
                Event::PlaybackRestart => {
                    if let Ok(decoder) = mpv.get_property::<String>("hwdec-current") {
                        tracing::debug!("decoding with {decoder}");
                    }
                    if let Ok(at) = mpv.get_property::<f64>("time-pos") {
                        send(Update::Time(at));
                    }
                    send(Update::Restarted);
                },
                Event::EndFile(mpv::EndReason::Error(error)) => {
                    let why = failure.lock().unwrap().take().unwrap_or_else(|| error.to_string());
                    send(Update::Failed(why));
                },
                Event::Log { prefix, level, text } => {
                    let text = text.trim_end();
                    match level.as_str() {
                        "fatal" | "error" => tracing::warn!("mpv {prefix}: {text}"),
                        _ => tracing::debug!("mpv {prefix}: {text}"),
                    }
                },
                _ => {},
            }
        }
    });
    if let Err(error) = spawned {
        tracing::error!("can't follow mpv: {error}");
    }
}

/// mpv's audio tracks, as (its id, the FFmpeg stream index the server's `index` is).
fn audio_tracks(mpv: &Mpv) -> Vec<(i64, i64)> {
    let Ok(tracks) = mpv.get_property::<Node>("track-list") else { return Vec::new() };
    tracks
        .as_array()
        .iter()
        .filter(|track| track.get("type").and_then(Node::as_str) == Some("audio"))
        .filter_map(|track| Some((track.get("id")?.as_i64()?, track.get("ff-index")?.as_i64()?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs a test server, a video with two audio tracks, and a hardware decoder"]
    fn direct_play_seeks_tracks_and_transcodes() {
        use crate::platform::{Current, Desktop};

        let origin = std::env::var("TINYSTREAM_TEST_SERVER").expect("set TINYSTREAM_TEST_SERVER").parse().unwrap();
        let token = std::env::var("TINYSTREAM_TEST_TOKEN").ok().map(crate::api::Token);
        let id = std::env::var("TINYSTREAM_TEST_VIDEO_ID").unwrap_or_else(|_| "1".into()).parse().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let api = Api::new(origin, token).unwrap();
        let data =
            runtime.block_on(api.request::<crate::api::Playback>(crate::api::playback::Variables { id })).unwrap();
        let pb = data.video.unwrap();
        assert!(pb.media.audio.len() >= 2, "the test video needs two audio tracks");
        let engine = Engine::with_config(api, runtime.handle().clone(), None).unwrap();
        engine.set("volume", 0.0);
        let instance =
            iced_wgpu::wgpu::Instance::new(iced_wgpu::wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = futures::executor::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, _queue) = Current::request_device(&adapter, &Default::default()).unwrap();
        let surface = Current::video(&device, &engine.mpv, Arc::new(|| {})).unwrap();
        surface.resize(320, 180);
        let (send, recv) = std::sync::mpsc::channel();
        engine.listener()(Box::new(move |update| {
            let _ = send.send(update);
        }));
        let wait = |at: f64, loading: bool| {
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut loaded = !loading;
            loop {
                assert!(surface.failure().is_none(), "{:?}", surface.failure());
                if surface.frame().is_some() {
                    surface.presented();
                }
                match recv.recv_timeout(Duration::from_millis(20)) {
                    Ok(Update::Loaded(_)) => loaded = true,
                    Ok(Update::Failed(error)) => panic!("{error}"),
                    Ok(Update::Restarted) if loaded => {
                        let time = engine.mpv.get_property::<f64>("time-pos").unwrap();
                        assert!((time - at).abs() < 2.0, "wanted {at}s, got {time}s");
                        return;
                    },
                    _ => {},
                }
                assert!(Instant::now() < deadline, "playback did not reach {at}s");
            }
        };
        let base = format!("api/media/{id}");
        engine.load(&Source::Direct.path(&base, None, None), 12.0, false);
        wait(12.0, true);
        assert_ne!(engine.mpv.get_property::<String>("hwdec-current").unwrap(), "no");
        let audio = &pb.media.audio[1];
        let aid = audio_tracks(&engine.mpv).into_iter().find(|(_, index)| *index == audio.index).unwrap().0;
        engine.set("aid", aid);
        assert_eq!(engine.mpv.get_property::<i64>("aid").unwrap(), aid);
        engine.seek(50.0);
        wait(50.0, false);
        engine.set("pause", true);
        assert!(engine.mpv.get_property::<bool>("pause").unwrap());
        let path = Source::Transcode { height: 720, start: 30.0 }.path(&base, Some(audio.index), Some(&audio.codec));
        engine.load(&path, 30.0, true);
        wait(30.0, true);
        assert!(engine.mpv.get_property::<bool>("pause").unwrap());
        assert!(
            engine.mpv.get_property::<Node>("track-list").unwrap().as_array().iter().any(|track| {
                track.get("type").and_then(Node::as_str) == Some("audio")
                    && track.get("selected").and_then(Node::as_bool) == Some(true)
            }),
            "transcoding lost the selected audio track"
        );
        engine.load(&Source::Direct.path(&base, None, None), 30.0, false);
        wait(30.0, true);
        assert_ne!(engine.mpv.get_property::<String>("hwdec-current").unwrap(), "no");
    }

    #[test]
    fn user_config_tunes_picture_without_overriding_playback() {
        let dir = crate::test_support::Directory::new("mpv-config");
        let config = dir.0.join("mpv.conf");
        std::fs::write(
            &config,
            "vo=null\nsid=auto\nosd-level=3\ninput-default-bindings=yes\nrebase-start-time=yes\ndeband=no\n",
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let api = Api::new("http://127.0.0.1/".parse().unwrap(), None).unwrap();
        let engine = Engine::with_config(api, runtime.handle().clone(), Some(config)).unwrap();
        assert_eq!(engine.mpv.get_property::<String>("vo").unwrap(), "libmpv");
        assert_eq!(engine.mpv.get_property::<String>("sid").unwrap(), "no");
        assert_eq!(engine.mpv.get_property::<i64>("osd-level").unwrap(), 0);
        assert!(!engine.mpv.get_property::<bool>("input-default-bindings").unwrap());
        assert!(!engine.mpv.get_property::<bool>("rebase-start-time").unwrap());
        assert!(!engine.mpv.get_property::<bool>("deband").unwrap());
    }

    #[test]
    fn mpv_has_every_option_the_app_sets() {
        for &(name, value) in FORCED.iter().chain(DEFAULTS) {
            assert!(Mpv::new(&[(name, value)]).is_ok(), "mpv has no {name}={value}");
        }
    }

    #[test]
    fn transcodes_keep_codecs_mp4_carries() {
        let base = "api/media/12";
        assert_eq!(Source::Direct.path(base, Some(1), Some("truehd")), "api/media/12/file");
        assert_eq!(
            Source::Transcode { height: 720, start: 61.5 }.path(base, Some(2), Some("eac3")),
            "api/media/12/stream?start=61.500&video=transcode&height=720&audio=2&audioMode=copy"
        );
        assert_eq!(
            Source::Transcode { height: 480, start: 0.0 }.path(base, None, Some("dts")),
            "api/media/12/stream?start=0.000&video=transcode&height=480&audioMode=aac"
        );
    }
}
