// SPDX-License-Identifier: AGPL-3.0-or-later

//! XHTML (EPUB's chapters) and FictionBook, read into a `Builder`. Both are
//! read leniently: a broken file keeps what was read before it broke.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::document::{BOLD, Builder, CODE, ITALIC, Kind, Link, STRIKE, SUB, SUP, Shape, UNDERLINE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Html,
    Fb2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// Nothing in it is shown.
    Skip,
    /// Only what's in it matters.
    Inline,
    Block,
    Heading(u8),
    Quote,
    Pre,
    List,
    Item,
    Section,
    Title,
}

struct Frame {
    name: String,
    role: Role,
    flags: u8,
    centered: bool,
    link: Option<Link>,
}

/// Turns an `href` in `file` into a link: out of the book, or to a file in it.
pub type Resolve<'a> = &'a dyn Fn(&str) -> Option<Link>;

fn local(name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name).to_ascii_lowercase();
    match name.rsplit_once(':') {
        Some((_, l)) => l.to_string(),
        None => name,
    }
}

pub fn attr(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes().flatten().find(|a| local(a.key.as_ref()) == name).map(|a| {
        let raw = String::from_utf8_lossy(&a.value).into_owned();
        unescape(&raw)
    })
}

/// `&amp;`, `&#233;` and HTML's named entities, which EPUBs use without declaring them.
pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let mut rest = s;

    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];

        match rest[1..].find(';').filter(|&j| j <= 32).and_then(|j| entity(&rest[1..=j]).map(|e| (e, j))) {
            Some((e, j)) => {
                out.push_str(&e);
                rest = &rest[j + 2..];
            },
            None => {
                out.push('&');
                rest = &rest[1..];
            },
        }
    }

    out.push_str(rest);
    out
}

pub fn entity(name: &str) -> Option<String> {
    if let Some(num) = name.strip_prefix('#') {
        let n = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(n).filter(|&c| c != '\0').map(String::from);
    }

    quick_xml::escape::resolve_html5_entity(name).map(str::to_string)
}

fn classes_say(e: &BytesStart, words: &[&str]) -> bool {
    attr(e, "class").is_some_and(|c| {
        c.split_whitespace().any(|c| {
            let c = c.to_ascii_lowercase();
            words.iter().any(|w| c == *w || c.contains(w))
        })
    })
}

fn style(e: &BytesStart) -> String {
    attr(e, "style").unwrap_or_default().to_ascii_lowercase().replace(' ', "")
}

fn html_role(name: &str) -> (Role, u8) {
    let role = match name {
        "head" | "script" | "style" | "rt" | "rp" | "title" | "template" | "noscript" | "object" | "iframe" => {
            Role::Skip
        },
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Role::Heading(name.as_bytes()[1] - b'0'),
        "blockquote" => Role::Quote,
        "pre" => Role::Pre,
        "ul" | "ol" | "dl" => Role::List,
        "li" | "dd" | "dt" => Role::Item,
        "p" | "div" | "section" | "article" | "header" | "footer" | "aside" | "main" | "body" | "figure"
        | "figcaption" | "nav" | "center" | "table" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption"
        | "address" | "hgroup" | "details" | "summary" | "html" => Role::Block,
        _ => Role::Inline,
    };

    let flags = match name {
        "b" | "strong" => BOLD,
        "i" | "em" | "cite" | "var" | "dfn" => ITALIC,
        "u" | "ins" => UNDERLINE,
        "s" | "strike" | "del" => STRIKE,
        "sup" => SUP,
        "sub" => SUB,
        "code" | "kbd" | "samp" | "tt" => CODE,
        _ => 0,
    };

    (role, flags)
}

fn fb2_role(name: &str) -> (Role, u8) {
    let role = match name {
        "binary" | "description" | "stylesheet" => Role::Skip,
        "section" => Role::Section,
        "title" => Role::Title,
        "subtitle" => Role::Heading(6),
        "epigraph" | "cite" | "poem" => Role::Quote,
        "p" | "v" | "stanza" | "text-author" | "body" | "table" | "tr" | "td" | "th" | "fictionbook" => Role::Block,
        _ => Role::Inline,
    };

    let flags = match name {
        "strong" => BOLD,
        "emphasis" => ITALIC,
        "strikethrough" => STRIKE,
        "sup" => SUP,
        "sub" => SUB,
        "code" => CODE,
        _ => 0,
    };

    (role, flags)
}

const VOID: &[&str] = &["br", "img", "hr", "meta", "link", "input", "col", "area", "base", "wbr", "source", "embed"];

pub struct Walker<'a> {
    b: &'a mut Builder,
    dialect: Dialect,
    resolve: Resolve<'a>,
    stack: Vec<Frame>,
    /// A FictionBook title being read: its text, and the block it started at.
    title: Option<(String, usize)>,
    notes: bool,
}

