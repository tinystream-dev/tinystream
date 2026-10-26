// SPDX-License-Identifier: AGPL-3.0-or-later

//! The pieces screens are made of, after `web/src/components/ui.tsx`.

use std::time::{Duration, Instant};

use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{
    Alignment, Background, Border, Color, Event, Length, Padding, Point, Rectangle, Renderer as _, Size, Theme, Vector,
    Widget as CoreWidget, mouse, renderer, window,
};
use iced_wgpu::graphics::geometry::Renderer as _;
use iced_widget::canvas::path::Arc;
use iced_widget::canvas::{LineCap, Path, Stroke};
use iced_widget::{Element, Renderer, Row, Widget, column, container, row, space, text_input, tooltip};

use super::icon::{Icon, icon};
use super::pressable::{Appearance, Status, pressable};
use super::squircle::{Keyed, Paint, squircle};
use super::text::{self as type_, MEDIUM, inherit, text};
use super::theme::{Look, Rounded};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    Primary,
    #[default]
    Quiet,
    Plain,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scale {
    Sm,
    #[default]
    Md,
    Lg,
}

/// Disabled things fade to 40%, as `disabled:opacity-40`.
fn fade(color: Color, disabled: bool) -> Color {
    if disabled { color.scale_alpha(0.4) } else { color }
}

/// How a button of `variant` looks in `status`.
pub fn button_style(look: &Look, variant: Variant) -> impl Fn(Status) -> Appearance + 'static {
    let look = *look;
    move |status| {
        let disabled = status == Status::Disabled;
        let (fill, ink, edge) = match (variant, status) {
            (Variant::Primary, Status::Hovered) => (Some(look.accent_hover), look.on_accent, false),
            (Variant::Primary, Status::Pressed) => (Some(look.accent.scale_alpha(0.85)), look.on_accent, false),
            (Variant::Primary, _) => (Some(look.accent), look.on_accent, false),
            (Variant::Quiet, Status::Hovered) => (Some(look.float), look.ink, true),
            (Variant::Quiet, _) => (Some(look.panel), look.ink, true),
            (Variant::Plain, Status::Hovered) => (Some(look.hover), look.ink, false),
            (Variant::Plain, Status::Pressed) => (Some(look.press), look.ink, false),
            (Variant::Plain, _) => (None, look.ink_2, false),
            (Variant::Danger, Status::Hovered) => (Some(look.danger.scale_alpha(0.1)), look.danger, false),
            (Variant::Danger, Status::Pressed) => (Some(look.danger.scale_alpha(0.15)), look.danger, false),
            (Variant::Danger, _) => (None, look.danger, false),
        };
        Appearance {
            paint: Paint { fill: fill.map(|c| fade(c, disabled)), edge: edge && !disabled, ..Paint::NONE },
            ink: fade(ink, disabled),
        }
    }
}

/// `<Button>`.
pub struct Button<'a, Message> {
    look: Look,
    variant: Variant,
    scale: Scale,
    icon: Option<Icon>,
    label: Option<String>,
    width: Length,
    radius: Option<f32>,
    on_press: Option<Message>,
    content: Option<Element<'a, Message>>,
}

pub fn button<'a, Message>(label: impl Into<String>, look: &Look) -> Button<'a, Message> {
    Button {
        look: *look,
        variant: Variant::Quiet,
        scale: Scale::Md,
        icon: None,
        label: Some(label.into()),
        width: Length::Shrink,
        radius: None,
        on_press: None,
        content: None,
    }
}

impl<'a, Message: Clone + 'a> Button<'a, Message> {
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(Variant::Primary)
    }

    pub fn plain(self) -> Self {
        self.variant(Variant::Plain)
    }

    pub fn danger(self) -> Self {
        self.variant(Variant::Danger)
    }

    pub fn scale(mut self, scale: Scale) -> Self {
        self.scale = scale;
        self
    }

    pub fn small(self) -> Self {
        self.scale(Scale::Sm)
    }

    pub fn large(self) -> Self {
        self.scale(Scale::Lg)
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// `w-full`.
    pub fn fill(mut self) -> Self {
        self.width = Length::Fill;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }

    /// Something other than an icon and a label, like a spinner.
    pub fn content(mut self, content: impl CoreWidget<Message, Theme, Renderer> + 'a) -> Self {
        self.content = Some(CoreWidget::_boxed(content));
        self
    }

    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn on_press_maybe(mut self, message: Option<Message>) -> Self {
        self.on_press = message;
        self
    }
}

