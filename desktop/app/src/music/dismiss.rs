// SPDX-License-Identifier: AGPL-3.0-or-later
use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree};
use iced_core::{Event, Length, Rectangle, Size, Theme, Vector, Widget, mouse, overlay, renderer};
use iced_widget::{Element, Renderer, Widget as _};

use super::Message;

pub fn dismiss_on_outside_press(content: Element<'_, Message>) -> Element<'_, Message> {
    Outside { content }.boxed()
}
struct Outside<'a> {
    content: Element<'a, Message>,
}
impl iced_core::widget::Meta for Outside<'_> {}
impl Widget<Message, Theme, Renderer> for Outside<'_> {
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
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)))
            && !cursor.is_over(layout.bounds())
        {
            shell.publish(Message::Collapse);
        }
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("music card");
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
        let (layout, tree) = layout.iter(&tree.children).next().expect("music card");
        self.content.draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("music card");
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
        let (layout, tree) = layout.iter(&tree.children).next().expect("music card");
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
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("music card");
        self.content.overlay(tree, layout, renderer, viewport, translation, window)
    }
}
