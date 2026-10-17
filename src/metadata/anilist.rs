// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use super::{
    Candidate, Details, EpisodeDetails, ItemKind, MediaCategory, PartSchedule, ScheduledEpisode, SeasonDetails,
    SeasonSchedule, ShowSchedule, strip_html,
};
use crate::config::Provider;

const ENDPOINT: &str = "https://graphql.anilist.co";
const SPACING: Duration = Duration::from_millis(2100);

pub struct Client {
    http: reqwest::Client,
    last: Mutex<Option<Instant>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Media {
    id: i64,
    format: Option<String>,
    title: Title,
    #[serde(default)]
    synonyms: Vec<String>,
    description: Option<String>,
    cover_image: Option<Cover>,
    banner_image: Option<String>,
    average_score: Option<f64>,
    #[serde(default)]
    genres: Vec<String>,
    season_year: Option<i64>,
    start_date: Option<StartDate>,
    episodes: Option<i64>,
    status: Option<String>,
    next_airing_episode: Option<Airing>,
    upcoming: Option<AiringPage>,
    #[serde(default)]
    streaming_episodes: Vec<StreamingEpisode>,
    relations: Option<Relations>,
}

#[derive(Debug, Deserialize)]
struct Title {
    english: Option<String>,
    romaji: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cover {
    extra_large: Option<String>,
    large: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StartDate {
    year: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct StreamingEpisode {
    title: Option<String>,
    thumbnail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Airing {
    episode: i64,
    airing_at: i64,
}

#[derive(Debug, Deserialize)]
struct AiringPage {
    nodes: Vec<Airing>,
}

#[derive(Debug, Deserialize)]
struct Relations {
    edges: Vec<Edge>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Edge {
    relation_type: String,
    node: RelNode,
}

#[derive(Debug, Deserialize)]
struct RelNode {
    id: i64,
    format: Option<String>,
    relations: Option<Relations>,
}

const SEARCH_FIELDS: &str = "id format title { english romaji } description(asHtml: false)
    coverImage { extraLarge large } seasonYear startDate { year }";

const ENTRY_FIELDS: &str = "id format title { english romaji } synonyms description(asHtml: false)
    coverImage { extraLarge large } bannerImage averageScore genres seasonYear startDate { year }
    episodes status nextAiringEpisode { episode airingAt }
    streamingEpisodes { title thumbnail }
    relations { edges { relationType node { id format relations { edges { relationType node { id format } } } } } }
    upcoming: airingSchedule(notYetAired: true, perPage: 50) { nodes { episode airingAt } }";

static EPISODE_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Episode\s+(\d+)\s*[-:–]\s*(.+)$").unwrap());

static CONTINUATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:part|cour)[ .]*(?:[2-9]|ii|iii|iv)\b|\b(?:2nd|3rd|[4-9]th)[ .]*cour\b").unwrap()
});

static CREDIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)\s*(?:\(\s*sources?\s*:[^)]*\)|\[\s*written by [^\]]*\])\s*$").unwrap());

fn is_series(format: Option<&str>) -> bool {
    matches!(format, Some("TV" | "TV_SHORT" | "ONA"))
}

fn searchable(s: &str) -> bool {
    let letters = s.chars().filter(|c| c.is_alphanumeric()).count();
    letters > 0 && s.chars().filter(|c| c.is_ascii_alphanumeric()).count() * 2 > letters
}

impl Media {
    fn title(&self) -> String {
        self.title.english.clone().or_else(|| self.title.romaji.clone()).unwrap_or_default()
    }

    fn year(&self) -> Option<i64> {
        self.season_year.or(self.start_date.as_ref().and_then(|d| d.year))
    }

    fn poster(&self) -> Option<String> {
        self.cover_image.as_ref().and_then(|c| c.extra_large.clone().or_else(|| c.large.clone()))
    }

