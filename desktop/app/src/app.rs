// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
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
    Credits,
}

pub struct App {
    pub history: History,
    pub chrome: Chrome,
    pub overlay: Option<Overlay>,
    pub query: String,
    pub session: crate::session::Session,
    /// The design system's gallery, shown instead of everything else (`--gallery`).
    pub gallery: Option<crate::ui::gallery::Gallery>,
    /// Whether the desktop asks for less motion.
    pub reduce_motion: bool,
    /// When the route last changed, and from where, while the pages are still crossing over.
    pub transition: Option<(Instant, Route)>,
    /// The video playing, while a `/watch` route is open.
    pub player: Option<crate::player::Player>,
    players: u64,
    video_active: bool,
    /// Where async work runs, for what needs it on other threads (mpv's streams).
    pub runtime: Option<tokio::runtime::Handle>,
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
    Gallery(crate::ui::gallery::Message),
    ReducedMotion(bool),
    /// For the player with this serial.
    Player(u64, crate::player::Message),
}

/// Sends the app a message from any thread.
pub type Sender = Arc<dyn Fn(Message) + Send + Sync>;

/// What the event loop does for the app after an update.
pub enum Effect {
    Window(chrome::Action),
    Operate(Box<dyn Operation>),
    Exit,
    Task(futures::future::BoxFuture<'static, Message>),
    Live(Option<crate::api::Api>, u64),
    Request(futures::future::BoxFuture<'static, Message>),
    Scale(f32),
    PictureInPicture(bool),
    VideoActivity(bool),
    PipWindow(chrome::Action),
    /// Starts something that sends messages for as long as it likes, like mpv's events.
    Listen(Box<dyn FnOnce(Sender) + Send>),
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
            gallery: None,
            reduce_motion: false,
            transition: None,
            player: None,
            players: 0,
            video_active: false,
            runtime: None,
            keys: Keys::default(),
            window_state,
        }
    }

    pub fn update(&mut self, message: Message, effects: &mut Vec<Effect>) {
        self.handle(message, effects);
        self.follow_route(effects);
        let active = self.player.as_ref().is_some_and(crate::player::Player::playback_active);
        if active != self.video_active {
            self.video_active = active;
            effects.push(Effect::VideoActivity(active));
        }
    }

