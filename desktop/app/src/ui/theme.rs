// SPDX-License-Identifier: AGPL-3.0-or-later

//! Colours and component styles: the server's appearance, as `web/src/styles.css` puts it on the
//! page. Every colour a screen uses is one of the 28 tokens here.

use iced_core::theme::palette::Seed;
use iced_core::{Color, Shadow, Theme, Vector};
use tinystream_theme::{self as scheme, TOKENS, Tokens};

/// How things are made: material and motion, never layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    /// How tinystream always looked: solid layers with soft shadows.
    #[default]
    Layered,
    /// No shadows, hairlines instead, tighter corners.
    Flat,
    /// See-through layers that blur what's behind them.
    Glass,
}

/// A glass material: what's behind is blurred and saturated, then tinted, and bent by a rounded
/// rim `bezel` wide (0 for flat, frosted glass).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub blur: f32,
    pub saturate: f32,
    pub tint: Color,
    pub bezel: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub canvas: Color,
    pub raised: Color,
    pub panel: Color,
    pub float: Color,
    pub ink: Color,
    pub ink_2: Color,
    pub ink_3: Color,
    pub line: Color,
    pub line_strong: Color,
    pub hover: Color,
    pub press: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub on_accent: Color,
    pub danger: Color,
    pub ok: Color,
    pub info: Color,
    pub info_deep: Color,
    pub warn: Color,
    pub warn_soft: Color,
    pub warn_deep: Color,
    pub highlight: Color,
    pub social: Color,
    pub focus: Color,
    pub selection: Color,
    pub scrollbar: Color,
    pub glow: Color,
    pub shade: Color,
    /// Over artwork and video: neutral, unless the scheme is asked to colour it too.
    pub media_ink: Color,
    pub media_shade: Color,
    pub media_panel: Color,
    pub media_canvas: Color,
    pub style: Style,
    pub dark: bool,
    /// Transitions, tilt and shimmer become instant or still.
    pub reduce_motion: bool,
}

impl Default for Look {
    /// Grey, the default scheme, with the Layered style.
    fn default() -> Self {
        let grey = scheme::find_builtin("grey").expect("Grey is built in").scheme;
        Self::new(&grey.tokens(), Style::Layered, false)
    }
}

impl Look {
    pub fn new(tokens: &Tokens, style: Style, media_tint: bool) -> Self {
        let get = |name: &str| color(tokens[scheme::token_index(name).expect("a token")]);
        let canvas = get("canvas");
        let ink = get("ink");
        let panel = get("panel");
        let (media_ink, media_shade, media_panel, media_canvas) = if media_tint {
            (ink, canvas, panel, canvas)
        } else {
            (Color::WHITE, Color::BLACK, Color::from_rgb8(0x26, 0x26, 0x26), Color::from_rgb8(0x10, 0x10, 0x10))
        };
        Self {
            canvas,
            raised: get("raised"),
            panel,
            float: get("float"),
            ink,
            ink_2: get("ink-2"),
            ink_3: get("ink-3"),
            line: get("line"),
            line_strong: get("line-strong"),
            hover: get("hover"),
            press: get("press"),
            accent: get("accent"),
            accent_hover: get("accent-hover"),
            on_accent: get("on-accent"),
            danger: get("danger"),
            ok: get("ok"),
            info: get("info"),
            info_deep: get("info-deep"),
            warn: get("warn"),
            warn_soft: get("warn-soft"),
            warn_deep: get("warn-deep"),
            highlight: get("highlight"),
            social: get("social"),
            focus: get("focus"),
            selection: get("selection"),
            scrollbar: get("scrollbar"),
            glow: get("glow"),
            shade: get("shade"),
            media_ink,
            media_shade,
            media_panel,
            media_canvas,
            style,
            dark: luma(canvas) < luma(ink),
            reduce_motion: false,
        }
    }

