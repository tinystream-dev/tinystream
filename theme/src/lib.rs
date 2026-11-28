// SPDX-License-Identifier: AGPL-3.0-or-later
//! Colour schemes: a few seed colours, everything else derived from them, and
//! any derived colour optionally pinned. Shared as short codes.
//!
//! `ts1` is frozen: its encoding, the token list and the derivation below must
//! never change, or old codes would render differently. Anything better ships
//! as `ts2`, next to it. Derivation is integer-only so every frontend that
//! ports it gets the same colours, to the bit.

use std::collections::BTreeMap;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Every colour the UI uses, in `ts1` order (an override's index is its place here).
pub const TOKENS: [&str; 28] = [
    "canvas",
    "raised",
    "panel",
    "float",
    "ink",
    "ink-2",
    "ink-3",
    "line",
    "line-strong",
    "hover",
    "press",
    "accent",
    "accent-hover",
    "on-accent",
    "danger",
    "ok",
    "info",
    "info-deep",
    "warn",
    "warn-soft",
    "warn-deep",
    "highlight",
    "social",
    "focus",
    "selection",
    "scrollbar",
    "glow",
    "shade",
];

/// The colours a scheme is made from, in `ts1` order.
pub const SEEDS: [&str; 9] = ["canvas", "ink", "accent", "danger", "ok", "info", "warn", "highlight", "social"];

pub const NAME_MAX: usize = 48;

const OPAQUE: u16 = 1000;

/// An sRGB colour; alpha in thousandths, so `0.055` stays exactly that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u16,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: OPAQUE }
    }

    const fn hex(v: u32) -> Self {
        Self::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    pub fn opaque(self) -> bool {
        self.a == OPAQUE
    }

    fn alpha(self, a: u16) -> Self {
        Self { a, ..self }
    }

    /// `#rrggbb`, or `rgb(r g b / a)` for anything see-through.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();

        if let Some(hex) = s.strip_prefix('#') {
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }

            return u32::from_str_radix(hex, 16).ok().map(Self::hex);
        }

        let inner = s.strip_prefix("rgb(")?.strip_suffix(')')?;
        let (channels, alpha) = inner.split_once('/')?;
        let mut channels = channels.split_whitespace().map(|c| c.parse::<u8>().ok());
        let (r, g, b) = (channels.next()??, channels.next()??, channels.next()??);

        if channels.next().is_some() {
            return None;
        }

        let a = parse_alpha(alpha.trim())?;
        Some(Self { r, g, b, a })
    }

    /// Weighted by how bright each channel looks; only compared, never shown.
    fn luma(self) -> u32 {
        2126 * self.r as u32 + 7152 * self.g as u32 + 722 * self.b as u32
    }

    /// `self` moved `t` thousandths of the way to `to`; keeps `self`'s alpha.
    fn mix(self, to: Color, t: u32) -> Self {
        let ch = |a: u8, b: u8| ((a as u32 * (1000 - t) + b as u32 * t + 500) / 1000) as u8;
        Self { r: ch(self.r, to.r), g: ch(self.g, to.g), b: ch(self.b, to.b), a: self.a }
    }
}

/// `0.055` → 55. At most three decimals, so every value has one spelling.
fn parse_alpha(s: &str) -> Option<u16> {
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));

    if frac.len() > 3 || !whole.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) || whole.is_empty() {
        return None;
    }

    let whole: u16 = whole.parse().ok()?;
    let frac: u16 = if frac.is_empty() { 0 } else { format!("{frac:0<3}").parse().ok()? };
    let a = whole.checked_mul(1000)?.checked_add(frac)?;
    (a <= OPAQUE).then_some(a)
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.opaque() {
            return write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b);
        }

        let alpha = format!("{:03}", self.a);

        match alpha.trim_end_matches('0') {
            "" => write!(f, "rgb({} {} {} / 0)", self.r, self.g, self.b),
            alpha => write!(f, "rgb({} {} {} / 0.{alpha})", self.r, self.g, self.b),
        }
    }
}

pub type Tokens = [Color; TOKENS.len()];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    pub seeds: [Color; SEEDS.len()],
    /// By index into [`TOKENS`].
    pub overrides: BTreeMap<usize, Color>,
}

