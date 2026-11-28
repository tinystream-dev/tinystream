// SPDX-License-Identifier: AGPL-3.0-or-later

//! A small safe wrapper over libmpv: the client handle, its properties, commands and events, the
//! OpenGL render context and stream callbacks.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::os::fd::RawFd;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex};

use mpv_sys::*;

/// An error code from libmpv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub c_int);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = unsafe { CStr::from_ptr(mpv_error_string(self.0)) };
        write!(f, "mpv: {}", text.to_string_lossy())
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn check(code: c_int) -> Result<()> {
    if code < 0 { Err(Error(code)) } else { Ok(()) }
}

fn cstring(text: &str) -> Result<CString> {
    CString::new(text).map_err(|_| Error(MPV_ERROR_INVALID_PARAMETER))
}

/// The core. Dropping the last [`Mpv`] (and every [`RenderContext`] made from it) terminates it.
struct Core {
    handle: NonNull<mpv_handle>,
    /// What mpv may call until it's destroyed: wakeup callbacks and protocols' openers.
    callbacks: Mutex<Vec<Box<dyn Send + Sync>>>,
    /// Held while taking an event: only one thread may wait at once, and the event is only valid
    /// until the next wait.
    events: Mutex<()>,
}

// libmpv's client API is thread-safe.
unsafe impl Send for Core {}
unsafe impl Sync for Core {}

impl Drop for Core {
    fn drop(&mut self) {
        unsafe { mpv_terminate_destroy(self.handle.as_ptr()) };
    }
}

/// A client handle to an mpv core.
#[derive(Clone)]
pub struct Mpv {
    core: Arc<Core>,
}

impl Mpv {
    /// Creates a core with `options` set before it's initialized (the ones that can't change later,
    /// like `vo`), and initializes it. mpv reads no config files and takes no input of its own.
    pub fn new(options: &[(&str, &str)]) -> Result<Mpv> {
        let handle = NonNull::new(unsafe { mpv_create() }).ok_or(Error(MPV_ERROR_NOMEM))?;
        let core = Arc::new(Core { handle, callbacks: Mutex::new(Vec::new()), events: Mutex::new(()) });

        for (name, value) in options {
            let (name, value) = (cstring(name)?, cstring(value)?);
            check(unsafe { mpv_set_option_string(handle.as_ptr(), name.as_ptr(), value.as_ptr()) })?;
        }

        check(unsafe { mpv_initialize(handle.as_ptr()) })?;
        Ok(Mpv { core })
    }

    fn handle(&self) -> *mut mpv_handle {
        self.core.handle.as_ptr()
    }

    /// Runs a command, like `["loadfile", url]`, and waits for it.
    pub fn command(&self, args: &[&str]) -> Result<()> {
        let args = args.iter().map(|arg| cstring(arg)).collect::<Result<Vec<_>>>()?;
        let mut pointers: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(ptr::null());
        check(unsafe { mpv_command(self.handle(), pointers.as_mut_ptr()) })
    }

    /// Runs a command without waiting; its result arrives as [`Event::CommandReply`] with `id`.
    pub fn command_async(&self, id: u64, args: &[&str]) -> Result<()> {
        let args = args.iter().map(|arg| cstring(arg)).collect::<Result<Vec<_>>>()?;
        let mut pointers: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(ptr::null());
        check(unsafe { mpv_command_async(self.handle(), id, pointers.as_mut_ptr()) })
    }

    pub fn set_property(&self, name: &str, value: impl SetProperty) -> Result<()> {
        let name = cstring(name)?;
        value.set(self, &name)
    }

    pub fn get_property<T: GetProperty>(&self, name: &str) -> Result<T> {
        let name = cstring(name)?;
        T::get(self, &name)
    }

    /// Reports changes to a property as [`Event::PropertyChange`] with `id`, starting with its
    /// current value.
    pub fn observe_property(&self, id: u64, name: &str, format: Format) -> Result<()> {
        let name = cstring(name)?;
        check(unsafe { mpv_observe_property(self.handle(), id, name.as_ptr(), format as mpv_format) })
    }

