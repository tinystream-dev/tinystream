// SPDX-License-Identifier: AGPL-3.0-or-later

//! The web UI's routes, so a web URL or `tinystream://` link (#92) opens the same screen.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Route {
    Home,
    Calendar,
    Clips,
    Discover,
    Downloads,
    Requests,
    Settings,
    Wanted,
    Library(String),
    Series { library: String, series: String },
    Title(u64),
    Watch(u64),
    Artist(u64),
    Album(u64),
    Playlist(u64),
    Book(u64),
    Read(u64),
    Together(String),
    Listen(String),
}

impl Route {
    /// Parses a path like `/title/12`, ignoring any query or fragment.
    pub fn parse(path: &str) -> Option<Self> {
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let parts: Vec<String> = path.split('/').filter(|part| !part.is_empty()).map(decode).collect();
        let id = |s: &str| s.parse().ok();

        Some(match parts.iter().map(String::as_str).collect::<Vec<_>>()[..] {
            [] => Self::Home,
            ["calendar"] => Self::Calendar,
            ["clips"] => Self::Clips,
            ["discover"] => Self::Discover,
            ["downloads"] => Self::Downloads,
            ["requests"] => Self::Requests,
            ["settings"] => Self::Settings,
            ["wanted"] => Self::Wanted,
            ["library", name] => Self::Library(name.to_owned()),
            ["library", library, "series", series] => {
                Self::Series { library: library.to_owned(), series: series.to_owned() }
            },
            ["title", n] => Self::Title(id(n)?),
            ["watch", n] => Self::Watch(id(n)?),
            ["artist", n] => Self::Artist(id(n)?),
            ["album", n] => Self::Album(id(n)?),
            ["playlist", n] => Self::Playlist(id(n)?),
            ["book", n] => Self::Book(id(n)?),
            ["read", n] => Self::Read(id(n)?),
            ["together", code] => Self::Together(code.to_owned()),
            ["listen", code] => Self::Listen(code.to_owned()),
            _ => return None,
        })
    }

    /// Screens that take the whole window, without the sidebar, as on web.
    pub fn is_focus(&self) -> bool {
        matches!(self, Self::Watch(_) | Self::Together(_) | Self::Listen(_))
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Home => f.write_str("/"),
            Self::Calendar => f.write_str("/calendar"),
            Self::Clips => f.write_str("/clips"),
            Self::Discover => f.write_str("/discover"),
            Self::Downloads => f.write_str("/downloads"),
            Self::Requests => f.write_str("/requests"),
            Self::Settings => f.write_str("/settings"),
            Self::Wanted => f.write_str("/wanted"),
            Self::Library(name) => write!(f, "/library/{}", encode(name)),
            Self::Series { library, series } => write!(f, "/library/{}/series/{}", encode(library), encode(series)),
            Self::Title(id) => write!(f, "/title/{id}"),
            Self::Watch(id) => write!(f, "/watch/{id}"),
            Self::Artist(id) => write!(f, "/artist/{id}"),
            Self::Album(id) => write!(f, "/album/{id}"),
            Self::Playlist(id) => write!(f, "/playlist/{id}"),
            Self::Book(id) => write!(f, "/book/{id}"),
            Self::Read(id) => write!(f, "/read/{id}"),
            Self::Together(code) => write!(f, "/together/{}", encode(code)),
            Self::Listen(code) => write!(f, "/listen/{}", encode(code)),
        }
    }
}

/// Where the user has been, newest last, with how far down each page was scrolled.
#[derive(Debug)]
pub struct History {
    entries: Vec<Entry>,
    forward: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub route: Route,
    pub scroll: f32,
}

impl History {
    pub fn new(route: Route) -> Self {
        Self { entries: vec![Entry { route, scroll: 0.0 }], forward: Vec::new() }
    }

    pub fn current(&self) -> &Entry {
        self.entries.last().expect("history is never empty")
    }

    pub fn can_go_back(&self) -> bool {
        self.entries.len() > 1
    }

    /// Opens `route` at the top. Opening the current route again does nothing.
    pub fn push(&mut self, route: Route) -> bool {
        if self.current().route == route {
            return false;
        }
        self.entries.push(Entry { route, scroll: 0.0 });
        self.forward.clear();
        true
    }

    pub fn back(&mut self) -> Option<&Entry> {
        if !self.can_go_back() {
            return None;
        }
        self.forward.extend(self.entries.pop());
        self.entries.last()
    }

    pub fn forward(&mut self) -> Option<&Entry> {
        let entry = self.forward.pop()?;
        self.entries.push(entry);
        self.entries.last()
    }

    pub fn set_scroll(&mut self, scroll: f32) {
        if let Some(entry) = self.entries.last_mut() {
            entry.scroll = scroll;
        }
    }
}

fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                i += 3;
            },
            (byte, ..) => {
                out.push(byte);
                i += 1;
            },
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_web_paths() {
        assert_eq!(Route::parse("/"), Some(Route::Home));
        assert_eq!(Route::parse(""), Some(Route::Home));
        assert_eq!(Route::parse("/title/42"), Some(Route::Title(42)));
        assert_eq!(Route::parse("/watch/7?t=30"), Some(Route::Watch(7)));
        assert_eq!(Route::parse("/library/Anime%20Shows"), Some(Route::Library("Anime Shows".into())));
        assert_eq!(
            Route::parse("/library/Books/series/Monogatari"),
            Some(Route::Series { library: "Books".into(), series: "Monogatari".into() })
        );
        assert_eq!(Route::parse("/title/nope"), None);
        assert_eq!(Route::parse("/nowhere"), None);
    }

    #[test]
    fn round_trips() {
        for route in [
            Route::Home,
            Route::Wanted,
            Route::Library("Films & Shows/日本".into()),
            Route::Series { library: "a b".into(), series: "100%".into() },
            Route::Together("AB12".into()),
            Route::Read(3),
        ] {
            assert_eq!(Route::parse(&route.to_string()), Some(route));
        }
    }

    #[test]
    fn remembers_scroll_when_going_back() {
        let mut history = History::new(Route::Home);
        history.set_scroll(640.0);
        assert!(history.push(Route::Title(1)));
        assert!(!history.push(Route::Title(1)));
        assert_eq!(history.current().scroll, 0.0);

        assert_eq!(history.back(), Some(&Entry { route: Route::Home, scroll: 640.0 }));
        assert_eq!(history.back(), None);
        assert_eq!(history.forward().map(|entry| &entry.route), Some(&Route::Title(1)));
    }

    #[test]
    fn pushing_drops_forward_entries() {
        let mut history = History::new(Route::Home);
        history.push(Route::Clips);
        history.back();
        history.push(Route::Settings);

        assert_eq!(history.forward(), None);
    }
}
