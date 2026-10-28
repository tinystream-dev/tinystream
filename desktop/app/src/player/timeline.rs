// SPDX-License-Identifier: AGPL-3.0-or-later

//! The player's bars: the timeline (`Timeline`), split into chapters, with what's buffered and a
//! knob under the pointer, and the volume slider (`VolumeControl`'s range input).

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{Border, Color, Event, Length, Rectangle, Size, Theme, Widget, mouse, renderer};
use iced_widget::Renderer;

use super::Hover;

type OnHover<Message> = Box<dyn Fn(Option<(f32, f32, f32)>) -> Message>;

/// What a bar shows and says.
pub struct Bar<Message> {
    /// The stretches it's split into, as fractions of its width, with a 3 px gap between.
    segments: Vec<(f32, f32)>,
    buffered: Option<(f32, f32)>,
    value: f32,
    ink: Color,
    height: f32,
    /// `at` (0 to 1) to a message, for where the pointer is, and when it lets go there.
    hover: Option<OnHover<Message>>,
    press: Box<dyn Fn(f32) -> Message>,
    release: Option<Box<dyn Fn(f32) -> Message>>,
}

/// The timeline for `duration` seconds, at `shown`, with `chapters` as (start, end) seconds.
pub fn timeline<Message>(
    shown: f64,
    duration: f64,
    buffered: (f64, f64),
    chapters: &[(f64, f64)],
    ink: Color,
    (on_hover, on_drag, on_seek): (
        impl Fn(Option<Hover>) -> Message + 'static,
        impl Fn(f64) -> Message + 'static,
        impl Fn(f64) -> Message + 'static,
    ),
) -> Bar<Message> {
    let part = |t: f64| if duration > 0.0 { (t / duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let segments = if chapters.len() > 1 {
        chapters.iter().map(|&(start, end)| (part(start), part(end))).collect()
    } else {
        vec![(0.0, 1.0)]
    };
    Bar {
        segments,
        buffered: Some((part(buffered.0), part(buffered.1))),
        value: part(shown),
        ink,
        height: 20.0,
        hover: Some(Box::new(move |at| {
            on_hover(at.map(|(at, x, width)| Hover { at: f64::from(at) * duration, x, width }))
        })),
        press: Box::new(move |at| on_drag(f64::from(at) * duration)),
        release: Some(Box::new(move |at| on_seek(f64::from(at) * duration))),
    }
}

/// The volume slider, 0 to 1.
pub fn volume<Message>(value: f64, ink: Color, on_change: impl Fn(f64) -> Message + 'static) -> Bar<Message> {
    Bar {
        segments: vec![(0.0, 1.0)],
        buffered: None,
        value: value as f32,
        ink,
        height: 16.0,
        hover: None,
        press: Box::new(move |at| on_change(f64::from(at))),
        release: None,
    }
}

#[derive(Default)]
struct State {
    hovered: bool,
    dragging: bool,
}

impl<Message> Bar<Message> {
    fn at(&self, bounds: Rectangle, x: f32) -> f32 {
        ((x - bounds.x) / bounds.width).clamp(0.0, 1.0)
    }
}

impl<Message> iced_core::widget::Meta for Bar<Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Bar<Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = limits.resolve(Length::Fill, Length::Fixed(self.height), Size::ZERO);
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
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let over = cursor.is_over(bounds);
                if over || state.dragging {
                    let at = self.at(bounds, position.x);
                    if let Some(hover) = &self.hover {
                        shell.publish(hover(Some((at, position.x - bounds.x, bounds.width))));
                    } else if state.dragging {
                        shell.publish((self.press)(at));
                    }
                } else if state.hovered
                    && let Some(hover) = &self.hover
                {
                    shell.publish(hover(None));
                }
                if over != state.hovered {
                    state.hovered = over;
                    shell.request_redraw();
                }
            },
            Event::Mouse(mouse::Event::CursorLeft) if state.hovered && !state.dragging => {
                state.hovered = false;
                if let Some(hover) = &self.hover {
                    shell.publish(hover(None));
                }
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_over(bounds) {
                    state.dragging = true;
                    shell.publish((self.press)(self.at(bounds, position.x)));
                    shell.capture_event();
                }
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                if let (Some(release), Some(position)) = (&self.release, cursor.position()) {
                    shell.publish(release(self.at(bounds, position.x)));
                }
                shell.capture_event();
            },
            _ => {},
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced_core::Renderer as _;

        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<State>();
        let active = state.hovered || state.dragging;
        let track = if active && self.hover.is_some() { 6.0 } else { 4.0 };
        let y = bounds.y + (bounds.height - track) / 2.0;
        let quad = |x: f32, width: f32, height: f32, y: f32| renderer::Quad {
            bounds: Rectangle { x, y, width: width.max(0.0), height },
            border: Border { radius: (height / 2.0).into(), ..Border::default() },
            ..renderer::Quad::default()
        };
        let count = self.segments.len();
        for (i, &(start, end)) in self.segments.iter().enumerate() {
            let gap = if i + 1 < count { 3.0 } else { 0.0 };
            let x = bounds.x + start * bounds.width;
            let width = (end - start) * bounds.width - gap;
            if width <= 0.0 {
                continue;
            }
            renderer.fill_quad(quad(x, width, track, y), self.ink.scale_alpha(0.2));
            let along = |at: f32| ((at - start) / (end - start)).clamp(0.0, 1.0) * width;
            if let Some((from, to)) = self.buffered {
                let (from, to) = (from.max(start), to.min(end));
                if to > from {
                    let left = along(from);
                    renderer.fill_quad(quad(x + left, along(to) - left, track, y), self.ink.scale_alpha(0.25));
                }
            }
            renderer.fill_quad(quad(x, along(self.value), track, y), self.ink);
        }
        if active {
            let x = bounds.x + self.value * bounds.width;
            let knob = renderer::Quad {
                bounds: Rectangle { x: x - 7.0, y: bounds.center_y() - 7.0, width: 14.0, height: 14.0 },
                border: Border { radius: 7.0.into(), ..Border::default() },
                shadow: iced_core::Shadow {
                    color: self.ink.scale_alpha(0.15),
                    offset: iced_core::Vector::ZERO,
                    blur_radius: 8.0,
                },
                ..renderer::Quad::default()
            };
            renderer.fill_quad(knob, self.ink);
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
        if cursor.is_over(layout.bounds()) || tree.state.downcast_ref::<State>().dragging {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}
