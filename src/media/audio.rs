// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::{c_int, c_void};
use std::path::{Path, PathBuf};
use std::{io, ptr};

use bytes::Bytes;
use tokio::sync::mpsc;

use super::Hold;
use super::ff::{self, Codec, Dict, Frame, Input, MemOutput, Packet, ffi};

/// A file's audio, decoded and converted to one sample format, rate and
/// channel count.
pub struct Decoder {
    input: Input,
    index: usize,
    dec: Codec,
    tb: ffi::AVRational,
    swr: *mut ffi::SwrContext,
    pkt: Packet,
    frame: Frame,
    held: bool,
    start: f64,
    format: ffi::AVSampleFormat,
    rate: c_int,
    layout: ffi::AVChannelLayout,
    state: Stage,
}

unsafe impl Send for Decoder {}

#[derive(PartialEq)]
enum Stage {
    Reading,
    Draining,
    Flushed,
    Done,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            ffi::swr_free(&mut self.swr);
            ffi::av_channel_layout_uninit(&mut self.layout);
        }
    }
}

impl Decoder {
    pub fn open(path: &Path, start: f64) -> anyhow::Result<Self> {
        let input = Input::open(path, true)?;
        let index = unsafe { ffi::av_find_best_stream(input.0, ffi::AVMEDIA_TYPE_AUDIO, -1, -1, ptr::null_mut(), 0) };

        if index < 0 {
            anyhow::bail!("no audio in {}", path.display());
        }

        let index = index as usize;

        for (i, &st) in input.streams().iter().enumerate() {
            if i != index {
                unsafe { (*st).discard = ffi::AVDISCARD_ALL };
            }
        }

        let st = input.stream(index).unwrap();
        let tb = st.time_base;
        let decoder = unsafe { ffi::avcodec_find_decoder((*st.codecpar).codec_id) };

        if decoder.is_null() {
            anyhow::bail!("no decoder for the audio in {}", path.display());
        }

        let mut dec = Codec::alloc(decoder)?;

        unsafe {
            ff::check(ffi::avcodec_parameters_to_context(dec.0, st.codecpar), "decoder params")?;
            dec.get_mut().pkt_timebase = tb;
        }

        dec.open(&mut Dict::new(&[]))?;

        if start > 0.05 {
            let ts = (start * ffi::AV_TIME_BASE as f64) as i64;
            let r = unsafe { ffi::av_seek_frame(input.0, -1, ts, ffi::AVSEEK_FLAG_BACKWARD as c_int) };

            if r < 0 {
                tracing::debug!("can't seek in {}: {}", path.display(), ff::err_str(r));
            }
        }

        let (format, rate, channels) = {
            let c = dec.get();
            (c.sample_fmt, c.sample_rate, c.ch_layout.nb_channels.max(1))
        };

        let mut layout: ffi::AVChannelLayout = unsafe { std::mem::zeroed() };
        unsafe { ffi::av_channel_layout_default(&mut layout, channels) };

        Ok(Self {
            input,
            index,
            dec,
            tb,
            swr: ptr::null_mut(),
            pkt: Packet::new(),
            frame: Frame::new(),
            held: false,
            start,
            format,
            rate,
            layout,
            state: Stage::Reading,
        })
    }

    pub fn rate(&self) -> c_int {
        self.dec.get().sample_rate
    }

    pub fn channels(&self) -> c_int {
        self.dec.get().ch_layout.nb_channels.max(1)
    }

    /// Bits per sample in the source, for lossless output that keeps all of them.
    pub fn bits(&self) -> c_int {
        let c = self.dec.get();

        if c.bits_per_raw_sample > 0 {
            c.bits_per_raw_sample
        } else {
            unsafe { ffi::av_get_bytes_per_sample(c.sample_fmt) * 8 }
        }
    }

    pub fn output(&mut self, format: ffi::AVSampleFormat, rate: c_int, channels: c_int) {
        self.format = format;
        self.rate = rate;

        unsafe {
            ffi::av_channel_layout_uninit(&mut self.layout);
            ffi::av_channel_layout_default(&mut self.layout, channels);
        }
    }

