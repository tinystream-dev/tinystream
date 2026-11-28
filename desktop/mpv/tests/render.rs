// SPDX-License-Identifier: AGPL-3.0-or-later

//! Plays a short file through the render API into an offscreen OpenGL framebuffer, on EGL's
//! surfaceless platform (Mesa's, so llvmpipe works without a GPU).

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr;
use std::time::{Duration, Instant};

use mpv::{EndReason, Event, GlInit, Mpv, RenderContext, Stream};

const CLIP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/testsrc.mp4");
const RENDER_NODE: &str = "/dev/dri/renderD128";

const EGL_PLATFORM_SURFACELESS_MESA: c_uint = 0x31DD;
const EGL_NONE: c_int = 0x3038;
const EGL_SURFACE_TYPE: c_int = 0x3033;
const EGL_PBUFFER_BIT: c_int = 0x0001;
const EGL_RENDERABLE_TYPE: c_int = 0x3040;
const EGL_OPENGL_BIT: c_int = 0x0008;
const EGL_OPENGL_API: c_uint = 0x30A2;

const GL_FRAMEBUFFER: c_uint = 0x8D40;
const GL_FRAMEBUFFER_COMPLETE: c_uint = 0x8CD5;
const GL_COLOR_ATTACHMENT0: c_uint = 0x8CE0;
const GL_TEXTURE_2D: c_uint = 0x0DE1;
const GL_RGBA8: c_int = 0x8058;
const GL_RGBA: c_uint = 0x1908;
const GL_UNSIGNED_BYTE: c_uint = 0x1401;

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetPlatformDisplay(platform: c_uint, native: *mut c_void, attribs: *const isize) -> *mut c_void;
    fn eglInitialize(display: *mut c_void, major: *mut c_int, minor: *mut c_int) -> c_uint;
    fn eglBindAPI(api: c_uint) -> c_uint;
    fn eglChooseConfig(
        display: *mut c_void,
        attribs: *const c_int,
        configs: *mut *mut c_void,
        size: c_int,
        count: *mut c_int,
    ) -> c_uint;
    fn eglCreateContext(
        display: *mut c_void,
        config: *mut c_void,
        share: *mut c_void,
        attribs: *const c_int,
    ) -> *mut c_void;
    fn eglMakeCurrent(display: *mut c_void, draw: *mut c_void, read: *mut c_void, context: *mut c_void) -> c_uint;
    fn eglDestroyContext(display: *mut c_void, context: *mut c_void) -> c_uint;
    fn eglTerminate(display: *mut c_void) -> c_uint;
    fn eglGetProcAddress(name: *const c_char) -> *mut c_void;
}

/// An OpenGL context with no surface, current on the thread that made it.
struct Egl {
    display: *mut c_void,
    context: *mut c_void,
}

impl Egl {
    fn new() -> Egl {
        unsafe {
            let display = eglGetPlatformDisplay(EGL_PLATFORM_SURFACELESS_MESA, ptr::null_mut(), ptr::null());
            assert!(!display.is_null(), "no surfaceless EGL display");
            assert_eq!(eglInitialize(display, ptr::null_mut(), ptr::null_mut()), 1, "can't initialize EGL");
            assert_eq!(eglBindAPI(EGL_OPENGL_API), 1, "no OpenGL in EGL");

            let attribs = [EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT, EGL_NONE];
            let (mut config, mut count) = (ptr::null_mut(), 0);
            eglChooseConfig(display, attribs.as_ptr(), &mut config, 1, &mut count);
            assert_eq!(count, 1, "no EGL config for OpenGL");

            let context = eglCreateContext(display, config, ptr::null_mut(), [EGL_NONE].as_ptr());
            assert!(!context.is_null(), "can't create an OpenGL context");
            assert_eq!(eglMakeCurrent(display, ptr::null_mut(), ptr::null_mut(), context), 1);

            Egl { display, context }
        }
    }

