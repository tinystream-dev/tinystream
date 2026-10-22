// SPDX-License-Identifier: AGPL-3.0-or-later

//! What books don't say about themselves, looked up once each: Apple Books
//! first (blurbs and large covers, no key needed), then Open Library.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::Notify;

use super::catalog::{self, BookRow};
use super::markup::plain;
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

#[derive(Default)]
pub struct Lookups {
    wake: Notify,
}

pub fn wake(state: &AppState) {
    state.books.online.wake.notify_one();
}

#[derive(Debug, Default, PartialEq)]
pub struct Found {
    pub provider: &'static str,
    pub id: String,
    pub overview: Option<String>,
    pub subjects: Vec<String>,
    pub cover: Option<String>,
    pub date: Option<String>,
    pub rating: Option<f64>,
    pub pages: Option<i64>,
}

fn normalize(s: &str) -> String {
    let s = s.to_lowercase().replace(['’', '\''], "");
    let s: String = s.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s.replace(" light novel", "").replace("light novel ", "")
}

fn numbers(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut n: Option<u32> = None;

    for c in s.chars() {
        match c.to_digit(10) {
            Some(d) => n = Some(n.unwrap_or(0).saturating_mul(10).saturating_add(d)),
            None => out.extend(n.take()),
        }
    }

    out.extend(n);
    out
}

/// How well a title found online matches the book's, if it does at all.
pub fn score(title: &str, authors: &[String], found_title: &str, found_authors: &[String]) -> Option<u32> {
    let (ours, theirs) = (normalize(title), normalize(found_title));

    if ours.is_empty() || numbers(&ours) != numbers(&theirs) {
        return None;
    }

    let title_score = if ours == theirs {
        3
    } else if theirs.starts_with(&format!("{ours} ")) || ours.starts_with(&format!("{theirs} ")) {
        2
    } else {
        return None;
    };

    if authors.is_empty() {
        return (title_score == 3).then_some(3);
    }

    let theirs: Vec<String> = found_authors.iter().map(|a| normalize(a)).collect();
    let by_them = authors.iter().any(|a| {
        let a = normalize(a);
        a.split(' ').filter(|w| w.chars().count() >= 2).any(|w| theirs.iter().any(|t| t.split(' ').any(|x| x == w)))
            || theirs.iter().any(|t| t == &a)
    });

    by_them.then_some(title_score + 2)
}

