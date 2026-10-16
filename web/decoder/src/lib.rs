// SPDX-License-Identifier: AGPL-3.0-or-later
//! The decoder (decoder/core) for the browser, reading files through the worker it runs in.

use std::io;
use std::sync::Arc;

use tinystream_decoder_core::sequence::{self, Event as CoreEvent};
use tinystream_decoder_core::{self as core, RangeSource};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Bytes `start..=end` of `url`; the worker answers with a synchronous request.
    #[wasm_bindgen(js_name = tsReadRange, catch)]
    fn read_range(url: &str, start: f64, end: f64) -> Result<Vec<u8>, JsValue>;

    /// How long `url` is, in bytes.
    #[wasm_bindgen(js_name = tsSize, catch)]
    fn size(url: &str) -> Result<f64, JsValue>;
}

fn io_error(e: JsValue) -> io::Error {
    io::Error::other(e.as_string().unwrap_or_else(|| "can't reach the server".into()))
}

/// A file on the server.
struct Remote(String);

impl RangeSource for Remote {
    fn len(&self) -> io::Result<u64> {
        size(&self.0).map(|n| n as u64).map_err(io_error)
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        read_range(&self.0, start as f64, end as f64).map_err(io_error)
    }
}

fn js(e: core::Error) -> JsError {
    JsError::new(&e.0)
}

/// One file being played.
#[wasm_bindgen]
pub struct Track(core::Track);

#[wasm_bindgen]
impl Track {
    /// Opens `url` (`size` bytes long; `ext` helps guess the format) to play at `out_rate`.
    #[wasm_bindgen(constructor)]
    pub fn new(url: String, size: f64, ext: &str, out_rate: u32) -> Result<Track, JsError> {
        core::Track::open(Arc::new(Remote(url)), size as u64, ext, out_rate).map(Track).map_err(js)
    }

    #[wasm_bindgen(getter)]
    pub fn duration(&self) -> f64 {
        self.0.duration()
    }

    #[wasm_bindgen(getter, js_name = sampleRate)]
    pub fn sample_rate(&self) -> u32 {
        self.0.sample_rate()
    }

    #[wasm_bindgen(setter)]
    pub fn set_gain(&mut self, gain: f32) {
        self.0.gain = gain;
    }

    /// Moves to `secs` in, to the sample.
    pub fn seek(&mut self, secs: f64) -> Result<(), JsError> {
        self.0.seek(secs).map_err(js)
    }

    /// Fills `out` with stereo frames; returns how many. Fewer than fit means it's over.
    pub fn read(&mut self, out: &mut [f32]) -> Result<u32, JsError> {
        self.0.read(out).map(|n| n as u32).map_err(js)
    }
}

/// Something to play, for [`Sequencer`].
#[wasm_bindgen]
pub struct Item(sequence::Item);

#[wasm_bindgen]
impl Item {
    /// `size` is in bytes, or 0 to ask the server; `crossfade` is seconds into the next.
    #[wasm_bindgen(constructor)]
    pub fn new(key: String, url: String, size: f64, ext: String, gain: f32, crossfade: f64) -> Item {
        Item(sequence::Item { key, source: Arc::new(Remote(url)), size: size as u64, ext, gain, crossfade })
    }
}

/// What the sequencer did, as `kind` says: `mark`, `end`, `rewind` or `error`.
#[wasm_bindgen(getter_with_clone)]
pub struct Event {
    pub kind: String,
    pub key: String,
    pub frame: f64,
    pub offset: f64,
    pub duration: f64,
    pub message: String,
}

impl From<CoreEvent> for Event {
    fn from(e: CoreEvent) -> Self {
        let (kind, key, frame, offset, duration, message) = match e {
            CoreEvent::Mark { key, frame, offset, duration } => ("mark", key, frame, offset, duration, String::new()),
            CoreEvent::End { frame } => ("end", String::new(), frame, 0.0, 0.0, String::new()),
            CoreEvent::Rewind { frame } => ("rewind", String::new(), frame, 0.0, 0.0, String::new()),
            CoreEvent::Error { key, message } => ("error", key, 0, 0.0, 0.0, message),
        };

        Event { kind: kind.into(), key, frame: frame as f64, offset, duration, message }
    }
}

/// The queue, decoded one track straight into the next (see decoder/core).
#[wasm_bindgen]
pub struct Sequencer(sequence::Sequencer);

#[wasm_bindgen]
impl Sequencer {
    #[wasm_bindgen(constructor)]
    pub fn new(rate: u32) -> Sequencer {
        Sequencer(sequence::Sequencer::new(rate))
    }

    /// Decodes `items` from `start` seconds into the first, counting frames on from `frame`.
    pub fn play(&mut self, items: Vec<Item>, start: f64, frame: f64) {
        self.0.play(items.into_iter().map(|i| i.0).collect(), start, frame as u64);
    }

    /// Changes what follows `after`; the frame to throw away everything from, or -1 to keep it all.
    pub fn upcoming(&mut self, after: &str, items: Vec<Item>, heard: f64) -> f64 {
        let items = items.into_iter().map(|i| i.0).collect();
        self.0.upcoming(after, items, heard as u64).map_or(-1.0, |f| f as f64)
    }

    pub fn gain(&mut self, key: &str, gain: f32) {
        self.0.gain(key, gain);
    }

    pub fn stop(&mut self) {
        self.0.stop();
    }

    /// Fills `out` with stereo frames; returns how many, fewer only when there's nothing left.
    pub fn read(&mut self, out: &mut [f32]) -> u32 {
        self.0.read(out) as u32
    }

    pub fn events(&mut self) -> Vec<Event> {
        self.0.events().into_iter().map(Event::from).collect()
    }
}
