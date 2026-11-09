// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native book details, character-based reading positions, and quotes.

mod article;
pub mod model;
mod view;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced_core::keyboard::{Key, Modifiers, key};
use iced_core::widget::operation::scrollable::AbsoluteOffset;
use iced_core::widget::operation::{self, Animation};
use iced_widget::image::Handle;
use model::{Document, Range};
use serde::Deserialize;
pub use view::view;

use crate::api::{self, Api};
use crate::app::{Effect, Message as AppMessage};
use crate::platform::{Current, Desktop};
use crate::route::Route;
pub const SCROLL: &str = "book-reader";
const SIZES: [f32; 8] = [15.0, 16.0, 17.0, 18.0, 20.0, 22.0, 24.0, 27.0];

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Detail {
    #[serde(flatten)]
    pub card: crate::browsing::model::Book,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub published: Option<String>,
    pub language: Option<String>,
    pub isbn: Option<String>,
    pub genres: Vec<String>,
    pub format: String,
    pub chapter_count: usize,
    pub pages: Option<f64>,
    pub rating: Option<f64>,
    pub size: u64,
    pub provider_url: Option<String>,
    pub content: String,
    pub files: String,
    pub file: String,
    pub quotes: Vec<Quote>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Quote {
    pub id: i64,
    pub name: String,
    pub mine: bool,
    pub can_manage: bool,
    pub link: Option<String>,
    pub owner: crate::clips::model::Person,
    pub quote: Option<QuoteText>,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct QuoteText {
    pub text: String,
    pub range: Option<Range>,
}
impl Quote {
    pub fn range(&self) -> Option<Range> {
        self.quote.as_ref()?.range
    }
}
#[derive(Deserialize)]
struct BookData {
    book: Option<Detail>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Detail(i64, Result<Arc<Detail>, api::Error>),
    Document(i64, Result<Arc<Document>, api::Error>),
    Image(i64, String, Option<Handle>),
    Images(Vec<usize>),
    Scrolled(f32, f32),
    Viewport(f32),
    LaidOut,
    Jump(usize),
    Step(f32),
    TextSize(i32),
    Focus,
    Poke,
    Contents,
    Quotes,
    Select(Range),
    ClearSelection,
    Note(String),
    SaveQuote,
    OpenQuote(i64),
    DeleteQuote(i64),
    ManageQuote(i64),
    CopySelection,
    CopyQuote(i64),
    Link(String),
    Finish(bool),
    Refresh,
    Retry,
    Save(u64),
    Saved(i64, usize, Result<(), api::Error>),
    Changed(i64, Result<(), api::Error>),
    Back,
    Go(Route),
}

pub struct Books {
    generation: u64,
    pub id: Option<i64>,
    api: Option<Api>,
    pub detail: Option<Arc<Detail>>,
    pub document: Option<Arc<Document>>,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub position: usize,
    known: usize,
    save_revision: u64,
    pub offset: f32,
    pub viewport: f32,
    pub focused: bool,
    pub contents: bool,
    pub show_quotes: bool,
    pub selection: Option<Range>,
    pub note: String,
    pub opened_quote: Option<i64>,
    pub busy: bool,
    pub can_quote: bool,
    pub can_edit: bool,
    text_size: f32,
    pub images: HashMap<String, Handle>,
    pub image_requests: HashSet<String>,
    pub metrics: Arc<Mutex<article::Metrics>>,
    loading: bool,
    reading: bool,
    pointer: Instant,
}
impl Default for Books {
    fn default() -> Self {
        let size = crate::paths::config_dir()
            .and_then(|p| std::fs::read_to_string(p.join("reader-size")).ok())
            .and_then(|s| s.parse().ok())
            .filter(|n| SIZES.contains(n))
            .unwrap_or(18.0);
        Self {
            generation: 0,
            id: None,
            api: None,
            detail: None,
            document: None,
            error: None,
            notice: None,
            position: 0,
            known: 0,
            save_revision: 0,
            offset: 0.0,
            viewport: 700.0,
            focused: false,
            contents: false,
            show_quotes: false,
            selection: None,
            note: String::new(),
            opened_quote: None,
            busy: false,
            can_quote: false,
            can_edit: false,
            text_size: size,
            images: HashMap::new(),
            image_requests: HashSet::new(),
            metrics: Default::default(),
            loading: false,
            reading: false,
            pointer: Instant::now(),
        }
    }
}
impl Books {
    fn anchor(&self) -> f32 {
        if self.focused { self.viewport * 0.18 } else { 12.0 }
    }

    pub fn size(&self) -> f32 {
        self.text_size
    }

    pub fn idle(&self) -> bool {
        self.pointer.elapsed() >= Duration::from_millis(2600)
    }

    pub fn animating(&self) -> bool {
        self.focused && !self.idle()
    }

    pub fn quotes(&self) -> &[Quote] {
        self.detail.as_ref().map_or(&[], |b| b.quotes.as_slice())
    }

    pub fn follow(&mut self, generation: u64, route: &Route, api: &Api, effects: &mut Vec<Effect>) {
        let (id, reading) = match route {
            Route::Book(id) => (Some(*id as i64), false),
            Route::Read(id) => (Some(*id as i64), true),
            _ => (None, false),
        };
        if !reading && self.reading {
            self.flush(effects);
            self.focused = false;
        }
        self.reading = reading;
        if self.generation != generation || self.id != id {
            self.flush(effects);
            let size = self.text_size;
            *self = Self { generation, id, api: Some(api.clone()), text_size: size, reading, ..Self::default() };
            if id.is_some() {
                self.reload(effects);
            }
        }
        if reading && self.detail.is_some() && self.document.is_none() && !self.loading && self.error.is_none() {
            self.load_document(effects);
        }
    }

    fn request(&self, effects: &mut Vec<Effect>, future: impl std::future::Future<Output = Message> + Send + 'static) {
        let generation = self.generation;
        effects.push(Effect::Request(Box::pin(async move { AppMessage::Books(generation, future.await) })));
    }

    pub fn reload(&self, effects: &mut Vec<Effect>) {
        let (Some(id), Some(api)) = (self.id, self.api.clone()) else { return };
        self.request(effects, async move {
            let result = api
                .request_data::<api::ReaderBook, BookData>(api::reader_book::Variables { id })
                .await
                .and_then(|r| r.book.map(Arc::new).ok_or_else(|| api::Error::message("This book isn't here anymore.")));
            Message::Detail(id, result)
        });
    }

    fn load_document(&mut self, effects: &mut Vec<Effect>) {
        let (Some(id), Some(api), Some(book)) = (self.id, self.api.clone(), self.detail.as_ref()) else { return };
        let path = book.content.clone();
        self.loading = true;
        self.request(effects, async move {
            let result = async {
                let mut response = api
                    .media(&path)?
                    .send()
                    .await
                    .map_err(|_| api::Error::message("Couldn't load this book. Try again."))?;
                if !response.status().is_success() {
                    return Err(api::Error::message("This book can't be read. Try refreshing it."));
                }
                let mut bytes = Vec::new();
                while let Some(chunk) =
                    response.chunk().await.map_err(|_| api::Error::message("The book download stopped."))?
                {
                    if bytes.len() + chunk.len() > 64 * 1024 * 1024 {
                        return Err(api::Error::message("This book is too large to open."));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                serde_json::from_slice::<Document>(&bytes)
                    .map(Arc::new)
                    .map_err(|_| api::Error::message("The book content could not be read."))
            }
            .await;
            Message::Document(id, result)
        });
    }

    fn scroll(&self, y: f32, effects: &mut Vec<Effect>) {
        effects.push(Effect::Operate(Box::new(operation::scrollable::scroll_to(
            SCROLL.into(),
            AbsoluteOffset { x: None, y: Some(y.max(0.0)) },
            Animation::Instant,
        ))));
    }

    pub fn flush(&mut self, effects: &mut Vec<Effect>) {
        let (Some(id), Some(api)) = (self.id, self.api.clone()) else { return };
        if self.position == self.known {
            return;
        }
        let position = self.position;
        self.known = position;
        self.request(effects, async move {
            Message::Saved(
                id,
                position,
                api.request::<api::ReaderSave>(api::reader_save::Variables { id, position: position as i64 })
                    .await
                    .map(|_| ()),
            )
        });
    }

    fn changed(
        &self,
        effects: &mut Vec<Effect>,
        future: impl std::future::Future<Output = Result<(), api::Error>> + Send + 'static,
    ) {
        let Some(id) = self.id else { return };
        self.request(effects, async move { Message::Changed(id, future.await) });
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) -> Option<Route> {
        match message {
            Message::Detail(id, _)
            | Message::Document(id, _)
            | Message::Image(id, _, _)
            | Message::Saved(id, _, _)
            | Message::Changed(id, _)
                if Some(id) != self.id =>
            {
                return None;
            },
            Message::Detail(_, result) => match result {
                Ok(book) => {
                    if self.detail.is_none() || !self.reading {
                        self.position =
                            book.card.reading.as_ref().filter(|r| !r.finished).map_or(0, |r| r.position as usize);
                        self.known = self.position;
                    }
                    if let Some(path) = &book.card.cover {
                        self.fetch_image("cover".into(), path.clone(), effects);
                    }
                    self.detail = Some(book);
                    self.error = None;
                },
                Err(e) => self.error = Some(e.message),
            },
            Message::Document(_, result) => {
                self.loading = false;
                match result {
                    Ok(doc) => {
                        self.position = self.position.min(doc.length);
                        self.document = Some(doc);
                        self.error = None;
                    },
                    Err(e) => self.error = Some(e.message),
                }
            },
            Message::Image(_, key, handle) => {
                if let Some(handle) = handle {
                    self.images.insert(key, handle);
                }
            },
            Message::Images(indices) => {
                if let (Some(doc), Some(detail)) = (&self.document, &self.detail) {
                    let paths: Vec<_> = indices
                        .into_iter()
                        .take(4)
                        .filter_map(|i| {
                            doc.blocks
                                .get(i)?
                                .src
                                .as_ref()
                                .map(|src| (i.to_string(), model::file_url(&detail.files, src)))
                        })
                        .collect();
                    for (key, path) in paths {
                        self.fetch_image(key, path, effects);
                    }
                }
            },
            Message::Viewport(height) => self.viewport = height,
            Message::Scrolled(y, height) => {
                self.offset = y;
                self.viewport = height;
                if let Some(doc) = &self.document {
                    let position = self.metrics.lock().unwrap().position(doc, y, height, self.anchor());
                    if position != self.position {
                        self.position = position;
                        self.save_revision += 1;
                        let revision = self.save_revision;
                        self.request(effects, async move {
                            tokio::time::sleep(Duration::from_millis(1500)).await;
                            Message::Save(revision)
                        });
                    }
                }
            },
            Message::LaidOut => {
                if let Some(doc) = &self.document {
                    let y = self.metrics.lock().unwrap().y(doc, self.position);
                    self.scroll(y - self.anchor(), effects);
                }
            },
            Message::Jump(position) => {
                if let Some(doc) = &self.document {
                    self.position = position.min(doc.length);
                    self.selection = None;
                    self.contents = false;
                    self.show_quotes = false;
                    let y = self.metrics.lock().unwrap().y(doc, self.position);
                    self.scroll(y - self.anchor(), effects);
                    self.flush(effects);
                }
            },
            Message::Step(by) => self.scroll(self.offset + self.viewport * by, effects),
            Message::TextSize(by) => {
                let i = SIZES.iter().position(|n| *n == self.text_size).unwrap_or(3) as i32;
                self.text_size = SIZES[(i + by).clamp(0, 7) as usize];
                if let Some(dir) = crate::paths::config_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(dir.join("reader-size"), self.text_size.to_string());
                }
            },
            Message::Focus => {
                self.focused = !self.focused;
                self.pointer = Instant::now();
            },
            Message::Poke => self.pointer = Instant::now(),
            Message::Contents => {
                self.contents = !self.contents;
                self.show_quotes = false;
            },
            Message::Quotes => {
                self.show_quotes = !self.show_quotes;
                self.contents = false;
            },
            Message::Select(range) => {
                self.selection = Some(range);
                self.opened_quote = None;
            },
            Message::ClearSelection => {
                self.selection = None;
                self.opened_quote = None;
                self.note.clear();
            },
            Message::Note(note) => self.note = note,
            Message::CopySelection => {
                if let (Some(doc), Some(range)) = (&self.document, self.selection) {
                    effects.push(Effect::Copy(doc.text_of(range)));
                }
            },
            Message::CopyQuote(id) => {
                if let Some(quote) = self.quotes().iter().find(|q| q.id == id).and_then(|q| q.quote.as_ref()) {
                    effects.push(Effect::Copy(quote.text.clone()));
                }
            },
            Message::OpenQuote(id) => {
                self.opened_quote = Some(id);
                self.show_quotes = true;
                self.contents = false;
                self.selection = None;
            },
            Message::ManageQuote(id) => return Some(Route::Clip(id as u64)),
            Message::SaveQuote => {
                if !self.can_quote || self.busy {
                    return None;
                }
                if let (Some(api), Some(book_id), Some(range)) = (self.api.clone(), self.id, self.selection) {
                    if range.end <= range.start || range.end - range.start > model::QUOTE_LENGTH {
                        self.notice = Some("Select between 1 and 3000 characters.".into());
                        return None;
                    }
                    self.busy = true;
                    let name = self.note.clone();
                    self.changed(effects, async move {
                        api.request::<api::ReaderQuote>(api::reader_quote::Variables {
                            input: api::reader_quote::NewQuote {
                                book_id,
                                start: range.start as i64,
                                end: range.end as i64,
                                name,
                                public: false,
                                recipients: vec![],
                            },
                        })
                        .await
                        .map(|_| ())
                    });
                }
            },
            Message::DeleteQuote(id) => {
                if !self.quotes().iter().any(|q| q.id == id && q.can_manage) || self.busy {
                    return None;
                }
                if let Some(api) = self.api.clone() {
                    self.busy = true;
                    self.changed(effects, async move {
                        api.request::<api::ReaderDeleteQuote>(api::reader_delete_quote::Variables { id })
                            .await
                            .map(|_| ())
                    });
                }
            },
            Message::Finish(finished) => {
                if let (Some(api), Some(id)) = (self.api.clone(), self.id) {
                    self.busy = true;
                    self.changed(effects, async move {
                        api.request::<api::ReaderFinished>(api::reader_finished::Variables { id, finished })
                            .await
                            .map(|_| ())
                    });
                }
            },
            Message::Refresh => {
                if let (Some(api), Some(id)) = (self.api.clone(), self.id) {
                    self.busy = true;
                    self.changed(effects, async move {
                        api.request::<api::ReaderRefresh>(api::reader_refresh::Variables { id }).await.map(|_| ())
                    });
                }
            },
            Message::Retry => {
                self.error = None;
                if self.detail.is_none() {
                    self.reload(effects);
                } else {
                    self.load_document(effects);
                }
            },
            Message::Save(revision) if revision == self.save_revision => self.flush(effects),
            Message::Save(_) => {},
            Message::Saved(_, position, Err(error)) => {
                if self.known == position {
                    self.known = usize::MAX;
                }
                self.notice = Some(format!("Progress wasn't saved: {}", error.message));
            },
            Message::Saved(_, _, Ok(())) => {},
            Message::Changed(_, result) => {
                self.busy = false;
                match result {
                    Ok(()) => {
                        self.selection = None;
                        self.note.clear();
                        self.reload(effects);
                    },
                    Err(e) => self.notice = Some(e.message),
                }
            },
            Message::Link(link) => {
                if let Ok(url) = url::Url::parse(&link) {
                    if matches!(url.scheme(), "http" | "https") {
                        self.request(effects, async move {
                            let _ = Current::open_url(link).await;
                            Message::Poke
                        });
                    }
                }
            },
            Message::Back => return self.id.map(|id| Route::Book(id as u64)),
            Message::Go(route) => return Some(route),
        }
        None
    }

    fn fetch_image(&mut self, key: String, path: String, effects: &mut Vec<Effect>) {
        if !self.image_requests.insert(key.clone()) {
            return;
        }
        let (Some(api), Some(id)) = (self.api.clone(), self.id) else { return };
        self.request(effects, async move {
            let handle = match crate::images::fetch(&api, &path).await {
                Some(bytes) => crate::images::decode(bytes).await,
                None => None,
            };
            Message::Image(id, key, handle)
        });
    }

    pub fn key(&self, key: &Key, modifiers: Modifiers) -> Option<Message> {
        if modifiers.control() || modifiers.alt() || modifiers.logo() {
            return None;
        }
        match key.as_ref() {
            Key::Character("f" | "F") => Some(Message::Focus),
            Key::Named(key::Named::Escape) if self.selection.is_some() || self.opened_quote.is_some() => {
                Some(Message::ClearSelection)
            },
            Key::Named(key::Named::Escape) if self.focused => Some(Message::Focus),
            Key::Character("+" | "=") => Some(Message::TextSize(1)),
            Key::Character("-") => Some(Message::TextSize(-1)),
            Key::Named(key::Named::PageDown | key::Named::ArrowRight | key::Named::Space) => {
                Some(Message::Step(if modifiers.shift() { -0.85 } else { 0.85 }))
            },
            Key::Named(key::Named::PageUp | key::Named::ArrowLeft) => Some(Message::Step(-0.85)),
            Key::Named(key::Named::ArrowDown) => Some(Message::Step(0.08)),
            Key::Named(key::Named::ArrowUp) => Some(Message::Step(-0.08)),
            Key::Named(key::Named::Home) => Some(Message::Jump(0)),
            Key::Named(key::Named::End) => self.document.as_ref().map(|doc| Message::Jump(doc.length)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Arc<Document> {
        let mut blocks = Vec::new();
        let mut at = 0;
        for i in 0..120 {
            let text = if i == 0 {
                "A quiet morning".to_owned()
            } else {
                "The light fell through the open window. We walked beside the river, talking about the books we had read. ".repeat(3)
            };
            blocks.push(serde_json::json!({"k": if i == 0 { "h" } else { "p" }, "at": at, "l": 1, "s": [{"t": text}]}));
            at += text.chars().count();
        }
        Arc::new(serde_json::from_value(serde_json::json!({"length": at, "chapters": [{"title":"A quiet morning","block":0,"at":0,"depth":0}],"blocks":blocks})).unwrap())
    }
    #[test]
    fn stale_book_responses_cannot_replace_the_current_document() {
        let mut books = Books { id: Some(2), ..Books::default() };
        books.update(Message::Document(1, Ok(document())), &mut Vec::new());
        assert!(books.document.is_none());
        books.update(Message::Document(2, Ok(document())), &mut Vec::new());
        assert!(books.document.is_some());
    }
    #[test]
    fn selection_and_keyboard_navigation_use_character_positions() {
        let mut books = Books { id: Some(1), document: Some(document()), ..Books::default() };
        let mut effects = Vec::new();
        books.update(Message::Select(Range { start: 0, end: 14 }), &mut effects);
        books.update(Message::CopySelection, &mut effects);
        assert!(matches!(&effects[0], Effect::Copy(text) if text == "A quiet mornin"));
        assert!(
            matches!(books.key(&Key::Named(key::Named::End), Modifiers::empty()), Some(Message::Jump(n)) if n == books.document.as_ref().unwrap().length)
        );
        assert!(books.key(&Key::Character("f".into()), Modifiers::CTRL).is_none());
        books.update(Message::Jump(usize::MAX), &mut effects);
        assert_eq!(books.position, books.document.as_ref().unwrap().length);
    }
    #[test]
    #[ignore = "requires a Vulkan adapter; native reader visual verification"]
    fn render_book_details_reader_and_focus() {
        use iced_core::Size;
        let dir = std::path::Path::new("/tmp/tinystream-reader-qa");
        std::fs::create_dir_all(dir).unwrap();
        let mut app = crate::app::App::new(Route::Read(1), crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        app.books.id = Some(1);
        app.books.reading = true;
        app.books.text_size = 18.0;
        app.books.document = Some(document());
        app.books.detail = Some(Arc::new(serde_json::from_value(serde_json::json!({"id":1,"title":"A quiet morning","authors":["Jane Reader"],"library":"Books","format":"EPUB","chapterCount":1,"description":"A book about ordinary days and extraordinary conversations.","quotes":[]})).unwrap()));
        app.books.can_quote = true;
        crate::snapshot::snapshot(&mut app, Size::new(1280, 800), 1.0, None, &dir.join("reader.png"), &[]).unwrap();
        assert!(app.books.metrics.lock().unwrap().total > 1000.0);
        app.books.focused = true;
        crate::snapshot::snapshot(&mut app, Size::new(900, 700), 1.0, None, &dir.join("focus.png"), &[]).unwrap();
        app.books.focused = false;
        app.books.selection = Some(Range { start: 15, end: 90 });
        app.books.contents = true;
        crate::snapshot::snapshot(&mut app, Size::new(1280, 800), 1.0, None, &dir.join("selection.png"), &[]).unwrap();
    }
}
