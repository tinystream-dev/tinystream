// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where the app keeps its files: the XDG directories on Linux, through `dirs` so other platforms
//! map onto their own.

use std::path::PathBuf;

const APP: &str = "tinystream";

/// Files edited by hand, like `mpv.conf`.
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(APP))
}

/// What the app remembers between runs, like the window's size.
pub fn state_dir() -> Option<PathBuf> {
    dirs::state_dir().or_else(dirs::data_local_dir).map(|dir| dir.join(APP))
}

pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join(APP))
}
