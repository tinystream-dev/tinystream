// SPDX-License-Identifier: AGPL-3.0-or-later

//! FictionBook 2: one XML file (or one zipped) with its pictures in it as base64.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::Context;
use base64::Engine;
use quick_xml::Reader;
use quick_xml::events::Event;

use super::Meta;
use super::document::{Builder, Document, Link};
use super::markup::{Dialect, Walker, attr, entity, plain};

pub struct Fb2 {
    pub meta: Meta,
    pub document: Document,
    pub pictures: HashMap<String, (String, Vec<u8>)>,
}

/// The file's text, in whatever encoding its declaration says.
fn text_of(bytes: &[u8]) -> String {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc.decode_without_bom_handling(&bytes[bom..]).0.into_owned();
    }

    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]).to_string();
    let label = regex::Regex::new(r#"encoding\s*=\s*["']([^"']+)["']"#)
        .ok()
        .and_then(|r| r.captures(&head).map(|c| c[1].to_string()));

    match label.and_then(|l| encoding_rs::Encoding::for_label(l.as_bytes())) {
        Some(enc) => enc.decode_without_bom_handling(bytes).0.into_owned(),
        None => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn link(href: &str) -> Option<Link> {
    let href = href.trim();

    if let Some(id) = href.strip_prefix('#') {
        return Some(Link::In(format!("#{id}")));
    }

    (href.starts_with("http://") || href.starts_with("https://")).then(|| Link::Out(href.to_string()))
}

fn genre(code: &str) -> Option<&'static str> {
    Some(match code.trim() {
        "sf" | "sf_social" | "sf_epic" | "sf_space" | "sf_cyberpunk" | "sf_heroic" => "Science Fiction",
        "sf_fantasy" | "fantasy" | "sf_fantasy_city" => "Fantasy",
        "sf_horror" | "horror" => "Horror",
        "sf_action" | "adventure" | "adv_history" => "Adventure",
        "sf_detective" | "det_classic" | "det_police" | "detective" | "det_irony" => "Mystery",
        "thriller" | "det_action" | "det_political" => "Thriller",
        "love" | "love_contemporary" | "love_history" | "love_short" | "love_sf" => "Romance",
        "prose_classic" | "prose_rus_classic" => "Classics",
        "prose_contemporary" | "prose" => "Fiction",
        "humor" | "humor_prose" | "sf_humor" => "Humor",
        "children" | "child_tale" => "Children",
        "poetry" => "Poetry",
        "nonf_biography" => "Biography",
        "sci_history" | "history" => "History",
        _ => return None,
    })
}

pub fn open(path: &Path) -> anyhow::Result<Fb2> {
    let bytes = if path.to_string_lossy().to_lowercase().ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
        let name = zip
            .file_names()
            .flatten()
            .find(|n| n.to_lowercase().ends_with(".fb2"))
            .context("no .fb2 in the zip")?
            .to_string();
        let mut out = Vec::new();
        zip.by_name(&name)?.read_to_end(&mut out)?;
        out
    } else {
        std::fs::read(path)?
    };

    Ok(parse(&text_of(&bytes)))
}

pub fn parse(text: &str) -> Fb2 {
    let mut meta = description(text);
    let mut pictures = HashMap::new();
    let mut r = Reader::from_str(text);
    r.config_mut().trim_text(false);
    let mut binary: Option<(String, String)> = None;
    let mut data = String::new();

    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"binary" => {
                binary = attr(&e, "id").map(|id| (id, attr(&e, "content-type").unwrap_or_else(|| "image/jpeg".into())));
                data.clear();
            },
            Ok(Event::Text(t)) if binary.is_some() => data.push_str(&t.decode().unwrap_or_default()),
            Ok(Event::End(e)) if e.local_name().as_ref() == b"binary" => {
                if let Some((id, mime)) = binary.take() {
                    let clean: String = data.chars().filter(|c| !c.is_whitespace()).collect();

                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(clean) {
                        pictures.insert(id, (mime, bytes));
                    }
                }
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }

    let mut b = Builder::new();
    b.start_file("");
    let resolve = |h: &str| link(h);
    Walker::new(&mut b, Dialect::Fb2, &resolve).walk(text);
    let mut document = b.finish(Vec::new());

    for block in &mut document.blocks {
        if let Some(src) = &mut block.src {
            *src = src.trim_start_matches('#').to_string();
        }
    }

    meta.cover = meta.cover.filter(|c| pictures.contains_key(c.trim_start_matches('#')));
    if meta.cover.is_none() {
        meta.cover = document.images().next().map(str::to_string);
    }

    Fb2 { meta, document, pictures }
}

