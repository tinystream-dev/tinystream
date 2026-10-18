// SPDX-License-Identifier: AGPL-3.0-or-later

//! EPUB 2 and 3: the package's metadata, its spine in reading order, and its
//! contents from the EPUB 3 navigation document or the EPUB 2 NCX.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Context;
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

use super::Meta;
use super::document::{Builder, Document, Link};
use super::markup::{Dialect, Walker, attr, entity, plain};

struct Item {
    path: String,
    media_type: String,
    properties: String,
}

pub struct Epub {
    zip: ZipArchive<File>,
    pub meta: Meta,
    spine: Vec<String>,
    nav: Option<String>,
    ncx: Option<String>,
}

/// A path in the book, from `href` in a file in `dir`.
pub fn join(dir: &str, href: &str) -> String {
    let href = decode(href.split('#').next().unwrap_or_default());
    let mut parts: Vec<&str> =
        if href.starts_with('/') { Vec::new() } else { dir.split('/').filter(|p| !p.is_empty()).collect() };

    for part in href.split('/') {
        match part {
            "" | "." => {},
            ".." => {
                parts.pop();
            },
            p => parts.push(p),
        }
    }

    parts.join("/")
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// What to do with an `href` in `file`.
pub fn link(file: &str, href: &str) -> Option<Link> {
    let href = href.trim();

    if href.is_empty() {
        return None;
    }

    if let Some((scheme, _)) = href.split_once(':')
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        let scheme = scheme.to_ascii_lowercase();
        return matches!(scheme.as_str(), "http" | "https" | "mailto").then(|| Link::Out(href.to_string()));
    }

    let (path, id) = match href.split_once('#') {
        Some((p, id)) => (p, Some(id)),
        None => (href, None),
    };

    let target = if path.is_empty() { file.to_string() } else { join(dir_of(file), path) };

    Some(Link::In(match id {
        Some(id) if !id.is_empty() => format!("{target}#{}", decode(id)),
        _ => target,
    }))
}

fn read_xml(text: &str) -> Reader<&[u8]> {
    let mut r = Reader::from_str(text);
    let c = r.config_mut();
    c.trim_text(false);
    c.check_end_names = false;
    c.allow_unmatched_ends = true;
    c.allow_dangling_amp = true;
    r
}

fn local(name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name).to_ascii_lowercase();
    name.rsplit_once(':').map(|(_, l)| l.to_string()).unwrap_or(name)
}

pub fn isbn(s: &str) -> Option<String> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x').collect();

    let valid = match digits.len() {
        13 => {
            let sum: u32 = digits
                .chars()
                .enumerate()
                .map(|(i, c)| c.to_digit(10).unwrap_or(99) * if i % 2 == 0 { 1 } else { 3 })
                .sum();
            sum % 10 == 0 && (digits.starts_with("978") || digits.starts_with("979"))
        },
        10 => {
            let sum: u32 = digits
                .chars()
                .enumerate()
                .map(|(i, c)| (10 - i as u32) * if c == 'X' || c == 'x' { 10 } else { c.to_digit(10).unwrap_or(99) })
                .sum();
            sum % 11 == 0
        },
        _ => false,
    };

    valid.then(|| digits.to_ascii_uppercase())
}

