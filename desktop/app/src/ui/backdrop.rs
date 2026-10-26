// SPDX-License-Identifier: AGPL-3.0-or-later

//! Blurring what's already drawn behind a region: Glass's `backdrop-filter`, and the blur pages
//! sharpen in from when they change. The event loop draws each frame into a texture of its own
//! ([`Frame`]) so this can read it back; where it can't, materials fall back to solid colours,
//! as the web's do without `backdrop-filter`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use iced_core::{Color, Rectangle};
use iced_wgpu::graphics::Viewport;
use iced_wgpu::primitive::{self, Primitive};
use iced_wgpu::wgpu;

/// The texture the current frame is drawn into, readable by [`Backdrop`]s.
pub struct Frame {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// Bumped whenever the texture is replaced, so bind groups on the old one are dropped.
    pub generation: u64,
}

static FRAME: Mutex<Option<Frame>> = Mutex::new(None);

fn frame() -> MutexGuard<'static, Option<Frame>> {
    FRAME.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Sets (or, with `None`, takes away) the texture frames are drawn into.
pub fn set_frame(texture: Option<wgpu::Texture>) {
    let mut frame = frame();
    let generation = frame.as_ref().map_or(0, |f| f.generation) + 1;
    *frame = texture.map(|texture| Frame { view: texture.create_view(&Default::default()), texture, generation });
}

/// Whether backdrops can be read back this frame.
pub fn available() -> bool {
    frame().is_some()
}

/// What's behind a squircle, blurred and saturated, then tinted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backdrop {
    /// The CSS blur radius (twice the standard deviation), in logical pixels.
    pub blur: f32,
    pub saturate: f32,
    /// Painted over the blur; at full alpha, only the tint shows.
    pub tint: Color,
    /// The squircle's radius and smoothing, in logical pixels.
    pub radius: f32,
    pub smoothing: f32,
    /// Shows what's behind a little bigger (above 1) around the middle.
    pub zoom: f32,
    /// How wide the rim that bends light is, in logical pixels; 0 is flat glass.
    pub bezel: f32,
    /// Tells this frame's backdrops apart between preparing and rendering them.
    id: u64,
}

static NEXT: AtomicU64 = AtomicU64::new(0);

impl Backdrop {
    pub fn new(blur: f32, saturate: f32, tint: Color, radius: f32, smoothing: f32, zoom: f32) -> Self {
        Self { blur, saturate, tint, radius, smoothing, zoom, bezel: 0.0, id: NEXT.fetch_add(1, Ordering::Relaxed) }
    }

    /// Seen through a lens with a rounded rim `bezel` wide.
    pub fn bezel(self, bezel: f32) -> Self {
        Self { bezel, ..self }
    }
}

impl Primitive for Backdrop {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let frame = frame();
        let Some(frame) = frame.as_ref() else { return };
        let scale = viewport.scale_factor();
        let size = frame.texture.size();
        let (fw, fh) = (size.width as f32, size.height as f32);

        let shape = Rectangle {
            x: bounds.x * scale,
            y: bounds.y * scale,
            width: bounds.width * scale,
            height: bounds.height * scale,
        };
        let bezel = (self.bezel * scale).min(shape.width.min(shape.height) / 2.0);
        // A standard deviation in frame pixels, and enough around the region for it to blur from,
        // past as far as the rim reaches out (at its steepest, 1.2 × tan(90° − asin(1 / 1.5))).
        let sigma = self.blur / 2.0 * scale;
        let down = (sigma / 3.0).floor().clamp(1.0, 8.0);
        let margin = (sigma * 3.0 + bezel * 1.35).ceil();
        let x0 = ((shape.x - margin).floor()).max(0.0);
        let y0 = ((shape.y - margin).floor()).max(0.0);
        let x1 = ((shape.x + shape.width + margin).ceil()).min(fw);
        let y1 = ((shape.y + shape.height + margin).ceil()).min(fh);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let texels = (((x1 - x0) / down).ceil().max(1.0) as u32, ((y1 - y0) / down).ceil().max(1.0) as u32);
        let region = [x0, y0, texels.0 as f32 * down, texels.1 as f32 * down];

        let index = pipeline.prepared.len();
        pipeline.prepared.insert(self.id, index);
        if pipeline.slots.len() <= index {
            pipeline.slots.push(Slot::new(device));
        }
        let slot = &mut pipeline.slots[index];
        slot.resize(device, texels, frame, &pipeline.layout, &pipeline.sampler);

