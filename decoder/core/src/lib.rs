// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{self, Read, Seek, SeekFrom};

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;

pub mod sequence;

pub trait RangeSource: Send + Sync {
    fn len(&self) -> io::Result<u64>;
    /// Inclusive byte range.
    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>>;
}

#[derive(Debug)]
pub struct DecoderError(String);
impl DecoderError {
    pub fn new(message: &str) -> Self {
        Self(message.into())
    }
}
impl std::fmt::Display for DecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DecoderError {}

const BLOCK: u64 = 512 * 1024;
const KEEP: usize = 12;

/// A file on the server, fetched in blocks as it's read.
struct Remote {
    source: Box<dyn RangeSource>,
    len: u64,
    pos: u64,
    blocks: Vec<(u64, Vec<u8>)>,
}

impl Remote {
    fn block(&mut self, index: u64) -> io::Result<&[u8]> {
        if let Some(i) = self.blocks.iter().position(|(b, _)| *b == index) {
            let entry = self.blocks.remove(i);
            self.blocks.push(entry);
        } else {
            let start = index * BLOCK;
            let end = (start + BLOCK).min(self.len) - 1;

            let data = self.source.read_range(start, end)?;
            if data.len() as u64 != end - start + 1 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete range response"));
            }

            if self.blocks.len() >= KEEP {
                self.blocks.remove(0);
            }

            self.blocks.push((index, data));
        }
        Ok(&self.blocks.last().unwrap().1)
    }
}

impl Read for Remote {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }

        let pos = self.pos;
        let block = self.block(pos / BLOCK)?;
        let offset = (pos % BLOCK) as usize;

        if offset >= block.len() {
            return Ok(0);
        }

        let n = buf.len().min(block.len() - offset);
        buf[..n].copy_from_slice(&block[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Remote {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let pos = match to {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(d) => self.len as i64 + d,
            SeekFrom::Current(d) => self.pos as i64 + d,
        };

        if pos < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"));
        }

        self.pos = pos as u64;
        Ok(self.pos)
    }
}

impl MediaSource for Remote {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

const CHUNK: usize = 1024;

/// A fixed-ratio resampler fed whatever comes, which keeps what doesn't fill a chunk yet.
struct Rate {
    fft: Fft<f32>,
    pending: Vec<f32>,
    out: Vec<f32>,
    skip: usize,
    taken: u64,
    given: u64,
    ratio: f64,
}

impl Rate {
    fn new(from: u32, to: u32) -> Result<Self, DecoderError> {
        let fft = Fft::<f32>::new(from as usize, to as usize, CHUNK, 2, FixedSync::Input)
            .map_err(|e| DecoderError::new(&format!("resampler: {e}")))?;

        let out = vec![0.0; fft.output_frames_max() * 2];
        let skip = fft.output_delay();
        Ok(Self { fft, pending: Vec::new(), out, skip, taken: 0, given: 0, ratio: to as f64 / from as f64 })
    }

    fn run(&mut self, frames: usize, partial: Option<usize>, into: &mut Vec<f32>) {
        let input = InterleavedSlice::new(&self.pending[..], 2, frames).expect("input fits");
        let cap = self.out.len() / 2;
        let mut output = InterleavedSlice::new_mut(&mut self.out[..], 2, cap).expect("output fits");
        let mut indexing = Indexing::new();
        indexing.partial_len = partial;
        let Ok((_, written)) = self.fft.process_into_buffer(&input, &mut output, Some(&indexing)) else { return };
        let drop = self.skip.min(written);
        self.skip -= drop;
        into.extend_from_slice(&self.out[drop * 2..written * 2]);
        self.given += (written - drop) as u64;
    }

    fn push(&mut self, samples: &[f32], into: &mut Vec<f32>) {
        self.pending.extend_from_slice(samples);
        self.taken += (samples.len() / 2) as u64;

        while self.pending.len() >= CHUNK * 2 {
            self.run(CHUNK, None, into);
            self.pending.drain(..CHUNK * 2);
        }
    }

    /// Whatever's left, and the tail the resampler still holds, cut to exactly as long as it should
    /// be.
    fn finish(&mut self, into: &mut Vec<f32>) {
        let expected = (self.taken as f64 * self.ratio).round() as u64;
        let left = self.pending.len() / 2;
        self.pending.resize(CHUNK * 2, 0.0);
        self.run(CHUNK, Some(left), into);
        self.pending.iter_mut().for_each(|s| *s = 0.0);
        let mut guard = 0;

        while self.given < expected && guard < 64 {
            self.run(CHUNK, Some(0), into);
            guard += 1;
        }

        if self.given > expected {
            let extra = (self.given - expected) as usize;
            into.truncate(into.len().saturating_sub(extra * 2));
            self.given = expected;
        }

        self.pending.clear();
    }
}

/// One file being played.
pub struct Track {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    id: u32,
    rate: u32,
    out_rate: u32,
    duration: f64,
    resampler: Option<Rate>,
    ready: Vec<f32>,
    taken: usize,
    scratch: Vec<f32>,
    skip: u64,
    gain: f32,
    done: bool,
}

fn stereo(src: &[f32], channels: usize, into: &mut Vec<f32>) {
    match channels {
        1 => src.iter().for_each(|&s| into.extend_from_slice(&[s, s])),
        2 => into.extend_from_slice(src),
        n => {
            const K: f32 = std::f32::consts::FRAC_1_SQRT_2;

            for f in src.chunks_exact(n) {
                let c = f.get(2).copied().unwrap_or(0.0);
                let (sl, sr) = (f.get(4).copied().unwrap_or(0.0), f.get(5).copied().unwrap_or(0.0));
                into.push((f[0] + K * c + K * sl) * K);
                into.push((f[1] + K * c + K * sr) * K);
            }
        },
    }
}
impl Track {
    /// Opens `url` (`size` bytes long; `ext` helps guess the format) to play at `out_rate`.
    pub fn new(source: Box<dyn RangeSource>, ext: &str, out_rate: u32) -> Result<Track, DecoderError> {
        if out_rate == 0 {
            return Err(DecoderError::new("invalid output rate"));
        }
        let len = source.len().map_err(|e| DecoderError::new(&e.to_string()))?;
        let remote = Remote { source, len, pos: 0, blocks: Vec::new() };
        let mss = MediaSourceStream::new(Box::new(remote), Default::default());
        let mut hint = Hint::new();
        hint.with_extension(ext);

        let format = symphonia::default::get_probe()
            .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
            .map_err(|e| DecoderError::new(&format!("unsupported file: {e}")))?;

        let track = format.default_track(TrackType::Audio).ok_or_else(|| DecoderError::new("no audio in this file"))?;

        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or_else(|| DecoderError::new("no audio in this file"))?;

        #[cfg(feature = "opus")]
        let codecs = {
            let mut registry = symphonia::core::codecs::registry::CodecRegistry::new();
            symphonia::default::register_enabled_codecs(&mut registry);
            registry.register_audio_decoder::<symphonia_adapter_libopus::OpusDecoder>();
            registry
        };
        #[cfg(not(feature = "opus"))]
        let codecs = symphonia::default::get_codecs();
        let decoder = codecs
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| DecoderError::new(&format!("unsupported codec: {e}")))?;

