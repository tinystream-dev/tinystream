// SPDX-License-Identifier: AGPL-3.0-or-later

use std::ffi::CStr;
#[cfg(not(target_vendor = "apple"))]
use std::ffi::{c_char, c_void};
use std::path::Path;
use std::ptr;
use std::sync::Mutex;

use serde::Serialize;

use super::ff::{self, BufferRef, ffi};
use crate::config::{Hardware, Transcode};

pub const SDR_TONEMAP: &str =
    "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv";

pub fn download_format(frame: &ffi::AVFrame) -> String {
    let format = if frame.hw_frames_ctx.is_null() {
        ffi::AV_PIX_FMT_NV12
    } else {
        unsafe { (*((*frame.hw_frames_ctx).data as *const ffi::AVHWFramesContext)).sw_format }
    };
    unsafe { ff::opt_str(ffi::av_get_pix_fmt_name(format)) }.unwrap_or_else(|| "nv12".into())
}

/// Whether the hardware works on frames in its own memory (VA-API's surfaces) rather than taking
/// ordinary ones (VideoToolbox's encoder).
pub const GPU_FRAMES: bool = cfg!(not(target_vendor = "apple"));

const API: &str = if cfg!(target_vendor = "apple") { "VideoToolbox" } else { "VA-API" };

pub fn h264_encoder(hardware: bool, device: Option<&BufferRef>) -> &'static CStr {
    if hardware && is_cuda(device) {
        return c"h264_nvenc";
    }
    match (hardware, GPU_FRAMES) {
        (false, _) => c"libx264",
        (true, true) => c"h264_vaapi",
        (true, false) => c"h264_videotoolbox",
    }
}

#[cfg(not(target_vendor = "apple"))]
unsafe extern "C" {
    fn vaQueryVendorString(dpy: *mut c_void) -> *const c_char;
    fn va_shim_error() -> *const c_char;
}

#[cfg(not(target_vendor = "apple"))]
#[repr(C)]
struct VaapiDeviceContext {
    display: *mut c_void,
    driver_quirks: u32,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[graphql(name = "Transcoding")]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub vaapi: Option<String>,
    pub vaapi_error: Option<String>,
    pub nvenc: Option<String>,
    pub nvenc_error: Option<String>,
    pub software_h264: bool,
}

pub struct Hw {
    state: Mutex<(Option<(Transcode, BufferRef)>, Capabilities)>,
}

impl Hw {
    pub fn new() -> Self {
        Self {
            state: Mutex::new((
                None,
                Capabilities {
                    vaapi: None,
                    vaapi_error: None,
                    nvenc: None,
                    nvenc_error: None,
                    software_h264: encoder_exists("libx264"),
                },
            )),
        }
    }

    pub fn configure(&self, t: &Transcode) {
        let mut state = self.state.lock().unwrap();

        if state.0.as_ref().is_some_and(|(config, _)| config == t) {
            return;
        }
        state.0 = None;
        state.1.vaapi = None;
        state.1.vaapi_error = None;
        state.1.nvenc = None;
        state.1.nvenc_error = None;
        if t.hardware == Hardware::Software {
            tracing::info!("transcoding: software (x264), as configured");
            return;
        }
        if t.hardware != Hardware::Nvenc {
            match open_hardware(Path::new(&t.vaapi_device)) {
                Ok((dev, vendor)) => {
                    tracing::info!("transcoding: {API} ({vendor})");
                    state.0 = Some((t.clone(), dev));
                    state.1.vaapi = Some(vendor);
                    return;
                },
                Err(error) => state.1.vaapi_error = Some(format!("{error:#}")),
            }
        }
        if matches!(t.hardware, Hardware::Auto | Hardware::Nvenc) {
            match open_cuda(t.cuda_device) {
                Ok((dev, vendor)) => {
                    tracing::info!("transcoding: NVENC ({vendor})");
                    state.0 = Some((t.clone(), dev));
                    state.1.nvenc = Some(vendor);
                    return;
                },
                Err(error) => state.1.nvenc_error = Some(format!("{error:#}")),
            }
        }
        tracing::info!(vaapi = ?state.1.vaapi_error, nvenc = ?state.1.nvenc_error, "transcoding: software (x264); hardware unavailable");
    }

    pub fn device(&self) -> Option<BufferRef> {
        self.state.lock().unwrap().0.as_ref().map(|(_, device)| device.clone())
    }

    pub fn capabilities(&self) -> Capabilities {
        self.state.lock().unwrap().1.clone()
    }
}

pub fn is_cuda(device: Option<&BufferRef>) -> bool {
    device.is_some_and(|dev| unsafe {
        (*((*dev.0).data as *const ffi::AVHWDeviceContext)).type_ == ffi::AV_HWDEVICE_TYPE_CUDA
    })
}

