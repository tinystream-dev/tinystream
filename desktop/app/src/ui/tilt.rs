// SPDX-License-Identifier: AGPL-3.0-or-later

//! Artwork that leans toward the pointer, with a glint following it (`lib/tilt.ts`, `.tilt` and
//! `.glare` in `styles.css`). Drawn by its own pipeline so the picture, its squircle and its lit
//! edge tilt together, in perspective. A [`card`] leans whatever it holds the same way, read back
//! from the frame after it's drawn flat.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use iced_core::layout::{self, Layout};
use iced_core::widget::{Tree, tree};
use iced_core::{
    Color, Event, Length, Rectangle, Renderer as _, Size, Theme, Vector, Widget, image, mouse, overlay, renderer,
    window,
};
use iced_wgpu::graphics::Viewport;
use iced_wgpu::primitive::{self, Primitive, Renderer as _};
use iced_wgpu::wgpu;
use iced_widget::{Element, Renderer, Widget as _};

use super::backdrop;
use super::theme::Look;

/// How far artwork leans at most, in degrees.
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
    fn target(&self, still: bool, max: f32) -> ((f32, f32), f32, f32) {
        let max = max.to_radians();
        match self.pointer {
            Some((x, y)) if !still => (((0.5 - y) * 2.0 * max, (x - 0.5) * 2.0 * max), 1.0, 1.0),
            Some(_) => ((0.0, 0.0), 0.0, 1.0),
            None => ((0.0, 0.0), 0.0, 0.0),
        }
    }

    /// Moves toward where the pointer says, as CSS transitions would; `true` while still moving.
    fn step(&mut self, now: Instant, still: bool, max: f32) -> bool {
        let dt = self.last.map_or(0.0, |last| now.saturating_duration_since(last).as_secs_f32());
        self.last = Some(now);
        let (tilt, glare, hover) = self.target(still, max);
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

    /// Follows the pointer over `bounds`, leaning up to `max` degrees.
    fn update<Message>(
        &mut self,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        shell: &mut iced_core::Shell<'_, Message>,
        look: &Look,
        max: f32,
    ) {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft) => {
                let pointer = cursor.position_over(bounds).map(|p| {
                    (
                        ((p.x - bounds.x) / bounds.width).clamp(0.0, 1.0),
                        ((p.y - bounds.y) / bounds.height).clamp(0.0, 1.0),
                    )
                });
                if pointer != self.pointer {
                    self.pointer = pointer;
                    shell.request_redraw();
                }
            },
            Event::Window(window::Event::RedrawRequested(now)) => {
                if self.step(*now, look.reduce_motion, max) {
                    shell.request_redraw();
                }
            },
            _ => {},
        }
    }

    /// What's drawn for a card laid out at `bounds`, and the room its corners lean out into.
    fn tilted(&self, bounds: Rectangle, source: Source, radius: f32, look: &Look, id: u64) -> (Tilted, Rectangle) {
        // The card rises 2 pixels under the pointer.
        let mut card = bounds;
        card.y -= 2.0 * self.hover;
        let tilted = Tilted {
            card,
            source,
            tilt: self.tilt,
            glare: self.glare,
            glare_opacity: self.glare_opacity,
            hover: self.hover,
            radius: look.radius(radius),
            smoothing: look.corners().1,
            background: look.raised,
            media_ink: look.media_ink,
            glow: look.glow,
            edge: look.edge(),
            id,
        };
        let reach = card.width.max(card.height) * 0.12;
        let room = Rectangle {
            x: card.x - reach,
            y: card.y - reach,
            width: card.width + reach * 2.0,
            height: card.height + reach * 2.0,
        };
        (tilted, room)
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
        let lean = tree.state.downcast_mut::<Lean>();
        lean.update(event, layout.bounds(), cursor, shell, &self.look, MAX);
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
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let source = Source::Picture(self.picture.clone());
        let (tilted, room) = lean.tilted(layout.bounds(), source, self.radius, &self.look, id);
        if room.intersection(viewport).is_none() {
            return;
        }
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

/// Whatever `content` draws, in a squircle with its lit edge, leaning toward the pointer up to
/// `max` degrees (0 keeps it still). It's drawn flat, then read back from the frame and drawn again
/// in perspective, clipped to the squircle, with what was behind it put back first. Where frames
/// can't be read back, it's just drawn flat.
pub fn card<'a, Message: 'a>(
    content: impl Widget<Message, Theme, Renderer> + 'a,
    radius: f32,
    max: f32,
    look: &Look,
) -> Element<'a, Message> {
    Card { content: Widget::_boxed(content), radius, max, look: *look }.boxed()
}

