// SPDX-License-Identifier: AGPL-3.0-or-later

//! mpv's frames in wgpu with no copy (#86). A thread of its own holds a surfaceless EGL context on
//! the GPU wgpu's Vulkan device is on, and mpv's OpenGL renderer draws there into a ring of three
//! images. Their memory is allocated in Vulkan, exported (`VK_KHR_external_memory_fd`) and
//! imported into OpenGL (`GL_EXT_memory_object_fd`), and the same images are wrapped back into
//! wgpu textures. That works on Mesa and on NVIDIA, unlike exporting dmabufs from EGL.
//!
//! Sync is as simple as it gets for now: `glFinish` before a frame is handed over.

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::ptr;
use std::sync::{Arc, Condvar, LazyLock, Mutex};

use ash::{khr, vk};
use wgpu::hal::api::Vulkan;

use crate::platform::{VideoFrame, VideoSurface};

/// How many frames go round: the one on screen, the newest finished one and the one being drawn.
const RING: usize = 3;

pub struct Surface {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    /// The ring's textures, all of `size`.
    textures: Vec<wgpu::Texture>,
    size: (u32, u32),
    wanted: (u32, u32),
    /// The newest finished frame and the one the UI took last, as indices into `textures`.
    ready: Option<usize>,
    shown: Option<usize>,
    /// Counts the frames rendered.
    frames: u64,
    shown_id: u64,
    /// mpv has a new frame.
    update: bool,
    /// Frames that reached the screen since mpv was last told.
    swaps: u32,
    quit: bool,
    failure: Option<String>,
}

