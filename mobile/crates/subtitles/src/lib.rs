// SPDX-License-Identifier: AGPL-3.0-or-later
//! The video player's subtitles (dev.tinystream.player.Ass): libass renders
//! a script at a time, and what it draws is blended into an Android bitmap
//! covering just the part of the frame that has anything on it.

use std::ffi::{CString, c_char, c_int};
use std::ptr;

use jni::JNIEnv;
use jni::objects::{JByteArray, JObject, JObjectArray};
use jni::sys::{jboolean, jint, jintArray, jlong};
use libass_src as _;

#[repr(C)]
struct AssLibrary {
    _private: [u8; 0],
}

#[repr(C)]
struct AssRenderer {
    _private: [u8; 0],
}

#[repr(C)]
struct AssTrack {
    _private: [u8; 0],
}

#[repr(C)]
struct AssImage {
    w: c_int,
    h: c_int,
    stride: c_int,
    bitmap: *const u8,
    /// RGBA, the alpha being transparency.
    color: u32,
    dst_x: c_int,
    dst_y: c_int,
    next: *const AssImage,
    kind: c_int,
}

const ASS_FONTPROVIDER_NONE: c_int = 0;

unsafe extern "C" {
    fn ass_library_init() -> *mut AssLibrary;
    fn ass_library_done(lib: *mut AssLibrary);
    fn ass_set_extract_fonts(lib: *mut AssLibrary, extract: c_int);
    fn ass_add_font(lib: *mut AssLibrary, name: *const c_char, data: *const c_char, size: c_int);
    fn ass_renderer_init(lib: *mut AssLibrary) -> *mut AssRenderer;
    fn ass_renderer_done(renderer: *mut AssRenderer);
    fn ass_set_frame_size(renderer: *mut AssRenderer, w: c_int, h: c_int);
    fn ass_set_storage_size(renderer: *mut AssRenderer, w: c_int, h: c_int);
    fn ass_set_margins(renderer: *mut AssRenderer, t: c_int, b: c_int, l: c_int, r: c_int);
    fn ass_set_use_margins(renderer: *mut AssRenderer, on: c_int);
    fn ass_set_fonts(
        renderer: *mut AssRenderer,
        default_font: *const c_char,
        default_family: *const c_char,
        provider: c_int,
        config: *const c_char,
        update: c_int,
    );
    fn ass_read_memory(lib: *mut AssLibrary, buf: *mut c_char, size: usize, codepage: *const c_char) -> *mut AssTrack;
    fn ass_free_track(track: *mut AssTrack);
    fn ass_render_frame(
        renderer: *mut AssRenderer,
        track: *mut AssTrack,
        now: i64,
        changed: *mut c_int,
    ) -> *const AssImage;
}

/// The family of the font every script falls back on.
const DEFAULT_FAMILY: &str = "Liberation Sans";

struct Subtitles {
    lib: *mut AssLibrary,
    renderer: *mut AssRenderer,
    track: *mut AssTrack,
    frame: (c_int, c_int),
    /// What the last render drew, and the box around it.
    images: *const AssImage,
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
    fn open(script: &[u8], fonts: &[Vec<u8>], default: &[u8]) -> Option<Box<Subtitles>> {
        unsafe {
            let lib = ass_library_init();

            if lib.is_null() {
                return None;
            }

            let mut this = Box::new(Subtitles {
                lib,
                renderer: ptr::null_mut(),
                track: ptr::null_mut(),
                frame: (0, 0),
                images: ptr::null(),
                area: Area::default(),
            });

            ass_set_extract_fonts(lib, 1);

            for (i, font) in std::iter::once(default).chain(fonts.iter().map(Vec::as_slice)).enumerate() {
                let name = CString::new(format!("font{i}")).unwrap();
                ass_add_font(lib, name.as_ptr(), font.as_ptr().cast(), font.len() as c_int);
            }

            this.renderer = ass_renderer_init(lib);

            if this.renderer.is_null() {
                return None;
            }

            let family = CString::new(DEFAULT_FAMILY).unwrap();
            ass_set_fonts(this.renderer, ptr::null(), family.as_ptr(), ASS_FONTPROVIDER_NONE, ptr::null(), 0);

            let mut buf = script.to_vec();
            this.track = ass_read_memory(lib, buf.as_mut_ptr().cast(), buf.len(), ptr::null());

            if this.track.is_null() {
                return None;
            }

            Some(this)
        }
    }

