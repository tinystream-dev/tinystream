// SPDX-License-Identifier: AGPL-3.0-or-later
//! The video player's subtitles: rustyass renders a script at a time, and what it draws is
//! blended into RGBA covering just the part of the frame that has anything on it.

use rustyass::{Fonts, Image, Margins, Renderer, Track};
use wasm_bindgen::Clamped;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Subtitles {
    /// The fonts every script gets; a script's own are added to these.
    fonts: Fonts,
    renderer: Renderer,
    track: Option<Track>,
    frame: (i32, i32),
    storage: (i32, i32),
    margins: Margins,
}

/// What a frame shows: RGBA pixels covering `width` × `height` at `(x, y)`, or nothing at all
/// when `width` is 0.
#[wasm_bindgen]
pub struct Frame {
    #[wasm_bindgen(readonly)]
    pub x: i32,
    #[wasm_bindgen(readonly)]
    pub y: i32,
    #[wasm_bindgen(readonly)]
    pub width: u32,
    #[wasm_bindgen(readonly)]
    pub height: u32,
    pixels: Vec<u8>,
}

#[wasm_bindgen]
impl Frame {
    /// The pixels, not premultiplied (as `ImageData` has them), taken out of the frame.
    pub fn pixels(&mut self) -> Clamped<Vec<u8>> {
        Clamped(std::mem::take(&mut self.pixels))
    }
}

#[derive(Clone, Copy, Default)]
struct Area {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[wasm_bindgen]
impl Subtitles {
    /// `font` is the default, for whatever no other font has.
    #[wasm_bindgen(constructor)]
    pub fn new(font: &[u8]) -> Subtitles {
        let mut fonts = Fonts::new();
        fonts.set_default(font);

        Subtitles {
            renderer: Renderer::new(fonts.clone()),
            fonts,
            track: None,
            frame: (0, 0),
            storage: (0, 0),
            margins: Margins::default(),
        }
    }

    /// Adds a font (one of the file's attachments) for every script.
    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, data: &[u8]) {
        self.fonts.add(data);
        self.renderer.fonts_mut().add(data);
    }

    /// Reads a script, with the fonts in its `[Fonts]` section, to render from now on.
    #[wasm_bindgen(js_name = setTrack)]
    pub fn set_track(&mut self, script: &[u8]) -> Result<(), JsError> {
        let track = Track::parse(script).ok_or_else(|| JsError::new("not an ASS or SSA script"))?;
        let mut fonts = self.fonts.clone();

        for (_, data) in track.fonts() {
            fonts.add(data);
        }

        self.renderer = Renderer::new(fonts);
        self.renderer.set_frame(self.frame, self.storage, self.margins);
        self.track = Some(track);
        Ok(())
    }

    #[wasm_bindgen(js_name = freeTrack)]
    pub fn free_track(&mut self) {
        self.track = None;
    }

    /// Lays frames out: `width` × `height` is what's drawn into, the storage size the video's own,
    /// and the margins how far the video is inside the frame (negative where it's cropped).
    #[allow(clippy::too_many_arguments)]
    pub fn resize(
        &mut self,
        width: i32,
        height: i32,
        storage_width: i32,
        storage_height: i32,
        top: i32,
        bottom: i32,
        left: i32,
        right: i32,
    ) {
        self.frame = (width, height);
        self.storage = (storage_width, storage_height);
        self.margins = Margins { top, bottom, left, right };
        self.renderer.set_frame(self.frame, self.storage, self.margins);
    }

    /// What's showing at `ms`, or nothing when that's what showed last time (unless `force`).
    pub fn render(&mut self, ms: f64, force: bool) -> Option<Frame> {
        let images: &[Image] = match &self.track {
            Some(track) => {
                let r = self.renderer.render(track, ms as i64);

                if !r.changed && !force {
                    return None;
                }

                r.images
            },
            None if force => &[],
            None => return None,
        };

        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);

