// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bindings to libmpv's `client.h`, `render.h`, `render_gl.h` and `stream_cb.h`, written by hand
//! for the mpv in native/mpv (client API 2.5). Check them against the headers when moving it.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_double, c_int, c_ulong, c_void};

use mpv_src as _;

pub const fn mpv_make_version(major: c_ulong, minor: c_ulong) -> c_ulong {
    (major << 16) | minor
}

pub const MPV_CLIENT_API_VERSION: c_ulong = mpv_make_version(2, 5);

#[repr(C)]
pub struct mpv_handle {
    _private: [u8; 0],
}

#[repr(C)]
pub struct mpv_render_context {
    _private: [u8; 0],
}

pub type mpv_error = c_int;
pub const MPV_ERROR_SUCCESS: mpv_error = 0;
pub const MPV_ERROR_EVENT_QUEUE_FULL: mpv_error = -1;
pub const MPV_ERROR_NOMEM: mpv_error = -2;
pub const MPV_ERROR_UNINITIALIZED: mpv_error = -3;
pub const MPV_ERROR_INVALID_PARAMETER: mpv_error = -4;
pub const MPV_ERROR_OPTION_NOT_FOUND: mpv_error = -5;
pub const MPV_ERROR_OPTION_FORMAT: mpv_error = -6;
pub const MPV_ERROR_OPTION_ERROR: mpv_error = -7;
pub const MPV_ERROR_PROPERTY_NOT_FOUND: mpv_error = -8;
pub const MPV_ERROR_PROPERTY_FORMAT: mpv_error = -9;
pub const MPV_ERROR_PROPERTY_UNAVAILABLE: mpv_error = -10;
pub const MPV_ERROR_PROPERTY_ERROR: mpv_error = -11;
pub const MPV_ERROR_COMMAND: mpv_error = -12;
pub const MPV_ERROR_LOADING_FAILED: mpv_error = -13;
pub const MPV_ERROR_AO_INIT_FAILED: mpv_error = -14;
pub const MPV_ERROR_VO_INIT_FAILED: mpv_error = -15;
pub const MPV_ERROR_NOTHING_TO_PLAY: mpv_error = -16;
pub const MPV_ERROR_UNKNOWN_FORMAT: mpv_error = -17;
pub const MPV_ERROR_UNSUPPORTED: mpv_error = -18;
pub const MPV_ERROR_NOT_IMPLEMENTED: mpv_error = -19;
pub const MPV_ERROR_GENERIC: mpv_error = -20;

pub type mpv_format = c_int;
pub const MPV_FORMAT_NONE: mpv_format = 0;
pub const MPV_FORMAT_STRING: mpv_format = 1;
pub const MPV_FORMAT_OSD_STRING: mpv_format = 2;
pub const MPV_FORMAT_FLAG: mpv_format = 3;
pub const MPV_FORMAT_INT64: mpv_format = 4;
pub const MPV_FORMAT_DOUBLE: mpv_format = 5;
pub const MPV_FORMAT_NODE: mpv_format = 6;
pub const MPV_FORMAT_NODE_ARRAY: mpv_format = 7;
pub const MPV_FORMAT_NODE_MAP: mpv_format = 8;
pub const MPV_FORMAT_BYTE_ARRAY: mpv_format = 9;

