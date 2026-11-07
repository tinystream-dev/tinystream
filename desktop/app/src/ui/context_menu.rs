// SPDX-License-Identifier: AGPL-3.0-or-later

//! A right-click menu anchored at the pointer, with the same material and key navigation as Select.

use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{Event, Length, Point, Rectangle, Size, Theme, Vector, Widget, keyboard, mouse, overlay, renderer};
use iced_widget::{Column, Element, Renderer, Widget as _};

use super::Look;
use super::controls::{menu_item, panel};
use super::text::{self as ty, text};

pub fn context_menu<'a, Message: Clone + 'a>(
    content: Element<'a, Message>,
    items: Vec<(String, Message)>,
    look: &Look,
) -> Element<'a, Message> {
    Context { content, items, look: *look }.boxed()
}
struct Context<'a, Message> {
    content: Element<'a, Message>,
    items: Vec<(String, Message)>,
    look: Look,
}
#[derive(Default)]
struct State {
    at: Option<Point>,
    active: usize,
}
impl<Message> iced_core::widget::Meta for Context<'_, Message> {}
impl<Message: Clone> Widget<Message, Theme, Renderer> for Context<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != 2 {
            tree.children = vec![Tree::new(&self.content), Tree::empty()];
        }
        tree.children[0].diff::<Message, Theme, Renderer>(&mut self.content);
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

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("menu target");
        self.content.operate(tree, child, viewport, renderer, operation);
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
        let state = tree.state.downcast_mut::<State>();
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)))
            && cursor.is_over(layout.bounds())
            && !self.items.is_empty()
        {
            state.at = cursor.position();
            state.active = 0;
            shell.capture_event();
            shell.request_redraw();
            return;
        }
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("menu target");
        self.content.update(tree, event, child, cursor, renderer, shell, viewport);
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
        let (child, tree) = layout.iter(&tree.children).next().expect("menu target");
        self.content.draw(tree, renderer, theme, style, child, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (child, tree) = layout.iter(&tree.children).next().expect("menu target");
        self.content.mouse_interaction(tree, child, cursor, viewport, renderer)
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
        if tree.state.downcast_ref::<State>().at.is_none() {
            let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("menu target");
            return self.content.overlay(tree, child, renderer, viewport, translation, window);
        }
        let state = tree.state.downcast_mut::<State>();
        let at = state.at.expect("open menu") + translation;
        let choices = self.items.iter().enumerate().map(|(i, (label, message))| {
            menu_item(
                None,
                text(label.clone(), ty::S13, self.look.ink),
                None,
                i == state.active,
                Some(message.clone()),
                &self.look,
            )
        });
        let mut content = panel(Column::with_children(choices), 12.0, &self.look).padding(6).width(248).boxed();
        let tree = &mut tree.children[1];
        tree.diff::<Message, Theme, Renderer>(&mut content);
        content.layout(tree, renderer, &layout::Limits::new(Size::ZERO, window));
        let layout = Layout::new(tree.size).move_to(Point::new(
            at.x.min(window.width - tree.size.width).max(0.0),
            at.y.min(window.height - tree.size.height).max(0.0),
        ));
        vec![overlay::Element::new(Box::new(Menu { content, tree, state, layout, window, items: &self.items }))]
    }
}
struct Menu<'a, Message> {
    content: Element<'a, Message>,
    tree: &'a mut Tree,
    state: &'a mut State,
    layout: Layout,
    window: Size,
    items: &'a [(String, Message)],
}
impl<Message: Clone> iced_core::Overlay<Message, Theme, Renderer> for Menu<'_, Message> {
    fn draw(&self, renderer: &mut Renderer, theme: &Theme, style: &renderer::Style, cursor: mouse::Cursor) {
        self.content.draw(self.tree, renderer, theme, style, self.layout, cursor, &Rectangle::with_size(self.window));
    }

    fn update(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
    ) {
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
            use keyboard::Key;
            use keyboard::key::Named;
            match key {
                Key::Named(Named::ArrowDown) => self.state.active = (self.state.active + 1) % self.items.len(),
                Key::Named(Named::ArrowUp) => {
                    self.state.active = (self.state.active + self.items.len() - 1) % self.items.len()
                },
                Key::Named(Named::Home) => self.state.active = 0,
                Key::Named(Named::End) => self.state.active = self.items.len() - 1,
                Key::Named(Named::Enter | Named::Space) => {
                    shell.publish(self.items[self.state.active].1.clone());
                    self.state.at = None;
                },
                Key::Named(Named::Escape | Named::Tab) => self.state.at = None,
                _ => return,
            }
            shell.capture_event();
            shell.request_redraw();
            return;
        }
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_))) && !cursor.is_over(self.layout.bounds()) {
            self.state.at = None;
            shell.capture_event();
            shell.request_redraw();
            return;
        }
        self.content.update(self.tree, event, self.layout, cursor, renderer, shell, &self.layout.bounds());
        if shell.is_event_captured() && matches!(event, Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)))
        {
            self.state.at = None;
            shell.request_redraw();
        }
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        self.content.mouse_interaction(self.tree, self.layout, cursor, &self.layout.bounds(), renderer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_the_targets_entire_widget_tree_before_its_first_layout() {
        let mut widget = context_menu::<()>(
            iced_widget::container(iced_widget::space()).boxed(),
            vec![("Open".into(), ())],
            &Look::default(),
        );
        let mut tree = Tree::new(&widget);
        widget.diff(&mut tree);
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.children[0].children.len(), 1);
    }
}