impl Epub {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let mut zip = ZipArchive::new(File::open(path)?).context("not a zip file")?;
        let container = read(&mut zip, "META-INF/container.xml").context("no META-INF/container.xml")?;
        let container = String::from_utf8_lossy(&container).into_owned();
        let opf_path = rootfile(&container).context("container.xml names no package")?;
        let opf = read(&mut zip, &opf_path).with_context(|| format!("no {opf_path}"))?;
        let opf = String::from_utf8_lossy(&opf).into_owned();
        let mut book = Self { zip, meta: Meta::default(), spine: Vec::new(), nav: None, ncx: None };
        book.package(&opf, dir_of(&opf_path));
        Ok(book)
    }

    fn package(&mut self, opf: &str, dir: &str) {
        let mut r = read_xml(opf);
        let mut manifest: HashMap<String, Item> = HashMap::new();
        let mut spine_ids = Vec::new();
        let mut toc_id = None;
        let mut cover_id = None;

        // (id, text, role) of each creator, and EPUB 3's refinements of them.
        let mut creators: Vec<(Option<String>, String, Option<String>)> = Vec::new();
        let mut collections: Vec<(Option<String>, String)> = Vec::new();
        let mut refines: Vec<(String, String, String)> = Vec::new();
        let mut identifiers: Vec<(Option<String>, String)> = Vec::new();

        let mut field: Option<(String, Option<String>, Option<String>, Option<String>)> = None;
        let mut text = String::new();
        let mut in_metadata = false;

        loop {
            let event = match r.read_event() {
                Ok(Event::Eof) | Err(_) => break,
                Ok(e) => e,
            };

            match event {
                Event::Start(e) | Event::Empty(e) => {
                    let name = local(e.name().as_ref());

                    match name.as_str() {
                        "metadata" => in_metadata = true,
                        "item" => {
                            let (Some(id), Some(href)) = (attr(&e, "id"), attr(&e, "href")) else { continue };
                            manifest.insert(
                                id,
                                Item {
                                    path: join(dir, &href),
                                    media_type: attr(&e, "media-type").unwrap_or_default(),
                                    properties: attr(&e, "properties").unwrap_or_default(),
                                },
                            );
                        },
                        "spine" => toc_id = attr(&e, "toc"),
                        "itemref" => {
                            if attr(&e, "linear").as_deref() != Some("no")
                                && let Some(id) = attr(&e, "idref")
                            {
                                spine_ids.push(id);
                            }
                        },
                        "meta" if in_metadata => {
                            if let (Some(n), Some(content)) = (attr(&e, "name"), attr(&e, "content")) {
                                match n.as_str() {
                                    "cover" => cover_id = Some(content),
                                    "calibre:series" => self.meta.series = Some(content),
                                    "calibre:series_index" => self.meta.series_index = content.trim().parse().ok(),
                                    "calibre:title_sort" => self.meta.sort_title = Some(content),
                                    _ => {},
                                }
                            } else if let Some(property) = attr(&e, "property") {
                                field = Some((
                                    format!("meta:{property}"),
                                    attr(&e, "id"),
                                    attr(&e, "refines").map(|r| r.trim_start_matches('#').to_string()),
                                    None,
                                ));
                                text.clear();
                            }
                        },
                        _ if in_metadata => {
                            field = Some((name, attr(&e, "id"), attr(&e, "role"), attr(&e, "scheme")));
                            text.clear();
                        },
                        _ => {},
                    }
                },
                Event::Text(t) => text.push_str(&t.decode().unwrap_or_default()),
                Event::CData(t) => text.push_str(&t.decode().unwrap_or_default()),
                Event::GeneralRef(g) => text.push_str(&entity(&g.decode().unwrap_or_default()).unwrap_or_default()),
                Event::End(e) => {
                    let name = local(e.name().as_ref());

                    if name == "metadata" {
                        in_metadata = false;
                        continue;
                    }

                    let Some((f, id, extra, scheme)) = field.take() else { continue };
                    let value = text.trim().to_string();

                    if value.is_empty() {
                        continue;
                    }

                    match f.as_str() {
                        "title" if self.meta.title.is_empty() => self.meta.title = value,
                        "creator" => creators.push((id, value, extra)),
                        "description" if self.meta.description.is_none() => self.meta.description = Some(plain(&value)),
                        "publisher" if self.meta.publisher.is_none() => self.meta.publisher = Some(value),
                        "date" if self.meta.published.is_none() => self.meta.published = Some(value),
                        "language" if self.meta.language.is_none() => self.meta.language = Some(value),
                        "subject" => self.meta.genres.push(value),
                        "identifier" => identifiers.push((scheme, value)),
                        "meta:belongs-to-collection" => collections.push((id, value)),
                        p if p.starts_with("meta:") => {
                            if let Some(target) = extra {
                                refines.push((target, p[5..].to_string(), value));
                            }
                        },
                        _ => {},
                    }
                },
                _ => {},
            }
        }

        let refined = |id: &Option<String>, property: &str| {
            id.as_ref()
                .and_then(|id| refines.iter().find(|(t, p, _)| t == id && p == property).map(|(_, _, v)| v.clone()))
        };

        let roles: Vec<(String, Option<String>)> = creators
            .iter()
            .map(|(id, name, role)| (name.clone(), role.clone().or_else(|| refined(id, "role"))))
            .collect();
        let authors: Vec<String> =
            roles.iter().filter(|(_, r)| r.as_deref().is_none_or(|r| r == "aut")).map(|(n, _)| n.clone()).collect();
        self.meta.authors = if authors.is_empty() { roles.into_iter().map(|(n, _)| n).collect() } else { authors };

        if self.meta.series.is_none()
            && let Some((id, name)) = collections.first()
        {
            self.meta.series = Some(name.clone());
            self.meta.series_index = refined(id, "group-position").and_then(|p| p.trim().parse().ok());
        }

        self.meta.isbn = identifiers
            .iter()
            .filter(|(scheme, v)| {
                scheme.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("isbn"))
                    || v.to_lowercase().contains("isbn")
                    || isbn(v).is_some()
            })
            .find_map(|(_, v)| isbn(v.trim_start_matches(|c: char| !c.is_ascii_digit())));

        self.spine = spine_ids.iter().filter_map(|id| manifest.get(id)).map(|i| i.path.clone()).collect();
        self.nav =
            manifest.values().find(|i| i.properties.split_whitespace().any(|p| p == "nav")).map(|i| i.path.clone());
        self.ncx = toc_id
            .and_then(|id| manifest.get(&id))
            .or_else(|| manifest.values().find(|i| i.media_type == "application/x-dtbncx+xml"))
            .map(|i| i.path.clone());

        let is_image = |i: &&Item| i.media_type.starts_with("image/");
        self.meta.cover = manifest
            .values()
            .find(|i| i.properties.split_whitespace().any(|p| p == "cover-image"))
            .or_else(|| cover_id.as_ref().and_then(|id| manifest.get(id)).filter(is_image))
            .or_else(|| {
                let mut found: Vec<(&String, &Item)> = manifest
                    .iter()
                    .filter(|(id, i)| {
                        is_image(i) && (id.to_lowercase().contains("cover") || i.path.to_lowercase().contains("cover"))
                    })
                    .collect();
                found.sort_by_key(|(_, i)| i.path.len());
                found.first().map(|(_, i)| *i)
            })
            .map(|i| i.path.clone());
    }

    pub fn resource(&mut self, name: &str) -> Option<Vec<u8>> {
        read(&mut self.zip, name)
    }

    fn toc(&mut self) -> Vec<(String, String, u8)> {
        if let Some(nav) = self.nav.clone()
            && let Some(text) = self.resource(&nav)
        {
            let toc = nav_toc(&String::from_utf8_lossy(&text), &nav);

            if !toc.is_empty() {
                return toc;
            }
        }

        match self.ncx.clone() {
            Some(ncx) => match self.resource(&ncx) {
                Some(text) => ncx_toc(&String::from_utf8_lossy(&text), &ncx),
                None => Vec::new(),
            },
            None => Vec::new(),
        }
    }

    pub fn document(&mut self) -> Document {
        let mut b = Builder::new();

        for file in self.spine.clone() {
            let Some(bytes) = self.resource(&file) else {
                tracing::debug!("{file} is in the spine but not in the book");
                continue;
            };

            b.start_file(&file);
            let resolve = |href: &str| link(&file, href);
            Walker::new(&mut b, Dialect::Html, &resolve).walk(&String::from_utf8_lossy(&bytes));
        }

        if self.meta.cover.is_none() {
            self.meta.cover = b.first_image();
        }

        let toc = self.toc();
        b.finish(toc)
    }
}

