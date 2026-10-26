// SPDX-License-Identifier: AGPL-3.0-or-later

//! A modal layer owns pointer input and keyboard focus until it closes.

use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree, operation, tree};
use iced_core::{Event, Length, Rectangle, Size, Theme, Vector, Widget, keyboard, mouse, overlay, renderer, window};
use iced_widget::{Element, Renderer, Widget as _};

pub fn modal<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    layer: Element<'a, Message>,
    close: Message,
) -> Element<'a, Message> {
    Modal { children: [base, layer], close }.boxed()
}

struct Modal<'a, Message> {
    children: [Element<'a, Message>; 2],
    close: Message,
}

impl<Message> iced_core::widget::Meta for Modal<'_, Message> {}

impl<Message: Clone> Widget<Message, Theme, Renderer> for Modal<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<bool>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(false)
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.children);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        for (child, tree) in self.children.iter_mut().zip(&mut tree.children) {
            child.layout(tree, renderer, limits);
        }
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (layout, tree) = layout.iter_mut(&mut tree.children).nth(1).expect("modal");
        self.children[1].operate(tree, layout, viewport, renderer, operation);
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
        if shell.is_event_captured() {
            return;
        }
        if !*tree.state.downcast_ref::<bool>() && matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            *tree.state.downcast_mut::<bool>() = true;
            self.operate(tree, layout, viewport, renderer, &mut operation::focusable::unfocus::<()>());
            let mut operation: Box<dyn Operation> = Box::new(operation::focusable::focus_next());
            loop {
                self.operate(tree, layout, viewport, renderer, operation.as_mut());
                match operation.finish() {
                    operation::Outcome::Chain(next) => operation = next,
                    _ => break,
                }
            }
        }
        if matches!(
            event,
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            })
        ) {
            shell.publish(self.close.clone());
            shell.capture_event();
            return;
        }
        let (layout, tree) = layout.iter_mut(&mut tree.children).nth(1).expect("modal");
        self.children[1].update(tree, event, layout, cursor, renderer, shell, viewport);
        if matches!(event, Event::Keyboard(_) | Event::Mouse(_) | Event::Touch(_)) {
            shell.capture_event();
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
        for (child, (layout, tree)) in self.children.iter().zip(layout.iter(&tree.children)) {
            child.draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (layout, tree) = layout.iter(&tree.children).nth(1).expect("modal");
        self.children[1].mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        let (layout, tree) = layout.iter_mut(&mut tree.children).nth(1).expect("modal");
        self.children[1].overlay(tree, layout, renderer, viewport, translation, window)
    }
}
