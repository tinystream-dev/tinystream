// SPDX-License-Identifier: AGPL-3.0-or-later

//! The web's dark tokens and the few styles the shell needs. The design system (#82) replaces this
//! with the shared theme crate and the server's appearance settings.

use iced_core::theme::palette::Seed;
use iced_core::{Background, Border, Color, Shadow, Theme, Vector, border};
use iced_widget::{button, container};

pub const CANVAS: Color = Color::from_rgb8(0x19, 0x19, 0x19);
pub const RAISED: Color = Color::from_rgb8(0x20, 0x20, 0x20);
pub const PANEL: Color = Color::from_rgb8(0x26, 0x26, 0x26);
pub const FLOAT: Color = Color::from_rgb8(0x2E, 0x2E, 0x2E);
pub const HOVER: Color = Color::from_rgba8(255, 255, 255, 0.055);
pub const PRESS: Color = Color::from_rgba8(255, 255, 255, 0.09);
pub const LINE: Color = Color::from_rgba8(255, 255, 255, 0.075);
pub const LINE_STRONG: Color = Color::from_rgba8(255, 255, 255, 0.13);
pub const INK: Color = Color::from_rgb8(0xEB, 0xEB, 0xEA);
pub const INK_2: Color = Color::from_rgb8(0x9B, 0x9B, 0x98);
pub const INK_3: Color = Color::from_rgb8(0x6E, 0x6E, 0x6B);
pub const DANGER: Color = Color::from_rgb8(0xE5, 0x48, 0x4D);
pub const SHADE: Color = Color::BLACK;

pub const SIDEBAR_WIDTH: f32 = 240.0;

pub fn theme() -> Theme {
    Theme::custom(
        "tinystream",
        Seed {
            background: CANVAS,
            text: INK,
            primary: INK,
            success: Color::from_rgb8(0x4C, 0xC3, 0x8A),
            warning: Color::from_rgb8(0xF5, 0xB7, 0x4F),
            danger: DANGER,
        },
    )
}

/// `bg-raised/60` with a hairline on the right.
pub fn sidebar(_: &Theme) -> container::Style {
    container::Style { background: Some(Background::Color(RAISED.scale_alpha(0.6))), ..container::Style::default() }
}

pub fn hairline(_: &Theme) -> container::Style {
    container::Style { background: Some(Background::Color(LINE)), ..container::Style::default() }
}

pub fn canvas(_: &Theme) -> container::Style {
    container::Style { background: Some(Background::Color(CANVAS)), ..container::Style::default() }
}

pub fn nav_item(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (background, text_color) = match (active, status) {
            (true, _) => (Some(PRESS), INK),
            (false, button::Status::Hovered | button::Status::Pressed) => (Some(HOVER), INK),
            (false, _) => (None, INK_2),
        };

        button::Style {
            background: background.map(Background::Color),
            text_color,
            border: border::rounded(8),
            ..button::Style::default()
        }
    }
}

/// The search field-like button at the top of the sidebar.
pub fn search(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(PANEL)),
        text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) { INK_2 } else { INK_3 },
        border: border::rounded(9).color(LINE).width(1),
        ..button::Style::default()
    }
}

pub fn menu_row(_: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);

    button::Style {
        background: hovered.then_some(Background::Color(HOVER)),
        text_color: if hovered { INK } else { INK_2 },
        border: border::rounded(10),
        ..button::Style::default()
    }
}

/// The window controls' pill, floating over the content.
pub fn controls_pill(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(FLOAT.scale_alpha(0.72))),
        border: Border { color: LINE_STRONG, width: 1.0, radius: 999.0.into() },
        shadow: Shadow { color: SHADE.scale_alpha(0.35), offset: Vector::new(0.0, 4.0), blur_radius: 16.0 },
        ..container::Style::default()
    }
}

pub fn window_button(close: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            button::Status::Hovered if close => Some(DANGER),
            button::Status::Pressed if close => Some(DANGER.scale_alpha(0.8)),
            button::Status::Hovered => Some(HOVER),
            button::Status::Pressed => Some(PRESS),
            _ => None,
        };

        button::Style {
            background: background.map(Background::Color),
            text_color: if close && matches!(status, button::Status::Hovered | button::Status::Pressed) {
                Color::WHITE
            } else {
                INK_2
            },
            border: border::rounded(999),
            ..button::Style::default()
        }
    }
}

/// The dimmed backdrop behind dialogs.
pub fn backdrop(_: &Theme) -> container::Style {
    container::Style { background: Some(Background::Color(SHADE.scale_alpha(0.5))), ..container::Style::default() }
}

pub fn dialog(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(FLOAT)),
        border: Border { color: LINE_STRONG, width: 1.0, radius: 18.0.into() },
        shadow: Shadow { color: SHADE.scale_alpha(0.5), offset: Vector::new(0.0, 16.0), blur_radius: 48.0 },
        ..container::Style::default()
    }
}

pub fn kbd(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(PANEL)),
        border: Border { color: LINE_STRONG, width: 1.0, radius: 6.0.into() },
        text_color: Some(INK),
        ..container::Style::default()
    }
}
