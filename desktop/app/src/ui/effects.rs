// SPDX-License-Identifier: AGPL-3.0-or-later

//! Motion and light from `styles.css`: things rising or popping into place, pages sharpening in,
//! ambient tints behind artwork and shimmering placeholders. All of it holds still with reduced
//! motion.

use std::time::{Duration, Instant};

use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{
    Background, Border, Color, Event, Gradient, Length, Radians, Rectangle, Renderer as _, Size, Theme, Transformation,
    Vector, Widget, gradient, mouse, overlay, renderer, window,
};
use iced_wgpu::graphics::color;
use iced_wgpu::graphics::mesh::{Indexed, Mesh, SolidVertex2D};
use iced_wgpu::primitive::Renderer as _;
use iced_widget::{Element, Renderer};

use super::backdrop::{self, Backdrop};
use super::motion::{self, Curve};
use super::theme::{Look, Rounded};

/// An entrance: where something starts, and how it gets to where it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrance {
    pub from: Vector,
    pub scale: f32,
    pub duration: Duration,
    pub curve: Curve,
    pub delay: Duration,
}

impl Entrance {
    /// `pop`: menus and popovers, from just above and a little small.
    pub fn pop(look: &Look) -> Self {
        let (y, scale) = look.pop();
        Self {
            from: Vector::new(0.0, y),
            scale,
            duration: Duration::from_millis(140),
            curve: motion::EASE_OUT,
            delay: Duration::ZERO,
        }
    }

    /// `rise`: toasts, from below.
    pub fn rise() -> Self {
        Self {
            from: Vector::new(0.0, 8.0),
            scale: 0.98,
            duration: Duration::from_millis(220),
            curve: motion::GLIDE,
            delay: Duration::ZERO,
        }
    }

    /// `develop-in`: content replacing a skeleton, the `n`th part of it a little later.
    pub fn arrive(n: usize) -> Self {
        Self {
            from: Vector::new(0.0, 6.0),
            scale: 1.0,
            duration: Duration::from_millis(900),
            curve: motion::SETTLE,
            delay: Duration::from_millis(70 * n.min(5) as u64),
        }
    }
}

/// `content`, moving into place the first time it's shown.
pub fn enter<'a, Message: 'a>(
    content: impl Widget<Message, Theme, Renderer> + 'a,
    entrance: Entrance,
    look: &Look,
) -> Element<'a, Message> {
    iced_widget::Widget::boxed(Enter { content: Widget::_boxed(content), entrance, still: look.reduce_motion })
}

struct Enter<'a, Message> {
    content: Element<'a, Message>,
    entrance: Entrance,
    still: bool,
}

#[derive(Default)]
struct Since {
    start: Option<Instant>,
    now: Option<Instant>,
}

impl<Message> Enter<'_, Message> {
    fn t(&self, since: &Since) -> f32 {
        match (since.start, since.now) {
            (Some(start), Some(now)) if !self.still => {
                let begun = start + self.entrance.delay;
                if now < begun { 0.0 } else { motion::progress(begun, self.entrance.duration, now) }
            },
            (None, _) if !self.still => 0.0,
            _ => 1.0,
        }
    }
}

impl<Message> iced_core::widget::Meta for Enter<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Enter<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Since>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Since::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        layout::padded(tree, limits, self.content.size().width, self.content.size().height, 0, |tree, limits| {
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let since = tree.state.downcast_mut::<Since>();
            since.start.get_or_insert(*now);
            since.now = Some(*now);
            if self.t(since) < 1.0 {
                shell.request_redraw();
            }
        }
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.update(tree, event, layout, cursor, renderer, shell, viewport);
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
        let t = self.entrance.curve.at(self.t(tree.state.downcast_ref::<Since>()));
        let (layout, tree) = layout.iter(&tree.children).next().expect("content");
        if t >= 1.0 {
            return self.content.draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
        let scale = motion::lerp(self.entrance.scale, 1.0, t);
        let offset = self.entrance.from * (1.0 - t);
        // Scaled around the middle, as CSS transforms are.
        let center = bounds.center();
        let transformation = Transformation::translate(center.x + offset.x, center.y + offset.y)
            * Transformation::scale(scale)
            * Transformation::translate(-center.x, -center.y);
        renderer.with_transformation(transformation, |renderer| {
            self.content.draw(tree, renderer, theme, style, layout, cursor, viewport);
        });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.operate(tree, layout, viewport, renderer, operation);
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

/// `content`, rising by `rise` and growing to `scale` while the pointer's over it (`transition
/// duration-300 ease-out`).
pub fn lift_on_hover<'a, Message: 'a>(
    content: impl Widget<Message, Theme, Renderer> + 'a,
    rise: f32,
    scale: f32,
    look: &Look,
) -> Element<'a, Message> {
    iced_widget::Widget::boxed(Lift { content: Widget::_boxed(content), rise, scale, still: look.reduce_motion })
}

struct Lift<'a, Message> {
    content: Element<'a, Message>,
    rise: f32,
    scale: f32,
    still: bool,
}

