// SPDX-License-Identifier: AGPL-3.0-or-later

//! Something drawn another way while the pointer's over it (`group-hover:`), like a list row whose
//! controls only show under the pointer. Unlike iced's `hover`, the two aren't drawn over each
//! other, so the hovered one can move or hide what the resting one shows.

use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{Event, Length, Rectangle, Size, Theme, Vector, Widget, mouse, overlay, renderer};
use iced_widget::{Element, Renderer, Widget as _};

/// `rest`, or `hovered` while the pointer's over it or a menu it opened is open. `hovered` is laid
/// out at the size `rest` takes.
pub fn reveal<'a, Message: 'a>(
    rest: impl Widget<Message, Theme, Renderer> + 'a,
    hovered: impl Widget<Message, Theme, Renderer> + 'a,
) -> Element<'a, Message> {
    Reveal { children: [Widget::_boxed(rest), Widget::_boxed(hovered)] }.boxed()
}

struct Reveal<'a, Message> {
    children: [Element<'a, Message>; 2],
}

#[derive(Default)]
struct State {
    over: bool,
    /// Whether `hovered` has a menu open, which keeps it shown when the pointer goes to the menu.
    open: bool,
}

impl<Message> iced_core::widget::Meta for Reveal<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Reveal<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.children);
    }

    fn size(&self) -> Size<Length> {
        self.children[0].size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.children[0].layout(&mut tree.children[0], renderer, limits);
        let size = tree.children[0].size;
        self.children[1].layout(&mut tree.children[1], renderer, &layout::Limits::new(size, size));
        tree.size = size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        for (child, (layout, tree)) in self.children.iter_mut().zip(layout.iter_mut(&mut tree.children)) {
            child.operate(tree, layout, viewport, renderer, operation);
        }
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
        let over = cursor.is_over(layout.bounds());
        let state = tree.state.downcast_mut::<State>();
        if over != state.over {
            state.over = over;
            shell.request_redraw();
        }
        let shown = over || state.open;
        let moved = matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft));
        let mut children = self.children.iter_mut().zip(layout.iter_mut(&mut tree.children));
        let (rest, (rest_layout, rest_tree)) = children.next().expect("resting content");
        let (hovered, (hovered_layout, hovered_tree)) = children.next().expect("hovered content");
        if shown || moved {
            hovered.update(hovered_tree, event, hovered_layout, cursor, renderer, shell, viewport);
        }
        if (!shown || moved) && !shell.is_event_captured() {
            rest.update(rest_tree, event, rest_layout, cursor, renderer, shell, viewport);
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
        let shown = cursor.is_over(layout.bounds()) || tree.state.downcast_ref::<State>().open;
        let (child, tree) = layout.iter(&tree.children).nth(usize::from(shown)).expect("content");
        self.children[usize::from(shown)].draw(tree, renderer, theme, style, child, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let shown = cursor.is_over(layout.bounds()) || tree.state.downcast_ref::<State>().open;
        let (child, tree) = layout.iter(&tree.children).nth(usize::from(shown)).expect("content");
        self.children[usize::from(shown)].mouse_interaction(tree, child, cursor, viewport, renderer)
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
        let state = tree.state.downcast_mut::<State>();
        let (child, tree) = layout.iter_mut(&mut tree.children).nth(1).expect("hovered content");
        let overlays = self.children[1].overlay(tree, child, renderer, viewport, translation, window);
        state.open = !overlays.is_empty();
        overlays
    }
}
