// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use super::burn::{self, Overlay};
use super::ff::{FilterGraph, Frame, ffi};
use super::thumb;

pub struct Shot {
    pub png: Vec<u8>,
    pub poster: Vec<u8>,
    pub width: i32,
    pub height: i32,
}

const POSTER_WIDTH: i32 = 640;

pub fn take(video: &Path, at: f64, subtitles: Option<burn::Source>, fonts: &burn::Fonts) -> anyhow::Result<Shot> {
    let (mut frame, tb) = thumb::decode(video, Some(at), false)?;
    let f = frame.get();

    let sar = if f.sample_aspect_ratio.num > 0 && f.sample_aspect_ratio.den > 0 {
        f.sample_aspect_ratio.num as f64 / f.sample_aspect_ratio.den as f64
    } else {
        1.0
    };

    let (w, h) = (((f.width as f64 * sar).round() as i32 + 1) & !1, f.height & !1);
    let source = (f.width, f.height);

    let hdr = matches!(f.color_trc, ffi::AVCOL_TRC_SMPTE2084 | ffi::AVCOL_TRC_ARIB_STD_B67);
    let scale = format!("scale={w}:{h}:flags=lanczos,setsar=1");

    let tonemap = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,\
                   zscale=t=bt709:m=bt709:r=tv";

    let mut candidates = vec![format!("{scale},format=yuv420p")];

    if hdr {
        candidates.insert(0, format!("{scale},{tonemap},format=yuv420p"));
    }

    let mut graph = None;

    for desc in &candidates {
        match FilterGraph::video(f, tb, desc, None) {
            Ok(g) => {
                graph = Some(g);
                break;
            },
            Err(e) => tracing::debug!("screenshot filter `{desc}`: {e:#}"),
        }
    }

    let mut graph = graph.ok_or_else(|| anyhow::anyhow!("can't size this frame"))?;
    graph.push(Some(&mut frame))?;
    let mut picture = Frame::new();
    anyhow::ensure!(graph.pull(&mut picture)?, "scaling failed");

    if let Some(src) = subtitles {
        let mut overlay = Overlay::new(src, video, fonts, (w, h), source, at, at + 0.1)?;
        overlay.draw(&mut picture, at)?;
    }

    let png = {
        let mut rgb = convert(&mut picture, tb, "format=rgb24")?;
        thumb::encode(&mut rgb, ffi::AV_CODEC_ID_PNG, None)?
    };

    let poster = {
        let height = thumb::scaled_height(w, h, POSTER_WIDTH);

        let mut small =
            convert(&mut picture, tb, &format!("scale={POSTER_WIDTH}:{height}:flags=bicubic,format=yuvj420p"))?;

        thumb::encode(&mut small, ffi::AV_CODEC_ID_MJPEG, Some(3))?
    };

    Ok(Shot { png, poster, width: w, height: h })
}

fn convert(frame: &mut Frame, tb: ffi::AVRational, desc: &str) -> anyhow::Result<Frame> {
    let mut graph = FilterGraph::video(frame.get(), tb, desc, None)?;
    let mut copy = Frame::new();
    super::ff::check(unsafe { ffi::av_frame_ref(copy.0, frame.0) }, "frame")?;
    graph.push(Some(&mut copy))?;
    let mut out = Frame::new();
    anyhow::ensure!(graph.pull(&mut out)?, "converting failed");
    Ok(out)
}