impl Surface {
    pub fn start(device: &wgpu::Device, mpv: &mpv::Mpv, wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self, String> {
        let gpu = Gpu::new(device)?;
        let shared = Arc::new(Shared { state: Mutex::default(), changed: Condvar::new() });
        let (thread_shared, mpv, device) = (shared.clone(), mpv.clone(), device.clone());
        // mpv needs its renderer before it opens a file, so this waits until it's there.
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("video".into())
            .spawn(move || {
                if let Err(error) = render(&thread_shared, &mpv, &device, &gpu, &*wake, &ready) {
                    tracing::error!("can't draw video: {error}");
                    thread_shared.state.lock().unwrap().failure = Some(error.clone());
                    let _ = ready.try_send(Err(error));
                    wake();
                }
            })
            .map_err(|error| error.to_string())?;
        let surface = Self { shared, thread: Some(thread) };
        started.recv().map_err(|_| "the video thread stopped".to_owned())??;
        Ok(surface)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap().quit = true;
        self.shared.changed.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl VideoSurface for Surface {
    fn resize(&self, width: u32, height: u32) {
        let mut state = self.shared.state.lock().unwrap();
        if state.wanted != (width, height) {
            state.wanted = (width, height);
            self.shared.changed.notify_all();
        }
    }

    fn frame(&self) -> Option<VideoFrame> {
        let mut state = self.shared.state.lock().unwrap();
        if let Some(ready) = state.ready.take() {
            state.shown = Some(ready);
            state.shown_id = state.frames;
            // The frame that was shown is free to be drawn into again.
            self.shared.changed.notify_all();
        }
        let texture = state.textures.get(state.shown?)?.clone();
        Some(VideoFrame { texture, id: state.shown_id })
    }

    fn presented(&self) {
        self.shared.state.lock().unwrap().swaps += 1;
        self.shared.changed.notify_all();
    }

    fn failure(&self) -> Option<String> {
        self.shared.state.lock().unwrap().failure.clone()
    }
}

/// What the render thread needs from wgpu's Vulkan device.
struct Gpu {
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    memory_fd: khr::external_memory_fd::Device,
    uuid: [u8; 16],
}

impl Gpu {
    fn new(device: &wgpu::Device) -> Result<Self, String> {
        // SAFETY: only used to clone handles out of; the guard is dropped at the end.
        let hal = unsafe { device.as_hal::<Vulkan>() }.ok_or("the GPU device isn't a Vulkan one")?;
        if !hal.enabled_device_extensions().contains(&khr::external_memory_fd::NAME) {
            return Err("the GPU driver can't share memory with OpenGL (VK_KHR_external_memory_fd)".into());
        }
        let instance = hal.shared_instance().raw_instance().clone();
        let physical = hal.raw_physical_device();
        let raw = hal.raw_device().clone();
        let memory_fd = khr::external_memory_fd::Device::new(&instance, &raw);

        let mut id = vk::PhysicalDeviceIDProperties::default();
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
        unsafe { instance.get_physical_device_properties2(physical, &mut properties) };
        let uuid = id.device_uuid;

        Ok(Self { instance, physical, device: raw, memory_fd, uuid })
    }

    fn memory_type(&self, bits: u32) -> Option<u32> {
        let properties = unsafe { self.instance.get_physical_device_memory_properties(self.physical) };
        properties
            .memory_types_as_slice()
            .iter()
            .enumerate()
            .find(|(i, kind)| {
                bits & (1 << i) != 0 && kind.property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .map(|(i, _)| i as u32)
    }
}

fn render(
    shared: &Arc<Shared>,
    mpv: &mpv::Mpv,
    device: &wgpu::Device,
    gpu: &Gpu,
    wake: &(dyn Fn() + Send + Sync),
    ready: &std::sync::mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let egl = Egl::new(&gpu.uuid)?;
    let gl = Gl::load()?;
    // VA-API decodes on the same GPU, through its render node.
    let node = egl.render_node.as_deref().and_then(|path| File::options().read(true).write(true).open(path).ok());

    let lookup = |name: &CStr| egl.proc(name);
    let init = mpv::GlInit { get_proc_address: &lookup, drm_render_fd: node.as_ref().map(AsRawFd::as_raw_fd) };
    let mut context = mpv::RenderContext::new_gl(mpv, init).map_err(|error| error.to_string())?;
    let update = shared.clone();
    context.set_update_callback(move || {
        update.state.lock().unwrap().update = true;
        update.changed.notify_all();
    });
    let _ = ready.try_send(Ok(()));

    let mut slots: Vec<Slot> = Vec::new();
    let mut redraw = false;
    loop {
        let (wanted, swaps, frame, target) = {
            let mut state = shared.state.lock().unwrap();
            loop {
                if state.quit {
                    drop(state);
                    for slot in slots.drain(..) {
                        slot.free(&gl);
                    }
                    drop(context);
                    return Ok(());
                }
                let free = (0..slots.len()).find(|i| state.ready != Some(*i) && state.shown != Some(*i));
                if state.swaps > 0 || state.wanted != state.size || ((state.update || redraw) && free.is_some()) {
                    break;
                }
                state = shared.changed.wait(state).unwrap();
            }
            let free = (0..slots.len()).find(|i| state.ready != Some(*i) && state.shown != Some(*i));
            (state.wanted, std::mem::take(&mut state.swaps), std::mem::take(&mut state.update), free)
        };

        for _ in 0..swaps {
            context.report_swap();
        }

        if wanted != (slots.first().map_or((0, 0), |slot| (slot.width, slot.height))) {
            for slot in slots.drain(..) {
                slot.free(&gl);
            }
            let (width, height) = wanted;
            let mut textures = Vec::new();
            if width > 0 && height > 0 {
                for _ in 0..RING {
                    let (slot, texture) = Slot::new(gpu, device, &gl, width, height)?;
                    slots.push(slot);
                    textures.push(texture);
                }
            }
            let mut state = shared.state.lock().unwrap();
            state.textures = textures;
            state.size = wanted;
            state.ready = None;
            state.shown = None;
            // Draw the frame that's up again, at the new size.
            redraw = true;
            continue;
        }

        let update = frame && context.update();
        if !(update || redraw) {
            continue;
        }
        let Some(target) = target else {
            // Every slot is busy; draw once the UI takes the newest one.
            redraw = true;
            continue;
        };
        redraw = false;

        // A slot stops being shown before the UI starts its next frame. Finish the preceding
        // Vulkan submission before OpenGL reuses its memory.
        device
            .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(5)) })
            .map_err(|error| format!("waiting for video memory: {error}"))?;
        let slot = &slots[target];
        context
            .render(slot.framebuffer as i32, slot.width as i32, slot.height as i32, false)
            .map_err(|error| error.to_string())?;
        unsafe { (gl.finish)() };

        let mut state = shared.state.lock().unwrap();
        state.ready = Some(target);
        state.frames += 1;
        if state.frames == 1 {
            tracing::debug!("first video frame drawn, {}×{}", slot.width, slot.height);
        }
        drop(state);
        wake();
    }
}

/// One frame of the ring: a Vulkan image, seen by OpenGL as a framebuffer.
struct Slot {
    width: u32,
    height: u32,
    memory_object: c_uint,
    texture: c_uint,
    framebuffer: c_uint,
}

impl Slot {
    fn new(
        gpu: &Gpu,
        device: &wgpu::Device,
        gl: &Gl,
        width: u32,
        height: u32,
    ) -> Result<(Self, wgpu::Texture), String> {
        let handle = vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;
        let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(handle);
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external);
        let image =
            unsafe { gpu.device.create_image(&info, None) }.map_err(|error| format!("vkCreateImage: {error}"))?;

        let requirements = unsafe { gpu.device.get_image_memory_requirements(image) };
        let Some(kind) = gpu.memory_type(requirements.memory_type_bits) else {
            unsafe { gpu.device.destroy_image(image, None) };
            return Err("no GPU memory for video frames".into());
        };
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut export = vk::ExportMemoryAllocateInfo::default().handle_types(handle);
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(kind)
            .push_next(&mut export)
            .push_next(&mut dedicated);
        let memory = match unsafe { gpu.device.allocate_memory(&allocation, None) } {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { gpu.device.destroy_image(image, None) };
                return Err(format!("vkAllocateMemory: {error}"));
            },
        };
        let fd = unsafe { gpu.device.bind_image_memory(image, memory, 0) }.and_then(|()| unsafe {
            gpu.memory_fd.get_memory_fd(&vk::MemoryGetFdInfoKHR::default().memory(memory).handle_type(handle))
        });
        let fd = match fd {
            Ok(fd) => fd,
            Err(error) => {
                unsafe {
                    gpu.device.destroy_image(image, None);
                    gpu.device.free_memory(memory, None);
                }
                return Err(format!("can't share video memory: {error}"));
            },
        };

        // wgpu owns the image and its memory from here, and frees them once the GPU is done.
        let descriptor = wgpu::TextureDescriptor {
            label: Some("tinystream video"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let texture = {
            let hal = unsafe { device.as_hal::<Vulkan>() }.ok_or("the GPU device isn't a Vulkan one")?;
            let raw = unsafe {
                hal.texture_from_raw(
                    image,
                    &wgpu::hal::TextureDescriptor {
                        label: descriptor.label,
                        size: descriptor.size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: descriptor.dimension,
                        format: descriptor.format,
                        usage: wgpu::TextureUses::RESOURCE | wgpu::TextureUses::COPY_SRC,
                        memory_flags: wgpu::hal::MemoryFlags::empty(),
                        view_formats: Vec::new(),
                    },
                    None,
                    wgpu::hal::vulkan::TextureMemory::Dedicated(memory),
                )
            };
            drop(hal);
            unsafe { device.create_texture_from_hal::<Vulkan>(raw, &descriptor, wgpu::TextureUses::RESOURCE) }
        };

        let (mut memory_object, mut gl_texture, mut framebuffer) = (0, 0, 0);
        unsafe {
            (gl.create_memory_objects)(1, &mut memory_object);
            let dedicated = 1;
            (gl.memory_object_parameteriv)(memory_object, GL_DEDICATED_MEMORY_OBJECT_EXT, &dedicated);
            // OpenGL owns the descriptor now.
            (gl.import_memory_fd)(memory_object, requirements.size, GL_HANDLE_TYPE_OPAQUE_FD_EXT, fd);
            (gl.gen_textures)(1, &mut gl_texture);
            (gl.bind_texture)(GL_TEXTURE_2D, gl_texture);
            (gl.tex_parameteri)(GL_TEXTURE_2D, GL_TEXTURE_TILING_EXT, GL_OPTIMAL_TILING_EXT as c_int);
            (gl.tex_storage_mem_2d)(GL_TEXTURE_2D, 1, GL_RGBA8, width as c_int, height as c_int, memory_object, 0);
            (gl.gen_framebuffers)(1, &mut framebuffer);
            (gl.bind_framebuffer)(GL_FRAMEBUFFER, framebuffer);
            (gl.framebuffer_texture_2d)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, gl_texture, 0);
            let status = (gl.check_framebuffer_status)(GL_FRAMEBUFFER);
            (gl.bind_framebuffer)(GL_FRAMEBUFFER, 0);
            let slot = Slot { width, height, memory_object, texture: gl_texture, framebuffer };
            if status != GL_FRAMEBUFFER_COMPLETE {
                slot.free(gl);
                return Err(format!("OpenGL can't draw into shared video memory ({status:#x})"));
            }
            Ok((slot, texture))
        }
    }

    fn free(self, gl: &Gl) {
        unsafe {
            (gl.delete_framebuffers)(1, &self.framebuffer);
            (gl.delete_textures)(1, &self.texture);
            (gl.delete_memory_objects)(1, &self.memory_object);
        }
    }
}

const EGL_NONE: c_int = 0x3038;
const EGL_EXTENSIONS: c_int = 0x3055;
const EGL_SURFACE_TYPE: c_int = 0x3033;
const EGL_RENDERABLE_TYPE: c_int = 0x3040;
const EGL_OPENGL_BIT: c_int = 0x0008;
const EGL_OPENGL_API: c_uint = 0x30A2;
const EGL_PLATFORM_DEVICE_EXT: c_uint = 0x313F;
const EGL_PLATFORM_SURFACELESS_MESA: c_uint = 0x31DD;
const EGL_DRM_RENDER_NODE_FILE_EXT: c_int = 0x3377;
const EGL_DEVICE_EXT: c_int = 0x322C;

const GL_FRAMEBUFFER: c_uint = 0x8D40;
const GL_FRAMEBUFFER_COMPLETE: c_uint = 0x8CD5;
const GL_COLOR_ATTACHMENT0: c_uint = 0x8CE0;
const GL_TEXTURE_2D: c_uint = 0x0DE1;
const GL_RGBA8: c_uint = 0x8058;
const GL_NUM_EXTENSIONS: c_uint = 0x821D;
const GL_EXTENSIONS: c_uint = 0x1F03;
const GL_TEXTURE_TILING_EXT: c_uint = 0x9580;
const GL_DEDICATED_MEMORY_OBJECT_EXT: c_uint = 0x9581;
const GL_OPTIMAL_TILING_EXT: c_uint = 0x9584;
const GL_HANDLE_TYPE_OPAQUE_FD_EXT: c_uint = 0x9586;
const GL_DEVICE_UUID_EXT: c_uint = 0x9597;

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetPlatformDisplay(platform: c_uint, native: *mut c_void, attribs: *const isize) -> *mut c_void;
    fn eglInitialize(display: *mut c_void, major: *mut c_int, minor: *mut c_int) -> c_uint;
    fn eglBindAPI(api: c_uint) -> c_uint;
    fn eglQueryString(display: *mut c_void, name: c_int) -> *const c_char;
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

type QueryDevices = unsafe extern "C" fn(c_int, *mut *mut c_void, *mut c_int) -> c_uint;
type QueryDeviceString = unsafe extern "C" fn(*mut c_void, c_int) -> *const c_char;
type QueryDisplay = unsafe extern "C" fn(*mut c_void, c_int, *mut isize) -> c_uint;

fn render_node(string: QueryDeviceString, device: *mut c_void) -> Option<String> {
    let node = unsafe { string(device, EGL_DRM_RENDER_NODE_FILE_EXT) };
    (!node.is_null()).then(|| unsafe { CStr::from_ptr(node) }.to_string_lossy().into_owned())
}

static DISPLAYS: LazyLock<Mutex<HashMap<usize, usize>>> = LazyLock::new(Mutex::default);

struct Display(*mut c_void);

impl Display {
    fn open(platform: c_uint, native: *mut c_void) -> Option<Self> {
        let raw = unsafe { eglGetPlatformDisplay(platform, native, ptr::null()) };
        if raw.is_null() {
            return None;
        }
        let mut displays = DISPLAYS.lock().unwrap();
        let users = displays.entry(raw as usize).or_default();
        if *users == 0 && unsafe { eglInitialize(raw, ptr::null_mut(), ptr::null_mut()) } != 1 {
            displays.remove(&(raw as usize));
            return None;
        }
        *users += 1;
        Some(Self(raw))
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        let mut displays = DISPLAYS.lock().unwrap();
        let users = displays.get_mut(&(self.0 as usize)).unwrap();
        *users -= 1;
        if *users == 0 {
            displays.remove(&(self.0 as usize));
            unsafe { eglTerminate(self.0) };
        }
    }
}

/// A surfaceless OpenGL context, current on the thread that made it, on the GPU with `uuid`.
struct Egl {
    display: Display,
    context: *mut c_void,
    render_node: Option<String>,
}

impl Egl {
    fn new(uuid: &[u8; 16]) -> Result<Self, String> {
        let client = unsafe { eglQueryString(ptr::null_mut(), EGL_EXTENSIONS) };
        let client = if client.is_null() { "" } else { unsafe { CStr::from_ptr(client) }.to_str().unwrap_or("") };
        let has = |name: &str| client.split(' ').any(|extension| extension == name);

        // Mesa's surfaceless display supports VA-API image imports. Device displays also work
        // on drivers without that platform, including NVIDIA's.
        let mut candidates = Vec::new();
        if has("EGL_MESA_platform_surfaceless") {
            candidates.push((EGL_PLATFORM_SURFACELESS_MESA, ptr::null_mut(), None));
        }
        let string: Option<QueryDeviceString> = proc(c"eglQueryDeviceStringEXT");
        if has("EGL_EXT_device_enumeration") && has("EGL_EXT_platform_device") {
            let query: Option<QueryDevices> = proc(c"eglQueryDevicesEXT");
            if let (Some(query), Some(string)) = (query, string) {
                let mut devices = [ptr::null_mut(); 16];
                let mut count = 0;
                unsafe { query(devices.len() as c_int, devices.as_mut_ptr(), &mut count) };
                for &device in &devices[..count.clamp(0, 16) as usize] {
                    candidates.push((EGL_PLATFORM_DEVICE_EXT, device, render_node(string, device)));
                }
            }
        }

        for (platform, native, node) in candidates {
            let Some(mut egl) = (unsafe { Self::open(platform, native) }) else {
                tracing::debug!("no OpenGL context on EGL device {node:?}");
                continue;
            };
            egl.render_node = node.or_else(|| egl.device().and_then(|device| render_node(string?, device)));
            match egl.device_uuid() {
                Some(found) if found == *uuid => return Ok(egl),
                found => tracing::debug!("EGL device {:?} is {found:02x?}, not {uuid:02x?}", egl.render_node),
            }
        }
        Err("no OpenGL on the GPU the window is drawn with".into())
    }

