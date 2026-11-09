// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: Option<i64>,
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

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Gain {
    Off,
    Track,
    Album,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

pub fn gain(index: usize, queue: &[Entry], settings: &Settings, shuffled: bool) -> f32 {
    let Some(entry) = queue.get(index) else { return 1.0 };
    let same_album = |other: Option<&Entry>| {
        !shuffled && entry.track.album_id.is_some() && other.is_some_and(|e| e.track.album_id == entry.track.album_id)
    };
    let mode = match settings.gain {
        Gain::Auto
            if same_album(index.checked_sub(1).and_then(|i| queue.get(i))) || same_album(queue.get(index + 1)) =>
        {
            Gain::Album
        },
        Gain::Auto => Gain::Track,
        other => other,
    };
    let g = &entry.track.gains;
    let (db, peak) = match mode {
        Gain::Off => return 1.0,
        Gain::Album => (g.album_gain.or(g.track_gain), g.album_peak.or(g.track_peak)),
        _ => (g.track_gain, g.track_peak),
    };
    let mut value = 10.0_f64.powf(db.unwrap_or(0.0) / 20.0);
    if let Some(peak) = peak.filter(|p| *p > 0.0) {
        value = value.min(1.0 / peak);
    }
    if value.is_finite() { value.max(0.0) as f32 } else { 1.0 }
}

pub fn fade(index: usize, queue: &[Entry], settings: &Settings) -> f64 {
    if queue.get(index + 1).is_none() {
        return 0.0;
    }
    let same = queue.get(index).zip(queue.get(index + 1)).is_some_and(|(a, b)| {
        a.track.album_id.is_some()
            && a.track.album_id == b.track.album_id
            && a.track.number.zip(b.track.number).is_some_and(|(a, b)| b == a + 1)
    });
    if same { 0.0 } else { settings.crossfade.clamp(0.0, 12.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(number: i64) -> Entry {
        Entry {
            key: number.to_string(),
            fallback: false,
            track: Track {
                number: Some(number),
                album_id: Some(1),
                gains: Gains { track_gain: Some(6.0), album_gain: Some(-6.0), ..Gains::default() },
                ..Track::default()
            },
        }
    }
    #[test]
    fn album_gain_and_fade_follow_the_shared_player() {
        let mut queue = vec![entry(1), entry(2)];
        let settings = Settings { crossfade: 5.0, ..Settings::default() };
        assert!((gain(0, &queue, &settings, false) - 0.5011872).abs() < 0.0001);
        assert!(gain(0, &queue, &settings, true) > 1.9);
        assert_eq!(fade(0, &queue, &settings), 0.0);
        queue[1].track.album_id = Some(2);
        assert_eq!(fade(0, &queue, &settings), 5.0);
        queue[0].track.gains.track_peak = Some(0.8);
        assert_eq!(gain(0, &queue, &settings, false), 1.25);
    }
}