pub fn token_index(name: &str) -> Option<usize> {
    TOKENS.iter().position(|t| *t == name)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodeError {
    #[error("that isn't a tinystream colour scheme code")]
    NotACode,
    #[error("this code is from a newer tinystream ({0}); update to use it")]
    Version(String),
    #[error("this code is damaged")]
    Damaged,
}

impl Scheme {
    /// Light ink on a dark canvas.
    pub fn dark(&self) -> bool {
        self.seeds[0].luma() < self.seeds[1].luma()
    }

    /// Every token, with the overrides applied. Frozen for `ts1`.
    pub fn tokens(&self) -> Tokens {
        let [canvas, ink, accent, danger, ok, info, warn, highlight, social] = self.seeds;
        let dark = self.dark();
        let glow = if dark { Color::WHITE } else { Color::BLACK };

        let [raised, panel, float] =
            if dark { [30, 57, 91] } else { [350, 650, 1000] }.map(|t| canvas.mix(Color::WHITE, t));

        let on_accent =
            if accent.luma().abs_diff(canvas.luma()) >= accent.luma().abs_diff(ink.luma()) { canvas } else { ink };

        let mut t = [
            canvas,
            raised,
            panel,
            float,
            ink,
            ink.mix(canvas, 381),
            ink.mix(canvas, 600),
            glow.alpha(75),
            glow.alpha(130),
            glow.alpha(55),
            glow.alpha(90),
            accent,
            accent.mix(glow, 250),
            on_accent,
            danger,
            ok,
            info,
            info.mix(canvas, 200),
            warn,
            warn.mix(ink, 400),
            warn.mix(canvas, 200),
            highlight,
            social,
            glow.alpha(550),
            glow.alpha(180),
            glow.alpha(120),
            glow,
            Color::BLACK,
        ];

        for (&i, &c) in &self.overrides {
            t[i] = c;
        }

        t
    }

    /// `ts1.<payload>`: what the scheme is, without its name.
    pub fn code(&self) -> String {
        let mut bytes = Vec::with_capacity(SEEDS.len() * 3 + self.overrides.len() * 6);

        for c in self.seeds {
            bytes.extend([c.r, c.g, c.b]);
        }

        for (&i, &c) in &self.overrides {
            let alpha = if c.opaque() { 0 } else { 0x80 };
            bytes.extend([i as u8 | alpha, c.r, c.g, c.b]);

            if !c.opaque() {
                bytes.extend(c.a.to_be_bytes());
            }
        }

        format!("ts1.{}", URL_SAFE_NO_PAD.encode(bytes))
    }

    /// The code with a name attached, for sharing.
    pub fn share_code(&self, name: &str) -> String {
        let name = clamp_name(name);
        if name.is_empty() { self.code() } else { format!("{}.{}", self.code(), URL_SAFE_NO_PAD.encode(name)) }
    }

    /// A code and the name it carried, if any.
    pub fn decode(code: &str) -> Result<(Scheme, Option<String>), CodeError> {
        let mut parts = code.trim().split('.');
        let version = parts.next().unwrap_or_default();

        if version != "ts1" {
            let newer = version.strip_prefix("ts").is_some_and(|v| v.parse::<u32>().is_ok_and(|v| v > 1));
            return Err(if newer { CodeError::Version(version.into()) } else { CodeError::NotACode });
        }

        let payload = parts.next().ok_or(CodeError::NotACode)?;
        let name = parts.next();

        if parts.next().is_some() {
            return Err(CodeError::Damaged);
        }

        let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|_| CodeError::Damaged)?;

        let name = match name {
            Some(n) => {
                let raw = URL_SAFE_NO_PAD.decode(n).map_err(|_| CodeError::Damaged)?;
                let name = String::from_utf8(raw).map_err(|_| CodeError::Damaged)?;
                Some(clamp_name(&name)).filter(|n| !n.is_empty())
            },
            None => None,
        };

        Ok((Self::from_bytes(&bytes)?, name))
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, CodeError> {
        let seeds_len = SEEDS.len() * 3;

        if bytes.len() < seeds_len {
            return Err(CodeError::Damaged);
        }

        let mut seeds = [Color::BLACK; SEEDS.len()];

        for (seed, &[r, g, b]) in seeds.iter_mut().zip(bytes[..seeds_len].as_chunks::<3>().0) {
            *seed = Color::rgb(r, g, b);
        }

        let mut overrides = BTreeMap::new();
        let mut rest = &bytes[seeds_len..];
        let mut last = None;

        while let [head, r, g, b, tail @ ..] = rest {
            let index = (head & 0x7F) as usize;

            if index >= TOKENS.len() || last.is_some_and(|l| index <= l) {
                return Err(CodeError::Damaged);
            }

            let mut color = Color::rgb(*r, *g, *b);
            rest = tail;

            if head & 0x80 != 0 {
                let [hi, lo, tail @ ..] = rest else { return Err(CodeError::Damaged) };
                color.a = u16::from_be_bytes([*hi, *lo]);

                if color.a >= OPAQUE {
                    return Err(CodeError::Damaged);
                }

                rest = tail;
            }

            overrides.insert(index, color);
            last = Some(index);
        }

        if !rest.is_empty() {
            return Err(CodeError::Damaged);
        }

        Ok(Self { seeds, overrides })
    }
}

pub fn clamp_name(name: &str) -> String {
    name.trim().chars().take(NAME_MAX).collect::<String>().trim_end().to_string()
}

pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    pub scheme: Scheme,
}

