// SPDX-License-Identifier: AGPL-3.0-or-later

//! Squircle fills and outlines, drawn by their own pipeline inside iced's render pass. Their
//! edge is antialiased by a fringe fading out across it, rather than multisampled: iced
//! multisamples meshes a whole frame at a time, once for every layer holding one, which made
//! scrolling past many squircles slow.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use iced_core::{Color, Rectangle, Size};
use iced_wgpu::graphics::Viewport;
use iced_wgpu::primitive::{self, Primitive};
use iced_wgpu::wgpu;

/// A squircle of `radius` and `smoothing` (logical pixels), to draw as a
/// primitive over [`Fill::bounds`].
#[derive(Debug)]
pub struct Fill {
    color: Color,
    radius: f32,
    smoothing: f32,
    id: u64,
    stroke: f32,
    gradient: [f32; 3],
    dashed: bool,
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// How far the primitive reaches past the squircle, in logical pixels, for its fringe.
const MARGIN: f32 = 2.0;

impl Fill {
    pub fn new(color: Color, radius: f32, smoothing: f32) -> Self {
        Self {
            color,
            radius,
            smoothing,
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            stroke: 0.0,
            gradient: [1.0; 3],
            dashed: false,
        }
    }

    /// A centred outline, with a vertical three-stop light and optional 4px dashes.
    pub fn outline(mut self, width: f32, gradient: [f32; 3], dashed: bool) -> Self {
        self.stroke = width;
        self.gradient = gradient;
        self.dashed = dashed;
        self
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
        let key = [size.width, size.height, self.radius * scale, self.smoothing, self.stroke * scale, scale]
            .map(f32::to_bits);
        let frame = pipeline.frame;
        let mesh = pipeline.meshes.entry(key).or_insert_with(|| {
            let data = bytes(&if self.stroke > 0.0 {
                outline_mesh(size, self.radius * scale, self.smoothing, inset, self.stroke * scale)
            } else {
                mesh(size, self.radius * scale, self.smoothing, inset)
            });
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream fill shape"),
                size: data.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &data);
            Mesh { buffer, count: (data.len() / 16) as u32, used: frame }
        });
        mesh.used = frame;

        let index = pipeline.prepared.len();
        if pipeline.slots.len() <= index {
            let params = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream fill"),
                size: 48,
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
        let [top, middle, bottom] = self.gradient;
        let params = [
            outer.width,
            outer.height,
            0.0,
            0.0,
            r,
            g,
            b,
            a,
            top,
            middle,
            bottom,
            if self.dashed { 8.0 * scale } else { 0.0 },
        ];
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
            out.extend([p.0, p.1, coverage, 0.0]);
        }
    }
    out
}

