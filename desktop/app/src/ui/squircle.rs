// SPDX-License-Identifier: AGPL-3.0-or-later

//! Boxes with smoothed corners (`Squircle.tsx`): a fill or a material, the lit edge on top, and
//! the lift under whatever floats.

use std::cell::RefCell;

use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{
    Border, Color, Event, Length, Padding, Point, Rectangle, Renderer as _, Shadow, Size, Theme, Vector, Widget, mouse,
    overlay, renderer,
};
use iced_wgpu::graphics::geometry::Renderer as _;
use iced_wgpu::primitive::Renderer as _;
use iced_widget::canvas::{self, Cache, Frame, LineDash, Path, Stroke};
use iced_widget::{Element, Renderer};

use super::backdrop::{self, Backdrop};
use super::theme::{Look, Material};

/// The outline of a `size` box at `origin`, as `squirclePath` draws it.
pub fn path(origin: Point, size: Size, radius: f32, smoothing: f32) -> Path {
    let at = |(x, y): (f64, f64)| Point::new(origin.x + x as f32, origin.y + y as f32);
    Path::new(|builder| {
        for segment in
            tinystream_squircle::outline(size.width as f64, size.height as f64, radius as f64, smoothing as f64)
        {
            match segment {
                tinystream_squircle::Segment::Move(p) => builder.move_to(at(p)),
                tinystream_squircle::Segment::Line(p) => builder.line_to(at(p)),
                tinystream_squircle::Segment::Cubic(a, b, p) => builder.bezier_curve_to(at(a), at(b), at(p)),
            }
        }
        builder.close();
    })
}

/// What a squircle is painted with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    pub fill: Option<Color>,
    /// See-through, blurring what's behind (Glass only).
    pub material: Option<Material>,
    pub edge: bool,
    pub dashed: bool,
    pub lift: bool,
    /// A ring around it, for keyboard focus.
    pub ring: Option<Color>,
}

impl Paint {
    pub const NONE: Paint = Paint { fill: None, material: None, edge: false, dashed: false, lift: false, ring: None };

    pub fn fill(color: Color) -> Self {
        Self { fill: Some(color), ..Self::NONE }
    }
}

/// Tessellated geometry, kept while what it was drawn from stays the same.
#[derive(Default)]
pub struct Keyed {
    cache: Cache,
    key: RefCell<Vec<u32>>,
}

impl Keyed {
    pub fn draw(
        &self,
        renderer: &Renderer,
        size: Size,
        key: &[f32],
        draw: impl FnOnce(&mut Frame),
    ) -> canvas::Geometry {
        let key: Vec<u32> = key.iter().map(|v| v.to_bits()).collect();
        if *self.key.borrow() != key {
            self.cache.clear();
            *self.key.borrow_mut() = key;
        }
        self.cache.draw(renderer, size, draw)
    }
}

/// A squircle's tessellated fill and edge.
#[derive(Default)]
pub struct Painted {
    fill: Keyed,
    edge: Keyed,
}

fn rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

#[derive(Clone, Copy)]
pub struct Outline {
    pub bounds: Rectangle,
    pub radius: f32,
    pub smoothing: f32,
}

