// SPDX-License-Identifier: AGPL-3.0-or-later

#![allow(
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    improper_ctypes,
    unnecessary_transmutes,
    unsafe_op_in_unsafe_fn,
    clippy::all
)]

use std::ffi::c_int;

use ffmpeg_src as _;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub const AV_NOPTS_VALUE: i64 = i64::MIN;

pub const AV_TIME_BASE_Q: AVRational = AVRational { num: 1, den: AV_TIME_BASE as c_int };

pub const fn MKTAG(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

pub const fn AVERROR(errno: u32) -> c_int {
    -(errno as c_int)
}

const fn FFERRTAG(a: u8, b: u8, c: u8, d: u8) -> c_int {
    -(MKTAG(a, b, c, d) as c_int)
}

pub const AVERROR_EOF: c_int = FFERRTAG(b'E', b'O', b'F', b' ');

pub const fn av_inv_q(q: AVRational) -> AVRational {
    AVRational { num: q.den, den: q.num }
}
