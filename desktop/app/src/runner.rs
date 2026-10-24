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
use crate::style;

pub enum UserEvent {
    Message(Message),
    /// An event from outside the loop for the widgets, like a finished clipboard read.
    Event(Event),
    /// A widget asked to be updated from another thread.
    Wake,
}

pub struct Runner {
    app: App,
    settings: window::Settings,
    proxy: EventLoopProxy<UserEvent>,
    theme: Theme,
    window: Option<Window>,
    clipboard: Clipboard,
    effects: Vec<Effect>,
}

struct Window {
    raw: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    format: wgpu::TextureFormat,
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
    pub fn new(app: App, settings: window::Settings, proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            app,
            settings,
            proxy,
            theme: style::theme(),
            window: None,
            clipboard: Clipboard::new(),
            effects: Vec::new(),
        }
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

        let engine = Engine::new(&adapter, device.clone(), queue, format, None, Shell::headless());
        let renderer = Renderer::new(engine, renderer::Settings::default());

        let size = raw.inner_size();
        let viewport = Viewport::with_physical_size(
            Size::new(size.width, size.height),
            renderer::Scale { window: raw.scale_factor() as f32, application: 1.0 },
        );

        let proxy = self.proxy.clone();
        let mut window = Window {
            raw,
            surface,
            device,
            format,
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

    /// Runs `message` through the app and carries out what it asks for.
    fn update(&mut self, event_loop: &ActiveEventLoop, message: Message) {
        self.app.update(message, &mut self.effects);

        for effect in std::mem::take(&mut self.effects) {
            match effect {
                Effect::Exit => event_loop.exit(),
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

        if let Some(window) = &self.window {
            window.raw.request_redraw();
        }
    }

    /// Feeds the window's pending events to the widgets, and their messages to the app.
    fn process(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = &mut self.window else { return };
        if window.events.is_empty() && window.operations.is_empty() {
            return;
        }

        let events = std::mem::take(&mut window.events);
        let mut messages = shell::Bus::new();
        let mut interface = UserInterface::build(
            self.app.view(),
            window.viewport.logical_size(),
            std::mem::take(&mut window.cache),
            &mut window.renderer,
        );

        for mut operation in window.operations.drain(..) {
            interface.operate(&window.renderer, operation.as_mut());
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
            .filter_map(|(event, status)| self.app.listen(event, status))
            .collect::<Vec<_>>();

        for message in messages.into_iter().chain(shortcuts) {
            self.update(event_loop, message);
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        self.process(event_loop);
        let Some(window) = &mut self.window else { return };
        let size = window.raw.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        if window.resized {
            window.configure();
        }

        let mut interface = UserInterface::build(
            self.app.view(),
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
        interface.draw(&mut window.renderer, &self.theme, &renderer::Style { text_color: style::INK }, window.cursor);
        window.cache = interface.into_cache();
        window.react(state, &mut self.clipboard, &self.proxy);

        let presented = iced_wgpu::window::compositor::present(
            &mut window.renderer,
            &mut window.surface,
            &window.viewport,
            style::CANVAS,
            || window.raw.pre_present_notify(),
        );
        if presented.is_err() {
            window.resized = true;
            window.raw.request_redraw();
        }

        for message in messages {
            self.update(event_loop, message);
        }
    }
}

impl Window {
    fn configure(&mut self) {
        let size = self.raw.inner_size();
        self.viewport = Viewport::with_physical_size(
            Size::new(size.width, size.height),
            renderer::Scale { window: self.raw.scale_factor() as f32, application: 1.0 },
        );
        if size.width > 0 && size.height > 0 {
            self.surface.configure(
                &self.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
        }
        self.resized = false;
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
                    self.raw.show_window_menu(winit::dpi::LogicalPosition::new(point.x, point.y));
                }
                Ok(())
            },
            Action::Close => Ok(()),
        };
        if let Err(error) = result {
            tracing::debug!("window action {action:?} failed: {error}");
        }
    }

    fn flags(&self) -> Message {
        let size = self.viewport.logical_size();

        Message::Resized { size, maximized: self.raw.is_maximized(), fullscreen: self.raw.fullscreen().is_some() }
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
            },
            Err(error) => {
                tracing::error!("can't open the window: {error}");
                event_loop.exit();
            },
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause
            && let Some(window) = &mut self.window
        {
            window.redraw_at = None;
            window.raw.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = &mut self.window else { return };

        match &event {
            WindowEvent::RedrawRequested => {
                self.redraw(event_loop);
                return;
            },
            WindowEvent::CloseRequested => {
                self.update(event_loop, Message::CloseRequested);
                return;
            },
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                window.resized = true;
                window.configure();
                let flags = window.flags();
                self.update(event_loop, flags);
            },
            WindowEvent::CursorMoved { position, .. } => {
                window.cursor =
                    mouse::Cursor::Available(conversion::cursor_position(*position, window.viewport.scale_factor()));
            },
            WindowEvent::CursorLeft { .. } => window.cursor = mouse::Cursor::Unavailable,
            WindowEvent::ModifiersChanged(modifiers) => window.modifiers = modifiers.state(),
            _ => {},
        }

        let Some(window) = &mut self.window else { return };
        if let Some(event) = conversion::window_event(event, window.viewport.scale_factor(), window.modifiers) {
            window.events.push(event);
        }
        self.process(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Message(message) => self.update(event_loop, message),
            UserEvent::Event(event) => {
                if let Some(window) = &mut self.window {
                    window.events.push(event);
                }
                self.process(event_loop);
            },
            UserEvent::Wake => {
                if let Some(window) = &self.window {
                    window.raw.request_redraw();
                }
            },
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.window.as_ref().and_then(|window| window.redraw_at) {
            Some(at) => event_loop.set_control_flow(ControlFlow::WaitUntil(at)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}
