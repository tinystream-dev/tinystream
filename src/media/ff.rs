// SPDX-License-Identifier: AGPL-3.0-or-later

#![allow(clippy::missing_safety_doc)]

use std::ffi::{CStr, CString, c_int, c_void};
use std::path::Path;
use std::ptr;

use anyhow::{anyhow, bail};
pub use ffmpeg_sys as ffi;

pub fn err_str(code: c_int) -> String {
    let mut buf = [0 as std::ffi::c_char; 256];

    unsafe {
        ffi::av_strerror(code, buf.as_mut_ptr(), buf.len());
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    }
}

pub fn check(code: c_int, what: &str) -> anyhow::Result<c_int> {
    if code < 0 { Err(anyhow!("{what}: {}", err_str(code))) } else { Ok(code) }
}

pub const EAGAIN: c_int = ffi::AVERROR(ffi::EAGAIN);
pub const EOF: c_int = ffi::AVERROR_EOF;

pub fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

pub unsafe fn opt_str(p: *const std::ffi::c_char) -> Option<String> {
    if p.is_null() { None } else { Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()) }
}

pub fn q(num: c_int, den: c_int) -> ffi::AVRational {
    ffi::AVRational { num, den }
}

pub fn q2d(r: ffi::AVRational) -> f64 {
    if r.den == 0 { 0.0 } else { r.num as f64 / r.den as f64 }
}

pub unsafe fn dict_get(dict: *const ffi::AVDictionary, key: &str) -> Option<String> {
    if dict.is_null() {
        return None;
    }

    let key = cstr(key);
    let entry = unsafe { ffi::av_dict_get(dict, key.as_ptr(), ptr::null(), 0) };
    if entry.is_null() { None } else { unsafe { opt_str((*entry).value) } }
}

pub struct Dict(pub *mut ffi::AVDictionary);

impl Dict {
    pub fn new(pairs: &[(&str, &str)]) -> Self {
        let mut d = Dict(ptr::null_mut());

        for (k, v) in pairs {
            d.set(k, v);
        }

        d
    }

    pub fn set(&mut self, k: &str, v: &str) {
        let (k, v) = (cstr(k), cstr(v));
        unsafe { ffi::av_dict_set(&mut self.0, k.as_ptr(), v.as_ptr(), 0) };
    }
}

impl Drop for Dict {
    fn drop(&mut self) {
        unsafe { ffi::av_dict_free(&mut self.0) }
    }
}

pub struct Input(pub *mut ffi::AVFormatContext);
unsafe impl Send for Input {}

impl Input {
    pub fn open(path: &Path, probe: bool) -> anyhow::Result<Self> {
        let c = cstr(&path.to_string_lossy());
        let mut ctx = ptr::null_mut();

        check(
            unsafe { ffi::avformat_open_input(&mut ctx, c.as_ptr(), ptr::null(), ptr::null_mut()) },
            &format!("can't open {}", path.display()),
        )?;

        let input = Input(ctx);

        if probe {
            check(unsafe { ffi::avformat_find_stream_info(ctx, ptr::null_mut()) }, "can't read stream info")?;
        }

        Ok(input)
    }

    pub fn streams(&self) -> &[*mut ffi::AVStream] {
        unsafe {
            let ctx = &*self.0;
            if ctx.streams.is_null() { &[] } else { std::slice::from_raw_parts(ctx.streams, ctx.nb_streams as usize) }
        }
    }

    pub fn stream(&self, i: usize) -> Option<&ffi::AVStream> {
        self.streams().get(i).map(|s| unsafe { &**s })
    }

    pub fn chapters(&self) -> &[*mut ffi::AVChapter] {
        unsafe {
            let ctx = &*self.0;

            if ctx.chapters.is_null() {
                &[]
            } else {
                std::slice::from_raw_parts(ctx.chapters, ctx.nb_chapters as usize)
            }
        }
    }

    pub fn read(&mut self, pkt: &mut Packet) -> anyhow::Result<bool> {
        let r = unsafe { ffi::av_read_frame(self.0, pkt.0) };

        if r == EOF {
            return Ok(false);
        }

        check(r, "read error")?;
        Ok(true)
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        unsafe { ffi::avformat_close_input(&mut self.0) }
    }
}

