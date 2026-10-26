// SPDX-License-Identifier: AGPL-3.0-or-later

//! Artwork that leans toward the pointer, with a glint following it (`lib/tilt.ts`, `.tilt` and
//! `.glare` in `styles.css`). Drawn by its own pipeline so the picture, its squircle and its lit
//! edge tilt together, in perspective.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{Color, Event, Length, Rectangle, Renderer as _, Size, Theme, Widget, image, mouse, renderer, window};
use iced_wgpu::graphics::Viewport;
use iced_wgpu::primitive::{self, Primitive, Renderer as _};
use iced_wgpu::wgpu;
use iced_widget::Renderer;

use super::theme::Look;

/// How far a card leans at most, in degrees.
const MAX: f32 = 7.0;
/// CSS's `perspective(700px)`.
const PERSPECTIVE: f32 = 700.0;

/// Artwork in a squircle that tilts under the pointer, sized `width` × `width · ratio`.
pub fn art(picture: Option<image::Handle>, radius: f32, ratio: f32, look: &Look) -> Art {
    Art { picture, radius, ratio, look: *look }
}

pub struct Art {
    picture: Option<image::Handle>,
    radius: f32,
    ratio: f32,
    look: Look,
}

#[derive(Default)]
struct Lean {
    /// Where the pointer is over the card, 0 to 1 each way, while it's there.
    pointer: Option<(f32, f32)>,
    tilt: (f32, f32),
    glare: (f32, f32),
    glare_opacity: f32,
    hover: f32,
    last: Option<Instant>,
}

impl Lean {
    fn target(&self, still: bool) -> ((f32, f32), f32, f32) {
        match self.pointer {
            Some((x, y)) if !still => {
                (((0.5 - y) * 2.0 * MAX.to_radians(), (x - 0.5) * 2.0 * MAX.to_radians()), 1.0, 1.0)
            },
            Some(_) => ((0.0, 0.0), 0.0, 1.0),
            None => ((0.0, 0.0), 0.0, 0.0),
        }
    }

    /// Moves toward where the pointer says, as CSS transitions would; `true` while still moving.
    fn step(&mut self, now: Instant, still: bool) -> bool {
        let dt = self.last.map_or(0.0, |last| now.saturating_duration_since(last).as_secs_f32());
        self.last = Some(now);
        let (tilt, glare, hover) = self.target(still);
        if still {
            self.tilt = tilt;
            self.glare_opacity = glare;
            self.hover = hover;
            return false;
        }
        // Leaning follows quickly (120ms) and settles back slowly (500ms); the glint fades over
        // 400ms.
        let towards = |value: &mut f32, target: f32, time: f32| {
            *value += (target - *value) * (1.0 - (-dt / (time / 3.0)).exp());
            if (*value - target).abs() < 1e-4 {
                *value = target;
            }
            *value != target
        };
        let lean = if self.pointer.is_some() { 0.12 } else { 0.5 };
        let mut moving = towards(&mut self.tilt.0, tilt.0, lean);
        moving |= towards(&mut self.tilt.1, tilt.1, lean);
        moving |= towards(&mut self.glare_opacity, glare, 0.4);
        moving |= towards(&mut self.hover, hover, 0.2);
        if let Some((x, y)) = self.pointer {
            self.glare = (x, y);
        }
        moving
    }
}

impl iced_core::widget::Meta for Art {}