fn builtin(id: &'static str, name: &'static str, seeds: [u32; 9], overrides: &[(&str, u32)]) -> Builtin {
    let overrides = overrides.iter().map(|&(t, c)| (token_index(t).expect("a token"), Color::hex(c))).collect();
    Builtin { id, name, scheme: Scheme { seeds: seeds.map(Color::hex), overrides } }
}

/// The schemes tinystream ships. Read-only; their codes never change.
pub fn builtins() -> [Builtin; 4] {
    let accents = [0xEB8A7A, 0x8FC79A, 0x7DD3FC, 0xFCD34D, 0xF9A8D4, 0xC4B5FD];

    let dark = |canvas: u32, ink: u32| {
        let [d, o, i, w, h, s] = accents;
        [canvas, ink, ink, d, o, i, w, h, s]
    };

    [
        builtin(
            "grey",
            "Grey",
            dark(0x191919, 0xEBEBEA),
            &[
                ("ink-2", 0x9B9B98),
                ("ink-3", 0x6E6E6B),
                ("accent-hover", 0xFFFFFF),
                ("info-deep", 0x38BDF8),
                ("warn-soft", 0xFDE68A),
                ("warn-deep", 0xFBBF24),
            ],
        ),
        builtin(
            "dark",
            "Dark",
            dark(0x0E0E0E, 0xE6E6E4),
            &[("accent-hover", 0xFFFFFF), ("info-deep", 0x38BDF8), ("warn-soft", 0xFDE68A), ("warn-deep", 0xFBBF24)],
        ),
        builtin(
            "oled",
            "OLED",
            dark(0x000000, 0xEBEBEA),
            &[("accent-hover", 0xFFFFFF), ("info-deep", 0x38BDF8), ("warn-soft", 0xFDE68A), ("warn-deep", 0xFBBF24)],
        ),
        builtin(
            "light",
            "Light",
            [0xF1F1EF, 0x1C1C1B, 0x1C1C1B, 0xC2412D, 0x2F8A46, 0x0B7FB8, 0xA86A00, 0xC0367E, 0x6D4FD6],
            &[("ink-3", 0x858583)],
        ),
    ]
}

pub fn find_builtin(id: &str) -> Option<Builtin> {
    builtins().into_iter().find(|b| b.id == id)
}

pub struct ContrastWarning {
    pub foreground: &'static str,
    pub background: &'static str,
    pub ratio: f64,
    pub minimum: f64,
}

/// Text needs 4.5:1 (WCAG AA); quieter text, icons and accents 3:1.
const PAIRS: [(&str, &str, f64); 11] = [
    ("ink", "canvas", 4.5),
    ("ink", "float", 4.5),
    ("ink-2", "canvas", 4.5),
    ("ink-3", "canvas", 3.0),
    ("on-accent", "accent", 4.5),
    ("danger", "canvas", 3.0),
    ("ok", "canvas", 3.0),
    ("info", "canvas", 3.0),
    ("warn", "canvas", 3.0),
    ("highlight", "canvas", 3.0),
    ("social", "canvas", 3.0),
];