    unsafe fn open(platform: c_uint, native: *mut c_void) -> Option<Self> {
        unsafe {
            let display = Display::open(platform, native)?;
            if eglBindAPI(EGL_OPENGL_API) != 1 {
                return None;
            }
            // Any surface type: there's no window here, which is what EGL picks otherwise.
            let attribs = [EGL_SURFACE_TYPE, 0, EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT, EGL_NONE];
            let (mut config, mut count) = (ptr::null_mut(), 0);
            eglChooseConfig(display.0, attribs.as_ptr(), &mut config, 1, &mut count);
            let context = if count == 1 {
                eglCreateContext(display.0, config, ptr::null_mut(), [EGL_NONE].as_ptr())
            } else {
                ptr::null_mut()
            };
            if context.is_null() || eglMakeCurrent(display.0, ptr::null_mut(), ptr::null_mut(), context) != 1 {
                if !context.is_null() {
                    eglDestroyContext(display.0, context);
                }
                return None;
            }
            Some(Egl { display, context, render_node: None })
        }
    }

    fn device(&self) -> Option<*mut c_void> {
        let query: QueryDisplay = proc(c"eglQueryDisplayAttribEXT")?;
        let mut device = 0isize;
        (unsafe { query(self.display.0, EGL_DEVICE_EXT, &mut device) } == 1 && device != 0)
            .then_some(device as *mut c_void)
    }