#[derive(Deserialize)]
struct AppleResults {
    results: Vec<AppleBook>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppleBook {
    track_id: i64,
    track_name: String,
    artist_name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
    release_date: Option<String>,
    artwork_url100: Option<String>,
    average_user_rating: Option<f64>,
    user_rating_count: Option<i64>,
}

impl AppleBook {
    fn found(self) -> Found {
        Found {
            provider: "apple-books",
            id: self.track_id.to_string(),
            overview: self.description.map(|d| plain(&d)).filter(|d| !d.is_empty()),
            subjects: self.genres.into_iter().filter(|g| g != "Books").take(6).collect(),
            cover: self.artwork_url100.map(|u| u.replace("100x100bb", "1000x1500bb")),
            date: self.release_date.map(|d| d.chars().take(10).collect()),
            rating: self.average_user_rating.filter(|_| self.user_rating_count.unwrap_or(0) >= 3).map(|r| r * 2.0),
            pages: None,
        }
    }
}

#[derive(Deserialize)]
struct OpenLibraryResults {
    docs: Vec<OpenLibraryBook>,
}

#[derive(Deserialize)]
struct OpenLibraryBook {
    key: String,
    title: String,
    #[serde(default)]
    author_name: Vec<String>,
    first_publish_year: Option<i64>,
    cover_i: Option<i64>,
    #[serde(default)]
    subject: Vec<String>,
    number_of_pages_median: Option<i64>,
    ratings_average: Option<f64>,
    ratings_count: Option<i64>,
}

#[derive(Deserialize)]
struct OpenLibraryWork {
    description: Option<Description>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Description {
    Text(String),
    Typed { value: String },
}

fn country(state: &AppState) -> String {
    let language = state.config.current().metadata.language.clone();
    language.split(['-', '_']).nth(1).filter(|c| c.len() == 2).unwrap_or("us").to_lowercase()
}

async fn apple(state: &AppState, book: &BookRow) -> anyhow::Result<Option<Found>> {
    let authors = book.authors();
    let country = country(state);

    if let Some(isbn) = &book.isbn {
        let url = format!("https://itunes.apple.com/lookup?isbn={isbn}&country={country}");
        let r: AppleResults = state.http.get(url).send().await?.error_for_status()?.json().await?;

        if let Some(b) = r.results.into_iter().next() {
            return Ok(Some(b.found()));
        }
    }

    let term = [book.title.as_str(), authors.first().map(String::as_str).unwrap_or_default()].join(" ");
    let mut url = url::Url::parse("https://itunes.apple.com/search")?;
    url.query_pairs_mut()
        .append_pair("term", term.trim())
        .append_pair("media", "ebook")
        .append_pair("entity", "ebook")
        .append_pair("country", &country)
        .append_pair("limit", "15");

    let r: AppleResults = state.http.get(url).send().await?.error_for_status()?.json().await?;

    Ok(r.results
        .into_iter()
        .filter_map(|b| {
            let s = score(&book.title, &authors, &b.track_name, &b.artist_name.iter().cloned().collect::<Vec<_>>())?;
            Some((s, b))
        })
        .max_by_key(|(s, _)| *s)
        .map(|(_, b)| b.found()))
}

async fn open_library(state: &AppState, book: &BookRow) -> anyhow::Result<Option<Found>> {
    let authors = book.authors();
    let mut url = url::Url::parse("https://openlibrary.org/search.json")?;

    match &book.isbn {
        Some(isbn) => {
            url.query_pairs_mut().append_pair("isbn", isbn);
        },
        None => {
            url.query_pairs_mut().append_pair("title", &book.title);

            if let Some(a) = authors.first() {
                url.query_pairs_mut().append_pair("author", a);
            }
        },
    }

    url.query_pairs_mut().append_pair("limit", "10").append_pair(
        "fields",
        "key,title,author_name,first_publish_year,cover_i,subject,number_of_pages_median,ratings_average,ratings_count",
    );

    let r: OpenLibraryResults = state.http.get(url).send().await?.error_for_status()?.json().await?;

    let best = r
        .docs
        .into_iter()
        .filter_map(|d| {
            let s = if book.isbn.is_some() { 9 } else { score(&book.title, &authors, &d.title, &d.author_name)? };
            Some((s, d))
        })
        .max_by_key(|(s, _)| *s)
        .map(|(_, d)| d);

    let Some(d) = best else { return Ok(None) };

    let work: Option<OpenLibraryWork> =
        async { state.http.get(format!("https://openlibrary.org{}.json", d.key)).send().await.ok()?.json().await.ok() }
            .await;

    let overview = work.and_then(|w| w.description).map(|d| match d {
        Description::Text(t) | Description::Typed { value: t } => t,
    });

    Ok(Some(Found {
        provider: "open-library",
        id: d.key.trim_start_matches("/works/").to_string(),
        overview: overview.map(|o| strip_links(&o)).filter(|o| !o.is_empty()),
        subjects: super::genres(&d.subject),
        cover: d.cover_i.map(|c| format!("https://covers.openlibrary.org/b/id/{c}-L.jpg")),
        date: d.first_publish_year.map(|y| y.to_string()),
        rating: d.ratings_average.filter(|_| d.ratings_count.unwrap_or(0) >= 3).map(|r| r * 2.0),
        pages: d.number_of_pages_median,
    }))
}

/// Open Library's descriptions end in Markdown links to where they came from, and the like.
fn strip_links(s: &str) -> String {
    let s = s.split("\r\n----------").next().unwrap_or(s).split("\n----------").next().unwrap_or(s);
    let link = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    link.replace_all(s, "$1").trim().to_string()
}

pub async fn lookup(state: &AppState, book: &BookRow) -> anyhow::Result<Option<Found>> {
    let found = match apple(state, book).await {
        Ok(Some(f)) => Some(f),
        Ok(None) => None,
        Err(e) => {
            tracing::debug!("Apple Books on {:?}: {e:#}", book.title);
            None
        },
    };

    match found {
        Some(f) if f.overview.is_some() || book.description.is_some() => Ok(Some(f)),
        found => match open_library(state, book).await? {
            Some(mut ol) => {
                if let Some(apple) = found {
                    ol.cover = apple.cover.or(ol.cover);
                    ol.rating = apple.rating.or(ol.rating);
                }
                Ok(Some(ol))
            },
            None => Ok(found),
        },
    }
}

async fn save(state: &AppState, id: i64, found: Option<Found>) -> sqlx::Result<()> {
    let f = found.unwrap_or_default();
    let provider = (!f.provider.is_empty()).then_some(f.provider);

    sqlx::query(
        "UPDATE books SET provider = ?, provider_id = ?, overview = ?, subjects = ?, online_cover = ?, online_date = ?,
            rating = ?, pages = ?, fetched_at = ? WHERE id = ?",
    )
    .bind(provider)
    .bind(provider.map(|_| f.id))
    .bind(f.overview)
    .bind(serde_json::to_string(&f.subjects).unwrap_or_else(|_| "[]".into()))
    .bind(f.cover)
    .bind(f.date)
    .bind(f.rating)
    .bind(f.pages)
    .bind(now())
    .bind(id)
    .execute(&state.db)
    .await?;

    Ok(())
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut changed: HashSet<String> = HashSet::new();
        let mut since = 0;

        loop {
            let next: Option<i64> =
                sqlx::query_scalar("SELECT id FROM books WHERE fetched_at IS NULL ORDER BY added_at DESC, id LIMIT 1")
                    .fetch_optional(&state.db)
                    .await
                    .unwrap_or(None);

            let book = match next {
                Some(id) => catalog::book(&state.db, id).await.ok().flatten(),
                None => None,
            };

            let Some(book) = book else {
                for library in changed.drain() {
                    state.events.send(Event::LibraryChanged { library });
                }

                tokio::select! {
                    _ = state.books.online.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(3600)) => {}
                }

                continue;
            };

            match lookup(&state, &book).await {
                Ok(found) => {
                    match &found {
                        Some(f) => tracing::info!("found {:?} on {} ({})", book.title, f.provider, f.id),
                        None => tracing::info!("found nothing online about {:?}", book.title),
                    }

                    if let Err(e) = save(&state, book.id, found).await {
                        tracing::warn!("saving what was found about {:?}: {e:#}", book.title);
                    }

                    changed.insert(book.library.clone());
                    since += 1;

                    // Every so often, so a big library shows what's been found as it goes.
                    if since >= 10 {
                        since = 0;

                        for library in changed.drain() {
                            state.events.send(Event::LibraryChanged { library });
                        }
                    }

                    // Apple's search takes about twenty a minute.
                    tokio::time::sleep(Duration::from_secs(3)).await;
                },
                Err(e) => {
                    tracing::warn!("looking up {:?} online: {e:#}; trying again later", book.title);

                    tokio::select! {
                        _ = state.books.online.wake.notified() => {}
                        _ = tokio::time::sleep(Duration::from_secs(900)) => {}
                    }
                },
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t: &str, a: &[&str], ft: &str, fa: &[&str]) -> Option<u32> {
        let a: Vec<String> = a.iter().map(|x| x.to_string()).collect();
        let fa: Vec<String> = fa.iter().map(|x| x.to_string()).collect();
        score(t, &a, ft, &fa)
    }

    #[test]
    fn matches_titles_carefully() {
        assert_eq!(
            s("A Wizard of Earthsea", &["Ursula K. Le Guin"], "A Wizard of Earthsea", &["Ursula K. Le Guin"]),
            Some(5)
        );
        assert_eq!(s("Dune", &["Frank Herbert"], "Dune: Deluxe Edition", &["Frank Herbert"]), Some(4));
        assert_eq!(
            s("Bookworm: Part 1 Volume 2", &["Miya Kazuki"], "Bookworm: Part 1 Volume 3", &["Miya Kazuki"]),
            None
        );
        assert_eq!(s("Dune", &["Frank Herbert"], "Dune", &["Someone Else"]), None);
        assert_eq!(s("Dune", &[], "Dune Messiah", &["Frank Herbert"]), None);
        assert_eq!(s("It's Over", &["Kazuki Miya"], "Its Over", &["Miya Kazuki"]), Some(5));
    }

    #[test]
    fn strips_open_library_sources() {
        assert_eq!(strip_links("A [book](https://x) about it.\n----------\nAlso see"), "A book about it.");
    }
}
