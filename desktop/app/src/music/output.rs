// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::ring::Ring;

pub struct Output {
    _stream: cpal::Stream,
    pub rate: u32,
    pub device: String,
    pub ring: Arc<Ring>,
    volume: Arc<AtomicU32>,
    playing: Arc<AtomicBool>,
    pub failed: Arc<AtomicBool>,
}
impl Output {
    pub fn open() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device().context("No audio output device is available")?;
        let name = device.description()?.to_string();
        let supported = device.default_output_config()?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let ring = Arc::new(Ring::new(config.sample_rate as usize / 2));
        let volume = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let playing = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        macro_rules! stream {
            ($type:ty) => {
                build::<$type>(&device, &config, ring.clone(), volume.clone(), playing.clone(), failed.clone())?
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
        Ok(Self { _stream: stream, rate: config.sample_rate, device: name, ring, volume, playing, failed })
    }

    pub fn playing(&self, value: bool) {
        self.playing.store(value, Ordering::Release);
    }

    pub fn volume(&self, value: f32) {
        self.volume.store(value.clamp(0.0, 1.0).powi(2).to_bits(), Ordering::Release);
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
    ring: Arc<Ring>,
    volume: Arc<AtomicU32>,
    playing: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let channels = config.channels as usize;
    anyhow::ensure!(channels > 0, "Audio device has no output channels");
    Ok(device.build_output_stream(
        config,
        move |samples: &mut [T], _| {
            let on = playing.load(Ordering::Acquire);
            let volume = f32::from_bits(volume.load(Ordering::Acquire));
            for frame in samples.chunks_mut(channels) {
                let stereo = if on { ring.frame().unwrap_or([0.0; 2]) } else { [0.0; 2] };
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
                    *sample = T::from_sample((value * volume).clamp(-1.0, 1.0));
                }
            }
        },
        move |error| {
            tracing::warn!(%error, "music output failed");
            failed.store(true, Ordering::Release);
        },
        None,
    )?)
}