struct Card<'a, Message> {
    content: Element<'a, Message>,
    radius: f32,
    max: f32,
    look: Look,
}

impl<Message> iced_core::widget::Meta for Card<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for Card<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Lean>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Lean { glare: (0.5, 0.0), ..Lean::default() })
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let size = self.content.size();
        layout::padded(tree, limits, size.width, size.height, 0, |tree, limits| {
            self.content.layout(tree, renderer, limits);
            tree.size
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.max > 0.0 {
            let lean = tree.state.downcast_mut::<Lean>();
            lean.update(event, layout.bounds(), cursor, shell, &self.look, self.max);
        }
        let (child, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.update(tree, event, child, cursor, renderer, shell, viewport);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let flat = layout.bounds();
        let lean = tree.state.downcast_ref::<Lean>();
        let (child, tree) = layout.iter(&tree.children).next().expect("content");
        let Some(clip) = flat.intersection(viewport).filter(|_| backdrop::available()) else {
            self.content.draw(tree, renderer, theme, style, child, cursor, viewport);
            return;
        };
        // Layers are drawn in the order they're opened, so these three follow each other.
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (behind, _) = lean.tilted(flat, Source::Behind, self.radius, &self.look, id);
        renderer.with_layer(clip, |renderer| renderer.draw_primitive(flat, behind));
        renderer.with_layer(*viewport, |renderer| {
            self.content.draw(tree, renderer, theme, style, child, cursor, viewport);
        });
        let (tilted, room) = lean.tilted(flat, Source::Drawn, self.radius, &self.look, id);
        renderer.with_layer(*viewport, |renderer| renderer.draw_primitive(room, tilted));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn iced_core::widget::Operation,
    ) {
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.operate(tree, layout, viewport, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (layout, tree) = layout.iter(&tree.children).next().expect("content");
        self.content.mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().expect("content");
        self.content.overlay(tree, layout, renderer, viewport, translation, window)
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// What's drawn in the card.
#[derive(Debug)]
enum Source {
    Picture(Option<image::Handle>),
    /// Keeps what's behind a [`card`] before its content is drawn.
    Behind,
    /// What a [`card`]'s content drew, which goes back to what was behind it before being drawn
    /// leaning.
    Drawn,
}

#[derive(Debug)]
struct Tilted {
    card: Rectangle,
    source: Source,
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
        if let Source::Behind = self.source {
            let frame = backdrop::frame();
            let Some(frame) = frame.as_ref() else { return };
            let flat = *bounds * viewport.scale_factor();
            let size = frame.texture.size();
            let x0 = flat.x.floor().max(0.0);
            let y0 = flat.y.floor().max(0.0);
            let x1 = (flat.x + flat.width).ceil().min(size.width as f32);
            let y1 = (flat.y + flat.height).ceil().min(size.height as f32);
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            let region = [x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32];
            let slot = pipeline.slot(device, self.id);
            slot.readback = Some(Readback::new(device, slot.readback.take(), frame.texture.format(), region, flat));
            return;
        }
        let margin = self.card.width.max(self.card.height) * 0.12;
        let transform = bounds.width / (self.card.width + margin * 2.0);
        let scale = viewport.scale_factor();
        let frame = viewport.physical_size();
        pipeline.size = frame;
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

        let (texture, cover) = match &self.source {
            Source::Picture(handle) => {
                let picture = handle.as_ref().map(image::Handle::id);
                if let (Some(handle), Some(id)) = (handle, picture)
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
                (texture, cover)
            },
            Source::Behind | Source::Drawn => {
                let readback = pipeline.prepared.get(&self.id).and_then(|&i| pipeline.slots[i].readback.as_ref());
                let Some(Readback { view, region: [x, y, width, height], flat, .. }) = readback else { return };
                let (width, height) = (*width as f32, *height as f32);
                let cover = [
                    flat.width / width,
                    flat.height / height,
                    (flat.x - *x as f32) / width,
                    (flat.y - *y as f32) / height,
                ];
                (view.clone(), cover)
            },
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

        let key = [card.width, card.height, self.radius * scale, self.smoothing].map(f32::to_bits);
        let frame = pipeline.frame;
        let shape = pipeline.meshes.entry(key).or_insert_with(|| {
            let data = bytes(&mesh(card.size(), self.radius * scale, self.smoothing));
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("tinystream tilt shape"),
                size: data.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, &data);
            Shape { buffer, count: (data.len() / 24) as u32, used: frame }
        });
        shape.used = frame;

        let (layout, sampler) = (pipeline.layout.clone(), pipeline.sampler.clone());
        let slot = pipeline.slot(device, self.id);
        queue.write_buffer(&slot.params, 0, &bytes(&params));
        slot.shape = key;
        if slot.group.as_ref().is_none_or(|(bound, _)| *bound != texture) {
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tinystream tilt"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: slot.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                ],
            });
            slot.group = Some((texture, group));
        }
    }

    fn draw(&self, pipeline: &Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        // Pictures need nothing read back, so they're drawn in iced's pass, without breaking it.
        let Source::Picture(_) = self.source else { return false };
        let Some(slot) = pipeline.prepared.get(&self.id).and_then(|&i| pipeline.slots.get(i)) else { return true };
        // iced fits the viewport to the primitive, but the shape is placed in the whole frame.
        render_pass.set_viewport(0.0, 0.0, pipeline.size.width as f32, pipeline.size.height as f32, 0.0, 1.0);
        pipeline.draw(slot, render_pass);
        true
    }

    fn render(
        &self,
        pipeline: &Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let Some(slot) = pipeline.prepared.get(&self.id).and_then(|&i| pipeline.slots.get(i)) else { return };
        match (&self.source, &slot.readback, backdrop::frame().as_ref()) {
            (Source::Behind, Some(readback), Some(frame)) => {
                let [x, y, width, height] = readback.region;
                copy(encoder, (&frame.texture, x, y), (&readback.behind, 0, 0), (width, height));
                return;
            },
            (Source::Drawn, Some(readback), Some(frame)) => {
                let [x, y, width, height] = readback.region;
                copy(encoder, (&frame.texture, x, y), (&readback.drawn, 0, 0), (width, height));
                copy(encoder, (&readback.behind, 0, 0), (&frame.texture, x, y), (width, height));
            },
            _ => return,
        }
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
        pipeline.draw(slot, &mut pass);
    }
}

/// The card as triangles (x, y, coverage, edge, then where it shows the picture from) in its own
/// pixels: a fan, a one-pixel lit edge and a fringe fading out for antialiasing. The edge and
/// fringe show what's just inside them, never what a card's content drew past its outline.
fn mesh(size: Size, radius: f32, smoothing: f32) -> Vec<f32> {
    let points =
        tinystream_squircle::polygon(size.width as f64, size.height as f64, radius as f64, smoothing as f64, 1.5);
    let n = points.len();
    let mut out = Vec::with_capacity(n * 72);
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
    let (outer, rim, inner) = (0.5, -0.5, -1.5);
    let mut push = |p: (f32, f32), shown: (f32, f32), coverage: f32, edge: f32| {
        out.extend([p.0, p.1, coverage, edge, shown.0, shown.1]);
    };
    for i in 0..n {
        let j = i + 1;
        let vertex = |k: usize, by: f32| (ring(k, by), ring(k, inner));
        push(center, center, 1.0, 0.0);
        push(ring(i, inner), ring(i, inner), 1.0, 0.0);
        push(ring(j, inner), ring(j, inner), 1.0, 0.0);
        for (from, to, (c0, e0), (c1, e1)) in
            [(inner, rim, (1.0, 0.0), (1.0, 1.0)), (rim, outer, (1.0, 1.0), (0.0, 1.0))]
        {
            for (k, by, c, e) in [
                (i, from, c0, e0),
                (i, to, c1, e1),
                (j, to, c1, e1),
                (i, from, c0, e0),
                (j, to, c1, e1),
                (j, from, c0, e0),
            ] {
                let (p, shown) = vertex(k, by);
                push(p, shown, c, e);
            }
        }
    }
    out
}

fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_ne_bytes()).collect()
}

