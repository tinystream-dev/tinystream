// SPDX-License-Identifier: AGPL-3.0-or-later

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use quick_xml::events::Event as Xml;
use serde::Serialize;

use super::release::parse_size;
use crate::config::{Source, SourceKind};

#[derive(Debug, Clone, Serialize, serde::Deserialize, async_graphql::SimpleObject, async_graphql::InputObject)]
#[graphql(input_name = "ReleaseInput")]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub title: String,
    pub source: String,

    pub link: String,
    pub info_hash: Option<String>,
    pub size: Option<i64>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,

    pub published: Option<i64>,

    pub page: Option<String>,
}

pub enum Torrent {
    Magnet(String),
    File(Vec<u8>),
}

#[derive(Debug, Serialize, async_graphql::SimpleObject)]
#[graphql(name = "DetectedSource")]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub kind: SourceKind,
    pub url: String,
    pub feed: Option<String>,
    pub name: Option<String>,
    pub searchable: bool,
    pub sample: Vec<Release>,
}

pub struct Sources {
    http: reqwest::Client,
    last_request: Mutex<HashMap<String, Instant>>,
}

const SPACING: Duration = Duration::from_millis(1500);

impl Sources {
    pub fn new() -> Self {
        let policy = reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() == "magnet" || attempt.previous().len() > 5 {
                attempt.stop()
            } else {
                attempt.follow()
            }
        });

        let http = reqwest::Client::builder()
            .user_agent(concat!("tinystream/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(25))
            .redirect(policy)
            .build()
            .expect("http client");

        Self { http, last_request: Mutex::new(HashMap::new()) }
    }

    async fn pace(&self, url: &str) {
        let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();

        let wait = {
            let mut last = self.last_request.lock().unwrap();
            let now = Instant::now();
            let next = last.get(&host).map(|t| *t + SPACING).filter(|t| *t > now).unwrap_or(now);
            last.insert(host, next);
            next - now
        };

        tokio::time::sleep(wait).await;
    }

    async fn get_text(&self, url: &str) -> anyhow::Result<String> {
        self.pace(url).await;
        let res = self.http.get(url).send().await.with_context(|| format!("can't reach {}", redact(url)))?;
        let status = res.status();
        let text = res.text().await?;

        if !status.is_success() {
            bail!("{} answered {status}", redact(url));
        }

        Ok(text)
    }

    pub fn can_search(source: &Source) -> bool {
        source.kind == SourceKind::Torznab || source.url.contains("{query}")
    }

    pub async fn search(&self, source: &Source, query: &str) -> anyhow::Result<Vec<Release>> {
        let url = match source.kind {
            SourceKind::Torznab => torznab_url(source, "search", Some(query))?,
            SourceKind::Rss => {
                if !source.url.contains("{query}") {
                    return Ok(Vec::new());
                }
                source.url.replace("{query}", &urlencode(query))
            },
        };

        parse_feed(&self.get_text(&url).await?, &source.name)
    }

    pub async fn feed(&self, source: &Source) -> anyhow::Result<Vec<Release>> {
        let url = match source.kind {
            SourceKind::Torznab => torznab_url(source, "search", None)?,
            SourceKind::Rss => source.feed.clone().unwrap_or_else(|| source.url.replace("{query}", "")),
        };

        parse_feed(&self.get_text(&url).await?, &source.name)
    }

    pub async fn fetch(&self, link: &str) -> anyhow::Result<Torrent> {
        if link.starts_with("magnet:") {
            return Ok(Torrent::Magnet(link.to_string()));
        }

        self.pace(link).await;
        let res = self.http.get(link).send().await.with_context(|| format!("can't download {}", redact(link)))?;

        if res.status().is_redirection()
            && let Some(location) = res.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok())
            && location.starts_with("magnet:")
        {
            return Ok(Torrent::Magnet(location.to_string()));
        }

        let status = res.status();

        if !status.is_success() {
            bail!("{} answered {status}", redact(link));
        }

        let bytes = res.bytes().await?;

        if bytes.first() != Some(&b'd') {
            bail!("{} didn't return a .torrent file", redact(link));
        }

        Ok(Torrent::File(bytes.to_vec()))
    }

    pub async fn detect(&self, url: &str, api_key: Option<&str>) -> anyhow::Result<Detected> {
        let url = url.trim();
        let parsed = url::Url::parse(&url.replace("{query}", "x")).context("that isn't a URL")?;

        if !matches!(parsed.scheme(), "http" | "https") {
            bail!("sources are http:// or https:// URLs");
        }

        if url.contains("{query}") {
            let text = self.get_text(&url.replace("{query}", "")).await?;
            let name = channel_title(&text);
            let sample = parse_feed(&text, "test")?;

            return Ok(Detected {
                kind: SourceKind::Rss,
                url: url.to_string(),
                feed: None,
                name,
                searchable: true,
                sample,
            });
        }

        let probe = Source {
            name: "test".into(),
            kind: SourceKind::Torznab,
            url: url.to_string(),
            feed: None,
            api_key: api_key.map(str::to_string),
            categories: Vec::new(),
            enabled: true,
            download_path: None,
            seeding: None,
        };

        if let Ok(caps_url) = torznab_url(&probe, "caps", None)
            && let Ok(text) = self.get_text(&caps_url).await
        {
            if let Some(error) = torznab_error(&text) {
                bail!("{error}");
            }
            if root_element(&text).as_deref() == Some("caps") {
                let text = self.get_text(&torznab_url(&probe, "search", None)?).await?;

                if let Some(error) = torznab_error(&text) {
                    bail!("{error}");
                }

                let sample = parse_feed(&text, "test")?;
                let name = caps_title(&text).or_else(|| channel_title(&text));

                return Ok(Detected {
                    kind: SourceKind::Torznab,
                    url: url.to_string(),
                    feed: None,
                    name,
                    searchable: true,
                    sample,
                });
            }
        }

        let text = self.get_text(url).await?;

        if !matches!(root_element(&text).as_deref(), Some("rss" | "feed" | "RDF")) {
            bail!("that page isn't an RSS feed or a Torznab API");
        }

        let sample = parse_feed(&text, "test")?;
        let name = channel_title(&text);

        let mut template = parsed.clone();

        let search_param = parsed
            .query_pairs()
            .map(|(k, _)| k.to_string())
            .find(|k| matches!(k.as_str(), "q" | "query" | "search" | "term" | "s" | "keywords"));

        if let Some(param) = search_param {
            let pairs: Vec<(String, String)> =
                parsed.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();

            template.query_pairs_mut().clear();

            for (k, v) in pairs {
                template.query_pairs_mut().append_pair(&k, if k == param { "QUERYHERE" } else { &v });
            }

            let search = template.to_string().replace("QUERYHERE", "{query}");

            return Ok(Detected {
                kind: SourceKind::Rss,
                feed: Some(search.replace("{query}", "")),
                url: search,
                name,
                searchable: true,
                sample,
            });
        }

        Ok(Detected {
            kind: SourceKind::Rss,
            url: url.to_string(),
            feed: Some(url.to_string()),
            name,
            searchable: false,
            sample,
        })
    }
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