    fn handle(&mut self, message: Message, effects: &mut Vec<Effect>) {
        match message {
            Message::Player(serial, message) => {
                let Some(player) = self.player.as_mut().filter(|player| player.serial == serial) else { return };
                if let Some(out) = player.update(message, effects) {
                    self.player_asks(out, effects);
                }
            },
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
                    if let Some(player) = &mut self.player {
                        player.pip = false;
                    }
                    effects.push(Effect::PictureInPicture(false));
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
                if let Some(player) = &mut self.player {
                    player.fullscreen = fullscreen;
                }
                self.window_state.maximized = maximized;
                if !maximized && !fullscreen && size.width > 0.0 && size.height > 0.0 {
                    self.window_state.width = size.width;
                    self.window_state.height = size.height;
                }
            },
            Message::CloseRequested => {
                if let Some(player) = &mut self.player {
                    player.save(effects);
                }
                if let Err(error) = self.window_state.save() {
                    tracing::warn!("can't remember the window's size: {error}");
                }
                effects.push(Effect::Exit);
            },
            Message::Chrome(chrome::Message::Control(chrome::Control::Close)) => {
                self.handle(Message::CloseRequested, effects);
            },
            Message::Chrome(message) => effects.extend(self.chrome.update(message).map(Effect::Window)),
            Message::ButtonLayout(layout) => self.chrome.layout = layout,
            Message::Shortcut(shortcut) => self.shortcut(shortcut, effects),
            Message::Navigate(route) => self.navigate(route, effects),
            Message::Scrolled(y) => self.history.set_scroll(y),
            Message::Overlay(overlay) => self.open(overlay, effects),
            Message::Query(query) => self.query = query,
            Message::Gallery(message) => {
                if let Some(gallery) = &mut self.gallery
                    && let Some(operation) = gallery.update(message, self.reduce_motion)
                {
                    effects.push(Effect::Operate(operation));
                }
            },
            Message::ReducedMotion(reduce) => {
                self.reduce_motion = reduce;
                self.session.reduce_motion = reduce;
            },
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

    pub fn ui_scale(&self) -> f32 {
        self.window_state.ui_scale
    }

    /// Starts the player when a video's route opens, and stops it (keeping its place) when it
    /// closes, or another video's opens.
    fn follow_route(&mut self, effects: &mut Vec<Effect>) {
        let watching = match self.history.current().route {
            Route::Watch(id, start) if self.session.screen == crate::session::Screen::Ready => Some((id as i64, start)),
            _ => None,
        };
        let keep_pip = self.player.as_ref().is_some_and(|player| player.pip)
            && self.session.screen == crate::session::Screen::Ready;
        if self.player.as_ref().is_some_and(|player| Some(player.id) != watching.map(|(id, _)| id))
            && !(keep_pip && watching.is_none())
            && let Some(mut player) = self.player.take()
        {
            player.save(effects);
            if watching.is_none() && self.chrome.fullscreen {
                effects.push(Effect::Window(chrome::Action::Fullscreen(false)));
            }
        }
        let (Some((id, start)), None) = (watching, &self.player) else { return };
        let Some(api) = self.session.api.clone() else { return };
        let server = self.session.config.active.unwrap_or_default();
        let user = self.session.config.active().and_then(|server| server.user.as_ref()).map_or(0, |user| user.id);
        self.players += 1;
        let mut player = crate::player::Player::new(
            self.players,
            id,
            start.map(f64::from),
            api,
            (server, user),
            self.runtime.clone(),
            effects,
        );
        player.fullscreen = self.chrome.fullscreen;
        player.pip = keep_pip;
        self.player = Some(player);
    }

    fn player_asks(&mut self, out: crate::player::Out, effects: &mut Vec<Effect>) {
        use crate::player::Out;
        match out {
            Out::Back { title } => {
                if self.chrome.fullscreen {
                    effects.push(Effect::Window(chrome::Action::Fullscreen(false)));
                }
                if self.history.can_go_back() {
                    self.shortcut(Shortcut::Back, effects);
                } else {
                    self.history.replace(title.map_or(Route::Home, |title| Route::Title(title as u64)));
                }
            },
            Out::Go(id) => self.history.replace(Route::Watch(id as u64, None)),
            Out::Fullscreen => {
                if let Some(player) = &mut self.player
                    && player.pip
                {
                    player.pip = false;
                    self.history.replace(Route::Watch(player.id as u64, None));
                    effects.push(Effect::PictureInPicture(false));
                }
                effects.push(Effect::Window(chrome::Action::Fullscreen(!self.chrome.fullscreen)));
            },
            Out::PictureInPicture(on) => {
                if on && self.chrome.fullscreen {
                    effects.push(Effect::Window(chrome::Action::Fullscreen(false)));
                }
                if !on && let Some(player) = &self.player {
                    self.history.replace(Route::Watch(player.id as u64, None));
                }
                effects.push(Effect::PictureInPicture(on));
            },
            Out::PipWindow(action) => effects.push(Effect::PipWindow(action)),
            Out::Clip(id) => self.navigate(Route::Clip(id as u64), effects),
        }
    }

    /// Whether something on screen moves without being asked to, so frames have to keep coming.
    pub fn animating(&self) -> bool {
        if self.player.as_ref().is_some_and(|player| player.animating()) {
            return true;
        }
        let crossing = self
            .transition
            .as_ref()
            .is_some_and(|(start, _)| crate::ui::effects::page_transition(start.elapsed()).is_some());
        crossing && !self.session.look().reduce_motion
            || self
                .gallery
                .as_ref()
                .is_some_and(|gallery| !gallery.look(self.reduce_motion).reduce_motion && gallery.animating())
    }

    /// Handles shell shortcuts, including search and Escape while a text field has focus.
    pub fn listen(&mut self, event: &Event, status: event::Status) -> Option<Message> {
        let watching = matches!(self.history.current().route, Route::Watch(..));
        self.listen_in(event, status, watching)
    }

    pub fn listen_in(&mut self, event: &Event, status: event::Status, watching: bool) -> Option<Message> {
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, modified_key, modifiers, .. }) = event
            && let Some(shortcut) =
                crate::keys::zoom(modified_key, *modifiers).or_else(|| crate::keys::zoom(key, *modifiers))
        {
            return Some(Message::Shortcut(shortcut));
        }
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if *key == keyboard::Key::Named(keyboard::key::Named::Tab) =>
            {
                Some(Message::Focus(modifiers.shift()))
            },
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if modifiers.command()
                    && matches!(key.as_ref(), keyboard::Key::Character("k" | "K"))
                    && self.session.removing.is_none()
                    && !self.gallery.as_ref().is_some_and(|g| g.modal_open())
                    && matches!(self.overlay, None | Some(Overlay::Search)) =>
            {
                Some(Message::Shortcut(Shortcut::ToggleSearch))
            },
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if *key == keyboard::Key::Named(keyboard::key::Named::Escape)
                    && self.overlay.is_some()
                    && status != event::Status::Captured =>
            {
                Some(Message::Overlay(None))
            },
            _ if status == event::Status::Captured => None,
            Event::Keyboard(keyboard::Event::KeyPressed { key, modified_key, modifiers, .. })
                if watching && self.overlay.is_none() && !self.keys.leading() && self.player.is_some() =>
            {
                let fullscreen = self.chrome.fullscreen;
                let player = self.player.as_mut()?;
                match player.key(key, modified_key, *modifiers, fullscreen) {
                    Some(message) => Some(Message::Player(player.serial, message)),
                    None if player.clipping.is_some() => None,
                    None => self.keys.press(modified_key, *modifiers, Instant::now()).map(Message::Shortcut),
                }
            },
            Event::Mouse(mouse::Event::CursorMoved { .. }) if watching && self.player.is_some() => {
                let player = self.player.as_ref()?;
                Some(Message::Player(player.serial, crate::player::Message::Poke))
            },
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
            Shortcut::Zoom(direction) => {
                if self.window_state.zoom(direction) {
                    effects.push(Effect::Scale(self.ui_scale()));
                }
            },
            Shortcut::ToggleSearch => self.toggle(Overlay::Search, effects),
            Shortcut::Search => self.open(Some(Overlay::Search), effects),
            Shortcut::ToggleShortcuts => self.toggle(Overlay::Shortcuts, effects),
            Shortcut::Notifications => self.open(Some(Overlay::Notifications), effects),
            Shortcut::Go(route) => self.navigate(route, effects),
            Shortcut::Back => {
                self.overlay = None;
                let from = self.history.current().route.clone();
                if self.history.back().is_some() {
                    self.transition = (!self.session.look().reduce_motion).then(|| (Instant::now(), from));
                    self.restore_scroll(effects);
                }
            },
            Shortcut::Forward => {
                self.overlay = None;
                let from = self.history.current().route.clone();
                if self.history.forward().is_some() {
                    self.transition = (!self.session.look().reduce_motion).then(|| (Instant::now(), from));
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
        let from = self.history.current().route.clone();
        if self.history.push(route) {
            if !self.session.look().reduce_motion {
                self.transition = Some((Instant::now(), from));
            }
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
    fn video_activity_follows_pause_end_and_detached_navigation() {
        use crate::player::Message as PlayerMessage;
        let mut app = App::new(Route::Watch(1, None), WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        app.player = Some(crate::player::tests::player());
        let serial = app.player.as_ref().unwrap().serial;
        let mut effects = Vec::new();
        app.update(Message::Player(serial, PlayerMessage::Poke), &mut effects);
        assert!(effects.iter().any(|effect| matches!(effect, Effect::VideoActivity(true))));
        effects.clear();
        app.update(Message::Player(serial, PlayerMessage::Toggle), &mut effects);
        assert!(effects.iter().any(|effect| matches!(effect, Effect::VideoActivity(false))));
        effects.clear();
        app.update(Message::Player(serial, PlayerMessage::Toggle), &mut effects);
        app.player.as_mut().unwrap().pip = true;
        effects.clear();
        app.update(Message::Navigate(Route::Home), &mut effects);
        assert!(app.player.is_some());
        assert!(!effects.iter().any(|effect| matches!(effect, Effect::VideoActivity(_))));
        app.player.as_mut().unwrap().time = 1799.0;
        effects.clear();
        app.player.as_mut().unwrap().ended = true;
        app.update(Message::Player(serial, PlayerMessage::Tick), &mut effects);
        assert!(effects.iter().any(|effect| matches!(effect, Effect::VideoActivity(false))));
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

    #[test]
    fn zoom_shortcuts_work_while_typing_and_in_dialogs() {
        let state = WindowState::default();
        let mut app = App::new(Route::Home, state);
        app.overlay = Some(Overlay::Credits);
        let mut effects = Vec::new();
        let mut plus = key("=", keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT);
        if let Event::Keyboard(keyboard::Event::KeyPressed { modified_key, .. }) = &mut plus {
            *modified_key = keyboard::Key::Character("+".into());
        }
        for event in [plus, key("=", keyboard::Modifiers::CTRL)] {
            let message = app.listen(&event, event::Status::Captured).unwrap();
            app.update(message, &mut effects);
        }
        assert_eq!(app.ui_scale(), 1.25);
        assert!(matches!(effects.last(), Some(Effect::Scale(1.25))));
        let message = app.listen(&key("-", keyboard::Modifiers::CTRL), event::Status::Captured).unwrap();
        app.update(message, &mut effects);
        assert_eq!(app.ui_scale(), 1.1);
        let message = app.listen(&key("0", keyboard::Modifiers::CTRL), event::Status::Captured).unwrap();
        app.update(message, &mut effects);
        assert_eq!(app.ui_scale(), 1.0);
        assert_eq!(app.window_state.size(), state.size());
        assert_eq!(app.overlay, Some(Overlay::Credits));
        assert!(app.listen(&key("+", keyboard::Modifiers::empty()), event::Status::Captured).is_none());
        assert!(
            app.listen(&key("0", keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT), event::Status::Captured)
                .is_none()
        );
    }
}