    fn proc<T>(&self, name: &CStr) -> T {
        let address = unsafe { eglGetProcAddress(name.as_ptr()) };
        assert!(!address.is_null(), "no {name:?}");
        unsafe { std::mem::transmute_copy(&address) }
    }
}

impl Drop for Egl {
    fn drop(&mut self) {
        unsafe {
            eglMakeCurrent(self.display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            eglDestroyContext(self.display, self.context);
            eglTerminate(self.display);
        }
    }
}

/// A `width` by `height` RGBA framebuffer to render into, and a way to read it back.
struct Target {
    fbo: c_uint,
    width: c_int,
    height: c_int,
    bind_framebuffer: unsafe extern "C" fn(c_uint, c_uint),
    read_pixels: unsafe extern "C" fn(c_int, c_int, c_int, c_int, c_uint, c_uint, *mut c_void),
}

impl Target {
    fn new(egl: &Egl, width: c_int, height: c_int) -> Target {
        let gen_framebuffers: unsafe extern "C" fn(c_int, *mut c_uint) = egl.proc(c"glGenFramebuffers");
        let bind_framebuffer: unsafe extern "C" fn(c_uint, c_uint) = egl.proc(c"glBindFramebuffer");
        let gen_textures: unsafe extern "C" fn(c_int, *mut c_uint) = egl.proc(c"glGenTextures");
        let bind_texture: unsafe extern "C" fn(c_uint, c_uint) = egl.proc(c"glBindTexture");
        let tex_image_2d: unsafe extern "C" fn(
            c_uint,
            c_int,
            c_int,
            c_int,
            c_int,
            c_int,
            c_uint,
            c_uint,
            *const c_void,
        ) = egl.proc(c"glTexImage2D");
        let framebuffer_texture_2d: unsafe extern "C" fn(c_uint, c_uint, c_uint, c_uint, c_int) =
            egl.proc(c"glFramebufferTexture2D");
        let check_framebuffer_status: unsafe extern "C" fn(c_uint) -> c_uint = egl.proc(c"glCheckFramebufferStatus");

        let (mut fbo, mut texture) = (0, 0);

        unsafe {
            gen_textures(1, &mut texture);
            bind_texture(GL_TEXTURE_2D, texture);
            tex_image_2d(GL_TEXTURE_2D, 0, GL_RGBA8, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, ptr::null());
            gen_framebuffers(1, &mut fbo);
            bind_framebuffer(GL_FRAMEBUFFER, fbo);
            framebuffer_texture_2d(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, texture, 0);
            assert_eq!(check_framebuffer_status(GL_FRAMEBUFFER), GL_FRAMEBUFFER_COMPLETE);
        }

        Target { fbo, width, height, bind_framebuffer, read_pixels: egl.proc(c"glReadPixels") }
    }

    fn pixels(&self) -> Vec<u8> {
        let mut pixels = vec![0u8; (self.width * self.height * 4) as usize];
        // mpv leaves the default framebuffer bound, which a surfaceless context doesn't have.
        unsafe {
            (self.bind_framebuffer)(GL_FRAMEBUFFER, self.fbo);
            (self.read_pixels)(0, 0, self.width, self.height, GL_RGBA, GL_UNSIGNED_BYTE, pixels.as_mut_ptr().cast());
        }
        pixels
    }
}

struct FileStream(File);

impl Stream for FileStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }

    fn seek(&mut self, offset: u64) -> std::io::Result<u64> {
        self.0.seek(SeekFrom::Start(offset))
    }

    fn size(&mut self) -> Option<u64> {
        self.0.metadata().ok().map(|metadata| metadata.len())
    }
}

