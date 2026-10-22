// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::ff::{self, Input, Packet, ffi};
use super::probe::{SubtitleTrack, TEXT_SUBTITLE_CODECS};

const SIDECAR_EXTENSIONS: &[&str] = &["ass", "ssa", "srt", "vtt"];

pub fn sidecars(video: &Path) -> Vec<SubtitleTrack> {
    sidecar_files(video)
        .into_iter()
        .enumerate()
        .map(|(n, (path, label))| {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();

            SubtitleTrack {
                id: format!("x{n}"),
                codec: ext,
                language: label.split('.').find(|p| p.len() == 2 || p.len() == 3).map(str::to_string),
                title: Some(if label.is_empty() { "External".into() } else { label.clone() }),
                default: false,
                forced: label.to_lowercase().contains("forced"),
                supported: true,
            }
        })
        .collect()
}

fn sidecar_files(video: &Path) -> Vec<(PathBuf, String)> {
    let (Some(dir), Some(stem)) = (video.parent(), video.file_stem().and_then(|s| s.to_str())) else {
        return Vec::new();
    };

    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };

    let mut out: Vec<(PathBuf, String)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| SIDECAR_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        })
        .filter_map(|p| {
            let s = p.file_stem()?.to_str()?;
            let label = s.strip_prefix(stem)?.trim_start_matches(['.', ' ', '_', '-']).to_string();
            Some((p.clone(), label))
        })
        .collect();

    out.sort();
    out
}

pub fn load(video: &Path, track: &str, cache_dir: &Path) -> anyhow::Result<String> {
    if let Some(n) = track.strip_prefix('x') {
        let n: usize = n.parse()?;

        let (path, _) =
            sidecar_files(video).into_iter().nth(n).ok_or_else(|| anyhow::anyhow!("no such subtitle file"))?;

        let bytes = std::fs::read(&path)?;
        let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)).into_owned();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();

        return Ok(match ext.as_str() {
            "ass" | "ssa" => text,
            _ => cues_to_ass(&text),
        });
    }

    let index: usize = track.strip_prefix('s').ok_or_else(|| anyhow::anyhow!("bad track id"))?.parse()?;
    let cached = cache_path(video, index, cache_dir);

    if let Ok(text) = std::fs::read_to_string(&cached) {
        return Ok(text);
    }

    extract_all(video, cache_dir)?;
    std::fs::read_to_string(&cached).map_err(|_| anyhow::anyhow!("track #{index} isn't a text subtitle track"))
}

pub fn is_cached(video: &Path, index: usize, cache_dir: &Path) -> bool {
    cache_path(video, index, cache_dir).exists()
}

fn cache_path(video: &Path, index: usize, cache_dir: &Path) -> PathBuf {
    use sha2::{Digest, Sha256};

    let mtime = std::fs::metadata(video)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let key = hex::encode(&Sha256::digest(format!("{}\0{mtime}", video.display()))[..12]);
    cache_dir.join("subtitles").join(format!("{key}-{index}.ass"))
}

struct Collected {
    codec: ffi::AVCodecID,
    header: String,
    time_base: ffi::AVRational,
    events: Vec<(i64, String)>,
}

fn extract_all(video: &Path, cache_dir: &Path) -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    let mut input = Input::open(video, false)?;
    let mut tracks: HashMap<usize, Collected> = HashMap::new();

    for (i, &st) in input.streams().iter().enumerate() {
        let st = unsafe { &mut *st };
        let par = unsafe { &*st.codecpar };

        if par.codec_type == ffi::AVMEDIA_TYPE_SUBTITLE && TEXT_SUBTITLE_CODECS.contains(&par.codec_id) {
            let header = if par.extradata.is_null() || par.extradata_size <= 0 {
                String::new()
            } else {
                String::from_utf8_lossy(unsafe {
                    std::slice::from_raw_parts(par.extradata, par.extradata_size as usize)
                })
                .into_owned()
            };

            tracks.insert(i, Collected { codec: par.codec_id, header, time_base: st.time_base, events: Vec::new() });
        } else {
            st.discard = ffi::AVDISCARD_ALL;
        }
    }

    if tracks.is_empty() {
        return Ok(());
    }

    let mut pkt = Packet::new();
    let mut order = 0i64;

    while input.read(&mut pkt)? {
        let p = pkt.get();

        if let Some(t) = tracks.get_mut(&(p.stream_index as usize)) {
            let start = if p.pts == ffi::AV_NOPTS_VALUE { p.dts } else { p.pts };
            let start = start as f64 * ff::q2d(t.time_base);
            let end = start + p.duration as f64 * ff::q2d(t.time_base);
            let data = String::from_utf8_lossy(pkt.data()).into_owned();
            order += 1;

            if let Some(ev) = dialogue(t.codec, &data, start, end, order) {
                t.events.push(ev);
            }
        }

        pkt.unref();
    }

    std::fs::create_dir_all(cache_dir.join("subtitles"))?;

    for (index, t) in tracks {
        let mut events = t.events;
        events.sort_by_key(|e| e.0);

        let mut out = if t.codec == ffi::AV_CODEC_ID_ASS || t.codec == ffi::AV_CODEC_ID_SSA {
            ass_header(&t.header)
        } else {
            DEFAULT_HEADER.to_string()
        };

        for (_, line) in events {
            out.push_str(&line);
            out.push('\n');
        }

        std::fs::write(cache_path(video, index, cache_dir), out)?;
    }

    tracing::debug!("extracted subtitles from {} in {:.1?}", video.display(), started.elapsed());
    Ok(())
}