    fn convert(&mut self, input: *const ffi::AVFrame) -> anyhow::Result<Frame> {
        unsafe {
            if self.swr.is_null() {
                let f = self.frame.get_mut();

                if f.ch_layout.order == ffi::AV_CHANNEL_ORDER_UNSPEC {
                    let n = f.ch_layout.nb_channels;
                    ffi::av_channel_layout_uninit(&mut f.ch_layout);
                    ffi::av_channel_layout_default(&mut f.ch_layout, n);
                }

                ff::check(
                    ffi::swr_alloc_set_opts2(
                        &mut self.swr,
                        &self.layout,
                        self.format,
                        self.rate,
                        &f.ch_layout,
                        f.format,
                        f.sample_rate,
                        0,
                        ptr::null_mut(),
                    ),
                    "resampler",
                )?;

                let dither = ff::cstr("dither_method");
                let triangular = ff::cstr("triangular_hp");
                ffi::av_opt_set(self.swr as *mut c_void, dither.as_ptr(), triangular.as_ptr(), 0);
                let filter = ff::cstr("filter_size");
                ffi::av_opt_set_int(self.swr as *mut c_void, filter.as_ptr(), 64, 0);
                ff::check(ffi::swr_init(self.swr), "resampler")?;
            }

            let mut out = Frame::new();
            let o = out.get_mut();
            ffi::av_channel_layout_copy(&mut o.ch_layout, &self.layout);
            o.sample_rate = self.rate;
            o.format = self.format;
            ff::check(ffi::swr_convert_frame(self.swr, out.0, input), "resample")?;
            Ok(out)
        }
    }

    /// The next stretch of converted audio, or `None` at the end.
    pub fn next(&mut self) -> anyhow::Result<Option<Frame>> {
        loop {
            if self.state == Stage::Done {
                return Ok(None);
            }
            if self.dec.receive_frame(&mut self.frame)? {
                let f = self.frame.get();
                let ts = if f.best_effort_timestamp != ffi::AV_NOPTS_VALUE { f.best_effort_timestamp } else { f.pts };
                let dur = f.nb_samples as f64 / f.sample_rate.max(1) as f64;

                if ts != ffi::AV_NOPTS_VALUE && (ts as f64 * ff::q2d(self.tb)) + dur < self.start {
                    self.frame.unref();
                    continue;
                }

                let out = self.convert(self.frame.0);
                self.frame.unref();
                let out = out?;

                if out.get().nb_samples > 0 {
                    return Ok(Some(out));
                }

                continue;
            }
            match self.state {
                Stage::Reading => {
                    if !self.held {
                        if !self.input.read(&mut self.pkt)? {
                            self.dec.send_packet(None)?;
                            self.state = Stage::Draining;
                            continue;
                        }

                        if self.pkt.get().stream_index as usize != self.index {
                            self.pkt.unref();
                            continue;
                        }

                        self.held = true;
                    }

                    if self.dec.send_packet(Some(&self.pkt))? {
                        self.pkt.unref();
                        self.held = false;
                    }
                },
                Stage::Draining => {
                    self.state = Stage::Flushed;

                    if !self.swr.is_null() {
                        let out = self.convert(ptr::null())?;

                        if out.get().nb_samples > 0 {
                            return Ok(Some(out));
                        }
                    }
                },
                Stage::Flushed | Stage::Done => self.state = Stage::Done,
            }
        }
    }
}

