// SPDX-License-Identifier: AGPL-3.0-or-later

//! The window's size and maximized state, kept between runs.

use std::path::PathBuf;
use std::{fs, io};

use iced_core::Size;
use serde::{Deserialize, Serialize};

use crate::paths;

/// The web's `md` breakpoint: narrower than this, the web switches to its phone layout.
pub const MIN_SIZE: Size = Size::new(768.0, 520.0);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
    pub ui_scale: f32,
}

impl Default for WindowState {
    fn default() -> Self {
        Self { width: 1280.0, height: 820.0, maximized: false, ui_scale: 1.0 }
    }
}

impl WindowState {
    /// Browser-style zoom steps, independent of the display's scale factor. Zero resets to 100%.
    pub fn zoom(&mut self, direction: i8) -> bool {
        const LEVELS: &[f32] = &[0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
        let next = match direction.cmp(&0) {
            std::cmp::Ordering::Greater => LEVELS.iter().copied().find(|s| *s > self.ui_scale).unwrap_or(2.0),
            std::cmp::Ordering::Less => LEVELS.iter().rev().copied().find(|s| *s < self.ui_scale).unwrap_or(0.5),
            std::cmp::Ordering::Equal => 1.0,
        };
        let changed = next != self.ui_scale;
        self.ui_scale = next;
        changed
    }

    pub fn load() -> Self {
        let Some(path) = file() else { return Self::default() };

        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Self>(&bytes) {
                Ok(state) => state.clamped(),
                Err(error) => {
                    tracing::warn!("ignoring {}: {error}", path.display());
                    Self::default()
                },
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                tracing::warn!("can't read {}: {error}", path.display());
                Self::default()
            },
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let path = file().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no state directory"))?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let partial = path.with_extension("json.partial");
        fs::write(&partial, serde_json::to_vec_pretty(self).map_err(io::Error::other)?)?;
        fs::rename(partial, path)
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    fn clamped(self) -> Self {
        let valid = |v: f32, min: f32| if v.is_finite() { v.max(min) } else { min };

        Self {
            width: valid(self.width, MIN_SIZE.width),
            height: valid(self.height, MIN_SIZE.height),
            ui_scale: if self.ui_scale.is_finite() { self.ui_scale.clamp(0.5, 2.0) } else { 1.0 },
            ..self
        }
    }
}

fn file() -> Option<PathBuf> {
    paths::state_dir().map(|dir| dir.join("window.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_to_the_minimum() {
        let state = WindowState { width: 300.0, height: f32::NAN, maximized: true, ..WindowState::default() }.clamped();

        assert_eq!(
            state,
            WindowState { width: MIN_SIZE.width, height: MIN_SIZE.height, maximized: true, ..WindowState::default() }
        );
    }

    #[test]
    fn keeps_larger_sizes() {
        let state = WindowState { width: 1920.0, height: 1080.0, ..WindowState::default() };

        assert_eq!(state.clamped(), state);
    }

    #[test]
    fn zoom_steps_reset_and_survive_serialization() {
        let mut state: WindowState = serde_json::from_str(r#"{"width":1280,"height":800,"maximized":false}"#).unwrap();
        assert_eq!(state.ui_scale, 1.0);
        state.zoom(1);
        assert_eq!(state.ui_scale, 1.1);
        state.zoom(1);
        assert_eq!(state.ui_scale, 1.25);
        let restored: WindowState = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(restored, state);
        for _ in 0..30 {
            state.zoom(-1);
        }
        assert_eq!(state.ui_scale, 0.5);
        for _ in 0..30 {
            state.zoom(1);
        }
        assert_eq!(state.ui_scale, 2.0);
        state.zoom(0);
        assert_eq!(state.ui_scale, 1.0);
        assert_eq!(state.size(), Size::new(1280.0, 800.0));
        state.ui_scale = f32::NAN;
        assert_eq!(state.clamped().ui_scale, 1.0);
        state.ui_scale = 10.0;
        assert_eq!(state.clamped().ui_scale, 2.0);
    }
}