pub struct Packet(pub *mut ffi::AVPacket);
unsafe impl Send for Packet {}

impl Packet {
    pub fn new() -> Self {
        Packet(unsafe { ffi::av_packet_alloc() })
    }

    pub fn get(&self) -> &ffi::AVPacket {
        unsafe { &*self.0 }
    }

    pub fn get_mut(&mut self) -> &mut ffi::AVPacket {
        unsafe { &mut *self.0 }
    }

    pub fn unref(&mut self) {
        unsafe { ffi::av_packet_unref(self.0) }
    }

    pub fn take(&mut self) -> Packet {
        let p = Packet::new();
        unsafe { ffi::av_packet_move_ref(p.0, self.0) };
        p
    }

    pub fn data(&self) -> &[u8] {
        let p = self.get();

        if p.data.is_null() || p.size <= 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(p.data, p.size as usize) }
        }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe { ffi::av_packet_free(&mut self.0) }
    }
}

pub struct Frame(pub *mut ffi::AVFrame);
unsafe impl Send for Frame {}

impl Frame {
    pub fn new() -> Self {
        Frame(unsafe { ffi::av_frame_alloc() })
    }

    pub fn get(&self) -> &ffi::AVFrame {
        unsafe { &*self.0 }
    }

    pub fn get_mut(&mut self) -> &mut ffi::AVFrame {
        unsafe { &mut *self.0 }
    }

    pub fn unref(&mut self) {
        unsafe { ffi::av_frame_unref(self.0) }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe { ffi::av_frame_free(&mut self.0) }
    }
}

pub struct Codec(pub *mut ffi::AVCodecContext);
unsafe impl Send for Codec {}

impl Codec {
    pub fn alloc(codec: *const ffi::AVCodec) -> anyhow::Result<Self> {
        let ctx = unsafe { ffi::avcodec_alloc_context3(codec) };

        if ctx.is_null() {
            bail!("out of memory");
        }

        Ok(Codec(ctx))
    }

    pub fn get(&self) -> &ffi::AVCodecContext {
        unsafe { &*self.0 }
    }

    pub fn get_mut(&mut self) -> &mut ffi::AVCodecContext {
        unsafe { &mut *self.0 }
    }

    pub fn open(&mut self, opts: &mut Dict) -> anyhow::Result<()> {
        let codec = self.get().codec;
        check(unsafe { ffi::avcodec_open2(self.0, codec, &mut opts.0) }, "can't open codec")?;
        Ok(())
    }

    pub fn send_packet(&mut self, pkt: Option<&Packet>) -> anyhow::Result<bool> {
        let r = unsafe { ffi::avcodec_send_packet(self.0, pkt.map_or(ptr::null(), |p| p.0)) };

        if r == EAGAIN {
            return Ok(false);
        }

        if r == EOF {
            return Ok(true);
        }

        check(r, "decode error")?;
        Ok(true)
    }

    pub fn receive_frame(&mut self, frame: &mut Frame) -> anyhow::Result<bool> {
        let r = unsafe { ffi::avcodec_receive_frame(self.0, frame.0) };

        if r == EAGAIN || r == EOF {
            return Ok(false);
        }

        check(r, "decode error")?;
        Ok(true)
    }

    pub fn send_frame(&mut self, frame: Option<&Frame>) -> anyhow::Result<()> {
        let r = unsafe { ffi::avcodec_send_frame(self.0, frame.map_or(ptr::null(), |f| f.0)) };

        if r == EOF {
            return Ok(());
        }

        check(r, "encode error")?;
        Ok(())
    }

    pub fn receive_packet(&mut self, pkt: &mut Packet) -> anyhow::Result<bool> {
        let r = unsafe { ffi::avcodec_receive_packet(self.0, pkt.0) };

        if r == EAGAIN || r == EOF {
            return Ok(false);
        }

        check(r, "encode error")?;
        Ok(true)
    }
}

impl Drop for Codec {
    fn drop(&mut self) {
        unsafe { ffi::avcodec_free_context(&mut self.0) }
    }
}

pub struct BufferRef(pub *mut ffi::AVBufferRef);
unsafe impl Send for BufferRef {}
unsafe impl Sync for BufferRef {}

