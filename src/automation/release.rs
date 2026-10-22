// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize, async_graphql::SimpleObject)]
#[graphql(name = "ReleaseAttributes")]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    pub group: Option<String>,

    pub resolution: Option<u32>,

    pub codec: Option<&'static str>,

    pub source: Option<&'static str>,
    pub dual_audio: bool,

    pub version: u32,
    pub proper: bool,
    pub ten_bit: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Numbers {
    pub season: Option<u32>,

    pub part: Option<u32>,

    pub episodes: Option<(u32, u32)>,

    pub batch: bool,
    pub nonstandard: bool,
}

macro_rules! re {
    ($name:ident, $pattern:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pattern).unwrap());
    };
}

re!(RESOLUTION, r"(?i)\b(2160|1440|1080|720|576|480|360|240)[pi]\b");
re!(RESOLUTION_WXH, r"(?i)\b(?:3840|1920|1280|1024|720)x(2160|1080|720|576|480)\b");
re!(UHD, r"(?i)\b(?:4k|uhd)\b");
re!(HEVC, r"(?i)\b(?:x\.?265|h\.?265|hevc)\b");
re!(AV1, r"(?i)\bav1\b");
re!(H264, r"(?i)\b(?:x\.?264|h\.?264|avc)\b");
re!(WEB, r"(?i)\b(?:web[ ._-]?dl|web[ ._-]?rip|web|amzn|nf|cr|dsnp|hmax|adn)\b");
re!(BLURAY, r"(?i)\b(?:blu[ ._-]?ray|bd[ ._-]?rip|bdremux|bd|bdmv)\b");
re!(DVD, r"(?i)\bdvd(?:[ ._-]?rip)?\b");
re!(HDTV, r"(?i)\b(?:hdtv|tvrip)\b");
re!(DUAL, r"(?i)\b(?:dual|multi)[ ._-]?audio\b");
re!(VERSION, r"(?i)\b\d{1,4}v(\d)\b");
re!(PROPER, r"(?i)\b(?:proper|repack)\b");
re!(TEN_BIT, r"(?i)\b(?:10[ ._-]?bits?|hi10p?)\b");
re!(LEADING_GROUP, r"^\s*[\[【]([^\]】]+)[\]】]");
re!(SCENE_GROUP, r"-([A-Za-z0-9][A-Za-z0-9]*)\s*$");
re!(TRAILING_TAGS, r"(?:\s*\[[^\]]*\]|\s*\([^)]*\))+\s*$");
re!(RECAP, r"(?:^|[\s\-_])\d{1,4}\.5(?:[\s\[\(_]|$)");
re!(SPECIAL, r"(?i)\b(?:ova|oad|specials?)\b");
re!(MOVIE, r"(?i)\b(?:movie|film|gekijouban)\b");

fn strip_extension(title: &str) -> &str {
    if let Some((stem, ext)) = title.rsplit_once('.')
        && (crate::library::parse::VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
            || ext.eq_ignore_ascii_case("torrent"))
    {
        return stem;
    }
    title
}

pub fn attributes(title: &str) -> Attributes {
    let title = strip_extension(title);

    let resolution = RESOLUTION
        .captures(title)
        .or_else(|| RESOLUTION_WXH.captures(title))
        .and_then(|c| c[1].parse().ok())
        .or_else(|| UHD.is_match(title).then_some(2160));

    let codec = if HEVC.is_match(title) {
        Some("hevc")
    } else if AV1.is_match(title) {
        Some("av1")
    } else if H264.is_match(title) {
        Some("h264")
    } else {
        None
    };

    let source = if BLURAY.is_match(title) {
        Some("bluray")
    } else if WEB.is_match(title) {
        Some("web")
    } else if DVD.is_match(title) {
        Some("dvd")
    } else if HDTV.is_match(title) {
        Some("tv")
    } else {
        None
    };

    Attributes {
        group: group(title),
        resolution,
        codec,
        source,
        dual_audio: DUAL.is_match(title),
        version: VERSION.captures(title).and_then(|c| c[1].parse().ok()).unwrap_or(1),
        proper: PROPER.is_match(title),
        ten_bit: TEN_BIT.is_match(title),
    }
}

fn group(title: &str) -> Option<String> {
    if let Some(c) = LEADING_GROUP.captures(title) {
        let g = c[1].trim();

        if !g.is_empty() && !RESOLUTION.is_match(g) && !g.chars().all(|c| c.is_ascii_digit()) {
            return Some(g.to_string());
        }
    }

    let untagged = TRAILING_TAGS.replace(title, "");

    SCENE_GROUP.captures(&untagged).map(|c| c[1].to_string()).filter(|g| {
        !H264.is_match(g) && !HEVC.is_match(g) && !RESOLUTION.is_match(g) && !g.chars().all(|c| c.is_ascii_digit())
    })
}

pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true;

    for c in s.chars() {
        if c == '\'' || c == '’' || c == '`' {
            continue;
        }
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            space = false;
        } else if c == '&' {
            if !space {
                out.push(' ');
            }

            out.push_str("and ");
            space = true;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }

    out.trim_end().to_string()
}

pub fn normalized_name(title: &str) -> String {
    let mut t = strip_extension(title);

    while let Some(m) = LEADING_GROUP.find(t) {
        t = &t[m.end()..];
    }

    normalize(t)
}

pub fn after_title<'a>(name: &'a str, alias: &str) -> Option<&'a str> {
    if alias.is_empty() {
        return None;
    }

    let rest = name.strip_prefix(alias)?;
    if rest.is_empty() { Some(rest) } else { rest.strip_prefix(' ') }
}

pub fn is_recap(title: &str) -> bool {
    RECAP.is_match(strip_extension(title))
}

pub fn is_movie(title: &str) -> bool {
    MOVIE.is_match(strip_extension(title))
}

pub fn media_category(title: &str, episodes: &[(u32, u32)]) -> crate::metadata::MediaCategory {
    use crate::metadata::MediaCategory;

    if !episodes.is_empty() {
        if episodes.iter().all(|e| e.0 == 0) {
            MediaCategory::Specials
        } else if episodes.iter().all(|e| e.0 > 0) {
            MediaCategory::Episodes
        } else {
            MediaCategory::Other
        }
    } else if let Some(ep) = crate::library::parse::episode_number(title) {
        if ep.season == 0 { MediaCategory::Specials } else { MediaCategory::Episodes }
    } else if is_movie(title) {
        MediaCategory::Movies
    } else if SPECIAL.is_match(strip_extension(title)) {
        MediaCategory::Specials
    } else if let Some(season) = crate::library::parse::season_number(title) {
        if season == 0 { MediaCategory::Specials } else { MediaCategory::Episodes }
    } else {
        MediaCategory::Other
    }
}

re!(SXXEYY, r"^s(\d{1,3})e(\d{1,4})(?:v\d)?(?:e(\d{1,4}))?$");
re!(SXX, r"^s(\d{1,3})$");
re!(EXX, r"^(?:e|ep)(\d{1,4})(?:v\d)?$");
re!(NUMBER, r"^(\d{1,4})(?:v\d)?$");
re!(ORDINAL, r"^(\d{1,2})(?:st|nd|rd|th)$");

fn number(token: &str) -> Option<u32> {
    NUMBER.captures(token).and_then(|c| c[1].parse().ok())
}

fn is_year(n: u32) -> bool {
    (1950..=2099).contains(&n)
}

