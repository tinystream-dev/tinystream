// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Instant;

use iced_core::widget::operation::scrollable::AbsoluteOffset;
use iced_core::widget::operation::{self, Animation, Operation};
use iced_core::{Event, Size, event, keyboard, mouse};
use iced_widget::Element;

use crate::chrome::{self, ButtonLayout, Chrome};
use crate::keys::{Keys, Shortcut};
use crate::route::{History, Route};
use crate::shell;
use crate::window_state::WindowState;

/// Something drawn over the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Search,
    Shortcuts,
    Notifications,
    Account,
}

pub struct App {
    pub history: History,
    pub chrome: Chrome,
    pub overlay: Option<Overlay>,
    pub query: String,
    pub session: crate::session::Session,
    keys: Keys,
    window_state: WindowState,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The window's size changed, along with how the compositor shows it.
    Resized {
        size: Size,
        maximized: bool,
        fullscreen: bool,
    },
    CloseRequested,
    Chrome(chrome::Message),
    ButtonLayout(ButtonLayout),
    Shortcut(Shortcut),
    Navigate(Route),
    Scrolled(f32),
    Overlay(Option<Overlay>),
    Query(String),
    Submit,
    Focus(bool),
    Session(crate::session::Message),
}

/// What the event loop does for the app after an update.
pub enum Effect {
    Window(chrome::Action),
    Operate(Box<dyn Operation>),
    Exit,
    Task(futures::future::BoxFuture<'static, Message>),
    Live(Option<crate::api::Api>, u64),
    Request(futures::future::BoxFuture<'static, Message>),
}

pub const SCROLL: &str = "page";

impl App {
    pub fn new(route: Route, window_state: WindowState) -> Self {
        let mut chrome = Chrome::default();
        chrome.maximized = window_state.maximized;

        Self {
            history: History::new(route),
            chrome,
            overlay: None,
            query: String::new(),
            session: crate::session::Session::default(),
            keys: Keys::default(),
            window_state,
        }
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) {
        match message {
            Message::Focus(reverse) => {
                let operation: Box<dyn Operation> = if reverse {
                    Box::new(operation::focusable::focus_previous())
                } else {
                    Box::new(operation::focusable::focus_next())
                };
                effects.push(Effect::Operate(operation));
            },
            Message::Session(message) => {
                let generation = self.session.generation;
                let focus = self.session.focus_target();
                self.session.update(message, effects);
                if (self.session.focus_target() != focus || self.session.generation != generation)
                    && let Some(target) = self.session.focus_target()
                {
                    effects.push(Effect::Operate(Box::new(operation::focusable::focus(target.into()))));
                }
                if self.session.generation != generation {
                    self.history = History::new(Route::Home);
                    self.overlay = None;
                    self.query.clear();
                    self.keys = Keys::default();
                    self.restore_scroll(effects);
                }
            },
            Message::Resized { size, maximized, fullscreen } => {
                self.chrome.maximized = maximized;
                self.chrome.fullscreen = fullscreen;
                self.window_state.maximized = maximized;
                if !maximized && !fullscreen && size.width > 0.0 && size.height > 0.0 {
                    self.window_state.width = size.width;
                    self.window_state.height = size.height;
                }
            },
            Message::CloseRequested => {
                if let Err(error) = self.window_state.save() {
                    tracing::warn!("can't remember the window's size: {error}");
                }
                effects.push(Effect::Exit);
            },
            Message::Chrome(chrome::Message::Control(chrome::Control::Close)) => {
                self.update(Message::CloseRequested, effects);
            },
            Message::Chrome(message) => effects.extend(self.chrome.update(message).map(Effect::Window)),
            Message::ButtonLayout(layout) => self.chrome.layout = layout,
            Message::Shortcut(shortcut) => self.shortcut(shortcut, effects),
            Message::Navigate(route) => self.navigate(route, effects),
            Message::Scrolled(y) => self.history.set_scroll(y),
            Message::Overlay(overlay) => self.open(overlay, effects),
            Message::Query(query) => self.query = query,
            Message::Submit => {
                if let Some((_, route)) = shell::commands(&self.query).into_iter().next() {
                    self.navigate(route, effects);
                }
            },
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        shell::view(self)
    }

    /// Handles shell shortcuts, including search and Escape while a text field has focus.
    pub fn listen(&mut self, event: &Event, status: event::Status) -> Option<Message> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if *key == keyboard::Key::Named(keyboard::key::Named::Tab) =>
            {
                Some(Message::Focus(modifiers.shift()))
            },
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if modifiers.command() && matches!(key.as_ref(), keyboard::Key::Character("k" | "K")) =>
            {
                Some(Message::Shortcut(Shortcut::ToggleSearch))
            },
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if *key == keyboard::Key::Named(keyboard::key::Named::Escape) && self.overlay.is_some() =>
            {
                Some(Message::Overlay(None))
            },
            _ if status == event::Status::Captured => None,
            Event::Keyboard(keyboard::Event::KeyPressed { modified_key: key, modifiers, .. }) => {
                self.keys.press(key, *modifiers, Instant::now()).map(Message::Shortcut)
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Back)) => Some(Message::Shortcut(Shortcut::Back)),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Forward)) => {
                Some(Message::Shortcut(Shortcut::Forward))
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(Message::Chrome(chrome::Message::Release))
            },
            _ => None,
        }
    }

    fn shortcut(&mut self, shortcut: Shortcut, effects: &mut Vec<Effect>) {
        match shortcut {
            Shortcut::ToggleSearch => self.toggle(Overlay::Search, effects),
            Shortcut::Search => self.open(Some(Overlay::Search), effects),
            Shortcut::ToggleShortcuts => self.toggle(Overlay::Shortcuts, effects),
            Shortcut::Notifications => self.open(Some(Overlay::Notifications), effects),
            Shortcut::Go(route) => self.navigate(route, effects),
            Shortcut::Back => {
                self.overlay = None;
                if self.history.back().is_some() {
                    self.restore_scroll(effects);
                }
            },
            Shortcut::Forward => {
                self.overlay = None;
                if self.history.forward().is_some() {
                    self.restore_scroll(effects);
                }
            },
        }
    }

    fn toggle(&mut self, overlay: Overlay, effects: &mut Vec<Effect>) {
        self.open((self.overlay != Some(overlay)).then_some(overlay), effects);
    }

    fn open(&mut self, overlay: Option<Overlay>, effects: &mut Vec<Effect>) {
        self.overlay = overlay;
        if overlay == Some(Overlay::Search) {
            self.query.clear();
            effects.push(Effect::Operate(Box::new(operation::focusable::focus(shell::SEARCH.into()))));
        }
    }

    fn navigate(&mut self, route: Route, effects: &mut Vec<Effect>) {
        self.overlay = None;
        if self.history.push(route) {
            self.restore_scroll(effects);
        }
    }

    fn restore_scroll(&self, effects: &mut Vec<Effect>) {
        let offset = AbsoluteOffset { x: None, y: Some(self.history.current().scroll) };

        effects.push(Effect::Operate(Box::new(operation::scrollable::scroll_to(
            SCROLL.into(),
            offset,
            Animation::Instant,
        ))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: &str, modifiers: keyboard::Modifiers) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character(key.into()),
            modified_key: keyboard::Key::Character(key.into()),
            physical_key: keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    #[test]
    fn search_toggles_while_typing() {
        let mut app = App::new(Route::Home, WindowState::default());
        let mut effects = Vec::new();
        app.update(Message::Overlay(Some(Overlay::Search)), &mut effects);

        let message = app.listen(&key("k", keyboard::Modifiers::CTRL), event::Status::Captured).unwrap();
        app.update(message, &mut effects);

        assert_eq!(app.overlay, None);
    }

    #[test]
    fn typing_does_not_trigger_navigation() {
        let mut app = App::new(Route::Home, WindowState::default());

        assert!(app.listen(&key("g", keyboard::Modifiers::empty()), event::Status::Captured).is_none());
        assert!(app.listen(&key("c", keyboard::Modifiers::empty()), event::Status::Ignored).is_none());
    }

    #[test]
    fn shifted_slash_opens_shortcuts() {
        let mut app = App::new(Route::Home, WindowState::default());
        let mut event = key("/", keyboard::Modifiers::SHIFT);
        if let Event::Keyboard(keyboard::Event::KeyPressed { modified_key, .. }) = &mut event {
            *modified_key = keyboard::Key::Character("?".into());
        }

        assert!(matches!(
            app.listen(&event, event::Status::Ignored),
            Some(Message::Shortcut(Shortcut::ToggleShortcuts))
        ));
    }
}
