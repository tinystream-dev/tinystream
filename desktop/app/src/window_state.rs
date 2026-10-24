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
pub struct WindowState {
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self { width: 1280.0, height: 820.0, maximized: false }
    }
}

impl WindowState {
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

        Self { width: valid(self.width, MIN_SIZE.width), height: valid(self.height, MIN_SIZE.height), ..self }
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
        let state = WindowState { width: 300.0, height: f32::NAN, maximized: true }.clamped();

        assert_eq!(state, WindowState { width: MIN_SIZE.width, height: MIN_SIZE.height, maximized: true });
    }

    #[test]
    fn keeps_larger_sizes() {
        let state = WindowState { width: 1920.0, height: 1080.0, maximized: false };

        assert_eq!(state.clamped(), state);
    }
}
