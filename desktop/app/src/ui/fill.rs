// SPDX-License-Identifier: AGPL-3.0-or-later

//! Squircles filled with one colour, drawn by their own pipeline inside iced's render pass. Their
//! edge is antialiased by a fringe fading out across it, rather than multisampled: iced
//! multisamples meshes a whole frame at a time, once for every layer holding one, which made
//! scrolling past many squircles slow.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use iced_core::{Color, Rectangle, Size};
use iced_wgpu::graphics::Viewport;
use iced_wgpu::primitive::{self, Primitive};
use iced_wgpu::wgpu;

/// A squircle of `radius` and `smoothing` (logical pixels) filling `bounds`, to draw as a
/// primitive over [`Fill::bounds`].
#[derive(Debug)]
pub struct Fill {
    color: Color,
    radius: f32,
    smoothing: f32,
    id: u64,
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// How far the primitive reaches past the squircle, in logical pixels, for its fringe.
const MARGIN: f32 = 1.0;

impl Fill {
    pub fn new(color: Color, radius: f32, smoothing: f32) -> Self {
        Self { color, radius, smoothing, id: NEXT.fetch_add(1, Ordering::Relaxed) }
    }

    /// Where the primitive for a squircle at `bounds` is drawn.
    pub fn bounds(bounds: Rectangle) -> Rectangle {
        bounds.expand(MARGIN)
    }
}

impl Primitive for Fill {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let scale = viewport.scale_factor();
        let outer = Size::new(bounds.width * scale, bounds.height * scale);
        let inset = MARGIN * scale;
        let size = Size::new(outer.width - inset * 2.0, outer.height - inset * 2.0);
        if size.width < 1.0 || size.height < 1.0 {
            return;
        }
        let key = [size.width, size.height, self.radius * scale, self.smoothing].map(f32::to_bits);
        let frame = pipeline.frame;
        let mesh = pipeline.meshes.entry(key).or_insert_with(|| {
            let data = bytes(&mesh(size, self.radius * scale, self.smoothing, inset));
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream fill shape"),
                size: data.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &data);
            Mesh { buffer, count: (data.len() / 12) as u32, used: frame }
        });
        mesh.used = frame;

        let index = pipeline.prepared.len();
        if pipeline.slots.len() <= index {
            let params = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream fill"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tinystream fill"),
                layout: &pipeline.layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() }],
            });
            pipeline.slots.push(Slot { params, group });
        }
        let [r, g, b, a] = iced_wgpu::graphics::color::pack(self.color).components();
        let params = [outer.width, outer.height, 0.0, 0.0, r, g, b, a];
        queue.write_buffer(&pipeline.slots[index].params, 0, &bytes(&params));
        pipeline.prepared.insert(self.id, (index, key));
    }

    fn draw(&self, pipeline: &Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(&(index, key)) = pipeline.prepared.get(&self.id) else { return true };
        let (Some(slot), Some(mesh)) = (pipeline.slots.get(index), pipeline.meshes.get(&key)) else { return true };
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &slot.group, &[]);
        render_pass.set_vertex_buffer(0, mesh.buffer.slice(..));
        render_pass.draw(0..mesh.count, 0..1);
        true
    }
}

/// The squircle as triangles (x, y, coverage) in the primitive's pixels, `inset` in from its
/// corner: a fan, and a one-pixel fringe across the outline fading out for antialiasing.
fn mesh(size: Size, radius: f32, smoothing: f32, inset: f32) -> Vec<f32> {
    let points =
        tinystream_squircle::polygon(size.width as f64, size.height as f64, radius as f64, smoothing as f64, 1.5);
    let n = points.len();
    let mut out = Vec::with_capacity(n * 36);
    if n < 3 {
        return out;
    }
    let at = |i: usize| (points[i % n].0 as f32 + inset, points[i % n].1 as f32 + inset);
    let normal = |i: usize| {
        let edge = |p: (f32, f32), q: (f32, f32)| {
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let length = (dx * dx + dy * dy).sqrt().max(1e-6);
            (dy / length, -dx / length)
        };
        let (a, b) = (edge(at(i + n - 1), at(i)), edge(at(i), at(i + 1)));
        let (x, y) = (a.0 + b.0, a.1 + b.1);
        let length = (x * x + y * y).sqrt().max(1e-6);
        (x / length, y / length)
    };
    let ring = |i: usize, by: f32| {
        let (p, d) = (at(i), normal(i));
        (p.0 + d.0 * by, p.1 + d.1 * by)
    };
    let center = (size.width / 2.0 + inset, size.height / 2.0 + inset);
    let (outer, inner) = (0.5, -0.5);
    for i in 0..n {
        let j = i + 1;
        for (p, coverage) in [
            (center, 1.0),
            (ring(i, inner), 1.0),
            (ring(j, inner), 1.0),
            (ring(i, inner), 1.0),
            (ring(i, outer), 0.0),
            (ring(j, outer), 0.0),
            (ring(i, inner), 1.0),
            (ring(j, outer), 0.0),
            (ring(j, inner), 1.0),
        ] {
            out.extend([p.0, p.1, coverage]);
        }
    }
    out
}

fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

struct Mesh {
    buffer: wgpu::Buffer,
    count: u32,
    used: u64,
}

struct Slot {
    params: wgpu::Buffer,
    group: wgpu::BindGroup,
}

pub struct Pipeline {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    /// Shapes by size, radius and smoothing, as they're the same from frame to frame.
    meshes: HashMap<[u32; 4], Mesh>,
    slots: Vec<Slot>,
    /// This frame's fills: their slot and shape.
    prepared: HashMap<u64, (usize, [u32; 4])>,
    frame: u64,
}

impl primitive::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tinystream fill"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tinystream fill"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fill.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tinystream fill"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tinystream fill"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array!(0 => Float32x2, 1 => Float32),
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
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
        Self { layout, pipeline, meshes: HashMap::new(), slots: Vec::new(), prepared: HashMap::new(), frame: 0 }
    }

    fn trim(&mut self) {
        self.slots.truncate(self.prepared.len());
        self.prepared.clear();
        // Shapes unused for a while (about ten seconds of frames) go.
        let frame = self.frame;
        self.meshes.retain(|_, mesh| frame.saturating_sub(mesh.used) < 600);
        self.frame += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fringe_fades_out_across_the_outline() {
        let mesh = mesh(Size::new(40.0, 20.0), 6.0, 0.6, 2.0);
        assert_eq!(mesh.len() % 27, 0);
        let (solid, clear): (Vec<&[f32]>, Vec<&[f32]>) = mesh.chunks(3).partition(|v| v[2] == 1.0);
        assert!(clear.iter().all(|v| v[2] == 0.0));
        // Inside the inset box, and the fringe half a pixel past it at most.
        assert!(solid.iter().all(|v| v[0] >= 2.0 && v[0] <= 42.0 && v[1] >= 2.0 && v[1] <= 22.0));
        assert!(clear.iter().all(|v| v[0] >= 1.5 && v[0] <= 42.5 && v[1] >= 1.5 && v[1] <= 22.5));
        assert!(clear.iter().any(|v| v[0] < 2.0) && clear.iter().any(|v| v[1] > 22.0));
    }
}