impl BufferRef {
    pub fn new_ref(&self) -> *mut ffi::AVBufferRef {
        unsafe { ffi::av_buffer_ref(self.0) }
    }
}

impl Clone for BufferRef {
    fn clone(&self) -> Self {
        BufferRef(self.new_ref())
    }
}

impl Drop for BufferRef {
    fn drop(&mut self) {
        unsafe { ffi::av_buffer_unref(&mut self.0) }
    }
}

pub struct FilterGraph {
    pub graph: *mut ffi::AVFilterGraph,
    pub src: *mut ffi::AVFilterContext,
    pub sink: *mut ffi::AVFilterContext,
}
unsafe impl Send for FilterGraph {}

impl Drop for FilterGraph {
    fn drop(&mut self) {
        unsafe { ffi::avfilter_graph_free(&mut self.graph) }
    }
}

impl FilterGraph {
    pub fn video(
        frame: &ffi::AVFrame,
        time_base: ffi::AVRational,
        desc: &str,
        hw_device: Option<&BufferRef>,
    ) -> anyhow::Result<Self> {
        unsafe {
            let graph = ffi::avfilter_graph_alloc();
            let mut fg = FilterGraph { graph, src: ptr::null_mut(), sink: ptr::null_mut() };
            let sar = if frame.sample_aspect_ratio.num > 0 { frame.sample_aspect_ratio } else { q(1, 1) };

            fg.src =
                ffi::avfilter_graph_alloc_filter(graph, ffi::avfilter_get_by_name(c"buffer".as_ptr()), c"in".as_ptr());

            if fg.src.is_null() {
                bail!("can't create the buffer source");
            }

            let par = ffi::av_buffersrc_parameters_alloc();
            (*par).format = frame.format;
            (*par).width = frame.width;
            (*par).height = frame.height;
            (*par).time_base = time_base;
            (*par).sample_aspect_ratio = sar;
            (*par).hw_frames_ctx = frame.hw_frames_ctx;
            (*par).color_space = frame.colorspace;
            (*par).color_range = frame.color_range;
            let r = ffi::av_buffersrc_parameters_set(fg.src, par);
            ffi::av_free(par as *mut c_void);
            check(r, "buffer source")?;
            check(ffi::avfilter_init_str(fg.src, ptr::null()), "buffer source")?;

            check(
                ffi::avfilter_graph_create_filter(
                    &mut fg.sink,
                    ffi::avfilter_get_by_name(c"buffersink".as_ptr()),
                    c"out".as_ptr(),
                    ptr::null(),
                    ptr::null_mut(),
                    graph,
                ),
                "buffer sink",
            )?;

            let d = cstr(desc);
            let mut seg = ptr::null_mut();
            let mut inputs: *mut ffi::AVFilterInOut = ptr::null_mut();
            let mut outputs: *mut ffi::AVFilterInOut = ptr::null_mut();
            let mut r = ffi::avfilter_graph_segment_parse(graph, d.as_ptr(), 0, &mut seg);

            if r >= 0 {
                r = ffi::avfilter_graph_segment_create_filters(seg, 0);
            }

            if r >= 0 {
                if let Some(dev) = hw_device {
                    for i in 0..(*graph).nb_filters {
                        let f = *(*graph).filters.add(i as usize);

                        if (*f).hw_device_ctx.is_null() {
                            (*f).hw_device_ctx = dev.new_ref();
                        }
                    }
                }

                r = ffi::avfilter_graph_segment_apply_opts(seg, 0);
            }

            if r >= 0 {
                r = ffi::avfilter_graph_segment_init(seg, 0);
            }

            if r >= 0 {
                r = ffi::avfilter_graph_segment_link(seg, 0, &mut inputs, &mut outputs);
            }

            if r >= 0 && (inputs.is_null() || outputs.is_null()) {
                r = ffi::AVERROR(ffi::EINVAL);
            }

            if r >= 0 {
                r = ffi::avfilter_link(fg.src, 0, (*inputs).filter_ctx, (*inputs).pad_idx as u32);
            }

            if r >= 0 {
                r = ffi::avfilter_link((*outputs).filter_ctx, (*outputs).pad_idx as u32, fg.sink, 0);
            }

            ffi::avfilter_graph_segment_free(&mut seg);
            ffi::avfilter_inout_free(&mut inputs);
            ffi::avfilter_inout_free(&mut outputs);
            check(r, &format!("filter `{desc}`"))?;
            check(ffi::avfilter_graph_config(graph, ptr::null_mut()), &format!("filter `{desc}`"))?;
            Ok(fg)
        }
    }