pub fn numbers(rest: &str) -> Option<Numbers> {
    let tokens: Vec<&str> = rest.split(' ').filter(|t| !t.is_empty()).collect();
    let mut n = Numbers::default();
    let mut i = 0;

    while i < tokens.len() {
        let t = tokens[i];
        let next = tokens.get(i + 1).copied();

        if matches!(t, "movie" | "ova" | "oad" | "ona" | "special" | "specials" | "sp" | "recap" | "pv" | "trailer") {
            return None;
        }

        if matches!(t, "batch" | "complete" | "collection") {
            n.batch = true;
        }

        if n.episodes.is_some() {
            i += 1;
            continue;
        }

        if let Some(c) = SXXEYY.captures(t) {
            let e: u32 = c[2].parse().ok()?;
            let mut end = c.get(3).and_then(|m| m.as_str().parse().ok()).filter(|x| *x > e).unwrap_or(e);

            if end == e
                && let Some(x) =
                    next.and_then(|t| EXX.captures(t)).and_then(|c| c[1].parse::<u32>().ok()).filter(|x| *x > e)
            {
                end = x;
                i += 1;
            }

            n.season = c[1].parse().ok();
            n.episodes = Some((e, end));

            if end > e {
                n.batch = true;
            }

            i += 1;
            continue;
        }

        if let Some(c) = SXX.captures(t) {
            n.season = c[1].parse().ok();

            i += 1;
            continue;
        }

        if let Some(c) = ORDINAL.captures(t)
            && matches!(next, Some("season" | "series"))
        {
            n.season = c[1].parse().ok();
            i += 2;
            continue;
        }

        if let Some(c) = ORDINAL.captures(t)
            && next == Some("cour")
        {
            n.part = c[1].parse().ok();
            i += 2;
            continue;
        }

        if let Some(c) = ORDINAL.captures(t) {
            let episode = next.is_some_and(|t| {
                number(t).is_some_and(|e| e > 0 && !is_year(e))
                    || EXX.is_match(t)
                    || (matches!(t, "ep" | "episode" | "e")
                        && tokens.get(i + 2).and_then(|t| number(t)).is_some_and(|e| e > 0))
            });

            let season = c[1].parse().ok()?;

            if !episode || n.season.is_some_and(|s| s != season) {
                return None;
            }

            n.season = Some(season);
            n.nonstandard = true;
            i += 1;
            continue;
        }

        if matches!(t, "part" | "cour")
            && let Some(p) = next.and_then(number).filter(|p| (1..10).contains(p))
        {
            n.part = Some(p);
            i += 2;
            continue;
        }

        if matches!(t, "season" | "series")
            && let Some(s) = next.and_then(number)
        {
            n.season = Some(s);
            i += 2;
            continue;
        }

        if matches!(t, "ep" | "episode" | "e")
            && let Some(e) = next.and_then(number)
        {
            n.episodes = Some((e, e));
            i += 2;
            continue;
        }

        if let Some(c) = EXX.captures(t) {
            let e = c[1].parse().ok()?;
            n.episodes = Some((e, e));
            i += 1;
            continue;
        }

        if let Some(e) = number(t) {
            if is_year(e) && next.and_then(number).is_some() {
                i += 1;
                continue;
            }

            if is_year(e) && t.len() == 4 && next.is_none_or(|x| !x.chars().all(|c| c.is_ascii_digit())) {
                i += 1;
                continue;
            }

            let end = next.and_then(number).filter(|x| *x > e && !is_year(*x));
            n.episodes = Some((e, end.unwrap_or(e)));

            if end.is_some() {
                n.batch = true;
                i += 1;
            }

            i += 1;
            continue;
        }

        i += 1;
    }

    if n.episodes.is_none() && (n.season.is_some() || n.part.is_some()) {
        n.batch = true;
    }

    if n.episodes.is_none() && n.season.is_none() && n.part.is_none() && !n.batch {
        return None;
    }

    Some(n)
}

