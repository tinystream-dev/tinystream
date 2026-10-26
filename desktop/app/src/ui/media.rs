// SPDX-License-Identifier: AGPL-3.0-or-later

//! People and artwork: avatars (`Avatar.tsx`), images that develop as they arrive (`Img.tsx`).

use std::time::{Duration, Instant};

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{
    Alignment, Border, Color, ContentFit, Event, Gradient, Length, Radians, Rectangle, Renderer as _, Size, Theme,
    Widget, gradient, mouse, renderer, window,
};
use iced_wgpu::primitive::Renderer as _;
use iced_widget::{Element, Renderer, Widget as _, container, image};

use super::backdrop::{self, Backdrop};
use super::motion;
use super::text::{SEMIBOLD, inherit};
use super::theme::Look;

/// A stable, quiet hue per name, so pictureless people still look distinct.
pub fn hue(name: &str) -> f32 {
    let mut h: u32 = 0;
    for unit in name.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(unit as u32);
    }
    (h % 360) as f32
}

/// CSS's `oklch()` in sRGB.
pub fn oklch(l: f32, c: f32, h: f32) -> Color {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let linear = [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ];
    let encode = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    };
    Color::from_rgb(encode(linear[0]), encode(linear[1]), encode(linear[2]))
}

/// Someone's picture, or the first letter of their name on their hue.
pub fn avatar<'a, Message: 'a>(
    name: &str,
    picture: Option<&image::Handle>,
    size: f32,
    fallback: Option<Element<'a, Message>>,
    look: &Look,
) -> Element<'a, Message> {
    let h = hue(name);
    let ring = look.glow.scale_alpha(0.08);
    let inside: Element<'a, Message> = match picture {
        Some(picture) => image(picture.clone())
            .width(size)
            .height(size)
            .content_fit(ContentFit::Cover)
            .border_radius(size / 2.0)
            .boxed(),
        None => fallback.unwrap_or_else(|| {
            let letter: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
            inherit(letter, super::text::Size { px: (size * 0.42).round(), line: size })
                .font(SEMIBOLD)
                .color(oklch(0.9, 0.05, h))
                .boxed()
        }),
    };
    let tinted = picture.is_none();
    container(inside)
        .center(size)
        .style(move |_| container::Style {
            background: tinted.then(|| {
                // 145deg, from a lighter hue to a darker, turned one.
                Gradient::Linear(
                    gradient::Linear::new(Radians(145f32.to_radians()))
                        .add_stop(0.0, oklch(0.42, 0.06, h))
                        .add_stop(1.0, oklch(0.3, 0.045, (h + 40.0) % 360.0)),
                )
                .into()
            }),
            text_color: Some(oklch(0.9, 0.05, h)),
            border: Border { color: ring, width: 1.0, radius: (size / 2.0).into() },
            ..container::Style::default()
        })
        .align_x(Alignment::Center)
        .boxed()
}

/// An image that develops as it arrives: blurry, washed out and a little too big, then sharp
/// (`.develop`). One that came from the cache is just there.
pub fn develop<'a, Message: 'a>(
    content: impl Widget<Message, Theme, Renderer> + 'a,
    arrived: Option<Instant>,
    background: Color,
    radius: f32,
    look: &Look,
) -> Element<'a, Message> {
    Develop {
        content: Widget::_boxed(content),
        arrived: arrived.filter(|_| !look.reduce_motion),
        background,
        radius,
        smoothing: look.corners().1,
    }
    .boxed()
}

struct Develop<'a, Message> {
    content: Element<'a, Message>,
    arrived: Option<Instant>,
    background: Color,
    radius: f32,
    smoothing: f32,
}

const DEVELOP: Duration = Duration::from_millis(900);

impl<Message> iced_core::widget::Meta for Develop<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Develop<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let size = self.content.size();
        layout::padded(tree, limits, size.width, size.height, 0, |tree, limits| {
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
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.update(tree, event, child, cursor, renderer, shell, viewport);
        if let (Event::Window(window::Event::RedrawRequested(now)), Some(arrived)) = (event, self.arrived)
            && now.saturating_duration_since(arrived) < DEVELOP
        {
            shell.request_redraw();
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
        let (child, tree) = layout.iter(&tree.children).next().expect("content");
        self.content.draw(tree, renderer, theme, style, child, cursor, viewport);
        let Some(arrived) = self.arrived else { return };
        let t = motion::SETTLE.at(motion::progress(arrived, DEVELOP, Instant::now()));
        if t >= 1.0 {
            return;
        }
        // Opacity fades in faster (500ms) than the blur clears.
        let shown = motion::EASE_OUT.at(motion::progress(arrived, Duration::from_millis(500), Instant::now()));
        let tint = self.background.scale_alpha(1.0 - shown);
        renderer.with_layer(bounds.intersection(viewport).unwrap_or(bounds), |renderer| {
            if backdrop::available() {
                let backdrop = Backdrop::new(
                    14.0 * (1.0 - t),
                    motion::lerp(0.3, 1.0, t),
                    tint,
                    self.radius,
                    self.smoothing,
                    motion::lerp(1.06, 1.0, t),
                );
                renderer.draw_primitive(bounds, backdrop);
            } else {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: Border::default().rounded(self.radius),
                        ..renderer::Quad::default()
                    },
                    tint,
                );
            }
        });
    }
}

