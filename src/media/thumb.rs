// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::c_int;
use std::path::Path;
use std::ptr;

use super::ff::{self, Codec, Dict, FilterGraph, Frame, Input, Packet, ffi};

pub fn capture(path: &Path, width: i32) -> anyhow::Result<Vec<u8>> {
    grab(path, width, None, 3, None)
}

pub fn preview(path: &Path, at: f64, width: i32) -> anyhow::Result<Vec<u8>> {
    grab(path, width, Some(at), 6, None)
}

/// A picture made `width` wide.
pub fn picture(path: &Path, width: i32) -> anyhow::Result<Vec<u8>> {
    grab(path, width, Some(0.0), 3, None)
}

/// A picture squashed to `size`×`size`, as packed RGB.
pub fn pixels(path: &Path, size: i32) -> anyhow::Result<Vec<u8>> {
    let (mut frame, tb) = decode(path, Some(0.0), true)?;
    let mut graph = FilterGraph::video(frame.get(), tb, &format!("scale={size}:{size}:flags=area,format=rgb24"), None)?;
    graph.push(Some(&mut frame))?;
    let mut out = Frame::new();
    anyhow::ensure!(graph.pull(&mut out)?, "scaling failed");
    let f = out.get();
    let row = size as usize * 3;
    let data = unsafe { std::slice::from_raw_parts(f.data[0], f.linesize[0] as usize * size as usize) };
    Ok(data.chunks(f.linesize[0] as usize).flat_map(|line| &line[..row]).copied().collect())
}

pub fn still(
    path: &Path,
    at: f64,
    width: i32,
    draw: &mut dyn FnMut(&mut Frame) -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    grab(path, width, Some(at), 4, Some(draw))
}

pub fn scaled_height(w: i32, h: i32, width: i32) -> i32 {
    ((h as i64 * width as i64 / w.max(1) as i64) as i32) & !1
}

fn grab(
    path: &Path,
    width: i32,
    at: Option<f64>,
    quality: c_int,
    draw: Option<&mut dyn FnMut(&mut Frame) -> anyhow::Result<()>>,
) -> anyhow::Result<Vec<u8>> {
    let (mut frame, tb) = decode(path, at, at.is_some() && draw.is_none())?;
    let f = frame.get();
    let height = scaled_height(f.width, f.height, width);
    let mut graph = FilterGraph::video(f, tb, &format!("scale={width}:{height}:flags=bicubic,format=yuvj420p"), None)?;
    graph.push(Some(&mut frame))?;
    let mut out = Frame::new();
    anyhow::ensure!(graph.pull(&mut out)?, "scaling failed");

    if let Some(draw) = draw {
        draw(&mut out)?;
    }

    encode(&mut out, ffi::AV_CODEC_ID_MJPEG, Some(quality))
}

pub(super) fn decode(path: &Path, at: Option<f64>, keyframe: bool) -> anyhow::Result<(Frame, ffi::AVRational)> {
    let mut input = Input::open(path, true)?;
    let v = unsafe { ffi::av_find_best_stream(input.0, ffi::AVMEDIA_TYPE_VIDEO, -1, -1, ptr::null_mut(), 0) };
    anyhow::ensure!(v >= 0, "no video track");
    let v = v as usize;

    for (i, &st) in input.streams().iter().enumerate() {
        if i != v {
            unsafe { (*st).discard = ffi::AVDISCARD_ALL };
        }
    }

    let st = input.stream(v).unwrap();
    let tb = st.time_base;
    let par = unsafe { &*st.codecpar };

    let duration = unsafe { (*input.0).duration };
    let at = at.unwrap_or(if duration > 0 { duration as f64 / ffi::AV_TIME_BASE as f64 / 3.0 } else { 60.0 });
    let target = (at / ff::q2d(tb)) as i64;

    unsafe {
        ffi::av_seek_frame(input.0, -1, (at * ffi::AV_TIME_BASE as f64) as i64, ffi::AVSEEK_FLAG_BACKWARD as c_int);
    }

    let mut decoder = unsafe { ffi::avcodec_find_decoder(par.codec_id) };

    if par.codec_id == ffi::AV_CODEC_ID_AV1 {
        let d = unsafe { ffi::avcodec_find_decoder_by_name(c"libdav1d".as_ptr()) };

        if !d.is_null() {
            decoder = d;
        }
    }

    anyhow::ensure!(!decoder.is_null(), "no decoder");
    let mut dec = Codec::alloc(decoder)?;

    unsafe {
        ff::check(ffi::avcodec_parameters_to_context(dec.0, st.codecpar), "decoder params")?;
        dec.get_mut().pkt_timebase = tb;
        dec.get_mut().thread_count = 0;
    }

    dec.open(&mut Dict::new(&[]))?;

    let mut pkt = Packet::new();
    let mut frame = Frame::new();
    let mut got = false;

    'read: while input.read(&mut pkt)? {
        if pkt.get().stream_index as usize != v {
            pkt.unref();
            continue;
        }

        dec.send_packet(Some(&pkt))?;
        pkt.unref();

        while dec.receive_frame(&mut frame)? {
            let f = frame.get();
            let pts = if f.best_effort_timestamp != ffi::AV_NOPTS_VALUE { f.best_effort_timestamp } else { f.pts };

            if keyframe || pts == ffi::AV_NOPTS_VALUE || pts >= target {
                got = true;
                break 'read;
            }

            frame.unref();
        }
    }

    if !got {
        dec.send_packet(None)?;
        got = dec.receive_frame(&mut frame)?;
    }

    anyhow::ensure!(got, "no frame found");
    Ok((frame, tb))
}

pub(super) fn encode(frame: &mut Frame, codec: ffi::AVCodecID, quality: Option<c_int>) -> anyhow::Result<Vec<u8>> {
    let mut enc = Codec::alloc(unsafe { ffi::avcodec_find_encoder(codec) })?;

    {
        let e = enc.get_mut();
        e.width = frame.get().width;
        e.height = frame.get().height;
        e.pix_fmt = frame.get().format;
        e.time_base = ff::q(1, 25);

        if let Some(q) = quality {
            e.flags |= ffi::AV_CODEC_FLAG_QSCALE as c_int;
            e.global_quality = q * ffi::FF_QP2LAMBDA as c_int;
        }
    }

    enc.open(&mut Dict::new(&[]))?;

    if let Some(q) = quality {
        frame.get_mut().quality = q * ffi::FF_QP2LAMBDA as c_int;
    }

    frame.get_mut().pts = 0;
    enc.send_frame(Some(frame))?;
    enc.send_frame(None)?;
    let mut image = Packet::new();
    anyhow::ensure!(enc.receive_packet(&mut image)?, "encoding failed");
    Ok(image.data().to_vec())
}