impl<'a, Message: Clone + 'a> Button<'a, Message> {
    pub fn boxed(self) -> Element<'a, Message> {
        let b = self;
        let (height, pad, size, gap, glyph) = match b.scale {
            Scale::Sm => (28.0, 10.0, type_::S13, 6.0, 14.0),
            Scale::Md => (32.0, 12.0, type_::SM, 8.0, 16.0),
            Scale::Lg => (44.0, 20.0, type_::S15, 10.0, 18.0),
        };
        let content = b.content.unwrap_or_else(|| {
            let mut label = Row::new().spacing(gap).align_y(Alignment::Center);
            if let Some(i) = b.icon {
                label = label.push(icon(i, glyph).boxed());
            }
            if let Some(words) = b.label.filter(|l| !l.is_empty()) {
                let words = inherit(words, size).wrapping(iced_core::text::Wrapping::None);
                label = label.push(if b.scale == Scale::Lg { words.font(MEDIUM) } else { words }.boxed());
            }
            label.boxed()
        });
        let inner = container(content).center_y(Length::Fill);
        let inner = if b.width == Length::Fill { inner.center_x(Length::Fill) } else { inner };
        pressable(
            inner,
            b.radius.unwrap_or(if b.scale == Scale::Lg { 14.0 } else { 9.0 }),
            &b.look,
            button_style(&b.look, b.variant),
        )
        .width(b.width)
        .height(height)
        .padding([0.0, pad])
        .on_press_maybe(b.on_press)
        .boxed()
    }
}

/// `<IconButton>`: a square of an icon, with its label as a tip.
pub fn icon_button<'a, Message: Clone + 'a>(
    glyph: Icon,
    label: &'a str,
    message: Option<Message>,
    look: &Look,
) -> Element<'a, Message> {
    let l = *look;
    let button = pressable(container(icon(glyph, 16.0)).center(32), 8.0, look, move |status| {
        let (fill, ink) = match status {
            Status::Hovered => (Some(l.hover), l.ink),
            Status::Pressed => (Some(l.press), l.ink),
            _ => (None, l.ink_2),
        };
        Appearance { paint: Paint { fill, ..Paint::NONE }, ink: fade(ink, status == Status::Disabled) }
    })
    .on_press_maybe(message);
    tip(button, label, look)
}

/// A short label under something once the pointer rests on it (`useTip`).
pub fn tip<'a, Message: 'a>(
    content: impl CoreWidget<Message, Theme, Renderer> + 'a,
    label: &'a str,
    look: &Look,
) -> Element<'a, Message> {
    let bubble = squircle(text(label, type_::XS, look.ink), 0.0, look).padding([4, 8]).paint(Paint {
        fill: Some(look.float),
        material: look.material(),
        ..Paint::NONE
    });
    let l = *look;
    let radius = look.rounded(Rounded::Md);
    tooltip(content, bubble, tooltip::Position::Bottom)
        .gap(6)
        .delay(Duration::from_millis(500))
        .style(move |_| container::Style {
            border: Border { color: l.line_strong, width: 1.0, radius: radius.into() },
            shadow: iced_core::Shadow {
                color: l.shade.scale_alpha(0.45),
                offset: Vector::new(0.0, 6.0),
                blur_radius: 16.0,
            },
            ..container::Style::default()
        })
        .boxed()
}

/// `<Panel>`: a floating layer with a squircle, lit edge and deep shadow.
pub fn panel<'a, Message: 'a>(
    content: impl CoreWidget<Message, Theme, Renderer> + 'a,
    radius: f32,
    look: &Look,
) -> super::squircle::Squircle<'a, Message> {
    squircle(content, radius, look).fill(look.float).material(look.material()).edge().lift()
}

