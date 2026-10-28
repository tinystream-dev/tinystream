// SPDX-License-Identifier: AGPL-3.0-or-later

//! The event loop. The app owns winit and the wgpu device instead of going through iced's runtime,
//! so it can open the device its own way (#86 needs Vulkan's external memory and semaphores) and
//! call the window directly. iced's widgets are built, updated and drawn here, as in iced's
//! `integration` example.

use std::sync::Arc;
use std::time::Instant;

use iced_core::widget::Operation;
use iced_core::window::RedrawRequest;
use iced_core::{Event, Size, Theme, clipboard, mouse, renderer, shell, window};
use iced_wgpu::graphics::{Shell, Viewport};
use iced_wgpu::{Engine, Renderer, wgpu};
use iced_widget::Widget;
use iced_winit::runtime::user_interface::{self, UserInterface};
use iced_winit::winit::application::ApplicationHandler;
use iced_winit::winit::event::{StartCause, WindowEvent};
use iced_winit::winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use iced_winit::winit::keyboard::ModifiersState;
use iced_winit::winit::window::WindowId;
use iced_winit::{Clipboard, conversion, winit};

use crate::app::{App, Effect, Message};
use crate::chrome::Action;
use crate::platform::{self, Desktop};

pub enum UserEvent {
    Message(Message),
    /// An event from outside the loop for the widgets, like a finished clipboard read.
    Event(Event),
    /// A widget asked to be updated from another thread.
    Wake,
}

struct RendererNotifier(EventLoopProxy<UserEvent>);

impl iced_wgpu::graphics::shell::Notifier for RendererNotifier {
    fn tick(&self) {
        let _ = self.0.send_event(UserEvent::Wake);
    }

    fn request_redraw(&self) {
        let _ = self.0.send_event(UserEvent::Wake);
    }

    fn invalidate_layout(&self) {
        let _ = self.0.send_event(UserEvent::Wake);
    }
}

pub struct Runner {
    app: App,
    runtime: tokio::runtime::Handle,
    live: Option<(u64, tokio::task::JoinHandle<()>)>,
    requests: Vec<tokio::task::JoinHandle<()>>,
    settings: window::Settings,
    proxy: EventLoopProxy<UserEvent>,
    theme: Theme,
    window: Option<Window>,
    pip: Option<Window>,
    clipboard: Clipboard,
    effects: Vec<Effect>,
}

struct Window {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    pip: bool,
    raw: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    /// Whether frames can be drawn into a texture of their own and copied to the surface, so
    /// glass can read back what's behind it.
    readback: bool,
    frame: Option<wgpu::Texture>,
    alpha_mode: wgpu::CompositeAlphaMode,
    renderer: Renderer,
    viewport: Viewport,
    cache: user_interface::Cache,
    cursor: mouse::Cursor,
    modifiers: ModifiersState,
    events: Vec<Event>,
    operations: Vec<Box<dyn Operation>>,
    waker: shell::Waker,
    redraw_at: Option<Instant>,
    resized: bool,
}

impl Runner {
    pub fn new(
        app: App,
        settings: window::Settings,
        proxy: EventLoopProxy<UserEvent>,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        Self {
            app,
            runtime,
            live: None,
            requests: Vec::new(),
            settings,
            proxy,
            theme: crate::ui::Look::default().theme(),
            window: None,
            pip: None,
            clipboard: Clipboard::new(),
            effects: Vec::new(),
        }
    }

