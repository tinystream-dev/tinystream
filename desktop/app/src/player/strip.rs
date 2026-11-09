// SPDX-License-Identifier: AGPL-3.0-or-later

//! The clip editor's range on a filmstrip (`RangeStrip`): the whole video in a thin bar for
//! jumping, and the part around the range below it, zoomed so a frame is easy to hit, with handles
//! for each end and the range itself to drag.

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{Border, Color, Event, Length, Rectangle, Shadow, Size, Theme, Vector, Widget, mouse, renderer};
use iced_widget::Renderer;

use crate::ui::Look;

/// Which part of the range is being moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
    Both,
}

/// What happened on the strip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strip {
    /// Show the frame at this time.
    Seek(f64),
    /// Started dragging an edge, or the whole range.
    Grab(Edge),
    /// The edge dragged is at this time now; for the whole range, where its start goes.
    Move(Edge, f64),
    Release,
    /// Put the range's middle here (the whole video's bar).
    Center(f64),
}

/// How the range strip is drawn: what's around the range, where things are, in seconds.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub view: (f64, f64),
    pub range: (f64, f64),
    pub time: f64,
}

/// The darkening, the range's outline, its handles and the playhead, over the filmstrip.
pub struct Overlay<Message> {
    range: Range,
    look: Look,
    on: Box<dyn Fn(Strip) -> Message>,
}

pub fn overlay<Message>(range: Range, look: &Look, on: impl Fn(Strip) -> Message + 'static) -> Overlay<Message> {
    Overlay { range, look: *look, on: Box::new(on) }
}

/// A handle's width, and how far it reaches past the strip at the top and bottom.
const HANDLE: f32 = 14.0;
const REACH: f32 = 4.0;

#[derive(Default)]
struct State {
    /// What's being dragged, and how far into the range it was grabbed, in seconds.
    grabbed: Option<(Edge, f64)>,
    seeking: bool,
}

impl<Message> Overlay<Message> {
    fn span(&self) -> f64 {
        let span = self.range.view.1 - self.range.view.0;
        if span > 0.0 { span } else { 1.0 }
    }

    fn x(&self, bounds: Rectangle, t: f64) -> f32 {
        bounds.x + ((t - self.range.view.0) / self.span()) as f32 * bounds.width
    }

    fn time_at(&self, bounds: Rectangle, x: f32) -> f64 {
        self.range.view.0 + f64::from(((x - bounds.x) / bounds.width).clamp(0.0, 1.0)) * self.span()
    }

    fn handles(&self, bounds: Rectangle) -> (Rectangle, Rectangle) {
        let handle = |x: f32| Rectangle { x, y: bounds.y - REACH, width: HANDLE, height: bounds.height + REACH * 2.0 };
        (handle(self.x(bounds, self.range.range.0) - HANDLE), handle(self.x(bounds, self.range.range.1)))
    }

    fn body(&self, bounds: Rectangle) -> Rectangle {
        let (start, end) = (self.x(bounds, self.range.range.0), self.x(bounds, self.range.range.1));
        Rectangle { x: start, y: bounds.y, width: (end - start).max(0.0), height: bounds.height }
    }

    /// What's under `point`: a handle, the range, or nothing.
    fn hit(&self, bounds: Rectangle, point: iced_core::Point) -> Option<Edge> {
        let (start, end) = self.handles(bounds);
        if start.contains(point) {
            Some(Edge::Start)
        } else if end.contains(point) {
            Some(Edge::End)
        } else if self.body(bounds).contains(point) {
            Some(Edge::Both)
        } else {
            None
        }
    }
}