        let rate = params.sample_rate.ok_or_else(|| DecoderError::new("unknown sample rate"))?;

        let duration = match (track.num_frames, track.time_base) {
            (Some(n), Some(tb)) => n as f64 * tb.numer.get() as f64 / tb.denom.get() as f64,
            (Some(n), None) => n as f64 / rate as f64,
            _ => 0.0,
        };

        let id = track.id;
        let resampler = if rate == out_rate { None } else { Some(Rate::new(rate, out_rate)?) };

        Ok(Track {
            format,
            decoder,
            id,
            rate,
            out_rate,
            duration,
            resampler,
            ready: Vec::new(),
            taken: 0,
            scratch: Vec::new(),
            skip: 0,
            gain: 1.0,
            done: false,
        })
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    /// Moves to `secs` in, to the sample.
    pub fn seek(&mut self, secs: f64) -> Result<(), DecoderError> {
        let time = Time::try_from_secs_f64(secs.max(0.0)).ok_or_else(|| DecoderError::new("bad position"))?;

        let to = self
            .format
            .seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(self.id) })
            .map_err(|e| DecoderError::new(&format!("can't seek: {e}")))?;

        self.decoder.reset();
        let behind = (to.required_ts.get() - to.actual_ts.get()).max(0) as u64;
        let tb = self.format.tracks().iter().find(|t| t.id == self.id).and_then(|t| t.time_base);

        self.skip = match tb {
            Some(tb) => {
                (behind as f64 * tb.numer.get() as f64 / tb.denom.get() as f64 * self.rate as f64).round() as u64
            },
            None => behind,
        };

        if self.resampler.is_some() {
            self.resampler = Some(Rate::new(self.rate, self.out_rate)?);
        }

        self.ready.clear();
        self.taken = 0;
        self.done = false;
        Ok(())
    }

    fn decode_more(&mut self) -> Result<(), DecoderError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return self.finish(),
                Err(Error::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => return self.finish(),
                Err(Error::ResetRequired) => return self.finish(),
                Err(e) => return Err(DecoderError::new(&format!("can't read the file: {e}"))),
            };

            if packet.track_id != self.id {
                continue;
            }

            let buf = match self.decoder.decode(&packet) {
                Ok(b) => b,
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(DecoderError::new(&format!("can't decode: {e}"))),
            };

            let channels = buf.spec().channels().count().max(1);
            self.scratch.clear();
            buf.copy_to_vec_interleaved::<f32>(&mut self.scratch);
            let mut frames = &self.scratch[..];

            if self.skip > 0 {
                let n = (self.skip as usize).min(frames.len() / channels);
                frames = &frames[n * channels..];
                self.skip -= n as u64;
            }

            let mut two = Vec::with_capacity(frames.len() / channels * 2);
            stereo(frames, channels, &mut two);

            match &mut self.resampler {
                Some(r) => r.push(&two, &mut self.ready),
                None => self.ready.extend_from_slice(&two),
            }

            if self.ready.len() > self.taken {
                return Ok(());
            }
        }
    }

    fn finish(&mut self) -> Result<(), DecoderError> {
        if let Some(r) = &mut self.resampler {
            r.finish(&mut self.ready);
        }

        self.done = true;
        Ok(())
    }

    /// Fills `out` with stereo frames; returns how many. Fewer than fit means it's over.
    pub fn read(&mut self, out: &mut [f32]) -> Result<u32, DecoderError> {
        let want = out.len() / 2;
        let mut written = 0;

        while written < want {
            if self.taken >= self.ready.len() {
                if self.done {
                    break;
                }

                if self.taken > 0 {
                    self.ready.drain(..self.taken);
                    self.taken = 0;
                }

                self.decode_more()?;
                continue;
            }

            let available = (self.ready.len() - self.taken) / 2;
            let n = available.min(want - written);
            let src = &self.ready[self.taken..self.taken + n * 2];

            for (o, s) in out[written * 2..(written + n) * 2].iter_mut().zip(src) {
                *o = s * self.gain;
            }

            self.taken += n * 2;
            written += n;
        }

        Ok(written as u32)
    }
}