    pub async fn finish(&mut self) {
        self.pip = None;
        self.window = None;
        self.app.player = None;
        let requests = std::mem::take(&mut self.requests);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), futures::future::join_all(requests)).await;
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<Window, String> {
        let attributes =
            conversion::window_attributes(self.settings.clone(), "tinystream", 1.0, event_loop.primary_monitor(), None);
        let raw = Arc::new(event_loop.create_window(attributes).map_err(|error| error.to_string())?);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(raw.clone()))
        });
        let surface = instance.create_surface(raw.clone()).map_err(|error| error.to_string())?;

        let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::HighPerformance),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|error| format!("no GPU adapter: {error}"))?;
        tracing::info!("GPU: {}", adapter.get_info().name);

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .filter(|format| {
                format.required_features().is_empty()
                    && !matches!(
                        format,
                        wgpu::TextureFormat::Rgb10a2Unorm
                            | wgpu::TextureFormat::Rgb10a2Uint
                            | wgpu::TextureFormat::Rgba16Float
                    )
            })
            .find(|format| format.is_srgb() == iced_wgpu::graphics::color::GAMMA_CORRECTION)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("the surface supports no formats")?;
        let readback = capabilities.usages.contains(wgpu::TextureUsages::COPY_DST);
        if !readback {
            tracing::info!("the surface can't be copied to; glass will be solid");
        }
        let alpha_mode = if capabilities.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            wgpu::CompositeAlphaMode::Auto
        };

        let descriptor = wgpu::DeviceDescriptor {
            label: Some("tinystream"),
            required_features: adapter.features() & wgpu::Features::SHADER_F16,
            required_limits: wgpu::Limits { max_non_sampler_bindings: 2048, ..wgpu::Limits::default() }
                .using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
        };
        let (device, queue) = match platform::Current::request_device(&adapter, &descriptor) {
            Some(device) => device,
            None => futures::executor::block_on(adapter.request_device(&descriptor))
                .map_err(|error| format!("can't open the GPU: {error}"))?,
        };

        let engine = Engine::new(
            &adapter,
            device.clone(),
            queue.clone(),
            format,
            Some(iced_wgpu::graphics::Antialiasing::MSAAx4),
            Shell::new(RendererNotifier(self.proxy.clone())),
        );
        let renderer = Renderer::new(
            engine,
            renderer::Settings {
                line_height: iced_core::text::LineHeight::Relative(1.5),
                ..renderer::Settings::default()
            },
        );

        let size = raw.inner_size();
        let viewport = Viewport::with_physical_size(
            Size::new(size.width, size.height),
            renderer::Scale { window: raw.scale_factor() as f32, application: self.app.ui_scale() },
        );

        let proxy = self.proxy.clone();
        crate::ui::video::set_window(
            device.clone(),
            Arc::new(move || {
                let _ = proxy.send_event(UserEvent::Wake);
            }),
        );

        let proxy = self.proxy.clone();
        let mut window = Window {
            instance,
            adapter,
            pip: false,
            raw,
            surface,
            device,
            queue,
            format,
            readback,
            frame: None,
            alpha_mode,
            renderer,
            viewport,
            cache: user_interface::Cache::new(),
            cursor: mouse::Cursor::Unavailable,
            modifiers: ModifiersState::default(),
            events: Vec::new(),
            operations: Vec::new(),
            waker: shell::Waker::new(move || {
                let _ = proxy.send_event(UserEvent::Wake);
            }),
            redraw_at: None,
            resized: false,
        };
        window.configure();
        Ok(window)
    }

    fn open_pip(&self, event_loop: &ActiveEventLoop) -> Result<Window, String> {
        let main = self.window.as_ref().ok_or("the main window is unavailable")?;
        let attributes = platform::Current::picture_in_picture_attributes();
        let raw = Arc::new(event_loop.create_window(attributes).map_err(|error| error.to_string())?);
        let surface = main.instance.create_surface(raw.clone()).map_err(|error| error.to_string())?;
        let capabilities = surface.get_capabilities(&main.adapter);
        if !capabilities.formats.contains(&main.format) {
            return Err("the display cannot show the shared video format".into());
        }
        let engine = Engine::new(
            &main.adapter,
            main.device.clone(),
            main.queue.clone(),
            main.format,
            None,
            Shell::new(RendererNotifier(self.proxy.clone())),
        );
        let renderer = Renderer::new(
            engine,
            renderer::Settings { line_height: iced_core::text::LineHeight::Relative(1.5), ..Default::default() },
        );
        let size = raw.inner_size();
        let viewport = Viewport::with_physical_size(
            Size::new(size.width, size.height),
            renderer::Scale { window: raw.scale_factor() as f32, application: 1.0 },
        );
        let proxy = self.proxy.clone();
        let mut window = Window {
            instance: main.instance.clone(),
            adapter: main.adapter.clone(),
            pip: true,
            raw,
            surface,
            device: main.device.clone(),
            queue: main.queue.clone(),
            format: main.format,
            readback: false,
            frame: None,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            renderer,
            viewport,
            cache: user_interface::Cache::new(),
            cursor: mouse::Cursor::Unavailable,
            modifiers: ModifiersState::default(),
            events: Vec::new(),
            operations: Vec::new(),
            waker: shell::Waker::new(move || {
                let _ = proxy.send_event(UserEvent::Wake);
            }),
            redraw_at: None,
            resized: false,
        };
        window.configure();
        platform::Current::picture_in_picture_opened();
        Ok(window)
    }

    /// Runs `message` through the app and carries out what it asks for.
    fn update(&mut self, event_loop: &ActiveEventLoop, message: Message) {
        self.app.update(message, &mut self.effects);

        for effect in std::mem::take(&mut self.effects) {
            match effect {
                Effect::PipWindow(action) => {
                    if let Some(window) = &self.pip {
                        window.act(action);
                    }
                },
                Effect::PictureInPicture(on) => {
                    if on && self.pip.is_none() {
                        match self.open_pip(event_loop) {
                            Ok(window) => {
                                window.raw.request_redraw();
                                self.pip = Some(window);
                            },
                            Err(error) => {
                                if let Some(player) = &self.app.player {
                                    let serial = player.serial;
                                    self.update(
                                        event_loop,
                                        Message::Player(serial, crate::player::Message::PipFailed(error)),
                                    );
                                }
                            },
                        }
                    } else if !on {
                        self.pip = None;
                        if let Some(window) = &self.window {
                            window.raw.focus_window();
                        }
                    }
                },
                Effect::Scale(scale) => {
                    if let Some(window) = &mut self.window {
                        window.rescale(scale);
                    }
                },
                Effect::Exit => {
                    if let Some((_, task)) = self.live.take() {
                        task.abort();
                    }
                    event_loop.exit();
                },
                Effect::Task(task) => {
                    let proxy = self.proxy.clone();
                    self.runtime.spawn(async move {
                        let _ = proxy.send_event(UserEvent::Message(task.await));
                    });
                },
                Effect::Request(task) => {
                    self.requests.retain(|task| !task.is_finished());
                    let proxy = self.proxy.clone();
                    self.requests.push(self.runtime.spawn(async move {
                        let _ = proxy.send_event(UserEvent::Message(task.await));
                    }));
                },
                Effect::Live(api, generation) => {
                    if api.is_none() {
                        for task in self.requests.drain(..) {
                            task.abort();
                        }
                    }
                    if api.is_some() && self.live.as_ref().is_some_and(|(current, _)| *current == generation) {
                        continue;
                    }
                    if let Some((_, task)) = self.live.take() {
                        task.abort();
                    }
                    if let Some(api) = api {
                        let proxy = self.proxy.clone();
                        let task = self.runtime.spawn(async move {
                            api.events(move |event| {
                                let _ = proxy.send_event(UserEvent::Message(Message::Session(
                                    crate::session::Message::Live(generation, event),
                                )));
                            })
                            .await;
                        });
                        self.live = Some((generation, task));
                    }
                },
                Effect::Listen(start) => {
                    let proxy = self.proxy.clone();
                    start(Arc::new(move |message| {
                        let _ = proxy.send_event(UserEvent::Message(message));
                    }));
                },
                Effect::Operate(operation) => {
                    if let Some(window) = &mut self.window {
                        window.operations.push(operation);
                    }
                },
                Effect::Window(action) => {
                    if let Some(window) = &self.window {
                        window.act(action);
                    }
                },
            }
        }

        if !self.app.player.as_ref().is_some_and(|player| player.pip) {
            self.pip = None;
        }
        for window in self.window.iter().chain(self.pip.iter()) {
            window.raw.request_redraw();
        }
    }

    /// Feeds the window's pending events to the widgets, and their messages to the app.
    fn process(&mut self, event_loop: &ActiveEventLoop, pip: bool) {
        let Some(window) = (if pip { &mut self.pip } else { &mut self.window }) else { return };
        if window.events.is_empty() && window.operations.is_empty() {
            return;
        }

        let events = std::mem::take(&mut window.events);
        let mut messages = shell::Bus::new();
        let mut interface = UserInterface::build(
            window_view(&self.app, pip),
            window.viewport.logical_size(),
            std::mem::take(&mut window.cache),
            &mut window.renderer,
        );

        for mut operation in window.operations.drain(..) {
            loop {
                interface.operate(&window.renderer, operation.as_mut());
                match operation.finish() {
                    iced_core::widget::operation::Outcome::Chain(next) => operation = next,
                    _ => break,
                }
            }
        }

        let (state, statuses) = interface.update(
            window.raw.as_ref(),
            &window.waker,
            &events,
            window.cursor,
            &mut window.renderer,
            &mut messages,
        );
        window.cache = interface.into_cache();
        window.react(state, &mut self.clipboard, &self.proxy);
        window.raw.request_redraw();

        let shortcuts = events
            .iter()
            .zip(statuses)
            .filter_map(
                |(event, status)| {
                    if pip { self.app.listen_in(event, status, true) } else { self.app.listen(event, status) }
                },
            )
            .collect::<Vec<_>>();

        for message in messages.into_iter().chain(shortcuts) {
            self.update(event_loop, message);
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop, pip: bool) {
        self.process(event_loop, pip);
        let Some(window) = (if pip { &mut self.pip } else { &mut self.window }) else { return };
        self.app.session.system_dark = window.raw.theme() != Some(winit::window::Theme::Light);
        let look = match &self.app.gallery {
            Some(gallery) => gallery.look(self.app.reduce_motion),
            None => self.app.session.look(),
        };
        self.theme = look.theme();
        iced_core::Renderer::tick(&mut window.renderer);
        let size = window.raw.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if window.resized {
            window.configure();
        }

        let mut interface = UserInterface::build(
            window_view(&self.app, pip),
            window.viewport.logical_size(),
            std::mem::take(&mut window.cache),
            &mut window.renderer,
        );
        let mut messages = shell::Bus::new();
        let (state, _) = interface.update(
            window.raw.as_ref(),
            &window.waker,
            &[Event::Window(window::Event::RedrawRequested(Instant::now()))],
            window.cursor,
            &mut window.renderer,
            &mut messages,
        );
        interface.draw(&mut window.renderer, &self.theme, &renderer::Style { text_color: look.ink }, window.cursor);
        window.cache = interface.into_cache();
        window.react(state, &mut self.clipboard, &self.proxy);

        if !window.present(look.canvas) {
            window.resized = true;
            window.raw.request_redraw();
        }
        if self.app.animating() && (pip || !self.app.player.as_ref().is_some_and(|player| player.pip)) {
            window.raw.request_redraw();
        }

        for message in messages {
            self.update(event_loop, message);
        }
    }
}

