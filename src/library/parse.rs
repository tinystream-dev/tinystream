// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

static SEASON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:s(?:eason|eries)?[ ._-]*(?P<n>\d{1,3})|(?P<specials>specials?))\b").unwrap()
});

static EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^a-z0-9])s(?P<s>\d{1,3})[ ._]?e(?P<e>\d{1,4})(?:-?e(?P<e2>\d{1,4}))?(?:[^0-9]|$)").unwrap()
});

static TITLE_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<t>.+?)\s*\((?P<y>(?:19|20)\d{2})\)\s*$").unwrap());

static SAMPLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:^|[^a-z])sample(?:[^a-z]|$)").unwrap());

pub const VIDEO_EXTENSIONS: &[&str] =
    &["mkv", "mp4", "m4v", "webm", "mov", "avi", "ts", "m2ts", "mts", "wmv", "flv", "mpg", "mpeg", "ogv"];

const QUIET_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "nfo", "txt", "md", "srt", "ass", "ssa", "vtt", "sub", "idx", "sup", "sfv",
    "nzb", "torrent", "url", "db", "ini", "json", "xml", "ds_store",
];

pub fn season_number(folder_name: &str) -> Option<u32> {
    let caps = SEASON.captures(folder_name)?;

    if caps.name("specials").is_some() {
        return Some(0);
    }

    caps.name("n")?.as_str().parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeNumber {
    pub season: u32,
    pub episode: u32,
    pub episode_end: Option<u32>,
}

pub fn episode_number(file_stem: &str) -> Option<EpisodeNumber> {
    let caps = EPISODE.captures(file_stem)?;
    let season = caps["s"].parse().ok()?;
    let episode = caps["e"].parse().ok()?;
    let episode_end = caps.name("e2").and_then(|m| m.as_str().parse().ok()).filter(|&e2| e2 > episode);
    Some(EpisodeNumber { season, episode, episode_end })
}

pub fn title_and_year(folder_name: &str) -> (String, Option<i64>) {
    match TITLE_YEAR.captures(folder_name) {
        Some(c) => (c["t"].trim().to_string(), c["y"].parse().ok()),
        None => (folder_name.trim().to_string(), None),
    }
}

pub fn sort_title(title: &str) -> String {
    let lower = title.to_lowercase();

    for article in ["the ", "a ", "an "] {
        if let Some(rest) = lower.strip_prefix(article) {
            return rest.to_string();
        }
    }

    lower
}

pub enum FileKind {
    Video,
    Quiet,
    Other,
}

pub fn file_kind(path: &Path) -> FileKind {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();

    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        FileKind::Video
    } else if QUIET_EXTENSIONS.contains(&ext.as_str()) {
        FileKind::Quiet
    } else {
        FileKind::Other
    }
}

pub fn is_sample(name: &str) -> bool {
    SAMPLE.is_match(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasons() {
        assert_eq!(season_number("Season 01"), Some(1));
        assert_eq!(season_number("Season 1"), Some(1));
        assert_eq!(season_number("season.12"), Some(12));
        assert_eq!(season_number("S02"), Some(2));
        assert_eq!(season_number("Season 00"), Some(0));
        assert_eq!(season_number("Specials"), Some(0));
        assert_eq!(season_number("Series 3"), Some(3));
        assert_eq!(season_number("Isekai Quartet S03 1080p WEBRip DD+ x265-EMBER"), Some(3));
        assert_eq!(season_number("Isekai Quartet S01 1080p Dual Audio  BDRip 10 bits AAC x265-EMBER"), Some(1));
        assert_eq!(season_number("Extras"), None);
        assert_eq!(season_number("Subs"), None);
        assert_eq!(season_number("Scans 2"), None);
    }

    #[test]
    fn episodes() {
        let e = episode_number("86 (Eighty-Six) S01E13").unwrap();
        assert_eq!((e.season, e.episode, e.episode_end), (1, 13, None));
        let e = episode_number("[Group] Show - S02E05 [1080p]").unwrap();
        assert_eq!((e.season, e.episode), (2, 5));
        let e = episode_number("show.s01e01e02.1080p").unwrap();
        assert_eq!((e.episode, e.episode_end), (1, Some(2)));
        let e = episode_number("Show S01E01-E02").unwrap();
        assert_eq!(e.episode_end, Some(2));
        let e = episode_number("Show S00E01").unwrap();
        assert_eq!(e.season, 0);
        assert!(episode_number("[Sokudo] Kaguya-sama Love Is War 05 [1080p BD][AV1][dual audio]").is_none());
        assert!(episode_number("Chainsaw Man 01").is_none());
    }

    #[test]
    fn titles() {
        assert_eq!(title_and_year("Interstellar (2014)"), ("Interstellar".into(), Some(2014)));
        assert_eq!(title_and_year("86 (Eighty-Six)"), ("86 (Eighty-Six)".into(), None));
        assert_eq!(sort_title("The Truman Show"), "truman show");
    }

    #[test]
    fn samples() {
        assert!(is_sample("movie-sample.mkv"));
        assert!(is_sample("Sample"));
        assert!(!is_sample("Samples of Life S01E01"));
    }
}