impl<'a> Walker<'a> {
    pub fn new(b: &'a mut Builder, dialect: Dialect, resolve: Resolve<'a>) -> Self {
        Self { b, dialect, resolve, stack: Vec::new(), title: None, notes: false }
    }

    fn shape(&self) -> Shape {
        let mut shape = Shape::default();
        let lists = self.stack.iter().filter(|f| f.role == Role::List).count() as u8;
        let quotes = self.stack.iter().filter(|f| f.role == Role::Quote).count() as u8;
        let sections = self.stack.iter().filter(|f| f.role == Role::Section).count() as u8;
        shape.centered = self.stack.iter().any(|f| f.centered);

        if let Some(level) = self.stack.iter().rev().find_map(|f| match f.role {
            Role::Heading(l) => Some(l),
            Role::Title => Some(sections.clamp(1, 5)),
            _ => None,
        }) {
            shape.kind = Kind::H;
            shape.level = level;
        } else if self.stack.iter().any(|f| f.role == Role::Pre) {
            shape.kind = Kind::Pre;
        } else if self.stack.iter().any(|f| f.role == Role::Item) {
            shape.kind = Kind::Li;
            shape.level = lists.max(1);
        } else if quotes > 0 {
            shape.kind = Kind::Quote;
            shape.level = quotes;
        }

        shape
    }

    fn skipping(&self) -> bool {
        self.stack.iter().any(|f| f.role == Role::Skip)
    }

    fn flags(&self) -> u8 {
        self.stack.iter().fold(0, |f, x| f | x.flags)
    }

    fn link(&self) -> Option<&Link> {
        self.stack.iter().rev().find_map(|f| f.link.as_ref())
    }

    fn text(&mut self, s: &str) {
        if self.skipping() {
            return;
        }

        if let Some((t, _)) = &mut self.title {
            t.push_str(s);
        }

        let flags = self.flags();
        let link = self.link().cloned();

        if self.stack.iter().any(|f| f.role == Role::Pre) {
            self.b.raw(s, flags, link.as_ref());
        } else {
            self.b.text(s, flags, link.as_ref());
        }
    }

    fn open(&mut self, e: &BytesStart, empty: bool) {
        let name = local(e.name().as_ref());
        let (role, mut flags) = match self.dialect {
            Dialect::Html => html_role(&name),
            Dialect::Fb2 => fb2_role(&name),
        };

        if self.skipping() {
            if !empty && !(self.dialect == Dialect::Html && VOID.contains(&name.as_str())) {
                self.stack.push(Frame { name, role: Role::Skip, flags: 0, centered: false, link: None });
            }
            return;
        }

        if let Some(id) = attr(e, "id").or_else(|| if name == "a" { attr(e, "name") } else { None }) {
            self.b.anchor(&id);
        }

        match (self.dialect, name.as_str()) {
            (Dialect::Html, "br") => {
                self.b.line_break();
                return;
            },
            (Dialect::Fb2, "empty-line") => return,
            (Dialect::Html, "hr") => {
                self.b.rule(self.shape());
                return;
            },
            (Dialect::Html, "img") | (_, "image") => {
                let src = attr(e, "src").or_else(|| attr(e, "href"));

                if let Some(Link::In(src)) = src.as_deref().and_then(|s| (self.resolve)(s)) {
                    self.b.image(src, attr(e, "alt").or_else(|| attr(e, "title")), self.shape());
                }

                if !empty && name != "img" {
                    self.stack.push(Frame { name, role: Role::Skip, flags: 0, centered: false, link: None });
                }
                return;
            },
            (Dialect::Html, n) if VOID.contains(&n) => return,
            _ => {},
        }

        if self.dialect == Dialect::Fb2 && name == "body" && attr(e, "name").is_some_and(|n| n == "notes") {
            self.notes = true;
            let at = self.b.blocks();
            self.b.chapter("Notes", at, 0);
        }

        if !matches!(role, Role::Inline | Role::Skip) {
            self.b.flush(self.shape());
        }

        if role == Role::Title && self.dialect == Dialect::Fb2 {
            self.title = Some((String::new(), self.b.blocks()));
        }

        if self.dialect == Dialect::Html && role != Role::Skip {
            if classes_say(e, &["italic", "ital", "emph"]) {
                flags |= ITALIC;
            }
            if classes_say(e, &["bold", "strong"]) {
                flags |= BOLD;
            }

            let style = style(e);
            if style.contains("font-style:italic") {
                flags |= ITALIC;
            }
            if ["font-weight:bold", "font-weight:600", "font-weight:700", "font-weight:800", "font-weight:900"]
                .iter()
                .any(|w| style.contains(w))
            {
                flags |= BOLD;
            }
        }

        let centered = role != Role::Inline
            && (name == "center"
                || (self.dialect == Dialect::Html && classes_say(e, &["center", "centre"]))
                || style(e).contains("text-align:center"));

        let link = if name == "a" { attr(e, "href").and_then(|h| (self.resolve)(&h)) } else { None };
        let frame = Frame { name, role, flags, centered, link };

        self.stack.push(frame);

        if empty {
            self.pop();
        }
    }