    pub fn unobserve_property(&self, id: u64) -> Result<()> {
        check(unsafe { mpv_unobserve_property(self.handle(), id) })
    }

    /// Sends mpv's log at `level` (like `"warn"`, or `"no"` to stop) as [`Event::Log`].
    pub fn request_log_messages(&self, level: &str) -> Result<()> {
        let level = cstring(level)?;
        check(unsafe { mpv_request_log_messages(self.handle(), level.as_ptr()) })
    }

    /// Waits up to `timeout` seconds (0 to poll, negative to wait forever) for the next event.
    pub fn wait_event(&self, timeout: f64) -> Option<Event> {
        let _waiting = self.core.events.lock().unwrap();
        let event = unsafe { &*mpv_wait_event(self.handle(), timeout) };
        unsafe { Event::from_raw(event) }
    }

    /// Calls `wake` (from any of mpv's threads) whenever events are waiting. It mustn't call back
    /// into mpv.
    pub fn set_wakeup_callback(&self, wake: impl Fn() + Send + Sync + 'static) {
        unsafe extern "C" fn trampoline(data: *mut c_void) {
            unsafe { (*(data as *const Box<dyn Fn() + Send + Sync>))() };
        }

        let wake: Box<Box<dyn Fn() + Send + Sync>> = Box::new(Box::new(wake));
        let data = (&raw const *wake).cast_mut().cast();
        unsafe { mpv_set_wakeup_callback(self.handle(), Some(trampoline), data) };

        // mpv doesn't say when it stops calling the one before, so they all live as long as the
        // core.
        self.core.callbacks.lock().unwrap().push(wake);
    }

    /// Makes `protocol://…` URLs open through `open`, which gets the whole URL and returns the
    /// stream to read, or `None` if it can't be opened.
    pub fn add_protocol(
        &self,
        protocol: &str,
        open: impl Fn(&str) -> Option<Box<dyn Stream>> + Send + Sync + 'static,
    ) -> Result<()> {
        let protocol = cstring(protocol)?;
        let open: Box<Box<Open>> = Box::new(Box::new(open));
        let data = (&raw const *open).cast_mut().cast();
        check(unsafe { mpv_stream_cb_add_ro(self.handle(), protocol.as_ptr(), data, Some(open_stream)) })?;
        self.core.callbacks.lock().unwrap().push(open);
        Ok(())
    }
}

/// The formats a property can be read or observed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Format {
    None = MPV_FORMAT_NONE,
    String = MPV_FORMAT_STRING,
    Flag = MPV_FORMAT_FLAG,
    Int64 = MPV_FORMAT_INT64,
    Double = MPV_FORMAT_DOUBLE,
}

/// A value a property can be set to.
pub trait SetProperty {
    #[doc(hidden)]
    fn set(self, mpv: &Mpv, name: &CStr) -> Result<()>;
}

/// A type a property can be read as.
pub trait GetProperty: Sized {
    #[doc(hidden)]
    fn get(mpv: &Mpv, name: &CStr) -> Result<Self>;
}

macro_rules! plain_property {
    ($type:ty, $format:expr, $raw:ty, $to:expr, $from:expr) => {
        impl SetProperty for $type {
            fn set(self, mpv: &Mpv, name: &CStr) -> Result<()> {
                let mut raw: $raw = $to(self);
                check(unsafe { mpv_set_property(mpv.handle(), name.as_ptr(), $format, (&raw mut raw).cast()) })
            }
        }

        impl GetProperty for $type {
            fn get(mpv: &Mpv, name: &CStr) -> Result<Self> {
                let mut raw: $raw = Default::default();
                check(unsafe { mpv_get_property(mpv.handle(), name.as_ptr(), $format, (&raw mut raw).cast()) })?;
                Ok($from(raw))
            }
        }
    };
}

