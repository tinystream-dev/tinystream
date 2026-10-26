// SPDX-License-Identifier: AGPL-3.0-or-later

//! Draws the app into a PNG without opening a window (`--snapshot <file>`, debug builds only), for
//! checking how screens look.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use iced_core::{Event, Size, mouse, renderer, window};
use iced_wgpu::graphics::{Shell, Viewport};
use iced_wgpu::{Engine, Renderer, wgpu};
use iced_winit::runtime::user_interface::{self, UserInterface};

use crate::app::{App, Effect, Message};
use crate::ui::backdrop;

pub fn snapshot(
    app: &mut App,
    size: Size<u32>,
    scale: f32,
    cursor: Option<iced_core::Point>,
    path: &Path,
    input: &[Event],
) -> anyhow::Result<Vec<Message>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .context("no GPU adapter")?;
    let (device, queue) = futures::executor::block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("tinystream snapshot"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits { max_non_sampler_bindings: 2048, ..wgpu::Limits::default() }
                .using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
        }),
    )?;

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let engine = Engine::new(
        &adapter,
        device.clone(),
        queue.clone(),
        format,
        Some(iced_wgpu::graphics::Antialiasing::MSAAx4),
        Shell::headless(),
    );
    let mut renderer = Renderer::new(
        engine,
        renderer::Settings { line_height: iced_core::text::LineHeight::Relative(1.5), ..renderer::Settings::default() },
    );
    let mut viewport =
        Viewport::with_physical_size(size, renderer::Scale { window: scale, application: app.ui_scale() });
    let extent = wgpu::Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tinystream snapshot"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    backdrop::set_frame(Some(texture.clone()));
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut cursor = cursor.map_or(mouse::Cursor::Unavailable, mouse::Cursor::Available);
    let mut cache = user_interface::Cache::new();
    let waker = iced_core::shell::Waker::new(|| {});
    let start = Instant::now();
    let mut emitted = Vec::new();
    let mut operations: Vec<Box<dyn iced_core::widget::Operation>> = Vec::new();
    // A few frames, far enough apart that entrances finish and pictures are uploaded.
    for frame in 0..6u32 {
        let next = Viewport::with_physical_size(size, renderer::Scale { window: scale, application: app.ui_scale() });
        if let mouse::Cursor::Available(point) = cursor {
            let ratio = viewport.scale_factor() / next.scale_factor();
            cursor = mouse::Cursor::Available(iced_core::Point::new(point.x * ratio, point.y * ratio));
        }
        viewport = next;
        iced_core::Renderer::hint(&mut renderer, viewport.scale());
        let look = app.gallery.as_ref().map_or_else(|| app.session.look(), |g| g.look(app.reduce_motion));
        let theme = look.theme();
        let mut interface = UserInterface::build(app.view(), viewport.logical_size(), cache, &mut renderer);
        for mut operation in operations.drain(..) {
            loop {
                interface.operate(&renderer, &mut *operation);
                match operation.finish() {
                    iced_core::widget::operation::Outcome::Chain(next) => operation = next,
                    _ => break,
                }
            }
        }
        let mut messages = iced_core::shell::Bus::new();
        if frame == 0
            && let Some(gallery) = &app.gallery
        {
            let mut scroll = iced_core::widget::operation::scrollable::scroll_to(
                "gallery".into(),
                iced_core::widget::operation::scrollable::AbsoluteOffset { x: None, y: Some(gallery.scroll) },
                iced_core::widget::operation::Animation::Instant,
            );
            interface.operate(&renderer, &mut scroll);
        }
        let mut events =
            vec![Event::Window(window::Event::RedrawRequested(start + Duration::from_secs(2 * frame as u64 + 2)))];
        if let mouse::Cursor::Available(position) = cursor {
            events.insert(0, Event::Mouse(mouse::Event::CursorMoved { position }));
        }
        if frame == 2 {
            events.extend_from_slice(input);
        }
        let (_, statuses) = interface.update(&window::Headless, &waker, &events, cursor, &mut renderer, &mut messages);
        interface.draw(&mut renderer, &theme, &renderer::Style { text_color: look.ink }, cursor);
        cache = interface.into_cache();
        for (event, status) in events.iter().zip(statuses) {
            if let Some(message) = app.listen(event, status) {
                messages.push(message);
            }
        }
        for message in messages {
            emitted.push(message.clone());
            let mut effects = Vec::new();
            app.update(message, &mut effects);
            for effect in effects {
                if let Effect::Operate(operation) = effect {
                    operations.push(operation);
                }
            }
        }
        renderer.present(Some(look.canvas), format, &view, &viewport);
        iced_core::Renderer::tick(&mut renderer);
        std::thread::sleep(Duration::from_millis(120));
    }

    let row = (size.width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tinystream snapshot"),
        size: (row * size.height) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tinystream snapshot") });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None },
        },
        extent,
    );
    let index = queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None });
    let mapped = slice.get_mapped_range().context("can't read the frame back")?;
    let pixels: Vec<u8> =
        mapped.chunks(row as usize).flat_map(|line| &line[..size.width as usize * 4]).copied().collect();
    ::image::save_buffer(path, &pixels, size.width, size.height, ::image::ColorType::Rgba8)?;
    backdrop::set_frame(None);
    Ok(emitted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::gallery;

    static RENDER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    #[ignore = "requires a Vulkan adapter; renders headlessly without connecting to a display"]
    fn ui_zoom_with_fractional_dpi_keeps_controls_interactive() {
        let _render = RENDER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        use iced_core::keyboard::key::Named;
        use iced_core::keyboard::{self, Key, Modifiers};
        let dir = crate::test_support::Directory::new("zoom-render");
        let mut app = App::new(crate::route::Route::Home, crate::window_state::WindowState::default());
        app.gallery = Some(gallery::Gallery::default());
        let press = |key: Key, modifiers: Modifiers| {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: key.clone(),
                modified_key: key,
                physical_key: keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
                location: keyboard::Location::Standard,
                modifiers,
                text: None,
                repeat: false,
            })
        };
        let path = dir.0.join("zoom.png");
        snapshot(
            &mut app,
            Size::new(1920, 1200),
            1.5,
            None,
            &path,
            &[press(Key::Character("+".into()), Modifiers::CTRL)],
        )
        .unwrap();
        assert_eq!(app.ui_scale(), 1.1);
        assert_eq!(image::open(&path).unwrap().width(), 1920);
        app.gallery.as_mut().unwrap().scroll = 1000.0;
        let messages = snapshot(
            &mut app,
            Size::new(1920, 1200),
            1.5,
            Some(iced_core::Point::new(200.0, 490.0)),
            &path,
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                press(Key::Named(Named::End), Modifiers::empty()),
                press(Key::Named(Named::Enter), Modifiers::empty()),
            ],
        )
        .unwrap();
        assert!(messages.iter().any(|m| matches!(m, Message::Gallery(gallery::Message::Place("Settings")))));
    }

    #[test]
    #[ignore = "requires a Vulkan adapter; renders headlessly without connecting to a display"]
    fn scrolled_artwork_and_modal_keyboard_input() {
        let _render = RENDER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = crate::test_support::Directory::new("design-render");
        let mut app = App::new(crate::route::Route::Home, crate::window_state::WindowState::default());
        app.gallery = Some(gallery::Gallery::default());
        let gallery = app.gallery.as_mut().unwrap();
        gallery.scroll = 2100.0;
        gallery.style = crate::ui::Style::Glass;
        gallery.custom = true;
        let path = dir.0.join("scrolled.png");
        snapshot(&mut app, Size::new(1280, 800), 1.0, None, &path, &[]).unwrap();
        let pixels = image::open(&path).unwrap().into_rgb8();
        let artwork = pixels
            .pixels()
            .filter(|p| {
                let (low, high) = (p.0.iter().min().unwrap(), p.0.iter().max().unwrap());
                high - low > 70
            })
            .count();
        assert!(artwork > 50_000, "scrolled artwork disappeared: {artwork} coloured pixels");

        app.gallery.as_mut().unwrap().scroll = 3000.0;
        let loading = dir.0.join("loading.png");
        snapshot(&mut app, Size::new(1280, 800), 1.0, None, &loading, &[]).unwrap();
        let digits = image::open(loading).unwrap().into_rgb8();
        let ink = (675..715)
            .flat_map(|y| (40..80).map(move |x| (x, y)))
            .filter(|&(x, y)| digits.get_pixel(x, y).0.iter().all(|c| *c > 180))
            .count();
        assert!(ink > 10, "scrolled ticker disappeared");

        app.gallery.as_mut().unwrap().update(gallery::Message::Dialog(true), false);
        let escape = Event::Keyboard(iced_core::keyboard::Event::KeyPressed {
            key: iced_core::keyboard::Key::Named(iced_core::keyboard::key::Named::Escape),
            modified_key: iced_core::keyboard::Key::Named(iced_core::keyboard::key::Named::Escape),
            physical_key: iced_core::keyboard::key::Physical::Unidentified(
                iced_core::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced_core::keyboard::Location::Standard,
            modifiers: iced_core::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        let messages =
            snapshot(&mut app, Size::new(1280, 800), 1.0, None, &dir.0.join("closed.png"), &[escape]).unwrap();
        assert!(messages.iter().any(|m| matches!(m, Message::Gallery(gallery::Message::Dialog(false)))));
        assert!(!messages.iter().any(|m| matches!(m, Message::Navigate(_))));
    }
}