pub fn redact(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            let pairs: Vec<(String, String)> = u
                .query_pairs()
                .map(|(k, v)| {
                    let secret =
                        matches!(k.to_ascii_lowercase().as_str(), "apikey" | "api_key" | "passkey" | "key" | "token");

                    (k.to_string(), if secret { "…".to_string() } else { v.to_string() })
                })
                .collect();

            if !pairs.is_empty() {
                u.query_pairs_mut().clear().extend_pairs(pairs);
            }

            u.to_string()
        },
        Err(_) => url.to_string(),
    }
}

fn torznab_url(source: &Source, t: &str, q: Option<&str>) -> anyhow::Result<String> {
    let mut url = url::Url::parse(&source.url).with_context(|| format!("{:?} isn't a URL", source.url))?;

    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("t", t);

        if let Some(q) = q {
            pairs.append_pair("q", q);
        }

        if let Some(key) = source.api_key.as_deref().filter(|k| !k.is_empty()) {
            pairs.append_pair("apikey", key);
        }

        if t == "search" {
            if !source.categories.is_empty() {
                let cats: Vec<String> = source.categories.iter().map(u32::to_string).collect();
                pairs.append_pair("cat", &cats.join(","));
            }

            pairs.append_pair("limit", "100");
        }
    }

    Ok(url.to_string())
}

fn reader(text: &str) -> quick_xml::Reader<&[u8]> {
    let mut r = quick_xml::Reader::from_reader(text.as_bytes());
    r.config_mut().trim_text(false);
    r
}

fn root_element(text: &str) -> Option<String> {
    let mut r = reader(text);

    loop {
        match r.read_event().ok()? {
            Xml::Start(e) | Xml::Empty(e) => {
                return Some(String::from_utf8_lossy(e.local_name().as_ref()).into_owned());
            },
            Xml::Eof => return None,
            _ => {},
        }
    }
}

fn attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.local_name().as_ref() == name.as_bytes()).and_then(|a| {
        #[allow(deprecated)]
        a.unescape_value().ok().map(Cow::into_owned)
    })
}

fn torznab_error(text: &str) -> Option<String> {
    let mut r = reader(text);

    loop {
        match r.read_event().ok()? {
            Xml::Start(e) | Xml::Empty(e) => {
                if e.local_name().as_ref() == b"error" {
                    return Some(attr(&e, "description").unwrap_or_else(|| "the source returned an error".into()));
                }

                return None;
            },
            Xml::Eof => return None,
            _ => {},
        }
    }
}