/// The web's poster card, including watch progress, fresh content and its caption.
pub struct Poster<'a> {
    pub name: &'a str,
    pub picture: Option<&'a image::Handle>,
    pub arrived: Option<Instant>,
    pub caption: String,
    pub progress: Option<f32>,
    pub fresh: Option<&'a str>,
}

pub fn poster<'a, Message: Clone + 'a>(card: Poster<'a>, on_press: Message, look: &Look) -> Element<'a, Message> {
    use iced_core::Padding;
    use iced_widget::{column, space};

    use super::pressable::{Appearance, pressable};
    use super::squircle::{Paint, squircle};
    use super::text::{self as type_, MEDIUM, text, truncate};

    let art = super::tilt::art(card.picture.cloned(), 14.0, 1.5, look);
    let mut layers = vec![develop(art, card.arrived, look.raised, look.radius(14.0), look)];
    if card.picture.is_none() {
        layers.push(
            container(text(card.name, type_::S15, look.ink_2).font(MEDIUM))
                .align_bottom(Length::Fill)
                .width(Length::Fill)
                .padding(12)
                .boxed(),
        );
    }
    if let Some(progress) = card.progress {
        let ink = look.media_ink;
        let track = look.media_shade.scale_alpha(0.5);
        let bar = iced_widget::responsive(move |size| {
            let fill = container(space().width(size.width * progress.clamp(0.04, 1.0)).height(4)).style(move |_| {
                container::Style {
                    background: Some(ink.into()),
                    border: Border::default().rounded(2),
                    ..Default::default()
                }
            });
            container(fill)
                .width(Length::Fill)
                .height(4)
                .style(move |_| container::Style {
                    background: Some(track.into()),
                    border: Border::default().rounded(2),
                    ..Default::default()
                })
                .boxed()
        })
        .height(4);
        layers.push(container(bar).align_bottom(Length::Fill).padding(10).boxed());
    }
    if let Some(fresh) = card.fresh {
        layers.push(
            container(
                squircle(text(fresh, type_::XS2, look.media_ink).font(MEDIUM), 6.0, look)
                    .fill(look.media_shade.scale_alpha(0.6))
                    .padding([2, 6]),
            )
            .align_right(Length::Fill)
            .padding(8)
            .boxed(),
        );
    }
    let art = iced_widget::Stack::with_children(layers).width(Length::Fill);
    let ink = look.ink;
    pressable(
        column![
            art,
            container(truncate(card.name, type_::S13, look.ink).font(MEDIUM))
                .padding(Padding { top: 8.0, ..Padding::ZERO }),
            truncate(card.caption, type_::XS, look.ink_3),
        ],
        14.0,
        look,
        move |_| Appearance { paint: Paint::NONE, ink },
    )
    .width(Length::Fill)
    .on_press(on_press)
    .boxed()
}

/// A cached image, developing only on its first network arrival.
pub fn img<'a, Message: 'a>(
    picture: Option<&crate::images::Picture>,
    radius: f32,
    look: &Look,
) -> Element<'a, Message> {
    match picture {
        Some(crate::images::Picture::Ready(handle, arrived)) => develop(
            image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(ContentFit::Cover)
                .border_radius(look.radius(radius)),
            *arrived,
            look.raised,
            look.radius(radius),
            look,
        ),
        Some(crate::images::Picture::Missing) => {
            super::squircle::squircle(iced_widget::space().width(Length::Fill).height(Length::Fill), radius, look)
                .fill(look.raised)
                .boxed()
        },
        _ => super::effects::bone_rounded(Length::Fill, Length::Fill, look.radius(radius), look),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hues_match_the_web() {
        // ((((68·31 + 97)·31 + 118)·31 + 105)·31 + 100) mod 360, as `hue()` in Avatar.tsx has it.
        assert_eq!(hue("David"), 68.0);
        assert_eq!(hue(""), 0.0);
    }

    #[test]
    fn oklch_white_and_black() {
        let white = oklch(1.0, 0.0, 0.0);
        assert!((white.r - 1.0).abs() < 1e-3 && (white.g - 1.0).abs() < 1e-3 && (white.b - 1.0).abs() < 1e-3);
        let black = oklch(0.0, 0.0, 0.0);
        assert!(black.r < 1e-4);
    }
}
