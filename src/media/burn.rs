// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::c_int;
use std::path::Path;
use std::ptr;

use anyhow::{Context, bail};
use rustyass::{Margins, Renderer, Track};

use super::ff::{self, Dict, Frame, Input, Packet, ffi};

#[derive(Clone)]
pub enum Source {
    Text { ass: String, fonts: Vec<(String, Vec<u8>)> },
    Bitmap { stream: usize },
}

pub struct Fonts<'a> {
    pub dir: Option<&'a Path>,
    pub default: &'a Path,
}

pub enum Overlay {
    Text(Box<Ass>),
    Bitmap(Bitmaps),
}

impl Overlay {
    pub fn new(
        src: Source,
        video: &Path,
        fonts: &Fonts,
        size: (i32, i32),
        source: (i32, i32),
        from: f64,
        to: f64,
    ) -> anyhow::Result<Self> {
        Ok(match src {
            Source::Text { ass, fonts: attached } => {
                Overlay::Text(Box::new(Ass::new(&ass, &attached, fonts, size, source)?))
            },
            Source::Bitmap { stream } => Overlay::Bitmap(Bitmaps::read(video, stream, size, source, from, to)?),
        })
    }

    pub fn draw(&mut self, frame: &mut Frame, at: f64) -> anyhow::Result<()> {
        match self {
            Overlay::Text(a) => a.draw(frame, at),
            Overlay::Bitmap(b) => b.draw(frame, at),
        }
    }
}

pub struct Ass {
    renderer: Renderer,
    track: Track,
}

impl Ass {
    fn new(
        text: &str,
        attached: &[(String, Vec<u8>)],
        fonts: &Fonts,
        (w, h): (i32, i32),
        (sw, sh): (i32, i32),
    ) -> anyhow::Result<Self> {
        let track = Track::parse(text.as_bytes()).context("can't read these subtitles")?;
        let mut all = rustyass::Fonts::new();

        for (name, data) in attached {
            if all.add(data.as_slice()) == 0 {
                tracing::debug!("rustyass: no fonts in attachment {name}");
            }
        }

        for (name, data) in track.fonts() {
            if all.add(data) == 0 {
                tracing::debug!("rustyass: no fonts in embedded {name}");
            }
        }

        if let Some(dir) = fonts.dir {
            match all.add_dir(dir) {
                Ok(n) => tracing::debug!("rustyass: {n} fonts in {}", dir.display()),
                Err(e) => tracing::debug!("rustyass: can't read fonts from {}: {e}", dir.display()),
            }
        }

        match std::fs::read(fonts.default) {
            Ok(data) => all.set_default(data),
            Err(e) => tracing::debug!("rustyass: can't read the default font {}: {e}", fonts.default.display()),
        }

        let mut renderer = Renderer::new(all);
        renderer.set_frame((w, h), (sw, sh), Margins::default());
        Ok(Ass { renderer, track })
    }

    fn draw(&mut self, frame: &mut Frame, at: f64) -> anyhow::Result<()> {
        let images = self.renderer.render(&self.track, (at * 1000.0).round() as i64).images;

        if images.is_empty() {
            return Ok(());
        }

        let mut planes = Planes::of(frame)?;

        for i in images {
            let c = i.color();
            let (opacity, mask, stride) = (c.a as u32, i.mask(), i.stride());

            if opacity == 0 {
                continue;
            }

            planes.blend(i.x, i.y, i.w, i.h, |x, y| {
                let a = mask[y as usize * stride + x as usize] as u32 * opacity / 255;
                (c.r, c.g, c.b, a as u8)
            });
        }

        Ok(())
    }
}

struct Picture {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rgba: Vec<u8>,
}

struct Shown {
    from: f64,
    to: f64,
    pictures: Vec<Picture>,
}

pub struct Bitmaps {
    shown: Vec<Shown>,
}

