// SPDX-License-Identifier: AGPL-3.0-or-later

//! The web's select: a focusable field and a material menu, with native-style keyboard navigation.

use std::time::{Duration, Instant};

use iced_core::layout::{self, Layout};
use iced_core::widget::operation::Focusable;
use iced_core::widget::{Operation, Tree, tree};
use iced_core::{
    Alignment, Event, Length, Rectangle, Renderer as _, Size, Theme, Vector, Widget, keyboard, mouse, overlay, renderer,
};
use iced_widget::{Column, Element, Renderer, Widget as _, container, row, scrollable, space};

use super::controls::panel;
use super::icon::{Icon, icon};
use super::pressable::{Appearance, Status, pressable};
use super::squircle::{Outline, Paint, Painted, draw_over, draw_under};
use super::text::{self as type_, inherit};
use super::theme::Look;

pub fn select<'a, Message: Clone + 'a, T: Clone + PartialEq + 'a>(
    value: Option<T>,
    options: &'a [T],
    label: impl Fn(&T) -> String,
    on_change: impl Fn(T) -> Message + 'a,
    look: &Look,
) -> Element<'a, Message> {
    let labels: Vec<_> = options.iter().map(label).collect();
    let chosen = value.as_ref().and_then(|v| options.iter().position(|o| o == v));
    let content = container(
        row![
            inherit(chosen.map_or("", |i| labels[i].as_str()).to_owned(), type_::SM).width(Length::Fill),
            icon(Icon::ChevronDown, 16.0).color(look.ink_3),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .center_y(36)
    .padding([0, 12])
    .boxed();
    Select { options, labels, chosen, on_change: Box::new(on_change), content, look: *look }.boxed()
}

struct Select<'a, T, Message> {
    options: &'a [T],
    labels: Vec<String>,
    chosen: Option<usize>,
    on_change: Box<dyn Fn(T) -> Message + 'a>,
    content: Element<'a, Message>,
    look: Look,
}

#[derive(Default)]
struct State {
    focused: bool,
    open: bool,
    active: usize,
    painted: Painted,
    typed: String,
    typed_at: Option<Instant>,
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
        self.open = false;
    }
}

impl State {
    fn key(&mut self, key: &keyboard::Key, labels: &[String]) -> (bool, Option<usize>) {
        use keyboard::key::Named;
        if labels.is_empty() {
            return (false, None);
        }
        self.active = self.active.min(labels.len() - 1);
        match key.as_ref() {
            keyboard::Key::Named(Named::Tab) => {
                self.open = false;
                return (false, None);
            },
            keyboard::Key::Named(Named::Escape) if self.open => {
                self.open = false;
                return (true, None);
            },
            keyboard::Key::Named(Named::Enter | Named::Space) => {
                self.open = !self.open;
                return (true, (!self.open).then_some(self.active));
            },
            keyboard::Key::Named(Named::ArrowDown) => self.active = (self.active + 1).min(labels.len() - 1),
            keyboard::Key::Named(Named::ArrowUp) => self.active = self.active.saturating_sub(1),
            keyboard::Key::Named(Named::Home) => self.active = 0,
            keyboard::Key::Named(Named::End) => self.active = labels.len() - 1,
            keyboard::Key::Character(character) => {
                let now = Instant::now();
                if self.typed_at.is_none_or(|last| now.duration_since(last) > Duration::from_millis(700)) {
                    self.typed.clear();
                }
                self.typed_at = Some(now);
                self.typed.push_str(&character.to_lowercase());
                if let Some(i) = labels.iter().position(|l| l.to_lowercase().starts_with(&self.typed)) {
                    self.active = i;
                }
            },
            _ => return (false, None),
        }
        (true, (!self.open).then_some(self.active))
    }
}

impl<T, Message> iced_core::widget::Meta for Select<'_, T, Message> {}

