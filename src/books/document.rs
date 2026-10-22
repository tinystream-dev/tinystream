// SPDX-License-Identifier: AGPL-3.0-or-later

//! A book as the readers show it: a flat list of blocks of styled text, with
//! where each one starts counted in characters. Positions are kept in those
//! characters, so they mean the same thing on every screen and font size.

use std::collections::HashMap;

use serde::Serialize;

pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;
pub const UNDERLINE: u8 = 4;
pub const STRIKE: u8 = 8;
pub const SUP: u8 = 16;
pub const SUB: u8 = 32;
pub const CODE: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    P,
    H,
    Quote,
    Pre,
    Li,
    Img,
    Hr,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Span {
    pub t: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub f: u8,
    /// A link out of the book.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// A link to another block in it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub k: Kind,
    /// Characters before this block.
    pub at: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub s: Vec<Span>,
    /// A heading's level, or how deep a list item or quote is.
    #[serde(skip_serializing_if = "is_zero")]
    pub l: u8,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub c: bool,
    /// An image's path in the book.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

impl Block {
    pub fn len(&self) -> u64 {
        match self.k {
            Kind::Img => 1,
            _ => self.s.iter().map(|s| s.t.chars().count() as u64).sum(),
        }
    }

    pub fn text(&self) -> String {
        self.s.iter().map(|s| s.t.as_str()).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Chapter {
    pub title: String,
    pub block: u32,
    pub at: u64,
    pub depth: u8,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Document {
    /// Characters in all, which positions count up to.
    pub length: u64,
    pub chapters: Vec<Chapter>,
    pub blocks: Vec<Block>,
}

impl Document {
    pub fn images(&self) -> impl Iterator<Item = &str> {
        self.blocks.iter().filter_map(|b| b.src.as_deref())
    }

    /// The text from character `start` to `end`, its blocks a line apart (as
    /// `packages/shared/src/books.ts` cuts it too).
    pub fn text(&self, start: u64, end: u64) -> String {
        let mut out = String::new();

        for b in self.blocks.iter().filter(|b| b.k != Kind::Img) {
            let to = b.at + b.len();

            if to <= start || b.at >= end {
                continue;
            }

            let (from, until) = (start.saturating_sub(b.at) as usize, (end.min(to) - b.at) as usize);
            let part: String = b.text().chars().skip(from).take(until - from).collect();

            if part.is_empty() {
                continue;
            }

            if !out.is_empty() {
                out.push_str("\n\n");
            }

            out.push_str(&part);
        }

        out
    }

    /// Where `text` (cut as `text` cuts it) is in the book, nearest to character `near`.
    pub fn find(&self, text: &str, near: u64) -> Option<(u64, u64)> {
        if text.is_empty() {
            return None;
        }

        let mut all = String::new();
        let mut starts: Vec<(usize, u64)> = Vec::new();

        for b in self.blocks.iter().filter(|b| b.k != Kind::Img) {
            let t = b.text();

            if t.is_empty() {
                continue;
            }

            if !all.is_empty() {
                all.push_str("\n\n");
            }

            starts.push((all.len(), b.at));
            all.push_str(&t);
        }

        let place = |byte: usize| {
            let i = starts.partition_point(|&(s, _)| s <= byte).saturating_sub(1);
            let (s, at) = starts[i];
            at + all[s..byte].chars().count() as u64
        };

        all.match_indices(text)
            .take(1000)
            .map(|(i, m)| (place(i), place(i + m.len())))
            .min_by_key(|&(start, _)| start.abs_diff(near))
    }
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// What a block of text is, from the elements it's in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub kind: Kind,
    pub level: u8,
    pub centered: bool,
}

impl Default for Shape {
    fn default() -> Self {
        Self { kind: Kind::P, level: 0, centered: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Out(String),
    /// A file in the book, and maybe an id in it.
    In(String),
}

/// Puts a document together from text as it's read, file by file.
#[derive(Default)]
pub struct Builder {
    doc: Document,
    spans: Vec<Span>,
    links: Vec<Option<String>>,
    space: bool,
    file: String,
    /// `file` and `file#id`, to the block they start at.
    anchors: HashMap<String, u32>,
    waiting: Vec<String>,
    pending: Vec<(u32, usize, String)>,
    /// Each file's first heading, for books whose contents are missing.
    headings: Vec<(String, u32)>,
    file_headed: bool,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn blocks(&self) -> usize {
        self.doc.blocks.len()
    }

    pub fn first_image(&self) -> Option<String> {
        self.doc.images().next().map(str::to_string)
    }

    pub fn start_file(&mut self, file: &str) {
        self.file = file.to_string();
        self.file_headed = false;
        self.waiting.push(file.to_string());
    }

    pub fn anchor(&mut self, id: &str) {
        self.waiting.push(format!("{}#{id}", self.file));
    }

    pub fn text(&mut self, s: &str, flags: u8, link: Option<&Link>) {
        for ch in s.chars() {
            if ch.is_whitespace() && ch != '\u{a0}' {
                self.space = true;
                continue;
            }

            if self.space
                && !self.ends_with_break()
                && let Some(last) = self.spans.last_mut()
            {
                last.t.push(' ');
            }

            self.space = false;
            self.push(ch, flags, link);
        }
    }

    /// Text whose spaces and line breaks are kept.
    pub fn raw(&mut self, s: &str, flags: u8, link: Option<&Link>) {
        self.space = false;

        for ch in s.chars() {
            if ch != '\r' {
                self.push(ch, flags, link);
            }
        }
    }

    pub fn line_break(&mut self) {
        if !self.spans.is_empty() {
            self.space = false;
            self.push('\n', 0, None);
        }
    }

    fn ends_with_break(&self) -> bool {
        self.spans.last().is_some_and(|s| s.t.ends_with('\n'))
    }

    fn push(&mut self, ch: char, flags: u8, link: Option<&Link>) {
        let href = link.map(|l| match l {
            Link::Out(u) => format!("out:{u}"),
            Link::In(t) => t.clone(),
        });

        match (self.spans.last_mut(), self.links.last()) {
            (Some(last), Some(l)) if last.f == flags && *l == href => last.t.push(ch),
            _ => {
                self.spans.push(Span { t: ch.to_string(), f: flags, href: None, to: None });
                self.links.push(href);
            },
        }
    }

    /// Ends the paragraph being read, if there's anything in it.
    pub fn flush(&mut self, shape: Shape) {
        let spans = std::mem::take(&mut self.spans);
        let links = std::mem::take(&mut self.links);
        self.space = false;

        let mut spans: Vec<(Span, Option<String>)> = spans.into_iter().zip(links).collect();

        while let Some((last, _)) = spans.last_mut() {
            let trimmed = last.t.trim_end_matches(['\n', ' ']).len();
            last.t.truncate(trimmed);

            if !last.t.is_empty() {
                break;
            }

            spans.pop();
        }

        if spans.iter().all(|(s, _)| s.t.chars().all(|c| c.is_whitespace())) {
            return;
        }

        let index = self.doc.blocks.len() as u32;
        let mut out = Vec::with_capacity(spans.len());

        for (i, (mut span, link)) in spans.into_iter().enumerate() {
            match link {
                Some(l) if l.starts_with("out:") => span.href = Some(l[4..].to_string()),
                Some(l) => self.pending.push((index, i, l)),
                None => {},
            }

            out.push(span);
        }

        if shape.kind == Kind::H && !self.file_headed {
            self.file_headed = true;
            self.headings.push((out.iter().map(|s| s.t.as_str()).collect(), index));
        }

        self.add(Block { k: shape.kind, at: 0, s: out, l: shape.level, c: shape.centered, src: None, alt: None });
    }

    pub fn image(&mut self, src: String, alt: Option<String>, shape: Shape) {
        self.flush(shape);
        let alt = alt.map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
        self.add(Block { k: Kind::Img, at: 0, s: Vec::new(), l: 0, c: true, src: Some(src), alt });
    }

    pub fn rule(&mut self, shape: Shape) {
        self.flush(shape);

        if self.doc.blocks.last().is_some_and(|b| b.k != Kind::Hr) {
            self.add(Block { k: Kind::Hr, at: 0, s: Vec::new(), l: 0, c: false, src: None, alt: None });
        }
    }

    fn add(&mut self, mut block: Block) {
        let index = self.doc.blocks.len() as u32;

        for a in self.waiting.drain(..) {
            self.anchors.entry(a).or_insert(index);
        }

        block.at = self.doc.length;
        self.doc.length += block.len();
        self.doc.blocks.push(block);
    }

    /// A chapter starting at `block` (one of the blocks already added, or the next).
    pub fn chapter(&mut self, title: &str, block: usize, depth: u8) {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");

        if !title.is_empty() {
            self.doc.chapters.push(Chapter { title, block: block as u32, at: 0, depth });
        }
    }

    /// The finished document; `toc` is its contents, as titles and links into its files.
    pub fn finish(mut self, toc: Vec<(String, String, u8)>) -> Document {
        let end = self.doc.blocks.len() as u32;

        for (block, span, target) in std::mem::take(&mut self.pending) {
            self.doc.blocks[block as usize].s[span].to = self.find(&target);
        }

        if self.doc.chapters.is_empty() {
            for (title, target, depth) in toc {
                if let Some(block) = self.find(&target) {
                    self.chapter(&title, block as usize, depth);
                }
            }
        }

        if self.doc.chapters.is_empty() {
            for (title, block) in std::mem::take(&mut self.headings) {
                self.chapter(&title, block as usize, 0);
            }
        }

        self.doc.chapters.retain(|c| c.block < end);
        self.doc.chapters.sort_by_key(|c| c.block);

        for c in &mut self.doc.chapters {
            c.at = self.doc.blocks[c.block as usize].at;
        }

        self.doc
    }

    fn find(&self, target: &str) -> Option<u32> {
        self.anchors.get(target).or_else(|| self.anchors.get(target.split('#').next()?)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_spaces_and_counts_characters() {
        let mut b = Builder::new();
        b.start_file("a.xhtml");
        b.text("  Hello,\n   ", 0, None);
        b.text("world", ITALIC, None);
        b.text("!  ", 0, None);
        b.flush(Shape::default());
        b.text("Two", 0, None);
        b.flush(Shape::default());
        let doc = b.finish(vec![("One".into(), "a.xhtml".into(), 0)]);

        assert_eq!(doc.blocks[0].text(), "Hello, world!");
        assert_eq!(doc.blocks[0].s[1].t, "world");
        assert_eq!(doc.blocks[1].at, 13);
        assert_eq!(doc.length, 16);
        assert_eq!(doc.chapters[0].title, "One");
    }

    #[test]
    fn links_inside_the_book_go_to_blocks() {
        let mut b = Builder::new();
        b.start_file("a.xhtml");
        b.text("See ", 0, None);
        b.text("note", 0, Some(&Link::In("b.xhtml#n1".into())));
        b.flush(Shape::default());
        b.start_file("b.xhtml");
        b.text("Intro", 0, None);
        b.flush(Shape::default());
        b.anchor("n1");
        b.text("The note", 0, None);
        b.flush(Shape::default());
        let doc = b.finish(Vec::new());

        assert_eq!(doc.blocks[0].s[1].to, Some(2));
    }

    #[test]
    fn cuts_and_finds_quotes() {
        let mut b = Builder::new();
        b.text("Call me Ishmael.", 0, None);
        b.flush(Shape::default());
        b.image("whale.png".into(), None, Shape::default());
        b.text("Some years ago, ", 0, None);
        b.text("never mind", ITALIC, None);
        b.text(" how long.", 0, None);
        b.flush(Shape::default());
        b.text("Call me Ishmael.", 0, None);
        b.flush(Shape::default());
        let doc = b.finish(Vec::new());

        let quote = doc.text(8, 31);
        assert_eq!(quote, "Ishmael.\n\nSome years ago");
        assert_eq!(doc.find(&quote, 0), Some((8, 31)));
        assert_eq!(doc.find("Call me", 60), Some((53, 60)));
        assert_eq!(doc.find("Call me", 3), Some((0, 7)));
        assert_eq!(doc.find("Ahab", 0), None);
    }

    #[test]
    fn skips_empty_paragraphs() {
        let mut b = Builder::new();
        b.text(" \u{a0} ", 0, None);
        b.flush(Shape::default());
        b.line_break();
        b.flush(Shape::default());
        assert!(b.finish(Vec::new()).blocks.is_empty());
    }
}
