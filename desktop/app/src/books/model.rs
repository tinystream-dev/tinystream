// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Deserialize;

pub const BOLD: u32 = 1;
pub const ITALIC: u32 = 2;
pub const UNDERLINE: u32 = 4;
pub const STRIKE: u32 = 8;
pub const SUP: u32 = 16;
pub const SUB: u32 = 32;
pub const CODE: u32 = 64;
pub const QUOTE_LENGTH: usize = 3000;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Span {
    pub t: String,
    #[serde(default)]
    pub f: u32,
    pub href: Option<String>,
    pub to: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    pub k: String,
    pub at: usize,
    #[serde(default)]
    pub s: Vec<Span>,
    #[serde(default)]
    pub l: usize,
    #[serde(default)]
    pub c: bool,
    pub src: Option<String>,
    pub alt: Option<String>,
}
impl Block {
    pub fn text(&self) -> String {
        self.s.iter().map(|s| s.t.as_str()).collect()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub block: usize,
    pub at: usize,
    pub depth: usize,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Document {
    pub length: usize,
    pub chapters: Vec<Chapter>,
    pub blocks: Vec<Block>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl Document {
    pub fn length_of(&self, i: usize) -> usize {
        self.blocks.get(i).map_or(0, |b| self.blocks.get(i + 1).map_or(self.length, |n| n.at).saturating_sub(b.at))
    }

    pub fn position_of(&self, i: usize, fraction: f32) -> usize {
        self.blocks.get(i).map_or(0, |b| {
            (b.at + (fraction.clamp(0.0, 1.0) * self.length_of(i) as f32).round() as usize).min(self.length)
        })
    }

    pub fn locate(&self, position: usize) -> (usize, f32) {
        if self.blocks.is_empty() {
            return (0, 0.0);
        }
        let i = self.blocks.partition_point(|b| b.at <= position).saturating_sub(1);
        let length = self.length_of(i);
        (
            i,
            if length == 0 {
                0.0
            } else {
                (position.saturating_sub(self.blocks[i].at) as f32 / length as f32).clamp(0.0, 1.0)
            },
        )
    }

    pub fn chapter_at(&self, position: usize) -> Option<&Chapter> {
        self.chapters.iter().take_while(|c| c.at <= position).last()
    }

    pub fn text_of(&self, range: Range) -> String {
        self.blocks
            .iter()
            .filter(|b| b.at < range.end && b.k != "img")
            .map(|b| slice(&b.text(), range.start.saturating_sub(b.at), range.end.saturating_sub(b.at)))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn word_at(&self, position: usize) -> Option<Range> {
        let b = self.blocks.get(self.locate(position).0)?;
        if b.k == "img" {
            return None;
        }
        let text: Vec<_> = b.text().chars().collect();
        if text.is_empty() {
            return None;
        }
        let mut i = position.saturating_sub(b.at).min(text.len() - 1);
        if text[i].is_whitespace() {
            let next = (i..text.len()).find(|n| !text[*n].is_whitespace());
            i = match next {
                Some(n) => n,
                None => (0..=i).rev().find(|n| !text[*n].is_whitespace())?,
            };
        }
        let mut a = i;
        let mut z = i + 1;
        while a > 0 && !text[a - 1].is_whitespace() {
            a -= 1;
        }
        while z < text.len() && !text[z].is_whitespace() {
            z += 1;
        }
        Some(Range { start: b.at + a, end: b.at + z })
    }

    pub fn snap_to_words(&self, start: usize, end: usize) -> Range {
        let (a, z) = (start.min(end), start.max(end));
        let first = self.word_at(a).unwrap_or(Range { start: a, end: a });
        let last = self.word_at(z.saturating_sub(1).max(a)).unwrap_or(Range { start: a, end: z });
        Range { start: first.start.min(last.start), end: first.end.max(last.end) }
    }
}

pub fn percent(position: usize, length: usize) -> usize {
    if length == 0 {
        0
    } else if position >= length {
        100
    } else {
        (position * 100 / length).min(99)
    }
}
pub fn slice(text: &str, from: usize, to: usize) -> String {
    text.chars().skip(from).take(to.saturating_sub(from)).collect()
}
pub fn file_url(base: &str, path: &str) -> String {
    let mut url = url::Url::parse("http://book.invalid/").unwrap();
    {
        let mut parts = url.path_segments_mut().unwrap();
        parts.clear();
        for part in path.split('/') {
            parts.push(part);
        }
    }
    format!("{base}{}", url.path().trim_start_matches('/'))
}

pub fn pieces(block: &Block, marks: &[Range]) -> Vec<(Span, Vec<Range>)> {
    let mut at = block.at;
    let mut out = Vec::new();
    for span in &block.s {
        let length = span.t.chars().count();
        let mut cuts = vec![0, length];
        for mark in marks {
            for pos in [mark.start, mark.end] {
                if pos > at && pos < at + length {
                    cuts.push(pos - at);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for cut in cuts.windows(2) {
            let (start, end) = (at + cut[0], at + cut[1]);
            let mut piece = span.clone();
            piece.t = slice(&span.t, cut[0], cut[1]);
            out.push((piece, marks.iter().copied().filter(|m| m.start < end && m.end > start).collect()));
        }
        at += length;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Document {
        serde_json::from_str(r#"{"length":69,"chapters":[],"blocks":[{"k":"p","at":0,"s":[{"t":"Call me Ishmael."}]},{"k":"img","at":16,"src":"whale.png"},{"k":"p","at":17,"s":[{"t":"Some years ago, "},{"t":"never mind","f":2},{"t":" how long."}]},{"k":"p","at":53,"s":[{"t":"Call me Ishmael."}]}]}"#).unwrap()
    }
    #[test]
    fn positions_and_unicode_match_the_shared_reader() {
        let doc: Document = serde_json::from_str(r#"{"length":30,"chapters":[{"title":"One","block":0,"at":0,"depth":0},{"title":"Two","block":3,"at":20,"depth":0}],"blocks":[{"k":"h","at":0,"s":[{"t":"One"}]},{"k":"p","at":3,"s":[{"t":"Seventeen letters"}]},{"k":"hr","at":20},{"k":"p","at":20,"s":[{"t":"Ten chars."}]}]}"#).unwrap();
        assert_eq!(doc.position_of(1, 0.5), 12);
        assert_eq!(doc.locate(12), (1, 9.0 / 17.0));
        assert_eq!(doc.locate(20).0, 3);
        assert_eq!(doc.locate(30), (3, 1.0));
        assert_eq!(doc.position_of(3, 2.0), 30);
        assert_eq!(doc.chapter_at(19).unwrap().title, "One");
        assert_eq!(doc.chapter_at(25).unwrap().title, "Two");
        assert_eq!(percent(299, 300), 99);
        assert_eq!(percent(30, 30), 100);
        assert_eq!(percent(5, 0), 0);
        assert_eq!("a😀b".chars().count(), 3);
        assert_eq!(slice("a😀bc", 1, 3), "😀b");
        assert_eq!(
            file_url("/api/books/1/files/", "OEBPS/Images/a b#.jpg"),
            "/api/books/1/files/OEBPS/Images/a%20b%23.jpg"
        );
    }
    #[test]
    fn quotes_selections_and_marks_match_the_shared_reader() {
        let doc = document();
        assert_eq!(doc.text_of(Range { start: 8, end: 31 }), "Ishmael.\n\nSome years ago");
        assert_eq!(doc.text_of(Range { start: 33, end: 43 }), "never mind");
        assert_eq!(doc.word_at(10), Some(Range { start: 8, end: 16 }));
        assert_eq!(doc.word_at(21), Some(Range { start: 22, end: 27 }));
        assert_eq!(doc.snap_to_words(10, 19), Range { start: 8, end: 21 });
        assert_eq!(doc.snap_to_words(19, 10), Range { start: 8, end: 21 });
        let parts = pieces(&doc.blocks[2], &[Range { start: 22, end: 38 }]);
        assert_eq!(
            parts.iter().map(|(s, m)| (s.t.as_str(), m.len())).collect::<Vec<_>>(),
            [("Some ", 0), ("years ago, ", 1), ("never", 1), (" mind", 0), (" how long.", 0)]
        );
        assert_eq!(parts[2].0.f, ITALIC);
    }
}
