// SPDX-License-Identifier: AGPL-3.0-or-later

//! The design system (#82): the web UI's tokens, type, icons, component styles and effects, and
//! the widgets screens are built from. `web/src/styles.css` and `web/src/components` are the spec.

pub mod backdrop;
pub mod controls;
pub mod credits;
pub mod effects;
pub mod gallery;
pub mod icon;
pub mod layout;
pub mod logo;
pub mod media;
pub mod modal;
pub mod motion;
pub mod numbers;
pub mod overlay;
pub mod pressable;
pub mod select;
pub mod squircle;
pub mod text;
pub mod theme;
pub mod tilt;

pub use theme::{Look, Style};