    fn close(&mut self, name: &str) {
        let Some(at) = self.stack.iter().rposition(|f| f.name == name) else { return };

        while self.stack.len() > at {
            self.pop();
        }
    }

    /// Closes the innermost element, ending the paragraph it made.
    fn pop(&mut self) {
        let shape = self.shape();
        let Some(frame) = self.stack.pop() else { return };

        if frame.role != Role::Inline && frame.role != Role::Skip {
            self.b.flush(shape);
        }

        self.finish_title(&frame);
    }

    fn finish_title(&mut self, frame: &Frame) {
        if frame.role != Role::Title {
            return;
        }

        if let Some((title, block)) = self.title.take() {
            let sections = self.stack.iter().filter(|f| f.role == Role::Section).count() as u8;

            if sections > 0 && !self.notes {
                self.b.chapter(&title, block, sections - 1);
            }
        }
    }

    pub fn walk(&mut self, text: &str) {
        let mut r = Reader::from_str(text);
        let c = r.config_mut();
        c.trim_text(false);
        c.check_end_names = false;
        c.allow_unmatched_ends = true;
        c.allow_dangling_amp = true;

        loop {
            match r.read_event() {
                Ok(Event::Start(e)) => self.open(&e, false),
                Ok(Event::Empty(e)) => self.open(&e, true),
                Ok(Event::End(e)) => self.close(&local(e.name().as_ref())),
                Ok(Event::Text(t)) => self.text(&t.decode().unwrap_or_default()),
                Ok(Event::CData(t)) => self.text(&t.decode().unwrap_or_default()),
                Ok(Event::GeneralRef(r)) => {
                    let name = r.decode().unwrap_or_default();
                    if let Some(s) = entity(&name) {
                        self.text(&s);
                    }
                },
                Ok(Event::Eof) => break,
                Ok(_) => {},
                Err(e) => {
                    tracing::debug!("stopped reading a chapter at {}: {e}", r.buffer_position());
                    break;
                },
            }
        }

        while !self.stack.is_empty() {
            self.pop();
        }

        self.b.flush(Shape::default());
    }
}

/// The text of a description that may be HTML, with its paragraphs on lines of their own.
pub fn plain(html: &str) -> String {
    let mut b = Builder::new();
    let resolve = |_: &str| None;
    Walker::new(&mut b, Dialect::Html, &resolve).walk(&format!("<div>{html}</div>"));

    b.finish(Vec::new()).blocks.iter().map(|b| b.text()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::books::document::Document;

    fn html(s: &str) -> Document {
        let mut b = Builder::new();
        b.start_file("c.xhtml");
        let resolve =
            |h: &str| Some(if h.starts_with("http") { Link::Out(h.into()) } else { Link::In(format!("{h}")) });
        Walker::new(&mut b, Dialect::Html, &resolve).walk(s);
        b.finish(Vec::new())
    }

    #[test]
    fn reads_paragraphs_headings_and_styles() {
        let doc = html(
            r#"<html><head><title>X</title><style>p{}</style></head><body>
            <h1 class="chapter">Chapter&#160;One</h1>
            <p>It was <i>very</i> <span class="bold">dark</span>&hellip;</p>
            <div>loose text<br/>next line</div>
            <blockquote><p>quoted</p></blockquote>
            <p class="center">* * *</p>
            <p><ruby>漢<rt>かん</rt></ruby>字</p>
            </body></html>"#,
        );

        let texts: Vec<String> = doc.blocks.iter().map(|b| b.text()).collect();
        assert_eq!(
            texts,
            ["Chapter\u{a0}One", "It was very dark…", "loose text\nnext line", "quoted", "* * *", "漢字"]
        );
        assert_eq!(doc.blocks[0].k, Kind::H);
        assert_eq!(doc.blocks[0].l, 1);
        assert_eq!(doc.blocks[1].s[1].f, ITALIC);
        assert_eq!(doc.blocks[1].s[2].f, BOLD);
        assert_eq!(doc.blocks[3].k, Kind::Quote);
        assert!(doc.blocks[4].c);
        assert_eq!(doc.chapters[0].title, "Chapter One");
    }

    #[test]
    fn survives_html_that_isnt_xml() {
        let doc = html("<body><p>one<br>two<p>three &copy; <img src=\"a.png\" alt=\"pic\"> four</body>");
        let texts: Vec<String> = doc.blocks.iter().map(|b| b.text()).collect();
        assert_eq!(texts, ["one\ntwo", "three ©", "", "four"]);
        assert_eq!(doc.blocks[2].src.as_deref(), Some("a.png"));
    }

    #[test]
    fn strips_descriptions() {
        assert_eq!(plain("<p>First &amp; <b>best</b>.</p><p>Second</p>"), "First & best.\n\nSecond");
        assert_eq!(plain("Just text"), "Just text");
    }
}