fn read(zip: &mut ZipArchive<File>, name: &str) -> Option<Vec<u8>> {
    // Some books get the case of their own paths wrong.
    let name = match zip.index_for_name(name) {
        Some(_) => name.to_string(),
        None => zip.file_names().flatten().find(|n| n.eq_ignore_ascii_case(name))?.to_string(),
    };

    let mut file = zip.by_name(&name).ok()?;

    let mut out = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut out).ok()?;
    Some(out)
}

fn rootfile(container: &str) -> Option<String> {
    let mut r = read_xml(container);

    loop {
        match r.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) if local(e.name().as_ref()) == "rootfile" => {
                return attr(&e, "full-path").map(|p| join("", &p));
            },
            Ok(Event::Eof) | Err(_) => return None,
            _ => {},
        }
    }
}

/// The table of contents in an EPUB 3 navigation document: `<nav epub:type="toc">`'s nested lists.
fn nav_toc(text: &str, file: &str) -> Vec<(String, String, u8)> {
    let mut r = read_xml(text);
    let mut out = Vec::new();
    let mut in_toc = false;
    let mut navs = 0;
    let mut depth: u8 = 0;
    let mut current: Option<(String, String)> = None;

    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()).as_str() {
                "nav" => {
                    navs += 1;
                    if !in_toc && attr(&e, "type").is_some_and(|t| t.split_whitespace().any(|t| t == "toc")) {
                        in_toc = true;
                        navs = 1;
                    }
                },
                "ol" if in_toc => depth = depth.saturating_add(1),
                "a" if in_toc => {
                    if let Some(Link::In(target)) = attr(&e, "href").and_then(|h| link(file, &h)) {
                        current = Some((String::new(), target));
                    }
                },
                _ => {},
            },
            Ok(Event::Text(t)) => {
                if let Some((title, _)) = &mut current {
                    title.push_str(&t.decode().unwrap_or_default());
                }
            },
            Ok(Event::GeneralRef(g)) => {
                if let Some((title, _)) = &mut current {
                    title.push_str(&entity(&g.decode().unwrap_or_default()).unwrap_or_default());
                }
            },
            Ok(Event::End(e)) => match local(e.name().as_ref()).as_str() {
                "nav" => {
                    navs -= 1;
                    if in_toc && navs == 0 {
                        break;
                    }
                },
                "ol" if in_toc => depth = depth.saturating_sub(1),
                "a" => {
                    if let Some((title, target)) = current.take() {
                        out.push((title, target, depth.saturating_sub(1)));
                    }
                },
                _ => {},
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }

    out
}