fn ass_header(extradata: &str) -> String {
    let mut h = extradata.trim_end().to_string();

    if !h.contains("[Events]") {
        h.push_str("\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text");
    }

    h.push('\n');
    h
}

fn dialogue(codec: ffi::AVCodecID, data: &str, start: f64, end: f64, order: i64) -> Option<(i64, String)> {
    let (s, e) = (ass_time(start), ass_time(end));

    if codec == ffi::AV_CODEC_ID_ASS || codec == ffi::AV_CODEC_ID_SSA {
        let mut parts = data.splitn(9, ',');
        let read_order: i64 = parts.next()?.trim().parse().unwrap_or(order);
        let layer = parts.next()?;
        let rest: Vec<&str> = parts.collect();

        if rest.len() != 7 {
            return None;
        }

        return Some((read_order, format!("Dialogue: {layer},{s},{e},{}", rest.join(","))));
    }

    Some((order, format!("Dialogue: 0,{s},{e},Default,,0,0,0,,{}", html_to_ass(data))))
}

fn ass_time(secs: f64) -> String {
    let cs = (secs.max(0.0) * 100.0).round() as u64;
    format!("{}:{:02}:{:02}.{:02}", cs / 360000, cs / 6000 % 60, cs / 100 % 60, cs % 100)
}

fn html_to_ass(text: &str) -> String {
    let mut s = text.trim().replace("\r\n", "\n").replace('\n', "\\N");

    for (from, to) in [
        ("<i>", "{\\i1}"),
        ("</i>", "{\\i0}"),
        ("<b>", "{\\b1}"),
        ("</b>", "{\\b0}"),
        ("<u>", "{\\u1}"),
        ("</u>", "{\\u0}"),
    ] {
        s = s.replace(from, to).replace(&from.to_uppercase(), to);
    }

    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;

    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {},
        }
    }

    out
}

const DEFAULT_HEADER: &str = "[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
ScaledBorderAndShadow: yes
WrapStyle: 0

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,64,&H00FFFFFF,&H000000FF,&H00101010,&H80000000,0,0,0,0,100,100,0,0,1,3,1,2,120,120,64,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
";

fn parse_cue_time(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();

    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };

    Some(h * 3600.0 + m * 60.0 + sec)
}

fn cues_to_ass(text: &str) -> String {
    let mut out = DEFAULT_HEADER.to_string();
    let text = text.replace("\r\n", "\n");

    for block in text.split("\n\n") {
        let mut lines = block.lines().skip_while(|l| !l.contains("-->"));
        let Some(timing) = lines.next() else { continue };
        let mut t = timing.split("-->");
        let (Some(a), Some(b)) = (t.next(), t.next()) else { continue };
        let b = b.split_whitespace().next().unwrap_or_default();
        let (Some(start), Some(end)) = (parse_cue_time(a), parse_cue_time(b)) else { continue };
        let body: Vec<&str> = lines.collect();

        let _ = writeln!(
            out,
            "Dialogue: 0,{},{},Default,,0,0,0,,{}",
            ass_time(start),
            ass_time(end),
            html_to_ass(&body.join("\n"))
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt() {
        let ass = cues_to_ass(
            "1\n00:00:01,000 --> 00:00:02,500\n<i>Hello</i>\nthere\n\n2\n00:01:00,000 --> 00:01:01,000\nBye\n",
        );

        assert!(ass.contains("Dialogue: 0,0:00:01.00,0:00:02.50,Default,,0,0,0,,{\\i1}Hello{\\i0}\\Nthere"));
        assert!(ass.contains("0:01:00.00,0:01:01.00"));
    }

    #[test]
    fn matroska_ass_packet() {
        let (order, line) = dialogue(ffi::AV_CODEC_ID_ASS, "12,0,Default,,0,0,0,,Hi, you", 1.0, 2.0, 0).unwrap();
        assert_eq!(order, 12);
        assert_eq!(line, "Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hi, you");
    }
}
