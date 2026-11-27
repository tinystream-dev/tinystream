// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::ring::{Ring, SECONDS};

/// What the stream's callback and the decoder share.
struct Shared {
    ring: Ring,
    volume: AtomicU32,
    playing: AtomicBool,
    starved: AtomicBool,
    /// Frames between what's been taken from the ring and what's coming out of the speakers.
    delay: AtomicU64,
    failed: AtomicBool,
}

pub struct Output {
    _stream: cpal::Stream,
    pub rate: u32,
    pub device: String,
    shared: Arc<Shared>,
}
impl Output {
    pub fn open() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device().context("No audio output device is available")?;
        let name = device.description()?.to_string();
        let supported = device.default_output_config()?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let shared = Arc::new(Shared {
            ring: Ring::new(config.sample_rate as usize * SECONDS),
            volume: AtomicU32::new(1.0_f32.to_bits()),
            playing: AtomicBool::new(false),
            starved: AtomicBool::new(false),
            delay: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });
        macro_rules! stream {
            ($type:ty) => {
                build::<$type>(&device, &config, shared.clone())?
            };
        }
        let stream = match format {
            cpal::SampleFormat::F32 => stream!(f32),
            cpal::SampleFormat::F64 => stream!(f64),
            cpal::SampleFormat::I8 => stream!(i8),
            cpal::SampleFormat::I16 => stream!(i16),
            cpal::SampleFormat::I24 => stream!(cpal::I24),
            cpal::SampleFormat::I32 => stream!(i32),
            cpal::SampleFormat::I64 => stream!(i64),
            cpal::SampleFormat::U8 => stream!(u8),
            cpal::SampleFormat::U16 => stream!(u16),
            cpal::SampleFormat::U32 => stream!(u32),
            cpal::SampleFormat::U64 => stream!(u64),
            other => anyhow::bail!("Unsupported audio output format: {other}"),
        };
        stream.play()?;
        Ok(Self { _stream: stream, rate: config.sample_rate, device: name, shared })
    }

    pub fn ring(&self) -> &Ring {
        &self.shared.ring
    }

    /// The frame coming out of the speakers now.
    pub fn heard(&self) -> u64 {
        self.shared.ring.heard().saturating_sub(self.shared.delay.load(Ordering::Acquire))
    }

    pub fn playing(&self, value: bool) {
        self.shared.playing.store(value, Ordering::Release);
    }

    pub fn volume(&self, value: f32) {
        self.shared.volume.store(value.clamp(0.0, 1.0).powi(2).to_bits(), Ordering::Release);
    }

    pub fn starved(&self) -> bool {
        self.shared.starved.load(Ordering::Acquire)
    }

    pub fn failed(&self) -> bool {
        self.shared.failed.load(Ordering::Acquire)
    }

    pub fn device_changed(&self) -> bool {
        cpal::default_host()
            .default_output_device()
            .and_then(|d| d.description().ok())
            .is_none_or(|d| d.to_string() != self.device)
    }
}

fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<Shared>,
) -> Result<cpal::Stream> {
    let channels = config.channels as usize;
    anyhow::ensure!(channels > 0, "Audio device has no output channels");
    let rate = config.sample_rate as f32;
    // How fast the level follows, about 8 ms to settle, so pausing never clicks.
    let smooth = 1.0 / (0.008 * rate);
    let mut level = 0.0_f32;
    let mut seen = 0;
    // Frames of music, not silence, still on their way to the speakers: it drains as they play.
    let mut flight = 0_u64;
    let failed = shared.clone();
    Ok(device.build_output_stream(
        config,
        move |samples: &mut [T], info| {
            let on = shared.playing.load(Ordering::Acquire);
            let target = if on { f32::from_bits(shared.volume.load(Ordering::Acquire)) } else { 0.0 };
            let mut starved = false;
            let before = shared.ring.heard();
            for frame in samples.chunks_mut(channels) {
                let stereo = if !on && level < 1e-4 {
                    level = 0.0;
                    [0.0; 2]
                } else {
                    level += (target - level) * smooth;
                    match shared.ring.frame(&mut seen) {
                        Some([l, r]) => [l * level, r * level],
                        None => {
                            starved |= on;
                            [0.0; 2]
                        },
                    }
                };
                for (channel, sample) in frame.iter_mut().enumerate() {
                    let value = if channels == 1 {
                        (stereo[0] + stereo[1]) * 0.5
                    } else {
                        match channel {
                            0 => stereo[0],
                            1 => stereo[1],
                            _ => 0.0,
                        }
                    };
                    *sample = T::from_sample(value.clamp(-1.0, 1.0));
                }
            }
            let timestamp = info.timestamp();
            let ahead = timestamp.playback.duration_since(&timestamp.callback).map_or(0.0, |d| d.as_secs_f32());
            let frames = (samples.len() / channels) as u64;
            let taken = shared.ring.heard().saturating_sub(before);
            flight = (flight.saturating_sub(frames) + taken).min((ahead * rate) as u64 + frames);
            shared.delay.store(flight, Ordering::Release);
            shared.starved.store(starved, Ordering::Release);
        },
        move |error| {
            tracing::warn!(%error, "music output failed");
            failed.failed.store(true, Ordering::Release);
        },
        None,
    )?)
}