/// `<Field>`: a label over a control, with an optional hint under it.
pub fn field<'a, Message: 'a>(
    label: &'a str,
    control: impl CoreWidget<Message, Theme, Renderer> + 'a,
    hint: Option<&'a str>,
    look: &Look,
) -> Element<'a, Message> {
    let mut field = column![
        container(text(label, type_::S13, look.ink_2)).padding(Padding { bottom: 6.0, ..Padding::ZERO }),
        control
    ];
    if let Some(hint) = hint {
        field = field.push(
            container(text(hint, type_::XS, look.ink_3).line_height(iced_core::text::LineHeight::Relative(1.625)))
                .padding(Padding { top: 6.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    field.width(Length::Fill).boxed()
}

/// `<Input>`: a text field in a raised squircle.
pub fn input<'a, Message: Clone + 'a>(
    id: &'static str,
    value: &'a str,
    placeholder: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    look: &Look,
) -> Input<'a, Message> {
    Input { id, value, placeholder, on_input: Box::new(on_input), on_submit: None, secure: false, look: *look }
}

pub struct Input<'a, Message> {
    id: &'static str,
    value: &'a str,
    placeholder: &'a str,
    on_input: Box<dyn Fn(String) -> Message + 'a>,
    on_submit: Option<Message>,
    secure: bool,
    look: Look,
}

impl<Message> Input<'_, Message> {
    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn on_submit(mut self, message: Message) -> Self {
        self.on_submit = Some(message);
        self
    }
}

impl<'a, Message: Clone + 'a> Input<'a, Message> {
    pub fn boxed(self) -> Element<'a, Message> {
        let i = self;
        let look = i.look;
        let mut field = text_input(i.placeholder, i.value)
            .id(i.id)
            .secure(i.secure)
            .on_input(i.on_input)
            .size(type_::SM.px)
            .line_height(iced_core::text::LineHeight::Absolute(20.0.into()))
            .padding([8, 12])
            .style(move |_: &Theme, status| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border {
                    color: if matches!(status, text_input::Status::Focused { .. }) {
                        look.focus
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 2.0,
                    radius: look.radius(9.0).into(),
                },
                placeholder: look.ink_3,
                value: look.ink,
                selection: look.selection,
            });
        if let Some(message) = i.on_submit {
            field = field.on_submit(message);
        }
        squircle(container(field).center_y(36), 9.0, &look).fill(look.raised).edge().width(Length::Fill).boxed()
    }
}

/// `<Toggle>`: a switch.
pub fn toggle<'a, Message: Clone + 'a>(
    on: bool,
    message: impl Fn(bool) -> Message,
    look: &Look,
) -> Element<'a, Message> {
    let l = *look;
    let knob = container(space().width(16).height(16)).style(move |_| container::Style {
        background: Some((if on { l.on_accent } else { l.ink_2 }).into()),
        border: Border::default().rounded(8),
        ..container::Style::default()
    });
    let track = row![space().width(if on { 16 } else { 2 }), knob].padding(Padding { top: 2.0, ..Padding::ZERO });
    pressable(container(track).width(34).height(20), 10.0, look, move |status| Appearance {
        paint: Paint::fill(fade(if on { l.accent } else { l.press }, status == Status::Disabled)),
        ink: l.ink,
    })
    .round()
    .on_press(message(!on))
    .boxed()
}

/// A checkbox.
pub fn checkbox<'a, Message: Clone + 'a>(on: bool, message: Option<Message>, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let check: Element<'a, Message> = if on {
        icon(Icon::Check, 12.0).stroke(3.0).color(l.on_accent).boxed()
    } else {
        space().width(12).height(12).boxed()
    };
    pressable(container(check).center(16), 5.0, look, move |status| {
        let fill = match (on, status) {
            (true, Status::Hovered | Status::Pressed) => l.accent_hover,
            (true, _) => l.accent,
            (false, Status::Hovered | Status::Pressed) => l.float,
            (false, _) => l.press,
        };
        Appearance { paint: Paint::fill(fade(fill, status == Status::Disabled)), ink: l.on_accent }
    })
    .round()
    .on_press_maybe(message)
    .boxed()
}