/// The table of contents in an EPUB 2 NCX: its nested `navPoint`s.
fn ncx_toc(text: &str, file: &str) -> Vec<(String, String, u8)> {
    let mut r = read_xml(text);
    let mut out: Vec<(String, String, u8)> = Vec::new();
    let mut depth: u8 = 0;
    let mut label: Option<String> = None;
    let mut reading_label = false;

    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(e.name().as_ref()).as_str() {
                "navpoint" => {
                    depth = depth.saturating_add(1);
                    label = None;
                },
                "text" => {
                    reading_label = true;
                    label.get_or_insert_with(String::new);
                },
                "content" => {
                    if let (Some(title), Some(Link::In(target))) =
                        (label.take(), attr(&e, "src").and_then(|s| link(file, &s)))
                    {
                        out.push((title, target, depth.saturating_sub(1)));
                    }
                },
                _ => {},
            },
            Ok(Event::Text(t)) if reading_label => {
                if let Some(l) = &mut label {
                    l.push_str(&t.decode().unwrap_or_default());
                }
            },
            Ok(Event::GeneralRef(g)) if reading_label => {
                if let Some(l) = &mut label {
                    l.push_str(&entity(&g.decode().unwrap_or_default()).unwrap_or_default());
                }
            },
            Ok(Event::End(e)) => match local(e.name().as_ref()).as_str() {
                "navpoint" => depth = depth.saturating_sub(1),
                "text" => reading_label = false,
                _ => {},
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {},
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_paths_in_the_book() {
        assert_eq!(join("OEBPS/Text", "../Images/a%20b.jpg#x"), "OEBPS/Images/a b.jpg");
        assert_eq!(join("", "content.opf"), "content.opf");
        assert_eq!(link("OEBPS/c1.xhtml", "#n1"), Some(Link::In("OEBPS/c1.xhtml#n1".into())));
        assert_eq!(link("OEBPS/c1.xhtml", "https://x.org"), Some(Link::Out("https://x.org".into())));
        assert_eq!(link("OEBPS/c1.xhtml", "javascript:alert(1)"), None);
    }

    #[test]
    fn checks_isbns() {
        assert_eq!(isbn("978-0-306-40615-7").as_deref(), Some("9780306406157"));
        assert_eq!(isbn("0-306-40615-2").as_deref(), Some("0306406152"));
        assert_eq!(isbn("978-0-306-40615-8"), None);
        assert_eq!(isbn("urn:uuid:1234"), None);
    }

    #[test]
    fn reads_both_kinds_of_contents() {
        let nav = r#"<html><body><nav epub:type="landmarks"><ol><li><a href="x.xhtml">Skip</a></li></ol></nav>
            <nav epub:type="toc"><ol><li><a href="c1.xhtml">One</a><ol><li><a href="c1.xhtml#s2">One &amp; a half</a></li></ol></li>
            <li><a href="c2.xhtml">Two</a></li></ol></nav></body></html>"#;
        assert_eq!(
            nav_toc(nav, "OEBPS/nav.xhtml"),
            [
                ("One".to_string(), "OEBPS/c1.xhtml".to_string(), 0),
                ("One & a half".into(), "OEBPS/c1.xhtml#s2".into(), 1),
                ("Two".into(), "OEBPS/c2.xhtml".into(), 0)
            ]
        );

        let ncx = r#"<ncx><navMap><navPoint id="a"><navLabel><text>One</text></navLabel><content src="c1.xhtml"/>
            <navPoint id="b"><navLabel><text>Inner</text></navLabel><content src="c1.xhtml#i"/></navPoint></navPoint></navMap></ncx>"#;
        assert_eq!(
            ncx_toc(ncx, "toc.ncx"),
            [("One".to_string(), "c1.xhtml".to_string(), 0), ("Inner".into(), "c1.xhtml#i".into(), 1)]
        );
    }
}