/// What `<description>` says: the title, its authors, series, cover…
fn description(text: &str) -> Meta {
    let mut r = Reader::from_str(text);
    r.config_mut().trim_text(false);
    let mut meta = Meta::default();
    let mut path: Vec<String> = Vec::new();
    let mut text_buf = String::new();
    let mut author: Vec<(String, String)> = Vec::new();
    let mut annotation: Option<String> = None;

    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_lowercase();

                if path.is_empty() && name == "body" {
                    break;
                }

                if annotation.is_some() {
                    annotation.as_mut().unwrap().push_str(&format!("<{name}>"));
                } else if name == "annotation" && path.last().is_some_and(|p| p == "title-info") {
                    annotation = Some(String::new());
                }

                if name == "author" {
                    author.clear();
                }

                path.push(name);
                text_buf.clear();
            },
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_lowercase();
                let parent = path.last().map(String::as_str).unwrap_or_default();

                match name.as_str() {
                    "sequence" if parent == "title-info" && meta.series.is_none() => {
                        meta.series = attr(&e, "name").filter(|n| !n.trim().is_empty());
                        meta.series_index = attr(&e, "number").and_then(|n| n.trim().parse().ok());
                    },
                    "image" if parent == "coverpage" => {
                        meta.cover = attr(&e, "href").map(|h| h.trim_start_matches('#').to_string());
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(t)) => {
                let s = t.decode().unwrap_or_default();
                text_buf.push_str(&s);
                if let Some(a) = &mut annotation {
                    a.push_str(&quick_xml::escape::escape(s.as_ref()));
                }
            },
            Ok(Event::GeneralRef(g)) => {
                let s = entity(&g.decode().unwrap_or_default()).unwrap_or_default();
                text_buf.push_str(&s);
                if let Some(a) = &mut annotation {
                    a.push_str(&quick_xml::escape::escape(s.as_str()));
                }
            },
            Ok(Event::End(_)) => {
                let Some(name) = path.pop() else { break };
                let parent = path.last().map(String::as_str).unwrap_or_default();
                let value = text_buf.trim().to_string();

                match (parent, name.as_str()) {
                    ("title-info", "annotation") => {
                        meta.description = annotation.take().map(|a| plain(&a)).filter(|a| !a.is_empty());
                    },
                    _ if annotation.is_some() => annotation.as_mut().unwrap().push_str(&format!("</{name}>")),
                    ("title-info", "book-title") => meta.title = value,
                    ("title-info", "genre") => meta.genres.extend(genre(&value).map(str::to_string)),
                    ("title-info", "lang") => meta.language = Some(value).filter(|v| !v.is_empty()),
                    ("title-info", "date") if meta.published.is_none() => {
                        meta.published = Some(value).filter(|v| !v.is_empty())
                    },
                    ("author", "first-name" | "middle-name" | "last-name" | "nickname") => {
                        author.push((name.clone(), value))
                    },
                    ("title-info", "author") => {
                        let part =
                            |p: &str| author.iter().find(|(n, _)| n == p).map(|(_, v)| v.as_str()).unwrap_or_default();
                        let full = [part("first-name"), part("middle-name"), part("last-name")]
                            .into_iter()
                            .filter(|p| !p.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let full = if full.is_empty() { part("nickname").to_string() } else { full };

                        if !full.is_empty() {
                            meta.authors.push(full);
                        }
                    },
                    ("publish-info", "publisher") => meta.publisher = Some(value).filter(|v| !v.is_empty()),
                    ("publish-info", "year") if meta.published.is_none() => {
                        meta.published = Some(value).filter(|v| !v.is_empty())
                    },
                    ("publish-info", "isbn") => meta.isbn = super::epub::isbn(&value),
                    _ => {},
                }

                text_buf.clear();
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }

    meta.genres.dedup();
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::books::document::Kind;

    const BOOK: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<description><title-info><genre>sf_fantasy</genre><author><first-name>Ursula</first-name><last-name>Le Guin</last-name></author>
<book-title>A Wizard of Earthsea</book-title><annotation><p>A boy &amp; his shadow.</p></annotation>
<coverpage><image l:href="#cover.jpg"/></coverpage><lang>en</lang><sequence name="Earthsea" number="1"/></title-info>
<publish-info><isbn>978-0-306-40615-7</isbn></publish-info></description>
<body><title><p>A Wizard of Earthsea</p></title>
<section><title><p>Warriors in the Mist</p></title><p>The island of <emphasis>Gont</emphasis>.<a l:href="#n1" type="note">1</a></p>
<section><title><p>Part</p></title><p>Deeper.</p></section></section>
<section><title><p>The Shadow</p></title><empty-line/><p>Next.</p></section></body>
<body name="notes"><section id="n1"><title><p>1</p></title><p>A note.</p></section></body>
<binary id="cover.jpg" content-type="image/jpeg">aGVsbG8=</binary></FictionBook>"##;

    #[test]
    fn reads_a_fictionbook() {
        let book = parse(BOOK);

        assert_eq!(book.meta.title, "A Wizard of Earthsea");
        assert_eq!(book.meta.authors, ["Ursula Le Guin"]);
        assert_eq!(book.meta.series.as_deref(), Some("Earthsea"));
        assert_eq!(book.meta.series_index, Some(1.0));
        assert_eq!(book.meta.description.as_deref(), Some("A boy & his shadow."));
        assert_eq!(book.meta.genres, ["Fantasy"]);
        assert_eq!(book.meta.cover.as_deref(), Some("cover.jpg"));
        assert_eq!(book.pictures["cover.jpg"].1, b"hello");

        let chapters: Vec<(&str, u8)> = book.document.chapters.iter().map(|c| (c.title.as_str(), c.depth)).collect();
        assert_eq!(chapters, [("Warriors in the Mist", 0), ("Part", 1), ("The Shadow", 0), ("Notes", 0)]);

        let doc = &book.document;
        assert_eq!(doc.blocks[0].k, Kind::H);
        assert_eq!(doc.blocks[2].text(), "The island of Gont.1");
        let note = doc.blocks[2].s.iter().find(|s| s.to.is_some()).unwrap();
        assert_eq!(doc.blocks[note.to.unwrap() as usize].text(), "1");
    }
}