impl<T: Clone, Message: Clone> Widget<Message, Theme, Renderer> for Select<'_, T, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State { active: self.chosen.unwrap_or(0), ..Default::default() })
    }

    fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != 2 {
            tree.children = vec![Tree::new(&self.content), Tree::empty()];
        }
        tree.children[0].diff::<Message, Theme, Renderer>(&mut self.content);
        let state = tree.state.downcast_mut::<State>();
        if !state.open {
            state.active = self.chosen.unwrap_or(0);
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(36.0))
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        layout::padded(tree, limits, Length::Fill, Length::Fixed(36.0), 0, |tree, limits| {
            self.content.layout(tree, renderer, limits);
            tree.size
        });
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout, _: &Rectangle, _: &Renderer, operation: &mut dyn Operation) {
        if !self.options.is_empty() {
            operation.focusable(None, layout.bounds(), tree.state.downcast_mut::<State>());
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        _: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if !self.options.is_empty() && !shell.is_event_captured() =>
            {
                if cursor.is_over(layout.bounds()) {
                    state.focused = true;
                    state.open = !state.open;
                    shell.capture_event();
                } else {
                    state.focused = false;
                    state.open = false;
                }
                shell.request_redraw();
            },
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if state.focused && !shell.is_event_captured() && !modifiers.command() && !modifiers.alt() =>
            {
                let (handled, pick) = state.key(key, &self.labels);
                if let Some(i) = pick {
                    shell.publish((self.on_change)(self.options[i].clone()));
                }
                if handled {
                    shell.capture_event();
                    shell.request_redraw();
                }
            },
            _ => {},
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let shape =
            Outline { bounds: layout.bounds(), radius: self.look.radius(9.0), smoothing: self.look.corners().1 };
        let paint = Paint {
            fill: Some(self.look.raised),
            edge: true,
            ring: state.focused.then_some(self.look.focus),
            ..Paint::NONE
        };
        draw_under(renderer, &state.painted, shape, &paint, &self.look, viewport);
        let (child, tree) = layout.iter(&tree.children).next().expect("field");
        renderer.with_layer(*viewport, |renderer| {
            self.content.draw(
                tree,
                renderer,
                theme,
                &renderer::Style { text_color: self.look.ink },
                child,
                cursor,
                viewport,
            );
        });
        draw_over(renderer, &state.painted, shape, &paint, &self.look, viewport);
    }

    fn mouse_interaction(
        &self,
        _: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _: &Rectangle,
        _: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) { mouse::Interaction::Idle } else { mouse::Interaction::None }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        _: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        if !state.open {
            return vec![];
        }
        let l = self.look;
        let options = self.options;
        let change = &self.on_change;
        let choices = self.labels.iter().enumerate().map(|(i, label)| {
            let active = i == state.active;
            let chosen = self.chosen == Some(i);
            pressable(
                row![
                    inherit(label.clone(), type_::SM).width(Length::Fill),
                    if chosen { icon(Icon::Check, 14.0).boxed() } else { space().width(14).boxed() }
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                8.0,
                &l,
                move |status| Appearance {
                    paint: Paint {
                        fill: (active || matches!(status, Status::Hovered | Status::Pressed)).then_some(l.hover),
                        ..Paint::NONE
                    },
                    ink: if active || chosen { l.ink } else { l.ink_2 },
                },
            )
            .width(Length::Fill)
            .padding([8, 10])
            .on_press(change(options[i].clone()))
            .boxed()
        });
        let anchor = layout.bounds();
        let position = anchor.position() + translation;
        let below = (window.height - position.y - anchor.height - 8.0).max(0.0);
        let above = (position.y - 8.0).max(0.0);
        let height = (self.options.len() as f32 * 36.0 + 12.0).min(320.0).min(below.max(above));
        let mut content = panel(
            scrollable(Column::with_children(choices)).id("select-options").height((height - 12.0).max(0.0)),
            12.0,
            &l,
        )
        .padding(6)
        .width(anchor.width)
        .boxed();
        let popup = &mut tree.children[1];
        popup.diff::<Message, Theme, Renderer>(&mut content);
        content.layout(popup, renderer, &layout::Limits::new(Size::ZERO, Size::new(anchor.width, height)));
        let y = if below >= height { position.y + anchor.height + 8.0 } else { (position.y - height - 8.0).max(0.0) };
        let placed = Layout::new(popup.size)
            .move_to(iced_core::Point::new(position.x.min(window.width - anchor.width).max(0.0), y));
        vec![overlay::Element::new(Box::new(Menu {
            content,
            tree: popup,
            state,
            layout: placed,
            window,
            labels: &self.labels,
            options,
            change,
        }))]
    }
}

struct Menu<'a, T, Message> {
    content: Element<'a, Message>,
    tree: &'a mut Tree,
    state: &'a mut State,
    layout: Layout,
    window: Size,
    labels: &'a [String],
    options: &'a [T],
    change: &'a dyn Fn(T) -> Message,
}

impl<T: Clone, Message: Clone> iced_core::Overlay<Message, Theme, Renderer> for Menu<'_, T, Message> {
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
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
            && !modifiers.command()
            && !modifiers.alt()
        {
            let (handled, pick) = self.state.key(key, self.labels);
            if let Some(i) = pick {
                shell.publish((self.change)(self.options[i].clone()));
            }
            if handled {
                shell.capture_event();
                let y = (self.state.active as f32 * 36.0 - self.layout.bounds().height / 2.0).max(0.0);
                let mut scroll = iced_core::widget::operation::scrollable::scroll_to::<()>(
                    "select-options".into(),
                    iced_core::widget::operation::scrollable::AbsoluteOffset { x: None, y: Some(y) },
                    iced_core::widget::operation::Animation::Instant,
                );
                self.content.operate(self.tree, self.layout, &self.layout.bounds(), renderer, &mut scroll);
            }
            shell.request_redraw();
            return;
        }
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)))
            && !cursor.is_over(self.layout.bounds())
        {
            self.state.open = false;
            shell.capture_event();
            shell.request_redraw();
            return;
        }
        self.content.update(self.tree, event, self.layout, cursor, renderer, shell, &self.layout.bounds());
        if shell.is_event_captured() && matches!(event, Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)))
        {
            self.state.open = false;
            shell.request_redraw();
        }
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        self.content.mouse_interaction(self.tree, self.layout, cursor, &self.layout.bounds(), renderer)
    }
}

#[cfg(test)]
mod tests {
    use keyboard::key::Named;

    use super::*;

    #[test]
    fn keyboard_selection_and_cancel_follow_native_behavior() {
        let labels = ["Home", "Library", "Calendar"].map(str::to_owned);
        let mut state = State::default();
        assert_eq!(state.key(&keyboard::Key::Named(Named::ArrowDown), &labels), (true, Some(1)));
        assert_eq!(state.key(&keyboard::Key::Named(Named::Enter), &labels), (true, None));
        assert!(state.open);
        assert_eq!(state.key(&keyboard::Key::Named(Named::End), &labels), (true, None));
        assert_eq!(state.key(&keyboard::Key::Named(Named::Escape), &labels), (true, None));
        assert!(!state.open);
        state.active = 1;
        state.key(&keyboard::Key::Named(Named::Space), &labels);
        assert_eq!(state.key(&keyboard::Key::Character("c".into()), &labels), (true, None));
        assert_eq!(state.key(&keyboard::Key::Named(Named::Enter), &labels), (true, Some(2)));
        assert!(!state.open);
    }
}
