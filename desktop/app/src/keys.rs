// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shell's keyboard shortcuts, as in the web's `Shell.tsx`.

use std::time::{Duration, Instant};

use iced_core::keyboard::{Key, Modifiers, key};

use crate::route::Route;

/// How long after `g` the second key of a `g` combination counts.
const LEADER: Duration = Duration::from_millis(1200);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shortcut {
    ToggleSearch,
    Search,
    ToggleShortcuts,
    Notifications,
    Go(Route),
    Back,
    Forward,
}

#[derive(Debug, Default)]
pub struct Keys {
    leader: Option<Instant>,
}

impl Keys {
    /// Handles a key press that no widget used (so not typing in a field).
    pub fn press(&mut self, key: &Key, modifiers: Modifiers, now: Instant) -> Option<Shortcut> {
        let leading = self.leader.take().is_some_and(|at| now.duration_since(at) < LEADER);

        if let Key::Named(named) = key.as_ref() {
            return match named {
                key::Named::BrowserBack => Some(Shortcut::Back),
                key::Named::BrowserForward => Some(Shortcut::Forward),
                key::Named::ArrowLeft if modifiers.alt() => Some(Shortcut::Back),
                key::Named::ArrowRight if modifiers.alt() => Some(Shortcut::Forward),
                _ => None,
            };
        }

        let Key::Character(c) = key.as_ref() else { return None };
        let c = c.to_lowercase();

        if modifiers.command() && c == "k" {
            return Some(Shortcut::ToggleSearch);
        }
        if modifiers.command() || modifiers.alt() || modifiers.logo() {
            return None;
        }

        match c.as_str() {
            "/" => Some(Shortcut::Search),
            "?" => Some(Shortcut::ToggleShortcuts),
            "g" => {
                self.leader = Some(now);
                None
            },
            _ if leading => {
                let route = match c.as_str() {
                    "h" => Route::Home,
                    "c" => Route::Calendar,
                    "d" => Route::Downloads,
                    "w" => Route::Wanted,
                    "s" => Route::Settings,
                    "a" => Route::Discover,
                    "r" => Route::Requests,
                    "x" => Route::Clips,
                    _ => return None,
                };
                Some(Shortcut::Go(route))
            },
            "n" => Some(Shortcut::Notifications),
            _ => None,
        }
    }
}

/// The shortcuts sheet's contents, as on web.
pub const SHEET: &[(&str, &[(&str, &str)])] = &[
    (
        "Anywhere",
        &[
            ("Ctrl+K", "Search and commands"),
            ("/", "Search"),
            ("?", "This list"),
            ("N", "Notifications"),
            ("G then H", "Home"),
            ("G then C", "Calendar"),
            ("G then A", "Add shows"),
            ("G then D", "Downloads"),
            ("G then W", "Wanted"),
            ("G then X", "Clips"),
            ("G then S", "Settings"),
            ("Alt+←  Alt+→", "Back or forward"),
        ],
    ),
    (
        "Watching",
        &[
            ("Space  K", "Play or pause"),
            ("←  →", "Back or forward 5 s"),
            ("J  L", "Back or forward 10 s"),
            ("↑  ↓", "Volume"),
            ("M", "Mute"),
            ("F", "Full screen"),
            ("C", "Next subtitle track"),
            ("S", "Skip opening or credits"),
            ("N", "Next episode"),
            ("<  >", "Slower or faster"),
            ("P", "Picture in picture"),
            ("X", "Make a clip"),
            ("Shift+S", "Take a screenshot"),
            ("0–9", "Jump to 0–90%"),
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn press(keys: &mut Keys, c: &str, modifiers: Modifiers, at: Instant) -> Option<Shortcut> {
        keys.press(&Key::Character(c.into()), modifiers, at)
    }

    #[test]
    fn g_combinations() {
        let mut keys = Keys::default();
        let now = Instant::now();

        assert_eq!(press(&mut keys, "g", Modifiers::empty(), now), None);
        assert_eq!(
            press(&mut keys, "c", Modifiers::empty(), now + Duration::from_millis(300)),
            Some(Shortcut::Go(Route::Calendar))
        );
        // The leader is used up.
        assert_eq!(press(&mut keys, "c", Modifiers::empty(), now + Duration::from_millis(400)), None);
    }

    #[test]
    fn leader_expires() {
        let mut keys = Keys::default();
        let now = Instant::now();

        press(&mut keys, "g", Modifiers::empty(), now);
        assert_eq!(press(&mut keys, "h", Modifiers::empty(), now + LEADER), None);
    }

    #[test]
    fn n_after_g_is_not_notifications() {
        let mut keys = Keys::default();
        let now = Instant::now();

        press(&mut keys, "g", Modifiers::empty(), now);
        assert_eq!(press(&mut keys, "n", Modifiers::empty(), now), None);
        assert_eq!(press(&mut keys, "n", Modifiers::empty(), now), Some(Shortcut::Notifications));
    }

    #[test]
    fn modifiers() {
        let mut keys = Keys::default();
        let now = Instant::now();

        assert_eq!(press(&mut keys, "K", Modifiers::CTRL, now), Some(Shortcut::ToggleSearch));
        assert_eq!(press(&mut keys, "/", Modifiers::CTRL, now), None);
        assert_eq!(press(&mut keys, "?", Modifiers::SHIFT, now), Some(Shortcut::ToggleShortcuts));
    }
}
