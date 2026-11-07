// SPDX-License-Identifier: AGPL-3.0-or-later

//! Defers image requests until their widget enters the scroll viewport.

use iced_core::layout::{self, Layout};
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{Event, Length, Rectangle, Size, Theme, Vector, Widget, mouse, overlay, renderer, window};
use iced_widget::{Element, Renderer, Widget as _};

pub fn visible<'a, Message: Clone + 'a>(
    content: Element<'a, Message>,
    key: String,
    message: Message,
) -> Element<'a, Message> {
    Visible { content, key, message }.boxed()
}
struct Visible<'a, Message> {
    content: Element<'a, Message>,
    key: String,
    message: Message,
}
#[derive(Default)]
struct State {
    key: String,
    sent: bool,
}
impl<Message> iced_core::widget::Meta for Visible<'_, Message> {}
impl<Message: Clone> Widget<Message, Theme, Renderer> for Visible<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.key != self.key {
            state.key = self.key.clone();
            state.sent = false;
        }
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

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("visible content");
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
        if !state.sent
            && matches!(event, Event::Window(window::Event::RedrawRequested(_)))
            && layout.bounds().intersection(viewport).is_some()
        {
            state.sent = true;
            shell.publish(self.message.clone());
        }
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("visible content");
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
        let (child, tree) = layout.iter(&tree.children).next().expect("visible content");
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
        let (child, tree) = layout.iter(&tree.children).next().expect("visible content");
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
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("visible content");
        self.content.overlay(tree, child, renderer, viewport, translation, window)
    }
}

/// Builds a fixed-size row only while it is in or near the scroll viewport.
pub fn deferred<'a, Message: 'a>(height: f32, build: impl Fn() -> Element<'a, Message> + 'a) -> Element<'a, Message> {
    Deferred { height, build: Box::new(build), content: None }.boxed()
}
struct Deferred<'a, Message> {
    height: f32,
    build: Box<dyn Fn() -> Element<'a, Message> + 'a>,
    content: Option<Element<'a, Message>>,
}
#[derive(Default)]
struct DeferredState {
    visible: bool,
}
impl<Message> iced_core::widget::Meta for Deferred<'_, Message> {}
impl<Message> Widget<Message, Theme, Renderer> for Deferred<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DeferredState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DeferredState::default())
    }

    fn diff(&mut self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height))
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, self.height);
        if tree.state.downcast_ref::<DeferredState>().visible {
            self.content = Some((self.build)());
            tree.diff_children(std::slice::from_mut(&mut self.content));
            self.content.layout(&mut tree.children[0], renderer, &layout::Limits::new(Size::ZERO, tree.size));
        } else {
            self.content = None;
            tree.children.clear();
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
        let visible = layout.bounds().intersection(&viewport.expand(self.height)).is_some();
        let state = tree.state.downcast_mut::<DeferredState>();
        if visible != state.visible {
            state.visible = visible;
            shell.invalidate_layout();
            shell.request_redraw();
        }
        if let Some(content) = &mut self.content {
            let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("deferred row");
            content.update(tree, event, child, cursor, renderer, shell, viewport);
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
        if let Some(content) = &self.content {
            let (child, tree) = layout.iter(&tree.children).next().expect("deferred row");
            content.draw(tree, renderer, theme, style, child, cursor, viewport);
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(content) = &mut self.content {
            let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("deferred row");
            content.operate(tree, child, viewport, renderer, operation);
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
        self.content.as_ref().map_or(mouse::Interaction::None, |content| {
            let (child, tree) = layout.iter(&tree.children).next().expect("deferred row");
            content.mouse_interaction(tree, child, cursor, viewport, renderer)
        })
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
        if let Some(content) = &mut self.content {
            let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("deferred row");
            content.overlay(tree, child, renderer, viewport, translation, window)
        } else {
            Vec::new()
        }
    }
}
