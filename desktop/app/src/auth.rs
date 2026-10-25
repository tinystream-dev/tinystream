// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::widget::{Operation, Tree, tree};
use iced_core::{
    Color, Event, Layout, Length, Point, Rectangle, Renderer as _, Shadow, Size, Theme, Vector, Widget as CoreWidget,
    layout, mouse, overlay, renderer,
};
use iced_wgpu::Renderer;
use iced_wgpu::graphics::geometry::Renderer as _;
use iced_widget::canvas::{self, Frame, Path, Stroke};
use iced_widget::{Element, Widget, button, container, text};

use crate::app::Message;
use crate::style::Look;

pub const MEDIUM: iced_core::Font =
    iced_core::Font { weight: iced_core::font::Weight::Medium, ..iced_core::Font::DEFAULT };
pub const SEMIBOLD: iced_core::Font =
    iced_core::Font { weight: iced_core::font::Weight::Semibold, ..iced_core::Font::DEFAULT };

pub fn panel<'a>(content: Element<'a, Message>, look: Look) -> Element<'a, Message> {
    surface(container(content).padding(20).boxed(), look, look.float, 20.0, true)
}

pub fn surface<'a>(
    content: Element<'a, Message>,
    look: Look,
    fill: Color,
    radius: f32,
    shadow: bool,
) -> Element<'a, Message> {
    Surface { content, look, fill, radius: radius * look.radius_scale, shadow }.boxed()
}

pub fn action(label: &str, message: super::session::Message, enabled: bool, look: Look) -> Element<'static, Message> {
    button(container(text(label.to_owned()).size(15).font(MEDIUM)).center(Length::Fill))
        .width(Length::Fill)
        .height(44)
        .padding([0, 20])
        .on_press_maybe(enabled.then_some(Message::Session(message)))
        .style(move |_, status| {
            let disabled = status == button::Status::Disabled;
            let bg = if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                look.accent_hover
            } else {
                look.accent
            };
            button::Style {
                background: Some(blend(bg, look.float, if disabled { 0.4 } else { 1.0 }).into()),
                text_color: blend(look.on_accent, look.float, if disabled { 0.4 } else { 1.0 }),
                border: iced_core::border::rounded(14.0 * look.radius_scale),
                ..button::Style::default()
            }
        })
        .boxed()
}

pub fn plain(label: &str, message: super::session::Message, enabled: bool, look: Look) -> Element<'static, Message> {
    button(text(label.to_owned()).size(13))
        .padding([5, 8])
        .on_press_maybe(enabled.then_some(Message::Session(message)))
        .style(move |_, status| button::Style {
            text_color: if status == button::Status::Hovered { look.ink } else { look.muted },
            background: (status == button::Status::Pressed).then_some(look.line.into()),
            border: iced_core::border::rounded(8),
            ..button::Style::default()
        })
        .boxed()
}

fn blend(top: Color, bottom: Color, alpha: f32) -> Color {
    Color::from_rgb(
        top.r * alpha + bottom.r * (1.0 - alpha),
        top.g * alpha + bottom.g * (1.0 - alpha),
        top.b * alpha + bottom.b * (1.0 - alpha),
    )
}

struct Surface<'a> {
    content: Element<'a, Message>,
    look: Look,
    fill: Color,
    radius: f32,
    shadow: bool,
}

impl iced_core::widget::Meta for Surface<'_> {}

impl CoreWidget<Message, Theme, Renderer> for Surface<'_> {
    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn tag(&self) -> tree::Tag {
        self.content.tag()
    }

    fn state(&self) -> tree::State {
        self.content.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.diff(tree);
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.content.layout(tree, renderer, limits);
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
        renderer.with_layer(*viewport, |renderer| {
            if self.shadow && self.look.elevated {
                for (y, blur, alpha) in [(1.0, 1.0, 0.35), (12.0, 32.0, 0.45)] {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: iced_core::border::rounded(self.radius),
                            shadow: Shadow {
                                color: self.look.shade.scale_alpha(alpha),
                                offset: Vector::new(0.0, y),
                                blur_radius: blur * 2.0,
                            },
                            ..renderer::Quad::default()
                        },
                        Color::TRANSPARENT,
                    );
                }
            }
            let mut frame = Frame::new(renderer, bounds.size());
            // The same continuous corners and lit edge as packages/shared/src/squircle.ts.
            let path = squircle(bounds.size(), self.radius, self.look.smoothing);
            frame.fill(&path, self.fill);
            let glow = self.look.glow;
            let edge = iced_wgpu::graphics::gradient::Linear::new(Point::ORIGIN, Point::new(0.0, bounds.height))
                .add_stop(0.0, glow.scale_alpha(0.14))
                .add_stop(0.35, glow.scale_alpha(0.07))
                .add_stop(1.0, glow.scale_alpha(0.045));
            frame
                .stroke(&path, Stroke { style: canvas::Style::Gradient(edge.into()), width: 1.0, ..Stroke::default() });
            renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
                renderer.draw_geometry(frame.into_geometry())
            });
        });
        renderer.with_layer(*viewport, |renderer| {
            self.content.draw(tree, renderer, theme, style, layout, cursor, viewport)
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
        self.content.operate(tree, layout, viewport, renderer, operation);
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
        self.content.mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.overlay(tree, layout, renderer, viewport, translation, window)
    }
}

