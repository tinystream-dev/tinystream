// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use iced_core::text::{self, Paragraph as _, Renderer as _};
use iced_core::widget::{Tree, tree};
use iced_core::{
    Color, Event, Font, Length, Point, Rectangle, Size, Theme, Vector, Widget, alignment, font, layout, mouse,
    renderer, window,
};
use iced_widget::Renderer;
use iced_widget::image::Handle;

use super::model::{self, Document, Range};
use super::{Books, Message};

type Paragraph = <Renderer as text::Renderer>::Paragraph;

#[derive(Default)]
pub struct Metrics {
    pub tops: Vec<f32>,
    pub heights: Vec<f32>,
    pub total: f32,
}
impl Metrics {
    pub fn y(&self, doc: &Document, position: usize) -> f32 {
        let (i, fraction) = doc.locate(position);
        self.tops.get(i).copied().unwrap_or(0.0) + self.heights.get(i).copied().unwrap_or(0.0) * fraction
    }

    pub fn position(&self, doc: &Document, y: f32, viewport: f32, anchor: f32) -> usize {
        if y > 0.0 && y + viewport >= self.total - 4.0 {
            return doc.length;
        }
        let i = self.tops.partition_point(|top| *top <= y + anchor).saturating_sub(1);
        let fraction = (y + anchor - self.tops.get(i).copied().unwrap_or(0.0))
            / self.heights.get(i).copied().unwrap_or(1.0).max(1.0);
        doc.position_of(i, fraction)
    }
}

struct Piece {
    range: Range,
    underline: bool,
    strike: bool,
    href: Option<String>,
    to: Option<usize>,
}
struct Row {
    paragraph: Paragraph,
    pieces: Vec<Piece>,
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}
#[derive(Default)]
struct State {
    key: Option<(usize, u32, u32, usize)>,
    viewport: f32,
    focused: bool,
    rows: Vec<Row>,
    drag: Option<(usize, Point)>,
    layout_changed: bool,
    images: usize,
}

pub struct Article<'a> {
    pub books: &'a Books,
    pub ink: Color,
    pub secondary: Color,
    pub highlight: Color,
}
impl iced_core::widget::Meta for Article<'_> {}