/// `<MenuItem>` and the account menu's `MenuRow`: a row in a menu.
pub fn menu_item<'a, Message: Clone + 'a>(
    glyph: Option<Icon>,
    label: impl CoreWidget<Message, Theme, Renderer> + 'a,
    hint: Option<&'a str>,
    active: bool,
    message: Option<Message>,
    look: &Look,
) -> Element<'a, Message> {
    let l = *look;
    let mut content = Row::new().spacing(10).align_y(Alignment::Center);
    if let Some(glyph) = glyph {
        content = content.push(icon(glyph, 14.0).boxed());
    }
    content = content.push(container(label).width(Length::Fill).boxed());
    if let Some(hint) = hint {
        content = content.push(text(hint, type_::XS2, l.ink_3).boxed());
    }
    pressable(content, 8.0, look, move |status| {
        let hovered = matches!(status, Status::Hovered | Status::Pressed);
        Appearance {
            paint: Paint { fill: hovered.then_some(l.hover), ..Paint::NONE },
            ink: if hovered || active { l.ink } else { l.ink_2 },
        }
    })
    .width(Length::Fill)
    .padding([6, 10])
    .on_press_maybe(message)
    .boxed()
}

/// `<Chip>`: a filter or sort to pick, in a row of them.
pub fn chip<'a, Message: Clone + 'a>(
    label: &'a str,
    active: bool,
    message: Message,
    look: &Look,
) -> Element<'a, Message> {
    let l = *look;
    pressable(container(inherit(label, type_::S13)).center_y(30), 9.0, look, move |status| {
        let hovered = matches!(status, Status::Hovered | Status::Pressed);
        Appearance {
            paint: Paint { fill: if active { Some(l.press) } else { hovered.then_some(l.hover) }, ..Paint::NONE },
            ink: if active || hovered { l.ink } else { l.ink_2 },
        }
    })
    .padding([0, 12])
    .on_press(message)
    .boxed()
}

/// `<Segmented>`: mutually exclusive choices, the chosen one lifted on a thumb that slides over.
pub fn segmented<'a, Message: Clone + 'a, T: Copy + PartialEq + 'a>(
    value: T,
    options: &[(T, &'a str)],
    message: impl Fn(T) -> Message,
    look: &Look,
) -> Element<'a, Message> {
    let l = *look;
    let choices = options.iter().map(|&(option, label)| {
        let on = option == value;
        let content = container(inherit(label, type_::S13)).center_y(32).padding([0, 12]);
        pressable(content, 9.0, &l, move |status| Appearance {
            paint: Paint::NONE,
            ink: if on || matches!(status, Status::Hovered | Status::Pressed) { l.ink } else { l.ink_2 },
        })
        .on_press(message(option))
        .boxed()
    });
    let on = options.iter().position(|&(option, _)| option == value);
    let row = Row::with_children(choices);
    squircle(Segments { row, on, look: l }, 11.0, look).fill(look.raised).edge().padding(2).boxed()
}

/// The choices of a `Segmented`, with the thumb under the chosen one. The thumb's leading edge
/// springs toward the new choice and the trailing edge catches up, as the web's and the mobile
/// app's do.
struct Segments<'a, Message> {
    row: Row<Element<'a, Message>>,
    on: Option<usize>,
    look: Look,
}

#[derive(Default)]
struct Slide {
    /// The choice the thumb is at, or is leaving.
    from: Option<usize>,
    /// The choice it's going to, which it's at once the slide is over.
    to: Option<usize>,
    start: Option<Instant>,
    now: Option<Instant>,
}

/// A slide's length: the trailing edge starts 50ms late and takes 420ms.
const SLIDE: Duration = Duration::from_millis(470);