fn outline_mesh(size: Size, radius: f32, smoothing: f32, inset: f32, width: f32) -> Vec<f32> {
    let mut points: Vec<(f32, f32)> =
        tinystream_squircle::polygon(size.width as f64, size.height as f64, radius as f64, smoothing as f64, 1.5)
            .into_iter()
            .map(|(x, y)| (x as f32, y as f32))
            .collect();
    let same = |a: &(f32, f32), b: &(f32, f32)| (a.0 - b.0).hypot(a.1 - b.1) < 1e-4;
    points.dedup_by(|a, b| same(a, b));
    if points.first().zip(points.last()).is_some_and(|(a, b)| same(a, b)) {
        points.pop();
    }
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    let at = |i: usize| {
        let p = points[i % n];
        (p.0 + inset, p.1 + inset)
    };
    let normal = |i: usize| {
        let edge = |p: (f32, f32), q: (f32, f32)| {
            let (x, y) = (q.1 - p.1, p.0 - q.0);
            let length = x.hypot(y).max(1e-6);
            (x / length, y / length)
        };
        let (a, b) = (edge(at(i + n - 1), at(i)), edge(at(i), at(i + 1)));
        let (x, y) = (a.0 + b.0, a.1 + b.1);
        let denominator = (x * b.0 + y * b.1).max(1e-6);
        (x / denominator, y / denominator)
    };
    let inner = (width * 0.5 - 0.5).max(0.0);
    let outer = width * 0.5 + 0.5;
    let bands = [(-outer, 0.0), (-inner, width.min(1.0)), (inner, width.min(1.0)), (outer, 0.0)];
    let mut out = Vec::with_capacity(n * 72);
    let mut along = 0.0;
    for i in 0..n {
        let (p, q) = (at(i), at(i + 1));
        let end = along + (q.0 - p.0).hypot(q.1 - p.1);
        for band in bands.windows(2) {
            for (index, side, distance) in
                [(i, 0, along), (i + 1, 0, end), (i + 1, 1, end), (i, 0, along), (i + 1, 1, end), (i, 1, along)]
            {
                let (p, d) = (at(index), normal(index % n));
                let (offset, coverage) = band[side];
                out.extend([p.0 + d.0 * offset, p.1 + d.1 * offset, coverage, distance]);
            }
        }
        along = end;
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
    meshes: HashMap<[u32; 6], Mesh>,
    slots: Vec<Slot>,
    /// This frame's fills: their slot and shape.
    prepared: HashMap<u64, (usize, [u32; 6])>,
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
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array!(0 => Float32x2, 1 => Float32, 2 => Float32),
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
    fn capsule_outlines_stay_on_the_perimeter() {
        let size = Size::new(93.0, 33.0);
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mesh = outline_mesh(
                Size::new(size.width * scale, size.height * scale),
                9999.0 * scale,
                0.0,
                2.0 * scale,
                scale,
            );
            for vertex in mesh.chunks(4) {
                assert!(
                    vertex[0] >= 0.0
                        && vertex[0] <= (size.width + 4.0) * scale
                        && vertex[1] >= 0.0
                        && vertex[1] <= (size.height + 4.0) * scale,
                    "scale {scale}: {vertex:?}"
                );
            }
        }
    }

    #[test]
    fn outlines_leave_the_middle_clear_and_fade_on_both_sides() {
        let center = (22.0, 12.0);
        for width in [0.75, 1.0, 2.0] {
            let mesh = outline_mesh(Size::new(40.0, 20.0), 6.0, 0.6, 2.0, width);
            assert!(mesh.iter().all(|v| v.is_finite()));
            assert!(mesh.chunks(4).any(|v| v[2] == 0.0));
            assert!(mesh.chunks(4).any(|v| v[2] == width.min(1.0)));
            for triangle in mesh.as_chunks::<12>().0 {
                let cross =
                    |a: &[f32], b: &[f32]| (b[0] - a[0]) * (center.1 - a[1]) - (b[1] - a[1]) * (center.0 - a[0]);
                let signs = [
                    cross(&triangle[0..4], &triangle[4..8]),
                    cross(&triangle[4..8], &triangle[8..12]),
                    cross(&triangle[8..12], &triangle[0..4]),
                ];
                assert!(signs.iter().any(|v| *v < 0.0) && signs.iter().any(|v| *v > 0.0));
            }
        }
    }

    #[test]
    fn fringe_fades_out_across_the_outline() {
        let mesh = mesh(Size::new(40.0, 20.0), 6.0, 0.6, 2.0);
        assert_eq!(mesh.len() % 36, 0);
        let (solid, clear): (Vec<&[f32]>, Vec<&[f32]>) = mesh.chunks(4).partition(|v| v[2] == 1.0);
        assert!(clear.iter().all(|v| v[2] == 0.0));
        // Inside the inset box, and the fringe half a pixel past it at most.
        assert!(solid.iter().all(|v| v[0] >= 2.0 && v[0] <= 42.0 && v[1] >= 2.0 && v[1] <= 22.0));
        assert!(clear.iter().all(|v| v[0] >= 1.5 && v[0] <= 42.5 && v[1] >= 1.5 && v[1] <= 22.5));
        assert!(clear.iter().any(|v| v[0] < 2.0) && clear.iter().any(|v| v[1] > 22.0));
    }
}
