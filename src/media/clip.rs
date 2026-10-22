// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::c_int;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, anyhow, bail};

use super::burn::{self, Overlay};
use super::ff::{self, BufferRef, Codec, Dict, FilterGraph, Frame, Input, Packet, ffi};
use super::hw;

#[derive(Clone)]
pub struct Recipe {
    pub start: f64,
    pub end: f64,

    pub audio: Option<usize>,
    pub subtitles: Option<burn::Source>,

    pub height: i32,

    pub half_rate: bool,
    pub title: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Rendered {
    pub width: i32,
    pub height: i32,
    pub fps: f64,
}

pub const HEIGHTS: [i32; 3] = [1080, 720, 480];

pub fn frame_rate(source: ffi::AVRational, half: bool) -> ffi::AVRational {
    let mut r = if source.num > 0 && source.den > 0 { source } else { ff::q(24, 1) };

    while ff::q2d(r) > 61.0 {
        r = halve(r);
    }

    if half && ff::q2d(r) > 31.0 { halve(r) } else { r }
}

fn halve(r: ffi::AVRational) -> ffi::AVRational {
    if r.num % 2 == 0 { ff::q(r.num / 2, r.den) } else { ff::q(r.num, r.den * 2) }
}

pub fn max_bitrate(height: i32, fps: f64) -> i64 {
    let fast = fps > 31.0;

    match height {
        h if h > 720 => {
            if fast {
                8_000_000
            } else {
                6_000_000
            }
        },
        h if h > 480 => {
            if fast {
                5_000_000
            } else {
                3_500_000
            }
        },
        _ => 1_500_000,
    }
}

pub const AUDIO_BITRATE: i64 = 160_000;

pub fn fit(width: i32, height: i32, sar: ffi::AVRational, box_height: i32) -> (i32, i32) {
    let sar = if sar.num > 0 && sar.den > 0 { ff::q2d(sar) } else { 1.0 };
    let (w, h) = (width as f64 * sar, height as f64);
    let box_w = box_height as f64 * 16.0 / 9.0;
    let scale = (box_w / w).min(box_height as f64 / h).min(1.0);
    let even = |v: f64| ((v / 2.0).round() as i32 * 2).max(2);
    (even(w * scale), even(h * scale))
}

pub fn render(
    video: &Path,
    recipe: &Recipe,
    out: &Path,
    hw: Option<&BufferRef>,
    fonts: &burn::Fonts,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> anyhow::Result<Rendered> {
    let tmp = out.with_extension("part");
    let mut result = run(video, recipe, &tmp, hw, fonts, progress, cancel);

    if let Err(e) = &result
        && hw.is_some()
        && !cancel.load(Ordering::Relaxed)
    {
        tracing::warn!(
            "hardware transcoding couldn't render a clip of {}: {e:#}; retrying in software",
            video.display()
        );
        result = run(video, recipe, &tmp, None, fonts, progress, cancel);
    }

    match result {
        Ok(r) => {
            std::fs::rename(&tmp, out).with_context(|| format!("can't save {}", out.display()))?;
            Ok(r)
        },
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        },
    }
}

fn run(
    video: &Path,
    recipe: &Recipe,
    out: &Path,
    hw: Option<&BufferRef>,
    fonts: &burn::Fonts,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> anyhow::Result<Rendered> {
    let (start, end) = (recipe.start, recipe.end);
    anyhow::ensure!(end > start, "the clip ends before it starts");
    let mut input = Input::open(video, true)?;
    let v_idx = unsafe { ffi::av_find_best_stream(input.0, ffi::AVMEDIA_TYPE_VIDEO, -1, -1, ptr::null_mut(), 0) };
    anyhow::ensure!(v_idx >= 0, "no video track");
    let v_idx = v_idx as usize;

    let a_idx = recipe.audio.or_else(|| {
        let i = unsafe {
            ffi::av_find_best_stream(input.0, ffi::AVMEDIA_TYPE_AUDIO, -1, v_idx as c_int, ptr::null_mut(), 0)
        };

        (i >= 0).then_some(i as usize)
    });

    for (i, &st) in input.streams().iter().enumerate() {
        if i != v_idx && Some(i) != a_idx {
            unsafe { (*st).discard = ffi::AVDISCARD_ALL };
        }
    }

    if start > 0.25 {
        let ts = (start * ffi::AV_TIME_BASE as f64) as i64;
        let r = unsafe { ffi::av_seek_frame(input.0, -1, ts, ffi::AVSEEK_FLAG_BACKWARD as c_int) };

        if r < 0 {
            tracing::warn!("can't seek in {}: {}; reading from the beginning", video.display(), ff::err_str(r));
        }
    }

    let vst = input.stream(v_idx).unwrap();
    let par = unsafe { &*vst.codecpar };
    let sar = if par.sample_aspect_ratio.num > 0 { par.sample_aspect_ratio } else { vst.sample_aspect_ratio };
    let size = fit(par.width, par.height, sar, recipe.height);
    let source_rate = if vst.avg_frame_rate.num > 0 { vst.avg_frame_rate } else { vst.r_frame_rate };
    let rate = frame_rate(source_rate, recipe.half_rate);

    let overlay = match &recipe.subtitles {
        Some(s) => Some(Overlay::new(s.clone(), video, fonts, size, (par.width, par.height), start, end)?),
        None => None,
    };

    let mut mux = Mux::create(out, &recipe.title, a_idx.is_some())?;
    let mut pic = Picture::new(vst, hw, size, rate, start, end, overlay)?;

    let mut sound = match a_idx {
        Some(i) => {
            Some(Sound::new(input.stream(i).ok_or_else(|| anyhow!("no audio track #{i}"))?, start, end, &mut mux)?)
        },
        None => None,
    };

    let mut pkt = Packet::new();
    let mut reported = -1.0f32;

    while input.read(&mut pkt)? {
        if cancel.load(Ordering::Relaxed) {
            bail!("cancelled");
        }

        let idx = pkt.get().stream_index as usize;

        if idx == v_idx && !pic.done {
            pic.packet(Some(&pkt), &mut mux)?;
            let p = ((pic.reached - start) / (end - start)).clamp(0.0, 1.0) as f32;

            if p - reported >= 0.01 {
                reported = p;
                progress(p * 0.98);
            }
        } else if Some(idx) == a_idx
            && let Some(s) = &mut sound
            && !s.done
        {
            s.packet(&pkt, &mut mux)?;
        }

        pkt.unref();

        if pic.done && sound.as_ref().is_none_or(|s| s.done) {
            break;
        }
    }

    pic.packet(None, &mut mux)?;

    if let Some(s) = &mut sound {
        s.finish(&mut mux)?;
    }

    anyhow::ensure!(mux.header, "there's no video in this range");
    mux.finish()?;
    progress(1.0);
    Ok(Rendered { width: size.0, height: size.1, fps: ff::q2d(rate) })
}

struct Mux {
    ctx: *mut ffi::AVFormatContext,
    pending: Vec<(Packet, ffi::AVRational)>,
    waiting_for: usize,
    header: bool,
}

impl Mux {
    fn create(path: &Path, title: &str, audio: bool) -> anyhow::Result<Self> {
        let p = ff::cstr(&path.to_string_lossy());
        let mut ctx = ptr::null_mut();

        ff::check(
            unsafe { ffi::avformat_alloc_output_context2(&mut ctx, ptr::null(), c"mp4".as_ptr(), p.as_ptr()) },
            "output format",
        )?;

        let mux = Mux { ctx, pending: Vec::new(), waiting_for: if audio { 2 } else { 1 }, header: false };

        unsafe {
            ff::check(
                ffi::avio_open(&mut (*ctx).pb, p.as_ptr(), ffi::AVIO_FLAG_WRITE as c_int),
                "can't create the clip",
            )?;

            if !title.trim().is_empty() {
                let (k, v) = (ff::cstr("title"), ff::cstr(title.trim()));
                ffi::av_dict_set(&mut (*ctx).metadata, k.as_ptr(), v.as_ptr(), 0);
            }

            ffi::avformat_new_stream(ctx, ptr::null());

            if audio {
                ffi::avformat_new_stream(ctx, ptr::null());
            }
        }

        Ok(mux)
    }

    fn stream(&self, i: usize) -> *mut ffi::AVStream {
        unsafe { *(*self.ctx).streams.add(i) }
    }

    fn ready(&mut self) -> anyhow::Result<()> {
        self.waiting_for -= 1;

        if self.waiting_for > 0 {
            return Ok(());
        }

        let mut opts = Dict::new(&[("movflags", "+faststart")]);
        ff::check(unsafe { ffi::avformat_write_header(self.ctx, &mut opts.0) }, "can't start the clip")?;
        self.header = true;

        for (pkt, tb) in std::mem::take(&mut self.pending) {
            self.write(pkt, tb)?;
        }

        Ok(())
    }

    fn write(&mut self, pkt: Packet, tb: ffi::AVRational) -> anyhow::Result<()> {
        if !self.header {
            self.pending.push((pkt, tb));
            return Ok(());
        }

        let st = self.stream(pkt.get().stream_index as usize);
        unsafe { ffi::av_packet_rescale_ts(pkt.0, tb, (*st).time_base) };
        ff::check(unsafe { ffi::av_interleaved_write_frame(self.ctx, pkt.0) }, "can't write the clip")?;
        Ok(())
    }

    fn finish(&mut self) -> anyhow::Result<()> {
        ff::check(unsafe { ffi::av_write_trailer(self.ctx) }, "can't finish the clip")?;
        Ok(())
    }
}

impl Drop for Mux {
    fn drop(&mut self) {
        unsafe {
            if !(*self.ctx).pb.is_null() {
                ffi::avio_closep(&mut (*self.ctx).pb);
            }

            ffi::avformat_free_context(self.ctx);
        }
    }
}

struct Picture {
    dec: Codec,
    tb: ffi::AVRational,
    hw: Option<BufferRef>,
    size: (i32, i32),
    rate: ffi::AVRational,
    start: f64,
    end: f64,
    overlay: Option<Overlay>,

    graph: Option<FilterGraph>,

    upload: Option<FilterGraph>,
    enc: Option<Codec>,
    frame: Frame,
    filtered: Frame,
    uploaded: Frame,
    out_pkt: Packet,

    reached: f64,
    done: bool,
}

impl Picture {
    fn new(
        st: &ffi::AVStream,
        hw: Option<&BufferRef>,
        size: (i32, i32),
        rate: ffi::AVRational,
        start: f64,
        end: f64,
        overlay: Option<Overlay>,
    ) -> anyhow::Result<Self> {
        let par = unsafe { &*st.codecpar };
        let mut decoder = unsafe { ffi::avcodec_find_decoder(par.codec_id) };

        if par.codec_id == ffi::AV_CODEC_ID_AV1 {
            let name = if hw.is_some() && hw::GPU_FRAMES { c"av1" } else { c"libdav1d" };
            let d = unsafe { ffi::avcodec_find_decoder_by_name(name.as_ptr()) };

            if !d.is_null() {
                decoder = d;
            }
        }

        anyhow::ensure!(!decoder.is_null(), "no decoder for this video codec");
        let mut dec = Codec::alloc(decoder)?;

        unsafe {
            ff::check(ffi::avcodec_parameters_to_context(dec.0, st.codecpar), "decoder params")?;
            let d = dec.get_mut();
            d.pkt_timebase = st.time_base;
            d.thread_count = 0;

            if let Some(dev) = hw.filter(|_| hw::GPU_FRAMES) {
                d.hw_device_ctx = dev.new_ref();
                d.get_format = Some(super::stream::prefer_vaapi);
                d.extra_hw_frames = 8;
            }
        }

        dec.open(&mut Dict::new(&[]))?;

        Ok(Self {
            dec,
            tb: st.time_base,
            hw: hw.cloned(),
            size,
            rate,
            start,
            end,
            overlay,
            graph: None,
            upload: None,
            enc: None,
            frame: Frame::new(),
            filtered: Frame::new(),
            uploaded: Frame::new(),
            out_pkt: Packet::new(),
            reached: start,
            done: false,
        })
    }

    fn packet(&mut self, pkt: Option<&Packet>, mux: &mut Mux) -> anyhow::Result<()> {
        if pkt.is_some() {
            loop {
                let accepted = self.dec.send_packet(pkt)?;
                self.drain_decoder(mux)?;

                if accepted || self.done {
                    return Ok(());
                }
            }
        }
        if !self.done {
            self.dec.send_packet(None)?;
            self.drain_decoder(mux)?;
        }
        if let Some(g) = &mut self.graph {
            g.push(None)?;
            self.drain_graph(mux)?;
        }
        if let Some(enc) = &mut self.enc {
            enc.send_frame(None)?;
            self.drain_encoder(mux)?;
        }
        Ok(())
    }

    fn drain_decoder(&mut self, mux: &mut Mux) -> anyhow::Result<()> {
        while !self.done && self.dec.receive_frame(&mut self.frame)? {
            let f = self.frame.get_mut();
            let pts = if f.best_effort_timestamp != ffi::AV_NOPTS_VALUE { f.best_effort_timestamp } else { f.pts };

            if pts == ffi::AV_NOPTS_VALUE {
                self.frame.unref();
                continue;
            }

            let at = pts as f64 * ff::q2d(self.tb);
            self.reached = self.reached.max(at);

            if at >= self.end {
                self.done = true;
                self.frame.unref();
                break;
            }

            if at < self.start - 0.001 {
                self.frame.unref();
                continue;
            }

            f.pts = pts - (self.start / ff::q2d(self.tb)).round() as i64;

            if self.graph.is_none() {
                self.graph = Some(self.build_graph()?);
            }

            self.graph.as_mut().unwrap().push(Some(&mut self.frame))?;
            self.frame.unref();
            self.drain_graph(mux)?;
        }
        Ok(())
    }

    fn build_graph(&self) -> anyhow::Result<FilterGraph> {
        let f = self.frame.get();
        let (w, h) = self.size;
        let fps = format!("fps={}/{}", self.rate.num, self.rate.den);
        let on_gpu = f.format == ffi::AV_PIX_FMT_VAAPI;
        let hdr = matches!(f.color_trc, ffi::AVCOL_TRC_SMPTE2084 | ffi::AVCOL_TRC_ARIB_STD_B67);

        let memory = if self.hw.is_some() { "nv12" } else { "yuv420p" };
        let down = if self.overlay.is_some() { ",hwdownload,format=nv12" } else { "" };
        let scale = format!("scale={w}:{h}:flags=bicubic");

        let tonemap = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,\
                       zscale=t=bt709:m=bt709:r=tv";

        let candidates = match (on_gpu, hdr) {
            (true, false) => vec![format!("{fps},scale_vaapi=w={w}:h={h}:format=nv12{down}")],
            (true, true) => vec![
                format!(
                    "{fps},tonemap_vaapi=format=nv12:matrix=bt709:primaries=bt709:transfer=bt709,\
                     scale_vaapi=w={w}:h={h}:format=nv12{down}"
                ),
                format!("{fps},hwdownload,format={},{scale},{tonemap},format={memory}", gpu_format(f)),
            ],
            (false, false) => vec![format!("{fps},{scale},format={memory}")],
            (false, true) => vec![format!("{fps},{scale},{tonemap},format={memory}")],
        };

        let mut last = None;

        for desc in candidates {
            let desc = format!("{desc},setsar=1");

            match FilterGraph::video(f, self.tb, &desc, self.hw.as_ref()) {
                Ok(g) => {
                    tracing::debug!("clip: {}x{} → {w}x{h} via `{desc}`", f.width, f.height);
                    return Ok(g);
                },
                Err(e) => {
                    tracing::debug!("clip: `{desc}` isn't usable here: {e:#}");
                    last = Some(e);
                },
            }
        }

        Err(last.unwrap())
    }

    fn drain_graph(&mut self, mux: &mut Mux) -> anyhow::Result<()> {
        loop {
            if !self.graph.as_mut().unwrap().pull(&mut self.filtered)? {
                return Ok(());
            }

            let sink = self.graph.as_ref().unwrap().sink;
            let tb = unsafe { ffi::av_buffersink_get_time_base(sink) };
            let at = self.filtered.get().pts as f64 * ff::q2d(tb);

            if at >= self.end - self.start - 1e-6 {
                self.filtered.unref();
                continue;
            }

            if let Some(o) = &mut self.overlay {
                o.draw(&mut self.filtered, self.start + at)?;
            }

            let on_gpu = self.filtered.get().format == ffi::AV_PIX_FMT_VAAPI;

            match self.hw.clone().filter(|_| hw::GPU_FRAMES && !on_gpu) {
                Some(dev) => {
                    if self.upload.is_none() {
                        self.upload =
                            Some(FilterGraph::video(self.filtered.get(), tb, "format=nv12,hwupload", Some(&dev))?);
                    }

                    self.upload.as_mut().unwrap().push(Some(&mut self.filtered))?;
                    self.filtered.unref();

                    while self.upload.as_mut().unwrap().pull(&mut self.uploaded)? {
                        let sink = self.upload.as_ref().unwrap().sink;
                        let mut frame = std::mem::replace(&mut self.uploaded, Frame::new());
                        self.encode(&mut frame, sink, mux)?;
                    }
                },
                None => {
                    let mut frame = std::mem::replace(&mut self.filtered, Frame::new());
                    self.encode(&mut frame, sink, mux)?;
                },
            }
        }
    }

    fn encode(&mut self, frame: &mut Frame, sink: *mut ffi::AVFilterContext, mux: &mut Mux) -> anyhow::Result<()> {
        if self.enc.is_none() {
            self.enc = Some(self.open_encoder(sink, mux)?);
        }

        let f = frame.get_mut();
        f.pict_type = ffi::AV_PICTURE_TYPE_NONE;
        f.sample_aspect_ratio = ff::q(1, 1);
        self.enc.as_mut().unwrap().send_frame(Some(frame))?;
        frame.unref();
        self.drain_encoder(mux)
    }

    fn open_encoder(&self, sink: *mut ffi::AVFilterContext, mux: &mut Mux) -> anyhow::Result<Codec> {
        let hw_frames = unsafe { ffi::av_buffersink_get_hw_frames_ctx(sink) };
        let fps = ff::q2d(self.rate);
        let level = if fps > 31.0 && self.size.1 > 720 { 42 } else { 41 };
        let cap = max_bitrate(self.size.1, fps);

        let setup = |enc: &mut Codec| unsafe {
            let e = enc.get_mut();
            e.width = ffi::av_buffersink_get_w(sink);
            e.height = ffi::av_buffersink_get_h(sink);
            e.time_base = ffi::av_buffersink_get_time_base(sink);
            e.framerate = self.rate;
            e.sample_aspect_ratio = ff::q(1, 1);
            e.pix_fmt = ffi::av_buffersink_get_format(sink);
            e.gop_size = (fps * 2.0).round() as c_int;
            e.profile = ffi::AV_PROFILE_H264_HIGH as c_int;
            e.color_primaries = ffi::AVCOL_PRI_BT709;
            e.color_trc = ffi::AVCOL_TRC_BT709;
            e.colorspace = ffi::AVCOL_SPC_BT709;
            e.color_range = ffi::AVCOL_RANGE_MPEG;
            e.rc_max_rate = cap;
            e.rc_buffer_size = (cap * 2) as c_int;
            e.flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;
        };

        let enc = if !hw::GPU_FRAMES && self.hw.is_some() {
            let codec = unsafe { ffi::avcodec_find_encoder_by_name(hw::h264_encoder(true).as_ptr()) };
            anyhow::ensure!(!codec.is_null(), "no h264_videotoolbox encoder in this build");
            let mut enc = Codec::alloc(codec)?;
            setup(&mut enc);

            let e = enc.get_mut();
            e.max_b_frames = 0;
            e.level = level;
            e.bit_rate = cap * 6 / 10;

            enc.open(&mut Dict::new(&[])).context("opening h264_videotoolbox")?;

            enc
        } else if hw_frames.is_null() {
            let codec = unsafe { ffi::avcodec_find_encoder_by_name(c"libx264".as_ptr()) };
            anyhow::ensure!(!codec.is_null(), "no libx264 encoder in this build");
            let mut enc = Codec::alloc(codec)?;
            setup(&mut enc);
            enc.get_mut().thread_count = 0;

            let level = format!("{}.{}", level / 10, level % 10);

            enc.open(&mut Dict::new(&[("preset", "medium"), ("crf", "22"), ("level", &level)]))
                .context("opening libx264")?;

            enc
        } else {
            let codec = unsafe { ffi::avcodec_find_encoder_by_name(c"h264_vaapi".as_ptr()) };
            anyhow::ensure!(!codec.is_null(), "no h264_vaapi encoder in this build");

            let mut opened = None;

            for (mode, quality) in [("QVBR", 23), ("VBR", 0)] {
                let mut enc = Codec::alloc(codec)?;
                setup(&mut enc);

                unsafe {
                    let e = enc.get_mut();
                    e.hw_frames_ctx = ffi::av_buffer_ref(hw_frames);
                    e.max_b_frames = 0;
                    e.level = level;
                    e.bit_rate = cap * 6 / 10;
                    e.global_quality = quality;
                }

                match enc.open(&mut Dict::new(&[("rc_mode", mode)])) {
                    Ok(()) => {
                        opened = Some(enc);
                        break;
                    },
                    Err(e) => tracing::debug!("h264_vaapi in {mode} mode: {e:#}"),
                }
            }

            opened.ok_or_else(|| anyhow!("the GPU's H.264 encoder won't open"))?
        };

        unsafe {
            let st = mux.stream(0);
            ff::check(ffi::avcodec_parameters_from_context((*st).codecpar, enc.0), "video params")?;
            (*st).time_base = enc.get().time_base;
            (*st).avg_frame_rate = self.rate;
        }

        mux.ready()?;
        Ok(enc)
    }

    fn drain_encoder(&mut self, mux: &mut Mux) -> anyhow::Result<()> {
        let enc = self.enc.as_mut().unwrap();
        let tb = enc.get().time_base;

        while enc.receive_packet(&mut self.out_pkt)? {
            let mut p = self.out_pkt.take();
            p.get_mut().stream_index = 0;
            mux.write(p, tb)?;
        }

        Ok(())
    }
}

fn gpu_format(f: &ffi::AVFrame) -> String {
    let fmt = if f.hw_frames_ctx.is_null() {
        ffi::AV_PIX_FMT_NV12
    } else {
        unsafe { (*((*f.hw_frames_ctx).data as *const ffi::AVHWFramesContext)).sw_format }
    };

    unsafe { ff::opt_str(ffi::av_get_pix_fmt_name(fmt)) }.unwrap_or_else(|| "nv12".into())
}

const RATE: c_int = 48_000;
const AAC_FRAME: c_int = 1024;

struct Sound {
    dec: Codec,
    enc: Codec,
    tb: ffi::AVRational,
    swr: *mut ffi::SwrContext,
    fifo: *mut ffi::AVAudioFifo,
    layout: ffi::AVChannelLayout,
    frame: Frame,
    out_pkt: Packet,
    start: f64,
    end: f64,

    skip: Option<i64>,

    written: i64,
    total: i64,
    done: bool,
}

unsafe impl Send for Sound {}

impl Drop for Sound {
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

impl Sound {
    fn new(st: &ffi::AVStream, start: f64, end: f64, mux: &mut Mux) -> anyhow::Result<Self> {
        let par = unsafe { &*st.codecpar };
        let decoder = unsafe { ffi::avcodec_find_decoder(par.codec_id) };
        anyhow::ensure!(!decoder.is_null(), "no decoder for this audio codec");
        let mut dec = Codec::alloc(decoder)?;

        unsafe {
            ff::check(ffi::avcodec_parameters_to_context(dec.0, st.codecpar), "decoder params")?;
            dec.get_mut().pkt_timebase = st.time_base;
        }

        dec.open(&mut Dict::new(&[]))?;

        let mut enc = Codec::alloc(unsafe { ffi::avcodec_find_encoder(ffi::AV_CODEC_ID_AAC) })?;
        let mut layout: ffi::AVChannelLayout = unsafe { std::mem::zeroed() };

        unsafe {
            ffi::av_channel_layout_default(&mut layout, 2);
            let e = enc.get_mut();
            ff::check(ffi::av_channel_layout_copy(&mut e.ch_layout, &layout), "channel layout")?;
            e.sample_rate = RATE;
            e.sample_fmt = ffi::AV_SAMPLE_FMT_FLTP;
            e.bit_rate = AUDIO_BITRATE;
            e.profile = ffi::AV_PROFILE_AAC_LOW as c_int;
            e.time_base = ff::q(1, RATE);
            e.flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;
        }

        enc.open(&mut Dict::new(&[]))?;

        unsafe {
            let st = mux.stream(1);
            ff::check(ffi::avcodec_parameters_from_context((*st).codecpar, enc.0), "audio params")?;
            (*st).time_base = enc.get().time_base;
        }

        mux.ready()?;
        let fifo = unsafe { ffi::av_audio_fifo_alloc(ffi::AV_SAMPLE_FMT_FLTP, 2, AAC_FRAME * 8) };

        Ok(Self {
            dec,
            enc,
            tb: st.time_base,
            swr: ptr::null_mut(),
            fifo,
            layout,
            frame: Frame::new(),
            out_pkt: Packet::new(),
            start,
            end,
            skip: None,
            written: 0,
            total: ((end - start) * RATE as f64).round() as i64,
            done: false,
        })
    }

    fn packet(&mut self, p: &Packet, mux: &mut Mux) -> anyhow::Result<()> {
        loop {
            let accepted = self.dec.send_packet(Some(p))?;
            self.drain_decoder(mux)?;

            if accepted || self.done {
                return Ok(());
            }
        }
    }

    fn drain_decoder(&mut self, mux: &mut Mux) -> anyhow::Result<()> {
        while !self.done && self.dec.receive_frame(&mut self.frame)? {
            let f = self.frame.get();
            let ts = if f.best_effort_timestamp != ffi::AV_NOPTS_VALUE { f.best_effort_timestamp } else { f.pts };
            let at = (ts != ffi::AV_NOPTS_VALUE).then(|| ts as f64 * ff::q2d(self.tb));
            let length = f.nb_samples as f64 / f.sample_rate.max(1) as f64;

            if self.skip.is_none() && at.is_some_and(|a| a + length < self.start - 0.5) {
                self.frame.unref();
                continue;
            }

            if at.is_some_and(|a| a >= self.end + 0.5) {
                self.done = true;
            }

            if self.skip.is_none() {
                let first = at.unwrap_or(self.start);
                let gap = ((first - self.start) * RATE as f64).round() as i64;

                if gap > 0 {
                    self.silence(gap)?;
                }

                self.skip = Some((-gap).max(0));
            }

            if self.swr.is_null() {
                self.open_resampler()?;
            }

            let mut out = Frame::new();

            unsafe {
                let o = out.get_mut();
                ffi::av_channel_layout_copy(&mut o.ch_layout, &self.layout);
                o.sample_rate = RATE;
                o.format = ffi::AV_SAMPLE_FMT_FLTP;
                ff::check(ffi::swr_convert_frame(self.swr, out.0, self.frame.0), "resample")?;
            }

            self.frame.unref();
            self.push(&out)?;
            self.encode(false, mux)?;
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
                    RATE,
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

    fn silence(&mut self, samples: i64) -> anyhow::Result<()> {
        let mut f = Frame::new();

        unsafe {
            let fr = f.get_mut();
            fr.nb_samples = samples.min(RATE as i64 * 10) as c_int;
            fr.format = ffi::AV_SAMPLE_FMT_FLTP;
            ffi::av_channel_layout_copy(&mut fr.ch_layout, &self.layout);
            ff::check(ffi::av_frame_get_buffer(f.0, 0), "audio buffer")?;
            let fr = f.get_mut();
            ffi::av_samples_set_silence(fr.extended_data, 0, fr.nb_samples, 2, ffi::AV_SAMPLE_FMT_FLTP);
        }

        self.write_fifo(&f, 0)
    }

    fn push(&mut self, out: &Frame) -> anyhow::Result<()> {
        let n = out.get().nb_samples as i64;
        let skip = self.skip.unwrap_or(0);
        let drop = skip.min(n);
        self.skip = Some(skip - drop);
        if drop < n { self.write_fifo(out, drop as c_int) } else { Ok(()) }
    }

    fn write_fifo(&mut self, f: &Frame, offset: c_int) -> anyhow::Result<()> {
        let f = f.get();

        let planes = [unsafe { (*f.extended_data).add(offset as usize * 4) } as *mut std::ffi::c_void, unsafe {
            (*f.extended_data.add(1)).add(offset as usize * 4)
        }
            as *mut std::ffi::c_void];

        ff::check(
            unsafe { ffi::av_audio_fifo_write(self.fifo, planes.as_ptr(), f.nb_samples - offset) },
            "audio fifo",
        )?;

        Ok(())
    }

    fn encode(&mut self, flush: bool, mux: &mut Mux) -> anyhow::Result<()> {
        loop {
            let left = self.total - self.written;
            let available = unsafe { ffi::av_audio_fifo_size(self.fifo) } as i64;

            if left <= 0 {
                self.done = true;
                return Ok(());
            }

            let n = available.min(AAC_FRAME as i64).min(left);

            if n < (AAC_FRAME as i64).min(left) && !(flush && n > 0) {
                return Ok(());
            }

            let mut frame = Frame::new();

            unsafe {
                let f = frame.get_mut();
                f.nb_samples = AAC_FRAME;
                f.format = ffi::AV_SAMPLE_FMT_FLTP;
                f.sample_rate = RATE;
                ffi::av_channel_layout_copy(&mut f.ch_layout, &self.layout);
                ff::check(ffi::av_frame_get_buffer(frame.0, 0), "audio buffer")?;
                let f = frame.get_mut();
                ffi::av_audio_fifo_read(self.fifo, f.extended_data as *const *mut std::ffi::c_void, n as c_int);

                if n < AAC_FRAME as i64 {
                    ffi::av_samples_set_silence(
                        f.extended_data,
                        n as c_int,
                        AAC_FRAME - n as c_int,
                        2,
                        ffi::AV_SAMPLE_FMT_FLTP,
                    );
                }

                f.pts = self.written;
            }

            self.written += n;
            self.enc.send_frame(Some(&frame))?;
            self.drain_encoder(mux)?;
        }
    }

    fn drain_encoder(&mut self, mux: &mut Mux) -> anyhow::Result<()> {
        let tb = self.enc.get().time_base;

        while self.enc.receive_packet(&mut self.out_pkt)? {
            let mut p = self.out_pkt.take();
            p.get_mut().stream_index = 1;
            mux.write(p, tb)?;
        }

        Ok(())
    }

    fn finish(&mut self, mux: &mut Mux) -> anyhow::Result<()> {
        if !self.done {
            self.dec.send_packet(None)?;
            self.drain_decoder(mux)?;
        }

        if !self.swr.is_null() {
            let mut out = Frame::new();

            unsafe {
                let o = out.get_mut();
                ffi::av_channel_layout_copy(&mut o.ch_layout, &self.layout);
                o.sample_rate = RATE;
                o.format = ffi::AV_SAMPLE_FMT_FLTP;
                ff::check(ffi::swr_convert_frame(self.swr, out.0, ptr::null()), "resample")?;
            }

            self.push(&out)?;
        }

        let missing = self.total - self.written - unsafe { ffi::av_audio_fifo_size(self.fifo) } as i64;

        if self.skip.is_some() && missing > 0 {
            self.silence(missing)?;
        }

        self.encode(true, mux)?;
        self.enc.send_frame(None)?;
        self.drain_encoder(mux)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rates() {
        let r = |n, d| ff::q(n, d);
        assert_eq!(ff::q2d(frame_rate(r(24000, 1001), false)), 24000.0 / 1001.0);
        assert_eq!(ff::q2d(frame_rate(r(24000, 1001), true)), 24000.0 / 1001.0);
        assert_eq!(ff::q2d(frame_rate(r(60000, 1001), true)), 30000.0 / 1001.0);
        assert_eq!(ff::q2d(frame_rate(r(120, 1), false)), 60.0);
        assert_eq!(ff::q2d(frame_rate(r(120, 1), true)), 30.0);
        assert_eq!(ff::q2d(frame_rate(r(25, 1), true)), 25.0);
    }

    #[test]
    fn fits_into_16_9() {
        assert_eq!(fit(1920, 1080, ff::q(1, 1), 1080), (1920, 1080));
        assert_eq!(fit(3840, 2160, ff::q(1, 1), 720), (1280, 720));
        assert_eq!(fit(1920, 800, ff::q(1, 1), 720), (1280, 534));

        assert_eq!(fit(720, 480, ff::q(32, 27), 1080), (854, 480));
        assert_eq!(fit(1280, 720, ff::q(1, 1), 1080), (1280, 720));
    }
}

#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore]
    fn live() {
        let video = std::path::PathBuf::from(std::env::var("TS_CLIP_TEST").expect("set TS_CLIP_TEST"));
        let at: f64 = std::env::var("TS_CLIP_AT").ok().and_then(|a| a.parse().ok()).unwrap_or(300.0);
        let cache = std::env::temp_dir().join("ts-clip-test");
        let info = super::super::probe::probe(&video).unwrap();

        let subtitles = info.subtitles.iter().find(|s| s.supported).map(|s| burn::Source::Text {
            ass: super::super::subs::load(&video, &s.id, &cache).unwrap(),
            fonts: info.fonts.iter().map(|f| super::super::probe::attachment(&video, f.index).unwrap()).collect(),
        });

        let subtitles = subtitles.or_else(|| {
            let s = info.subtitles.first()?;
            Some(burn::Source::Bitmap { stream: s.id.trim_start_matches('s').parse().ok()? })
        });

        let hw = super::super::hw::Hw::new();

        if std::env::var_os("TS_CLIP_SOFTWARE").is_none() {
            hw.configure(&crate::config::Transcode::default());
        }

        let _ = tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).with_test_writer().try_init();
        println!("{:?}", hw.capabilities());
        let font = cache.join("NotoSans-Regular.ttf");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(&font, include_bytes!("../../assets/fonts/NotoSans-Regular.ttf")).unwrap();
        let fonts = burn::Fonts { dir: None, default: &font };

        for (height, half) in [(1080, false), (720, true)] {
            let recipe = Recipe {
                start: at,
                end: at + 20.0,
                audio: None,
                subtitles: subtitles.clone(),
                height,
                half_rate: half,
                title: "Test clip".into(),
            };

            let out = cache.join(format!("clip-{height}.mp4"));
            let t = std::time::Instant::now();

            let r = render(&video, &recipe, &out, hw.device().as_ref(), &fonts, &mut |_| {}, &AtomicBool::new(false))
                .unwrap();

            println!(
                "{r:?} in {:.1?}: {} ({} KB)",
                t.elapsed(),
                out.display(),
                std::fs::metadata(&out).unwrap().len() / 1024
            );
        }
    }
}
