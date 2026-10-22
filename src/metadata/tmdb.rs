// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use anyhow::{Context, bail};
use serde::Deserialize;

use super::{
    Candidate, Details, EpisodeDetails, ItemKind, MediaCategory, ScheduledEpisode, SeasonDetails, SeasonSchedule,
    ShowSchedule,
};
use crate::config::{Metadata, Provider};

const API: &str = "https://api.themoviedb.org/3";
const IMG: &str = "https://image.tmdb.org/t/p";

pub struct Client {
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct SearchPage {
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    id: i64,
    #[serde(alias = "name")]
    title: String,
    #[serde(alias = "first_air_date")]
    release_date: Option<String>,
    poster_path: Option<String>,
    overview: Option<String>,
}

#[derive(Deserialize)]
struct Genre {
    name: String,
}

#[derive(Deserialize)]
struct Show {
    #[serde(alias = "title")]
    name: String,
    #[serde(alias = "release_date")]
    first_air_date: Option<String>,
    overview: Option<String>,
    #[serde(default)]
    genres: Vec<Genre>,
    vote_average: Option<f64>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    seasons: Vec<SeasonSummary>,
}

#[derive(Deserialize)]
struct SeasonSummary {
    season_number: i64,
    episode_count: Option<i64>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct ScheduleShow {
    name: String,
    original_name: Option<String>,
    status: Option<String>,
    #[serde(default)]
    seasons: Vec<SeasonSummary>,
}

#[derive(Deserialize)]
struct AltTitles {
    #[serde(default)]
    results: Vec<AltTitle>,
}

#[derive(Deserialize)]
struct AltTitle {
    title: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ExternalIds {
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
}

#[derive(Deserialize)]
struct Season {
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    #[serde(default)]
    episodes: Vec<Episode>,
}

#[derive(Deserialize)]
struct Episode {
    episode_number: i64,
    name: Option<String>,
    overview: Option<String>,
    still_path: Option<String>,
    air_date: Option<String>,
}

fn parse_date(s: &str) -> Option<time::Date> {
    time::Date::parse(s, time::macros::format_description!("[year]-[month]-[day]")).ok()
}

fn year(date: &Option<String>) -> Option<i64> {
    date.as_deref()?.get(..4)?.parse().ok()
}

fn img(size: &str, path: &Option<String>) -> Option<String> {
    path.as_ref().map(|p| format!("{IMG}/{size}{p}"))
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.trim().is_empty())
}

fn candidate(r: SearchResult, kind: ItemKind) -> Candidate {
    Candidate {
        category: if kind == ItemKind::Movie { MediaCategory::Movies } else { MediaCategory::Episodes },
        provider: Provider::Tmdb,
        id: r.id.to_string(),
        year: year(&r.release_date),
        poster: img("w342", &r.poster_path),
        overview: non_empty(r.overview),
        title: r.title,
        romaji: None,
    }
}

impl Client {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        config: &Metadata,
        path: &str,
        query: &[(&str, String)],
    ) -> anyhow::Result<T> {
        let Some(key) = config.tmdb_api_key.as_deref().filter(|k| !k.is_empty()) else {
            bail!("TMDB needs an API key: set `tmdb-api-key` under [metadata] in config.toml (or in Settings)");
        };

        let mut req =
            self.http.get(format!("{API}{path}")).query(query).query(&[("language", config.language.as_str())]);

        req = if key.len() > 40 { req.bearer_auth(key) } else { req.query(&[("api_key", key)]) };
        let res = req.send().await.context("can't reach TMDB")?;

        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            bail!("TMDB rejected the API key in config.toml");
        }

        Ok(res.error_for_status()?.json().await?)
    }

    pub async fn search(
        &self,
        config: &Metadata,
        kind: ItemKind,
        query: &str,
        year_hint: Option<i64>,
    ) -> anyhow::Result<Vec<Candidate>> {
        let (path, year_param) = match kind {
            ItemKind::Show => ("/search/tv", "first_air_date_year"),
            ItemKind::Movie => ("/search/movie", "year"),
        };

        let mut q = vec![("query", query.to_string())];

        if let Some(y) = year_hint {
            q.push((year_param, y.to_string()));
        }

        let mut page: SearchPage = self.get(config, path, &q).await?;

        if page.results.is_empty() && year_hint.is_some() {
            page = self.get(config, path, &q[..1]).await?;
        }

        Ok(page.results.into_iter().map(|r| candidate(r, kind)).collect())
    }

    pub async fn trending(&self, config: &Metadata, kind: ItemKind) -> anyhow::Result<Vec<Candidate>> {
        let path = if kind == ItemKind::Movie { "/trending/movie/week" } else { "/trending/tv/week" };
        let page: SearchPage = self.get(config, path, &[]).await?;
        Ok(page.results.into_iter().map(|r| candidate(r, kind)).collect())
    }

