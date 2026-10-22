// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use anyhow::bail;
use serde::Deserialize;

use super::tmdb::ExternalIds;

const API: &str = "https://api.tvmaze.com";

pub struct Client {
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct Show {
    id: i64,
    premiered: Option<String>,
}

#[derive(Deserialize)]
struct SearchResult {
    show: Show,
}

#[derive(Deserialize)]
pub struct Episode {
    pub season: i64,
    pub number: Option<i64>,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub image: Option<Image>,
    pub airdate: Option<String>,
    airstamp: Option<String>,
}

#[derive(Deserialize)]
pub struct Image {
    pub medium: Option<String>,
    pub original: Option<String>,
}

impl Client {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    async fn episodes(&self, show: i64) -> anyhow::Result<Vec<Episode>> {
        Ok(self.http.get(format!("{API}/shows/{show}/episodes")).send().await?.error_for_status()?.json().await?)
    }

    pub async fn air_times(&self, ids: &ExternalIds) -> anyhow::Result<HashMap<(i64, i64), i64>> {
        let lookup = if let Some(tvdb) = ids.tvdb_id {
            format!("{API}/lookup/shows?thetvdb={tvdb}")
        } else if let Some(imdb) = &ids.imdb_id {
            format!("{API}/lookup/shows?imdb={imdb}")
        } else {
            bail!("no TVDB or IMDb id to look the show up by");
        };

        let res = self.http.get(lookup).send().await?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("not on TVmaze");
        }

        let show: Show = res.error_for_status()?.json().await?;

        Ok(self
            .episodes(show.id)
            .await?
            .into_iter()
            .filter_map(|e| {
                let at =
                    time::OffsetDateTime::parse(e.airstamp.as_deref()?, &time::format_description::well_known::Rfc3339)
                        .ok()?;

                Some(((e.season, e.number?), at.unix_timestamp()))
            })
            .collect())
    }

    pub async fn episodes_by_title(&self, titles: &[String], year: i64) -> anyhow::Result<Option<Vec<Episode>>> {
        for title in titles {
            let results: Vec<SearchResult> = self
                .http
                .get(format!("{API}/search/shows"))
                .query(&[("q", title)])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            let premiered = |s: &Show| s.premiered.as_deref()?.get(..4)?.parse::<i64>().ok();

            if let Some(r) = results.iter().find(|r| premiered(&r.show).is_some_and(|y| (y - year).abs() <= 1)) {
                return Ok(Some(self.episodes(r.show.id).await?));
            }
        }
        Ok(None)
    }
}
