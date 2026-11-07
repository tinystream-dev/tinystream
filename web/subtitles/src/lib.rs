// SPDX-License-Identifier: AGPL-3.0-or-later
//! The web player's thin WASM interface to the shared subtitle renderer.

use tinystream_subtitle_renderer as renderer;
use wasm_bindgen::Clamped;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Subtitles(renderer::Subtitles);

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
    pub fn pixels(&mut self) -> Clamped<Vec<u8>> {
        Clamped(std::mem::take(&mut self.pixels))
    }
}

#[wasm_bindgen]
impl Subtitles {
    #[wasm_bindgen(constructor)]
    pub fn new(font: &[u8]) -> Self {
        Self(renderer::Subtitles::new(font))
    }

    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, data: &[u8]) {
        self.0.add_font(data);
    }

    #[wasm_bindgen(js_name = setTrack)]
    pub fn set_track(&mut self, script: &[u8]) -> Result<(), JsError> {
        self.0.set_track(script).map_err(JsError::new)
    }

    #[wasm_bindgen(js_name = freeTrack)]
    pub fn free_track(&mut self) {
        self.0.free_track();
    }

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
        self.0.resize(width, height, storage_width, storage_height, top, bottom, left, right);
    }

    pub fn render(&mut self, ms: f64, force: bool) -> Option<Frame> {
        self.0.render(ms, force).map(|frame| Frame {
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels,
        })
    }
}