        let tint = self.tint;
        let (corner, exponent) = corner(shape.width, shape.height, self.radius * scale, self.smoothing);
        let params = |sigma: f32, vertical: f32| -> [f32; 24] {
            [
                region[0],
                region[1],
                region[2],
                region[3],
                fw,
                fh,
                down,
                (down / 2.0).ceil(),
                sigma,
                self.saturate,
                self.zoom,
                vertical,
                tint.r,
                tint.g,
                tint.b,
                tint.a,
                shape.x,
                shape.y,
                shape.width,
                shape.height,
                corner,
                exponent,
                bezel,
                LIGHT,
            ]
        };
        let blur = sigma / down;
        for (buffer, values) in slot.params.iter().zip([params(0.0, 0.0), params(blur, 0.0), params(blur, 1.0)]) {
            queue.write_buffer(buffer, 0, &bytes(&values));
        }

        let mesh = mesh(&shape, self.radius * scale, self.smoothing);
        let data = bytes(&mesh);
        if slot.vertices.size() < data.len() as u64 {
            slot.vertices = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream backdrop shape"),
                size: (data.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&slot.vertices, 0, &data);
        slot.count = (mesh.len() / 3) as u32;
    }

    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(slot) = pipeline.prepared.get(&self.id).and_then(|&i| pipeline.slots.get(i)) else { return };
        let (Some([down, horizontal, vertical]), Some(shape)) = (&slot.groups, &slot.shape) else { return };

        let pass = |encoder: &mut wgpu::CommandEncoder,
                    view: &wgpu::TextureView,
                    pipeline: &wgpu::RenderPipeline,
                    group: &wgpu::BindGroup| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tinystream backdrop"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        };
        let [a, b] = &slot.views;
        pass(encoder, a, &pipeline.down, down);
        pass(encoder, b, &pipeline.blur, horizontal);
        pass(encoder, a, &pipeline.blur, vertical);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tinystream backdrop shape"),
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
        pass.set_pipeline(&pipeline.shape);
        pass.set_bind_group(0, shape, &[]);
        pass.set_vertex_buffer(0, slot.vertices.slice(..));
        pass.draw(0..slot.count, 0..1);
    }
}

/// How bright a lens' rim catches the light.
const LIGHT: f32 = 0.5;

/// How far a squircle's corners reach along each side, as `tinystream_squircle` clamps them, and
/// the exponent of a rounded box (`|x|ⁿ + |y|ⁿ = rⁿ` corners) that follows their curve to within a
/// third of a pixel, for the shader's distance to the edge.
fn corner(width: f32, height: f32, radius: f32, smoothing: f32) -> (f32, f32) {
    let most = width.min(height) / 2.0;
    let radius = radius.min(most);
    if radius <= 0.0 {
        return (0.0, 2.0);
    }
    let smoothing = smoothing.min(most / radius - 1.0).max(0.0);
    (((1.0 + smoothing) * radius).min(most), 2.0 + 2.45 * smoothing)
}

/// A convex squircle as triangles (x, y, coverage), with a pixel-wide fringe fading out for
/// antialiasing.
fn mesh(shape: &Rectangle, radius: f32, smoothing: f32) -> Vec<f32> {
    let points =
        tinystream_squircle::polygon(shape.width as f64, shape.height as f64, radius as f64, smoothing as f64, 1.5);
    let n = points.len();
    let mut out = Vec::with_capacity(n * 27);
    if n < 3 {
        return out;
    }
    let at = |i: usize| (shape.x + points[i % n].0 as f32, shape.y + points[i % n].1 as f32);
    let normal = |i: usize| {
        let (a, b, c) = (at(i + n - 1), at(i), at(i + 1));
        let edge = |p: (f32, f32), q: (f32, f32)| {
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let length = (dx * dx + dy * dy).sqrt().max(1e-6);
            (dy / length, -dx / length)
        };
        let (n1, n2) = (edge(a, b), edge(b, c));
        let (x, y) = (n1.0 + n2.0, n1.1 + n2.1);
        let length = (x * x + y * y).sqrt().max(1e-6);
        (x / length, y / length)
    };
    let ring = |i: usize, by: f32| {
        let (p, d) = (at(i), normal(i));
        (p.0 + d.0 * by, p.1 + d.1 * by)
    };
    let center = (shape.x + shape.width / 2.0, shape.y + shape.height / 2.0);
    let mut push = |p: (f32, f32), coverage: f32| out.extend([p.0, p.1, coverage]);
    for i in 0..n {
        let (inner, next_inner) = (ring(i, -0.5), ring(i + 1, -0.5));
        let (outer, next_outer) = (ring(i, 0.5), ring(i + 1, 0.5));
        push(center, 1.0);
        push(inner, 1.0);
        push(next_inner, 1.0);

        push(inner, 1.0);
        push(outer, 0.0);
        push(next_outer, 0.0);
        push(inner, 1.0);
        push(next_outer, 0.0);
        push(next_inner, 1.0);
    }
    out
}

fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