#[derive(Default)]
struct Lifted {
    over: bool,
    t: f32,
    last: Option<Instant>,
}

impl<Message> iced_core::widget::Meta for Lift<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Lift<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Lifted>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Lifted::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        layout::padded(tree, limits, self.content.size().width, self.content.size().height, 0, |tree, limits| {
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
        {
            let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
            self.content.update(tree, event, child, cursor, renderer, shell, viewport);
        }
        let state = tree.state.downcast_mut::<Lifted>();
        if self.still {
            state.t = 0.0;
        }
        let over = cursor.is_over(layout.bounds()) && !self.still;
        if over != state.over {
            state.over = over;
            state.last = None;
            shell.request_redraw();
        }
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let dt = state.last.map_or(0.0, |last| now.saturating_duration_since(last).as_secs_f32());
            state.last = Some(*now);
            let target = if state.over { 1.0 } else { 0.0 };
            let step = dt / 0.3;
            state.t = if state.t < target { (state.t + step).min(1.0) } else { (state.t - step).max(0.0) };
            if state.t != target {
                shell.request_redraw();
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
        let t = motion::EASE_OUT.at(tree.state.downcast_ref::<Lifted>().t);
        let (child, tree) = layout.iter(&tree.children).next().expect("content");
        if t <= 0.0 {
            return self.content.draw(tree, renderer, theme, style, child, cursor, viewport);
        }
        let center = bounds.center();
        let scale = motion::lerp(1.0, self.scale, t);
        let transformation = Transformation::translate(center.x, center.y + self.rise * t)
            * Transformation::scale(scale)
            * Transformation::translate(-center.x, -center.y);
        renderer.with_layer(*viewport, |renderer| {
            renderer.with_transformation(transformation, |renderer| {
                self.content.draw(tree, renderer, theme, style, child, cursor, viewport);
            });
        });
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
}

/// How a page looks part way through changing: the old one blurring away, then the new one
/// sharpening in (`page-out`, `page-in`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Veil {
    /// How much of the page shows through.
    pub opacity: f32,
    pub blur: f32,
    pub zoom: f32,
}

/// Where a route change is after `elapsed`: the old page while it's still leaving, then the new
/// one; `None` once it's over.
pub fn page_transition(elapsed: Duration) -> Option<(bool, Veil)> {
    const OUT: Duration = Duration::from_millis(220);
    const IN: Duration = Duration::from_millis(420);
    if elapsed < OUT {
        let t = motion::EASE_IN.at(elapsed.as_secs_f32() / OUT.as_secs_f32());
        Some((false, Veil { opacity: 1.0 - t, blur: 8.0 * t, zoom: motion::lerp(1.0, 0.992, t) }))
    } else if elapsed < OUT + IN {
        let t = motion::SETTLE.at((elapsed - OUT).as_secs_f32() / IN.as_secs_f32());
        Some((true, Veil { opacity: t, blur: 8.0 * (1.0 - t), zoom: motion::lerp(1.008, 1.0, t) }))
    } else {
        None
    }
}

/// A veil over everything under it in `bounds`: what's there, blurred and faded into `canvas`.
pub fn veil<'a, Message: 'a>(veil: Veil, look: &Look) -> Element<'a, Message> {
    iced_widget::Widget::boxed(Cover { veil, canvas: look.canvas })
}

struct Cover {
    veil: Veil,
    canvas: Color,
}

impl iced_core::widget::Meta for Cover {}

impl<Message> Widget<Message, Theme, Renderer> for Cover {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let tint = self.canvas.scale_alpha(1.0 - self.veil.opacity.clamp(0.0, 1.0));
        renderer.with_layer(*viewport, |renderer| {
            if backdrop::available() {
                renderer.draw_primitive(bounds, Backdrop::new(self.veil.blur, 1.0, tint, 0.0, 0.0, self.veil.zoom));
            } else {
                renderer.fill_quad(renderer::Quad { bounds, ..renderer::Quad::default() }, tint);
            }
        });
    }
}

/// An ambient glow: `radial-gradient(<size> at <center>, color, transparent <reach>)`, with the
/// size and centre as fractions of the box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ambient {
    pub color: Color,
    pub size: (f32, f32),
    pub center: (f32, f32),
    pub reach: f32,
}

impl Ambient {
    /// Behind the title page's hero (`70% 60% at 15% 0%`, fading by 70%).
    pub fn title(color: Color) -> Self {
        Self { color: color.scale_alpha(0.22), size: (0.7, 0.6), center: (0.15, 0.0), reach: 0.7 }
    }

    /// Behind Home's featured card (`120% 90% at 0% 100%`, fading by 60%).
    pub fn corner(color: Color) -> Self {
        Self { color: color.scale_alpha(0.38), size: (1.2, 0.9), center: (0.0, 1.0), reach: 0.6 }
    }
}

