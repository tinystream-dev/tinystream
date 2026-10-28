// SPDX-License-Identifier: AGPL-3.0-or-later

//! mpv's picture (the `<video>` in `Player.tsx`), letterboxed at the stage's physical pixel size,
//! with the light it spills around it (`Ambilight`) and the frame receding while paused
//! (`.recede`). The platform's [`VideoSurface`] hands over the frames.

use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use iced_core::layout::{self, Layout};
use iced_core::widget::Tree;
use iced_core::{Length, Rectangle, Size, Theme, Widget, mouse, renderer};
use iced_wgpu::graphics::Viewport;
use iced_wgpu::primitive::{self, Primitive, Renderer as _};
use iced_wgpu::wgpu;
use iced_widget::Renderer;

use crate::platform::{self, Desktop, VideoSurface};

/// The ambient light's size, as in `video.wgsl`.
const AMBIENT: (u32, u32) = (32, 18);
/// How often the ambient light takes in the picture (the web's interval).
const SAMPLE: Duration = Duration::from_millis(150);

/// What a player draws its video with. Its surface has to be there before mpv opens a file, so it
/// starts with the screen, on the window's GPU device.
pub struct Screen {
    mpv: mpv::Mpv,
    surface: OnceLock<Result<Box<dyn VideoSurface>, String>>,
}

impl fmt::Debug for Screen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Screen")
    }
}

impl Screen {
    pub fn new(mpv: mpv::Mpv) -> Arc<Self> {
        let screen = Arc::new(Self { mpv, surface: OnceLock::new() });
        if let Some(device) = DEVICE.get() {
            screen.surface(device);
        }
        screen
    }

    /// Why the video can't be drawn, if it can't.
    pub fn failure(&self) -> Option<String> {
        match self.surface.get()? {
            Ok(surface) => surface.failure(),
            Err(error) => Some(error.clone()),
        }
    }

    fn surface(&self, device: &wgpu::Device) -> Option<&dyn VideoSurface> {
        let surface = self.surface.get_or_init(|| {
            let wake = WAKE.get().cloned().unwrap_or_else(|| Arc::new(|| {}));
            platform::Current::video(device, &self.mpv, wake)
                .inspect_err(|error| tracing::error!("can't show video: {error}"))
        });
        surface.as_ref().ok().map(|surface| &**surface)
    }
}

static WAKE: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();
static DEVICE: OnceLock<wgpu::Device> = OnceLock::new();
/// The screens drawn into the frame being made, told once it's on screen.
static DRAWN: Mutex<Vec<Arc<Screen>>> = Mutex::new(Vec::new());

/// Sets the window's GPU device, and what asks it for a redraw when a frame is ready, from any
/// thread.
pub fn set_window(device: wgpu::Device, wake: Arc<dyn Fn() + Send + Sync>) {
    let _ = DEVICE.set(device);
    let _ = WAKE.set(wake);
}

/// Says the frame just presented reached the screen.
pub fn presented() {
    let drawn = std::mem::take(&mut *DRAWN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
    for screen in drawn {
        if let Some(Ok(surface)) = screen.surface.get() {
            surface.presented();
        }
    }
}

/// `screen`'s video, filling what it's given; `aspect` is the picture's width over its height.
pub fn video(screen: &Arc<Screen>, aspect: Option<f32>) -> Video {
    Video { screen: screen.clone(), aspect, recede: 0.0, ambient: false, paused: false }
}

pub struct Video {
    screen: Arc<Screen>,
    aspect: Option<f32>,
    recede: f32,
    ambient: bool,
    paused: bool,
}

impl Video {
    /// How far the frame has receded behind what's playing, from 0 to 1.
    pub fn recede(mut self, amount: f32) -> Self {
        self.recede = amount.clamp(0.0, 1.0);
        self
    }

    /// Spills the picture's light around it; a paused one takes it in at once rather than
    /// glowing over.
    pub fn ambient(mut self, on: bool, paused: bool) -> Self {
        self.ambient = on;
        self.paused = paused;
        self
    }
}

impl iced_core::widget::Meta for Video {}

impl<Message> Widget<Message, Theme, Renderer> for Video {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        renderer.draw_primitive(
            layout.bounds(),
            Picture {
                screen: self.screen.clone(),
                aspect: self.aspect,
                recede: self.recede,
                ambient: self.ambient,
                paused: self.paused,
            },
        );
    }
}