/// Pairs that are hard to read. Only advice: nothing is refused for it.
pub fn contrast_warnings(tokens: &Tokens) -> Vec<ContrastWarning> {
    let get = |name| tokens[token_index(name).expect("a token")];

    PAIRS
        .iter()
        .filter_map(|&(fg, bg, minimum)| {
            let back = over(get(bg), get("canvas"));
            let ratio = contrast(over(get(fg), back), back);
            (ratio < minimum).then_some(ContrastWarning { foreground: fg, background: bg, ratio, minimum })
        })
        .collect()
}

/// `top` painted over an opaque `bottom`.
fn over(top: Color, bottom: Color) -> Color {
    let a = top.a as u32;
    let ch = |t: u8, b: u8| ((t as u32 * a + b as u32 * (1000 - a) + 500) / 1000) as u8;
    Color::rgb(ch(top.r, bottom.r), ch(top.g, bottom.g), ch(top.b, bottom.b))
}

fn luminance(c: Color) -> f64 {
    let lin = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };

    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

fn contrast(a: Color, b: Color) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey() -> Scheme {
        find_builtin("grey").unwrap().scheme
    }

    fn rendered(s: &Scheme) -> Vec<String> {
        TOKENS.iter().zip(s.tokens()).map(|(n, c)| format!("{n}: {c}")).collect()
    }

    /// Grey is the palette tinystream had before schemes, to the digit.
    #[test]
    fn grey_is_the_old_palette() {
        let code = "ts1.GRkZ6-vq6-vq64p6j8eafdP8_NNN-ajUxLX9BZubmAZubmsM____ETi9-BP95ooU-78k";

        assert_eq!(
            rendered(&Scheme::decode(code).unwrap().0),
            [
                "canvas: #191919",
                "raised: #202020",
                "panel: #262626",
                "float: #2e2e2e",
                "ink: #ebebea",
                "ink-2: #9b9b98",
                "ink-3: #6e6e6b",
                "line: rgb(255 255 255 / 0.075)",
                "line-strong: rgb(255 255 255 / 0.13)",
                "hover: rgb(255 255 255 / 0.055)",
                "press: rgb(255 255 255 / 0.09)",
                "accent: #ebebea",
                "accent-hover: #ffffff",
                "on-accent: #191919",
                "danger: #eb8a7a",
                "ok: #8fc79a",
                "info: #7dd3fc",
                "info-deep: #38bdf8",
                "warn: #fcd34d",
                "warn-soft: #fde68a",
                "warn-deep: #fbbf24",
                "highlight: #f9a8d4",
                "social: #c4b5fd",
                "focus: rgb(255 255 255 / 0.55)",
                "selection: rgb(255 255 255 / 0.18)",
                "scrollbar: rgb(255 255 255 / 0.12)",
                "glow: #ffffff",
                "shade: #000000",
            ]
        );
    }

    /// `ts1` is frozen: these codes must keep decoding to these colours forever.
    #[test]
    fn ts1_vectors() {
        let vectors: [(&str, &str); 5] = [
            ("grey", "ts1.GRkZ6-vq6-vq64p6j8eafdP8_NNN-ajUxLX9BZubmAZubmsM____ETi9-BP95ooU-78k"),
            ("dark", "ts1.Dg4O5ubk5ubk64p6j8eafdP8_NNN-ajUxLX9DP___xE4vfgT_eaKFPu_JA"),
            ("oled", "ts1.AAAA6-vq6-vq64p6j8eafdP8_NNN-ajUxLX9DP___xE4vfgT_eaKFPu_JA"),
            ("light", "ts1.8fHvHBwbHBwbwkEtL4pGC3-4qGoAwDZ-bU_WBoWFgw"),
            ("custom", "ts1.GRkZ6-vq6-vq64p6j8eafdP8_NNN-ajUxLX9hwAAAAAqDQAAAJr_ABIC6w"),
        ];

        let mut custom = grey();

        custom.overrides = BTreeMap::from([
            (token_index("line").unwrap(), Color::BLACK.alpha(42)),
            (token_index("on-accent").unwrap(), Color::rgb(0, 0, 0)),
            (token_index("glow").unwrap(), Color { r: 255, g: 0, b: 18, a: 747 }),
        ]);

        for (id, code) in vectors {
            let scheme = if id == "custom" { custom.clone() } else { find_builtin(id).unwrap().scheme };
            assert_eq!(scheme.code(), code, "{id}");
            let (decoded, name) = Scheme::decode(code).unwrap();
            assert_eq!(decoded, scheme, "{id}");
            assert_eq!(name, None);
            assert_eq!(decoded.code(), code, "{id}");
        }

        let tokens = Scheme::decode(vectors[4].1).unwrap().0.tokens();
        assert_eq!(tokens[token_index("line").unwrap()].to_string(), "rgb(0 0 0 / 0.042)");
        assert_eq!(tokens[token_index("glow").unwrap()].to_string(), "rgb(255 0 18 / 0.747)");

        assert_eq!(
            rendered(&Scheme::decode(vectors[3].1).unwrap().0)[..7],
            [
                "canvas: #f1f1ef",
                "raised: #f6f6f5",
                "panel: #fafaf9",
                "float: #ffffff",
                "ink: #1c1c1b",
                "ink-2: #6d6d6c",
                "ink-3: #858583",
            ]
        );
    }

    #[test]
    fn names_ride_along_but_are_not_identity() {
        let s = grey();
        let shared = s.share_code("Grey, mine");
        assert!(shared.starts_with(&s.code()));
        let (back, name) = Scheme::decode(&shared).unwrap();
        assert_eq!(back, s);
        assert_eq!(name.as_deref(), Some("Grey, mine"));
        assert_eq!(s.share_code("  "), s.code());
        let long = "ä".repeat(80);
        assert_eq!(Scheme::decode(&s.share_code(&long)).unwrap().1.unwrap().chars().count(), NAME_MAX);
    }

    #[test]
    fn rejects_what_is_not_canonical() {
        let code = grey().code();
        assert_eq!(Scheme::decode("hello"), Err(CodeError::NotACode));
        assert_eq!(Scheme::decode("ts2.AAAA"), Err(CodeError::Version("ts2".into())));
        assert_eq!(Scheme::decode("ts1.AAAA"), Err(CodeError::Damaged));
        assert_eq!(Scheme::decode(&format!("{code}=")), Err(CodeError::Damaged));
        assert_eq!(Scheme::decode(&format!("{code}.bmFtZQ.x")), Err(CodeError::Damaged));
        let mut bytes = URL_SAFE_NO_PAD.decode(&code[4..]).unwrap();

        let first = 27;
        bytes.swap(first, first + 4);
        assert!(Scheme::decode(&format!("ts1.{}", URL_SAFE_NO_PAD.encode(&bytes))).is_err());

        let mut bytes = URL_SAFE_NO_PAD.decode(&code[4..]).unwrap();
        bytes.extend([0x80 | 27, 0, 0, 0, 0x03, 0xE8]);
        assert!(Scheme::decode(&format!("ts1.{}", URL_SAFE_NO_PAD.encode(&bytes))).is_err());
    }

    #[test]
    fn parses_what_it_prints() {
        for s in ["#00ff7f", "rgb(1 2 3 / 0.5)", "rgb(255 255 255 / 0.055)", "rgb(0 0 0 / 0)"] {
            assert_eq!(Color::parse(s).unwrap().to_string(), s);
        }

        assert_eq!(Color::parse("rgb(0 0 0 / 1)"), Some(Color::BLACK));

        for s in ["#fff", "#gggggg", "rgb(0 0 / 0.5)", "rgb(0 0 0 / 0.0555)", "rgb(0 0 0 / 1.5)", "red"] {
            assert_eq!(Color::parse(s), None, "{s}");
        }
    }

    #[test]
    fn built_ins_read_well() {
        for b in builtins() {
            let warnings = contrast_warnings(&b.scheme.tokens());
            assert!(warnings.is_empty(), "{}: {:?}", b.id, warnings.iter().map(|w| w.foreground).collect::<Vec<_>>());
        }
    }
}