fn open_cuda(index: u32) -> anyhow::Result<(BufferRef, String)> {
    anyhow::ensure!(encoder_exists("h264_nvenc"), "this build has no h264_nvenc encoder");
    let mut buf = ptr::null_mut();
    let device = ff::cstr(&index.to_string());
    ff::check(
        unsafe {
            ffi::av_hwdevice_ctx_create(&mut buf, ffi::AV_HWDEVICE_TYPE_CUDA, device.as_ptr(), ptr::null_mut(), 0)
        },
        "can't open CUDA device",
    )?;
    let device = BufferRef(buf);
    let codec = unsafe { ffi::avcodec_find_encoder_by_name(c"h264_nvenc".as_ptr()) };
    let mut encoder = ff::Codec::alloc(codec)?;
    let context = encoder.get_mut();
    context.width = 256;
    context.height = 256;
    context.pix_fmt = ffi::AV_PIX_FMT_NV12;
    context.time_base = ff::q(1, 24);
    context.hw_device_ctx = device.new_ref();
    encoder.open(&mut ff::Dict::new(&[("preset", "p4")]))?;
    let name = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader", &format!("--id={index}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("CUDA device {index}"));
    Ok((device, name))
}

fn encoder_exists(name: &str) -> bool {
    let n = ff::cstr(name);
    !unsafe { ffi::avcodec_find_encoder_by_name(n.as_ptr()) }.is_null()
}

#[cfg(target_vendor = "apple")]
fn open_hardware(_device: &Path) -> anyhow::Result<(BufferRef, String)> {
    if !encoder_exists("h264_videotoolbox") {
        anyhow::bail!("this build has no h264_videotoolbox encoder");
    }

    let mut buf = ptr::null_mut();

    ff::check(
        unsafe {
            ffi::av_hwdevice_ctx_create(&mut buf, ffi::AV_HWDEVICE_TYPE_VIDEOTOOLBOX, ptr::null(), ptr::null_mut(), 0)
        },
        "can't open VideoToolbox",
    )?;

    Ok((BufferRef(buf), "Apple".into()))
}

#[cfg(not(target_vendor = "apple"))]
fn open_hardware(device: &Path) -> anyhow::Result<(BufferRef, String)> {
    if !encoder_exists("h264_vaapi") {
        anyhow::bail!("this build has no h264_vaapi encoder");
    }

    if let Some(e) = unsafe { ff::opt_str(va_shim_error()) } {
        anyhow::bail!("can't load libva: {e}");
    }

    let mut buf = ptr::null_mut();
    let dev = ff::cstr(&device.to_string_lossy());

    ff::check(
        unsafe { ffi::av_hwdevice_ctx_create(&mut buf, ffi::AV_HWDEVICE_TYPE_VAAPI, dev.as_ptr(), ptr::null_mut(), 0) },
        &format!("can't open {}", device.display()),
    )?;

    let buf = BufferRef(buf);

    let vendor = unsafe {
        let hwdev = (*buf.0).data as *mut ffi::AVHWDeviceContext;
        let va = (*hwdev).hwctx as *mut VaapiDeviceContext;
        ff::opt_str(vaQueryVendorString((*va).display)).unwrap_or_else(|| "unknown driver".into())
    };

    Ok((buf, vendor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ff::{FilterGraph, Frame};

    #[test]
    fn unavailable_cuda_falls_back_without_affecting_software() {
        let hw = Hw::new();
        hw.configure(&Transcode { hardware: Hardware::Nvenc, cuda_device: u32::MAX, ..Default::default() });
        assert!(hw.device().is_none());
        assert!(hw.capabilities().nvenc_error.is_some());
        hw.configure(&Transcode { hardware: Hardware::Software, ..Default::default() });
        assert!(hw.device().is_none());
        assert!(hw.capabilities().nvenc_error.is_none());
        assert!(hw.capabilities().software_h264);
    }

    #[test]
    fn pq_and_hlg_frames_become_tagged_sdr() {
        for transfer in [ffi::AVCOL_TRC_SMPTE2084, ffi::AVCOL_TRC_ARIB_STD_B67] {
            let mut frame = Frame::new();
            let f = frame.get_mut();
            f.width = 32;
            f.height = 32;
            f.format = ffi::AV_PIX_FMT_YUV420P10LE;
            f.color_primaries = ffi::AVCOL_PRI_BT2020;
            f.color_trc = transfer;
            f.colorspace = ffi::AVCOL_SPC_BT2020_NCL;
            f.color_range = ffi::AVCOL_RANGE_MPEG;
            ff::check(unsafe { ffi::av_frame_get_buffer(frame.0, 0) }, "frame buffer").unwrap();
            let f = frame.get_mut();
            for plane in 0..3 {
                unsafe {
                    ptr::write_bytes(f.data[plane], 0, f.linesize[plane] as usize * if plane == 0 { 32 } else { 16 })
                };
            }
            let mut graph =
                FilterGraph::video(f, ff::q(1, 24), &format!("{SDR_TONEMAP},format=yuv420p"), None).unwrap();
            graph.push(Some(&mut frame)).unwrap();
            let mut output = Frame::new();
            assert!(graph.pull(&mut output).unwrap());
            assert_eq!(output.get().color_trc, ffi::AVCOL_TRC_BT709);
            assert_eq!(output.get().color_primaries, ffi::AVCOL_PRI_BT709);
            assert_eq!(output.get().colorspace, ffi::AVCOL_SPC_BT709);
            assert_eq!(output.get().color_range, ffi::AVCOL_RANGE_MPEG);
        }
    }
}