/// How loud a file is: EBU R128 integrated loudness (LUFS) and its sample
/// peak. `wait` is called between stretches, so the caller can hold it up.
pub fn measure(path: &Path, wait: &dyn Fn()) -> anyhow::Result<(Option<f64>, f64)> {
    let mut dec = Decoder::open(path, 0.0)?;
    let channels = dec.channels().min(8);
    let rate = dec.rate();
    dec.output(ffi::AV_SAMPLE_FMT_FLT, rate, channels);
    let mut meter = ebur128::EbuR128::new(channels as u32, rate as u32, ebur128::Mode::I | ebur128::Mode::SAMPLE_PEAK)?;
    let mut n = 0u32;

    while let Some(frame) = dec.next()? {
        let f = frame.get();

        let samples =
            unsafe { std::slice::from_raw_parts(f.data[0] as *const f32, (f.nb_samples * channels) as usize) };

        meter.add_frames_f32(samples)?;
        n += 1;

        if n.is_multiple_of(64) {
            wait();
        }
    }

    let loudness = meter.loudness_global()?;
    let peak = (0..channels as u32).filter_map(|c| meter.sample_peak(c).ok()).fold(0.0, f64::max);
    Ok((loudness.is_finite().then_some(loudness), peak))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Flac,
    Opus,
    Mp3,
    Aac,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "flac" => Some(Format::Flac),
            "opus" | "ogg" | "oga" => Some(Format::Opus),
            "mp3" => Some(Format::Mp3),
            "aac" | "m4a" => Some(Format::Aac),
            _ => None,
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Format::Flac => "audio/flac",
            Format::Opus => "audio/ogg",
            Format::Mp3 => "audio/mpeg",
            Format::Aac => "audio/aac",
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Format::Flac => "flac",
            Format::Opus => "opus",
            Format::Mp3 => "mp3",
            Format::Aac => "aac",
        }
    }

    pub fn lossless(self) -> bool {
        self == Format::Flac
    }

    /// kbit/s when nobody says.
    pub fn default_bitrate(self) -> u32 {
        match self {
            Format::Flac => 0,
            Format::Opus => 160,
            Format::Mp3 => 320,
            Format::Aac => 256,
        }
    }

    fn muxer(self) -> &'static str {
        match self {
            Format::Flac => "flac",
            Format::Opus => "ogg",
            Format::Mp3 => "mp3",
            Format::Aac => "adts",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Transcode {
    pub path: PathBuf,
    pub start: f64,
    pub format: Format,

    /// kbit/s; ignored for lossless.
    pub bitrate: Option<u32>,
}

pub type Chunk = Result<Bytes, io::Error>;

enum Stop {
    Closed,
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for Stop {
    fn from(e: anyhow::Error) -> Self {
        Stop::Failed(e)
    }
}

pub fn spawn(req: Transcode, hold: Hold) -> mpsc::Receiver<Chunk> {
    let (tx, rx) = mpsc::channel(8);

    std::thread::Builder::new()
        .name("tinystream-audio".into())
        .spawn(move || {
            let _hold = hold;

            match run(&req, &tx) {
                Ok(()) | Err(Stop::Closed) => {},
                Err(Stop::Failed(e)) => {
                    tracing::error!("converting {} failed: {e:#}", req.path.display());
                    let _ = tx.blocking_send(Err(io::Error::other(format!("{e:#}"))));
                },
            }
        })
        .expect("can't spawn a thread");

    rx
}

fn send(tx: &mpsc::Sender<Chunk>, bytes: Vec<u8>) -> Result<(), Stop> {
    if bytes.is_empty() {
        return Ok(());
    }
    tx.blocking_send(Ok(Bytes::from(bytes))).map_err(|_| Stop::Closed)
}

const LOSSY_RATES: &[c_int] = &[48_000, 44_100, 32_000, 24_000, 22_050, 16_000];

fn lossy_rate(src: c_int) -> c_int {
    if LOSSY_RATES.contains(&src) {
        src
    } else if src % 44_100 == 0 || src % 11_025 == 0 {
        44_100
    } else {
        48_000
    }
}

fn run(req: &Transcode, tx: &mpsc::Sender<Chunk>) -> Result<(), Stop> {
    let mut dec = Decoder::open(&req.path, req.start)?;
    let (src_rate, src_channels, src_bits) = (dec.rate(), dec.channels(), dec.bits());

    let encoder = unsafe {
        match req.format {
            Format::Flac => ffi::avcodec_find_encoder(ffi::AV_CODEC_ID_FLAC),
            Format::Opus => ffi::avcodec_find_encoder_by_name(c"libopus".as_ptr()),
            Format::Mp3 => ffi::avcodec_find_encoder_by_name(c"libmp3lame".as_ptr()),
            Format::Aac => ffi::avcodec_find_encoder(ffi::AV_CODEC_ID_AAC),
        }
    };

    if encoder.is_null() {
        return Err(Stop::Failed(anyhow::anyhow!("this build can't make {:?}", req.format)));
    }

    let (format, rate, channels) = match req.format {
        Format::Flac => {
            (if src_bits > 16 { ffi::AV_SAMPLE_FMT_S32 } else { ffi::AV_SAMPLE_FMT_S16 }, src_rate, src_channels.min(8))
        },
        Format::Opus => (ffi::AV_SAMPLE_FMT_FLT, 48_000, src_channels.min(2)),
        Format::Mp3 => (ffi::AV_SAMPLE_FMT_FLTP, lossy_rate(src_rate), src_channels.min(2)),
        Format::Aac => (ffi::AV_SAMPLE_FMT_FLTP, lossy_rate(src_rate), src_channels.min(2)),
    };

    dec.output(format, rate, channels);

    let mut out = MemOutput::new(req.format.muxer())?;
    let mut enc = Codec::alloc(encoder)?;

    unsafe {
        let e = enc.get_mut();
        ffi::av_channel_layout_default(&mut e.ch_layout, channels);
        e.sample_rate = rate;
        e.sample_fmt = format;
        e.time_base = ff::q(1, rate);

        if req.format == Format::Flac {
            e.bits_per_raw_sample = if src_bits > 16 { src_bits.min(24) } else { 16 };
            e.compression_level = 5;
        } else {
            let kbps = req.bitrate.filter(|&b| b > 0).unwrap_or(req.format.default_bitrate()).clamp(32, 320);
            e.bit_rate = kbps as i64 * 1000;
        }

        if (*(*out.ctx).oformat).flags & ffi::AVFMT_GLOBALHEADER as c_int != 0 {
            e.flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;
        }
    }

    enc.open(&mut Dict::new(&[]))?;
    let st = out.add_stream();

    unsafe {
        ff::check(ffi::avcodec_parameters_from_context((*st).codecpar, enc.0), "encoder params")?;
        (*st).time_base = enc.get().time_base;
    }

    let mut opts = match req.format {
        Format::Mp3 => Dict::new(&[("write_xing", "0"), ("id3v2_version", "0")]),
        _ => Dict::new(&[]),
    };

    out.write_header(&mut opts)?;
    send(tx, out.take())?;

    let caps = unsafe { (*encoder).capabilities };
    let variable = caps & ffi::AV_CODEC_CAP_VARIABLE_FRAME_SIZE as c_int != 0;
    let small_last = caps & ffi::AV_CODEC_CAP_SMALL_LAST_FRAME as c_int != 0 || variable;

    let frame_size = match enc.get().frame_size {
        n if n > 0 && !variable => n,
        _ => 4096,
    };

    let fifo = Fifo(unsafe { ffi::av_audio_fifo_alloc(format, channels, frame_size * 4) });
    let mut pts = 0i64;
    let mut pkt = Packet::new();
    let tb = enc.get().time_base;

    let mut encode = |enc: &mut Codec, out: &mut MemOutput, frame: Option<&Frame>| -> Result<(), Stop> {
        enc.send_frame(frame)?;

        while enc.receive_packet(&mut pkt)? {
            let mut p = pkt.take();
            p.get_mut().stream_index = 0;
            unsafe { ffi::av_packet_rescale_ts(p.0, tb, (*st).time_base) };
            out.write(&mut p)?;
        }

        send(tx, out.take())
    };

    let mut finished = false;

    loop {
        if !finished {
            match dec.next()? {
                Some(frame) => {
                    let f = frame.get();

                    unsafe {
                        ff::check(
                            ffi::av_audio_fifo_write(fifo.0, f.extended_data as *const *mut c_void, f.nb_samples),
                            "audio fifo",
                        )?;
                    }
                },
                None => finished = true,
            }
        }
        loop {
            let available = unsafe { ffi::av_audio_fifo_size(fifo.0) };

            if available == 0 || (available < frame_size && !finished) {
                break;
            }

            let n = available.min(frame_size);
            let size = if n < frame_size && !small_last { frame_size } else { n };
            let mut frame = Frame::new();

            unsafe {
                let f = frame.get_mut();
                f.nb_samples = size;
                f.format = format;
                f.sample_rate = rate;
                ffi::av_channel_layout_default(&mut f.ch_layout, channels);
                ff::check(ffi::av_frame_get_buffer(frame.0, 0), "audio buffer")?;
                let f = frame.get_mut();
                ffi::av_audio_fifo_read(fifo.0, f.extended_data as *const *mut c_void, n);

                if n < size {
                    ffi::av_samples_set_silence(f.extended_data, n, size - n, channels, format);
                }

                f.pts = pts;
            }

            pts += size as i64;
            encode(&mut enc, &mut out, Some(&frame))?;
        }
        if finished {
            break;
        }
    }

    encode(&mut enc, &mut out, None)?;
    out.finish()?;
    send(tx, out.take())
}

struct Fifo(*mut ffi::AVAudioFifo);

impl Drop for Fifo {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { ffi::av_audio_fifo_free(self.0) }
        }
    }
}