impl Window {
    fn rescale(&mut self, application: f32) {
        let old = self.viewport.scale_factor();
        let size = self.raw.inner_size();
        self.viewport = Viewport::with_physical_size(
            Size::new(size.width, size.height),
            renderer::Scale { window: self.raw.scale_factor() as f32, application },
        );
        iced_core::Renderer::hint(&mut self.renderer, self.viewport.scale());
        if let mouse::Cursor::Available(point) = self.cursor {
            let point = iced_core::Point::new(
                point.x * old / self.viewport.scale_factor(),
                point.y * old / self.viewport.scale_factor(),
            );
            self.cursor = mouse::Cursor::Available(point);
            self.events.push(Event::Mouse(mouse::Event::CursorMoved { position: point }));
        }
    }

    fn configure(&mut self) {
        let size = self.raw.inner_size();
        self.rescale(self.viewport.scale().application);
        if size.width > 0 && size.height > 0 {
            let usage = if self.readback {
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST
            } else {
                wgpu::TextureUsages::RENDER_ATTACHMENT
            };
            self.surface.configure(
                &self.device,
                &wgpu::SurfaceConfiguration {
                    usage,
                    format: self.format,
                    width: size.width,
                    height: size.height,
                    present_mode: wgpu::PresentMode::AutoVsync,
                    alpha_mode: self.alpha_mode,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 1,
                    color_space: wgpu::SurfaceColorSpace::Auto,
                },
            );
            self.frame = self.readback.then(|| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("tinystream frame"),
                    size: wgpu::Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            });
            if !self.pip {
                crate::ui::backdrop::set_frame(self.frame.clone());
            }
        }
        self.resized = false;
    }

    /// Draws the frame and shows it, or returns `false` if the surface has to be set up again.
    fn present(&mut self, background: iced_core::Color) -> bool {
        let surface = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface) => surface,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return true,
            _ => return false,
        };
        match &self.frame {
            Some(frame) => {
                let view = frame.create_view(&wgpu::TextureViewDescriptor::default());
                self.renderer.present(Some(background), self.format, &view, &self.viewport);
                let mut encoder = self
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tinystream present") });
                encoder.copy_texture_to_texture(frame.as_image_copy(), surface.texture.as_image_copy(), frame.size());
                self.queue.submit([encoder.finish()]);
            },
            None => {
                let view = surface.texture.create_view(&wgpu::TextureViewDescriptor::default());
                self.renderer.present(Some(background), self.format, &view, &self.viewport);
            },
        }
        self.raw.pre_present_notify();
        self.queue.present(surface);
        crate::ui::video::presented();
        true
    }

    /// Applies what the widgets asked for after an update: the cursor, redraws and the clipboard.
    fn react(&mut self, state: user_interface::State, clipboard: &mut Clipboard, proxy: &EventLoopProxy<UserEvent>) {
        let user_interface::State::Updated { mouse_interaction, redraw_request, clipboard: requests, .. } = state
        else {
            return;
        };

        match conversion::mouse_interaction(mouse_interaction) {
            Some(icon) => {
                self.raw.set_cursor(icon);
                self.raw.set_cursor_visible(true);
            },
            None => self.raw.set_cursor_visible(false),
        }

        match redraw_request {
            RedrawRequest::NextFrame => self.raw.request_redraw(),
            RedrawRequest::At(at) => self.redraw_at = Some(self.redraw_at.map_or(at, |current| current.min(at))),
            RedrawRequest::Wait => {},
        }

        for kind in requests.reads {
            let proxy = proxy.clone();
            clipboard.read(kind, move |result| {
                let _ =
                    proxy.send_event(UserEvent::Event(Event::Clipboard(clipboard::Event::Read(result.map(Arc::new)))));
            });
        }
        if let Some(content) = requests.write {
            let proxy = proxy.clone();
            clipboard.write(content, move |result| {
                let _ = proxy.send_event(UserEvent::Event(Event::Clipboard(clipboard::Event::Written(result))));
            });
        }
    }

    fn act(&self, action: Action) {
        let result = match action {
            Action::Drag => self.raw.drag_window(),
            Action::Resize(direction) => self.raw.drag_resize_window(conversion::resize_direction(direction)),
            Action::ToggleMaximize => {
                self.raw.set_maximized(!self.raw.is_maximized());
                Ok(())
            },
            Action::Minimize => {
                self.raw.set_minimized(true);
                Ok(())
            },
            Action::Menu => {
                if let mouse::Cursor::Available(point) = self.cursor {
                    let scale = self.viewport.scale().application;
                    self.raw.show_window_menu(winit::dpi::LogicalPosition::new(point.x * scale, point.y * scale));
                }
                Ok(())
            },
            Action::Fullscreen(on) => {
                self.raw.set_fullscreen(on.then_some(winit::window::Fullscreen::Borderless(None)));
                Ok(())
            },
            Action::Close => Ok(()),
        };
        if let Err(error) = result {
            tracing::debug!("window action {action:?} failed: {error}");
        }
    }

    fn flags(&self) -> Message {
        let size = self.raw.inner_size().to_logical::<f32>(self.raw.scale_factor());

        Message::Resized {
            size: Size::new(size.width, size.height),
            maximized: self.raw.is_maximized(),
            fullscreen: self.raw.fullscreen().is_some(),
        }
    }
}