impl<Message> Widget<Message, Theme, Renderer> for Art {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Lean>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Lean { glare: (0.5, 0.0), ..Lean::default() })
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        let width = limits.resolve_width(Length::Fill, 0.0);
        tree.size = limits.resolve(Length::Fill, Length::Shrink, Size::new(width, width * self.ratio));
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let lean = tree.state.downcast_mut::<Lean>();
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft) => {
                let pointer = cursor.position_over(bounds).map(|p| {
                    (
                        ((p.x - bounds.x) / bounds.width).clamp(0.0, 1.0),
                        ((p.y - bounds.y) / bounds.height).clamp(0.0, 1.0),
                    )
                });
                if pointer != lean.pointer {
                    lean.pointer = pointer;
                    shell.request_redraw();
                }
            },
            Event::Window(window::Event::RedrawRequested(now)) => {
                if lean.step(*now, self.look.reduce_motion) {
                    shell.request_redraw();
                }
            },
            _ => {},
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let lean = tree.state.downcast_ref::<Lean>();
        // The card rises 2 pixels under the pointer.
        let mut card = layout.bounds();
        card.y -= 2.0 * lean.hover;
        let look = &self.look;
        let tilted = Tilted {
            card,
            picture: self.picture.clone(),
            tilt: lean.tilt,
            glare: lean.glare,
            glare_opacity: lean.glare_opacity,
            hover: lean.hover,
            radius: look.radius(self.radius),
            smoothing: look.corners().1,
            background: look.raised,
            media_ink: look.media_ink,
            glow: look.glow,
            edge: look.edge(),
            id: NEXT.fetch_add(1, Ordering::Relaxed),
        };
        // Room for the corners that lean out of the box.
        let reach = card.width.max(card.height) * 0.12;
        let room = Rectangle {
            x: card.x - reach,
            y: card.y - reach,
            width: card.width + reach * 2.0,
            height: card.height + reach * 2.0,
        };
        renderer.with_layer(*viewport, |renderer| renderer.draw_primitive(room, tilted));
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::None
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Tilted {
    card: Rectangle,
    picture: Option<image::Handle>,
    tilt: (f32, f32),
    glare: (f32, f32),
    glare_opacity: f32,
    hover: f32,
    radius: f32,
    smoothing: f32,
    background: Color,
    media_ink: Color,
    glow: Color,
    edge: [f32; 3],
    id: u64,
}

/// A picture as pixels, whatever the handle holds.
fn pixels(handle: &image::Handle) -> Option<(u32, u32, Vec<u8>)> {
    match handle {
        image::Handle::Rgba { width, height, pixels, .. } => Some((*width, *height, pixels.to_vec())),
        image::Handle::Bytes(_, bytes) => {
            let decoded = ::image::load_from_memory(bytes).ok()?.into_rgba8();
            Some((decoded.width(), decoded.height(), decoded.into_raw()))
        },
        image::Handle::Path(_, path) => {
            let decoded = ::image::open(path).ok()?.into_rgba8();
            Some((decoded.width(), decoded.height(), decoded.into_raw()))
        },
    }
}

impl Primitive for Tilted {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let margin = self.card.width.max(self.card.height) * 0.12;
        let transform = bounds.width / (self.card.width + margin * 2.0);
        let scale = viewport.scale_factor();
        let frame = viewport.physical_size();
        let card = Rectangle {
            x: (bounds.x + margin * transform) * scale,
            y: (bounds.y + margin * transform) * scale,
            width: self.card.width * transform * scale,
            height: self.card.height * transform * scale,
        };
        let scale = scale * transform;
        if card.width < 1.0 || card.height < 1.0 {
            return;
        }

        let picture = self.picture.as_ref().map(image::Handle::id);
        if let (Some(handle), Some(id)) = (&self.picture, picture)
            && !pipeline.pictures.contains_key(&id)
            && let Some(uploaded) = pixels(handle).map(|p| upload(device, queue, p))
        {
            pipeline.pictures.insert(id, uploaded);
        }
        let (texture, size) = match picture.and_then(|id| pipeline.pictures.get_mut(&id)) {
            Some(uploaded) => {
                uploaded.used = pipeline.frame;
                (uploaded.view.clone(), uploaded.size)
            },
            None => (pipeline.blank.clone(), (1, 1)),
        };

        // object-fit: cover, centred.
        let (art, boxed) = (size.0 as f32 / size.1 as f32, card.width / card.height);
        let cover = if art > boxed {
            let s = boxed / art;
            [s, 1.0, (1.0 - s) / 2.0, 0.0]
        } else {
            let s = art / boxed;
            [1.0, s, 0.0, (1.0 - s) / 2.0]
        };
        let rgba = |c: Color| [c.r, c.g, c.b, c.a];
        let mut params = vec![card.x, card.y, card.width, card.height, frame.width as f32, frame.height as f32];
        params.extend([self.tilt.0, self.tilt.1, PERSPECTIVE * scale, self.glare_opacity]);
        params.extend([self.glare.0, self.glare.1, 0.65 * card.width.min(card.height), self.hover]);
        // `cover` is a vec4, so it starts on the next 16 bytes.
        params.extend([0.0, 0.0]);
        params.extend(cover);
        params.extend(rgba(self.background));
        params.extend(rgba(self.media_ink));
        params.extend(rgba(self.glow));
        params.extend([self.edge[0], self.edge[1], self.edge[2], 0.0]);

