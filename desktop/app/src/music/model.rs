// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub artists: Vec<Ref>,
    pub album: String,
    pub album_id: Option<i64>,
    pub disc: Option<i64>,
    pub number: Option<i64>,
    pub duration: f64,
    pub codec: String,
    pub suffix: String,
    pub size: u64,
    pub file: String,
    pub flac: String,
    pub sample_rate: Option<u32>,
    pub cover: Option<String>,
    pub cover_tint: Option<String>,
    pub starred: bool,
    pub rating: Option<i64>,
    pub lossless: bool,
    pub bitrate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub channels: Option<u32>,
    pub gains: Gains,
}

/// An artist or owner as a list credits them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Ref {
    pub id: i64,
    #[serde(alias = "username")]
    pub name: String,
}

/// `quality()`: the codec, and depth and rate for lossless files or the bitrate for lossy ones.
pub fn quality(track: &Track) -> String {
    let name = match track.codec.as_str() {
        "pcm" => "PCM".into(),
        "wavpack" => "WavPack".into(),
        "musepack" => "Musepack".into(),
        codec => codec.to_uppercase(),
    };
    match (track.lossless, track.sample_rate, track.bitrate) {
        (true, Some(rate), _) => {
            let khz = rate as f64 / 1000.0;
            let khz = if khz.fract() == 0.0 { format!("{khz}") } else { format!("{khz:.1}") };
            format!("{name} {}/{khz}", track.bit_depth.unwrap_or(16))
        },
        (_, _, Some(bitrate)) => format!("{name} {bitrate}"),
        _ => name,
    }
}

/// `hiRes()`: more than CD's 16 bits or 48 kHz.
pub fn hi_res(track: &Track) -> bool {
    track.lossless && (track.bit_depth.unwrap_or(16) > 16 || track.sample_rate.unwrap_or(44100) > 48000)
}

/// `duration()`: "3:07", or "1:02:07".
pub fn duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    let (h, m, r) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{r:02}") } else { format!("{m}:{r:02}") }
}

/// `length()`: "1 hr 12 min", for whole albums.
pub fn length(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{} sec", seconds.round());
    }
    let m = (seconds / 60.0).round() as u64;
    if m >= 60 { format!("{} hr {} min", m / 60, m % 60) } else { format!("{m} min") }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Gains {
    pub track_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
    pub pending: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Gain {
    Off,
    Track,
    Album,
    #[default]
    Auto,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub volume: f32,
    pub muted: bool,
    pub gain: Gain,
    pub crossfade: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self { volume: 1.0, muted: false, gain: Gain::Auto, crossfade: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub key: String,
    pub track: Track,
    pub fallback: bool,
}

/// Something for the decoder to play.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub key: String,
    pub path: String,
    /// Bytes, or 0 to ask the server.
    pub size: u64,
    pub ext: String,
    /// Linear; ReplayGain with clipping kept off.
    pub gain: f32,
    /// Seconds to overlap with the next track; 0 for none.
    pub crossfade: f64,
}
