// SPDX-License-Identifier: AGPL-3.0-or-later
//! The video player's subtitles (dev.tinystream.player.Ass): rustyass renders
//! a script at a time, and what it draws is blended into an Android bitmap
//! covering just the part of the frame that has anything on it.

use std::ffi::c_int;
use std::ptr;

use jni::JNIEnv;
use jni::objects::{JByteArray, JObject, JObjectArray};
use jni::sys::{jboolean, jint, jintArray, jlong};
use rustyass::{Fonts, Margins, Renderer, Rgba, Track};

struct Subtitles {
    renderer: Renderer,
    track: Track,
    frame: (c_int, c_int),
    /// The box around what the last render drew.
    area: Area,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Area {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Subtitles {
    /// `script` with the video's `fonts` and its own, falling back on `default`.
    fn open(script: &[u8], fonts: &[Vec<u8>], default: &[u8]) -> Option<Box<Subtitles>> {
        let track = Track::parse(script)?;
        let mut all = Fonts::new();
        all.set_default(default);

        for font in fonts {
            all.add(font.as_slice());
        }

        for (_, data) in track.fonts() {
            all.add(data);
        }

        Some(Box::new(Subtitles { renderer: Renderer::new(all), track, frame: (0, 0), area: Area::default() }))
    }

    /// Lays the frame out: `frame` is what's drawn into (the view), `storage` the video's own size,
    /// and `margins` (top, bottom, left, right) how far the picture is inside the frame:
    /// negative when cropped, in which case dialogue is kept in what's visible.
    fn resize(
        &mut self,
        frame: (c_int, c_int),
        storage: (c_int, c_int),
        (top, bottom, left, right): (c_int, c_int, c_int, c_int),
    ) {
        self.frame = frame;
        self.renderer.set_frame(frame, storage, Margins { top, bottom, left, right });
    }

    /// What's showing at `ms`, or None when that's what showed last time.
    fn render(&mut self, ms: i64, force: bool) -> Option<Area> {
        if self.frame.0 <= 0 || self.frame.1 <= 0 {
            return None;
        }

        let r = self.renderer.render(&self.track, ms);

        if !r.changed && !force {
            return None;
        }

        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);

        for i in r.images {
            x0 = x0.min(i.x.max(0));
            y0 = y0.min(i.y.max(0));
            x1 = x1.max((i.x + i.w).min(self.frame.0));
            y1 = y1.max((i.y + i.h).min(self.frame.1));
        }

        self.area = if x0 < x1 && y0 < y1 { Area { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } } else { Area::default() };
        Some(self.area)
    }

    /// Paints the last render into `pixels` (premultiplied RGBA, `stride` bytes a row), which
    /// covers its area.
    #[cfg(target_os = "android")]
    fn paint(&self, pixels: &mut [u8], (w, h): (i32, i32), stride: usize) {
        for row in 0..h as usize {
            pixels[row * stride..row * stride + w as usize * 4].fill(0);
        }

        for i in self.renderer.images() {
            blend(
                pixels,
                (w, h),
                stride,
                i.color(),
                i.mask(),
                i.stride(),
                (i.x - self.area.x, i.y - self.area.y),
                (i.w, i.h),
            );
        }
    }
}

/// Blends one of rustyass's masks, all in `color`, over premultiplied RGBA pixels at `at`.
#[allow(clippy::too_many_arguments)]
fn blend(
    pixels: &mut [u8],
    (w, h): (i32, i32),
    stride: usize,
    color: Rgba,
    mask: &[u8],
    pitch: usize,
    at: (i32, i32),
    (mw, mh): (i32, i32),
) {
    let (r, g, b, opacity) = (color.r as u32, color.g as u32, color.b as u32, color.a as u32);

    if opacity == 0 {
        return;
    }

    let (xa, xb) = (at.0.max(0), (at.0 + mw).min(w));
    let (ya, yb) = (at.1.max(0), (at.1 + mh).min(h));

    for y in ya..yb {
        let src = &mask[(y - at.1) as usize * pitch..];
        let dst = &mut pixels[y as usize * stride..];

        for x in xa..xb {
            let a = (src[(x - at.0) as usize] as u32 * opacity + 127) / 255;

            if a == 0 {
                continue;
            }

            let p = &mut dst[x as usize * 4..x as usize * 4 + 4];
            let keep = 255 - a;
            p[0] = ((r * a + p[0] as u32 * keep + 127) / 255) as u8;
            p[1] = ((g * a + p[1] as u32 * keep + 127) / 255) as u8;
            p[2] = ((b * a + p[2] as u32 * keep + 127) / 255) as u8;
            p[3] = (a + (p[3] as u32 * keep + 127) / 255) as u8;
        }
    }
}

fn subtitles<'a>(handle: jlong) -> &'a mut Subtitles {
    unsafe { &mut *(handle as *mut Subtitles) }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tinystream_player_Ass_open<'l>(
    mut env: JNIEnv<'l>,
    _: JObject<'l>,
    script: JByteArray<'l>,
    fonts: JObjectArray<'l>,
    default: JByteArray<'l>,
) -> jlong {
    let mut read = || -> jni::errors::Result<_> {
        let script = env.convert_byte_array(&script)?;
        let default = env.convert_byte_array(&default)?;
        let mut all = Vec::new();

        for i in 0..env.get_array_length(&fonts)? {
            let font = JByteArray::from(env.get_object_array_element(&fonts, i)?);
            all.push(env.convert_byte_array(&font)?);
        }

        Ok((script, all, default))
    };

    match read() {
        Ok((script, fonts, default)) => {
            Subtitles::open(&script, &fonts, &default).map_or(0, |s| Box::into_raw(s) as jlong)
        },
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tinystream_player_Ass_close(_: JNIEnv, _: JObject, handle: jlong) {
    if handle != 0 {
        drop(unsafe { Box::from_raw(handle as *mut Subtitles) });
    }
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_dev_tinystream_player_Ass_resize(
    _: JNIEnv,
    _: JObject,
    handle: jlong,
    width: jint,
    height: jint,
    storage_width: jint,
    storage_height: jint,
    top: jint,
    bottom: jint,
    left: jint,
    right: jint,
) {
    subtitles(handle).resize((width, height), (storage_width, storage_height), (top, bottom, left, right));
}

/// `[x, y, width, height]` of what shows at `ms`, or null when nothing changed since the last
/// render.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tinystream_player_Ass_render<'l>(
    env: JNIEnv<'l>,
    _: JObject<'l>,
    handle: jlong,
    ms: jlong,
    force: jboolean,
) -> jintArray {
    let Some(a) = subtitles(handle).render(ms, force != 0) else {
        return ptr::null_mut();
    };

    let Ok(out) = env.new_int_array(4) else {
        return ptr::null_mut();
    };

    let _ = env.set_int_array_region(&out, 0, &[a.x, a.y, a.w, a.h]);
    out.into_raw()
}

#[cfg(target_os = "android")]
#[repr(C)]
#[derive(Default)]
struct AndroidBitmapInfo {
    width: u32,
    height: u32,
    stride: u32,
    format: i32,
    flags: u32,
}

#[cfg(target_os = "android")]
const ANDROID_BITMAP_FORMAT_RGBA_8888: i32 = 1;

#[cfg(target_os = "android")]
#[link(name = "jnigraphics")]
unsafe extern "C" {
    fn AndroidBitmap_getInfo(
        env: *mut jni::sys::JNIEnv,
        bitmap: jni::sys::jobject,
        info: *mut AndroidBitmapInfo,
    ) -> c_int;
    fn AndroidBitmap_lockPixels(
        env: *mut jni::sys::JNIEnv,
        bitmap: jni::sys::jobject,
        pixels: *mut *mut std::ffi::c_void,
    ) -> c_int;
    fn AndroidBitmap_unlockPixels(env: *mut jni::sys::JNIEnv, bitmap: jni::sys::jobject) -> c_int;
}

/// Paints the last render into `bitmap` (ARGB_8888, the size of its area).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_tinystream_player_Ass_paint<'l>(
    env: JNIEnv<'l>,
    _: JObject<'l>,
    handle: jlong,
    bitmap: JObject<'l>,
) -> jboolean {
    let s = subtitles(handle);
    let (raw, bitmap) = (env.get_raw(), bitmap.as_raw());
    let mut info = AndroidBitmapInfo::default();
    let mut pixels: *mut std::ffi::c_void = ptr::null_mut();

    unsafe {
        if AndroidBitmap_getInfo(raw, bitmap, &mut info) != 0 || info.format != ANDROID_BITMAP_FORMAT_RGBA_8888 {
            return 0;
        }

        if AndroidBitmap_lockPixels(raw, bitmap, &mut pixels) != 0 || pixels.is_null() {
            return 0;
        }

        let (w, h) = ((info.width as i32).min(s.area.w), (info.height as i32).min(s.area.h));
        let len = info.stride as usize * info.height as usize;
        s.paint(std::slice::from_raw_parts_mut(pixels.cast(), len), (w, h), info.stride as usize);
        AndroidBitmap_unlockPixels(raw, bitmap);
    }

    1
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 127 };
    const RED: Rgba = Rgba { r: 255, g: 0, b: 0, a: 255 };
    const GREEN: Rgba = Rgba { r: 0, g: 255, b: 0, a: 255 };

    const SCRIPT: &[u8] = b"[Script Info]\nScriptType: v4.00+\nPlayResX: 640\nPlayResY: 360\n\n[V4+ Styles]\n\
        Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, \
        Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, \
        MarginR, MarginV, Encoding\n\
        Style: Default,Liberation Sans,40,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,1,2,10,10,20,1\n\n\
        [Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
        Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n";

    #[test]
    fn blends_premultiplied() {
        let mut px = vec![0u8; 2 * 4];
        // White at half transparency, through a full and an empty mask pixel.
        blend(&mut px, (2, 1), 8, WHITE, &[255, 0], 2, (0, 0), (2, 1));
        assert_eq!(&px[..4], &[127, 127, 127, 127]);
        assert_eq!(&px[4..], &[0, 0, 0, 0]);

        // Opaque red over that.
        blend(&mut px, (2, 1), 8, RED, &[255, 0], 2, (0, 0), (2, 1));
        assert_eq!(&px[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn clips_to_the_bitmap() {
        let mut px = vec![0u8; 4];
        blend(&mut px, (1, 1), 4, GREEN, &[255, 255, 255, 255], 2, (-1, -1), (2, 2));
        assert_eq!(px, [0, 255, 0, 255]);
    }

    #[test]
    fn renders_only_what_changed() {
        let font =
            include_bytes!("../../../modules/player/android/src/main/assets/subtitles/LiberationSans-Regular.ttf");
        let mut s = Subtitles::open(SCRIPT, &[], font).unwrap();
        assert!(s.render(1500, false).is_none());

        s.resize((640, 360), (640, 360), (0, 0, 0, 0));
        let a = s.render(1500, false).unwrap();
        assert!(a.w > 0 && a.h > 0 && a.y + a.h <= 360);
        assert!(s.render(1600, false).is_none());
        assert_eq!(s.render(1600, true), Some(a));
        assert_eq!(s.render(2500, false), Some(Area::default()));
    }
}