/// Draws what's under a squircle's content: its lift, and its fill or material.
pub fn draw_under(
    renderer: &mut Renderer,
    painted: &Painted,
    outline: Outline,
    paint: &Paint,
    look: &Look,
    viewport: &Rectangle,
) {
    let Outline { bounds, radius, smoothing } = outline;
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    // Glass reads back what's already drawn behind it, which mustn't include its own shadow, so
    // that goes on afterwards: around the whole box rather than tucked under it, as quads leave
    // their shadow out from under themselves.
    let glass = paint.material.is_some() && backdrop::available();
    renderer.with_layer(*viewport, |renderer| {
        if paint.lift && !glass {
            lift(renderer, bounds, radius, look, true);
        }
        match (paint.material, glass) {
            (Some(material), true) => renderer.draw_primitive(
                bounds,
                Backdrop::new(material.blur, material.saturate, material.tint, radius, smoothing, 1.0)
                    .bezel(material.bezel),
            ),
            (material, _) => {
                let fill = if material.is_some() { Some(look.solid_material()) } else { paint.fill };
                if let Some(fill) = fill.filter(|c| c.a > 0.0) {
                    let mut key = vec![bounds.width, bounds.height, radius, smoothing];
                    key.extend(rgba(fill));
                    let geometry = painted.fill.draw(renderer, bounds.size(), &key, |frame| {
                        frame.fill(&path(Point::ORIGIN, bounds.size(), radius, smoothing), fill);
                    });
                    renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
                        renderer.draw_geometry(geometry);
                    });
                }
            },
        }
    });
    if paint.lift && glass {
        renderer.with_layer(*viewport, |renderer| lift(renderer, bounds, radius, look, false));
    }
}

/// The deep shadow under a floating box, `tucked` in by its spread.
fn lift(renderer: &mut Renderer, bounds: Rectangle, radius: f32, look: &Look, tucked: bool) {
    for lift in look.lift() {
        let shrink = if tucked { -lift.spread } else { 0.0 };
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x + shrink,
                    y: bounds.y + shrink,
                    width: (bounds.width - shrink * 2.0).max(0.0),
                    height: (bounds.height - shrink * 2.0).max(0.0),
                },
                border: Border::default().rounded((radius - shrink).max(0.0)),
                shadow: Shadow { color: lift.color, offset: Vector::new(0.0, lift.offset), blur_radius: lift.blur },
                ..renderer::Quad::default()
            },
            Color::TRANSPARENT,
        );
    }
}

/// Draws what's over a squircle's content: the lit (or dashed) edge, and the focus ring.
pub fn draw_over(
    renderer: &mut Renderer,
    painted: &Painted,
    outline: Outline,
    paint: &Paint,
    look: &Look,
    viewport: &Rectangle,
) {
    let Outline { bounds, radius, smoothing } = outline;
    if !(paint.edge || paint.dashed || paint.ring.is_some()) || bounds.width <= 1.0 || bounds.height <= 1.0 {
        return;
    }
    // Room around the box for the ring, which sits outside it.
    let pad = if paint.ring.is_some() { 4.0 } else { 0.0 };
    let mut key =
        vec![bounds.width, bounds.height, radius, smoothing, paint.edge as u8 as f32, paint.dashed as u8 as f32];
    key.extend(rgba(paint.ring.unwrap_or(Color::TRANSPARENT)));
    key.extend(rgba(look.glow));
    key.extend(rgba(look.line_strong));
    key.extend(look.edge());
    let outer = Size::new(bounds.width + pad * 2.0, bounds.height + pad * 2.0);
    let geometry = painted.edge.draw(renderer, outer, &key, |frame| {
        // Half a pixel in, so the line falls inside the box like the web's clipped stroke.
        let inside = path(
            Point::new(pad + 0.5, pad + 0.5),
            Size::new(bounds.width - 1.0, bounds.height - 1.0),
            (radius - 0.5).max(0.0),
            smoothing,
        );
        if paint.dashed {
            frame.stroke(
                &inside,
                Stroke {
                    line_dash: LineDash { segments: &[4.0, 4.0], offset: 0 },
                    ..Stroke::default().with_width(1.0).with_color(look.line_strong)
                },
            );
        } else if paint.edge {
            let [top, middle, bottom] = look.edge();
            let glow = look.glow;
            let gradient =
                iced_wgpu::graphics::gradient::Linear::new(Point::new(0.0, pad), Point::new(0.0, pad + bounds.height))
                    .add_stop(0.0, glow.scale_alpha(top))
                    .add_stop(0.35, glow.scale_alpha(middle))
                    .add_stop(1.0, glow.scale_alpha(bottom));
            frame.stroke(
                &inside,
                Stroke { style: canvas::Style::Gradient(gradient.into()), width: 1.0, ..Stroke::default() },
            );
        }
        if let Some(ring) = paint.ring {
            // `outline: 2px solid var(--color-focus)`, two pixels out.
            let around = path(
                Point::new(pad - 3.0, pad - 3.0),
                Size::new(bounds.width + 6.0, bounds.height + 6.0),
                radius + 3.0,
                smoothing,
            );
            frame.stroke(&around, Stroke::default().with_width(2.0).with_color(ring));
        }
    });
    renderer.with_layer(*viewport, |renderer| {
        renderer.with_translation(Vector::new(bounds.x - pad, bounds.y - pad), |renderer| {
            renderer.draw_geometry(geometry);
        });
    });
}

