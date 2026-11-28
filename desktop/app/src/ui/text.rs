// SPDX-License-Identifier: AGPL-3.0-or-later

//! Type: Geist everywhere, at the sizes the web uses (Tailwind's scale plus a few exact ones).

use iced_core::text::{Ellipsis, LineHeight, Wrapping};
use iced_core::{Color, Font, font};
use iced_widget::Text;
use iced_widget::text::IntoFragment;

pub const GEIST: &[u8] = include_bytes!("../../assets/Geist.ttf");

pub const REGULAR: Font = Font::DEFAULT;
pub const MEDIUM: Font = Font { weight: font::Weight::Medium, ..Font::DEFAULT };
pub const SEMIBOLD: Font = Font { weight: font::Weight::Semibold, ..Font::DEFAULT };

/// A size and the line it sits on, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub px: f32,
    pub line: f32,
}

/// `text-2xs`.
pub const XS2: Size = Size { px: 11.0, line: 16.0 };
/// `text-xs`.
pub const XS: Size = Size { px: 12.0, line: 16.0 };
/// `text-[13px]`, the size of menus and small buttons.
pub const S13: Size = Size { px: 13.0, line: 19.5 };
/// `text-sm`.
pub const SM: Size = Size { px: 14.0, line: 20.0 };
/// `text-[15px]`: section titles and large buttons.
pub const S15: Size = Size { px: 15.0, line: 22.5 };
/// `text-base`.
pub const BASE: Size = Size { px: 16.0, line: 24.0 };
/// `text-lg`.
pub const LG: Size = Size { px: 18.0, line: 28.0 };
/// `text-xl`.
pub const XL: Size = Size { px: 20.0, line: 28.0 };
/// `text-2xl`.
pub const XL2: Size = Size { px: 24.0, line: 32.0 };
/// `text-[28px] leading-tight`: page titles.
pub const TITLE: Size = Size { px: 28.0, line: 35.0 };

/// Text at one of the sizes above.
pub fn text<'a>(content: impl IntoFragment<'a>, size: Size, color: Color) -> Text<'a> {
    inherit(content, size).color(color)
}

/// Text in the colour of what it's in, like a button's label.
pub fn inherit<'a>(content: impl IntoFragment<'a>, size: Size) -> Text<'a> {
    iced_widget::text(content).size(size.px).line_height(LineHeight::Absolute(size.line.into()))
}

/// One line, cut short with an ellipsis (`truncate`).
pub fn truncate<'a>(content: impl IntoFragment<'a>, size: Size, color: Color) -> Text<'a> {
    text(content, size, color).wrapping(Wrapping::None).ellipsis(Ellipsis::End)
}