impl ApplicationHandler<UserEvent> for Runner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        match self.open(event_loop) {
            Ok(window) => {
                let flags = window.flags();
                window.raw.request_redraw();
                self.window = Some(window);
                self.update(event_loop, flags);
                if self.app.gallery.is_none() {
                    self.update(event_loop, Message::Session(crate::session::Message::Start));
                    let proxy = self.proxy.clone();
                    self.runtime.spawn(async move {
                        loop {
                            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                            if proxy
                                .send_event(UserEvent::Message(Message::Session(crate::session::Message::Tick)))
                                .is_err()
                            {
                                break;
                            }
                        }
                    });
                }
            },
            Err(error) => {
                tracing::error!("can't open the window: {error}");
                event_loop.exit();
            },
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            for window in self.window.iter_mut().chain(self.pip.iter_mut()) {
                window.redraw_at = None;
                window.raw.request_redraw();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let pip = self.pip.as_ref().is_some_and(|window| window.raw.id() == id);
        if !pip && !self.window.as_ref().is_some_and(|window| window.raw.id() == id) {
            return;
        }
        let Some(window) = (if pip { &mut self.pip } else { &mut self.window }) else { return };

        match &event {
            WindowEvent::RedrawRequested => {
                self.redraw(event_loop, pip);
                return;
            },
            WindowEvent::CloseRequested => {
                if pip {
                    if let Some(player) = &self.app.player {
                        self.update(
                            event_loop,
                            Message::Player(player.serial, crate::player::Message::PictureInPicture),
                        );
                    }
                } else {
                    self.update(event_loop, Message::CloseRequested);
                }
                return;
            },
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                window.resized = true;
                window.configure();
                if !pip {
                    let flags = window.flags();
                    self.update(event_loop, flags);
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                window.cursor =
                    mouse::Cursor::Available(conversion::cursor_position(*position, window.viewport.scale_factor()));
            },
            WindowEvent::CursorLeft { .. } => window.cursor = mouse::Cursor::Unavailable,
            WindowEvent::ModifiersChanged(modifiers) => window.modifiers = modifiers.state(),
            _ => {},
        }

        let Some(window) = (if pip { &mut self.pip } else { &mut self.window }) else { return };
        if let Some(event) = conversion::window_event(event, window.viewport.scale_factor(), window.modifiers) {
            window.events.push(event);
        }
        self.process(event_loop, pip);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Message(message) => self.update(event_loop, message),
            UserEvent::Event(event) => {
                if let Some(window) = &mut self.window {
                    window.events.push(event);
                }
                self.process(event_loop, false);
            },
            UserEvent::Wake => {
                if let Some(window) = self.pip.as_ref().or(self.window.as_ref()) {
                    window.raw.request_redraw();
                }
            },
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.window.iter().chain(self.pip.iter()).filter_map(|window| window.redraw_at).min() {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}

fn window_view(app: &App, pip: bool) -> iced_widget::Element<'_, Message> {
    if pip && let Some(player) = &app.player {
        let serial = player.serial;
        return crate::player::pip_view(player, &app.session.look())
            .map(move |message| Message::Player(serial, message))
            .boxed();
    }
    app.view()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use winit::event_loop::EventLoop;
    use winit::platform::wayland::EventLoopBuilderExtWayland;

    use super::*;
    use crate::player::Message as PlayerMessage;
    use crate::route::Route;

    #[test]
    #[ignore = "opens native windows; requires a Wayland compositor and Vulkan"]
    fn picture_in_picture_keeps_one_player_and_survives_navigation() {
        struct Test {
            runner: Runner,
            until: Instant,
            serial: u64,
            rendered: bool,
            fullscreen_requested: bool,
        }
        impl ApplicationHandler<UserEvent> for Test {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                let window = self.runner.open(event_loop).unwrap();
                self.runner.window = Some(window);
                self.runner.update(event_loop, Message::Player(self.serial, PlayerMessage::PictureInPicture));
                let main = self.runner.window.as_ref().unwrap();
                let pip = self.runner.pip.as_ref().unwrap();
                assert_eq!(main.device, pip.device);
                assert_ne!(main.raw.id(), pip.raw.id());
                self.runner.update(event_loop, Message::Navigate(Route::Home));
                assert!(self.runner.pip.is_some());
                assert_eq!(self.runner.app.player.as_ref().unwrap().serial, self.serial);
            }

            fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
                if matches!(event, WindowEvent::RedrawRequested)
                    && self.runner.pip.as_ref().is_some_and(|window| window.raw.id() == id)
                {
                    self.rendered = true;
                }
                self.runner.window_event(event_loop, id, event);
            }

            fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
                self.runner.user_event(event_loop, event);
            }

            fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
                if Instant::now() < self.until {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(self.until));
                    return;
                }
                if self.fullscreen_requested {
                    assert!(self.runner.window.as_ref().unwrap().raw.fullscreen().is_some());
                    event_loop.exit();
                    return;
                }
                assert!(self.rendered, "PiP did not present a frame");
                if std::env::var_os("NIRI_SOCKET").is_some() {
                    let output =
                        std::process::Command::new("niri").args(["msg", "--json", "windows"]).output().unwrap();
                    let windows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                    let window = windows
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|window| {
                            window["pid"].as_u64() == Some(u64::from(std::process::id()))
                                && window["app_id"] == "dev.tinystream.Tinystream.PiP"
                        })
                        .unwrap();
                    assert_eq!(window["is_floating"], true);
                }
                let id = self.runner.pip.as_ref().unwrap().raw.id();
                self.runner.window_event(event_loop, id, WindowEvent::CloseRequested);
                assert!(self.runner.pip.is_none());
                assert!(matches!(self.runner.app.history.current().route, Route::Watch(1, _)));
                assert_eq!(self.runner.app.player.as_ref().unwrap().serial, self.serial);
                self.runner.update(event_loop, Message::Player(self.serial, PlayerMessage::PictureInPicture));
                assert!(self.runner.pip.is_some());
                self.runner.update(event_loop, Message::Player(self.serial, PlayerMessage::Fullscreen));
                assert!(self.runner.pip.is_none());
                self.fullscreen_requested = true;
                self.until = Instant::now() + std::time::Duration::from_millis(500);
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.until));
            }
        }
        let mut builder = EventLoop::<UserEvent>::with_user_event();
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        let event_loop = builder.build().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut app = App::new(Route::Watch(1, None), crate::window_state::WindowState::default());
        app.session.screen = crate::session::Screen::Ready;
        app.player = Some(crate::player::tests::player());
        let serial = app.player.as_ref().unwrap().serial;
        let runner = Runner::new(
            app,
            window::Settings { decorations: false, size: Size::new(960.0, 600.0), ..Default::default() },
            event_loop.create_proxy(),
            runtime.handle().clone(),
        );
        let mut test = Test {
            runner,
            until: Instant::now() + std::time::Duration::from_secs(2),
            serial,
            rendered: false,
            fullscreen_requested: false,
        };
        event_loop.run_app(&mut test).unwrap();
        runtime.block_on(test.runner.finish());
    }
}