plain_property!(bool, MPV_FORMAT_FLAG, c_int, c_int::from, |raw| raw != 0);
plain_property!(i64, MPV_FORMAT_INT64, i64, |value| value, |raw| raw);
plain_property!(f64, MPV_FORMAT_DOUBLE, f64, |value| value, |raw| raw);

impl SetProperty for &str {
    fn set(self, mpv: &Mpv, name: &CStr) -> Result<()> {
        let value = cstring(self)?;
        check(unsafe { mpv_set_property_string(mpv.handle(), name.as_ptr(), value.as_ptr()) })
    }
}

impl SetProperty for String {
    fn set(self, mpv: &Mpv, name: &CStr) -> Result<()> {
        self.as_str().set(mpv, name)
    }
}

impl GetProperty for String {
    fn get(mpv: &Mpv, name: &CStr) -> Result<Self> {
        let mut raw: *mut c_char = ptr::null_mut();
        check(unsafe { mpv_get_property(mpv.handle(), name.as_ptr(), MPV_FORMAT_STRING, (&raw mut raw).cast()) })?;
        let value = unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned();
        unsafe { mpv_free(raw.cast()) };
        Ok(value)
    }
}

/// A structured value, like `track-list` or `demuxer-cache-state`.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    None,
    String(String),
    Flag(bool),
    Int64(i64),
    Double(f64),
    Array(Vec<Node>),
    Map(Vec<(String, Node)>),
}

impl Node {
    unsafe fn from_raw(node: &mpv_node) -> Node {
        let text = |text: *const c_char| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();

        unsafe {
            match node.format {
                MPV_FORMAT_STRING => Node::String(text(node.u.string)),
                MPV_FORMAT_FLAG => Node::Flag(node.u.flag != 0),
                MPV_FORMAT_INT64 => Node::Int64(node.u.int64),
                MPV_FORMAT_DOUBLE => Node::Double(node.u.double_),
                MPV_FORMAT_NODE_ARRAY | MPV_FORMAT_NODE_MAP => {
                    let list = &*node.u.list;
                    if list.num <= 0 {
                        return if node.format == MPV_FORMAT_NODE_ARRAY {
                            Node::Array(Vec::new())
                        } else {
                            Node::Map(Vec::new())
                        };
                    }
                    let values = std::slice::from_raw_parts(list.values, list.num as usize);
                    if node.format == MPV_FORMAT_NODE_ARRAY {
                        Node::Array(values.iter().map(|value| Node::from_raw(value)).collect())
                    } else {
                        let keys = std::slice::from_raw_parts(list.keys, values.len());
                        Node::Map(
                            keys.iter().zip(values).map(|(&key, value)| (text(key), Node::from_raw(value))).collect(),
                        )
                    }
                },
                _ => Node::None,
            }
        }
    }