impl<Message> Segments<'_, Message> {
    /// The thumb's bounds now: between the two choices, each edge eased on its own curve.
    fn thumb(&self, slide: &Slide, row: Layout, choices: &[Tree]) -> Option<Rectangle> {
        let choice = |i: Option<usize>| row.iter(choices).nth(i?).map(|(layout, _)| layout.bounds());
        let (from, to) = (choice(slide.from)?, choice(slide.to)?);
        let (leading, trailing) = match (slide.start, slide.now) {
            (Some(start), Some(now)) => (
                super::motion::progress(start, Duration::from_millis(340), now),
                super::motion::progress(start + Duration::from_millis(50), Duration::from_millis(420), now),
            ),
            _ => (0.0, 0.0),
        };
        // Going right, the right edge leads and the left catches up; going left, the reverse.
        let (left, left_t, right, right_t) = if slide.to > slide.from {
            (super::motion::CATCH_UP, trailing, super::motion::SPRING, leading)
        } else {
            (super::motion::SPRING, leading, super::motion::CATCH_UP, trailing)
        };
        let track = row.bounds();
        let x = super::motion::lerp(from.x, to.x, left.at(left_t)).max(track.x);
        let far =
            super::motion::lerp(from.x + from.width, to.x + to.width, right.at(right_t)).min(track.x + track.width);
        Some(Rectangle { x, width: (far - x).max(0.0), ..from })
    }
}

impl<Message> iced_core::widget::Meta for Segments<'_, Message> {}

impl<Message> CoreWidget<Message, Theme, Renderer> for Segments<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Slide>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Slide::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.row));
        let slide = tree.state.downcast_mut::<Slide>();
        if slide.to != self.on {
            // The first choice is where the thumb starts; a change slides from the last one, or
            // jumps when motion is reduced.
            slide.from = if self.look.reduce_motion { self.on } else { slide.to.or(self.on) };
            slide.to = self.on;
            slide.start = None;
        }
    }

    fn size(&self) -> Size<Length> {
        self.row.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.row.layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (row, row_tree) = layout.iter_mut(&mut tree.children).next().expect("a row");
        self.row.operate(row_tree, row, viewport, renderer, operation);
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
        let (row, row_tree) = layout.iter_mut(&mut tree.children).next().expect("a row");
        self.row.update(row_tree, event, row, cursor, renderer, shell, viewport);
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let slide = tree.state.downcast_mut::<Slide>();
            if slide.from != slide.to {
                let start = *slide.start.get_or_insert(*now);
                slide.now = Some(*now);
                if now.saturating_duration_since(start) < SLIDE {
                    shell.request_redraw();
                } else {
                    slide.from = slide.to;
                    slide.start = None;
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
        use iced_core::Renderer as _;
        let (row, row_tree) = layout.iter(&tree.children).next().expect("a row");
        if let Some(thumb) = self.thumb(tree.state.downcast_ref::<Slide>(), row, &row_tree.children) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: thumb,
                    border: Border::default().rounded(self.look.radius(9.0)),
                    shadow: self.look.chip_shadow(),
                    ..renderer::Quad::default()
                },
                self.look.float,
            );
        }
        self.row.draw(row_tree, renderer, theme, style, row, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (row, row_tree) = layout.iter(&tree.children).next().expect("a row");
        self.row.mouse_interaction(row_tree, row, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<iced_core::overlay::Element<'b, Message, Theme, Renderer>> {
        let (row, row_tree) = layout.iter_mut(&mut tree.children).next().expect("a row");
        self.row.overlay(row_tree, row, renderer, viewport, translation, window)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Quiet,
    Ok,
    Danger,
    Live,
    Warn,
    Strong,
}

/// `<Badge>`: a small label.
pub fn badge<'a, Message: 'a>(label: impl Into<String>, tone: Tone, look: &Look) -> Element<'a, Message> {
    let (fill, ink) = match tone {
        Tone::Quiet => (look.panel, look.ink_2),
        Tone::Ok => (look.ok.scale_alpha(0.12), look.ok),
        Tone::Danger => (look.danger.scale_alpha(0.12), look.danger),
        Tone::Live => (look.info_deep.scale_alpha(0.12), look.info),
        Tone::Warn => (look.warn_deep.scale_alpha(0.12), look.warn),
        Tone::Strong => (look.ink, look.canvas),
    };
    let radius = look.rounded(Rounded::Md);
    container(text(label.into(), type_::XS2, ink).font(MEDIUM))
        .center_y(20)
        .padding([0, 6])
        .style(move |_| container::Style {
            background: Some(fill.into()),
            border: Border::default().rounded(radius),
            ..container::Style::default()
        })
        .boxed()
}