impl Bitmaps {
    fn read(
        video: &Path,
        stream: usize,
        (w, h): (i32, i32),
        (sw, sh): (i32, i32),
        from: f64,
        to: f64,
    ) -> anyhow::Result<Self> {
        let mut input = Input::open(video, true)?;
        let st = input.stream(stream).context("no such subtitle track")?;
        let (tb, par) = (st.time_base, st.codecpar);

        for (i, &s) in input.streams().iter().enumerate() {
            if i != stream {
                unsafe { (*s).discard = ffi::AVDISCARD_ALL };
            }
        }

        let decoder = unsafe { ffi::avcodec_find_decoder((*par).codec_id) };

        if decoder.is_null() {
            bail!("no decoder for this subtitle track");
        }

        let mut dec = ff::Codec::alloc(decoder)?;

        unsafe {
            ff::check(ffi::avcodec_parameters_to_context(dec.0, par), "decoder params")?;
            dec.get_mut().pkt_timebase = tb;
        }

        dec.open(&mut Dict::new(&[]))?;
        let seek = ((from - 30.0).max(0.0) * ffi::AV_TIME_BASE as f64) as i64;
        unsafe { ffi::av_seek_frame(input.0, -1, seek, ffi::AVSEEK_FLAG_BACKWARD as c_int) };

        let mut shown: Vec<Shown> = Vec::new();
        let mut pkt = Packet::new();

        while input.read(&mut pkt)? {
            let p = pkt.get();
            let ts = if p.pts != ffi::AV_NOPTS_VALUE { p.pts } else { p.dts };
            let at = ts as f64 * ff::q2d(tb);

            if at > to + 1.0 {
                break;
            }

            let mut sub: ffi::AVSubtitle = unsafe { std::mem::zeroed() };
            let mut got = 0;
            let r = unsafe { ffi::avcodec_decode_subtitle2(dec.0, &mut sub, &mut got, pkt.0) };
            pkt.unref();

            if r < 0 || got == 0 {
                continue;
            }

            let (cw, ch) = match (dec.get().width, dec.get().height) {
                (cw, ch) if cw > 0 && ch > 0 => (cw, ch),
                _ => (sw, sh),
            };

            let start = at + sub.start_display_time as f64 / 1000.0;

            let end = if sub.end_display_time > sub.start_display_time && sub.end_display_time != u32::MAX {
                at + sub.end_display_time as f64 / 1000.0
            } else {
                f64::INFINITY
            };

            if let Some(last) = shown.last_mut() {
                last.to = last.to.min(start);
            }

            let rects = unsafe { std::slice::from_raw_parts(sub.rects, sub.num_rects as usize) };

            let pictures = rects
                .iter()
                .filter_map(|&r| unsafe { picture(&*r, (w as f64 / cw as f64, h as f64 / ch as f64)) })
                .collect::<Vec<_>>();

            unsafe { ffi::avsubtitle_free(&mut sub) };
            shown.push(Shown { from: start, to: end, pictures });
        }

        shown.retain(|s| !s.pictures.is_empty() && s.to > from && s.from < to);
        Ok(Self { shown })
    }

    fn draw(&mut self, frame: &mut Frame, at: f64) -> anyhow::Result<()> {
        let mut planes = None;

        for s in self.shown.iter().filter(|s| s.from <= at && at < s.to) {
            let planes = match &mut planes {
                Some(p) => p,
                None => planes.insert(Planes::of(frame)?),
            };

            for p in &s.pictures {
                planes.blend(p.x, p.y, p.w, p.h, |x, y| {
                    let i = ((y * p.w + x) * 4) as usize;
                    (p.rgba[i], p.rgba[i + 1], p.rgba[i + 2], p.rgba[i + 3])
                });
            }
        }

        Ok(())
    }
}

unsafe fn picture(r: &ffi::AVSubtitleRect, (sx, sy): (f64, f64)) -> Option<Picture> {
    if r.type_ != ffi::SUBTITLE_BITMAP || r.w <= 0 || r.h <= 0 || r.data[0].is_null() || r.data[1].is_null() {
        return None;
    }

    let palette = unsafe { std::slice::from_raw_parts(r.data[1] as *const u32, 256) };
    let mut rgba = vec![0u8; (r.w * r.h * 4) as usize];

    for y in 0..r.h {
        let row = unsafe { std::slice::from_raw_parts(r.data[0].add((y * r.linesize[0]) as usize), r.w as usize) };

        for (x, &idx) in row.iter().enumerate() {
            let c = palette[idx as usize];
            let o = ((y * r.w) as usize + x) * 4;
            rgba[o..o + 4].copy_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
        }
    }

    let (w, h) = (((r.w as f64 * sx).round() as i32).max(1), ((r.h as f64 * sy).round() as i32).max(1));
    let rgba = if (w, h) == (r.w, r.h) { rgba } else { scale_rgba(&rgba, (r.w, r.h), (w, h))? };
    Some(Picture { x: (r.x as f64 * sx).round() as i32, y: (r.y as f64 * sy).round() as i32, w, h, rgba })
}

