// SPDX-License-Identifier: AGPL-3.0-or-later
use std::io;
use std::sync::{Arc, Mutex};

use tinystream_decoder_core::RangeSource as CoreSource;
use tinystream_decoder_core::sequence::{Event as CoreEvent, Item as CoreItem, Sequencer as CoreSequencer};
uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AudioError {
    #[error("{reason}")]
    Read { reason: String },
}
#[uniffi::export(with_foreign)]
pub trait RangeSource: Send + Sync {
    fn len(&self) -> Result<u64, AudioError>;
    fn read_range(&self, start: u64, end: u64) -> Result<Vec<u8>, AudioError>;
}
struct Source(Arc<dyn RangeSource>);
impl CoreSource for Source {
    fn len(&self) -> io::Result<u64> {
        self.0.len().map_err(io::Error::other)
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        self.0.read_range(start, end).map_err(io::Error::other)
    }
}
#[derive(uniffi::Record)]
pub struct Item {
    pub key: String,
    pub source: Arc<dyn RangeSource>,
    pub ext: String,
    pub gain: f32,
    pub crossfade: f64,
}
#[derive(uniffi::Enum)]
pub enum Event {
    Mark { key: String, frame: u64, offset: f64, duration: f64 },
    End { frame: u64 },
    Rewind { frame: u64 },
    Error { key: String, message: String },
}
#[derive(uniffi::Record)]
pub struct Output {
    pub samples: Vec<f32>,
    pub events: Vec<Event>,
}
fn items(inputs: Vec<Item>) -> Vec<CoreItem> {
    inputs
        .into_iter()
        .map(|i| CoreItem {
            key: i.key,
            source: Arc::new(Source(i.source)),
            ext: i.ext,
            gain: i.gain,
            crossfade: i.crossfade,
        })
        .collect()
}
#[derive(uniffi::Object)]
pub struct Sequencer(Mutex<CoreSequencer>);
#[uniffi::export]
impl Sequencer {
    #[uniffi::constructor]
    pub fn new(rate: u32) -> Self {
        Self(Mutex::new(CoreSequencer::new(rate)))
    }

    pub fn play(&self, inputs: Vec<Item>, start: f64) {
        self.0.lock().unwrap().play(items(inputs), start, 0);
    }

    pub fn upcoming(&self, after: String, inputs: Vec<Item>, heard: u64) -> i64 {
        self.0.lock().unwrap().upcoming(&after, items(inputs), heard).map(|n| n as i64).unwrap_or(-1)
    }

    pub fn gain(&self, key: String, gain: f32) {
        self.0.lock().unwrap().gain(&key, gain);
    }

    pub fn stop(&self) {
        self.0.lock().unwrap().stop();
    }

    pub fn read(&self, frames: u32) -> Output {
        let mut core = self.0.lock().unwrap();
        let mut samples = vec![0.0; frames.min(16384) as usize * 2];
        let n = core.read(&mut samples);
        samples.truncate(n * 2);
        let events = core
            .events()
            .into_iter()
            .map(|e| match e {
                CoreEvent::Mark { key, frame, offset, duration } => Event::Mark { key, frame, offset, duration },
                CoreEvent::Rewind { frame } => Event::Rewind { frame },
                CoreEvent::End { frame } => Event::End { frame },
                CoreEvent::Error { key, message } => Event::Error { key, message },
            })
            .collect();
        Output { samples, events }
    }
}