fn caps_title(text: &str) -> Option<String> {
    let mut r = reader(text);

    loop {
        match r.read_event().ok()? {
            Xml::Start(e) | Xml::Empty(e) if e.local_name().as_ref() == b"server" => return attr(&e, "title"),
            Xml::Eof => return None,
            _ => {},
        }
    }
}

fn channel_title(text: &str) -> Option<String> {
    let mut r = reader(text);
    let mut path: Vec<Vec<u8>> = Vec::new();

    loop {
        match r.read_event().ok()? {
            Xml::Start(e) => path.push(e.local_name().as_ref().to_vec()),
            Xml::End(_) => {
                path.pop();
            },
            Xml::Text(t) if path.last().map(Vec::as_slice) == Some(b"title") && path.len() == 3 => {
                return t.decode().ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            },
            Xml::Eof => return None,
            _ => {},
        }
    }
}

#[derive(Default)]
struct Item {
    fields: HashMap<String, String>,
    attrs: HashMap<String, String>,
    enclosure: Option<(String, Option<i64>)>,
}

pub fn parse_feed(text: &str, source: &str) -> anyhow::Result<Vec<Release>> {
    if let Some(error) = torznab_error(text) {
        bail!("{error}");
    }

    let mut r = reader(text);
    let mut out = Vec::new();
    let mut item: Option<Item> = None;
    let mut field: Option<String> = None;
    let mut buf = String::new();

    loop {
        let event = r.read_event().map_err(|e| anyhow::anyhow!("the feed isn't valid XML: {e}"))?;

        match event {
            Xml::Start(e) if matches!(e.local_name().as_ref(), b"item" | b"entry") => {
                item = Some(Item::default());
            },
            Xml::Start(e) => {
                if let Some(it) = item.as_mut() {
                    let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();

                    if name == "link"
                        && let Some(href) = attr(&e, "href")
                    {
                        it.fields.entry("link".into()).or_insert(href);
                    }

                    field = Some(name);
                    buf.clear();
                }
            },
            Xml::Empty(e) => {
                if let Some(it) = item.as_mut() {
                    match e.local_name().as_ref() {
                        b"enclosure" => {
                            if let Some(url) = attr(&e, "url") {
                                it.enclosure = Some((url, attr(&e, "length").and_then(|l| l.parse().ok())));
                            }
                        },
                        b"attr" => {
                            if let (Some(n), Some(v)) = (attr(&e, "name"), attr(&e, "value")) {
                                it.attrs.insert(n.to_ascii_lowercase(), v);
                            }
                        },
                        b"link" => {
                            if let Some(href) = attr(&e, "href") {
                                it.fields.entry("link".into()).or_insert(href);
                            }
                        },
                        _ => {},
                    }
                }
            },
            Xml::Text(t) => {
                if field.is_some() {
                    buf.push_str(&t.decode().unwrap_or_default());
                }
            },
            Xml::CData(t) => {
                if field.is_some() {
                    buf.push_str(&t.decode().unwrap_or_default());
                }
            },
            Xml::GeneralRef(r) => {
                if field.is_some() {
                    let name = r.decode().unwrap_or_default();

                    let resolved =
                        quick_xml::escape::unescape(&format!("&{name};")).map(Cow::into_owned).unwrap_or_default();

                    buf.push_str(&resolved);
                }
            },
            Xml::End(e) => {
                let name = e.local_name();

                if matches!(name.as_ref(), b"item" | b"entry") {
                    if let Some(it) = item.take()
                        && let Some(release) = to_release(it, source)
                    {
                        out.push(release);
                    }
                } else if let (Some(it), Some(f)) = (item.as_mut(), field.take()) {
                    let value = buf.trim().to_string();

                    if !value.is_empty() {
                        it.fields.entry(f).or_insert(value);
                    }

                    buf.clear();
                }
            },
            Xml::Eof => break,
            _ => {},
        }
    }

    Ok(out)
}