    fn overview(&self) -> Option<String> {
        let text = strip_html(self.description.as_deref()?);
        let text = CREDIT.replace(&text, "").trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    fn candidate(self) -> Candidate {
        Candidate {
            category: MediaCategory::anilist(self.format.as_deref()),
            provider: Provider::Anilist,
            id: self.id.to_string(),
            title: self.title(),
            romaji: self.title.romaji.clone().filter(|r| *r != self.title()),
            year: self.year(),
            poster: self.poster(),
            overview: self.overview(),
        }
    }

    fn aliases(&self) -> Vec<String> {
        let mut titles: Vec<String> =
            [&self.title.english, &self.title.romaji].into_iter().flatten().cloned().collect();

        titles.extend(self.synonyms.iter().cloned());
        titles.retain(|t| searchable(t));
        titles.dedup();
        titles
    }

    fn related(&self, relation: &str) -> Option<i64> {
        let edges = || self.relations.iter().flat_map(|r| &r.edges).filter(|e| e.relation_type == relation);

        edges().find(|e| is_series(e.node.format.as_deref())).map(|e| e.node.id).or_else(|| {
            edges()
                .flat_map(|e| e.node.relations.iter().flat_map(|r| &r.edges))
                .find(|e| e.relation_type == relation && is_series(e.node.format.as_deref()))
                .map(|e| e.node.id)
        })
    }
}

type Streamed = (i64, String, Option<String>);

struct Entry {
    media: Media,
    air: HashMap<i64, i64>,
}

impl Entry {
    fn known(&self) -> i64 {
        self.media.episodes.or_else(|| self.air.keys().copied().max()).unwrap_or(0)
    }
}

struct Season {
    number: i64,
    parts: Vec<Part>,
}

struct Part {
    entry: Entry,
    offset: i64,
    before: Option<i64>,
    streaming: Vec<Streamed>,
}

impl Season {
    fn episodes(&self) -> Option<i64> {
        self.parts.iter().map(|p| p.entry.media.episodes).sum()
    }

    fn first(&self) -> &Media {
        &self.parts[0].entry.media
    }

    fn reading(&self, i: usize) -> Option<(usize, i64, i64, i64)> {
        self.reading_of(i, &self.parts[i].streaming)
    }

    fn reading_of(&self, i: usize, list: &[Streamed]) -> Option<(usize, i64, i64, i64)> {
        let p = &self.parts[i];
        let length: i64 = self.parts.iter().map(|p| p.entry.known()).sum();
        let mut readings = vec![(p.offset, p.offset + 1, p.offset + p.entry.known()), (0, 1, length)];
        readings.extend(self.parts[0].before.map(|b| (-b, 1, length)));
        let mut best: Option<(usize, i64, i64, i64)> = None;

        for (shift, lo, hi) in readings {
            let n = list.iter().filter(|e| (lo..=hi).contains(&(e.0 + shift))).count();

            if best.is_none_or(|b| n > b.0) {
                best = Some((n, shift, lo, hi));
            }
        }

        best.filter(|b| b.0 > 0 && b.0 * 2 >= list.len())
    }