fn squircle(size: Size, radius: f32, smoothing: f32) -> Path {
    let r = radius.min(size.width.min(size.height) / 2.0);
    let s = smoothing.min((size.width.min(size.height) / 2.0 / r - 1.0).max(0.0));
    let p = (1.0 + s) * r;
    let beta = (45.0 * s).to_radians();
    let arc = (45.0 * (1.0 - s)).to_radians().sin() * r * 2.0_f32.sqrt();
    let c = r * (beta / 2.0).tan() * beta.cos();
    let d = c * beta.tan();
    let b = (p - arc - c - d) / 3.0;
    let a = 2.0 * b;
    let k = 4.0 / 3.0 * ((90.0 * (1.0 - s)).to_radians() / 4.0).tan() * r;
    Path::new(|path| {
        path.move_to(Point::new(size.width - p, 0.0));
        for side in 0..4 {
            let map = |x, y| match side {
                0 => Point::new(size.width - p + x, y),
                1 => Point::new(size.width - y, size.height - p + x),
                2 => Point::new(p - x, size.height - y),
                _ => Point::new(y, p - x),
            };
            path.line_to(map(0.0, 0.0));
            path.bezier_curve_to(map(a, 0.0), map(a + b, 0.0), map(a + b + c, d));
            let x = a + b + c;
            let y = d;
            path.bezier_curve_to(
                map(x + k * beta.cos(), y + k * beta.sin()),
                map(x + arc - k * beta.sin(), y + arc - k * beta.cos()),
                map(x + arc, y + arc),
            );
            path.bezier_curve_to(map(p, p - a - b), map(p, p - a), map(p, p));
        }
        path.close();
    })
}

include!(concat!(env!("OUT_DIR"), "/logo.rs"));

pub fn logo<'a>(color: Color, size: f32) -> Element<'a, Message> {
    canvas::Canvas::new(Logo(color)).width(size).height(size).boxed()
}

struct Logo(Color);
impl canvas::Program<Message> for Logo {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.scale(bounds.width / 1024.0);
        frame.fill(&logo_path(), self.0);
        vec![frame.into_geometry()]
    }
}

fn logo_path() -> Path {
    let source = LOGO
        .chars()
        .flat_map(|ch| if ch.is_ascii_alphabetic() { format!(" {ch} ").chars().collect::<Vec<_>>() } else { vec![ch] })
        .collect::<String>();
    let mut tokens = source.split_whitespace();
    Path::new(|path| {
        let mut current = Point::ORIGIN;
        while let Some(command) = tokens.next() {
            let mut number = || tokens.next().expect("shared logo coordinate").parse::<f32>().expect("logo number");
            match command {
                "M" | "L" => {
                    current = Point::new(number(), number());
                    if command == "M" {
                        path.move_to(current);
                    } else {
                        path.line_to(current);
                    }
                },
                "C" => {
                    let one = Point::new(number(), number());
                    let two = Point::new(number(), number());
                    current = Point::new(number(), number());
                    path.bezier_curve_to(one, two, current);
                },
                "A" => {
                    let radius = number();
                    let _ry = number();
                    let _rotation = number();
                    let large = number() != 0.0;
                    let sweep = number() != 0.0;
                    let end = Point::new(number(), number());
                    let half = (current - end) * 0.5;
                    let factor = ((radius * radius - half.x * half.x - half.y * half.y)
                        / (half.x * half.x + half.y * half.y))
                        .max(0.0)
                        .sqrt()
                        * if large == sweep { -1.0 } else { 1.0 };
                    let center = Point::new(
                        (current.x + end.x) / 2.0 + factor * half.y,
                        (current.y + end.y) / 2.0 - factor * half.x,
                    );
                    let start = (current.y - center.y).atan2(current.x - center.x);
                    let mut delta = (end.y - center.y).atan2(end.x - center.x) - start;
                    if sweep && delta < 0.0 {
                        delta += std::f32::consts::TAU;
                    }
                    if !sweep && delta > 0.0 {
                        delta -= std::f32::consts::TAU;
                    }
                    for step in 1..=8 {
                        let angle = start + delta * step as f32 / 8.0;
                        path.line_to(Point::new(center.x + radius * angle.cos(), center.y + radius * angle.sin()));
                    }
                    current = end;
                },
                "Z" => path.close(),
                _ => unreachable!("shared logo command"),
            }
        }
    })
}