#[derive(Debug)]
struct Picture {
    screen: Arc<Screen>,
    aspect: Option<f32>,
    recede: f32,
    ambient: bool,
    paused: bool,
}

/// The largest `aspect` rectangle in `stage`, centred, on whole pixels (`object-fit: contain`).
fn contain(stage: Rectangle, aspect: f32) -> Rectangle {
    let (width, height) = if stage.width / stage.height > aspect {
        (stage.height * aspect, stage.height)
    } else {
        (stage.width, stage.width / aspect)
    };
    let (width, height) = (width.round().max(1.0), height.round().max(1.0));
    Rectangle {
        x: (stage.x + (stage.width - width) / 2.0).round(),
        y: (stage.y + (stage.height - height) / 2.0).round(),
        width,
        height,
    }
}

impl Primitive for Picture {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        pipeline.prepared = None;
        let Some(aspect) = self.aspect.filter(|aspect| aspect.is_finite() && *aspect > 0.0) else { return };
        let Some(surface) = self.screen.surface(device) else { return };
        let scale = viewport.scale_factor();
        let target = viewport.physical_size();
        let stage = Rectangle {
            x: bounds.x * scale,
            y: bounds.y * scale,
            width: bounds.width * scale,
            height: bounds.height * scale,
        };
        if stage.width < 1.0 || stage.height < 1.0 {
            return;
        }
        let fitted = contain(stage, aspect);
        surface.resize(fitted.width as u32, fitted.height as u32);
        let Some(frame) = surface.frame() else { return };
        DRAWN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(self.screen.clone());

        let view = frame.texture.create_view(&Default::default());
        let group = |buffer: &wgpu::Buffer, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tinystream video"),
                layout: &pipeline.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&pipeline.sampler) },
                ],
            })
        };
        let linear = if pipeline.linear { 1.0 } else { 0.0 };

        // The frame grows a little as it recedes (`scale(1.015)`).
        let grow = 1.0 + 0.015 * self.recede;
        let shown = Rectangle {
            x: fitted.center_x() - fitted.width * grow / 2.0,
            y: fitted.center_y() - fitted.height * grow / 2.0,
            width: fitted.width * grow,
            height: fitted.height * grow,
        };
        let params = |rect: Rectangle, size: (f32, f32), opacity: f32, blend: f32| {
            bytes(&[
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                size.0,
                size.1,
                self.recede,
                6.0 * scale,
                opacity,
                linear,
                blend,
                0.0,
            ])
        };
        let size = (target.width as f32, target.height as f32);
        queue.write_buffer(&pipeline.params[0], 0, &params(shown, size, 1.0, 0.0));
        let picture = group(&pipeline.params[0], &view);

        let ambient = self.ambient.then(|| {
            let small = Rectangle { x: 0.0, y: 0.0, width: AMBIENT.0 as f32, height: AMBIENT.1 as f32 };
            let due = pipeline.sampled.is_none_or(|(at, id)| id != frame.id && at.elapsed() >= SAMPLE);
            if due || self.paused && pipeline.sampled.is_some_and(|(_, id)| id != frame.id) {
                pipeline.sampled = Some((Instant::now(), frame.id));
                // Each sample is blended over the last, so a cut glows over rather than flashing.
                let blend = if self.paused || !pipeline.warm { 1.0 } else { 0.3 };
                pipeline.warm = true;
                queue.write_buffer(
                    &pipeline.params[1],
                    0,
                    &params(small, (AMBIENT.0 as f32, AMBIENT.1 as f32), 1.0, blend),
                );
                let mut encoder = device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tinystream ambient") });
                for (pipe, from, into) in [
                    (&pipeline.accumulate, group(&pipeline.params[1], &view), &pipeline.light[0]),
                    (&pipeline.blur, group(&pipeline.params[1], &pipeline.light[0]), &pipeline.light[1]),
                ] {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("tinystream ambient"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: into,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_pipeline(pipe);
                    pass.set_blend_constant(wgpu::Color { r: blend as f64, g: blend as f64, b: blend as f64, a: 1.0 });
                    pass.set_bind_group(0, &from, &[]);
                    pass.draw(0..4, 0..1);
                }
                queue.submit([encoder.finish()]);
            }
            // `scale-110`, around the stage's middle.
            let wide = Rectangle {
                x: stage.center_x() - stage.width * 0.55,
                y: stage.center_y() - stage.height * 0.55,
                width: stage.width * 1.1,
                height: stage.height * 1.1,
            };
            queue.write_buffer(&pipeline.params[2], 0, &params(wide, size, 0.55, 0.0));
            group(&pipeline.params[2], &pipeline.light[1])
        });
        if !self.ambient {
            pipeline.warm = false;
        }
        pipeline.prepared = Some(Prepared { picture, ambient });
    }

    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(prepared) = &pipeline.prepared else { return };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tinystream video"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_scissor_rect(clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height);
        if let Some(ambient) = &prepared.ambient {
            pass.set_pipeline(&pipeline.ambient);
            pass.set_bind_group(0, ambient, &[]);
            pass.draw(0..4, 0..1);
        }
        pass.set_pipeline(&pipeline.video);
        pass.set_bind_group(0, &prepared.picture, &[]);
        pass.draw(0..4, 0..1);
    }
}

fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

struct Prepared {
    picture: wgpu::BindGroup,
    ambient: Option<wgpu::BindGroup>,
}

pub struct Pipeline {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    video: wgpu::RenderPipeline,
    ambient: wgpu::RenderPipeline,
    accumulate: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    /// The picture's, the ambient light's samples' and the ambient light's parameters.
    params: [wgpu::Buffer; 3],
    /// The ambient light as sampled, and blurred.
    light: [wgpu::TextureView; 2],
    /// When the ambient light last took in the picture, and which frame.
    sampled: Option<(Instant, u64)>,
    /// Whether the ambient light has something in it to blend over.
    warm: bool,
    linear: bool,
    prepared: Option<Prepared>,
}

impl primitive::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tinystream video"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tinystream video"),
            source: wgpu::ShaderSource::Wgsl(include_str!("video.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tinystream video"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str, format: wgpu::TextureFormat, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tinystream video"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..wgpu::PrimitiveState::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let light = wgpu::TextureFormat::Rgba8Unorm;
        let over = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Constant,
            dst_factor: wgpu::BlendFactor::OneMinusConstant,
            operation: wgpu::BlendOperation::Add,
        };
        let params = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream video params"),
                size: 48,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let light_views = std::array::from_fn(|_| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("tinystream ambient"),
                    size: wgpu::Extent3d { width: AMBIENT.0, height: AMBIENT.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: light,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        });
        Self {
            video: pipeline("video", format, Some(wgpu::BlendState::REPLACE)),
            ambient: pipeline("ambient", format, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)),
            accumulate: pipeline("accumulate", light, Some(wgpu::BlendState { color: over, alpha: over })),
            blur: pipeline("blur", light, None),
            layout,
            sampler,
            params,
            light: light_views,
            sampled: None,
            warm: false,
            linear: format.is_srgb(),
            prepared: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterboxes_on_whole_pixels() {
        let stage = Rectangle { x: 0.0, y: 0.0, width: 1920.0, height: 1200.0 };
        assert_eq!(contain(stage, 16.0 / 9.0), Rectangle { x: 0.0, y: 60.0, width: 1920.0, height: 1080.0 });
        let tall = Rectangle { x: 10.0, y: 0.0, width: 1000.0, height: 1000.0 };
        assert_eq!(contain(tall, 2.0), Rectangle { x: 10.0, y: 250.0, width: 1000.0, height: 500.0 });
        assert_eq!(contain(stage, 4.0 / 3.0).width, 1600.0);
    }
}
