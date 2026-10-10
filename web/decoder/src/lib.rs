// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io;
use std::sync::Arc;

use tinystream_decoder_core::RangeSource;
use tinystream_decoder_core::sequence::{Event, Item};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = tsReadRange, catch)]
    fn read_range(url: &str, start: f64, end: f64) -> Result<Vec<u8>, JsValue>;
    #[wasm_bindgen(js_name = tsSizeOf, catch)]
    fn size_of(url: &str) -> Result<f64, JsValue>;
}
struct Remote {
    url: String,
    size: u64,
}
impl RangeSource for Remote {
    fn len(&self) -> io::Result<u64> {
        if self.size > 0 {
            Ok(self.size)
        } else {
            size_of(&self.url).map(|n| n as u64).map_err(|e| io::Error::other(e.as_string().unwrap_or_default()))
        }
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        read_range(&self.url, start as f64, end as f64).map_err(|e| io::Error::other(e.as_string().unwrap_or_default()))
    }
}
#[wasm_bindgen]
pub struct Track(tinystream_decoder_core::Track);
#[wasm_bindgen]
impl Track {
    #[wasm_bindgen(constructor)]
    pub fn new(url: String, size: f64, ext: &str, rate: u32) -> Result<Track, JsError> {
        tinystream_decoder_core::Track::new(Box::new(Remote { url, size: size as u64 }), ext, rate)
            .map(Self)
            .map_err(|e| JsError::new(&e.to_string()))
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
        self.0.set_gain(gain);
    }

    pub fn seek(&mut self, secs: f64) -> Result<(), JsError> {
        self.0.seek(secs).map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn read(&mut self, out: &mut [f32]) -> Result<u32, JsError> {
        self.0.read(out).map_err(|e| JsError::new(&e.to_string()))
    }
}
#[derive(serde::Deserialize)]
struct Input {
    key: String,
    url: String,
    size: f64,
    ext: String,
    gain: f32,
    crossfade: f64,
}
fn items(json: &str) -> Result<Vec<Item>, JsError> {
    let inputs: Vec<Input> = serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(inputs
        .into_iter()
        .map(|i| Item {
            key: i.key,
            source: Arc::new(Remote { url: i.url, size: i.size as u64 }),
            ext: i.ext,
            gain: i.gain,
            crossfade: i.crossfade,
        })
        .collect())
}
#[wasm_bindgen]
pub struct Sequencer(tinystream_decoder_core::sequence::Sequencer);
#[wasm_bindgen]
impl Sequencer {
    #[wasm_bindgen(constructor)]
    pub fn new(rate: u32) -> Self {
        Self(tinystream_decoder_core::sequence::Sequencer::new(rate))
    }

    pub fn play(&mut self, json: &str, start: f64, frame: f64) -> Result<(), JsError> {
        self.0.play(items(json)?, start, frame as u64);
        Ok(())
    }

    pub fn upcoming(&mut self, after: &str, json: &str, heard: f64) -> Result<f64, JsError> {
        Ok(self.0.upcoming(after, items(json)?, heard as u64).map(|n| n as f64).unwrap_or(-1.0))
    }

    pub fn gain(&mut self, key: &str, gain: f32) {
        self.0.gain(key, gain);
    }

    pub fn stop(&mut self) {
        self.0.stop();
    }

    pub fn read(&mut self, out: &mut [f32]) -> u32 {
        self.0.read(out) as u32
    }

    pub fn events(&mut self) -> String {
        let events: Vec<_> = self
            .0
            .events()
            .into_iter()
            .map(|e| match e {
                Event::Mark { key, frame, offset, duration } => {
                    serde_json::json!({"type":"mark", "key":key,"frame":frame,"offset":offset,"duration":duration})
                },
                Event::Rewind { frame } => serde_json::json!({"type":"rewind","frame":frame}),
                Event::End { frame } => serde_json::json!({"type":"end","frame":frame}),
                Event::Error { key, message } => serde_json::json!({"type":"error","key":key,"message":message}),
            })
            .collect();
        serde_json::to_string(&events).unwrap()
    }
}