    /// The value under `key`, in a map.
    pub fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Map(entries) => entries.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Node::Double(value) => Some(value),
            Node::Int64(value) => Some(value as f64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Node::Int64(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Node::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Node::Flag(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> &[Node] {
        match self {
            Node::Array(values) => values,
            _ => &[],
        }
    }
}

impl GetProperty for Node {
    fn get(mpv: &Mpv, name: &CStr) -> Result<Self> {
        let mut raw = mpv_node { u: mpv_node_u { int64: 0 }, format: MPV_FORMAT_NONE };
        check(unsafe { mpv_get_property(mpv.handle(), name.as_ptr(), MPV_FORMAT_NODE, (&raw mut raw).cast()) })?;
        let node = unsafe { Node::from_raw(&raw) };
        unsafe { mpv_free_node_contents(&mut raw) };
        Ok(node)
    }
}

/// A property's value in an event.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Unavailable, or observed with [`Format::None`].
    None,
    String(String),
    Flag(bool),
    Int64(i64),
    Double(f64),
}

impl Value {
    unsafe fn from_raw(format: mpv_format, data: *mut c_void) -> Value {
        unsafe {
            match format {
                MPV_FORMAT_STRING => {
                    Value::String(CStr::from_ptr(*(data as *const *const c_char)).to_string_lossy().into_owned())
                },
                MPV_FORMAT_FLAG => Value::Flag(*(data as *const c_int) != 0),
                MPV_FORMAT_INT64 => Value::Int64(*(data as *const i64)),
                MPV_FORMAT_DOUBLE => Value::Double(*(data as *const f64)),
                _ => Value::None,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Eof,
    Stop,
    Quit,
    Error(Error),
    Redirect,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Shutdown,
    Log {
        prefix: String,
        level: String,
        text: String,
    },
    CommandReply {
        id: u64,
        result: Result<()>,
    },
    StartFile,
    EndFile(EndReason),
    FileLoaded,
    VideoReconfig,
    AudioReconfig,
    Seek,
    PlaybackRestart,
    PropertyChange {
        id: u64,
        name: String,
        value: Value,
    },
    /// Events were dropped because nobody took them.
    QueueOverflow,
    /// Anything else mpv sends, by its `mpv_event_id`.
    Other(c_int),
}

impl Event {
    unsafe fn from_raw(event: &mpv_event) -> Option<Event> {
        let text = |text: *const c_char| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();

        Some(match event.event_id {
            MPV_EVENT_NONE => return None,
            MPV_EVENT_SHUTDOWN => Event::Shutdown,
            MPV_EVENT_LOG_MESSAGE => {
                let message = unsafe { &*(event.data as *const mpv_event_log_message) };
                Event::Log { prefix: text(message.prefix), level: text(message.level), text: text(message.text) }
            },
            MPV_EVENT_COMMAND_REPLY => Event::CommandReply { id: event.reply_userdata, result: check(event.error) },
            MPV_EVENT_START_FILE => Event::StartFile,
            MPV_EVENT_END_FILE => {
                let end = unsafe { &*(event.data as *const mpv_event_end_file) };
                Event::EndFile(match end.reason {
                    MPV_END_FILE_REASON_EOF => EndReason::Eof,
                    MPV_END_FILE_REASON_STOP => EndReason::Stop,
                    MPV_END_FILE_REASON_QUIT => EndReason::Quit,
                    MPV_END_FILE_REASON_REDIRECT => EndReason::Redirect,
                    _ => EndReason::Error(Error(end.error)),
                })
            },
            MPV_EVENT_FILE_LOADED => Event::FileLoaded,
            MPV_EVENT_VIDEO_RECONFIG => Event::VideoReconfig,
            MPV_EVENT_AUDIO_RECONFIG => Event::AudioReconfig,
            MPV_EVENT_SEEK => Event::Seek,
            MPV_EVENT_PLAYBACK_RESTART => Event::PlaybackRestart,
            MPV_EVENT_PROPERTY_CHANGE => {
                let property = unsafe { &*(event.data as *const mpv_event_property) };
                Event::PropertyChange {
                    id: event.reply_userdata,
                    name: text(property.name),
                    value: unsafe { Value::from_raw(property.format, property.data) },
                }
            },
            MPV_EVENT_QUEUE_OVERFLOW => Event::QueueOverflow,
            other => Event::Other(other),
        })
    }
}

/// A stream mpv reads through [`Mpv::add_protocol`], on its own threads.
pub trait Stream: Send {
    /// Reads into `buf`, blocking until at least a byte is there; 0 is the end of the stream.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;

    /// Moves to `offset` from the start, returning it.
    fn seek(&mut self, offset: u64) -> std::io::Result<u64>;

    /// The stream's length, if it's known.
    fn size(&mut self) -> Option<u64>;

    /// Whether [`Stream::seek`] works. mpv only seeks a stream that can't within what it has
    /// cached.
    fn seekable(&self) -> bool {
        true
    }

    /// Something that makes a blocked [`Stream::read`] return early (with an error), called from
    /// another thread when mpv gives up on the stream.
    fn canceler(&self) -> Option<Box<dyn Fn() + Send + Sync>> {
        None
    }
}

type Open = dyn Fn(&str) -> Option<Box<dyn Stream>> + Send + Sync;

struct Cookie {
    stream: Box<dyn Stream>,
    cancel: Option<Box<dyn Fn() + Send + Sync>>,
}

unsafe extern "C" fn open_stream(user_data: *mut c_void, uri: *mut c_char, info: *mut mpv_stream_cb_info) -> c_int {
    let open = unsafe { &*(user_data as *const Box<Open>) };
    let uri = unsafe { CStr::from_ptr(uri) }.to_string_lossy();

    let Some(stream) = open(&uri) else {
        return MPV_ERROR_LOADING_FAILED;
    };

    let cancel = stream.canceler();
    let seekable = stream.seekable();
    let cookie = Box::into_raw(Box::new(Cookie { stream, cancel }));

    unsafe {
        *info = mpv_stream_cb_info {
            cookie: cookie.cast(),
            read_fn: Some(read_stream),
            seek_fn: if seekable { Some(seek_stream) } else { None },
            size_fn: Some(stream_size),
            close_fn: Some(close_stream),
            cancel_fn: Some(cancel_stream),
        };
    }

    0
}

// mpv calls `cancel_fn` from another thread while a read may be running, so each function reaches
// only its own field of the cookie.

unsafe extern "C" fn read_stream(cookie: *mut c_void, buf: *mut c_char, size: u64) -> i64 {
    let stream = unsafe { &mut (*(cookie as *mut Cookie)).stream };
    let buf = unsafe { std::slice::from_raw_parts_mut(buf.cast::<u8>(), size as usize) };
    stream.read(buf).map_or(-1, |read| read as i64)
}

unsafe extern "C" fn seek_stream(cookie: *mut c_void, offset: i64) -> i64 {
    let stream = unsafe { &mut (*(cookie as *mut Cookie)).stream };
    let Ok(offset) = u64::try_from(offset) else {
        return MPV_ERROR_GENERIC as i64;
    };
    stream.seek(offset).map_or(MPV_ERROR_GENERIC as i64, |position| position as i64)
}

unsafe extern "C" fn stream_size(cookie: *mut c_void) -> i64 {
    let stream = unsafe { &mut (*(cookie as *mut Cookie)).stream };
    stream.size().map_or(MPV_ERROR_UNSUPPORTED as i64, |size| size as i64)
}

unsafe extern "C" fn cancel_stream(cookie: *mut c_void) {
    if let Some(cancel) = unsafe { &(*(cookie as *const Cookie)).cancel } {
        cancel();
    }
}

unsafe extern "C" fn close_stream(cookie: *mut c_void) {
    drop(unsafe { Box::from_raw(cookie as *mut Cookie) });
}

/// What [`RenderContext::new_gl`] needs.
pub struct GlInit<'a> {
    /// Looks up an OpenGL function in the context that's current, like `eglGetProcAddress`.
    pub get_proc_address: &'a dyn Fn(&CStr) -> *mut c_void,
    /// A DRM render node for VA-API to decode on: the one of the GPU the context is on. mpv keeps
    /// using it, so it must stay open as long as the [`RenderContext`].
    pub drm_render_fd: Option<RawFd>,
}

/// mpv's renderer, drawing video into the app's OpenGL framebuffers. Everything on it, dropping it
/// too, has to happen on the thread with its OpenGL context current.
pub struct RenderContext {
    context: NonNull<mpv_render_context>,
    // Both are read by mpv after creation; the core outlives the context.
    _drm: Box<mpv_opengl_drm_params_v2>,
    update: Option<Box<Box<dyn Fn() + Send + Sync>>>,
    _core: Arc<Core>,
}

unsafe impl Send for RenderContext {}

impl RenderContext {
    /// Creates the renderer on the OpenGL context that's current. Use only one per core, with `vo`
    /// set to `libmpv`.
    pub fn new_gl(mpv: &Mpv, init: GlInit<'_>) -> Result<RenderContext> {
        unsafe extern "C" fn get_proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
            let lookup = unsafe { &*(ctx as *const &dyn Fn(&CStr) -> *mut c_void) };
            lookup(unsafe { CStr::from_ptr(name) })
        }

        let mut opengl = mpv_opengl_init_params {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: (&raw const init.get_proc_address).cast_mut().cast(),
        };

        let mut drm = Box::new(mpv_opengl_drm_params_v2 {
            fd: -1,
            crtc_id: -1,
            connector_id: -1,
            atomic_request_ptr: ptr::null_mut(),
            render_fd: init.drm_render_fd.unwrap_or(-1),
        });

        let mut params = vec![
            mpv_render_param {
                type_: MPV_RENDER_PARAM_API_TYPE,
                data: MPV_RENDER_API_TYPE_OPENGL.as_ptr().cast_mut().cast(),
            },
            mpv_render_param { type_: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, data: (&raw mut opengl).cast() },
        ];

        if init.drm_render_fd.is_some() {
            params.push(mpv_render_param { type_: MPV_RENDER_PARAM_DRM_DISPLAY_V2, data: (&raw mut *drm).cast() });
        }

        params.push(mpv_render_param { type_: MPV_RENDER_PARAM_INVALID, data: ptr::null_mut() });

        let mut context = ptr::null_mut();
        check(unsafe { mpv_render_context_create(&mut context, mpv.handle(), params.as_mut_ptr()) })?;

        Ok(RenderContext {
            context: NonNull::new(context).ok_or(Error(MPV_ERROR_GENERIC))?,
            _drm: drm,
            update: None,
            _core: mpv.core.clone(),
        })
    }

    /// Calls `update` (from any of mpv's threads) when there's a new frame to render. It mustn't
    /// call back into mpv; wake the render thread instead, which then calls
    /// [`RenderContext::update`].
    pub fn set_update_callback(&mut self, update: impl Fn() + Send + Sync + 'static) {
        unsafe extern "C" fn trampoline(data: *mut c_void) {
            unsafe { (*(data as *const Box<dyn Fn() + Send + Sync>))() };
        }

        let update: Box<Box<dyn Fn() + Send + Sync>> = Box::new(Box::new(update));
        let data = (&raw const *update).cast_mut().cast();
        // mpv calls it under a lock it takes here too, so the one before is done with after this.
        unsafe { mpv_render_context_set_update_callback(self.context.as_ptr(), Some(trampoline), data) };
        self.update = Some(update);
    }

    /// Whether a new frame is waiting to be rendered.
    pub fn update(&self) -> bool {
        unsafe { mpv_render_context_update(self.context.as_ptr()) & MPV_RENDER_UPDATE_FRAME != 0 }
    }

    /// Renders the current frame into framebuffer `fbo` (0 for the default one) of `width` by
    /// `height`, upside down when `flip` (for the default framebuffer).
    pub fn render(&self, fbo: i32, width: i32, height: i32, flip: bool) -> Result<()> {
        let mut target = mpv_opengl_fbo { fbo, w: width, h: height, internal_format: 0 };
        let mut flip = c_int::from(flip);
        let mut params = [
            mpv_render_param { type_: MPV_RENDER_PARAM_OPENGL_FBO, data: (&raw mut target).cast() },
            mpv_render_param { type_: MPV_RENDER_PARAM_FLIP_Y, data: (&raw mut flip).cast() },
            mpv_render_param { type_: MPV_RENDER_PARAM_INVALID, data: ptr::null_mut() },
        ];
        check(unsafe { mpv_render_context_render(self.context.as_ptr(), params.as_mut_ptr()) })
    }

    /// Tells mpv the rendered frame reached the screen, for its timing.
    pub fn report_swap(&self) {
        unsafe { mpv_render_context_report_swap(self.context.as_ptr()) };
    }
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        // The update callback, the DRM parameters and the core are dropped after.
        unsafe { mpv_render_context_free(self.context.as_ptr()) };
    }
}