pub fn parse_size(s: &str) -> Option<i64> {
    let s = s.trim();

    if let Ok(n) = s.parse::<i64>() {
        return Some(n);
    }

    let split = s.find(|c: char| c.is_alphabetic())?;
    let (num, unit) = s.split_at(split);
    let num: f64 = num.trim().replace(',', "").parse().ok()?;

    let mult: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "b" | "bytes" => 1.0,
        "kb" => 1e3,
        "kib" => 1024.0,
        "mb" => 1e6,
        "mib" => 1024.0 * 1024.0,
        "gb" => 1e9,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1e12,
        "tib" => 1024f64.powi(4),
        _ => return None,
    };

    Some((num * mult) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_types() {
        use crate::metadata::MediaCategory::*;
        assert_eq!(media_category("Show", &[(1, 1)]), Episodes);
        assert_eq!(media_category("Show", &[(0, 1)]), Specials);
        assert_eq!(media_category("Show", &[(0, 1), (1, 1)]), Other);
        assert_eq!(media_category("Show Movie", &[]), Movies);
        assert_eq!(media_category("Show OVA", &[]), Specials);
        assert_eq!(media_category("Show OAD", &[]), Specials);
        assert_eq!(media_category("Unknown download", &[]), Other);
        assert_eq!(media_category("Show S00E01.mkv", &[]), Specials);
        assert_eq!(media_category("Show S02E01.mkv", &[]), Episodes);
        assert_eq!(media_category("Show Season 02", &[]), Episodes);
        assert_eq!(media_category("Special Agent S01E01", &[(1, 1)]), Episodes);
    }

    fn after(release: &str, alias: &str) -> Option<Numbers> {
        numbers(after_title(&normalized_name(release), &normalize(alias))?)
    }

    #[test]
    fn fansub_single() {
        let n = after("[SubsPlease] Sousou no Frieren - 05 (1080p) [A1B2C3D4].mkv", "Sousou no Frieren").unwrap();
        assert_eq!(n, Numbers { episodes: Some((5, 5)), ..Numbers::default() });
        let a = attributes("[SubsPlease] Sousou no Frieren - 05 (1080p) [A1B2C3D4].mkv");
        assert_eq!(a.group.as_deref(), Some("SubsPlease"));
        assert_eq!(a.resolution, Some(1080));
    }

    #[test]
    fn explicit_seasons() {
        let n = after(
            "[Erai-raws] Re Zero kara Hajimeru Isekai Seikatsu 3rd Season - 05 [1080p][HEVC]",
            "Re:Zero kara Hajimeru Isekai Seikatsu",
        )
        .unwrap();

        assert_eq!((n.season, n.episodes), (Some(3), Some((5, 5))));
        let n = after("[Group] Show S2 - 05 [720p]", "Show").unwrap();
        assert_eq!((n.season, n.episodes), (Some(2), Some((5, 5))));
        let n = after("Show.S02E05.1080p.WEB.h264-GROUP", "Show").unwrap();
        assert_eq!((n.season, n.episodes), (Some(2), Some((5, 5))));
        let n = after("Show Season 2 Episode 7", "Show").unwrap();
        assert_eq!((n.season, n.episodes), (Some(2), Some((7, 7))));
        let a = attributes("Show.S02E05.1080p.WEB.h264-GROUP");
        assert_eq!((a.group.as_deref(), a.codec, a.source), (Some("GROUP"), Some("h264"), Some("web")));
    }

    #[test]
    fn ordinal_season_without_label() {
        for suffix in ["4th_18", "4th 18", "4TH-18", "4th.E18", "4th Episode 18"] {
            let n = after(&format!("Show {suffix}"), "Show").unwrap();
            assert_eq!((n.season, n.episodes), (Some(4), Some((18, 18))), "{suffix}");
            assert!(n.nonstandard);
        }

        assert!(!after("Show 4th Season 18", "Show").unwrap().nonstandard);

        for suffix in ["4th", "4th unknown 18", "4th 1080p", "4th 2024", "S2 4th 18"] {
            assert!(after(&format!("Show {suffix}"), "Show").is_none(), "{suffix}");
        }
    }

    #[test]
    fn split_seasons() {
        let n = after(
            "[G] Re Zero kara Hajimeru Isekai Seikatsu Season 2 Part 2 - 03 [1080p]",
            "Re:Zero kara Hajimeru Isekai Seikatsu",
        )
        .unwrap();

        assert_eq!((n.season, n.part, n.episodes), (Some(2), Some(2), Some((3, 3))));
        let n = after("[G] Show 2nd Cour - 05 [1080p]", "Show").unwrap();
        assert_eq!((n.season, n.part, n.episodes), (None, Some(2), Some((5, 5))));
        let n = after("[G] Show S2 Part 2 [Batch]", "Show").unwrap();
        assert_eq!((n.season, n.part, n.episodes, n.batch), (Some(2), Some(2), None, true));
    }

    #[test]
    fn batches() {
        let n = after("[Judas] Show (Season 1) [1080p][HEVC x265 10bit][Batch]", "Show").unwrap();
        assert_eq!((n.season, n.episodes, n.batch), (Some(1), None, true));
        let n = after("[Group] Show (01-12) [1080p]", "Show").unwrap();
        assert_eq!((n.episodes, n.batch), (Some((1, 12)), true));
        let n = after("Show S01 1080p BluRay x265-GROUP", "Show").unwrap();
        assert_eq!((n.season, n.batch), (Some(1), true));
        let n = after("Show S01E01-E03 1080p", "Show").unwrap();
        assert_eq!((n.episodes, n.batch), (Some((1, 3)), true));
    }

    #[test]
    fn years_and_long_runners() {
        let n = after("[Group] Show (2023) - 05 [1080p]", "Show").unwrap();
        assert_eq!(n.episodes, Some((5, 5)));
        let n = after("[SubsPlease] One Piece - 1105 (1080p)", "One Piece").unwrap();
        assert_eq!(n.episodes, Some((1105, 1105)));
        let n = after("[Group] Show - 05v2 [1080p]", "Show").unwrap();
        assert_eq!(n.episodes, Some((5, 5)));
        assert_eq!(attributes("[Group] Show - 05v2 [1080p]").version, 2);
    }

    #[test]
    fn not_episodes() {
        assert!(after("[Group] Show Movie [1080p]", "Show").is_none());
        assert!(after("[Group] Show - OVA [1080p]", "Show").is_none());
        assert!(after("[Group] Show 2nd", "Show 2nd Season").is_none());
        assert!(is_recap("[Group] Show - 12.5 (1080p)"));
        assert!(!is_recap("[Group] Show - 12 (1080p) [AAC 2.0]"));
        assert!(!is_recap("Show S01E01 1080p AAC 5.1 x264"));

        assert!(after_title(&normalized_name("[G] Showdown - 01"), "show").is_none());
    }

    #[test]
    fn sizes() {
        assert_eq!(parse_size("1.5 GiB"), Some(1610612736));
        assert_eq!(parse_size("700 MB"), Some(700_000_000));
        assert_eq!(parse_size("1234"), Some(1234));
    }
}