    /// A palette as the API sends it (`name`/`value` pairs), over Grey for anything missing.
    pub fn from_palette<'a>(
        tokens: impl IntoIterator<Item = (&'a str, &'a str)>,
        style: Style,
        media_tint: bool,
    ) -> Self {
        let mut all = scheme::find_builtin("grey").expect("Grey is built in").scheme.tokens();
        for (name, value) in tokens {
            if let (Some(i), Some(c)) = (scheme::token_index(name), scheme::Color::parse(value)) {
                all[i] = c;
            }
        }
        Self::new(&all, style, media_tint)
    }

    /// A `ts1` scheme code, as shared from Appearance settings.
    pub fn from_code(code: &str, style: Style) -> Result<Self, scheme::CodeError> {
        Ok(Self::new(&scheme::Scheme::decode(code)?.0.tokens(), style, false))
    }

    /// A token by its name in the scheme (`ink-2`, `line-strong`…).
    pub fn token(&self, name: &str) -> Option<Color> {
        let i = TOKENS.iter().position(|t| *t == name)?;
        Some(
            [
                self.canvas,
                self.raised,
                self.panel,
                self.float,
                self.ink,
                self.ink_2,
                self.ink_3,
                self.line,
                self.line_strong,
                self.hover,
                self.press,
                self.accent,
                self.accent_hover,
                self.on_accent,
                self.danger,
                self.ok,
                self.info,
                self.info_deep,
                self.warn,
                self.warn_soft,
                self.warn_deep,
                self.highlight,
                self.social,
                self.focus,
                self.selection,
                self.scrollbar,
                self.glow,
                self.shade,
            ][i],
        )
    }

    /// How big corners are relative to Layered, and how smooth (`CORNERS` in the shared theme).
    pub fn corners(&self) -> (f32, f32) {
        match self.style {
            Style::Layered => (1.0, 0.6),
            Style::Flat => (0.8, 0.0),
            Style::Glass => (1.25, 0.8),
        }
    }

    /// A squircle radius from the Layered one, as `Squircle.tsx` scales it.
    pub fn radius(&self, base: f32) -> f32 {
        (base * self.corners().0).round()
    }

    /// Tailwind's `rounded-*` sizes, which Flat and Glass redefine.
    pub fn rounded(&self, size: Rounded) -> f32 {
        let rem = match (size, self.style) {
            (Rounded::Md, Style::Flat) => 0.25,
            (Rounded::Md, Style::Glass) => 0.5,
            (Rounded::Md, _) => 0.375,
            (Rounded::Lg, Style::Flat) => 0.375,
            (Rounded::Lg, Style::Glass) => 0.75,
            (Rounded::Lg, _) => 0.5,
            (Rounded::Xl, Style::Flat) => 0.5,
            (Rounded::Xl, Style::Glass) => 1.0,
            (Rounded::Xl, _) => 0.75,
            (Rounded::Xl2, Style::Flat) => 0.75,
            (Rounded::Xl2, Style::Glass) => 1.25,
            (Rounded::Xl2, _) => 1.0,
        };
        rem * 16.0
    }

    /// The lit edge's opacity at the top, a third of the way down, and at the bottom.
    pub fn edge(&self) -> [f32; 3] {
        match self.style {
            Style::Layered => [0.14, 0.07, 0.045],
            Style::Flat => [0.1, 0.1, 0.1],
            Style::Glass => [0.26, 0.1, 0.16],
        }
    }

    /// The deep shadow under floating things (`--lift`, or the box shadow `.lift` gets on Glass).
    pub fn lift(&self) -> Vec<Lift> {
        match self.style {
            Style::Layered => vec![
                Lift { offset: 1.0, blur: 1.0, spread: 0.0, color: self.shade.scale_alpha(0.35) },
                Lift { offset: 12.0, blur: 32.0, spread: 0.0, color: self.shade.scale_alpha(0.45) },
            ],
            Style::Flat => Vec::new(),
            Style::Glass => vec![Lift { offset: 18.0, blur: 48.0, spread: -10.0, color: self.shade.scale_alpha(0.45) }],
        }
    }

    /// `--shadow-chip`: the lift of a chosen segment.
    pub fn chip_shadow(&self) -> Shadow {
        match self.style {
            Style::Layered => {
                Shadow { color: self.shade.scale_alpha(0.4), offset: Vector::new(0.0, 1.0), blur_radius: 2.0 }
            },
            Style::Flat => Shadow::default(),
            Style::Glass => {
                Shadow { color: self.shade.scale_alpha(0.3), offset: Vector::new(0.0, 1.0), blur_radius: 3.0 }
            },
        }
    }

    /// What dims the page behind a dialog or sheet: the canvas itself, so the background keeps its
    /// colour and only what's behind the panel fades.
    pub fn scrim(&self) -> Color {
        self.canvas.scale_alpha(0.7)
    }

    /// Where a popover starts its `pop`: how far up, and how small.
    pub fn pop(&self) -> (f32, f32) {
        match self.style {
            Style::Layered => (-3.0, 0.985),
            Style::Flat => (0.0, 1.0),
            Style::Glass => (-6.0, 0.95),
        }
    }

    /// `.material`: floating panels. Only Glass is see-through.
    pub fn material(&self) -> Option<Material> {
        (self.style == Style::Glass).then(|| Material {
            blur: 12.0,
            saturate: 1.8,
            tint: self.float.scale_alpha(0.45),
            bezel: 18.0,
        })
    }

    /// `.material-bar`: bars over the page.
    pub fn material_bar(&self) -> Option<Material> {
        (self.style == Style::Glass).then(|| Material {
            blur: 32.0,
            saturate: 1.8,
            tint: self.canvas.scale_alpha(0.62),
            bezel: 0.0,
        })
    }

    /// What's under a material that can't blur: the same shape, solid enough to read.
    pub fn solid_material(&self) -> Color {
        if self.style == Style::Glass { self.float.scale_alpha(0.96) } else { self.float }
    }

    /// For iced's own widgets, which take a palette rather than tokens.
    pub fn theme(&self) -> Theme {
        Theme::custom(
            "tinystream",
            Seed {
                background: self.canvas,
                text: self.ink,
                primary: self.accent,
                success: self.ok,
                warning: self.warn,
                danger: self.danger,
            },
        )
    }
}