    fn streaming(&self) -> BTreeMap<i64, (String, Option<String>)> {
        let mut out = BTreeMap::new();

        for (i, p) in self.parts.iter().enumerate() {
            let Some((_, shift, lo, hi)) = self.reading(i) else {
                continue;
            };

            for (number, title, still) in &p.streaming {
                let e = number + shift;

                if (lo..=hi).contains(&e) {
                    out.entry(e).or_insert_with(|| (title.clone(), still.clone()));
                }
            }
        }

        out
    }
}

fn share_streaming(seasons: &mut [Season]) {
    let all: Vec<(usize, usize)> =
        seasons.iter().enumerate().flat_map(|(si, s)| (0..s.parts.len()).map(move |pi| (si, pi))).collect();

    let mut lists: Vec<(Vec<Streamed>, Vec<(usize, usize)>)> = Vec::new();

    for &(si, pi) in &all {
        let list = &seasons[si].parts[pi].streaming;

        if list.is_empty() {
            continue;
        }

        match lists.iter_mut().find(|(l, _)| l == list) {
            Some((_, carriers)) => carriers.push((si, pi)),
            None => lists.push((list.clone(), vec![(si, pi)])),
        }
    }

    for (list, carriers) in lists.into_iter().filter(|(_, c)| c.len() > 1) {
        let fit = |(si, pi): (usize, usize)| seasons[si].reading_of(pi, &list).map_or(0, |r| r.0);
        let mut best = carriers[0];

        for &c in &carriers[1..] {
            if fit(c) > fit(best) {
                best = c;
            }
        }

        let after = carriers[0];

        for &c in all.iter().filter(|&&c| c > after && seasons[c.0].parts[c.1].streaming.is_empty()) {
            if fit(c) > fit(best) {
                best = c;
            }
        }

        for &(si, pi) in &carriers {
            seasons[si].parts[pi].streaming.clear();
        }

        let (si, pi) = best;
        seasons[si].parts[pi].streaming = list;
    }
}

fn streaming(m: &Media) -> Vec<Streamed> {
    m.streaming_episodes
        .iter()
        .filter_map(|e| {
            let caps = EPISODE_TITLE.captures(e.title.as_deref()?)?;
            Some((caps[1].parse().ok()?, caps[2].trim().to_string(), e.thumbnail.clone()))
        })
        .collect()
}

fn continues(season: &Season, m: &Media, library: &BTreeMap<i64, i64>) -> bool {
    let Some(length) = season.episodes() else {
        return false;
    };

    match library.get(&season.number) {
        Some(&last) if last > length => true,
        Some(_) if library.contains_key(&(season.number + 1)) => false,
        _ => [&m.title.english, &m.title.romaji].into_iter().flatten().any(|t| CONTINUATION.is_match(t)),
    }
}

impl Client {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, last: Mutex::new(None) }
    }

    async fn query<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> anyhow::Result<T> {
        for attempt in 0..3 {
            {
                let mut last = self.last.lock().await;

                if let Some(t) = *last {
                    let wait = SPACING.saturating_sub(t.elapsed());
                    tokio::time::sleep(wait).await;
                }

                *last = Some(Instant::now());
            }

            let res = self
                .http
                .post(ENDPOINT)
                .json(&json!({ "query": query, "variables": variables }))
                .send()
                .await
                .context("can't reach AniList")?;

            if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry =
                    res.headers().get("retry-after").and_then(|v| v.to_str().ok()?.parse().ok()).unwrap_or(60u64);

                tracing::debug!("AniList rate limit hit (attempt {attempt}); waiting {retry}s");
                tokio::time::sleep(Duration::from_secs(retry)).await;
                continue;
            }

            let body: serde_json::Value = res.error_for_status()?.json().await?;

            if let Some(errors) = body.get("errors") {
                bail!("AniList: {errors}");
            }

            return Ok(serde_json::from_value(body["data"].clone())?);
        }
        bail!("AniList kept rate-limiting us; will try again later")
    }

    async fn entry(&self, id: i64) -> anyhow::Result<Entry> {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]

        struct Data {
            media: Media,
            page: RecentPage,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]

        struct RecentPage {
            airing_schedules: Vec<Airing>,
        }

        let q = format!(
            "query ($id: Int) {{ Media(id: $id) {{ {ENTRY_FIELDS} }}
               Page(perPage: 50) {{ airingSchedules(mediaId: $id, notYetAired: false, sort: TIME_DESC) {{ episode airingAt }} }} }}"
        );

        let data: Data = self.query(&q, json!({ "id": id })).await?;
        let mut air: HashMap<i64, i64> = data.page.airing_schedules.iter().map(|a| (a.episode, a.airing_at)).collect();
        let m = &data.media;

        for a in m.upcoming.iter().flat_map(|u| &u.nodes).chain(m.next_airing_episode.as_ref()) {
            air.insert(a.episode, a.airing_at);
        }

        Ok(Entry { media: data.media, air })
    }

    async fn seasons(&self, id: i64, library: &BTreeMap<i64, i64>, last: Option<i64>) -> anyhow::Result<Vec<Season>> {
        let mut seasons: Vec<Season> = Vec::new();
        let mut before = Some(0);
        let mut seen = HashSet::new();
        let mut next = Some(id);

        while let Some(id) = next.take() {
            if !seen.insert(id) || seen.len() > 40 {
                break;
            }

            let entry = self.entry(id).await?;
            next = entry.media.related("SEQUEL");
            let episodes = entry.media.episodes;

            match seasons.last_mut() {
                Some(s) if continues(s, &entry.media, library) => {
                    let offset = s.episodes().unwrap_or(0);
                    s.parts.push(Part { streaming: streaming(&entry.media), entry, offset, before });
                },
                _ => {
                    let number = seasons.len() as i64 + 1;

                    if last.is_some_and(|l| number > l) {
                        break;
                    }

                    let part = Part { streaming: streaming(&entry.media), entry, offset: 0, before };
                    seasons.push(Season { number, parts: vec![part] });
                },
            }

            before = before.zip(episodes).map(|(b, n)| b + n);
        }

        share_streaming(&mut seasons);
        Ok(seasons)
    }

    pub async fn first_season(&self, id: &str) -> anyhow::Result<Candidate> {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]

        struct Data {
            media: Media,
        }

        let q = format!(
            "query ($id: Int) {{ Media(id: $id) {{ {SEARCH_FIELDS}
                relations {{ edges {{ relationType node {{ id format relations {{ edges {{ relationType node {{ id format }} }} }} }} }} }} }} }}"
        );

        let mut id: i64 = id.parse().context("AniList ids are numbers")?;
        let mut seen = HashSet::from([id]);

        loop {
            let data: Data = self.query(&q, json!({ "id": id })).await?;
            let m = data.media;

            match m.related("PREQUEL") {
                Some(prequel) if is_series(m.format.as_deref()) && seen.insert(prequel) && seen.len() <= 40 => {
                    id = prequel
                },
                _ => return Ok(m.candidate()),
            }
        }
    }

    pub async fn search(&self, kind: ItemKind, query: &str) -> anyhow::Result<Vec<Candidate>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]

        struct Data {
            page: Page,
        }

        #[derive(Deserialize)]
        struct Page {
            media: Vec<Media>,
        }

        let formats = match kind {
            ItemKind::Show => json!(["TV", "TV_SHORT", "ONA", "OVA", "SPECIAL"]),
            ItemKind::Movie => json!(["MOVIE"]),
        };

        let q = format!(
            "query ($search: String, $formats: [MediaFormat]) {{ Page(perPage: 10) {{
                media(search: $search, type: ANIME, format_in: $formats, sort: SEARCH_MATCH) {{ {SEARCH_FIELDS} }} }} }}"
        );

        let data: Data = self.query(&q, json!({ "search": query, "formats": formats })).await?;
        Ok(data.page.media.into_iter().map(Media::candidate).collect())
    }

    pub async fn recommendations(
        &self,
        kind: ItemKind,
        ids: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<Candidate>>> {
        let ids: Vec<i64> = ids.iter().filter_map(|id| id.parse().ok()).collect();
        let mut out = HashMap::new();
        for chunk in ids.chunks(10) {
            match self.recommendations_of(kind, chunk).await {
                Ok(found) => out.extend(found),

                Err(e) if chunk.len() > 1 => {
                    tracing::debug!("AniList recommendations for {chunk:?}: {e:#}");

                    for &id in chunk {
                        match self.recommendations_of(kind, &[id]).await {
                            Ok(found) => out.extend(found),
                            Err(e) => tracing::debug!("AniList recommendations for {id}: {e:#}"),
                        }
                    }
                },
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    async fn recommendations_of(&self, kind: ItemKind, ids: &[i64]) -> anyhow::Result<HashMap<String, Vec<Candidate>>> {
        #[derive(Deserialize)]
        struct Entry {
            recommendations: Option<Page>,
        }

        #[derive(Deserialize)]
        struct Page {
            nodes: Vec<Node>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]

        struct Node {
            media_recommendation: Option<Recommended>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]

        struct Recommended {
            #[serde(flatten)]
            media: Media,
            #[serde(default)]
            is_adult: bool,
        }

        let fields = ids
            .iter()
            .map(|id| {
                format!(
                    "m{id}: Media(id: {id}) {{ recommendations(sort: [RATING_DESC, ID], perPage: 20) {{
                        nodes {{ mediaRecommendation {{ {SEARCH_FIELDS} isAdult }} }} }} }}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let data: HashMap<String, Option<Entry>> = self.query(&format!("query {{ {fields} }}"), json!({})).await?;

        let wanted = |format: Option<&str>| match kind {
            ItemKind::Show => matches!(format, Some("TV" | "TV_SHORT" | "ONA" | "OVA" | "SPECIAL")),
            ItemKind::Movie => format == Some("MOVIE"),
        };

        Ok(data
            .into_iter()
            .map(|(alias, entry)| {
                let list = entry
                    .and_then(|e| e.recommendations)
                    .map(|p| p.nodes)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|n| n.media_recommendation)
                    .filter(|r| !r.is_adult && wanted(r.media.format.as_deref()))
                    .map(|r| r.media.candidate())
                    .collect();

                (alias.trim_start_matches('m').to_string(), list)
            })
            .collect())
    }

    pub async fn trending(&self, kind: ItemKind) -> anyhow::Result<Vec<Candidate>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]

        struct Data {
            page: Page,
        }

        #[derive(Deserialize)]
        struct Page {
            media: Vec<Media>,
        }

        let formats = match kind {
            ItemKind::Show => "[TV, TV_SHORT, ONA, OVA, SPECIAL]",
            ItemKind::Movie => "[MOVIE]",
        };

        let q = format!(
            "query {{ Page(perPage: 30) {{
                media(type: ANIME, format_in: {formats}, isAdult: false, sort: TRENDING_DESC) {{ {SEARCH_FIELDS} }} }} }}"
        );

        let data: Data = self.query(&q, json!({})).await?;
        Ok(data.page.media.into_iter().map(Media::candidate).collect())
    }

    pub async fn schedule(&self, id: &str, library: &BTreeMap<i64, i64>) -> anyhow::Result<ShowSchedule> {
        let id: i64 = id.parse().context("AniList ids are numbers")?;
        let seasons = self.seasons(id, library, None).await?;

        let mut schedule = ShowSchedule {
            aliases: seasons.first().map(|s| s.first().aliases()).unwrap_or_default(),
            ..Default::default()
        };

        for s in &seasons {
            let first = s.first();

            let mut season = SeasonSchedule {
                number: s.number,
                provider_id: Some(first.id.to_string()),
                title: first.title.english.clone().or(first.title.romaji.clone()),
                aliases: first.aliases(),
                episodes: s.episodes(),
                ..Default::default()
            };

            let titles = s.streaming();

            for (i, p) in s.parts.iter().enumerate() {
                let m = &p.entry.media;

                schedule.status = match m.status.as_deref() {
                    Some("RELEASING") => Some("airing".into()),
                    Some("FINISHED") => Some("finished".into()),
                    Some("NOT_YET_RELEASED") => Some("upcoming".into()),
                    Some("HIATUS") => Some("hiatus".into()),
                    _ => schedule.status,
                };

                let finished = m.status.as_deref() == Some("FINISHED");
                let next_ep = m.next_airing_episode.as_ref().map(|a| a.episode);

                season.airing.extend((1..=p.entry.known()).map(|n| {
                    let air_at = p.entry.air.get(&n).copied();

                    ScheduledEpisode {
                        number: p.offset + n,
                        title: titles.get(&(p.offset + n)).map(|t| t.0.clone()),
                        air_at,
                        aired: finished
                            || next_ep.is_some_and(|x| n < x)
                            || air_at.is_some_and(|t| t <= crate::db::now()),
                    }
                }));

                if i > 0 {
                    season.parts.push(PartSchedule {
                        provider_id: Some(m.id.to_string()),
                        offset: p.offset,
                        episodes: m.episodes,
                        aliases: m.aliases(),
                    });
                }
            }

            schedule.seasons.push(season);
        }

        Ok(schedule)
    }

    pub async fn details(&self, kind: ItemKind, id: &str, library: &BTreeMap<i64, i64>) -> anyhow::Result<Details> {
        let id: i64 = id.parse().context("AniList ids are numbers")?;

        if kind == ItemKind::Movie {
            let first = self.entry(id).await?.media;
            return Ok(Self::show(&first));
        }

        let last = library.keys().copied().max().unwrap_or(1).max(1);
        let seasons = self.seasons(id, library, Some(last)).await?;
        let Some(first) = seasons.first() else { bail!("AniList has no entry {id}") };
        let mut details = Self::show(first.first());

        for s in seasons.iter().filter(|s| library.contains_key(&s.number)) {
            let m = s.first();

            details.seasons.push(SeasonDetails {
                number: s.number,
                title: Some(m.title()),
                overview: m.overview(),
                poster: m.poster(),
                length: s.episodes(),
                provider_ids: s.parts.iter().map(|p| p.entry.media.id.to_string()).collect(),
                episodes: s
                    .streaming()
                    .into_iter()
                    .map(|(number, (title, still))| EpisodeDetails {
                        number,
                        title: Some(title),
                        still,
                        ..Default::default()
                    })
                    .collect(),
            });
        }

        Ok(details)
    }

    fn show(m: &Media) -> Details {
        Details {
            title: m.title(),
            year: m.year(),
            overview: m.overview(),
            genres: m.genres.clone(),
            rating: m.average_score.map(|s| s / 10.0),
            poster: m.poster(),
            backdrop: m.banner_image.clone(),
            aliases: m.aliases(),
            seasons: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_keep_the_provider_format() {
        for (format, category) in [
            ("TV", MediaCategory::Episodes),
            ("OVA", MediaCategory::Specials),
            ("SPECIAL", MediaCategory::Specials),
            ("MOVIE", MediaCategory::Movies),
        ] {
            let media: Media = serde_json::from_value(json!({
                "id": 1, "format": format, "title": { "english": "Title", "romaji": null }
            }))
            .unwrap();

            assert_eq!(media.candidate().category, category);
        }
    }

    #[test]
    fn drops_the_credit_from_descriptions() {
        let strip = |s: &str| CREDIT.replace(s, "").to_string();
        assert_eq!(strip("Iruma performs.\n\n(Source: Crunchyroll News)"), "Iruma performs.");
        assert_eq!(strip("A story. [Written by MAL Rewrite]"), "A story.");
        assert_eq!(strip("Keeps (Source: inline) notes mid-text."), "Keeps (Source: inline) notes mid-text.");
    }

    #[test]
    fn continuation_titles() {
        for t in [
            "Re:ZERO -Starting Life in Another World- Season 2 Part 2",
            "Re:Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2",
            "Mushoku Tensei: Isekai Ittara Honki Dasu Part 2",
            "NieR:Automata Ver1.1a Cour 2",
            "Shingeki no Kyojin: The Final Season Part 2",
            "Some Show 2nd Cour",
        ] {
            assert!(CONTINUATION.is_match(t), "{t}");
        }
        for t in [
            "Re:ZERO -Starting Life in Another World- Season 2",
            "Mushoku Tensei II: Isekai Ittara Honki Dasu",
            "Sword Art Online II",
            "Kono Subarashii Sekai ni Shukufuku wo! 2",
            "Show Part 1",
        ] {
            assert!(!CONTINUATION.is_match(t), "{t}");
        }
    }

    fn entry(id: i64, title: &str, episodes: i64, streaming: &[i64]) -> Entry {
        let eps: Vec<_> = streaming.iter().map(|n| json!({ "title": format!("Episode {n} - Title {n}") })).collect();
        let media = json!({ "id": id, "title": { "english": title }, "episodes": episodes, "streamingEpisodes": eps });
        Entry { media: serde_json::from_value(media).unwrap(), air: HashMap::new() }
    }

    fn group(entries: Vec<Entry>, library: &[(i64, i64)]) -> Vec<Season> {
        let library: BTreeMap<i64, i64> = library.iter().copied().collect();
        let mut seasons: Vec<Season> = Vec::new();
        let mut before = Some(0);

        for entry in entries {
            let episodes = entry.media.episodes;
            let streaming = streaming(&entry.media);

            match seasons.last_mut() {
                Some(s) if continues(s, &entry.media, &library) => {
                    let offset = s.episodes().unwrap();
                    s.parts.push(Part { streaming, entry, offset, before });
                },
                _ => seasons.push(Season {
                    number: seasons.len() as i64 + 1,
                    parts: vec![Part { streaming, entry, offset: 0, before }],
                }),
            }

            before = before.zip(episodes).map(|(b, n)| b + n);
        }

        share_streaming(&mut seasons);
        seasons
    }

    fn rezero() -> Vec<Entry> {
        let s3: Vec<i64> = (51..=66).collect();

        vec![
            entry(1, "Re:ZERO", 25, &s3),
            entry(2, "Re:ZERO Season 2", 13, &s3),
            entry(3, "Re:ZERO Season 2 Part 2", 12, &s3),
            entry(4, "Re:ZERO Season 3", 16, &[]),
            entry(5, "Re:ZERO Season 4", 19, &[]),
        ]
    }

    #[test]
    fn split_seasons_join() {
        let lengths = |seasons: &[Season]| seasons.iter().map(|s| s.episodes().unwrap()).collect::<Vec<_>>();
        assert_eq!(lengths(&group(rezero(), &[(1, 25), (2, 25), (3, 16)])), vec![25, 25, 16, 19]);
        assert_eq!(lengths(&group(rezero(), &[])), vec![25, 25, 16, 19]);
        assert_eq!(lengths(&group(rezero(), &[(1, 25), (2, 13), (3, 12)])), vec![25, 13, 12, 16, 19]);
    }

    #[test]
    fn streaming_titles_go_where_they_fit() {
        let seasons = group(rezero(), &[(1, 25), (2, 25), (3, 16)]);
        let titled = |s: &Season| s.streaming().into_keys().collect::<Vec<_>>();
        assert_eq!(titled(&seasons[0]), Vec::<i64>::new());
        assert_eq!(titled(&seasons[1]), Vec::<i64>::new());
        assert_eq!(titled(&seasons[2]), (1..=16).collect::<Vec<_>>());
        assert_eq!(seasons[2].streaming()[&1].0, "Title 51");

        let s1: Vec<i64> = (1..=25).collect();
        let seasons = group(vec![entry(1, "SAO", 25, &s1), entry(2, "SAO II", 24, &s1)], &[(1, 25), (2, 24)]);
        assert_eq!((titled(&seasons[0]).len(), titled(&seasons[1]).len()), (25, 0));

        let seasons = group(
            vec![entry(1, "Show", 12, &(1..=12).collect::<Vec<_>>()), entry(2, "Show Part 2", 12, &[1, 2, 3])],
            &[],
        );

        assert_eq!(titled(&seasons[0]), (1..=15).collect::<Vec<_>>());
        assert_eq!(seasons[0].streaming()[&13].0, "Title 1");
    }
}