        let index = pipeline.prepared.len();
        pipeline.prepared.insert(self.id, index);
        if pipeline.slots.len() <= index {
            pipeline.slots.push(Slot::new(device));
        }
        let slot = &mut pipeline.slots[index];
        queue.write_buffer(&slot.params, 0, &bytes(&params));
        let mesh = mesh(card.size(), self.radius * scale, self.smoothing);
        let data = bytes(&mesh);
        if slot.vertices.size() < data.len() as u64 {
            slot.vertices = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream tilt shape"),
                size: (data.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&slot.vertices, 0, &data);
        slot.count = (mesh.len() / 4) as u32;
        slot.group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tinystream tilt"),
            layout: &pipeline.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: slot.params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&pipeline.sampler) },
            ],
        }));
    }

    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(slot) = pipeline.prepared.get(&self.id).and_then(|&i| pipeline.slots.get(i)) else { return };
        let Some(group) = &slot.group else { return };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tinystream tilt"),
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
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.set_vertex_buffer(0, slot.vertices.slice(..));
        pass.draw(0..slot.count, 0..1);
    }
}

/// The card as triangles (x, y, coverage, edge) in its own pixels: a fan, a one-pixel lit edge
/// and a fringe fading out for antialiasing.
fn mesh(size: Size, radius: f32, smoothing: f32) -> Vec<f32> {
    let points =
        tinystream_squircle::polygon(size.width as f64, size.height as f64, radius as f64, smoothing as f64, 1.5);
    let n = points.len();
    let mut out = Vec::with_capacity(n * 48);
    if n < 3 {
        return out;
    }
    let at = |i: usize| (points[i % n].0 as f32, points[i % n].1 as f32);
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
    let center = (size.width / 2.0, size.height / 2.0);
    let mut push = |p: (f32, f32), coverage: f32, edge: f32| out.extend([p.0, p.1, coverage, edge]);
    for i in 0..n {
        let (j, outer, rim, inner) = (i + 1, 0.5, -0.5, -1.5);
        push(center, 1.0, 0.0);
        push(ring(i, inner), 1.0, 0.0);
        push(ring(j, inner), 1.0, 0.0);
        for (from, to, (c0, e0), (c1, e1)) in
            [(inner, rim, (1.0, 0.0), (1.0, 1.0)), (rim, outer, (1.0, 1.0), (0.0, 1.0))]
        {
            push(ring(i, from), c0, e0);
            push(ring(i, to), c1, e1);
            push(ring(j, to), c1, e1);
            push(ring(i, from), c0, e0);
            push(ring(j, to), c1, e1);
            push(ring(j, from), c0, e0);
        }
    }
    out
}

fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

struct Uploaded {
    view: wgpu::TextureView,
    size: (u32, u32),
    used: u64,
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, (width, height, pixels): (u32, u32, Vec<u8>)) -> Uploaded {
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tinystream artwork"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) },
        size,
    );
    Uploaded { view: texture.create_view(&Default::default()), size: (width, height), used: 0 }
}

struct Slot {
    params: wgpu::Buffer,
    vertices: wgpu::Buffer,
    count: u32,
    group: Option<wgpu::BindGroup>,
}

impl Slot {
    fn new(device: &wgpu::Device) -> Self {
        let buffer = |size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream tilt"),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            params: buffer(160, wgpu::BufferUsages::UNIFORM),
            vertices: buffer(16384, wgpu::BufferUsages::VERTEX),
            count: 0,
            group: None,
        }
    }
}

pub struct Pipeline {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    blank: wgpu::TextureView,
    pictures: HashMap<image::Id, Uploaded>,
    slots: Vec<Slot>,
    prepared: HashMap<u64, usize>,
    frame: u64,
}

impl primitive::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tinystream tilt"),
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
            label: Some("tinystream tilt"),
            source: wgpu::ShaderSource::Wgsl(include_str!("tilt.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tinystream tilt"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tinystream tilt"),
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
        let blank = upload(device, queue, (1, 1, vec![0, 0, 0, 0])).view;
        Self {
            layout,
            sampler,
            pipeline,
            blank,
            pictures: HashMap::new(),
            slots: Vec::new(),
            prepared: HashMap::new(),
            frame: 0,
        }
    }

    fn trim(&mut self) {
        self.slots.truncate(self.prepared.len());
        self.prepared.clear();
        // Pictures off screen for a while (about ten seconds of frames) go.
        let frame = self.frame;
        self.pictures.retain(|_, picture| frame.saturating_sub(picture.used) < 600);
        self.frame += 1;
    }
}
