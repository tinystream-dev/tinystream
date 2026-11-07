// SPDX-License-Identifier: AGPL-3.0-or-later

//! Lucide icons, drawn as vectors in the colour of the text around them (`currentColor`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use iced_core::layout::{self, Layout};
use iced_core::widget::Tree;
use iced_core::{Color, Length, Point, Rectangle, Size, Theme, Widget, mouse, renderer};
use iced_wgpu::graphics::Viewport;
use iced_wgpu::primitive::{self, Primitive, Renderer as _};
use iced_wgpu::wgpu;
use iced_widget::Renderer;
use iced_widget::canvas::Path;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// One step of an outline, in its drawing's own units.
pub(super) enum Step {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

/// `icon` at `size` pixels, with Lucide's stroke (2 in 24, scaled with the icon), in the colour
/// text would have where it's put.
pub fn icon(icon: Icon, size: f32) -> Glyph {
    Glyph { icon, size, stroke: 2.0, color: None, filled: false }
}

pub struct Glyph {
    icon: Icon,
    size: f32,
    stroke: f32,
    color: Option<Color>,
    filled: bool,
}

impl Glyph {
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Fills its shapes as well as outlining them (`fill-current`).
    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    /// Its own stroke width, in Lucide's 24×24 units (`strokeWidth`).
    pub fn stroke(mut self, stroke: f32) -> Self {
        self.stroke = stroke;
        self
    }
}

impl iced_core::widget::Meta for Glyph {}

impl<Message> Widget<Message, Theme, Renderer> for Glyph {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, self.size, self.size);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let color = self.color.unwrap_or(style.text_color);
        renderer.draw_primitive(
            bounds.expand(1.0),
            IconPrimitive {
                icon: self.icon,
                size: self.size,
                stroke: self.stroke * self.size / 24.0,
                color,
                filled: self.filled,
                id: NEXT.fetch_add(1, Ordering::Relaxed),
            },
        );
    }
}

#[derive(Debug)]
struct IconPrimitive {
    icon: Icon,
    size: f32,
    stroke: f32,
    color: Color,
    filled: bool,
    id: u64,
}
static NEXT: AtomicU64 = AtomicU64::new(0);

fn segments(icon: Icon, size: f32, filled: bool, tolerance: f32) -> Vec<f32> {
    use iced_wgpu::graphics::geometry::path::lyon_path::Event;
    use iced_wgpu::graphics::geometry::path::lyon_path::iterator::PathIterator;
    let mut out = Vec::new();
    for (fill, steps) in icon.shapes() {
        let fill = *fill || filled;
        for event in path(steps, size / 24.0).raw().iter().flattened(tolerance) {
            let (a, b, stroke) = match event {
                Event::Line { from, to } => (from, to, true),
                Event::End { last, first, close } if close || fill => (last, first, close),
                _ => continue,
            };
            out.extend([a.x, a.y, b.x, b.y, u8::from(fill) as f32, u8::from(stroke) as f32, 0.0, 0.0]);
        }
    }
    out
}

struct Shape {
    buffer: wgpu::Buffer,
    count: u32,
    used: u64,
}
struct Slot {
    params: wgpu::Buffer,
    group: wgpu::BindGroup,
    key: [u32; 4],
}
pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    shapes: HashMap<[u32; 4], Shape>,
    slots: Vec<Slot>,
    prepared: HashMap<u64, usize>,
    frame: u64,
}

impl Primitive for IconPrimitive {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let scale = viewport.scale_factor();
        let key = [self.icon as u32, self.size.to_bits(), u32::from(self.filled), scale.to_bits()];
        let shape = pipeline.shapes.entry(key).or_insert_with(|| {
            let data = segments(self.icon, self.size, self.filled, 0.1 / scale);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream icon outline"),
                size: (data.len() * 4).max(32) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &bytes(&data));
            Shape { buffer, count: (data.len() / 8) as u32, used: pipeline.frame }
        });
        shape.used = pipeline.frame;
        let index = pipeline.prepared.len();
        if pipeline.slots.len() <= index {
            let params = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream icon"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let group = icon_group(device, &pipeline.layout, &params, &shape.buffer);
            pipeline.slots.push(Slot { params, group, key });
        }
        let slot = &mut pipeline.slots[index];
        if slot.key != key {
            slot.group = icon_group(device, &pipeline.layout, &slot.params, &shape.buffer);
            slot.key = key;
        }
        let [r, g, b, a] = iced_wgpu::graphics::color::pack(self.color).components();
        queue.write_buffer(&slot.params, 0, &bytes(&[self.size, self.stroke, scale, shape.count as f32, r, g, b, a]));
        pipeline.prepared.insert(self.id, index);
    }

    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        if let Some(&index) = pipeline.prepared.get(&self.id) {
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, &pipeline.slots[index].group, &[]);
            pass.draw(0..6, 0..1);
        }
        true
    }
}
fn icon_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    params: &wgpu::Buffer,
    shape: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tinystream icon"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: shape.as_entire_binding() },
        ],
    })
}
fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}
impl primitive::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let buffer = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tinystream icon"),
            entries: &[
                buffer(0, wgpu::BufferBindingType::Uniform),
                buffer(1, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tinystream icon"),
            source: wgpu::ShaderSource::Wgsl(include_str!("icon.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tinystream icon"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tinystream icon"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
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
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout, shapes: HashMap::new(), slots: Vec::new(), prepared: HashMap::new(), frame: 0 }
    }

    fn trim(&mut self) {
        self.slots.truncate(self.prepared.len());
        self.prepared.clear();
        self.shapes.retain(|_, shape| self.frame.saturating_sub(shape.used) < 600);
        self.frame += 1;
    }
}

pub(super) fn path(steps: &[Step], scale: f32) -> Path {
    let at = |x: f32, y: f32| Point::new(x * scale, y * scale);
    Path::new(|builder| {
        for step in steps {
            match *step {
                Step::Move(x, y) => builder.move_to(at(x, y)),
                Step::Line(x, y) => builder.line_to(at(x, y)),
                Step::Cubic(a, b, c, d, x, y) => builder.bezier_curve_to(at(a, b), at(c, d), at(x, y)),
                Step::Close => builder.close(),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_inside_their_box() {
        for icon in [Icon::Settings, Icon::KeyRound, Icon::House, Icon::Inbox, Icon::Tv, Icon::CircleAlert] {
            for (_, steps) in icon.shapes() {
                for step in *steps {
                    let points: &[f32] = match step {
                        Step::Move(x, y) | Step::Line(x, y) => &[*x, *y],
                        Step::Cubic(a, b, c, d, x, y) => &[*a, *b, *c, *d, *x, *y],
                        Step::Close => &[],
                    };
                    assert!(points.iter().all(|v| (-0.5..=24.5).contains(v)), "{icon:?}: {points:?}");
                }
            }
        }
    }
}
