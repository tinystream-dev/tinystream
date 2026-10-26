// SPDX-License-Identifier: AGPL-3.0-or-later

//! Anything that can be pressed: a squircle that changes with the pointer, takes keyboard focus
//! (Tab, then Enter or Space) and shows the web's focus ring while focused.

use iced_core::keyboard::{self, key};
use iced_core::layout::{self, Layout};
use iced_core::widget::operation::Focusable;
use iced_core::widget::{self, Operation, Tree, tree};
use iced_core::{
    Color, Event, Length, Padding, Rectangle, Renderer as _, Size, Theme, Vector, Widget, mouse, overlay, renderer,
    touch, window,
};
use iced_widget::{Element, Renderer};

use super::squircle::{Outline, Paint, Painted, draw_over, draw_under};
use super::theme::Look;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Hovered,
    Pressed,
    Disabled,
}

/// How a pressable looks in some status: its squircle, and the colour of what's in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    pub paint: Paint,
    pub ink: Color,
}

pub struct Pressable<'a, Message> {
    content: Element<'a, Message>,
    on_press: Option<Message>,
    width: Length,
    height: Length,
    padding: Padding,
    radius: f32,
    /// Whether the radius is Layered's, scaled for the other styles, or exact (`rounded-full`).
    scaled: bool,
    smoothing: Option<f32>,
    look: Look,
    style: Box<dyn Fn(Status) -> Appearance + 'a>,
    id: Option<widget::Id>,
    status: Option<Status>,
}

pub fn pressable<'a, Message>(
    content: impl Widget<Message, Theme, Renderer> + 'a,
    radius: f32,
    look: &Look,
    style: impl Fn(Status) -> Appearance + 'a,
) -> Pressable<'a, Message> {
    Pressable {
        content: Widget::_boxed(content),
        on_press: None,
        width: Length::Shrink,
        height: Length::Shrink,
        padding: Padding::ZERO,
        radius,
        scaled: true,
        smoothing: None,
        look: *look,
        style: Box::new(style),
        id: None,
        status: None,
    }
}

impl<'a, Message> Pressable<'a, Message> {
    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn on_press_maybe(mut self, message: Option<Message>) -> Self {
        self.on_press = message;
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

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Plain round corners of exactly `radius`, whatever the style.
    pub fn round(mut self) -> Self {
        self.scaled = false;
        self.smoothing = Some(0.0);
        self
    }

    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }
}

#[derive(Default)]
struct State {
    pressed: bool,
    focused: bool,
    painted: Painted,
}

impl Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
    }

    fn unfocus(&mut self) {
        self.focused = false;
    }
}

impl<Message> iced_core::widget::Meta for Pressable<'_, Message> {}

impl<'a, Message: Clone + 'a> Widget<Message, Theme, Renderer> for Pressable<'a, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
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

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if self.on_press.is_some() {
            operation.focusable(self.id.as_ref(), layout.bounds(), tree.state.downcast_mut::<State>());
        }
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
        {
            let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
            self.content.update(tree, event, layout, cursor, renderer, shell, viewport);
        }
        if shell.is_event_captured() {
            return;
        }

        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                // Clicking anywhere moves focus away from the keyboard's choice, as on the web.
                state.focused = false;
                if self.on_press.is_some() && cursor.is_over(bounds) {
                    state.pressed = true;
                    shell.capture_event();
                    shell.request_redraw();
                }
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. }) => {
                if state.pressed {
                    state.pressed = false;
                    shell.request_redraw();
                    if let Some(message) = &self.on_press
                        && cursor.is_over(bounds)
                    {
                        shell.publish(message.clone());
                        shell.capture_event();
                    }
                }
            },
            Event::Touch(touch::Event::FingerLost { .. }) => state.pressed = false,
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focused && matches!(key, keyboard::Key::Named(key::Named::Enter | key::Named::Space)) =>
            {
                if let Some(message) = &self.on_press {
                    shell.publish(message.clone());
                    shell.capture_event();
                }
            },
            _ => {},
        }

        let status = if self.on_press.is_none() {
            Status::Disabled
        } else if cursor.is_over(bounds) {
            if state.pressed { Status::Pressed } else { Status::Hovered }
        } else {
            Status::Active
        };
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            self.status = Some(status);
        } else if self.status.is_some_and(|current| current != status) {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<State>();
        let status = self.status.unwrap_or(if self.on_press.is_none() { Status::Disabled } else { Status::Active });
        let mut appearance = (self.style)(status);
        if state.focused && self.on_press.is_some() {
            appearance.paint.ring = Some(self.look.focus);
        }
        let radius = if self.scaled { self.look.radius(self.radius) } else { self.radius };
        let smoothing = self.smoothing.unwrap_or(self.look.corners().1);
        draw_under(
            renderer,
            &state.painted,
            Outline { bounds, radius, smoothing },
            &appearance.paint,
            &self.look,
            viewport,
        );
        let (layout, tree) = layout.iter(&tree.children).next().expect("content");
        renderer.with_layer(*viewport, |renderer| {
            self.content.draw(
                tree,
                renderer,
                theme,
                &renderer::Style { text_color: appearance.ink },
                layout,
                cursor,
                viewport,
            );
        });
        draw_over(
            renderer,
            &state.painted,
            Outline { bounds, radius, smoothing },
            &appearance.paint,
            &self.look,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (child, tree) = layout.iter(&tree.children).next().expect("content");
        let inner = self.content.mouse_interaction(tree, child, cursor, viewport, renderer);
        // The web leaves buttons with the arrow (`cursor: default`).
        if inner == mouse::Interaction::None && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            inner
        }
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