/// `<Progress>`: a thin bar, sliding back and forth while there's no number yet.
pub fn progress<'a, Message: 'a>(value: Option<f32>, tone: Color, look: &Look) -> Element<'a, Message> {
    Bar { value, tone, track: look.press, still: look.reduce_motion }.boxed()
}

struct Bar {
    value: Option<f32>,
    tone: Color,
    track: Color,
    still: bool,
}

#[derive(Default)]
struct Clock {
    start: Option<Instant>,
    now: Option<Instant>,
}

impl iced_core::widget::Meta for Bar {}

impl<Message> CoreWidget<Message, Theme, Renderer> for Bar {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Clock>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Clock::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(4.0))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, 4.0);
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && self.value.is_none()
            && !self.still
        {
            let clock = tree.state.downcast_mut::<Clock>();
            clock.start.get_or_insert(*now);
            clock.now = Some(*now);
            shell.request_redraw();
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
        let pill = |bounds: Rectangle| renderer::Quad {
            bounds,
            border: Border::default().rounded(2),
            ..renderer::Quad::default()
        };
        renderer.fill_quad(pill(bounds), self.track);
        let bar = match self.value {
            Some(value) => Rectangle { width: bounds.width * value.clamp(0.0, 1.0), ..bounds },
            None => {
                // `slide`: a third of the width, from a third off the left to past the right.
                let clock = tree.state.downcast_ref::<Clock>();
                let t = if self.still {
                    0.5
                } else {
                    match (clock.start, clock.now) {
                        (Some(start), Some(now)) => (now - start).as_secs_f32() % 1.2 / 1.2,
                        _ => 0.0,
                    }
                };
                let x = super::motion::lerp(-0.33, 1.0, super::motion::EASE_IN_OUT.at(t)) * bounds.width;
                let left = (bounds.x + x).max(bounds.x);
                let right = (bounds.x + x + bounds.width / 3.0).min(bounds.x + bounds.width);
                Rectangle { x: left, width: (right - left).max(0.0), ..bounds }
            },
        };
        if bar.width > 0.0 {
            renderer.with_layer(bounds, |renderer| renderer.fill_quad(pill(bar), self.tone));
        }
    }
}

/// `<Spinner>`: a ring with a quarter of it lit, turning.
pub fn spinner<'a, Message: 'a>(size: f32, look: &Look) -> Element<'a, Message> {
    Spinner { size, color: None, still: look.reduce_motion }.boxed()
}

struct Spinner {
    size: f32,
    color: Option<Color>,
    still: bool,
}

#[derive(Default)]
struct Spinning {
    clock: Clock,
    track: Keyed,
}

impl iced_core::widget::Meta for Spinner {}

impl<Message> CoreWidget<Message, Theme, Renderer> for Spinner {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Spinning>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Spinning::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, self.size, self.size);
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && !self.still
        {
            let clock = &mut tree.state.downcast_mut::<Spinning>().clock;
            clock.start.get_or_insert(*now);
            clock.now = Some(*now);
            shell.request_redraw();
        }
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
        let state = tree.state.downcast_ref::<Spinning>();
        let color = self.color.unwrap_or(style.text_color);
        let turn = match (state.clock.start, state.clock.now) {
            (Some(start), Some(now)) => (now - start).as_secs_f32() % 1.0,
            _ => 0.0,
        };
        // In a 24 box: a ring of 9.5 at 15%, and a quarter of it from the top, turning once a
        // second.
        let scale = self.size / 24.0;
        let center = Point::new(12.0 * scale, 12.0 * scale);
        let stroke = |c: Color| Stroke::default().with_width(2.5 * scale).with_color(c).with_line_cap(LineCap::Round);
        let key = [self.size, color.r, color.g, color.b, color.a];
        let ring = state.track.draw(renderer, bounds.size(), &key, |frame| {
            frame.stroke(&Path::circle(center, 9.5 * scale), stroke(color.scale_alpha(0.15)));
        });
        let start = -std::f32::consts::FRAC_PI_2 + turn * std::f32::consts::TAU;
        let mut frame = iced_widget::canvas::Frame::new(renderer, bounds.size());
        frame.stroke(
            &Path::new(|b| {
                b.arc(Arc {
                    center,
                    radius: 9.5 * scale,
                    start_angle: iced_core::Radians(start),
                    end_angle: iced_core::Radians(start + std::f32::consts::FRAC_PI_2),
                })
            }),
            stroke(color),
        );
        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.draw_geometry(ring);
            renderer.draw_geometry(frame.into_geometry());
        });
    }
}

