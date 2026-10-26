// SPDX-License-Identifier: AGPL-3.0-or-later

//! Numbers that move: digits rolling to a new value (`Ticker.tsx`) and the soft charts of recent
//! values (`Sparkline.tsx`).

use std::time::{Duration, Instant};

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{
    Color, Event, Length, Point, Rectangle, Renderer as _, Size, Theme, Vector, Widget, mouse, renderer, window,
};
use iced_widget::canvas::{self, Frame, LineJoin, Path, Stroke};
use iced_widget::{Column, Element, Renderer, Row, Widget as _};

use super::motion;
use super::text::{Size as TextSize, inherit};
use super::theme::Look;

const ROLL: Duration = Duration::from_millis(500);

/// `<Ticker>`: `value`, its digits rolling to new ones. Places count from the end, so `9` to `10`
/// rolls the last one and a new one appears in front.
pub fn ticker<'a, Message: 'a>(value: &str, size: TextSize, look: &Look) -> Element<'a, Message> {
    let still = look.reduce_motion;
    let chars: Vec<char> = value.chars().collect();
    let content = Row::with_children(chars.iter().map(|&c| {
        match c.to_digit(10) {
            Some(d) => Digit {
                strip: Column::with_children((0..10).map(|n| inherit(n.to_string(), size).boxed())).boxed(),
                digit: d as f32,
                line: size.line,
                still,
            }
            .boxed(),
            None => inherit(c.to_string(), size).boxed(),
        }
    }))
    .boxed();
    Ticker { content, places: chars.len() }.boxed()
}

struct Ticker<'a, Message> {
    content: Element<'a, Message>,
    places: usize,
}
impl<Message> iced_core::widget::Meta for Ticker<'_, Message> {}
impl<Message> Widget<Message, Theme, Renderer> for Ticker<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<usize>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(self.places)
    }

    fn diff(&mut self, tree: &mut Tree) {
        if let Some(row) = tree.children.first_mut() {
            let old = *tree.state.downcast_ref::<usize>();
            if self.places > old {
                row.children.splice(0..0, std::iter::repeat_with(Tree::empty).take(self.places - old));
            } else if self.places < old {
                row.children.drain(0..(old - self.places).min(row.children.len()));
            }
        }
        *tree.state.downcast_mut::<usize>() = self.places;
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        layout::padded(tree, limits, Length::Shrink, Length::Shrink, 0, |tree, limits| {
            self.content.layout(tree, renderer, limits);
            tree.size
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("digits");
        self.content.update(tree, event, child, cursor, renderer, shell, viewport);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let (child, tree) = layout.iter(&tree.children).next().expect("digits");
        self.content.draw(tree, renderer, theme, style, child, cursor, viewport);
    }
}

struct Digit<'a, Message> {
    strip: Element<'a, Message>,
    digit: f32,
    line: f32,
    still: bool,
}

struct Roll {
    from: f32,
    to: f32,
    start: Option<Instant>,
    now: Option<Instant>,
}

impl Roll {
    fn at(&self) -> f32 {
        match (self.start, self.now) {
            (Some(start), Some(now)) => {
                motion::lerp(self.from, self.to, motion::GLIDE.at(motion::progress(start, ROLL, now)))
            },
            _ => self.to,
        }
    }
}

impl<Message> iced_core::widget::Meta for Digit<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Digit<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Roll>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Roll { from: self.digit, to: self.digit, start: None, now: None })
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.strip));
        let roll = tree.state.downcast_mut::<Roll>();
        if roll.to != self.digit {
            *roll = Roll { from: roll.at(), to: self.digit, start: None, now: None };
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fixed(self.line))
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        layout::padded(tree, &limits.loose(), Length::Shrink, Length::Shrink, 0, |tree, limits| {
            let strip = layout::Limits::new(Size::ZERO, Size::new(limits.max.width, self.line * 10.0));
            self.strip.layout(tree, renderer, &strip);
            tree.size
        });
        tree.size.height = self.line;
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let roll = tree.state.downcast_mut::<Roll>();
            if self.still {
                roll.from = roll.to;
            } else if roll.from != roll.to {
                roll.start.get_or_insert(*now);
                roll.now = Some(*now);
                if roll.at() != roll.to {
                    shell.request_redraw();
                } else {
                    roll.from = roll.to;
                }
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let offset = tree.state.downcast_ref::<Roll>().at() * self.line;
        let (strip, tree) = layout.iter(&tree.children).next().expect("digits");
        renderer.with_layer(bounds, |renderer| {
            renderer.with_translation(Vector::new(0.0, -offset), |renderer| {
                let visible = Rectangle { y: viewport.y + offset, ..*viewport };
                self.strip.draw(tree, renderer, theme, style, strip, cursor, &visible);
            });
        });
    }
}