/// An ambient glow filling its space.
pub fn ambient<'a, Message: 'a>(glow: Ambient) -> Element<'a, Message> {
    iced_widget::Widget::boxed(Glow(glow))
}

struct Glow(Ambient);

impl iced_core::widget::Meta for Glow {}

impl<Message> Widget<Message, Theme, Renderer> for Glow {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced_wgpu::graphics::mesh::Renderer as _;
        let bounds = layout.bounds();
        let Ambient { color, size, center, reach } = self.0;
        if color.a <= 0.0 {
            return;
        }
        // CSS sizes an ellipse by its radii; rings out to `reach` of them fade linearly to nothing.
        let (rx, ry) = (size.0 * bounds.width, size.1 * bounds.height);
        let (cx, cy) = (center.0 * bounds.width, center.1 * bounds.height);
        const RINGS: usize = 24;
        const SPOKES: usize = 96;
        let mut vertices = vec![SolidVertex2D { position: [cx, cy], color: color::pack(color) }];
        for ring in 1..=RINGS {
            let r = reach * ring as f32 / RINGS as f32;
            let c = color.scale_alpha(1.0 - ring as f32 / RINGS as f32);
            for spoke in 0..SPOKES {
                let a = Radians(std::f32::consts::TAU * spoke as f32 / SPOKES as f32).0;
                vertices.push(SolidVertex2D {
                    position: [cx + a.cos() * rx * r, cy + a.sin() * ry * r],
                    color: color::pack(c),
                });
            }
        }
        let mut indices = Vec::new();
        let at = |ring: usize, spoke: usize| (1 + (ring - 1) * SPOKES + spoke % SPOKES) as u32;
        for spoke in 0..SPOKES {
            indices.extend([0, at(1, spoke), at(1, spoke + 1)]);
        }
        for ring in 1..RINGS {
            for spoke in 0..SPOKES {
                let (a, b, c, d) = (at(ring, spoke), at(ring, spoke + 1), at(ring + 1, spoke), at(ring + 1, spoke + 1));
                indices.extend([a, c, b, b, c, d]);
            }
        }
        let clip = Rectangle { x: 0.0, y: 0.0, ..bounds };
        renderer.with_layer(bounds.intersection(viewport).unwrap_or(bounds), |renderer| {
            renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
                renderer.draw_mesh(Mesh::Solid {
                    buffers: Indexed { vertices, indices },
                    transformation: Transformation::IDENTITY,
                    clip_bounds: clip,
                });
            });
        });
    }
}

/// `bone`: a placeholder shaped like the content about to arrive, with light drifting across.
pub fn bone<'a, Message: 'a>(
    width: impl Into<Length>,
    height: impl Into<Length>,
    radius: Rounded,
    look: &Look,
) -> Element<'a, Message> {
    iced_widget::Widget::boxed(Bone {
        width: width.into(),
        height: height.into(),
        radius: look.rounded(radius),
        look: *look,
    })
}

/// A bone with exact corners, like a poster's.
pub fn bone_rounded<'a, Message: 'a>(
    width: impl Into<Length>,
    height: impl Into<Length>,
    radius: f32,
    look: &Look,
) -> Element<'a, Message> {
    iced_widget::Widget::boxed(Bone { width: width.into(), height: height.into(), radius, look: *look })
}

struct Bone {
    width: Length,
    height: Length,
    radius: f32,
    look: Look,
}

impl iced_core::widget::Meta for Bone {}

impl<Message> Widget<Message, Theme, Renderer> for Bone {
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, self.width, self.height);
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(_)) = event
            && !self.look.reduce_motion
        {
            shell.request_redraw();
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
        let bounds = layout.bounds();
        let (raised, panel) = (self.look.raised, self.look.panel);
        let quad =
            renderer::Quad { bounds, border: Border::default().rounded(self.radius), ..renderer::Quad::default() };
        if self.look.reduce_motion {
            return renderer.fill_quad(quad, raised);
        }
        // `linear-gradient(90deg, raised 30%, panel 50%, raised 70%) 0 0 / 250% 100%`, its position
        // going from 150% to -50% every 1.6s, eased in and out. In the box's own widths, the bright
        // band's middle goes from -1 to 2, and it's half a width each way.
        let t = motion::EASE_IN_OUT.at(epoch().elapsed().as_secs_f32() % 1.6 / 1.6);
        let middle = 1.25 - 1.5 * motion::lerp(1.5, -0.5, t);
        let at = |x: f32| super::theme::mix(panel, raised, (1.0 - (x - middle).abs() / 0.5).max(0.0));
        let mut stops = vec![0.0, middle - 0.5, middle, middle + 0.5, 1.0];
        stops.iter_mut().for_each(|x| *x = x.clamp(0.0, 1.0));
        let gradient = stops
            .into_iter()
            .fold(gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2)), |g, x| g.add_stop(x, at(x)));
        renderer.fill_quad(quad, Background::Gradient(Gradient::Linear(gradient)));
    }
}

/// When shimmering started, so every bone on screen moves together.
fn epoch() -> Instant {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}