/// Tailwind's corner sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounded {
    Md,
    Lg,
    Xl,
    Xl2,
}

/// One layer of a lift: a shadow below, blurred as CSS blurs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lift {
    pub offset: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

pub fn color(c: scheme::Color) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 1000.0)
}

fn luma(c: Color) -> f32 {
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

/// `top` painted over `bottom` at `amount`, like `color-mix(in srgb, top amount, bottom)`.
pub fn mix(top: Color, bottom: Color, amount: f32) -> Color {
    Color {
        r: top.r * amount + bottom.r * (1.0 - amount),
        g: top.g * amount + bottom.g * (1.0 - amount),
        b: top.b * amount + bottom.b * (1.0 - amount),
        a: top.a * amount + bottom.a * (1.0 - amount),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_grey() {
        let look = Look::default();
        assert_eq!(look.canvas, Color::from_rgb8(0x19, 0x19, 0x19));
        assert_eq!(look.ink_2, Color::from_rgb8(0x9B, 0x9B, 0x98));
        assert_eq!(look.hover, Color::from_rgba8(255, 255, 255, 0.055));
        assert!(look.dark);
    }

    #[test]
    fn every_token_has_a_place() {
        let look = Look::default();
        for name in TOKENS {
            assert!(look.token(name).is_some(), "{name}");
        }
        assert_eq!(look.token("line-strong"), Some(look.line_strong));
    }

    #[test]
    fn palettes_and_codes() {
        let light = Look::from_code("ts1.8fHvHBwbHBwbwkEtL4pGC3-4qGoAwDZ-bU_WBoWFgw", Style::Flat).unwrap();
        assert_eq!(light.raised, Color::from_rgb8(0xF6, 0xF6, 0xF5));
        assert!(!light.dark);
        assert_eq!(light.radius(9.0), 7.0);

        let custom = Look::from_palette([("canvas", "#000000"), ("line", "rgb(0 0 0 / 0.042)")], Style::Glass, true);
        assert_eq!(custom.canvas, Color::BLACK);
        assert_eq!(custom.line, Color::from_rgba8(0, 0, 0, 0.042));
        assert_eq!(custom.ink, Look::default().ink);
        assert_eq!(custom.media_canvas, Color::BLACK);
        assert_eq!(custom.radius(9.0), 11.0);
    }
}
