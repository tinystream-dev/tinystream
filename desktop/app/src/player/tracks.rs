// SPDX-License-Identifier: AGPL-3.0-or-later

//! Which audio and subtitles to play, and how they're named, as the web player picks them
//! (`Player.tsx`, `sync.ts`), with what was picked kept per signed-in user, under the web's
//! `localStorage` keys.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::api::playback::{Playback, PlaybackMediaAudio as Audio, PlaybackMediaSubtitles as Subtitle};

/// What a picked audio track was, to find the same one in other files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioChoice {
    pub language: Option<String>,
    pub title: Option<String>,
    pub channels: Option<i64>,
    pub codec: Option<String>,
}

impl AudioChoice {
    pub fn of(track: &Audio) -> Self {
        Self {
            language: track.language.clone(),
            title: track.title.clone(),
            channels: Some(track.channels),
            codec: Some(track.codec.clone()),
        }
    }
}

/// What a picked subtitle track was, to find the same one in other files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleChoice {
    pub language: Option<String>,
    pub title: Option<String>,
    pub forced: bool,
    pub codec: Option<String>,
}

impl SubtitleChoice {
    pub fn of(track: &Subtitle) -> Self {
        Self {
            language: track.language.clone(),
            title: track.title.clone(),
            forced: track.forced,
            codec: Some(track.codec.clone()),
        }
    }
}

/// The audio track most like the one picked before: same language first, then name, then the rest.
pub fn same_audio<'a>(audio: &'a [Audio], want: &AudioChoice) -> Option<&'a Audio> {
    let score = |a: &Audio| {
        (if a.title == want.title { 4 } else { 0 })
            + (if Some(a.channels) == want.channels { 2 } else { 0 })
            + (if Some(&a.codec) == want.codec.as_ref() { 1 } else { 0 })
    };
    audio
        .iter()
        .filter(|a| match &want.language {
            Some(language) => a.language.as_ref() == Some(language),
            None => want.title.is_some() && a.title == want.title,
        })
        .fold(
            None,
            |best: Option<&Audio>, a| if best.is_none_or(|best| score(a) > score(best)) { Some(a) } else { best },
        )
}

/// The subtitle track most like the one picked before.
pub fn same_subtitle(usable: &[&Subtitle], want: &SubtitleChoice) -> Option<String> {
    let score = |s: &Subtitle| {
        (if s.title == want.title { 4 } else { 0 })
            + (if s.forced == want.forced { 2 } else { 0 })
            + (if Some(&s.codec) == want.codec.as_ref() { 1 } else { 0 })
    };
    usable
        .iter()
        .copied()
        .filter(|s| match &want.language {
            Some(language) => s.language.as_ref() == Some(language),
            None => want.title.is_some() && s.title == want.title,
        })
        .fold(
            None,
            |best: Option<&Subtitle>, s| if best.is_none_or(|best| score(s) > score(best)) { Some(s) } else { best },
        )
        .map(|s| s.id.clone())
}

/// A remembered choice: a track, or `'off'`.
enum Choice<T> {
    Off,
    Track(T),
}

/// What the player remembers between videos, for one user on one server.
#[derive(Debug, Default)]
pub struct Prefs {
    values: BTreeMap<String, String>,
    file: Option<PathBuf>,
}

