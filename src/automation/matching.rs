// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::SqlitePool;

use super::release::{self, Numbers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, async_graphql::Enum)]
#[serde(rename_all = "lowercase")]
pub enum Numbering {
    Auto,
    Seasonal,
    Absolute,
}

impl Numbering {
    pub fn parse(s: &str) -> Self {
        match s {
            "seasonal" => Numbering::Seasonal,
            "absolute" => Numbering::Absolute,
            _ => Numbering::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope {
    pub season: u32,

    pub offset: u32,

    pub episodes: Option<u32>,
}

impl Scope {
    pub fn season(season: u32) -> Self {
        Self { season, offset: 0, episodes: None }
    }

    fn holds(&self, e: u32) -> bool {
        self.offset > 0 && e >= 1 && e <= self.episodes.unwrap_or(self.offset)
    }
}

#[derive(Debug, Clone)]
pub struct SeasonTitles {
    pub scope: Scope,
    pub titles: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Matcher {
    aliases: Vec<(String, Option<Scope>)>,

    listed: Vec<(String, Option<Scope>)>,

    seasons: BTreeMap<u32, Option<u32>>,

    parts: BTreeMap<u32, Vec<Scope>>,

    scheduled: BTreeMap<u32, u32>,
    numbering: Numbering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Match {
    pub episodes: Vec<(u32, u32)>,
    pub batch: bool,
    pub nonstandard: bool,
}

impl Matcher {
    pub fn new(
        show_titles: &[String],
        season_titles: &[SeasonTitles],
        seasons: BTreeMap<u32, Option<u32>>,
        numbering: Numbering,
    ) -> Self {
        let mut aliases: Vec<(String, Option<Scope>)> = Vec::new();

        let mut push = |title: &str, scope: Option<Scope>| {
            let n = release::normalize(title);

            if n.len() >= 2 && !aliases.iter().any(|(a, _)| *a == n) {
                aliases.push((n, scope));
            }
        };

        for t in show_titles {
            push(t, None);
        }

        let mut parts: BTreeMap<u32, Vec<Scope>> = BTreeMap::new();

        for SeasonTitles { scope, titles } in season_titles {
            if scope.offset > 0 {
                parts.entry(scope.season).or_default().push(*scope);
            }
            for t in titles {
                push(t, (scope.season > 1 || scope.offset > 0).then_some(*scope));
            }
        }

        for p in parts.values_mut() {
            p.sort_by_key(|s| s.offset);
        }

        let listed = aliases.clone();
        aliases.sort_by_key(|(a, _)| std::cmp::Reverse(a.len()));
        Self { aliases, listed, seasons, parts, scheduled: BTreeMap::new(), numbering }
    }

    pub async fn load(db: &SqlitePool, series_id: i64) -> anyhow::Result<Self> {
        let (title, path, aliases, known_as, numbering): (String, String, String, String, String) =
            sqlx::query_as("SELECT title, path, aliases, known_as, numbering FROM series WHERE id = ?")
                .bind(series_id)
                .fetch_one(db)
                .await?;

        let folder =
            std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        let (folder_title, _) = crate::library::parse::title_and_year(&folder);
        let mut show: Vec<String> = vec![title, folder_title];
        show.extend(serde_json::from_str::<Vec<String>>(&aliases).unwrap_or_default());
        show.extend(serde_json::from_str::<Vec<String>>(&known_as).unwrap_or_default());

        let rows: Vec<(i64, Option<String>, String, Option<i64>, String)> = sqlx::query_as(
            "SELECT season, title, aliases, episodes, parts FROM series_seasons WHERE series_id = ? ORDER BY season",
        )
        .bind(series_id)
        .fetch_all(db)
        .await?;

        let known: Vec<(i64, i64)> =
            sqlx::query_as("SELECT season, MAX(episode) FROM episodes WHERE series_id = ? GROUP BY season")
                .bind(series_id)
                .fetch_all(db)
                .await?;

        let mut seasons = BTreeMap::new();
        let mut season_titles = Vec::new();

        for (season, t, aliases, episodes, parts) in rows {
            let season = season as u32;
            let mut titles: Vec<String> = serde_json::from_str(&aliases).unwrap_or_default();
            titles.extend(t);
            season_titles.push(SeasonTitles { scope: Scope::season(season), titles });
            let parts: Vec<crate::metadata::PartSchedule> = serde_json::from_str(&parts).unwrap_or_default();

            for p in parts {
                let scope = Scope { season, offset: p.offset as u32, episodes: p.episodes.map(|e| e as u32) };
                season_titles.push(SeasonTitles { scope, titles: p.aliases });
            }

            seasons.insert(season, episodes.map(|e| e as u32));
        }

        for &(season, max) in &known {
            seasons.entry(season as u32).or_insert(Some(max as u32));
        }

        let mut matcher = Self::new(&show, &season_titles, seasons, Numbering::parse(&numbering));
        matcher.scheduled = known.into_iter().map(|(s, max)| (s as u32, max as u32)).collect();
        Ok(matcher)
    }

    pub fn search_titles(&self, season: Option<u32>) -> Vec<String> {
        let ranked = |keep: &dyn Fn(&Option<Scope>) -> bool| -> Vec<String> {
            let mut group: Vec<String> = self.listed.iter().filter(|(_, s)| keep(s)).map(|(a, _)| a.clone()).collect();
            group.sort_by_key(|a| !a.is_ascii());

            let extended =
                |a: &str| self.listed.iter().filter(|(b, _)| b.starts_with(a) && b[a.len()..].starts_with(' ')).count();

            if let Some(core) =
                group.iter().filter(|a| a.is_ascii() && extended(a) > 0).max_by_key(|a| (extended(a), a.len())).cloned()
            {
                group.retain(|a| *a != core);
                group.insert(0, core);
            }

            group
        };

        let scoped = ranked(&|s| season.is_some_and(|x| s.is_some_and(|s| s.season == x)));
        let show = ranked(&|s| s.is_none());

        let mut out: Vec<String> = scoped.iter().take(1).chain(show.iter().take(1)).cloned().collect();

        for t in scoped.into_iter().chain(show) {
            if !out.contains(&t) {
                out.push(t);
            }
        }

        out
    }

    pub fn matches(&self, title: &str) -> Option<Match> {
        if release::is_recap(title) {
            return None;
        }

        let name = release::normalized_name(title);

        for (alias, scope) in &self.aliases {
            let Some(rest) = release::after_title(&name, alias) else { continue };
            let Some(numbers) = release::numbers(rest) else { return None };
            return self.map(*scope, numbers);
        }

        None
    }

    pub fn matches_file(&self, file_name: &str) -> Option<Match> {
        self.matches(file_name).or_else(|| {
            if let Some(ep) = crate::library::parse::episode_number(file_name) {
                return Some(Match { episodes: vec![(ep.season, ep.episode)], batch: false, nonstandard: false });
            }

            let numbers = release::numbers(&release::normalized_name(file_name))?;
            self.map(None, numbers)
        })
    }

    fn count(&self, season: u32) -> Option<u32> {
        self.seasons.get(&season).copied().flatten()
    }

    fn absolute(&self, n: u32) -> Option<(u32, u32)> {
        let mut left = n;

        for (&season, &count) in self.seasons.range(1..) {
            match count {
                Some(c) if left > c => left -= c,
                _ => return Some((season, left)),
            }
        }

        self.seasons.range(1..).next().is_none().then_some((1, n))
    }

    fn before(&self, season: u32) -> Option<u32> {
        self.seasons.range(1..season).map(|(_, c)| *c).sum()
    }

    fn part(&self, season: u32, k: u32) -> Option<Scope> {
        let parts = self.parts.get(&season)?;

        match k {
            0 => None,
            1 => Some(Scope { season, offset: 0, episodes: parts.first().map(|p| p.offset) }),
            _ => parts.get(k as usize - 2).copied(),
        }
    }

    pub fn map(&self, scope: Option<Scope>, n: Numbers) -> Option<Match> {
        let season = n.season.or(scope.map(|s| s.season));

        let part = match n.part {
            Some(k) => self.part(season.unwrap_or(1), k),
            None => scope.filter(|s| s.offset > 0 && n.season.is_none_or(|x| x == s.season)),
        };

        let Some((first, last)) = n.episodes else {
            if let Some(p) = part.filter(|p| p.offset > 0 || n.part.is_some()) {
                let count = p.episodes?;

                return Some(Match {
                    episodes: (1..=count).map(|e| (p.season, p.offset + e)).collect(),
                    batch: true,
                    nonstandard: n.nonstandard,
                });
            }

            if n.part.is_some_and(|k| k > 1) {
                return None;
            }

            let s = season.or_else(|| (self.seasons.range(1..).count() <= 1).then_some(1))?;
            let count = self.count(s)?;

            return Some(Match {
                episodes: (1..=count).map(|e| (s, e)).collect(),
                batch: true,
                nonstandard: n.nonstandard,
            });
        };

        let mut episodes = Vec::new();

        for e in first..=last {
            episodes.push(match part {
                Some(p) if p.holds(e) => (p.season, p.offset + e),
                _ => self.one(season, e, n.season.is_some())?,
            });
        }

        Some(Match { episodes, batch: n.batch, nonstandard: n.nonstandard })
    }

    fn one(&self, season: Option<u32>, e: u32, explicit: bool) -> Option<(u32, u32)> {
        if self.numbering == Numbering::Absolute && !explicit {
            return self.absolute(e);
        }

        let s = season.unwrap_or(1);
        let before = self.before(s);

        match self.count(s) {
            Some(c) if e > c => {
                if self.numbering == Numbering::Seasonal {
                    return None;
                }

                if let Some(b) = before
                    && e > b
                    && e - b <= c
                {
                    return Some((s, e - b));
                }

                if let Some(offset) = before
                    && let Some(hit) = self.absolute(offset + e).filter(|(ss, _)| *ss > s)
                {
                    return Some(hit);
                }
                self.absolute(e).filter(|(ss, _)| season.is_none() || *ss >= s)
            },
            None if self.numbering != Numbering::Seasonal => {
                let scheduled = self.scheduled.get(&s).copied().unwrap_or(0);

                match before {
                    Some(b) if b > 0 && e > b && e > scheduled && e - b <= scheduled => Some((s, e - b)),
                    _ => Some((s, e)),
                }
            },
            _ => Some((s, e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(season: u32, titles: &[&str]) -> SeasonTitles {
        SeasonTitles { scope: Scope::season(season), titles: titles.iter().map(|t| t.to_string()).collect() }
    }

    fn show() -> Vec<String> {
        vec!["Re:ZERO -Starting Life in Another World-".into(), "Re:Zero kara Hajimeru Isekai Seikatsu".into()]
    }

    fn rezero() -> Matcher {
        let part2 = SeasonTitles {
            scope: Scope { season: 2, offset: 13, episodes: Some(12) },
            titles: vec!["Re:Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2".into()],
        };

        let mut m = Matcher::new(
            &show(),
            &[
                titles(2, &["Re:Zero kara Hajimeru Isekai Seikatsu 2nd Season"]),
                part2,
                titles(3, &["Re:Zero kara Hajimeru Isekai Seikatsu 3rd Season"]),
                titles(4, &["Re:Zero kara Hajimeru Isekai Seikatsu 4th Season"]),
            ],
            BTreeMap::from([(1, Some(25)), (2, Some(25)), (3, Some(16)), (4, None)]),
            Numbering::Auto,
        );

        m.scheduled = BTreeMap::from([(1, 25), (2, 25), (3, 16), (4, 19)]);
        m
    }

    fn rezero_split() -> Matcher {
        Matcher::new(
            &show(),
            &[
                titles(2, &["Re:Zero kara Hajimeru Isekai Seikatsu 2nd Season"]),
                titles(3, &["Re:Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2"]),
                titles(4, &["Re:Zero kara Hajimeru Isekai Seikatsu 3rd Season"]),
            ],
            BTreeMap::from([(1, Some(25)), (2, Some(13)), (3, Some(12)), (4, Some(16)), (5, None)]),
            Numbering::Auto,
        )
    }

    fn eps(m: &Matcher, title: &str) -> Vec<(u32, u32)> {
        m.matches(title).unwrap().episodes
    }

    #[test]
    fn season_titles() {
        let m = rezero();
        assert_eq!(eps(&m, "[SubsPlease] Re Zero kara Hajimeru Isekai Seikatsu 3rd Season - 05 (1080p)"), vec![(3, 5)]);
        assert_eq!(eps(&m, "[SubsPlease] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season - 05 (1080p)"), vec![(2, 5)]);

        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2 - 03 (1080p)"), vec![(2, 16)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2 - 16 (1080p)"), vec![(2, 16)]);

        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2 - 41 (1080p)"), vec![(2, 16)]);

        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu S2 Part 2 - 03 (1080p)"), vec![(2, 16)]);

        let m = rezero_split();
        assert_eq!(eps(&m, "[SubsPlease] Re Zero kara Hajimeru Isekai Seikatsu 3rd Season - 05 (1080p)"), vec![(4, 5)]);

        assert_eq!(
            eps(&m, "[SubsPlease] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2 - 03 (1080p)"),
            vec![(3, 3)]
        );
    }

    #[test]
    fn absolute_numbering() {
        let m = rezero();

        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu - 84 (1080p)"), vec![(4, 18)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu 4th Season - 84 (1080p)"), vec![(4, 18)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu S4 - 84 (1080p)"), vec![(4, 18)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu 4th Season - 18 (1080p)"), vec![(4, 18)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu - 27 (1080p)"), vec![(2, 2)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season - 39 (1080p)"), vec![(2, 14)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu - 05 (1080p)"), vec![(1, 5)]);
    }

    #[test]
    fn continued_numbering() {
        let m = rezero_split();

        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu S2 - 14 (1080p)"), vec![(3, 1)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu - 27 (1080p)"), vec![(2, 2)]);
        assert_eq!(eps(&m, "[G] Re Zero kara Hajimeru Isekai Seikatsu - 67 (1080p)"), vec![(5, 1)]);
    }

    #[test]
    fn airing_season_counts_from_its_start() {
        let mut m = Matcher::new(&["Show".into()], &[], BTreeMap::from([(1, Some(12)), (2, None)]), Numbering::Auto);
        m.scheduled = BTreeMap::from([(1, 12), (2, 24)]);
        assert_eq!(eps(&m, "[G] Show S2 - 13"), vec![(2, 13)]);
        assert_eq!(eps(&m, "[G] Show S2 - 30"), vec![(2, 18)]);
    }

    #[test]
    fn packs_and_other_shows() {
        let m = rezero();
        let hit = m.matches("[G] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season [Batch] [1080p]").unwrap();
        assert_eq!(hit.episodes.len(), 25);
        assert!(hit.batch);
        let hit = m.matches("[G] Re Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2 [Batch] [1080p]").unwrap();
        assert_eq!(hit.episodes, (14..=25).map(|e| (2, e)).collect::<Vec<_>>());
        assert!(m.matches("[G] Re Zero Break Time - 05").is_none());
        assert!(m.matches("[G] Re Zero kara Hajimeru Isekai Seikatsu Movie (1080p)").is_none());
    }

    #[test]
    fn files_without_titles() {
        let m = rezero();
        assert_eq!(m.matches_file("S04E05 - The Title.mkv").unwrap().episodes, vec![(4, 5)]);
        assert_eq!(m.matches_file("05 - The Title.mkv").unwrap().episodes, vec![(1, 5)]);
        let hit = m.matches_file("4th_18.mkv").unwrap();
        assert_eq!(hit.episodes, vec![(4, 18)]);
        assert!(hit.nonstandard);
        assert!(m.matches_file("4th_unknown_18.mkv").is_none());
    }

    #[test]
    fn ordinal_seasons_keep_their_season() {
        let mut m = rezero();

        for numbering in [Numbering::Auto, Numbering::Seasonal, Numbering::Absolute] {
            m.numbering = numbering;
            let hit = m.matches("[G] Re Zero kara Hajimeru Isekai Seikatsu 4th_18 [1080p]").unwrap();
            assert_eq!(hit.episodes, vec![(4, 18)]);
            assert!(hit.nonstandard);
        }
    }

    #[test]
    fn searches_the_core_title_before_long_translations() {
        let show: Vec<String> = [
            "Re:ZERO -Starting Life in Another World-",
            "Re:Zero kara Hajimeru Isekai Seikatsu",
            "Re: Life in a different world from zero",
            "ReZero",
            "Re Zero",
            "Re:Zero Empezar de cero en un mundo diferente",
            "Re:Zero – Bắt đầu lại ở thế giới khác",
        ]
        .map(String::from)
        .to_vec();

        let seasons = [titles(
            2,
            &["Re:ZERO -Starting Life in Another World- Season 2", "Re:Zero kara Hajimeru Isekai Seikatsu 2nd Season"],
        )];

        let m = Matcher::new(&show, &seasons, BTreeMap::from([(1, Some(25)), (2, Some(25))]), Numbering::Auto);
        let s1 = m.search_titles(Some(1));
        assert_eq!(s1[..2], ["re zero", "re zero starting life in another world"]);
        assert_eq!(s1.last().unwrap(), "re zero bắt đầu lại ở thế giới khác");
        let s2 = m.search_titles(Some(2));

        assert_eq!(
            s2[..3],
            [
                "re zero starting life in another world season 2",
                "re zero",
                "re zero kara hajimeru isekai seikatsu 2nd season"
            ]
        );
    }
}