/// A squircle around `content`.
pub struct Squircle<'a, Message> {
    content: Element<'a, Message>,
    radius: f32,
    /// Whether the radius is exactly what's given rather than scaled by the style.
    exact: bool,
    smoothing: Option<f32>,
    paint: Paint,
    padding: Padding,
    width: Length,
    height: Length,
    look: Look,
}

/// `content` in a squircle of `radius` (Layered's; other styles scale it).
pub fn squircle<'a, Message>(
    content: impl Widget<Message, Theme, Renderer> + 'a,
    radius: f32,
    look: &Look,
) -> Squircle<'a, Message> {
    Squircle {
        content: Widget::_boxed(content),
        radius,
        exact: false,
        smoothing: None,
        paint: Paint::NONE,
        padding: Padding::ZERO,
        width: Length::Shrink,
        height: Length::Shrink,
        look: *look,
    }
}

impl<'a, Message> Squircle<'a, Message> {
    pub fn fill(mut self, color: Color) -> Self {
        self.paint.fill = Some(color);
        self
    }

    pub fn edge(mut self) -> Self {
        self.paint.edge = true;
        self
    }

    pub fn dashed(mut self) -> Self {
        self.paint.dashed = true;
        self
    }

    pub fn lift(mut self) -> Self {
        self.paint.lift = true;
        self
    }

    /// `material`: blurs what's behind on Glass, a solid fill elsewhere.
    pub fn material(mut self, material: Option<Material>) -> Self {
        self.paint.material = material;
        self
    }

    pub fn paint(mut self, paint: Paint) -> Self {
        self.paint = paint;
        self
    }

    pub fn round(mut self) -> Self {
        self.exact = true;
        self.smoothing = Some(0.0);
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

impl<Message> iced_core::widget::Meta for Squircle<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Squircle<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Painted>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Painted::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
        let size = self.content.size();
        self.width = self.width.stack(size.width);
        self.height = self.height.stack(size.height);
    }

    fn size(&self) -> Size<Length> {
        Size { width: self.width, height: self.height }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        layout::padded(tree, limits, self.width, self.height, self.padding, |tree, limits| {
            self.content.layout(tree, renderer, limits);
            tree.size
        });
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
        let painted = tree.state.downcast_ref::<Painted>();
        let radius = if self.exact { self.radius } else { self.look.radius(self.radius) };
        let smoothing = self.smoothing.unwrap_or(self.look.corners().1);
        draw_under(renderer, painted, Outline { bounds, radius, smoothing }, &self.paint, &self.look, viewport);
        let (layout, tree) = layout.iter(&tree.children).next().expect("content");
        renderer.with_layer(*viewport, |renderer| {
            self.content.draw(tree, renderer, theme, style, layout, cursor, viewport);
        });
        draw_over(renderer, painted, Outline { bounds, radius, smoothing }, &self.paint, &self.look, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
            self.content.operate(tree, layout, viewport, renderer, operation);
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
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (layout, tree) = layout.iter(&tree.children).next().expect("content");
        self.content.mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.overlay(tree, layout, renderer, viewport, translation, window)
    }
}
