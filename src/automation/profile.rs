// SPDX-License-Identifier: AGPL-3.0-or-later

use regex::{Regex, RegexBuilder};
use serde::Serialize;

use super::release::{Attributes, is_movie};
use super::sources::Release;
use crate::config::Profile;

pub struct Rules {
    pub name: String,
    resolutions: Vec<u32>,
    groups: Vec<String>,
    require: Vec<(String, Regex)>,
    reject: Vec<(String, Regex)>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    codecs: Vec<String>,
    prefer_dual_audio: bool,
    pub batches: bool,
    min_seeders: u32,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub accepted: bool,
    pub score: i64,

    pub rejections: Vec<String>,

    pub warnings: Vec<String>,
    pub nonstandard: bool,
}

const EPISODE_WARNING: u64 = 3 << 30;
const MOVIE_WARNING: u64 = 15 << 30;

fn compile(patterns: &[String]) -> Vec<(String, Regex)> {
    patterns
        .iter()
        .filter_map(|p| {
            let source = if p.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-') {
                format!(r"\b{}\b", regex::escape(p))
            } else {
                p.clone()
            };

            RegexBuilder::new(&source).case_insensitive(true).build().ok().map(|r| (p.clone(), r))
        })
        .collect()
}

impl Rules {
    pub fn new(profile: &Profile, pinned_groups: &[String]) -> Self {
        Self {
            name: profile.name.clone(),
            resolutions: profile
                .resolutions
                .iter()
                .filter_map(|r| r.trim().trim_end_matches(['p', 'P']).parse().ok())
                .collect(),
            groups: if pinned_groups.is_empty() { profile.groups.clone() } else { pinned_groups.to_vec() },
            require: compile(&profile.require),
            reject: compile(&profile.reject),
            min_size: profile.min_size,
            max_size: profile.max_size,
            codecs: profile.codecs.iter().map(|c| c.to_ascii_lowercase()).collect(),
            prefer_dual_audio: profile.prefer_dual_audio,
            batches: profile.batches,
            min_seeders: profile.min_seeders,
        }
    }

    pub fn judge(&self, release: &Release, a: &Attributes, episodes: u32, batch: bool) -> Verdict {
        let mut rejections = Vec::new();
        let mut warnings = Vec::new();
        let mut score: i64 = 0;

        match a.resolution {
            Some(r) if !self.resolutions.is_empty() => match self.resolutions.iter().position(|x| *x == r) {
                Some(i) => score += (self.resolutions.len() - i) as i64 * 1000,
                None => rejections.push(format!("{r}p isn't allowed by {}", self.name)),
            },
            Some(r) => score += r as i64,

            None => {},
        }

        if !self.groups.is_empty() {
            let group = a.group.as_deref().unwrap_or("");

            if let Some(i) = self.groups.iter().position(|g| g.eq_ignore_ascii_case(group)) {
                score += (self.groups.len() - i) as i64 * 100;
            }
        }

        for (word, re) in &self.require {
            if !re.is_match(&release.title) {
                rejections.push(format!("doesn't contain “{word}”"));
            }
        }
        for (word, re) in &self.reject {
            if re.is_match(&release.title) {
                rejections.push(format!("contains “{word}”"));
            }
        }

        if let Some(size) = release.size {
            let per_episode = (size / episodes.max(1) as i64) as u64 / 1_000_000;

            if let Some(min) = self.min_size
                && per_episode < min
            {
                rejections.push(format!("{per_episode} MB per episode is under the {min} MB minimum"));
            }

            if let Some(max) = self.max_size
                && per_episode > max
            {
                rejections.push(format!("{per_episode} MB per episode is over the {max} MB maximum"));
            }

            let movie = is_movie(&release.title);
            let limit = if movie { MOVIE_WARNING } else { EPISODE_WARNING };
            let bytes = (size / episodes.max(1) as i64) as u64;

            if self.max_size.is_none() && bytes > limit {
                let gib = bytes as f64 / (1u64 << 30) as f64;

                warnings.push(if movie {
                    format!("{gib:.1} GiB for one movie")
                } else {
                    format!("{gib:.1} GiB per episode")
                });
            }
        }

        if let Some(codec) = a.codec
            && let Some(i) = self
                .codecs
                .iter()
                .position(|c| c == codec || (c == "x265" && codec == "hevc") || (c == "x264" && codec == "h264"))
        {
            score += (self.codecs.len() - i) as i64 * 10;
        }
        if self.prefer_dual_audio && a.dual_audio {
            score += 50;
        }
        score += (a.version as i64 - 1) * 5 + if a.proper { 5 } else { 0 };

        if batch && !self.batches {
            rejections.push("season packs are turned off".into());
        }
        match release.seeders {
            Some(s) if s < self.min_seeders => {
                rejections.push(if s == 0 { "nobody is seeding it".into() } else { format!("only {s} seeding") })
            },
            Some(s) => score += (s.min(200) as i64) / 10,
            None => {},
        }

        Verdict { accepted: rejections.is_empty(), score, rejections, warnings, nonstandard: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::release::attributes;

    fn release(title: &str, seeders: u32) -> Release {
        Release {
            title: title.into(),
            source: "t".into(),
            link: "magnet:?".into(),
            info_hash: None,
            size: Some(1_400_000_000),
            seeders: Some(seeders),
            leechers: None,
            published: None,
            page: None,
        }
    }

    #[test]
    fn prefers_resolution_then_group() {
        let profile = Profile { groups: vec!["SubsPlease".into()], ..Profile::default() };
        let rules = Rules::new(&profile, &[]);
        let judge = |t: &str| rules.judge(&release(t, 50), &attributes(t), 1, false);
        let a = judge("[SubsPlease] Show - 05 (1080p)");
        let b = judge("[Other] Show - 05 (1080p)");
        let c = judge("[SubsPlease] Show - 05 (720p)");
        assert!(a.accepted && b.accepted && c.accepted);
        assert!(a.score > b.score && b.score > c.score);
        assert!(!judge("[SubsPlease] Show - 05 (360p)").accepted);
    }

    #[test]
    fn rejections_explain_themselves() {
        let profile = Profile { reject: vec!["dub".into()], min_seeders: 2, ..Profile::default() };
        let rules = Rules::new(&profile, &[]);
        let v = rules.judge(&release("[G] Show - 05 (1080p) Dub", 1), &attributes("x"), 1, false);
        assert_eq!(v.rejections, vec!["contains “dub”".to_string(), "only 1 seeding".to_string()]);
    }

    #[test]
    fn warns_about_huge_releases() {
        let rules = Rules::new(&Profile::default(), &[]);
        let sized = |t: &str, size: i64| Release { size: Some(size), ..release(t, 50) };
        let judge = |t: &str, size: i64, episodes: u32| rules.judge(&sized(t, size), &attributes(t), episodes, false);
        assert!(judge("[G] Show - 05 (1080p)", 2 << 30, 1).warnings.is_empty());
        assert_eq!(judge("[G] Show - 05 (1080p)", 60 << 30, 1).warnings, vec!["60.0 GiB per episode".to_string()]);
        assert!(judge("[G] Show (01-12) (1080p)", 24 << 30, 12).warnings.is_empty());
        assert!(judge("[G] Show Movie (1080p)", 10 << 30, 1).warnings.is_empty());
        assert!(!judge("[G] Show Movie (1080p)", 20 << 30, 1).warnings.is_empty());

        let capped = Rules::new(&Profile { max_size: Some(100_000), ..Profile::default() }, &[]);
        assert!(capped.judge(&sized("[G] Show - 05", 60 << 30), &attributes("x"), 1, false).warnings.is_empty());
    }
}