    fn proc(&self, name: &CStr) -> *mut c_void {
        unsafe { eglGetProcAddress(name.as_ptr()) }
    }

    /// The GPU this context is on, as Vulkan names it (`GL_EXT_memory_object`).
    fn device_uuid(&self) -> Option<[u8; 16]> {
        let get: unsafe extern "C" fn(c_uint, c_uint, *mut u8) = proc(c"glGetUnsignedBytei_vEXT")?;
        let mut uuid = [0; 16];
        unsafe { get(GL_DEVICE_UUID_EXT, 0, uuid.as_mut_ptr()) };
        (uuid != [0; 16]).then_some(uuid)
    }
}

impl Drop for Egl {
    fn drop(&mut self) {
        unsafe {
            eglMakeCurrent(self.display.0, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            eglDestroyContext(self.display.0, self.context);
        }
    }
}

fn proc<T>(name: &CStr) -> Option<T> {
    let address = unsafe { eglGetProcAddress(name.as_ptr()) };
    (!address.is_null()).then(|| unsafe { std::mem::transmute_copy(&address) })
}

/// The OpenGL functions the ring needs.
struct Gl {
    finish: unsafe extern "C" fn(),
    create_memory_objects: unsafe extern "C" fn(c_int, *mut c_uint),
    delete_memory_objects: unsafe extern "C" fn(c_int, *const c_uint),
    memory_object_parameteriv: unsafe extern "C" fn(c_uint, c_uint, *const c_int),
    import_memory_fd: unsafe extern "C" fn(c_uint, u64, c_uint, c_int),
    tex_storage_mem_2d: unsafe extern "C" fn(c_uint, c_int, c_uint, c_int, c_int, c_uint, u64),
    gen_textures: unsafe extern "C" fn(c_int, *mut c_uint),
    delete_textures: unsafe extern "C" fn(c_int, *const c_uint),
    bind_texture: unsafe extern "C" fn(c_uint, c_uint),
    tex_parameteri: unsafe extern "C" fn(c_uint, c_uint, c_int),
    gen_framebuffers: unsafe extern "C" fn(c_int, *mut c_uint),
    delete_framebuffers: unsafe extern "C" fn(c_int, *const c_uint),
    bind_framebuffer: unsafe extern "C" fn(c_uint, c_uint),
    framebuffer_texture_2d: unsafe extern "C" fn(c_uint, c_uint, c_uint, c_uint, c_int),
    check_framebuffer_status: unsafe extern "C" fn(c_uint) -> c_uint,
}

impl Gl {
    /// Loads them from the context that's current.
    fn load() -> Result<Self, String> {
        let get_integerv: unsafe extern "C" fn(c_uint, *mut c_int) =
            proc(c"glGetIntegerv").ok_or("OpenGL has no glGetIntegerv")?;
        let get_stringi: unsafe extern "C" fn(c_uint, c_uint) -> *const c_char =
            proc(c"glGetStringi").ok_or("OpenGL has no glGetStringi")?;
        let mut count = 0;
        unsafe { get_integerv(GL_NUM_EXTENSIONS, &mut count) };
        let extensions: Vec<String> = (0..count.max(0) as c_uint)
            .filter_map(|i| {
                let name = unsafe { get_stringi(GL_EXTENSIONS, i) };
                (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned())
            })
            .collect();
        if !extensions.iter().any(|name| name == "GL_EXT_memory_object_fd") {
            return Err("OpenGL can't use memory from Vulkan (GL_EXT_memory_object_fd)".into());
        }

        macro_rules! load {
            ($name:literal) => {
                proc($name).ok_or_else(|| format!("OpenGL has no {}", $name.to_string_lossy()))?
            };
        }

        Ok(Self {
            finish: load!(c"glFinish"),
            create_memory_objects: load!(c"glCreateMemoryObjectsEXT"),
            delete_memory_objects: load!(c"glDeleteMemoryObjectsEXT"),
            memory_object_parameteriv: load!(c"glMemoryObjectParameterivEXT"),
            import_memory_fd: load!(c"glImportMemoryFdEXT"),
            tex_storage_mem_2d: load!(c"glTexStorageMem2DEXT"),
            gen_textures: load!(c"glGenTextures"),
            delete_textures: load!(c"glDeleteTextures"),
            bind_texture: load!(c"glBindTexture"),
            tex_parameteri: load!(c"glTexParameteri"),
            gen_framebuffers: load!(c"glGenFramebuffers"),
            delete_framebuffers: load!(c"glDeleteFramebuffers"),
            bind_framebuffer: load!(c"glBindFramebuffer"),
            framebuffer_texture_2d: load!(c"glFramebufferTexture2D"),
            check_framebuffer_status: load!(c"glCheckFramebufferStatus"),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    #[ignore = "needs Vulkan/OpenGL external memory and a hardware decoder"]
    fn shares_decoded_frames_with_vulkan_across_players() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = futures::executor::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) = super::super::gpu::request_device(&adapter, &Default::default()).unwrap();
        let clip = std::env::var("TINYSTREAM_TEST_VIDEO")
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../mpv/tests/data/testsrc.mp4").into());
        let open = || {
            let mpv = mpv::Mpv::new(&[("vo", "libmpv"), ("ao", "null"), ("hwdec", "auto-safe"), ("keep-open", "yes")])
                .unwrap();
            let surface = Surface::start(&device, &mpv, Arc::new(|| {})).unwrap();
            surface.resize(64, 36);
            mpv.request_log_messages("error").unwrap();
            mpv.command(&["loadfile", &clip]).unwrap();
            (mpv, surface)
        };
        let pixels = |mpv: &mpv::Mpv, surface: &Surface, after: u64| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                assert!(surface.failure().is_none(), "{:?}", surface.failure());
                while let Some(event) = mpv.wait_event(0.0) {
                    match event {
                        mpv::Event::EndFile(mpv::EndReason::Error(error)) => panic!("{error}"),
                        mpv::Event::Log { text, .. }
                            if text.contains("Mapping hardware") || text.contains("import surface") =>
                        {
                            panic!("{text}")
                        },
                        _ => {},
                    }
                }
                if let Some(frame) = surface.frame().filter(|frame| frame.id > after) {
                    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: None,
                        size: 256 * 36,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    });
                    let mut encoder = device.create_command_encoder(&Default::default());
                    encoder.copy_texture_to_buffer(
                        frame.texture.as_image_copy(),
                        wgpu::TexelCopyBufferInfo {
                            buffer: &buffer,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(256),
                                rows_per_image: None,
                            },
                        },
                        wgpu::Extent3d { width: 64, height: 36, depth_or_array_layers: 1 },
                    );
                    queue.submit([encoder.finish()]);
                    let (send, recv) = std::sync::mpsc::channel();
                    buffer.map_async(wgpu::MapMode::Read, .., move |result| send.send(result).unwrap());
                    device
                        .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(Duration::from_secs(5)) })
                        .unwrap();
                    recv.recv().unwrap().unwrap();
                    let colored = buffer
                        .get_mapped_range(..)
                        .unwrap()
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|pixel| pixel[0].abs_diff(pixel[1]) > 32 || pixel[1].abs_diff(pixel[2]) > 32)
                        .count();
                    surface.presented();
                    if colored > 100 {
                        assert_ne!(mpv.get_property::<String>("hwdec-current").unwrap(), "no");
                        return frame.id;
                    }
                }
                assert!(Instant::now() < deadline, "no colored frame reached Vulkan");
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        let (first_mpv, first) = open();
        pixels(&first_mpv, &first, 0);
        let (second_mpv, second) = open();
        let before = pixels(&second_mpv, &second, 0);
        drop(first);
        drop(first_mpv);
        second_mpv.command(&["loadfile", &clip]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !matches!(second_mpv.wait_event(0.1), Some(mpv::Event::FileLoaded)) {
            assert!(Instant::now() < deadline, "second file did not load");
        }
        let before = before.max(second.shared.state.lock().unwrap().frames);
        let mut before = pixels(&second_mpv, &second, before);
        for size in [(128, 72), (64, 36)] {
            second.resize(size.0, size.1);
            let deadline = Instant::now() + Duration::from_secs(5);
            while second.shared.state.lock().unwrap().size != size {
                assert!(Instant::now() < deadline, "video textures were not resized");
                std::thread::sleep(Duration::from_millis(10));
            }
            before = pixels(&second_mpv, &second, before);
        }
    }
}
