// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod catalog;
pub mod document;
pub mod epub;
pub mod fb2;
pub mod markup;
pub mod online;
pub mod txt;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use bytes::Bytes;
use document::Document;

use crate::auth::User;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Epub,
    Fb2,
    Txt,
}

impl Format {
    pub fn of(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().to_lowercase();

        if name.ends_with(".epub") || name.ends_with(".kepub") {
            Some(Self::Epub)
        } else if name.ends_with(".fb2") || name.ends_with(".fb2.zip") {
            Some(Self::Fb2)
        } else if name.ends_with(".txt") {
            Some(Self::Txt)
        } else {
            None
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Epub => "epub",
            Self::Fb2 => "fb2",
            Self::Txt => "txt",
        }
    }
}

/// What a book says about itself.
#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub title: String,
    pub sort_title: Option<String>,
    pub authors: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub published: Option<String>,
    pub language: Option<String>,
    pub isbn: Option<String>,
    pub genres: Vec<String>,
    /// Where its cover is in it.
    pub cover: Option<String>,
}

/// Subjects as genres: library headings ("Whales -- Fiction") cut down, and
/// the ones about people and places dropped.
pub fn genres(subjects: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for s in subjects.iter().flat_map(|s| s.split([';', '/'])) {
        let s = s.split(" -- ").next().unwrap_or_default().trim();

        if s.is_empty() || s.len() > 32 || s.contains(['(', ',']) {
            continue;
        }

        let mut s = s.to_string();
        if let Some(first) = s.get(..1) {
            s.replace_range(..1, &first.to_uppercase());
        }

        if !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
    }

    out.truncate(6);
    out
}

/// A book, read through.
pub struct Book {
    pub format: Format,
    pub meta: Meta,
    pub document: Document,
    /// A FictionBook's pictures, which are in it rather than beside it.
    pictures: HashMap<String, (String, Vec<u8>)>,
    json: OnceLock<Bytes>,
}

impl Book {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let format = Format::of(path).ok_or_else(|| anyhow::anyhow!("not a book"))?;

        let mut book = match format {
            Format::Epub => {
                let mut epub = epub::Epub::open(path)?;
                let document = epub.document();
                Self { format, meta: epub.meta, document, pictures: HashMap::new(), json: OnceLock::new() }
            },
            Format::Fb2 => {
                let fb2 = fb2::open(path)?;
                Self { format, meta: fb2.meta, document: fb2.document, pictures: fb2.pictures, json: OnceLock::new() }
            },
            Format::Txt => {
                let (meta, document) = txt::open(path)?;
                Self { format, meta, document, pictures: HashMap::new(), json: OnceLock::new() }
            },
        };

        if book.meta.title.trim().is_empty() {
            book.meta.title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        }

        book.meta.genres = genres(&book.meta.genres);
        Ok(book)
    }

    /// The document as the readers take it, made once.
    pub fn json(&self) -> Bytes {
        self.json.get_or_init(|| serde_json::to_vec(&self.document).unwrap_or_default().into()).clone()
    }

    /// A file in the book (a picture, mostly), and what it is.
    pub fn resource(&self, path: &Path, name: &str) -> Option<(String, Vec<u8>)> {
        match self.format {
            Format::Epub => {
                let data = epub::Epub::open(path).ok()?.resource(name)?;
                Some((mime_guess::from_path(name).first_or_octet_stream().to_string(), data))
            },
            Format::Fb2 => self.pictures.get(name).cloned(),
            Format::Txt => None,
        }
    }
}

/// The picture on a book's cover, read without reading the rest of it where that can be helped.
pub fn cover(path: &Path) -> Option<(String, Vec<u8>)> {
    match Format::of(path)? {
        Format::Epub => {
            let mut epub = epub::Epub::open(path).ok()?;
            let name = match epub.meta.cover.clone() {
                Some(name) => name,
                None => {
                    epub.document();
                    epub.meta.cover.clone()?
                },
            };
            let data = epub.resource(&name)?;
            Some((mime_guess::from_path(&name).first_or_octet_stream().to_string(), data))
        },
        Format::Fb2 => {
            let mut fb2 = fb2::open(path).ok()?;
            fb2.pictures.remove(&fb2.meta.cover?)
        },
        Format::Txt => None,
    }
}

const KEEP: usize = 4;

/// The last few books opened, so turning back to one doesn't read it all again.
#[derive(Default)]
pub struct Books {
    open: Mutex<Vec<(PathBuf, i64, Arc<Book>)>>,
    pub online: online::Lookups,
}

impl Books {
    pub async fn get(&self, path: &Path, mtime: i64) -> anyhow::Result<Arc<Book>> {
        {
            let mut open = self.open.lock().unwrap();

            if let Some(i) = open.iter().position(|(p, m, _)| p == path && *m == mtime) {
                let entry = open.remove(i);
                let book = entry.2.clone();
                open.push(entry);
                return Ok(book);
            }
        }

        let p = path.to_path_buf();
        let book = Arc::new(tokio::task::spawn_blocking(move || Book::read(&p)).await??);
        let mut open = self.open.lock().unwrap();
        open.retain(|(p, ..)| p != path);
        open.push((path.to_path_buf(), mtime, book.clone()));

        if open.len() > KEEP {
            open.remove(0);
        }

        Ok(book)
    }
}

/// The book libraries someone can see.
pub fn libraries(state: &AppState, user: &User) -> Vec<String> {
    state
        .config
        .current()
        .libraries
        .iter()
        .filter(|l| l.is_books() && user.permissions.can_see(&l.name))
        .map(|l| l.name.clone())
        .collect()
}

pub fn spawn(state: Arc<AppState>) {
    online::spawn(state);
}

#[cfg(test)]
mod tests {
    #[test]
    fn tidies_subjects() {
        let subjects = [
            "Whaling -- Fiction",
            "Sea stories",
            "Ahab, Captain (Fictitious character) -- Fiction",
            "sea Stories",
            "fantasy",
        ];
        assert_eq!(super::genres(&subjects.map(String::from)), ["Whaling", "Sea stories", "Fantasy"]);
    }
}
