// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::c_int;
use std::path::PathBuf;
use std::{io, ptr};

use bytes::Bytes;
use tokio::sync::mpsc;

use super::ff::{self, BufferRef, Codec, Dict, FilterGraph, Frame, Input, MemOutput, Packet, ffi};
use super::hw;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoMode {
    Copy,

    Transcode { max_height: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioMode {
    Copy,

    Aac,
}

#[derive(Debug, Clone)]
pub struct StreamRequest {
    pub path: PathBuf,
    pub start: f64,
    pub video: VideoMode,
    pub audio_stream: Option<usize>,
    pub audio: AudioMode,
}

pub type Chunk = Result<Bytes, io::Error>;

enum Stop {
    Closed,

    Hardware(anyhow::Error),
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for Stop {
    fn from(e: anyhow::Error) -> Self {
        Stop::Failed(e)
    }
}

struct Sink {
    tx: mpsc::Sender<Chunk>,
    sent: bool,
}

impl Sink {
    fn send(&mut self, bytes: Vec<u8>) -> Result<(), Stop> {
        if bytes.is_empty() {
            return Ok(());
        }

        self.sent = true;
        self.tx.blocking_send(Ok(Bytes::from(bytes))).map_err(|_| Stop::Closed)
    }
}

pub fn spawn(req: StreamRequest, hw: Option<BufferRef>) -> mpsc::Receiver<Chunk> {
    let (tx, rx) = mpsc::channel(4);

    std::thread::Builder::new()
        .name("tinystream-stream".into())
        .spawn(move || {
            let started = std::time::Instant::now();
            let mut sink = Sink { tx, sent: false };
            let mut result = run(&req, hw.as_ref(), &mut sink);

            if let Err(Stop::Hardware(e)) = &result {
                tracing::warn!(
                    "hardware transcoding couldn't handle {}: {e:#}; retrying in software",
                    req.path.display()
                );
                result = run(&req, None, &mut sink);
            }

            match result {
                Ok(()) => tracing::debug!("stream of {} finished in {:.1?}", req.path.display(), started.elapsed()),
                Err(Stop::Closed) => tracing::debug!("stream of {} closed by the client", req.path.display()),
                Err(Stop::Hardware(e) | Stop::Failed(e)) => {
                    tracing::error!("streaming {} failed: {e:#}", req.path.display());
                    let _ = sink.tx.blocking_send(Err(io::Error::other(format!("{e:#}"))));
                },
            }
        })
        .expect("can't spawn a thread");

    rx
}

struct Mux {
    out: MemOutput,

    pending: Vec<(Packet, ffi::AVRational)>,
    ready: Vec<bool>,
    opts: Dict,
}

impl Mux {
    fn write(&mut self, mut pkt: Packet, tb: ffi::AVRational, sink: &mut Sink) -> Result<(), Stop> {
        if !self.out.header_written() {
            self.pending.push((pkt, tb));
            return Ok(());
        }

        let idx = pkt.get().stream_index as usize;
        let ost_tb = self.out.stream(idx).time_base;
        unsafe { ffi::av_packet_rescale_ts(pkt.0, tb, ost_tb) };
        self.out.write(&mut pkt)?;
        sink.send(self.out.take())
    }

    fn mark_ready(&mut self, idx: usize, sink: &mut Sink) -> Result<(), Stop> {
        self.ready[idx] = true;

        if self.ready.iter().all(|r| *r) && !self.out.header_written() {
            self.out.write_header(&mut self.opts)?;
            sink.send(self.out.take())?;

            for (pkt, tb) in std::mem::take(&mut self.pending) {
                self.write(pkt, tb, sink)?;
            }
        }

        Ok(())
    }
}

fn run(req: &StreamRequest, hw: Option<&BufferRef>, sink: &mut Sink) -> Result<(), Stop> {
    let mut input = Input::open(&req.path, true)?;
    let v_idx = unsafe { ffi::av_find_best_stream(input.0, ffi::AVMEDIA_TYPE_VIDEO, -1, -1, ptr::null_mut(), 0) };

    if v_idx < 0 {
        return Err(Stop::Failed(anyhow::anyhow!("no video track")));
    }

    let v_idx = v_idx as usize;

    let a_idx = match req.audio_stream {
        Some(i) => Some(i),
        None => {
            let i = unsafe {
                ffi::av_find_best_stream(input.0, ffi::AVMEDIA_TYPE_AUDIO, -1, v_idx as c_int, ptr::null_mut(), 0)
            };

            (i >= 0).then_some(i as usize)
        },
    };

    for (i, &st) in input.streams().iter().enumerate() {
        if i != v_idx && Some(i) != a_idx {
            unsafe { (*st).discard = ffi::AVDISCARD_ALL };
        }
    }

    if req.start > 0.25 {
        let ts = (req.start * ffi::AV_TIME_BASE as f64) as i64;
        let r = unsafe { ffi::av_seek_frame(input.0, -1, ts, ffi::AVSEEK_FLAG_BACKWARD as c_int) };

        if r < 0 {
            tracing::warn!("can't seek in {}: {}; starting from the beginning", req.path.display(), ff::err_str(r));
        }
    }

    let mut out = MemOutput::new("mp4")?;
    unsafe { (*out.ctx).strict_std_compliance = -2 };

    let frag_duration = match req.video {
        VideoMode::Copy => "1000000",
        VideoMode::Transcode { .. } => "250000",
    };

    let copies_ac3 = req.audio == AudioMode::Copy
        && a_idx.and_then(|i| input.stream(i)).is_some_and(|st| {
            matches!(unsafe { (*st.codecpar).codec_id }, ffi::AV_CODEC_ID_AC3 | ffi::AV_CODEC_ID_EAC3)
        });

    let movflags = if copies_ac3 {
        "empty_moov+delay_moov+default_base_moof+frag_keyframe+negative_cts_offsets+frag_discont"
    } else {
        "empty_moov+default_base_moof+frag_keyframe+negative_cts_offsets+frag_discont"
    };

    let opts = Dict::new(&[("movflags", movflags), ("frag_duration", frag_duration)]);

    let vst = input.stream(v_idx).unwrap();
    let v_tb = vst.time_base;
    let v_out = out.add_stream();

    let mut video = match req.video {
        VideoMode::Copy => {
            unsafe {
                ff::check(ffi::avcodec_parameters_copy((*v_out).codecpar, vst.codecpar), "copy video params")?;
                let par = &mut *(*v_out).codecpar;

                par.codec_tag =
                    if par.codec_id == ffi::AV_CODEC_ID_HEVC { ffi::MKTAG(b'h', b'v', b'c', b'1') } else { 0 };

                (*v_out).time_base = v_tb;
            }
            Video::Copy { started: false }
        },
        VideoMode::Transcode { max_height } => Video::Transcode(Box::new(VideoTranscode::new(
            vst,
            hw,
            max_height,
            (req.start * ff::q2d(ffi::av_inv_q(v_tb))) as i64,
        )?)),
    };

    let mut audio = match a_idx {
        None => None,
        Some(i) => {
            let ast = input.stream(i).ok_or_else(|| anyhow::anyhow!("no audio track #{i}"))?;
            let a_out = out.add_stream();

            Some(match req.audio {
                AudioMode::Copy => {
                    unsafe {
                        ff::check(ffi::avcodec_parameters_copy((*a_out).codecpar, ast.codecpar), "copy audio params")?;
                        (*(*a_out).codecpar).codec_tag = 0;
                        (*a_out).time_base = ast.time_base;
                    }
                    Audio::Copy { tb: ast.time_base }
                },
                AudioMode::Aac => {
                    let t = AudioTranscode::new(ast)?;

                    unsafe {
                        ff::check(ffi::avcodec_parameters_from_context((*a_out).codecpar, t.enc.0), "audio params")?;
                        (*a_out).time_base = t.enc.get().time_base;
                    }

                    Audio::Transcode(Box::new(t))
                },
            })
        },
    };

    let mut mux = Mux { out, pending: Vec::new(), ready: vec![false; if audio.is_some() { 2 } else { 1 }], opts };

    if matches!(video, Video::Copy { .. }) {
        mux.mark_ready(0, sink)?;
    }

    if audio.is_some() {
        mux.mark_ready(1, sink)?;
    }

    let mut video_start: Option<f64> = match req.video {
        VideoMode::Copy => None,
        VideoMode::Transcode { .. } => Some(req.start),
    };

    let mut held_audio: Vec<Packet> = Vec::new();
    let mut pkt = Packet::new();

    while input.read(&mut pkt)? {
        let idx = pkt.get().stream_index as usize;

        if idx == v_idx {
            match &mut video {
                Video::Copy { started } => {
                    if !*started {
                        if pkt.get().flags & ffi::AV_PKT_FLAG_KEY as c_int == 0 {
                            pkt.unref();
                            continue;
                        }

                        *started = true;
                        let p = pkt.get();
                        let ts = if p.pts != ffi::AV_NOPTS_VALUE { p.pts } else { p.dts };
                        video_start = Some(ts as f64 * ff::q2d(v_tb));
                    }

                    let mut p = pkt.take();
                    p.get_mut().stream_index = 0;
                    mux.write(p, v_tb, sink)?;
                },
                Video::Transcode(t) => {
                    let r = t.packet(Some(&pkt), &mut mux, sink);
                    pkt.unref();

                    match r {
                        Err(Stop::Failed(e)) if t.hw_attempted && !sink.sent => return Err(Stop::Hardware(e)),
                        r => r?,
                    }
                },
            }
            if let Some(start) = video_start
                && !held_audio.is_empty()
            {
                for p in std::mem::take(&mut held_audio) {
                    if let Some(a) = &mut audio {
                        a.packet(p, start, &mut mux, sink)?;
                    }
                }
            }
        } else if Some(idx) == a_idx {
            let p = pkt.take();

            match (video_start, &mut audio) {
                (Some(start), Some(a)) => a.packet(p, start, &mut mux, sink)?,
                _ => held_audio.push(p),
            }
        } else {
            pkt.unref();
        }
    }

    if let Video::Transcode(t) = &mut video {
        t.packet(None, &mut mux, sink)?;
    }

    if let Some(Audio::Transcode(t)) = &mut audio {
        t.flush(&mut mux, sink)?;
    }

    if !mux.out.header_written() {
        return Err(Stop::Failed(anyhow::anyhow!("no video frames at or after this position")));
    }

    mux.out.finish()?;
    sink.send(mux.out.take())?;
    Ok(())
}

enum Video {
    Copy { started: bool },
    Transcode(Box<VideoTranscode>),
}

struct VideoTranscode {
    dec: Codec,
    tb: ffi::AVRational,
    frame_rate: ffi::AVRational,
    hw: Option<BufferRef>,
    hw_attempted: bool,
    max_height: i32,
    start_pts: i64,
    graph: Option<FilterGraph>,
    enc: Option<Codec>,
    frame: Frame,
    filtered: Frame,
    out_pkt: Packet,
    first: bool,
}

pub(super) unsafe extern "C" fn prefer_vaapi(
    _ctx: *mut ffi::AVCodecContext,
    fmts: *const ffi::AVPixelFormat,
) -> ffi::AVPixelFormat {
    let mut p = fmts;

    unsafe {
        while *p != ffi::AV_PIX_FMT_NONE {
            if *p == ffi::AV_PIX_FMT_VAAPI {
                return *p;
            }

            p = p.add(1);
        }

        *fmts
    }
}

fn bitrate_for(height: i32) -> i64 {
    match height {
        h if h >= 2000 => 20_000_000,
        h if h >= 1400 => 12_000_000,
        h if h >= 1000 => 8_000_000,
        h if h >= 700 => 4_500_000,
        _ => 2_000_000,
    }
}

impl VideoTranscode {
    fn new(st: &ffi::AVStream, hw: Option<&BufferRef>, max_height: i32, start_pts: i64) -> anyhow::Result<Self> {
        let par = unsafe { &*st.codecpar };
        let mut decoder = unsafe { ffi::avcodec_find_decoder(par.codec_id) };

        if par.codec_id == ffi::AV_CODEC_ID_AV1 {
            let name = if hw.is_some() && hw::GPU_FRAMES { c"av1" } else { c"libdav1d" };
            let d = unsafe { ffi::avcodec_find_decoder_by_name(name.as_ptr()) };

            if !d.is_null() {
                decoder = d;
            }
        }

        if decoder.is_null() {
            anyhow::bail!("no decoder for this video codec");
        }

        let mut dec = Codec::alloc(decoder)?;

        unsafe {
            ff::check(ffi::avcodec_parameters_to_context(dec.0, st.codecpar), "decoder params")?;
            let d = dec.get_mut();
            d.pkt_timebase = st.time_base;
            d.thread_count = 0;

            if let Some(dev) = hw.filter(|_| hw::GPU_FRAMES) {
                d.hw_device_ctx = dev.new_ref();
                d.get_format = Some(prefer_vaapi);
                d.extra_hw_frames = 8;
            }
        }

        dec.open(&mut Dict::new(&[]))?;
        let frame_rate = if st.avg_frame_rate.num > 0 { st.avg_frame_rate } else { st.r_frame_rate };

        Ok(Self {
            dec,
            tb: st.time_base,
            frame_rate: if frame_rate.num > 0 { frame_rate } else { ff::q(24, 1) },
            hw: hw.cloned(),
            hw_attempted: hw.is_some(),
            max_height,
            start_pts,
            graph: None,
            enc: None,
            frame: Frame::new(),
            filtered: Frame::new(),
            out_pkt: Packet::new(),
            first: true,
        })
    }

    fn packet(&mut self, pkt: Option<&Packet>, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        loop {
            let accepted = self.dec.send_packet(pkt)?;
            self.drain_decoder(mux, sink)?;

            if accepted {
                break;
            }
        }
        if pkt.is_none() {
            if let Some(g) = &mut self.graph {
                g.push(None)?;
                self.drain_filter(mux, sink)?;
            }
            if let Some(enc) = &mut self.enc {
                enc.send_frame(None)?;
                self.drain_encoder(mux, sink)?;
            }
        }
        Ok(())
    }

    fn drain_decoder(&mut self, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        while self.dec.receive_frame(&mut self.frame)? {
            let f = self.frame.get_mut();
            let pts = if f.best_effort_timestamp != ffi::AV_NOPTS_VALUE { f.best_effort_timestamp } else { f.pts };

            if pts != ffi::AV_NOPTS_VALUE && pts < self.start_pts {
                self.frame.unref();
                continue;
            }

            f.pts = pts;

            if self.graph.is_none() {
                self.graph = Some(self.build_graph()?);
            }

            self.graph.as_mut().unwrap().push(Some(&mut self.frame))?;
            self.frame.unref();
            self.drain_filter(mux, sink)?;
        }
        Ok(())
    }

    fn build_graph(&mut self) -> anyhow::Result<FilterGraph> {
        let f = self.frame.get();
        let height = f.height.min(self.max_height.max(144)) & !1;
        let width = ((f.width as i64 * height as i64 / f.height.max(1) as i64) as i32 + 1) & !1;

        let (desc, device) = match (&self.hw, f.format == ffi::AV_PIX_FMT_VAAPI) {
            (Some(_), true) => (format!("scale_vaapi=w={width}:h={height}:format=nv12"), None),
            (Some(dev), false) if hw::GPU_FRAMES => (format!("scale={width}:{height},format=nv12,hwupload"), Some(dev)),
            (Some(_), false) => (format!("scale={width}:{height}:flags=bicubic,format=nv12"), None),
            (None, _) => (format!("scale={width}:{height}:flags=bicubic,format=yuv420p"), None),
        };

        tracing::debug!(
            "transcoding {}x{} → {width}x{height} via `{desc}` ({} decode)",
            f.width,
            f.height,
            if f.format == ffi::AV_PIX_FMT_VAAPI { "VA-API" } else { "software" }
        );

        FilterGraph::video(f, self.tb, &desc, device)
    }

    fn drain_filter(&mut self, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        loop {
            let got = self.graph.as_mut().unwrap().pull(&mut self.filtered)?;

            if !got {
                return Ok(());
            }

            if self.enc.is_none() {
                self.open_encoder(mux, sink)?;
            }

            let f = self.filtered.get_mut();
            f.pict_type = if self.first { ffi::AV_PICTURE_TYPE_I } else { ffi::AV_PICTURE_TYPE_NONE };
            self.first = false;
            self.enc.as_mut().unwrap().send_frame(Some(&self.filtered))?;
            self.filtered.unref();
            self.drain_encoder(mux, sink)?;
        }
    }

    fn open_encoder(&mut self, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        let g = self.graph.as_ref().unwrap();
        let hw_frames = unsafe { ffi::av_buffersink_get_hw_frames_ctx(g.sink) };
        let hardware = if hw::GPU_FRAMES { !hw_frames.is_null() } else { self.hw.is_some() };
        let name = hw::h264_encoder(hardware);
        let codec = unsafe { ffi::avcodec_find_encoder_by_name(name.as_ptr()) };

        if codec.is_null() {
            return Err(Stop::Failed(anyhow::anyhow!("no {name:?} encoder in this build")));
        }

        let mut enc = Codec::alloc(codec)?;
        let (w, h) = unsafe { (ffi::av_buffersink_get_w(g.sink), ffi::av_buffersink_get_h(g.sink)) };
        let bitrate = bitrate_for(h);
        let fps = ff::q2d(self.frame_rate).round().max(1.0) as c_int;

        unsafe {
            let e = enc.get_mut();
            e.width = w;
            e.height = h;
            e.time_base = ffi::av_buffersink_get_time_base(g.sink);
            e.framerate = self.frame_rate;
            e.sample_aspect_ratio = ffi::av_buffersink_get_sample_aspect_ratio(g.sink);
            e.pix_fmt = ffi::av_buffersink_get_format(g.sink);
            e.gop_size = fps * 2;
            e.bit_rate = bitrate;
            e.rc_max_rate = bitrate * 3 / 2;
            e.rc_buffer_size = (bitrate * 2) as c_int;
            e.profile = ffi::AV_PROFILE_H264_HIGH as c_int;
            e.flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;

            if !hw_frames.is_null() {
                e.hw_frames_ctx = ffi::av_buffer_ref(hw_frames);
            }

            if hardware {
                e.max_b_frames = 0;
            } else {
                e.thread_count = 0;
            }
        }

        let mut opts = if hardware { Dict::new(&[]) } else { Dict::new(&[("preset", "veryfast"), ("crf", "21")]) };

        if let Err(e) = enc.open(&mut opts) {
            let e = e.context(format!("opening {name:?}"));
            return Err(if self.hw_attempted && !sink.sent { Stop::Hardware(e) } else { Stop::Failed(e) });
        }

        unsafe {
            let st = *(*mux.out.ctx).streams;
            ff::check(ffi::avcodec_parameters_from_context((*st).codecpar, enc.0), "video params")?;
            (*st).time_base = enc.get().time_base;
            (*st).avg_frame_rate = self.frame_rate;
        }

        self.enc = Some(enc);
        mux.mark_ready(0, sink)
    }

    fn drain_encoder(&mut self, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        let enc = self.enc.as_mut().unwrap();
        let tb = enc.get().time_base;

        while enc.receive_packet(&mut self.out_pkt)? {
            let mut p = self.out_pkt.take();
            p.get_mut().stream_index = 0;
            mux.write(p, tb, sink)?;
        }

        Ok(())
    }
}

enum Audio {
    Copy { tb: ffi::AVRational },
    Transcode(Box<AudioTranscode>),
}

impl Audio {
    fn packet(&mut self, mut p: Packet, video_start: f64, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        match self {
            Audio::Copy { tb } => {
                let pk = p.get();

                if pk.pts != ffi::AV_NOPTS_VALUE {
                    let end = (pk.pts + pk.duration) as f64 * ff::q2d(*tb);

                    if end < video_start {
                        return Ok(());
                    }
                }

                p.get_mut().stream_index = 1;
                mux.write(p, *tb, sink)
            },
            Audio::Transcode(t) => t.packet(&p, video_start, mux, sink),
        }
    }
}

const AAC_RATE: c_int = 48_000;
const AAC_FRAME: c_int = 1024;

struct AudioTranscode {
    dec: Codec,
    enc: Codec,
    tb: ffi::AVRational,
    swr: *mut ffi::SwrContext,
    fifo: *mut ffi::AVAudioFifo,
    next_pts: Option<i64>,
    frame: Frame,
    out_pkt: Packet,
    layout: ffi::AVChannelLayout,
}

unsafe impl Send for AudioTranscode {}

impl Drop for AudioTranscode {
    fn drop(&mut self) {
        unsafe {
            ffi::swr_free(&mut self.swr);

            if !self.fifo.is_null() {
                ffi::av_audio_fifo_free(self.fifo);
            }

            ffi::av_channel_layout_uninit(&mut self.layout);
        }
    }
}

impl AudioTranscode {
    fn new(st: &ffi::AVStream) -> anyhow::Result<Self> {
        let par = unsafe { &*st.codecpar };
        let decoder = unsafe { ffi::avcodec_find_decoder(par.codec_id) };

        if decoder.is_null() {
            anyhow::bail!("no decoder for this audio codec");
        }

        let mut dec = Codec::alloc(decoder)?;

        unsafe {
            ff::check(ffi::avcodec_parameters_to_context(dec.0, st.codecpar), "decoder params")?;
            dec.get_mut().pkt_timebase = st.time_base;
        }

        dec.open(&mut Dict::new(&[]))?;

        let encoder = unsafe { ffi::avcodec_find_encoder(ffi::AV_CODEC_ID_AAC) };
        let mut enc = Codec::alloc(encoder)?;
        let mut layout: ffi::AVChannelLayout = unsafe { std::mem::zeroed() };

        unsafe {
            ffi::av_channel_layout_default(&mut layout, 2);
            let e = enc.get_mut();
            ff::check(ffi::av_channel_layout_copy(&mut e.ch_layout, &layout), "channel layout")?;
            e.sample_rate = AAC_RATE;
            e.sample_fmt = ffi::AV_SAMPLE_FMT_FLTP;
            e.bit_rate = 192_000;
            e.time_base = ff::q(1, AAC_RATE);
            e.flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;
        }

        enc.open(&mut Dict::new(&[]))?;
        let fifo = unsafe { ffi::av_audio_fifo_alloc(ffi::AV_SAMPLE_FMT_FLTP, 2, AAC_FRAME * 8) };

        Ok(Self {
            dec,
            enc,
            tb: st.time_base,
            swr: ptr::null_mut(),
            fifo,
            next_pts: None,
            frame: Frame::new(),
            out_pkt: Packet::new(),
            layout,
        })
    }

    fn packet(&mut self, p: &Packet, video_start: f64, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        loop {
            let accepted = self.dec.send_packet(Some(p))?;
            self.drain_decoder(video_start, mux, sink)?;

            if accepted {
                return Ok(());
            }
        }
    }

    fn drain_decoder(&mut self, video_start: f64, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        while self.dec.receive_frame(&mut self.frame)? {
            let f = self.frame.get();
            let ts = if f.best_effort_timestamp != ffi::AV_NOPTS_VALUE { f.best_effort_timestamp } else { f.pts };
            let secs = if ts == ffi::AV_NOPTS_VALUE { None } else { Some(ts as f64 * ff::q2d(self.tb)) };
            let dur = f.nb_samples as f64 / f.sample_rate.max(1) as f64;

            if secs.is_some_and(|s| s + dur < video_start) {
                self.frame.unref();
                continue;
            }

            if self.next_pts.is_none() {
                self.next_pts = Some((secs.unwrap_or(video_start) * AAC_RATE as f64).round() as i64);
            }

            if self.swr.is_null() {
                self.open_resampler()?;
            }

            let mut out = Frame::new();

            unsafe {
                let o = out.get_mut();
                ffi::av_channel_layout_copy(&mut o.ch_layout, &self.layout);
                o.sample_rate = AAC_RATE;
                o.format = ffi::AV_SAMPLE_FMT_FLTP;
                ff::check(ffi::swr_convert_frame(self.swr, out.0, self.frame.0), "resample")?;
            }

            self.frame.unref();
            self.push_fifo(&out)?;
            self.encode_fifo(false, mux, sink)?;
        }
        Ok(())
    }

    fn open_resampler(&mut self) -> anyhow::Result<()> {
        unsafe {
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
                    ffi::AV_SAMPLE_FMT_FLTP,
                    AAC_RATE,
                    &f.ch_layout,
                    f.format,
                    f.sample_rate,
                    0,
                    ptr::null_mut(),
                ),
                "resampler",
            )?;

            ff::check(ffi::swr_init(self.swr), "resampler")?;
        }
        Ok(())
    }

    fn push_fifo(&mut self, out: &Frame) -> anyhow::Result<()> {
        let o = out.get();

        if o.nb_samples > 0 {
            unsafe {
                ff::check(
                    ffi::av_audio_fifo_write(self.fifo, o.extended_data as *const *mut std::ffi::c_void, o.nb_samples),
                    "audio fifo",
                )?;
            }
        }

        Ok(())
    }

    fn encode_fifo(&mut self, flush: bool, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        loop {
            let available = unsafe { ffi::av_audio_fifo_size(self.fifo) };

            if available < AAC_FRAME && !(flush && available > 0) {
                return Ok(());
            }

            let n = available.min(AAC_FRAME);
            let mut frame = Frame::new();

            unsafe {
                let f = frame.get_mut();
                f.nb_samples = AAC_FRAME;
                f.format = ffi::AV_SAMPLE_FMT_FLTP;
                f.sample_rate = AAC_RATE;
                ffi::av_channel_layout_copy(&mut f.ch_layout, &self.layout);
                ff::check(ffi::av_frame_get_buffer(frame.0, 0), "audio buffer")?;
                let f = frame.get_mut();
                ffi::av_audio_fifo_read(self.fifo, f.extended_data as *const *mut std::ffi::c_void, n);

                if n < AAC_FRAME {
                    ffi::av_samples_set_silence(f.extended_data, n, AAC_FRAME - n, 2, ffi::AV_SAMPLE_FMT_FLTP);
                }

                let pts = self.next_pts.get_or_insert(0);
                f.pts = *pts;
                *pts += n as i64;
            }

            self.enc.send_frame(Some(&frame))?;
            self.drain_encoder(mux, sink)?;
        }
    }

    fn drain_encoder(&mut self, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        let tb = self.enc.get().time_base;

        while self.enc.receive_packet(&mut self.out_pkt)? {
            let mut p = self.out_pkt.take();
            p.get_mut().stream_index = 1;
            mux.write(p, tb, sink)?;
        }

        Ok(())
    }

    fn flush(&mut self, mux: &mut Mux, sink: &mut Sink) -> Result<(), Stop> {
        self.dec.send_packet(None)?;
        self.drain_decoder(f64::MIN, mux, sink)?;

        if !self.swr.is_null() {
            let mut out = Frame::new();

            unsafe {
                let o = out.get_mut();
                ffi::av_channel_layout_copy(&mut o.ch_layout, &self.layout);
                o.sample_rate = AAC_RATE;
                o.format = ffi::AV_SAMPLE_FMT_FLTP;
                ff::check(ffi::swr_convert_frame(self.swr, out.0, ptr::null()), "resample")?;
            }

            self.push_fifo(&out)?;
        }

        self.encode_fifo(true, mux, sink)?;
        self.enc.send_frame(None)?;
        self.drain_encoder(mux, sink)
    }
}