/// `<Sparkline>`: a soft area chart of recent values, scaled to its own peak.
pub fn sparkline<'a, Message: 'a>(
    values: &[f32],
    color: Color,
    width: impl Into<Length>,
    height: impl Into<Length>,
    interval: Duration,
    look: &Look,
) -> Element<'a, Message> {
    canvas::Canvas::new(Spark {
        values: values[values.len().saturating_sub(SAMPLES)..].to_vec(),
        color,
        interval,
        still: look.reduce_motion,
    })
    .width(width)
    .height(height)
    .boxed()
}

struct Spark {
    values: Vec<f32>,
    color: Color,
    interval: Duration,
    still: bool,
}

#[derive(Default)]
struct Samples {
    values: Vec<f32>,
    peak: f32,
    since: Option<Instant>,
    now: Option<Instant>,
}

/// How many samples a sparkline spans.
const SAMPLES: usize = 60;

impl<Message> canvas::Program<Message> for Spark {
    type State = Samples;

    fn update(
        &self,
        state: &mut Samples,
        event: &Event,
        _: Rectangle,
        _: mouse::Cursor,
    ) -> Option<iced_widget::Action<Message>> {
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return None;
        };
        let target = self.values.iter().copied().fold(1.0f32, f32::max);
        if state.values != self.values {
            if state.values.is_empty() {
                state.peak = target;
            }
            state.values.clone_from(&self.values);
            state.since = Some(*now);
        }
        let dt = state.now.map_or(0.0, |last| now.saturating_duration_since(last).as_secs_f32());
        state.now = Some(*now);
        state.peak += (target - state.peak) * (1.0 - (-dt / 0.3).exp());
        if self.still {
            state.peak = target;
            return None;
        }
        if state.since.is_some_and(|since| now.saturating_duration_since(since) < self.interval)
            || (state.peak - target).abs() > target / 1000.0
        {
            Some(iced_widget::Action::request_redraw())
        } else {
            None
        }
    }

    fn draw(
        &self,
        state: &Samples,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let (w, h) = (bounds.width, bounds.height);
        if self.values.iter().all(|v| *v <= 0.0) {
            return Vec::new();
        }
        let max = state.peak.max(1.0);
        let t = if self.still {
            1.0
        } else {
            match (state.since, state.now) {
                (Some(since), Some(now)) => motion::progress(since, self.interval, now),
                _ => 1.0,
            }
        };
        let first = self.values.first().copied().unwrap_or(0.0);
        let padded: Vec<f32> = std::iter::repeat_n(first, SAMPLES.saturating_sub(self.values.len()))
            .chain(self.values.iter().copied())
            .collect();
        // The web's 100 × 30 box, stretched to ours.
        let step = 100.0 / (SAMPLES as f32 - 2.0);
        let left = 100.0 + step * (1.0 - t) - step * (padded.len() as f32 - 1.0);
        let points: Vec<Point> = padded
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Point::new((left + i as f32 * step) / 100.0 * w, (30.0 - (v.min(max) / max) * 28.0 - 1.0) / 30.0 * h)
            })
            .collect();
        let curve = |builder: &mut canvas::path::Builder| {
            builder.move_to(points[0]);
            for pair in points.windows(2) {
                let mx = (pair[0].x + pair[1].x) / 2.0;
                builder.bezier_curve_to(Point::new(mx, pair[0].y), Point::new(mx, pair[1].y), pair[1]);
            }
        };
        let mut frame = Frame::new(renderer, bounds.size());
        let area = Path::new(|builder| {
            curve(builder);
            builder.line_to(Point::new(points[points.len() - 1].x, h));
            builder.line_to(Point::new(points[0].x, h));
            builder.close();
        });
        let fill = iced_wgpu::graphics::gradient::Linear::new(Point::ORIGIN, Point::new(0.0, h))
            .add_stop(0.0, self.color.scale_alpha(0.28))
            .add_stop(1.0, self.color.scale_alpha(0.0));
        frame.fill(&area, canvas::Fill { style: canvas::Style::Gradient(fill.into()), ..canvas::Fill::default() });
        frame.stroke(
            &Path::new(curve),
            Stroke::default().with_width(1.25).with_color(self.color).with_line_join(LineJoin::Round),
        );
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_a_place_preserves_the_units_roll() {
        let look = Look::default();
        let mut nine = ticker::<()>("9", super::super::text::XL2, &look);
        let mut tree = Tree::new(&nine);
        tree.diff::<(), Theme, Renderer>(&mut nine);
        let mut ten = ticker::<()>("10", super::super::text::XL2, &look);
        tree.diff::<(), Theme, Renderer>(&mut ten);
        let digits = &tree.children[0].children;
        let tens = digits[0].state.downcast_ref::<Roll>();
        let units = digits[1].state.downcast_ref::<Roll>();
        assert_eq!((tens.from, tens.to), (1.0, 1.0));
        assert_eq!((units.from, units.to), (9.0, 0.0));
    }
}
