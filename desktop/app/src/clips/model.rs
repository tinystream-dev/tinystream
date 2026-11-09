// SPDX-License-Identifier: AGPL-3.0-or-later

//! Clips as the shared `ClipFields` and `ClipAllowanceFields` select them, read the same from every
//! operation, and what the web's `lib/clips.ts` works out from them.

use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Person {
    pub id: i64,
    pub username: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Clip {
    pub id: i64,
    pub kind: String,
    pub screenshot: bool,
    pub name: String,
    pub mine: bool,
    pub can_manage: bool,
    pub owner: Person,
    pub source: Source,
    pub quote: Option<Quote>,
    pub start: f64,
    pub end: f64,
    pub audio: Option<i64>,
    pub subtitles: Option<String>,
    pub quality: Quality,
    pub state: String,
    pub progress: Option<f64>,
    pub error: Option<String>,
    pub bytes: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub rendered_at: Option<i64>,
    pub created_at: i64,
    pub shared_at: Option<i64>,
    pub public: bool,
    pub link: Option<String>,
    pub link_live: bool,
    pub recipients: Vec<Recipient>,
    pub file: String,
    pub poster: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Source {
    pub video: Option<Id>,
    pub title: Option<Id>,
    pub book: Option<Book>,
    pub name: String,
    pub kind: String,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Id {
    pub id: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Book {
    pub id: i64,
    pub title: String,
    pub cover: Option<String>,
    pub cover_tint: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Quote {
    pub text: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Quality {
    pub height: i64,
    pub half_rate: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Recipient {
    pub user: Person,
    pub shared_at: i64,
    pub hidden: bool,
}

/// What you may do with clips, and how much of your space they take.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Allowance {
    pub can_clip: bool,
    pub can_link: bool,
    pub max_length: i64,
    pub bytes: i64,
    pub rendered: i64,
    pub storage: i64,
    pub limit: i64,
    pub custom_default_font: bool,
}

impl Clip {
    pub fn ready(&self) -> bool {
        self.state == "READY"
    }

    /// `clipName`: its own name, or where it's from.
    pub fn display_name(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        if self.quote.is_some() {
            return self.source.name.clone();
        }
        [Some(self.source.name.as_str()), self.source.label.as_deref()]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// `clipSource`: the show and episode, or the movie and its year.
    pub fn source_line(&self) -> String {
        if self.source.kind == "SHOW" || self.quote.is_some() {
            [Some(self.source.name.as_str()), self.source.label.as_deref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        } else {
            match self.source.year {
                Some(year) => format!("{} ({year})", self.source.name),
                None => self.source.name.clone(),
            }
        }
    }
}

/// Image subtitles can be burned in, though the player can't show them as text.
pub const BITMAP_SUBTITLES: [&str; 4] = ["hdmv_pgs_subtitle", "dvd_subtitle", "dvb_subtitle", "xsub"];

/// What a clip can come out as, worked out the same way the server renders it
/// (`src/media/clip.rs`): fitted into a 16:9 box, at the source's own frame rate or half of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub height: i64,
    pub half_rate: bool,
    pub width: i64,
    /// What actually comes out, which can be smaller than the box.
    pub out_height: i64,
    pub fps: f64,
}

impl Preset {
    pub fn label(&self) -> String {
        format!("{}p{}{}", self.out_height, self.fps.round(), if self.half_rate { " · half rate" } else { "" })
    }
}

/// The frame rate a clip gets: the source's, at most 60, halved when asked (and above 30).
pub fn frame_rate(source: f64, half: bool) -> f64 {
    let mut rate = if source > 0.0 { source } else { 24.0 };
    while rate > 61.0 {
        rate /= 2.0;
    }
    if half && rate > 31.0 { rate / 2.0 } else { rate }
}

fn fit(width: f64, height: f64, size: f64) -> (i64, i64) {
    let scale = (size * 16.0 / 9.0 / width).min(size / height).min(1.0);
    let even = |v: f64| (((v / 2.0).round() * 2.0) as i64).max(2);
    (even(width * scale), even(height * scale))
}

/// Every preset a video of this size and rate can be clipped at.
pub fn presets(width: Option<i64>, height: Option<i64>, fps: Option<f64>) -> Vec<Preset> {
    let (width, height, fps) = (width.unwrap_or(1920) as f64, height.unwrap_or(1080) as f64, fps.unwrap_or(24.0));
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for size in [1080, 720, 480] {
        let (w, h) = fit(width, height, size as f64);
        if seen.contains(&h) {
            continue;
        }
        seen.push(h);
        let rates: &[bool] = if frame_rate(fps, false) > 31.0 { &[false, true] } else { &[false] };
        for &half in rates {
            out.push(Preset { height: size, half_rate: half, width: w, out_height: h, fps: frame_rate(fps, half) });
        }
    }
    out
}

/// The ceiling a clip's video stays under, in bits per second.
fn max_bitrate(height: i64, fps: f64) -> f64 {
    let fast = fps > 31.0;
    if height > 720 {
        if fast { 8_000_000.0 } else { 6_000_000.0 }
    } else if height > 480 {
        if fast { 5_000_000.0 } else { 3_500_000.0 }
    } else {
        1_500_000.0
    }
}

/// The most a clip can weigh, in bytes.
pub fn worst_size(preset: &Preset, seconds: f64) -> f64 {
    (max_bitrate(preset.out_height, preset.fps) + 160_000.0) * seconds / 8.0
}

/// "0:42.3": a position with a tenth of a second.
pub fn stamp(seconds: f64) -> String {
    let s = seconds.max(0.0);
    let minutes = (s / 60.0).floor();
    format!("{}:{:0>4}", minutes, format!("{:.1}", s - minutes * 60.0))
}

/// "42 s", "1:05"
pub fn length(seconds: f64) -> String {
    let s = seconds.round() as i64;
    if s < 60 { format!("{s} s") } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// A clip space limit (in MB, as permissions keep it), in the GB people set it in.
pub fn space(mb: i64) -> String {
    let gb = (mb as f64 / 1024.0 * 10.0).round() / 10.0;
    format!("{} GB", crate::player::number(gb))
}

/// Byte counts as the shared `bytes()` writes them.
pub fn bytes(n: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let (mut value, mut unit) = (n as f64, 0);
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if value >= 100.0 || unit == 0 {
        format!("{} {}", value.round(), UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// "3 h 12 min", "45 s" (the shared `duration()`).
fn duration(seconds: f64) -> String {
    let s = seconds.round().max(0.0) as i64;
    let (d, h, m) = (s / 86400, s % 86400 / 3600, s % 3600 / 60);
    match (d, h, m) {
        (0, 0, 0) => format!("{s} s"),
        (0, 0, m) => format!("{m} min"),
        (0, h, 0) => format!("{h} h"),
        (0, h, m) => format!("{h} h {m} min"),
        (d, 0, _) => format!("{d} d"),
        (d, h, _) => format!("{d} d {h} h"),
    }
}

/// "5 min ago", "in 2 d 4 h", "now" (the shared `relative()`).
pub fn relative(at: i64) -> String {
    let diff = (at - crate::browsing::now()) as f64;
    if diff.abs() < 45.0 {
        "now".into()
    } else if diff > 0.0 {
        format!("in {}", duration(diff))
    } else {
        format!("{} ago", duration(-diff))
    }
}

/// A public link made absolute against `origin`.
pub fn absolute(path: &str, origin: &str) -> String {
    format!("{}/{}", origin.trim_end_matches('/'), path.trim_start_matches('/'))
}

/// Languages whose text the built-in clip font doesn't have.
pub const CJK: [&str; 7] = ["jpn", "ja", "chi", "zho", "zh", "kor", "ko"];