    pub async fn recommendations(
        &self,
        config: &Metadata,
        kind: ItemKind,
        ids: &[String],
    ) -> anyhow::Result<HashMap<String, Vec<Candidate>>> {
        let base = match kind {
            ItemKind::Show => "/tv",
            ItemKind::Movie => "/movie",
        };

        let pages = futures::future::join_all(ids.iter().map(|id| async move {
            let page: anyhow::Result<SearchPage> = self.get(config, &format!("{base}/{id}/recommendations"), &[]).await;
            (id, page)
        }))
        .await;

        let mut out = HashMap::new();

        for (id, page) in pages {
            match page {
                Ok(page) => {
                    out.insert(id.clone(), page.results.into_iter().map(|r| candidate(r, kind)).collect());
                },
                Err(e) if config.tmdb_api_key.as_deref().is_none_or(str::is_empty) => {
                    return Err(e);
                },
                Err(e) => tracing::debug!("TMDB recommendations for {id}: {e:#}"),
            }
        }

        Ok(out)
    }

    pub async fn schedule(&self, config: &Metadata, id: &str) -> anyhow::Result<(ShowSchedule, ExternalIds)> {
        let show: ScheduleShow = self.get(config, &format!("/tv/{id}"), &[]).await?;

        let alt: AltTitles = self
            .get(config, &format!("/tv/{id}/alternative_titles"), &[])
            .await
            .unwrap_or(AltTitles { results: vec![] });

        let external: ExternalIds = self.get(config, &format!("/tv/{id}/external_ids"), &[]).await.unwrap_or_default();
        let mut aliases = vec![show.name.clone()];
        aliases.extend(show.original_name.clone());
        aliases.extend(alt.results.into_iter().map(|a| a.title));
        aliases.dedup();

        let mut schedule = ShowSchedule {
            status: show.status.as_deref().map(|s| {
                match s {
                    "Returning Series" | "In Production" => "airing",
                    "Planned" => "upcoming",
                    _ => "finished",
                }
                .to_string()
            }),
            aliases,
            seasons: Vec::new(),
        };

        let today = time::OffsetDateTime::now_utc().date();

        for summary in show.seasons.iter().take(60) {
            let n = summary.season_number;
            let season: Season = self.get(config, &format!("/tv/{id}/season/{n}"), &[]).await?;

            schedule.seasons.push(SeasonSchedule {
                number: n,
                provider_id: None,
                title: non_empty(summary.name.clone()),
                aliases: Vec::new(),
                parts: Vec::new(),
                episodes: summary.episode_count,
                airing: season
                    .episodes
                    .into_iter()
                    .map(|e| {
                        let date = e.air_date.as_deref().and_then(parse_date);

                        ScheduledEpisode {
                            number: e.episode_number,
                            title: non_empty(e.name),
                            air_at: date.map(|d| d.midnight().assume_utc().unix_timestamp()),
                            aired: date.is_some_and(|d| d < today),
                        }
                    })
                    .collect(),
            });
        }

        Ok((schedule, external))
    }

    pub async fn details(
        &self,
        config: &Metadata,
        kind: ItemKind,
        id: &str,
        seasons: &[i64],
    ) -> anyhow::Result<Details> {
        let path = match kind {
            ItemKind::Show => format!("/tv/{id}"),
            ItemKind::Movie => format!("/movie/{id}"),
        };

        let show: Show = self.get(config, &path, &[]).await?;

        let mut details = Details {
            title: show.name,
            year: year(&show.first_air_date),
            overview: non_empty(show.overview),
            genres: show.genres.into_iter().map(|g| g.name).collect(),
            rating: show.vote_average.filter(|v| *v > 0.0),
            poster: img("w780", &show.poster_path),
            backdrop: img("w1280", &show.backdrop_path),
            aliases: Vec::new(),
            seasons: Vec::new(),
        };

        for summary in show.seasons.iter().filter(|s| seasons.contains(&s.season_number)) {
            let n = summary.season_number;
            let season: Season = self.get(config, &format!("/tv/{id}/season/{n}"), &[]).await?;

            details.seasons.push(SeasonDetails {
                number: n,
                title: non_empty(season.name),
                overview: non_empty(season.overview),
                poster: img("w500", &season.poster_path),
                length: summary.episode_count,
                provider_ids: Vec::new(),
                episodes: season
                    .episodes
                    .into_iter()
                    .map(|e| EpisodeDetails {
                        number: e.episode_number,
                        title: non_empty(e.name),
                        overview: non_empty(e.overview),
                        still: img("w500", &e.still_path),
                        air_date: e.air_date,
                    })
                    .collect(),
            });
        }

        Ok(details)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_keep_the_search_kind() {
        let result =
            |title| serde_json::from_value::<SearchResult>(serde_json::json!({ "id": 1, "title": title })).unwrap();

        assert_eq!(candidate(result("Show"), ItemKind::Show).category, MediaCategory::Episodes);
        assert_eq!(candidate(result("Movie"), ItemKind::Movie).category, MediaCategory::Movies);
    }
}