impl Prefs {
    /// The prefs of user `user` on the saved server `server`.
    pub fn load(server: u64, user: i64) -> Self {
        let file = crate::paths::state_dir().map(|dir| dir.join("players").join(format!("{server:016x}-{user}.json")));
        let values = file
            .as_ref()
            .and_then(|file| std::fs::read(file).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { values, file }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        self.values.insert(key.to_owned(), value.into());
        let Some(file) = &self.file else { return };
        let saved =
            file.parent().map(std::fs::create_dir_all).transpose().and_then(|_| {
                std::fs::write(file, serde_json::to_vec_pretty(&self.values).expect("strings serialize"))
            });
        if let Err(error) = saved {
            tracing::warn!("can't remember player settings: {error}");
        }
    }

    fn choice<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<Choice<T>> {
        match self.get(key)? {
            "off" => Some(Choice::Off),
            value => serde_json::from_str(value).ok().map(Choice::Track),
        }
    }

    pub fn f64(&self, key: &str) -> Option<f64> {
        self.get(key)?.parse().ok().filter(|value: &f64| value.is_finite())
    }
}

/// The audio picked last for this show, or anywhere when it's new.
pub fn default_audio(pb: &Playback, prefs: &Prefs) -> Option<i64> {
    let choice = prefs
        .choice::<AudioChoice>(&format!("audio.{}", pb.title.id))
        .or_else(|| prefs.choice::<AudioChoice>("audioTrack"));
    let language = prefs.get("audioLanguage");
    let audio = &pb.media.audio;
    match choice {
        Some(Choice::Track(choice)) => same_audio(audio, &choice),
        _ => None,
    }
    .or_else(|| language.and_then(|language| audio.iter().find(|a| a.language.as_deref() == Some(language))))
    .or_else(|| audio.iter().find(|a| a.default))
    .map(|a| a.index)
}

/// The subtitles picked last for this show, or anywhere when it's new.
pub fn default_subtitle(pb: &Playback, prefs: &Prefs) -> Option<String> {
    let usable: Vec<&Subtitle> = pb.media.subtitles.iter().filter(|s| usable(s)).collect();
    let choice = prefs
        .choice::<SubtitleChoice>(&format!("subtitles.{}", pb.title.id))
        .or_else(|| prefs.choice::<SubtitleChoice>("subtitleTrack"));
    if usable.is_empty()
        || matches!(choice, Some(Choice::Off))
        || (choice.is_none() && prefs.get("subtitles") == Some("off"))
    {
        return None;
    }
    let language = prefs.get("subtitleLanguage");
    match &choice {
        Some(Choice::Track(choice)) => same_subtitle(&usable, choice),
        _ => None,
    }
    .or_else(|| {
        language.and_then(|language| {
            usable.iter().find(|s| s.language.as_deref() == Some(language) && !s.forced).map(|s| s.id.clone())
        })
    })
    .or_else(|| usable.iter().find(|s| s.default).map(|s| s.id.clone()))
    .or_else(|| usable.iter().find(|s| !s.forced).map(|s| s.id.clone()))
    .or_else(|| Some(usable[0].id.clone()))
}

pub fn bitmap(track: &Subtitle) -> bool {
    track.id.starts_with('s') && matches!(track.codec.as_str(), "hdmv_pgs_subtitle" | "dvd_subtitle" | "pgs" | "vobsub")
}

pub fn usable(track: &Subtitle) -> bool {
    track.supported || bitmap(track)
}

/// Remembers `audio` as picked for this show, and as the one to look for elsewhere.
pub fn remember_audio(pb: &Playback, track: &Audio, prefs: &mut Prefs) {
    if let Some(language) = &track.language {
        prefs.set("audioLanguage", language.clone());
    }
    let choice = serde_json::to_string(&AudioChoice::of(track)).expect("a choice serializes");
    prefs.set(&format!("audio.{}", pb.title.id), choice.clone());
    prefs.set("audioTrack", choice);
}

/// Remembers the subtitles picked (`None` is off).
pub fn remember_subtitle(pb: &Playback, track: Option<&Subtitle>, prefs: &mut Prefs) {
    prefs.set("subtitles", if track.is_some() { "on" } else { "off" });
    if let Some(language) = track.and_then(|t| t.language.clone()) {
        prefs.set("subtitleLanguage", language);
    }
    let choice = match track {
        Some(track) => serde_json::to_string(&SubtitleChoice::of(track)).expect("a choice serializes"),
        None => "off".into(),
    };
    prefs.set(&format!("subtitles.{}", pb.title.id), choice.clone());
    if track.is_some() {
        prefs.set("subtitleTrack", choice);
    }
}

/// A track's name in menus: its language and title, whichever it has (`trackName`).
pub fn track_name(title: Option<&str>, language_code: Option<&str>, fallback: &str) -> String {
    let language = language(language_code);
    match (title.filter(|t| !t.is_empty()), language) {
        (Some(title), Some(language)) if !title.to_lowercase().contains(&language.to_lowercase()) => {
            format!("{language} · {title}")
        },
        (Some(title), _) => title.to_owned(),
        (None, Some(language)) => language.into_owned(),
        (None, None) => fallback.to_owned(),
    }
}

/// A language's English name from its ISO code, or the code itself (`language` in `format.ts`).
pub fn language(code: Option<&str>) -> Option<std::borrow::Cow<'_, str>> {
    let code = code.filter(|code| !code.is_empty())?;
    let name = match code.to_lowercase().as_str() {
        "eng" | "en" => "English",
        "jpn" | "ja" => "Japanese",
        "ger" | "deu" | "de" => "German",
        "fre" | "fra" | "fr" => "French",
        "spa" | "es" => "Spanish",
        "ita" | "it" => "Italian",
        "por" | "pt" => "Portuguese",
        "rus" | "ru" => "Russian",
        "chi" | "zho" | "zh" => "Chinese",
        "kor" | "ko" => "Korean",
        "ara" | "ar" => "Arabic",
        "hun" | "hu" => "Hungarian",
        "pol" | "pl" => "Polish",
        "dut" | "nld" | "nl" => "Dutch",
        "swe" | "sv" => "Swedish",
        "tur" | "tr" => "Turkish",
        _ => return Some(code.into()),
    };
    Some(name.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::playback::*;

    fn audio(index: i64, language: &str, title: Option<&str>, channels: i64, default: bool) -> Audio {
        Audio {
            index,
            codec: "aac".into(),
            codec_string: None,
            channels,
            language: Some(language.into()),
            title: title.map(Into::into),
            default,
        }
    }

    fn subtitle(id: &str, language: &str, forced: bool, default: bool, supported: bool) -> Subtitle {
        Subtitle {
            id: id.into(),
            codec: "ass".into(),
            language: Some(language.into()),
            title: None,
            default,
            forced,
            supported,
        }
    }

    fn playback(audio: Vec<Audio>, subtitles: Vec<Subtitle>) -> Playback {
        Playback {
            id: 1,
            still: String::new(),
            label: None,
            name: None,
            position: None,
            finished: None,
            title: PlaybackTitle { id: 7, kind: TitleKind::SHOW, name: "Show".into(), backdrop: None },
            previous: None,
            next: None,
            media: PlaybackMedia {
                duration: Some(1440.0),
                video: None,
                audio,
                subtitles,
                fonts: vec![],
                chapters: vec![],
            },
        }
    }

    #[test]
    fn audio_follows_what_was_picked_then_language_then_default() {
        let pb = playback(
            vec![
                audio(1, "jpn", None, 2, true),
                audio(2, "eng", Some("Commentary"), 2, false),
                audio(3, "eng", None, 6, false),
            ],
            vec![],
        );
        let mut prefs = Prefs::default();
        assert_eq!(default_audio(&pb, &prefs), Some(1));
        prefs.set("audioLanguage", "eng");
        assert_eq!(default_audio(&pb, &prefs), Some(2));
        remember_audio(&pb, &pb.media.audio[2], &mut prefs);
        assert_eq!(default_audio(&pb, &prefs), Some(3));
        assert_eq!(prefs.get("audio.7"), Some(r#"{"language":"eng","title":null,"channels":6,"codec":"aac"}"#));
    }

    #[test]
    fn subtitles_stay_off_once_turned_off() {
        let pb = playback(
            vec![],
            vec![
                subtitle("s2", "eng", true, false, true),
                subtitle("s3", "eng", false, false, true),
                subtitle("s4", "jpn", false, false, false),
            ],
        );
        let mut prefs = Prefs::default();
        assert_eq!(default_subtitle(&pb, &prefs).as_deref(), Some("s3"));
        remember_subtitle(&pb, None, &mut prefs);
        assert_eq!(default_subtitle(&pb, &prefs), None);
        remember_subtitle(&pb, Some(&pb.media.subtitles[0]), &mut prefs);
        assert_eq!(default_subtitle(&pb, &prefs).as_deref(), Some("s2"));
    }

    #[test]
    fn names_tracks_like_the_web() {
        assert_eq!(track_name(Some("Commentary"), Some("eng"), "Track 2"), "English · Commentary");
        assert_eq!(track_name(Some("English (SDH)"), Some("en"), "Track 2"), "English (SDH)");
        assert_eq!(track_name(None, Some("jpn"), "Track 2"), "Japanese");
        assert_eq!(track_name(None, Some("fin"), "Track 2"), "fin");
        assert_eq!(track_name(None, None, "Track 2"), "Track 2");
    }
}