fn to_release(it: Item, source: &str) -> Option<Release> {
    let f = |k: &str| it.fields.get(k).map(String::as_str);
    let a = |k: &str| it.attrs.get(k).map(String::as_str);
    let title = f("title")?.trim().to_string();
    let is_http = |s: &&str| s.starts_with("http://") || s.starts_with("https://");
    let info_hash = a("infohash").or(f("infoHash")).map(|h| h.to_ascii_lowercase());

    let link = it
        .enclosure
        .as_ref()
        .map(|(u, _)| u.as_str())
        .filter(is_http)
        .or(f("link").filter(|l| is_http(l) && !looks_like_page(l, f("guid"))))
        .map(str::to_string)
        .or_else(|| a("magneturl").map(str::to_string))
        .or_else(|| f("link").filter(|l| l.starts_with("magnet:")).map(str::to_string))
        .or_else(|| f("magnetURI").or(f("magnetUri")).map(str::to_string))
        .or_else(|| info_hash.as_ref().map(|h| format!("magnet:?xt=urn:btih:{h}&dn={}", urlencode(&title))))?;

    let size = a("size")
        .and_then(|s| s.parse().ok())
        .or_else(|| f("size").and_then(parse_size))
        .or_else(|| f("contentLength").and_then(|s| s.parse().ok()))
        .or_else(|| it.enclosure.as_ref().and_then(|(_, l)| *l).filter(|l| *l > 0));

    let seeders = a("seeders").or(f("seeders")).and_then(|s| s.parse().ok());

    let leechers = a("peers")
        .and_then(|p| p.parse::<u32>().ok())
        .map(|p| p.saturating_sub(seeders.unwrap_or(0)))
        .or_else(|| a("leechers").or(f("leechers")).and_then(|s| s.parse().ok()));

    let published = f("pubDate")
        .or(f("published"))
        .or(f("updated"))
        .and_then(|d| {
            time::OffsetDateTime::parse(d, &time::format_description::well_known::Rfc2822)
                .or_else(|_| time::OffsetDateTime::parse(d, &time::format_description::well_known::Rfc3339))
                .ok()
        })
        .map(|t| t.unix_timestamp());

    let page = f("comments").or(f("guid").filter(is_http)).filter(|p| !p.ends_with(".torrent")).map(str::to_string);
    Some(Release { title, source: source.to_string(), link, info_hash, size, seeders, leechers, published, page })
}

fn looks_like_page(link: &str, guid: Option<&str>) -> bool {
    !link.contains(".torrent") && !link.contains("download") && guid == Some(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NYAA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss xmlns:atom="http://www.w3.org/2005/Atom" xmlns:nyaa="https://nyaa.si/xmlns/nyaa" version="2.0">
  <channel>
    <title>Nyaa - Home - Torrent File RSS</title>
    <item>
      <title>[SubsPlease] Sousou no Frieren - 05 (1080p) [A1B2C3D4].mkv</title>
      <link>https://example.org/download/1.torrent</link>
      <guid isPermaLink="true">https://example.org/view/1</guid>
      <pubDate>Fri, 06 Oct 2023 15:01:01 -0000</pubDate>
      <nyaa:seeders>1234</nyaa:seeders>
      <nyaa:leechers>56</nyaa:leechers>
      <nyaa:infoHash>ABCDEF0123456789ABCDEF0123456789ABCDEF01</nyaa:infoHash>
      <nyaa:size>1.4 GiB</nyaa:size>
    </item>
  </channel>
</rss>"#;

    const TORZNAB: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:torznab="http://torznab.com/schemas/2015/feed">
<channel><title>Tracker</title>
<item>
  <title>Show.S02E05.1080p.WEB.h264-GROUP &amp; friends</title>
  <guid>https://tracker.example/t/9</guid>
  <link>https://tracker.example/dl/9?passkey=secret</link>
  <enclosure url="https://tracker.example/dl/9?passkey=secret" length="2000000000" type="application/x-bittorrent" />
  <torznab:attr name="seeders" value="10" />
  <torznab:attr name="peers" value="15" />
  <torznab:attr name="size" value="2000000000" />
</item>
</channel></rss>"#;

    #[test]
    fn nyaa() {
        let r = parse_feed(NYAA, "Nyaa").unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].link, "https://example.org/download/1.torrent");
        assert_eq!(r[0].seeders, Some(1234));
        assert_eq!(r[0].size, Some(1503238553));
        assert_eq!(r[0].info_hash.as_deref(), Some("abcdef0123456789abcdef0123456789abcdef01"));
        assert_eq!(r[0].page.as_deref(), Some("https://example.org/view/1"));
        assert!(r[0].published.is_some());
        assert_eq!(channel_title(NYAA).as_deref(), Some("Nyaa - Home - Torrent File RSS"));
    }

    #[test]
    fn torznab() {
        let r = parse_feed(TORZNAB, "T").unwrap();
        assert_eq!(r[0].title, "Show.S02E05.1080p.WEB.h264-GROUP & friends");
        assert_eq!((r[0].seeders, r[0].leechers, r[0].size), (Some(10), Some(5), Some(2000000000)));
        assert_eq!(root_element(TORZNAB).as_deref(), Some("rss"));
        assert!(redact("https://t.example/api?apikey=abc&t=search").contains("apikey=%E2%80%A6"));
    }

    #[test]
    fn errors() {
        assert_eq!(
            torznab_error(r#"<error code="100" description="Invalid API Key"/>"#).as_deref(),
            Some("Invalid API Key")
        );

        assert!(parse_feed(r#"<error code="100" description="Invalid API Key"/>"#, "x").is_err());
    }
}