fn copy(
    encoder: &mut wgpu::CommandEncoder,
    (from, from_x, from_y): (&wgpu::Texture, u32, u32),
    (to, to_x, to_y): (&wgpu::Texture, u32, u32),
    (width, height): (u32, u32),
) {
    let at = |texture, x, y| wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d { x, y, z: 0 },
        aspect: wgpu::TextureAspect::All,
    };
    encoder.copy_texture_to_texture(
        at(from, from_x, from_y),
        at(to, to_x, to_y),
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
}

/// A card's region of the frame (`region`, in whole frame pixels, around `flat`): what was
/// behind it, and what its content drew there.
struct Readback {
    behind: wgpu::Texture,
    drawn: wgpu::Texture,
    view: wgpu::TextureView,
    region: [u32; 4],
    flat: Rectangle,
}

impl Readback {
    fn new(
        device: &wgpu::Device,
        old: Option<Readback>,
        format: wgpu::TextureFormat,
        region: [u32; 4],
        flat: Rectangle,
    ) -> Self {
        let size = wgpu::Extent3d { width: region[2], height: region[3], depth_or_array_layers: 1 };
        if let Some(old) = old.filter(|old| old.drawn.size() == size && old.drawn.format() == format) {
            return Self { region, flat, ..old };
        }
        let texture = |usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tinystream tilt readback"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: usage | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let behind = texture(wgpu::TextureUsages::COPY_SRC);
        let drawn = texture(wgpu::TextureUsages::TEXTURE_BINDING);
        Self { view: drawn.create_view(&Default::default()), behind, drawn, region, flat }
    }
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

/// A card's outline, kept while cards of its size are drawn.
struct Shape {
    buffer: wgpu::Buffer,
    count: u32,
    used: u64,
}

struct Slot {
    params: wgpu::Buffer,
    shape: [u32; 4],
    /// The texture it's bound to, and the bind group, kept while it's the same.
    group: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    readback: Option<Readback>,
}

impl Slot {
    fn new(device: &wgpu::Device) -> Self {
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tinystream tilt"),
            size: 160,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { params, shape: [0; 4], group: None, readback: None }
    }
}

pub struct Pipeline {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    blank: wgpu::TextureView,
    pictures: HashMap<image::Id, Uploaded>,
    meshes: HashMap<[u32; 4], Shape>,
    slots: Vec<Slot>,
    prepared: HashMap<u64, usize>,
    frame: u64,
    /// The frame's size in pixels.
    size: iced_core::Size<u32>,
}

impl Pipeline {
    fn draw(&self, slot: &Slot, pass: &mut wgpu::RenderPass<'_>) {
        let (Some((_, group)), Some(shape)) = (&slot.group, self.meshes.get(&slot.shape)) else { return };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.set_vertex_buffer(0, shape.buffer.slice(..));
        pass.draw(0..shape.count, 0..1);
    }

    /// The slot `id` was given this frame, or a new one: a card's two primitives share one.
    fn slot(&mut self, device: &wgpu::Device, id: u64) -> &mut Slot {
        let next = self.prepared.len();
        let index = *self.prepared.entry(id).or_insert(next);
        if self.slots.len() <= index {
            self.slots.push(Slot::new(device));
        }
        &mut self.slots[index]
    }
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
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array!(
                        0 => Float32x2,
                        1 => Float32,
                        2 => Float32,
                        3 => Float32x2
                    ),
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
            meshes: HashMap::new(),
            slots: Vec::new(),
            prepared: HashMap::new(),
            frame: 0,
            size: iced_core::Size::new(0, 0),
        }
    }

    fn trim(&mut self) {
        self.slots.truncate(self.prepared.len());
        self.prepared.clear();
        // Pictures off screen for a while (about ten seconds of frames) go.
        let frame = self.frame;
        self.pictures.retain(|_, picture| frame.saturating_sub(picture.used) < 600);
        self.meshes.retain(|_, shape| frame.saturating_sub(shape.used) < 600);
        self.frame += 1;
    }
}