    /// Lays the frame out: `frame` is what's drawn into (the view), `storage` the video's own size,
    /// and `margins` (top, bottom, left, right) how far the picture is inside the frame:
    /// negative when cropped, in which case dialogue is kept in what's visible.
    fn resize(&mut self, frame: (c_int, c_int), storage: (c_int, c_int), (t, b, l, r): (c_int, c_int, c_int, c_int)) {
        self.frame = frame;
        self.images = ptr::null();

        unsafe {
            ass_set_frame_size(self.renderer, frame.0, frame.1);
            ass_set_storage_size(self.renderer, storage.0, storage.1);
            ass_set_margins(self.renderer, t, b, l, r);
            ass_set_use_margins(self.renderer, (t < 0 || b < 0 || l < 0 || r < 0) as c_int);
        }
    }

    /// What's showing at `ms`, or None when that's what showed last time.
    fn render(&mut self, ms: i64, force: bool) -> Option<Area> {
        if self.frame.0 <= 0 || self.frame.1 <= 0 {
            return None;
        }

        let mut changed = 0;
        self.images = unsafe { ass_render_frame(self.renderer, self.track, ms, &mut changed) };

        if changed == 0 && !force {
            return None;
        }

        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);

        for i in self.iter() {
            x0 = x0.min(i.dst_x.max(0));
            y0 = y0.min(i.dst_y.max(0));
            x1 = x1.max((i.dst_x + i.w).min(self.frame.0));
            y1 = y1.max((i.dst_y + i.h).min(self.frame.1));
        }

        self.area = if x0 < x1 && y0 < y1 { Area { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } } else { Area::default() };
        Some(self.area)
    }

    fn iter(&self) -> impl Iterator<Item = &AssImage> {
        let mut next = self.images;

        std::iter::from_fn(move || {
            let image = unsafe { next.as_ref()? };
            next = image.next;
            Some(image)
        })
        .filter(|i| i.w > 0 && i.h > 0)
    }

    /// Paints the last render into `pixels` (premultiplied RGBA, `stride` bytes a row), which
    /// covers its area.
    #[cfg(target_os = "android")]
    fn paint(&self, pixels: &mut [u8], (w, h): (i32, i32), stride: usize) {
        for row in 0..h as usize {
            pixels[row * stride..row * stride + w as usize * 4].fill(0);
        }

        for i in self.iter() {
            let mask = unsafe { std::slice::from_raw_parts(i.bitmap, (i.stride * (i.h - 1) + i.w) as usize) };
            blend(
                pixels,
                (w, h),
                stride,
                i.color,
                mask,
                i.stride as usize,
                (i.dst_x - self.area.x, i.dst_y - self.area.y),
                (i.w, i.h),
            );
        }
    }
}

/// Blends one of libass's masks, all in `color`, over premultiplied RGBA pixels at `at`.
#[allow(clippy::too_many_arguments)]
fn blend(
    pixels: &mut [u8],
    (w, h): (i32, i32),
    stride: usize,
    color: u32,
    mask: &[u8],
    pitch: usize,
    at: (i32, i32),
    (mw, mh): (i32, i32),
) {
    let (r, g, b) = ((color >> 24) as u32, (color >> 16 & 0xFF) as u32, (color >> 8 & 0xFF) as u32);
    let opacity = 255 - (color & 0xFF);

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

impl Drop for Subtitles {
    fn drop(&mut self) {
        unsafe {
            if !self.track.is_null() {
                ass_free_track(self.track);
            }

            if !self.renderer.is_null() {
                ass_renderer_done(self.renderer);
            }

            ass_library_done(self.lib);
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
    use super::blend;

    #[test]
    fn blends_premultiplied() {
        let mut px = vec![0u8; 2 * 4];
        // White at half transparency, through a full and an empty mask pixel.
        blend(&mut px, (2, 1), 8, 0xFFFFFF80, &[255, 0], 2, (0, 0), (2, 1));
        assert_eq!(&px[..4], &[127, 127, 127, 127]);
        assert_eq!(&px[4..], &[0, 0, 0, 0]);

        // Opaque red over that.
        blend(&mut px, (2, 1), 8, 0xFF000000, &[255, 0], 2, (0, 0), (2, 1));
        assert_eq!(&px[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn clips_to_the_bitmap() {
        let mut px = vec![0u8; 4];
        blend(&mut px, (1, 1), 4, 0x00FF0000, &[255, 255, 255, 255], 2, (-1, -1), (2, 2));
        assert_eq!(px, [0, 255, 0, 255]);
    }
}
