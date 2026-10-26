// SPDX-License-Identifier: AGPL-3.0-or-later

//! Lucide icons, drawn as vectors in the colour of the text around them (`currentColor`).

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{Color, Length, Point, Rectangle, Renderer as _, Size, Theme, Vector, Widget, mouse, renderer};
use iced_wgpu::graphics::geometry::Renderer as _;
use iced_widget::Renderer;
use iced_widget::canvas::{Frame, LineCap, LineJoin, Path, Stroke};

use super::squircle::Keyed;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// One step of an outline, in its drawing's own units.
pub(super) enum Step {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

/// `icon` at `size` pixels, with Lucide's stroke (2 in 24, scaled with the icon), in the colour
/// text would have where it's put.
pub fn icon(icon: Icon, size: f32) -> Glyph {
    Glyph { icon, size, stroke: 2.0, color: None }
}

pub struct Glyph {
    icon: Icon,
    size: f32,
    stroke: f32,
    color: Option<Color>,
}

impl Glyph {
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Its own stroke width, in Lucide's 24×24 units (`strokeWidth`).
    pub fn stroke(mut self, stroke: f32) -> Self {
        self.stroke = stroke;
        self
    }
}

impl iced_core::widget::Meta for Glyph {}

impl<Message> Widget<Message, Theme, Renderer> for Glyph {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Keyed>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Keyed::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, self.size, self.size);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let color = self.color.unwrap_or(style.text_color);
        let key = [self.size, self.stroke, color.r, color.g, color.b, color.a, self.icon as u8 as f32];
        let geometry = tree.state.downcast_ref::<Keyed>().draw(renderer, bounds.size(), &key, |frame| {
            self.paint(frame, color, self.size / 24.0);
        });
        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| renderer.draw_geometry(geometry));
    }
}

impl Glyph {
    fn paint(&self, frame: &mut Frame, color: Color, scale: f32) {
        let stroke = Stroke::default()
            .with_width(self.stroke * scale)
            .with_color(color)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);
        for (filled, steps) in self.icon.shapes() {
            let path = path(steps, scale);
            if *filled {
                frame.fill(&path, color);
            }
            frame.stroke(&path, stroke);
        }
    }
}

pub(super) fn path(steps: &[Step], scale: f32) -> Path {
    let at = |x: f32, y: f32| Point::new(x * scale, y * scale);
    Path::new(|builder| {
        for step in steps {
            match *step {
                Step::Move(x, y) => builder.move_to(at(x, y)),
                Step::Line(x, y) => builder.line_to(at(x, y)),
                Step::Cubic(a, b, c, d, x, y) => builder.bezier_curve_to(at(a, b), at(c, d), at(x, y)),
                Step::Close => builder.close(),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_inside_their_box() {
        for icon in [Icon::Settings, Icon::KeyRound, Icon::House, Icon::Inbox, Icon::Tv, Icon::CircleAlert] {
            for (_, steps) in icon.shapes() {
                for step in *steps {
                    let points: &[f32] = match step {
                        Step::Move(x, y) | Step::Line(x, y) => &[*x, *y],
                        Step::Cubic(a, b, c, d, x, y) => &[*a, *b, *c, *d, *x, *y],
                        Step::Close => &[],
                    };
                    assert!(points.iter().all(|v| (-0.5..=24.5).contains(v)), "{icon:?}: {points:?}");
                }
            }
        }
    }
}