fn scale_rgba(src: &[u8], (sw, sh): (i32, i32), (w, h): (i32, i32)) -> Option<Vec<u8>> {
    let mut out = vec![0u8; (w * h * 4) as usize];

    unsafe {
        let ctx = ffi::sws_getContext(
            sw,
            sh,
            ffi::AV_PIX_FMT_RGBA,
            w,
            h,
            ffi::AV_PIX_FMT_RGBA,
            ffi::SWS_BICUBIC as c_int,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
        );

        if ctx.is_null() {
            return None;
        }

        let src_planes = [src.as_ptr(), ptr::null(), ptr::null(), ptr::null()];
        let src_strides = [sw * 4, 0, 0, 0];
        let dst_planes = [out.as_mut_ptr(), ptr::null_mut(), ptr::null_mut(), ptr::null_mut()];
        let dst_strides = [w * 4, 0, 0, 0];

        ffi::sws_scale(
            ctx,
            src_planes.as_ptr(),
            src_strides.as_ptr(),
            0,
            sh,
            dst_planes.as_ptr(),
            dst_strides.as_ptr(),
        );

        ffi::sws_freeContext(ctx);
    }

    Some(out)
}

struct Planes {
    w: i32,
    h: i32,
    y: (*mut u8, isize),
    u: (*mut u8, isize),
    v: (*mut u8, isize),
    step: isize,
}

impl Planes {
    fn of(frame: &mut Frame) -> anyhow::Result<Self> {
        ff::check(unsafe { ffi::av_frame_make_writable(frame.0) }, "frame")?;
        let f = frame.get();
        let plane = |i: usize| (f.data[i], f.linesize[i] as isize);

        Ok(match f.format {
            ffi::AV_PIX_FMT_NV12 => {
                let (uv, s) = plane(1);
                Planes { w: f.width, h: f.height, y: plane(0), u: (uv, s), v: (unsafe { uv.add(1) }, s), step: 2 }
            },
            ffi::AV_PIX_FMT_YUV420P | ffi::AV_PIX_FMT_YUVJ420P => {
                Planes { w: f.width, h: f.height, y: plane(0), u: plane(1), v: plane(2), step: 1 }
            },
            other => bail!("can't draw subtitles on pixel format {other}"),
        })
    }

    fn blend(&mut self, x0: i32, y0: i32, w: i32, h: i32, px: impl Fn(i32, i32) -> (u8, u8, u8, u8)) {
        let (xa, xb) = (x0.max(0), (x0 + w).min(self.w));
        let (ya, yb) = (y0.max(0), (y0 + h).min(self.h));

        if xa >= xb || ya >= yb {
            return;
        }

        let mix = |dst: *mut u8, value: f32, alpha: f32| unsafe {
            let d = *dst as f32;
            *dst = (d + (value - d) * alpha).round().clamp(0.0, 255.0) as u8;
        };

        for y in ya..yb {
            for x in xa..xb {
                let (r, g, b, a) = px(x - x0, y - y0);

                if a == 0 {
                    continue;
                }

                let (luma, _, _) = yuv(r, g, b);
                mix(unsafe { self.y.0.offset(y as isize * self.y.1 + x as isize) }, luma, a as f32 / 255.0);
            }
        }

        for cy in ya / 2..(yb + 1) / 2 {
            for cx in xa / 2..(xb + 1) / 2 {
                let (mut cover, mut u_sum, mut v_sum) = (0.0f32, 0.0f32, 0.0f32);

                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (x, y) = (cx * 2 + dx, cy * 2 + dy);

                    if x < xa || x >= xb || y < ya || y >= yb {
                        continue;
                    }

                    let (r, g, b, a) = px(x - x0, y - y0);
                    let a = a as f32 / 255.0;
                    let (_, u, v) = yuv(r, g, b);
                    cover += a;
                    u_sum += u * a;
                    v_sum += v * a;
                }

                if cover <= 0.0 {
                    continue;
                }

                let o = |p: (*mut u8, isize)| unsafe { p.0.offset(cy as isize * p.1 + cx as isize * self.step) };
                mix(o(self.u), u_sum / cover, cover / 4.0);
                mix(o(self.v), v_sum / cover, cover / 4.0);
            }
        }
    }
}

fn yuv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32, g as f32, b as f32);

    (
        16.0 + 0.1826 * r + 0.6142 * g + 0.0620 * b,
        128.0 - 0.1006 * r - 0.3386 * g + 0.4392 * b,
        128.0 + 0.4392 * r - 0.3989 * g - 0.0403 * b,
    )
}

#[cfg(test)]
mod tests {
    use super::yuv;

    #[test]
    fn bt709_limited() {
        let (y, u, v) = yuv(255, 255, 255);
        assert!((y - 235.0).abs() < 1.0 && (u - 128.0).abs() < 1.0 && (v - 128.0).abs() < 1.0);
        let (y, ..) = yuv(0, 0, 0);
        assert!((y - 16.0).abs() < 0.5);
    }
}