/// What one backdrop needs from frame to frame: two scratch textures and its bind groups.
struct Slot {
    texels: (u32, u32),
    generation: u64,
    views: [wgpu::TextureView; 2],
    params: [wgpu::Buffer; 3],
    groups: Option<[wgpu::BindGroup; 3]>,
    shape: Option<wgpu::BindGroup>,
    vertices: wgpu::Buffer,
    count: u32,
}

impl Slot {
    fn new(device: &wgpu::Device) -> Self {
        let params = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream backdrop params"),
                size: 96,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tinystream backdrop shape"),
            size: 4096,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            texels: (0, 0),
            generation: 0,
            views: std::array::from_fn(|_| scratch(device, (1, 1))),
            params,
            groups: None,
            shape: None,
            vertices,
            count: 0,
        }
    }

    fn resize(
        &mut self,
        device: &wgpu::Device,
        texels: (u32, u32),
        frame: &Frame,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) {
        if self.texels == texels && self.generation == frame.generation && self.groups.is_some() {
            return;
        }
        if self.texels != texels {
            self.views = std::array::from_fn(|_| scratch(device, texels));
            self.texels = texels;
        }
        self.generation = frame.generation;
        let group = |params: &wgpu::Buffer, source: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tinystream backdrop"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(source) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        };
        let [down, horizontal, vertical] = &self.params;
        let [a, b] = &self.views;
        self.groups = Some([group(down, &frame.view), group(horizontal, a), group(vertical, b)]);
        self.shape = Some(group(down, a));
    }
}

fn scratch(device: &wgpu::Device, (width, height): (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("tinystream backdrop scratch"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

pub struct Pipeline {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    down: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    shape: wgpu::RenderPipeline,
    slots: Vec<Slot>,
    /// Which slot each backdrop drawn this frame has.
    prepared: HashMap<u64, usize>,
}

impl primitive::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tinystream backdrop"),
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
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tinystream backdrop"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tinystream backdrop"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let full = |fragment: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tinystream backdrop"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("full_vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let shape = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tinystream backdrop shape"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("shape_vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array!(0 => Float32x2, 1 => Float32),
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("shape_fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            down: full("down_fs", wgpu::TextureFormat::Rgba8Unorm),
            blur: full("blur_fs", wgpu::TextureFormat::Rgba8Unorm),
            shape,
            layout,
            sampler,
            slots: Vec::new(),
            prepared: HashMap::new(),
        }
    }

    fn trim(&mut self) {
        self.slots.truncate(self.prepared.len());
        self.prepared.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_follow_the_squircle() {
        for (width, height, radius, smoothing) in
            [(320.0, 200.0, 25.0, 0.8), (224.0, 130.0, 17.5, 0.6), (100.0, 40.0, 20.0, 0.8), (80.0, 80.0, 0.0, 0.6)]
        {
            let (corner, n) = corner(width, height, radius, smoothing);
            let outline =
                tinystream_squircle::polygon(width as f64, height as f64, radius as f64, smoothing as f64, 0.5);
            for (x, y) in outline {
                let q = (
                    (x as f32 - width / 2.0).abs() - width / 2.0 + corner,
                    (y as f32 - height / 2.0).abs() - height / 2.0 + corner,
                );
                let (ox, oy) = (q.0.max(0.0), q.1.max(0.0));
                let distance = (ox.powf(n) + oy.powf(n)).powf(1.0 / n) + q.0.max(q.1).min(0.0) - corner;
                assert!(distance.abs() < 0.35, "{width}×{height} r{radius} s{smoothing} at ({x}, {y}): {distance}");
            }
        }
    }

    #[test]
    fn meshes_cover_the_shape() {
        let shape = Rectangle { x: 10.0, y: 20.0, width: 100.0, height: 60.0 };
        let mesh = mesh(&shape, 14.0, 0.6);
        assert_eq!(mesh.len() % 9, 0);
        let solid: Vec<_> = mesh.chunks(3).filter(|v| v[2] == 1.0).collect();
        assert!(solid.iter().all(|v| (9.4..=110.6).contains(&v[0]) && (19.4..=80.6).contains(&v[1])));
        let area: f32 = mesh
            .chunks(9)
            .filter(|t| t[2] == 1.0 && t[5] == 1.0 && t[8] == 1.0)
            .map(|t| ((t[3] - t[0]) * (t[7] - t[1]) - (t[6] - t[0]) * (t[4] - t[1])).abs() / 2.0)
            .sum();
        let outline = tinystream_squircle::polygon(100.0, 60.0, 14.0, 0.6, 1.5);
        let mut reference = 0.0;
        let mut perimeter = 0.0;
        for (a, b) in outline.iter().zip(outline.iter().cycle().skip(1)) {
            reference += a.0 * b.1 - b.0 * a.1;
            perimeter += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        }
        let inset = reference.abs() / 2.0 - perimeter / 2.0;
        assert!((area as f64 - inset).abs() < 2.0, "{area} vs {inset}");
    }
}