/// A hairline across (`h-px bg-line`).
pub fn divider<'a, Message: 'a>(look: &Look) -> Element<'a, Message> {
    let line = look.line;
    container(space().width(Length::Fill).height(1))
        .style(move |_| container::Style { background: Some(line.into()), ..container::Style::default() })
        .boxed()
}

/// The web's `or` between two ways of doing something.
pub fn or<'a, Message: 'a>(look: &Look) -> Element<'a, Message> {
    row![divider(look), text("or", type_::XS, look.ink_3), divider(look)].spacing(12).align_y(Alignment::Center).boxed()
}

/// A warning or notice, tinted by its tone (the mobile `MismatchBanner`).
pub fn banner<'a, Message: Clone + 'a>(
    glyph: Icon,
    tone: Color,
    body: impl CoreWidget<Message, Theme, Renderer> + 'a,
    dismiss: Option<Message>,
    look: &Look,
) -> Element<'a, Message> {
    let mut content = row![
        container(icon(glyph, 18.0).color(tone)).padding(Padding { top: 1.0, ..Padding::ZERO }),
        container(body).width(Length::Fill)
    ]
    .spacing(12);
    if let Some(dismiss) = dismiss {
        content = content.push(icon_button(Icon::X, "Dismiss", Some(dismiss), look));
    }
    squircle(content.align_y(Alignment::Start), 14.0, look)
        .fill(tone.scale_alpha(0.1))
        .padding(14)
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thumb's left and right edges sliding from one choice to another, `elapsed` ms in.
    /// Three choices, 60, 80 and 50 wide, in a track of 190.
    fn thumb(from: usize, to: usize, elapsed: Option<u64>) -> (f32, f32) {
        let mut choices = [Tree::empty(), Tree::empty(), Tree::empty()];
        for (choice, (x, width)) in choices.iter_mut().zip([(0.0, 60.0), (60.0, 80.0), (140.0, 50.0)]) {
            choice.size = Size::new(width, 32.0);
            choice.translation = Vector::new(x, 0.0);
        }
        let row = Layout::new(Size::new(190.0, 32.0));
        let segments = Segments::<()> { row: Row::new(), on: Some(to), look: Look::default() };
        let start = Instant::now();
        let slide = Slide {
            from: Some(from),
            to: Some(to),
            start: elapsed.map(|_| start),
            now: elapsed.map(|ms| start + Duration::from_millis(ms)),
        };
        let thumb = segments.thumb(&slide, row, &choices).expect("a thumb");
        (thumb.x, thumb.x + thumb.width)
    }

    #[test]
    fn thumb_rests_on_its_choice() {
        assert_eq!(thumb(1, 1, None), (60.0, 140.0));
        assert_eq!(thumb(1, 1, Some(300)), (60.0, 140.0));
    }

    #[test]
    fn thumb_leads_then_catches_up() {
        let (left, right) = thumb(0, 1, Some(170));
        assert!(left < 20.0 && right > 120.0, "{left} {right}");
        assert_eq!(thumb(0, 1, Some(470)), (60.0, 140.0));
    }

    #[test]
    fn thumb_stays_in_its_track() {
        let (left, right) = thumb(1, 0, Some(170));
        assert!(left >= 0.0 && right <= 190.0, "{left} {right}");
    }
}