        for i in images {
            x0 = x0.min(i.x.max(0));
            y0 = y0.min(i.y.max(0));
            x1 = x1.max((i.x + i.w).min(self.frame.0));
            y1 = y1.max((i.y + i.h).min(self.frame.1));
        }

        let area = if x0 < x1 && y0 < y1 { Area { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } } else { Area::default() };
        let mut pixels = vec![0; area.w as usize * area.h as usize * 4];

        for i in images {
            blend(&mut pixels, area, i);
        }

        unpremultiply(&mut pixels);

        Some(Frame { x: area.x, y: area.y, width: area.w as u32, height: area.h as u32, pixels })
    }
}

/// Blends one of rustyass's masks, all in its colour, over premultiplied RGBA covering `area`.
fn blend(pixels: &mut [u8], area: Area, image: &Image) {
    let c = image.color();
    let (r, g, b, opacity) = (c.r as u32, c.g as u32, c.b as u32, c.a as u32);

    if opacity == 0 {
        return;
    }

    let (xa, xb) = (image.x.max(area.x), (image.x + image.w).min(area.x + area.w));
    let (ya, yb) = (image.y.max(area.y), (image.y + image.h).min(area.y + area.h));

    if xa >= xb {
        return;
    }

    for y in ya..yb {
        let src = &image.row((y - image.y) as usize)[(xa - image.x) as usize..(xb - image.x) as usize];
        let at = ((y - area.y) * area.w + xa - area.x) as usize * 4;
        let dst = &mut pixels[at..at + src.len() * 4];

        for (&m, p) in src.iter().zip(dst.as_chunks_mut::<4>().0) {
            let a = (m as u32 * opacity + 127) / 255;

            if a == 0 {
                continue;
            }

            let keep = 255 - a;
            p[0] = ((r * a + p[0] as u32 * keep + 127) / 255) as u8;
            p[1] = ((g * a + p[1] as u32 * keep + 127) / 255) as u8;
            p[2] = ((b * a + p[2] as u32 * keep + 127) / 255) as u8;
            p[3] = (a + (p[3] as u32 * keep + 127) / 255) as u8;
        }
    }
}

/// Divides the colour by the alpha, as `ImageData` wants it; the canvas multiplies it back.
fn unpremultiply(pixels: &mut [u8]) {
    for p in pixels.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;

        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &[u8] = b"[Script Info]\nScriptType: v4.00+\nPlayResX: 640\nPlayResY: 360\n\n[V4+ Styles]\n\
        Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, \
        Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, \
        MarginR, MarginV, Encoding\n\
        Style: Default,Noto Sans,40,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,1,2,10,10,20,1\n\n\
        [Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
        Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n";

    fn subtitles() -> Subtitles {
        let mut s = Subtitles::new(include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf"));
        s.set_track(SCRIPT).unwrap();
        s.resize(640, 360, 640, 360, 0, 0, 0, 0);
        s
    }

    #[test]
    fn draws_only_what_changed() {
        let mut s = subtitles();
        let f = s.render(1500.0, false).unwrap();
        assert!(f.width > 0 && f.height > 0);
        assert!(f.y as u32 + f.height <= 360);
        assert_eq!(f.pixels.len(), f.width as usize * f.height as usize * 4);
        assert!(f.pixels.chunks_exact(4).any(|p| p == [255, 255, 255, 255]));
        assert!(s.render(1600.0, false).is_none());
        assert!(s.render(1600.0, true).is_some());
        assert_eq!(s.render(2500.0, false).unwrap().width, 0);
    }

    #[test]
    fn clears_when_the_track_goes() {
        let mut s = subtitles();
        s.render(1500.0, false).unwrap();
        s.free_track();
        assert!(s.render(1500.0, false).is_none());
        assert_eq!(s.render(1500.0, true).unwrap().width, 0);
    }

    #[test]
    fn unpremultiplies() {
        let mut p = [64, 32, 0, 128, 10, 20, 30, 255, 0, 0, 0, 0];
        unpremultiply(&mut p);
        assert_eq!(p, [128, 64, 0, 128, 10, 20, 30, 255, 0, 0, 0, 0]);
    }
}
