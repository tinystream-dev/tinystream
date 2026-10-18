// SPDX-License-Identifier: AGPL-3.0-or-later

//! Plain text: in whatever encoding it turns out to be in, with its chapters
//! found by their headings ("Chapter 3", "第三章", "Prologue"…).

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::Meta;
use super::document::{Builder, Document, Kind, Shape};

static HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)^(
            (?i:chapter|ch\.|part|book|volume|vol\.|act|episode)\s*
                (\d+|[IVXLCDM]+|(?i:one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|twenty|thirty|forty|fifty)[a-z-]*)\b.*
            | (?i:prologue|epilogue|interlude|afterword|foreword|preface|introduction|side\s+story|intermission)\b.*
            | 第\s*[0-9０-９一二三四五六七八九十百千万零〇两]+\s*[章节節回卷部话話幕].*
            | (序章|终章|終章|序幕|楔子|尾声|尾聲|后记|後記|あとがき|番外|プロローグ|エピローグ).*
            | \#{1,3}\s+.+
        )$",
    )
    .unwrap()
});

static FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(Title|Author|Language):\s*(.+)$").unwrap());

/// The file's text, from its BOM, as UTF-8, or as whatever it looks most like.
pub fn decode(bytes: &[u8]) -> String {
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc.decode_without_bom_handling(&bytes[bom..]).0.into_owned();
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(&bytes[..bytes.len().min(256 * 1024)], bytes.len() <= 256 * 1024);
    detector.guess(None, chardetng::Utf8Detection::Allow).decode_without_bom_handling(bytes).0.into_owned()
}

fn heading(line: &str) -> bool {
    line.chars().count() <= 80 && HEADING.is_match(line)
}

fn cjk(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
}

pub fn open(path: &Path) -> anyhow::Result<(Meta, Document)> {
    let text = decode(&std::fs::read(path)?);
    let title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    Ok(parse(&text, &title))
}

pub fn parse(text: &str, title: &str) -> (Meta, Document) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let mut meta = Meta { title: title.to_string(), ..Meta::default() };

    // Project Gutenberg's header says what the book is.
    for line in lines.iter().take(60) {
        if let Some(c) = FIELD.captures(line.trim()) {
            match &c[1] {
                "Title" => meta.title = c[2].trim().to_string(),
                "Author" => meta.authors = vec![c[2].trim().to_string()],
                _ => meta.language = Some(c[2].trim().to_string()),
            }
        }
    }

    // Not Project Gutenberg's licence, before and after the book.
    if let Some(start) = lines.iter().position(|l| l.starts_with("*** START OF")) {
        lines.drain(..=start);
    }
    if let Some(end) = lines.iter().position(|l| l.starts_with("*** END OF")) {
        lines.truncate(end);
    }

    // Paragraphs are split by blank lines, unless there are hardly any of them: then each line is
    // one.
    let filled = lines.iter().filter(|l| !l.trim().is_empty()).count();
    let blank = lines.iter().filter(|l| l.trim().is_empty()).count();
    let by_blank_lines = filled > 0 && blank * 10 >= filled;

    let mut b = Builder::new();
    let mut paragraph = String::new();

    let end = |b: &mut Builder, paragraph: &mut String| {
        if !paragraph.is_empty() {
            b.raw(paragraph, 0, None);
            b.flush(Shape::default());
            paragraph.clear();
        }
    };

    for line in lines {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if by_blank_lines {
                end(&mut b, &mut paragraph);
            }
            continue;
        }

        if heading(trimmed) {
            end(&mut b, &mut paragraph);
            let name = trimmed.trim_start_matches('#').trim();
            let at = b.blocks();
            b.chapter(name, at, 0);
            b.text(name, 0, None);
            b.flush(Shape { kind: Kind::H, level: 2, centered: false });
            continue;
        }

        if !by_blank_lines {
            b.text(trimmed, 0, None);
            b.flush(Shape::default());
            continue;
        }

        if let (Some(last), Some(first)) = (paragraph.chars().last(), trimmed.chars().next())
            && !(cjk(last) && cjk(first))
        {
            paragraph.push(' ');
        }

        paragraph.push_str(&trimmed.split_whitespace().collect::<Vec<_>>().join(" "));
    }

    end(&mut b, &mut paragraph);
    (meta, b.finish(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_paragraphs_and_chapters() {
        let (meta, doc) = parse(
            "Title: The Book\nAuthor: Someone\n\n*** START OF THE BOOK ***\nChapter 1: Start\n\nIt was a\nlong night.\n\nThe end\nof it.\n\nCHAPTER II.\n\nMore.\n",
            "file",
        );

        assert_eq!(meta.title, "The Book");
        assert_eq!(meta.authors, ["Someone"]);
        let texts: Vec<String> = doc.blocks.iter().map(|b| b.text()).collect();
        assert_eq!(texts, ["Chapter 1: Start", "It was a long night.", "The end of it.", "CHAPTER II.", "More."]);
        let chapters: Vec<&str> = doc.chapters.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(chapters, ["Chapter 1: Start", "CHAPTER II."]);
        assert!(!heading("Book did not arrive."));
        assert!(heading("Chapter Twenty-One"));
    }

    #[test]
    fn reads_one_paragraph_a_line() {
        let (_, doc) = parse("第一章 开始\n他走了。\n她来了。\n第二章 结束\n完。", "书");
        let texts: Vec<String> = doc.blocks.iter().map(|b| b.text()).collect();
        assert_eq!(texts, ["第一章 开始", "他走了。", "她来了。", "第二章 结束", "完。"]);
        assert_eq!(doc.chapters.len(), 2);
    }

    #[test]
    fn guesses_encodings() {
        let (gbk, _, _) = encoding_rs::GBK.encode("第一章 很久以前，有一个小村庄。村里住着一位老人。");
        assert!(decode(&gbk).starts_with("第一章"));
    }
}
