// SPDX-License-Identifier: AGPL-3.0-or-later

//! The logo in the scheme's colours, finished like the component style (`Logo.tsx`).

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{Length, Point, Rectangle, Renderer as _, Size, Theme, Vector, Widget, mouse, renderer};
use iced_wgpu::graphics::geometry::Renderer as _;
use iced_widget::canvas::{self, Stroke};
use iced_widget::{Element, Renderer, Widget as _};

use super::icon::{Step, path};
use super::squircle::Keyed;
use super::theme::{Look, Style, mix};

include!(concat!(env!("OUT_DIR"), "/logo.rs"));

pub fn logo<'a, Message: 'a>(size: f32, look: &Look) -> Element<'a, Message> {
    Logo { size, look: *look }.boxed()
}

struct Logo {
    size: f32,
    look: Look,
}

impl iced_core::widget::Meta for Logo {}

impl<Message> Widget<Message, Theme, Renderer> for Logo {
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
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let look = &self.look;
        let scale = self.size / 1024.0;
        let mut key = vec![self.size, look.style as u8 as f32];
        for c in [look.ink, look.canvas, look.shade, look.glow] {
            key.extend([c.r, c.g, c.b, c.a]);
        }
        let geometry = tree.state.downcast_ref::<Keyed>().draw(renderer, bounds.size(), &key, |frame| {
            let gradient = |top, bottom| {
                iced_wgpu::graphics::gradient::Linear::new(Point::ORIGIN, Point::new(0.0, self.size))
                    .add_stop(0.0, top)
                    .add_stop(1.0, bottom)
            };
            match look.style {
                Style::Glass => {
                    // A clear pane with "Ts" cut out of it, its rim catching the light.
                    let cut = path(CUT, scale);
                    let glass = |amount: f32| look.glow.scale_alpha(amount);
                    let body = iced_wgpu::graphics::gradient::Linear::new(
                        Point::ORIGIN,
                        Point::new(self.size * 0.3, self.size),
                    )
                    .add_stop(0.0, glass(if look.dark { 0.14 } else { 0.5 }))
                    .add_stop(0.6, glass(if look.dark { 0.06 } else { 0.28 }))
                    .add_stop(1.0, glass(if look.dark { 0.1 } else { 0.4 }));
                    frame.fill(
                        &cut,
                        canvas::Fill { style: canvas::Style::Gradient(body.into()), ..canvas::Fill::default() },
                    );
                    let rim =
                        if look.dark { iced_core::Color::WHITE.scale_alpha(0.32) } else { look.ink.scale_alpha(0.35) };
                    frame.stroke(&cut, Stroke::default().with_width((self.size / 48.0).max(0.75)).with_color(rim));
                },
                style => {
                    let plate = path(PLATE, scale);
                    if style == Style::Layered {
                        let fill = gradient(look.ink, mix(look.ink, look.shade, 0.86));
                        frame.fill(
                            &plate,
                            canvas::Fill { style: canvas::Style::Gradient(fill.into()), ..canvas::Fill::default() },
                        );
                    } else {
                        frame.fill(&plate, look.ink);
                    }
                    frame.fill(&path(GLYPH, scale), look.canvas);
                },
            }
        });
        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| renderer.draw_geometry(geometry));
    }
}