/// Plays the clip from a `tinystream://` stream, rendering what mpv has, until a frame with
/// something on it arrives; returns the decoder mpv used.
fn play(hwdec: &str, render_node: Option<&File>) -> String {
    let mpv = Mpv::new(&[("vo", "libmpv"), ("ao", "null"), ("hwdec", hwdec), ("keep-open", "yes")]).unwrap();
    mpv.add_protocol("tinystream", |uri| {
        assert_eq!(uri, "tinystream://testsrc");
        Some(Box::new(FileStream(File::open(CLIP).ok()?)) as Box<dyn Stream>)
    })
    .unwrap();

    let egl = Egl::new();
    let lookup = |name: &CStr| unsafe { eglGetProcAddress(name.as_ptr()) };
    let init = GlInit { get_proc_address: &lookup, drm_render_fd: render_node.map(AsRawFd::as_raw_fd) };
    let render = RenderContext::new_gl(&mpv, init).unwrap();
    let target = Target::new(&egl, 64, 36);

    mpv.command(&["loadfile", "tinystream://testsrc"]).unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut decoder = String::new();

    while Instant::now() < deadline {
        while let Some(event) = mpv.wait_event(0.0) {
            if let Event::EndFile(EndReason::Error(error)) = event {
                panic!("playback failed: {error}");
            }
        }

        if render.update() {
            render.render(target.fbo as i32, target.width, target.height, false).unwrap();

            if let Ok(current) = mpv.get_property::<String>("hwdec-current") {
                decoder = current;
            }

            if target.pixels().chunks(4).any(|pixel| pixel[..3].iter().any(|&channel| channel > 32)) {
                return decoder;
            }
        }

        std::thread::sleep(Duration::from_millis(5));
    }

    panic!("no frame was rendered");
}

#[test]
fn renders_offscreen() {
    assert_eq!(play("no", None), "no");
}

#[test]
fn renders_offscreen_with_vaapi() {
    if !Path::new(RENDER_NODE).exists() {
        eprintln!("skipped: no {RENDER_NODE}");
        return;
    }

    let node = File::options().read(true).write(true).open(RENDER_NODE).unwrap();
    assert_eq!(play("vaapi", Some(&node)), "vaapi");
}

/// mpv takes any `hwdec`, but only considers the ones it has; without NVIDIA it then fails to open
/// the device and decodes in software.
#[test]
fn has_nvdec() {
    let mpv = Mpv::new(&[("vo", "null"), ("ao", "null"), ("hwdec", "nvdec-copy")]).unwrap();
    mpv.request_log_messages("v").unwrap();
    mpv.command(&["loadfile", CLIP]).unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);

    while Instant::now() < deadline {
        match mpv.wait_event(0.1) {
            Some(Event::Log { text, .. }) if text.starts_with("Looking at hwdec h264-nvdec-copy") => return,
            Some(Event::Log { text, .. }) if text.starts_with("Unsupported hwdec") => panic!("{text}"),
            Some(Event::EndFile(_)) => break,
            _ => {},
        }
    }

    panic!("mpv never considered NVDEC");
}

#[test]
fn has_no_libass() {
    let mpv = Mpv::new(&[("vo", "null"), ("ao", "null")]).unwrap();
    assert!(mpv.get_property::<i64>("libass-version").is_err());
}

#[test]
fn reads_nodes() {
    let mpv = Mpv::new(&[("vo", "null"), ("ao", "null"), ("pause", "yes")]).unwrap();
    assert_eq!(mpv.get_property::<mpv::Node>("track-list").unwrap(), mpv::Node::Array(Vec::new()));
    mpv.command(&["loadfile", CLIP]).unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);

    while Instant::now() < deadline {
        if let Some(Event::FileLoaded) = mpv.wait_event(0.1) {
            let tracks = mpv.get_property::<mpv::Node>("track-list").unwrap();
            let video =
                tracks.as_array().iter().find(|track| track.get("type").and_then(|t| t.as_str()) == Some("video"));
            assert_eq!(video.and_then(|track| track.get("demux-w")).and_then(|w| w.as_i64()), Some(320));
            return;
        }
    }

    panic!("the clip never loaded");
}
