// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::theme::palette::Seed;
use iced_core::{Background, Color, Shadow, Theme, Vector, border};
use iced_widget::{button, container};

pub const SIDEBAR_WIDTH: f32 = 240.0;

pub fn theme() -> Theme {
    Look::default().theme()
}

pub fn sidebar(look: Look) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style { background: Some(look.raised.scale_alpha(0.6).into()), ..container::Style::default() }
}
pub fn hairline(look: Look) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style { background: Some(look.line.into()), ..container::Style::default() }
}
pub fn nav_item(active: bool, look: Look) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: if active { Some(look.press.into()) } else { hovered.then_some(look.hover.into()) },
            text_color: if active || hovered { look.ink } else { look.muted },
            border: border::rounded(8),
            ..button::Style::default()
        }
    }
}
pub fn search(look: Look) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: Some(look.panel.into()),
        text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            look.muted
        } else {
            look.faint
        },
        border: border::rounded(9).color(look.line).width(1),
        ..button::Style::default()
    }
}
pub fn menu_row(look: Look) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: hovered.then_some(look.hover.into()),
            text_color: if hovered { look.ink } else { look.muted },
            border: border::rounded(10),
            ..button::Style::default()
        }
    }
}
pub fn controls_pill(look: Look) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(look.float.scale_alpha(0.72).into()),
        border: border::rounded(999).color(look.line_strong).width(1),
        shadow: Shadow { color: look.shade.scale_alpha(0.35), offset: Vector::new(0.0, 4.0), blur_radius: 16.0 },
        ..container::Style::default()
    }
}
pub fn window_button(close: bool, look: Look) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match status {
            button::Status::Hovered if close => Some(look.danger),
            button::Status::Pressed if close => Some(look.danger.scale_alpha(0.8)),
            button::Status::Hovered => Some(look.hover),
            button::Status::Pressed => Some(look.press),
            _ => None,
        };
        button::Style {
            background: background.map(Background::Color),
            text_color: if close && matches!(status, button::Status::Hovered | button::Status::Pressed) {
                Color::WHITE
            } else {
                look.muted
            },
            border: border::rounded(999),
            ..button::Style::default()
        }
    }
}
pub fn backdrop(look: Look) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style { background: Some(look.shade.scale_alpha(0.5).into()), ..container::Style::default() }
}
pub fn dialog(look: Look) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(look.float.into()),
        border: border::rounded(18).color(look.line_strong).width(1),
        shadow: Shadow { color: look.shade.scale_alpha(0.5), offset: Vector::new(0.0, 16.0), blur_radius: 48.0 },
        ..container::Style::default()
    }
}
pub fn kbd(look: Look) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(look.panel.into()),
        border: border::rounded(6).color(look.line_strong).width(1),
        text_color: Some(look.ink),
        ..container::Style::default()
    }
}

#[derive(Clone, Copy)]
pub struct Look {
    pub canvas: Color,
    pub raised: Color,
    pub float: Color,
    pub panel: Color,
    pub hover: Color,
    pub press: Color,
    pub line_strong: Color,
    pub ink: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub on_accent: Color,
    pub danger: Color,
    pub line: Color,
    pub focus: Color,
    pub glow: Color,
    pub shade: Color,
    pub smoothing: f32,
    pub radius_scale: f32,
    pub elevated: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            canvas: Color::from_rgb8(14, 14, 14),
            raised: Color::from_rgb8(21, 21, 21),
            float: Color::from_rgb8(36, 36, 36),
            panel: Color::from_rgb8(28, 28, 28),
            hover: Color::from_rgba8(255, 255, 255, 0.055),
            press: Color::from_rgba8(255, 255, 255, 0.09),
            line_strong: Color::from_rgba8(255, 255, 255, 0.13),
            ink: Color::from_rgb8(230, 230, 228),
            muted: Color::from_rgb8(149, 149, 148),
            faint: Color::from_rgb8(105, 105, 104),
            accent: Color::from_rgb8(230, 230, 228),
            accent_hover: Color::WHITE,
            on_accent: Color::from_rgb8(14, 14, 14),
            danger: Color::from_rgb8(235, 138, 122),
            line: Color::from_rgba8(255, 255, 255, 0.075),
            focus: Color::from_rgba8(230, 230, 228, 0.35),
            glow: Color::WHITE,
            shade: Color::BLACK,
            smoothing: 0.6,
            radius_scale: 1.0,
            elevated: true,
        }
    }
}

impl Look {
    pub fn theme(self) -> Theme {
        Theme::custom(
            "tinystream",
            Seed {
                background: self.canvas,
                text: self.ink,
                primary: self.accent,
                success: Color::from_rgb8(128, 190, 140),
                warning: Color::from_rgb8(220, 180, 100),
                danger: self.danger,
            },
        )
    }

    pub fn token(&mut self, name: &str, value: &str) {
        let Some(color) = parse_color(value) else { return };
        match name {
            "canvas" => self.canvas = color,
            "raised" => self.raised = color,
            "float" => self.float = color,
            "panel" => self.panel = color,
            "hover" => self.hover = color,
            "press" => self.press = color,
            "line-strong" => self.line_strong = color,
            "ink" => self.ink = color,
            "ink-2" => self.muted = color,
            "ink-3" => self.faint = color,
            "accent" => self.accent = color,
            "accent-hover" => self.accent_hover = color,
            "on-accent" => self.on_accent = color,
            "danger" => self.danger = color,
            "line" => self.line = color,
            "focus" => self.focus = color,
            "glow" => self.glow = color,
            "shade" => self.shade = color,
            _ => {},
        }
    }
}

fn parse_color(value: &str) -> Option<Color> {
    if let Some(hex) = value.strip_prefix('#').filter(|hex| hex.len() == 6) {
        let rgb = u32::from_str_radix(hex, 16).ok()?;
        return Some(Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8));
    }
    let rgb = value.strip_prefix("rgb(")?.strip_suffix(')')?;
    let mut parts = rgb.split_whitespace().filter(|part| *part != "/");
    Some(Color::from_rgba8(
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}