impl Widget<Message, Theme, Renderer> for Article<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        let Some(doc) = &self.books.document else { return };
        let width = limits.max.width;
        let size = self.books.size();
        let text_width = (width - 48.0).min(size * 38.0 - 48.0).max(80.0);
        let state = tree.state.downcast_mut::<State>();
        let key = (Arc::as_ptr(doc) as usize, width.to_bits(), size.to_bits(), self.books.quotes().len());
        if state.viewport != self.books.viewport || state.focused != self.books.focused {
            state.viewport = self.books.viewport;
            state.focused = self.books.focused;
            state.layout_changed = true;
        }
        if state.key != Some(key) {
            state.key = Some(key);
            state.rows = doc
                .blocks
                .iter()
                .map(|block| {
                    let indent = match block.k.as_str() {
                        "li" => block.l.max(1) as f32 * size * 1.25,
                        "quote" => block.l.max(1) as f32 * size,
                        _ => 0.0,
                    };
                    let heading = block.k == "h";
                    let em = if heading {
                        match block.l {
                            0 | 1 => 1.6,
                            2 => 1.35,
                            _ => 1.15,
                        }
                    } else if block.k == "pre" {
                        0.85
                    } else {
                        1.0
                    };
                    let mut spans: Vec<text::Span<'_, ()>> = Vec::new();
                    let mut pieces = Vec::new();
                    let mut at = block.at;
                    let marks = self.books.quotes().iter().filter_map(|q| q.range()).collect::<Vec<_>>();
                    let styled = model::pieces(block, &marks);
                    for (span, _) in &styled {
                        let mut from = 0;
                        for (end, c) in span.t.char_indices().map(|(i, c)| (i + c.len_utf8(), c)) {
                            if c.is_whitespace() || end == span.t.len() {
                                let word = &span.t[from..end];
                                let mut font = Font::DEFAULT;
                                if span.f & model::BOLD != 0 || heading {
                                    font.weight = font::Weight::Semibold;
                                }
                                if span.f & model::ITALIC != 0 {
                                    font.style = font::Style::Italic;
                                }
                                if span.f & model::CODE != 0 || block.k == "pre" {
                                    font.family = font::Family::Monospace;
                                }
                                let small = span.f & (model::SUP | model::SUB) != 0;
                                spans.push(
                                    text::Span::new(word).font(font).size(size * em * if small { 0.75 } else { 1.0 }),
                                );
                                let end = at + word.chars().count();
                                pieces.push(Piece {
                                    range: Range { start: at, end },
                                    underline: span.f & model::UNDERLINE != 0
                                        || span.href.is_some()
                                        || span.to.is_some(),
                                    strike: span.f & model::STRIKE != 0,
                                    href: span.href.clone(),
                                    to: span.to,
                                });
                                at = end;
                                from += word.len();
                            }
                        }
                    }
                    if block.k == "img" && spans.is_empty() {
                        spans.push(text::Span::new(block.alt.as_deref().unwrap_or("Loading image…")));
                    }
                    let paragraph = Paragraph::with_spans(text::Text {
                        content: &spans,
                        bounds: Size::new((text_width - indent).max(40.0), f32::INFINITY),
                        size: (size * em).into(),
                        line_height: text::LineHeight::Relative(if heading { 1.35 } else { 1.75 }),
                        font: Font::DEFAULT,
                        align_x: if block.c { text::Alignment::Center } else { text::Alignment::Default },
                        align_y: alignment::Vertical::Top,
                        shaping: text::Shaping::Advanced,
                        wrapping: text::Wrapping::WordOrGlyph,
                        ellipsis: text::Ellipsis::None,
                        hint_factor: None,
                    });
                    let height = paragraph.min_height().max(size);
                    Row {
                        paragraph,
                        pieces,
                        text: block.text(),
                        x: (width - text_width) / 2.0 + indent,
                        y: 0.0,
                        width: text_width - indent,
                        height,
                    }
                })
                .collect();
            state.layout_changed = true;
        }
        if state.images != self.books.images.len() {
            state.images = self.books.images.len();
            state.layout_changed = true;
        }
        let mut y = if self.books.focused { self.books.viewport * 0.18 } else { 40.0 };
        let mut metrics = self.books.metrics.lock().unwrap();
        metrics.tops.clear();
        metrics.heights.clear();
        for (i, row) in state.rows.iter_mut().enumerate() {
            let block = &doc.blocks[i];
            if block.k == "h" {
                y += size * 0.8;
            }
            row.y = y;
            let height = if block.k == "img" {
                self.books
                    .images
                    .get(&i.to_string())
                    .and_then(|handle| match handle {
                        Handle::Rgba { width: w, height: h, .. } => {
                            Some((text_width * *h as f32 / (*w).max(1) as f32).min(self.books.viewport * 0.85))
                        },
                        _ => None,
                    })
                    .unwrap_or(240.0)
            } else if block.k == "hr" {
                size * 3.0
            } else {
                row.paragraph.min_height().max(size)
            };
            row.height = height;
            metrics.tops.push(y);
            metrics.heights.push(height + size * 0.9);
            y += height + size * 0.9;
        }
        metrics.total = y + self.books.viewport * 0.4;
        tree.size = Size::new(width, metrics.total);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: layout::Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced_core::Renderer as _;
        use iced_core::image::Renderer as _;
        let Some(doc) = &self.books.document else { return };
        let state = tree.state.downcast_ref::<State>();
        let origin = layout.bounds().position();
        for (i, row) in state.rows.iter().enumerate() {
            let point = origin + Vector::new(row.x, row.y);
            let bounds = Rectangle::new(point, Size::new(row.width, row.height));
            if !bounds.intersects(viewport) {
                continue;
            }
            let block = &doc.blocks[i];
            if block.k == "img" {
                if let Some(handle) = self.books.images.get(&i.to_string()) {
                    let (w, h) = match handle {
                        Handle::Rgba { width, height, .. } => (*width as f32, *height as f32),
                        _ => (row.width, row.height),
                    };
                    let scale = (row.width / w).min(row.height / h).min(1.0);
                    let image_bounds = Rectangle::new(
                        Point::new(point.x + (row.width - w * scale) / 2.0, point.y),
                        Size::new(w * scale, h * scale),
                    );
                    renderer.draw_image(iced_core::image::Image::new(handle), image_bounds, *viewport);
                } else {
                    renderer.fill_paragraph(&row.paragraph, point, self.secondary, *viewport);
                }
                continue;
            }
            if block.k == "hr" {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle::new(
                            Point::new(point.x + row.width / 2.0 - 32.0, point.y + row.height / 2.0),
                            Size::new(64.0, 1.0),
                        ),
                        ..Default::default()
                    },
                    self.secondary.scale_alpha(0.4),
                );
                continue;
            }
            if block.k == "quote" {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle::new(
                            Point::new(point.x - self.books.size() * 0.7, point.y),
                            Size::new(2.0, row.height),
                        ),
                        ..Default::default()
                    },
                    self.secondary.scale_alpha(0.4),
                );
            }
            for (span, piece) in row.pieces.iter().enumerate() {
                let selected = self.books.selection.is_some_and(|r| overlaps(r, piece.range));
                let mark =
                    self.books.quotes().iter().any(|q| q.mine && q.range().is_some_and(|r| overlaps(r, piece.range)));
                let shared =
                    self.books.quotes().iter().any(|q| !q.mine && q.range().is_some_and(|r| overlaps(r, piece.range)));
                if selected || mark || shared || piece.underline || piece.strike {
                    for rect in row.paragraph.span_bounds(span) {
                        let rect = rect + Vector::new(point.x, point.y);
                        if selected || mark {
                            renderer.fill_quad(
                                renderer::Quad { bounds: rect, ..Default::default() },
                                self.highlight.scale_alpha(if selected { 0.45 } else { 0.23 }),
                            );
                        }
                        for y in [
                            (piece.underline || shared).then_some(rect.y + rect.height - 1.0),
                            piece.strike.then_some(rect.y + rect.height * 0.55),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: Rectangle::new(Point::new(rect.x, y), Size::new(rect.width, 1.0)),
                                    ..Default::default()
                                },
                                self.secondary,
                            );
                        }
                    }
                }
            }
            renderer.fill_paragraph(
                &row.paragraph,
                point,
                if block.k == "quote" { self.secondary } else { self.ink },
                *viewport,
            );
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: layout::Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let Some(doc) = &self.books.document else { return };
        let state = tree.state.downcast_mut::<State>();
        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            if (self.books.viewport - viewport.height).abs() > 1.0 {
                shell.publish(Message::Viewport(viewport.height));
            }
            if state.layout_changed {
                state.layout_changed = false;
                shell.publish(Message::LaidOut);
            }
            let images: Vec<_> = doc
                .blocks
                .iter()
                .enumerate()
                .filter(|(i, b)| {
                    b.k == "img"
                        && !self.books.images.contains_key(&i.to_string())
                        && !self.books.image_requests.contains(&i.to_string())
                        && state.rows.get(*i).is_some_and(|r| {
                            Rectangle::new(
                                layout.bounds().position() + Vector::new(r.x, r.y),
                                Size::new(r.width, r.height),
                            )
                            .intersects(viewport)
                        })
                })
                .map(|(i, _)| i)
                .collect();
            if !images.is_empty() {
                shell.publish(Message::Images(images));
            }
        }
        if matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. })) {
            shell.publish(Message::Poke);
        }
        let Some(point) = cursor.position() else { return };
        let local = point - Vector::new(layout.bounds().x, layout.bounds().y);
        let row_index = state.rows.partition_point(|r| r.y <= local.y).saturating_sub(1);
        let Some(row) = state.rows.get(row_index) else { return };
        let position = row
            .paragraph
            .hit_test(Point::new(local.x - row.x, local.y - row.y))
            .map(|hit| {
                let before = row.text.split_inclusive('\n').take(hit.line).map(str::len).sum::<usize>();
                let at = before.saturating_add(hit.index).min(row.text.len());
                doc.blocks[row_index].at + row.text[..row.text.floor_char_boundary(at)].chars().count()
            })
            .unwrap_or(doc.blocks[row_index].at);
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.is_over(*viewport) && cursor.is_over(layout.bounds()) =>
            {
                state.drag = Some((position, point));
                shell.capture_event();
            },
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.drag.is_some() => {
                let (start, _) = state.drag.unwrap();
                shell.publish(Message::Select(doc.snap_to_words(start, position)));
                shell.capture_event();
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some((start, origin)) = state.drag.take() {
                    if (point.x - origin.x).hypot(point.y - origin.y) < 3.0 {
                        let piece = row
                            .paragraph
                            .hit_span(Point::new(local.x - row.x, local.y - row.y))
                            .and_then(|i| row.pieces.get(i));
                        if let Some(to) = piece.and_then(|p| p.to) {
                            shell.publish(Message::Jump(doc.blocks.get(to).map_or(0, |b| b.at)));
                        } else if let Some(href) = piece.and_then(|p| p.href.clone()) {
                            shell.publish(Message::Link(href));
                        } else if let Some(quote) = self
                            .books
                            .quotes()
                            .iter()
                            .find(|q| q.range().is_some_and(|r| position >= r.start && position < r.end))
                        {
                            shell.publish(Message::OpenQuote(quote.id));
                        } else {
                            shell.publish(Message::Select(doc.snap_to_words(start, position.saturating_add(1))));
                        }
                    } else {
                        shell.publish(Message::Select(doc.snap_to_words(start, position)));
                    }
                    shell.capture_event();
                }
            },
            _ => {},
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: layout::Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.books.focused && self.books.idle() {
            mouse::Interaction::Hidden
        } else if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}
fn overlaps(a: Range, b: Range) -> bool {
    a.start < b.end && a.end > b.start
}