impl<Message> iced_core::widget::Meta for Overlay<Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Overlay<Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let reach = Rectangle { y: bounds.y - REACH, height: bounds.height + REACH * 2.0, ..bounds };
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(point) = cursor.position_over(reach) else { return };
                let at = self.time_at(bounds, point.x);
                match self.hit(bounds, point) {
                    Some(edge) => {
                        state.grabbed = Some((edge, at - self.range.range.0));
                        shell.publish((self.on)(Strip::Grab(edge)));
                    },
                    None if cursor.is_over(bounds) => {
                        state.seeking = true;
                        shell.publish((self.on)(Strip::Seek(at)));
                    },
                    None => return,
                }
                shell.capture_event();
            },
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let at = self.time_at(bounds, position.x);
                if let Some((edge, offset)) = state.grabbed {
                    let to = if edge == Edge::Both { at - offset } else { at };
                    shell.publish((self.on)(Strip::Move(edge, to)));
                } else if state.seeking {
                    shell.publish((self.on)(Strip::Seek(at)));
                } else {
                    return;
                }
                shell.capture_event();
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.grabbed.is_some() || state.seeking =>
            {
                if state.grabbed.take().is_some() {
                    shell.publish((self.on)(Strip::Release));
                }
                state.seeking = false;
                shell.capture_event();
            },
            _ => {},
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced_core::Renderer as _;

        let bounds = layout.bounds();
        let l = self.look;
        let quad = |bounds: Rectangle, radius: [f32; 4]| renderer::Quad {
            bounds,
            border: Border { radius: corners(radius), ..Border::default() },
            ..renderer::Quad::default()
        };
        let (start, end) = (self.x(bounds, self.range.range.0), self.x(bounds, self.range.range.1));
        let corner = l.radius(10.0);
        // Outside the range goes dark.
        let shade = l.media_shade.scale_alpha(0.55);
        let left = Rectangle { width: (start - bounds.x).clamp(0.0, bounds.width), ..bounds };
        if left.width > 0.0 {
            renderer.fill_quad(quad(left, [corner, 0.0, 0.0, corner]), shade);
        }
        let right_x = end.clamp(bounds.x, bounds.x + bounds.width);
        let right = Rectangle { x: right_x, width: bounds.x + bounds.width - right_x, ..bounds };
        if right.width > 0.0 {
            renderer.fill_quad(quad(right, [0.0, corner, corner, 0.0]), shade);
        }
        // The range, outlined (`shadow-[inset_0_0_0_2px_var(--color-highlight)]`).
        let body = self.body(bounds);
        renderer.fill_quad(
            renderer::Quad {
                bounds: body,
                border: Border { color: l.highlight, width: 2.0, radius: l.radius(6.0).into() },
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );
        let (start_handle, end_handle) = self.handles(bounds);
        for handle in [start_handle, end_handle] {
            renderer.fill_quad(quad(handle, [l.radius(5.0); 4]), l.highlight);
            let grip =
                Rectangle { x: handle.center_x() - 1.0, y: handle.y + 16.0, width: 2.0, height: handle.height - 32.0 };
            if grip.height > 0.0 {
                renderer.fill_quad(quad(grip, [1.0; 4]), l.media_shade.scale_alpha(0.4));
            }
        }
        let time = self.range.time;
        if time >= self.range.view.0 && time <= self.range.view.1 {
            let x = self.x(bounds, time);
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: x - 1.0,
                        y: bounds.y - REACH,
                        width: 2.0,
                        height: bounds.height + REACH * 2.0,
                    },
                    border: Border { radius: 1.0.into(), ..Border::default() },
                    shadow: Shadow { color: l.media_shade.scale_alpha(0.6), offset: Vector::ZERO, blur_radius: 6.0 },
                    ..renderer::Quad::default()
                },
                l.media_ink,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        match tree.state.downcast_ref::<State>().grabbed {
            Some((Edge::Both, _)) => return mouse::Interaction::Grabbing,
            Some(_) => return mouse::Interaction::ResizingHorizontally,
            None => {},
        }
        let reach = Rectangle { y: bounds.y - REACH, height: bounds.height + REACH * 2.0, ..bounds };
        match cursor.position_over(reach).map(|point| self.hit(bounds, point)) {
            Some(Some(Edge::Both)) => mouse::Interaction::Grab,
            Some(Some(_)) => mouse::Interaction::ResizingHorizontally,
            Some(None) if cursor.is_over(bounds) => mouse::Interaction::Crosshair,
            _ => mouse::Interaction::None,
        }
    }
}

/// Corners clockwise from the top left, as CSS lists them.
pub fn corners([top_left, top_right, bottom_right, bottom_left]: [f32; 4]) -> iced_core::border::Radius {
    iced_core::border::Radius { top_left, top_right, bottom_right, bottom_left }
}

/// The whole video in a thin bar: where the strip's view and the range are. A press puts the
/// range's middle there.
pub struct Overview<Message> {
    duration: f64,
    view: (f64, f64),
    range: (f64, f64),
    look: Look,
    on: Box<dyn Fn(Strip) -> Message>,
}

pub fn overview<Message>(
    duration: f64,
    view: (f64, f64),
    range: (f64, f64),
    look: &Look,
    on: impl Fn(Strip) -> Message + 'static,
) -> Overview<Message> {
    Overview { duration, view, range, look: *look, on: Box::new(on) }
}

impl<Message> iced_core::widget::Meta for Overview<Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Overview<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(8.0))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = limits.resolve(Length::Fill, Length::Fixed(8.0), Size::ZERO);
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && let Some(point) = cursor.position_over(bounds)
            && self.duration > 0.0
        {
            let at = f64::from((point.x - bounds.x) / bounds.width) * self.duration;
            shell.publish((self.on)(Strip::Center(at)));
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced_core::Renderer as _;

        let bounds = layout.bounds();
        let duration = if self.duration > 0.0 { self.duration } else { 1.0 };
        let pill = |x: f64, width: f64, min: f32| {
            let x = bounds.x + (x / duration) as f32 * bounds.width;
            renderer::Quad {
                bounds: Rectangle { x, width: ((width / duration) as f32 * bounds.width).max(min), ..bounds },
                border: Border { radius: (bounds.height / 2.0).into(), ..Border::default() },
                ..renderer::Quad::default()
            }
        };
        let ink = self.look.media_ink;
        renderer.fill_quad(pill(0.0, duration, 0.0), ink.scale_alpha(0.08));
        renderer.fill_quad(pill(self.view.0, self.view.1 - self.view.0, 0.0), ink.scale_alpha(0.15));
        renderer.fill_quad(pill(self.range.0, self.range.1 - self.range.0, 4.0), self.look.highlight);
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) { mouse::Interaction::Pointer } else { mouse::Interaction::None }
    }
}