    pub fn push(&mut self, frame: Option<&mut Frame>) -> anyhow::Result<()> {
        let f = frame.map_or(ptr::null_mut(), |f| f.0);
        check(unsafe { ffi::av_buffersrc_add_frame_flags(self.src, f, 0) }, "filter input")?;
        Ok(())
    }

    pub fn pull(&mut self, frame: &mut Frame) -> anyhow::Result<bool> {
        let r = unsafe { ffi::av_buffersink_get_frame(self.sink, frame.0) };

        if r == EAGAIN || r == EOF {
            return Ok(false);
        }

        check(r, "filter output")?;
        Ok(true)
    }
}

pub struct MemOutput {
    pub ctx: *mut ffi::AVFormatContext,
    sink: *mut Vec<u8>,
    header_written: bool,
}
unsafe impl Send for MemOutput {}

unsafe extern "C" fn write_to_vec(opaque: *mut c_void, buf: *const u8, size: c_int) -> c_int {
    let sink = unsafe { &mut *(opaque as *mut Vec<u8>) };
    sink.extend_from_slice(unsafe { std::slice::from_raw_parts(buf, size as usize) });
    size
}

impl MemOutput {
    pub fn new(format: &str) -> anyhow::Result<Self> {
        unsafe {
            let mut ctx = ptr::null_mut();
            let f = cstr(format);

            check(
                ffi::avformat_alloc_output_context2(&mut ctx, ptr::null(), f.as_ptr(), ptr::null()),
                "output format",
            )?;

            let sink = Box::into_raw(Box::new(Vec::<u8>::with_capacity(1 << 20)));
            const SIZE: usize = 1 << 16;
            let buffer = ffi::av_malloc(SIZE) as *mut u8;

            let pb =
                ffi::avio_alloc_context(buffer, SIZE as c_int, 1, sink as *mut c_void, None, Some(write_to_vec), None);

            (*ctx).pb = pb;
            (*ctx).flags |= ffi::AVFMT_FLAG_CUSTOM_IO as c_int;
            Ok(MemOutput { ctx, sink, header_written: false })
        }
    }

    pub fn add_stream(&mut self) -> *mut ffi::AVStream {
        unsafe { ffi::avformat_new_stream(self.ctx, ptr::null()) }
    }

    pub fn stream(&self, i: usize) -> &ffi::AVStream {
        unsafe { &**(*self.ctx).streams.add(i) }
    }

    pub fn write_header(&mut self, opts: &mut Dict) -> anyhow::Result<()> {
        check(unsafe { ffi::avformat_write_header(self.ctx, &mut opts.0) }, "can't start the stream")?;
        self.header_written = true;
        Ok(())
    }

    pub fn header_written(&self) -> bool {
        self.header_written
    }

    pub fn write(&mut self, pkt: &mut Packet) -> anyhow::Result<()> {
        check(unsafe { ffi::av_interleaved_write_frame(self.ctx, pkt.0) }, "mux error")?;
        Ok(())
    }

    pub fn finish(&mut self) -> anyhow::Result<()> {
        if self.header_written {
            check(unsafe { ffi::av_write_trailer(self.ctx) }, "can't finish the stream")?;
        }
        Ok(())
    }

    pub fn take(&mut self) -> Vec<u8> {
        unsafe {
            ffi::avio_flush((*self.ctx).pb);
            std::mem::take(&mut *self.sink)
        }
    }
}

impl Drop for MemOutput {
    fn drop(&mut self) {
        unsafe {
            let mut pb = (*self.ctx).pb;

            if !pb.is_null() {
                ffi::av_freep(&mut (*pb).buffer as *mut *mut u8 as *mut c_void);
                ffi::avio_context_free(&mut pb);
            }

            ffi::avformat_free_context(self.ctx);
            drop(Box::from_raw(self.sink));
        }
    }
}