#[repr(C)]
#[derive(Clone, Copy)]
pub union mpv_node_u {
    pub string: *mut c_char,
    pub flag: c_int,
    pub int64: i64,
    pub double_: c_double,
    pub list: *mut mpv_node_list,
    pub ba: *mut mpv_byte_array,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_node {
    pub u: mpv_node_u,
    pub format: mpv_format,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_node_list {
    pub num: c_int,
    pub values: *mut mpv_node,
    pub keys: *mut *mut c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_byte_array {
    pub data: *mut c_void,
    pub size: usize,
}

pub type mpv_event_id = c_int;
pub const MPV_EVENT_NONE: mpv_event_id = 0;
pub const MPV_EVENT_SHUTDOWN: mpv_event_id = 1;
pub const MPV_EVENT_LOG_MESSAGE: mpv_event_id = 2;
pub const MPV_EVENT_GET_PROPERTY_REPLY: mpv_event_id = 3;
pub const MPV_EVENT_SET_PROPERTY_REPLY: mpv_event_id = 4;
pub const MPV_EVENT_COMMAND_REPLY: mpv_event_id = 5;
pub const MPV_EVENT_START_FILE: mpv_event_id = 6;
pub const MPV_EVENT_END_FILE: mpv_event_id = 7;
pub const MPV_EVENT_FILE_LOADED: mpv_event_id = 8;
pub const MPV_EVENT_CLIENT_MESSAGE: mpv_event_id = 16;
pub const MPV_EVENT_VIDEO_RECONFIG: mpv_event_id = 17;
pub const MPV_EVENT_AUDIO_RECONFIG: mpv_event_id = 18;
pub const MPV_EVENT_SEEK: mpv_event_id = 20;
pub const MPV_EVENT_PLAYBACK_RESTART: mpv_event_id = 21;
pub const MPV_EVENT_PROPERTY_CHANGE: mpv_event_id = 22;
pub const MPV_EVENT_QUEUE_OVERFLOW: mpv_event_id = 24;
pub const MPV_EVENT_HOOK: mpv_event_id = 25;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event_property {
    pub name: *const c_char,
    pub format: mpv_format,
    pub data: *mut c_void,
}

pub type mpv_log_level = c_int;
pub const MPV_LOG_LEVEL_NONE: mpv_log_level = 0;
pub const MPV_LOG_LEVEL_FATAL: mpv_log_level = 10;
pub const MPV_LOG_LEVEL_ERROR: mpv_log_level = 20;
pub const MPV_LOG_LEVEL_WARN: mpv_log_level = 30;
pub const MPV_LOG_LEVEL_INFO: mpv_log_level = 40;
pub const MPV_LOG_LEVEL_V: mpv_log_level = 50;
pub const MPV_LOG_LEVEL_DEBUG: mpv_log_level = 60;
pub const MPV_LOG_LEVEL_TRACE: mpv_log_level = 70;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event_log_message {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    pub log_level: mpv_log_level,
}

pub type mpv_end_file_reason = c_int;
pub const MPV_END_FILE_REASON_EOF: mpv_end_file_reason = 0;
pub const MPV_END_FILE_REASON_STOP: mpv_end_file_reason = 2;
pub const MPV_END_FILE_REASON_QUIT: mpv_end_file_reason = 3;
pub const MPV_END_FILE_REASON_ERROR: mpv_end_file_reason = 4;
pub const MPV_END_FILE_REASON_REDIRECT: mpv_end_file_reason = 5;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event_start_file {
    pub playlist_entry_id: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event_end_file {
    pub reason: mpv_end_file_reason,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event_client_message {
    pub num_args: c_int,
    pub args: *mut *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event_hook {
    pub name: *const c_char,
    pub id: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event_command {
    pub result: mpv_node,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_event {
    pub event_id: mpv_event_id,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

pub type mpv_render_param_type = c_int;
pub const MPV_RENDER_PARAM_INVALID: mpv_render_param_type = 0;
pub const MPV_RENDER_PARAM_API_TYPE: mpv_render_param_type = 1;
pub const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: mpv_render_param_type = 2;
pub const MPV_RENDER_PARAM_OPENGL_FBO: mpv_render_param_type = 3;
pub const MPV_RENDER_PARAM_FLIP_Y: mpv_render_param_type = 4;
pub const MPV_RENDER_PARAM_DEPTH: mpv_render_param_type = 5;
pub const MPV_RENDER_PARAM_ICC_PROFILE: mpv_render_param_type = 6;
pub const MPV_RENDER_PARAM_AMBIENT_LIGHT: mpv_render_param_type = 7;
pub const MPV_RENDER_PARAM_X11_DISPLAY: mpv_render_param_type = 8;
pub const MPV_RENDER_PARAM_WL_DISPLAY: mpv_render_param_type = 9;
pub const MPV_RENDER_PARAM_ADVANCED_CONTROL: mpv_render_param_type = 10;
pub const MPV_RENDER_PARAM_NEXT_FRAME_INFO: mpv_render_param_type = 11;
pub const MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME: mpv_render_param_type = 12;
pub const MPV_RENDER_PARAM_SKIP_RENDERING: mpv_render_param_type = 13;
pub const MPV_RENDER_PARAM_DRM_DISPLAY: mpv_render_param_type = 14;
pub const MPV_RENDER_PARAM_DRM_DRAW_SURFACE_SIZE: mpv_render_param_type = 15;
pub const MPV_RENDER_PARAM_DRM_DISPLAY_V2: mpv_render_param_type = 16;
pub const MPV_RENDER_PARAM_SW_SIZE: mpv_render_param_type = 17;
pub const MPV_RENDER_PARAM_SW_FORMAT: mpv_render_param_type = 18;
pub const MPV_RENDER_PARAM_SW_STRIDE: mpv_render_param_type = 19;
pub const MPV_RENDER_PARAM_SW_POINTER: mpv_render_param_type = 20;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_render_param {
    pub type_: mpv_render_param_type,
    pub data: *mut c_void,
}

pub const MPV_RENDER_API_TYPE_OPENGL: &std::ffi::CStr = c"opengl";
pub const MPV_RENDER_API_TYPE_SW: &std::ffi::CStr = c"sw";

pub type mpv_render_frame_info_flag = u64;
pub const MPV_RENDER_FRAME_INFO_PRESENT: mpv_render_frame_info_flag = 1 << 0;
pub const MPV_RENDER_FRAME_INFO_REDRAW: mpv_render_frame_info_flag = 1 << 1;
pub const MPV_RENDER_FRAME_INFO_REPEAT: mpv_render_frame_info_flag = 1 << 2;
pub const MPV_RENDER_FRAME_INFO_BLOCK_VSYNC: mpv_render_frame_info_flag = 1 << 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_render_frame_info {
    pub flags: u64,
    pub target_time: i64,
}

pub type mpv_render_update_fn = Option<unsafe extern "C" fn(cb_ctx: *mut c_void)>;

pub type mpv_render_update_flag = u64;
pub const MPV_RENDER_UPDATE_FRAME: mpv_render_update_flag = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_opengl_init_params {
    pub get_proc_address: Option<unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> *mut c_void>,
    pub get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_opengl_fbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

#[repr(C)]
pub struct _drmModeAtomicReq {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_opengl_drm_params_v2 {
    pub fd: c_int,
    pub crtc_id: c_int,
    pub connector_id: c_int,
    pub atomic_request_ptr: *mut *mut _drmModeAtomicReq,
    pub render_fd: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_opengl_drm_draw_surface_size {
    pub width: c_int,
    pub height: c_int,
}

pub type mpv_stream_cb_read_fn =
    Option<unsafe extern "C" fn(cookie: *mut c_void, buf: *mut c_char, nbytes: u64) -> i64>;
pub type mpv_stream_cb_seek_fn = Option<unsafe extern "C" fn(cookie: *mut c_void, offset: i64) -> i64>;
pub type mpv_stream_cb_size_fn = Option<unsafe extern "C" fn(cookie: *mut c_void) -> i64>;
pub type mpv_stream_cb_close_fn = Option<unsafe extern "C" fn(cookie: *mut c_void)>;
pub type mpv_stream_cb_cancel_fn = Option<unsafe extern "C" fn(cookie: *mut c_void)>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpv_stream_cb_info {
    pub cookie: *mut c_void,
    pub read_fn: mpv_stream_cb_read_fn,
    pub seek_fn: mpv_stream_cb_seek_fn,
    pub size_fn: mpv_stream_cb_size_fn,
    pub close_fn: mpv_stream_cb_close_fn,
    pub cancel_fn: mpv_stream_cb_cancel_fn,
}

pub type mpv_stream_cb_open_ro_fn =
    Option<unsafe extern "C" fn(user_data: *mut c_void, uri: *mut c_char, info: *mut mpv_stream_cb_info) -> c_int>;

unsafe extern "C" {
    pub fn mpv_client_api_version() -> c_ulong;
    pub fn mpv_error_string(error: c_int) -> *const c_char;
    pub fn mpv_free(data: *mut c_void);
    pub fn mpv_client_name(ctx: *mut mpv_handle) -> *const c_char;
    pub fn mpv_client_id(ctx: *mut mpv_handle) -> i64;
    pub fn mpv_create() -> *mut mpv_handle;
    pub fn mpv_initialize(ctx: *mut mpv_handle) -> c_int;
    pub fn mpv_destroy(ctx: *mut mpv_handle);
    pub fn mpv_terminate_destroy(ctx: *mut mpv_handle);
    pub fn mpv_create_client(ctx: *mut mpv_handle, name: *const c_char) -> *mut mpv_handle;
    pub fn mpv_create_weak_client(ctx: *mut mpv_handle, name: *const c_char) -> *mut mpv_handle;
    pub fn mpv_load_config_file(ctx: *mut mpv_handle, filename: *const c_char) -> c_int;
    pub fn mpv_get_time_ns(ctx: *mut mpv_handle) -> i64;
    pub fn mpv_get_time_us(ctx: *mut mpv_handle) -> i64;
    pub fn mpv_free_node_contents(node: *mut mpv_node);
    pub fn mpv_set_option(ctx: *mut mpv_handle, name: *const c_char, format: mpv_format, data: *mut c_void) -> c_int;
    pub fn mpv_set_option_string(ctx: *mut mpv_handle, name: *const c_char, data: *const c_char) -> c_int;
    pub fn mpv_command(ctx: *mut mpv_handle, args: *mut *const c_char) -> c_int;
    pub fn mpv_command_node(ctx: *mut mpv_handle, args: *mut mpv_node, result: *mut mpv_node) -> c_int;
    pub fn mpv_command_ret(ctx: *mut mpv_handle, args: *mut *const c_char, result: *mut mpv_node) -> c_int;
    pub fn mpv_command_string(ctx: *mut mpv_handle, args: *const c_char) -> c_int;
    pub fn mpv_command_async(ctx: *mut mpv_handle, reply_userdata: u64, args: *mut *const c_char) -> c_int;
    pub fn mpv_command_node_async(ctx: *mut mpv_handle, reply_userdata: u64, args: *mut mpv_node) -> c_int;
    pub fn mpv_abort_async_command(ctx: *mut mpv_handle, reply_userdata: u64);
    pub fn mpv_set_property(ctx: *mut mpv_handle, name: *const c_char, format: mpv_format, data: *mut c_void) -> c_int;
    pub fn mpv_set_property_string(ctx: *mut mpv_handle, name: *const c_char, data: *const c_char) -> c_int;
    pub fn mpv_del_property(ctx: *mut mpv_handle, name: *const c_char) -> c_int;
    pub fn mpv_set_property_async(
        ctx: *mut mpv_handle,
        reply_userdata: u64,
        name: *const c_char,
        format: mpv_format,
        data: *mut c_void,
    ) -> c_int;
    pub fn mpv_get_property(ctx: *mut mpv_handle, name: *const c_char, format: mpv_format, data: *mut c_void) -> c_int;
    pub fn mpv_get_property_string(ctx: *mut mpv_handle, name: *const c_char) -> *mut c_char;
    pub fn mpv_get_property_osd_string(ctx: *mut mpv_handle, name: *const c_char) -> *mut c_char;
    pub fn mpv_get_property_async(
        ctx: *mut mpv_handle,
        reply_userdata: u64,
        name: *const c_char,
        format: mpv_format,
    ) -> c_int;
    pub fn mpv_observe_property(
        mpv: *mut mpv_handle,
        reply_userdata: u64,
        name: *const c_char,
        format: mpv_format,
    ) -> c_int;
    pub fn mpv_unobserve_property(mpv: *mut mpv_handle, registered_reply_userdata: u64) -> c_int;
    pub fn mpv_event_name(event: mpv_event_id) -> *const c_char;
    pub fn mpv_event_to_node(dst: *mut mpv_node, src: *mut mpv_event) -> c_int;
    pub fn mpv_request_event(ctx: *mut mpv_handle, event: mpv_event_id, enable: c_int) -> c_int;
    pub fn mpv_request_log_messages(ctx: *mut mpv_handle, min_level: *const c_char) -> c_int;
    pub fn mpv_wait_event(ctx: *mut mpv_handle, timeout: c_double) -> *mut mpv_event;
    pub fn mpv_wakeup(ctx: *mut mpv_handle);
    pub fn mpv_set_wakeup_callback(
        ctx: *mut mpv_handle,
        cb: Option<unsafe extern "C" fn(d: *mut c_void)>,
        d: *mut c_void,
    );
    pub fn mpv_wait_async_requests(ctx: *mut mpv_handle);
    pub fn mpv_hook_add(ctx: *mut mpv_handle, reply_userdata: u64, name: *const c_char, priority: c_int) -> c_int;
    pub fn mpv_hook_continue(ctx: *mut mpv_handle, id: u64) -> c_int;

    pub fn mpv_render_context_create(
        res: *mut *mut mpv_render_context,
        mpv: *mut mpv_handle,
        params: *mut mpv_render_param,
    ) -> c_int;
    pub fn mpv_render_context_set_parameter(ctx: *mut mpv_render_context, param: mpv_render_param) -> c_int;
    pub fn mpv_render_context_get_info(ctx: *mut mpv_render_context, param: mpv_render_param) -> c_int;
    pub fn mpv_render_context_set_update_callback(
        ctx: *mut mpv_render_context,
        callback: mpv_render_update_fn,
        callback_ctx: *mut c_void,
    );
    pub fn mpv_render_context_update(ctx: *mut mpv_render_context) -> u64;
    pub fn mpv_render_context_render(ctx: *mut mpv_render_context, params: *mut mpv_render_param) -> c_int;
    pub fn mpv_render_context_report_swap(ctx: *mut mpv_render_context);
    pub fn mpv_render_context_free(ctx: *mut mpv_render_context);

    pub fn mpv_stream_cb_add_ro(
        ctx: *mut mpv_handle,
        protocol: *const c_char,
        user_data: *mut c_void,
        open_fn: mpv_stream_cb_open_ro_fn,
    ) -> c_int;
}
