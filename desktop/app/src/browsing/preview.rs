// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deterministic browsing scenes for the existing headless snapshot renderer.

use serde_json::json;

use super::{Browsing, Document, Entry, Key, now, week_start};
use crate::app::{App, Overlay};
use crate::route::{History, Route};

pub fn prepare(app: &mut App, scene: &str, style: crate::ui::Style, custom: bool, still: Option<bool>) -> bool {
    let route = match scene {
        "home" => Route::Home,
        "books" => Route::Library("Books".into()),
        "series" => Route::Series { library: "Books".into(), series: "The northern road".into() },
        "library" => Route::Library("Shows".into()),
        "title" => Route::Title(1),
        "discover" | "add" => Route::Discover,
        "releases" | "series-settings" => Route::Title(1),
        "calendar" => Route::Calendar,
        "requests" => Route::Requests,
        "wanted" => Route::Wanted,
        "downloads" => Route::Downloads,
        "notifications" | "search" => Route::Home,
        _ => return false,
    };
    app.update(crate::app::Message::Shortcut(crate::keys::Shortcut::Zoom(0)), &mut Vec::new());
    if let Some(data) = &app.session.connection {
        let mut data = serde_json::to_value(data).expect("preview connection");
        data["appearance"]["style"] = json!(match style {
            crate::ui::Style::Flat => "FLAT",
            crate::ui::Style::Glass => "GLASS",
            _ => "LAYERED",
        });
        if custom {
            let tokens =
                tinystream_theme::Scheme::decode(crate::ui::gallery::CUSTOM).expect("preview scheme").0.tokens();
            data["appearance"]["light"]["palette"]["tokens"] = json!(
                tinystream_theme::TOKENS
                    .iter()
                    .zip(tokens)
                    .map(|(name, color)| json!({"name":name,"value":color.to_string()}))
                    .collect::<Vec<_>>()
            );
        }
        app.session.connection = Some(serde_json::from_value(data).expect("preview appearance"));
    }
    app.session.config.reduce_motion = still;
    app.session.screen = crate::session::Screen::Ready;
    app.history = History::new(route);
    let b = &mut app.browsing;
    let card = json!({"id":1,"kind":"SHOW","library":"Shows","name":"The quiet orbit","year":2026,"poster":"poster","backdrop":"poster","videoCount":12,"watchedCount":3,"progress":0.35,"freshCount":1});
    let video = json!({"id":10,"season":1,"episode":4,"label":"S01E04","name":"A signal from the shore","overview":"A distant transmission brings the crew back to a place they thought they had left behind.","duration":1440,"position":320,"finished":false,"still":"poster","title":card});
    let series = json!({"id":5,"name":"The quiet orbit","library":"Shows","managed":true,"monitor":"MISSING","status":"CONTINUING","numbering":"AUTO","style":{"file":"{show} - {episode:00}","samples":12},"counts":{"have":3,"total":12,"wanted":8,"upcoming":1},"episodes":[],"sources":[],"groups":[],"aliases":[]});
    b.put(Key::Shell,json!({"viewer":{"id":1,"username":"David","isAdmin":true,"permissions":{"request":true,"manageShows":true,"manageRequests":true,"downloads":true,"editMetadata":true}},"libraries":[{"name":"Shows","kind":"VIDEO","showCount":18}],"server":{"downloads":true,"clips":true}}));
    b.put(Key::Home,json!({"home":{"continueWatching":[{"position":320,"upNext":false,"newEpisode":false,"video":video}],"recentlyAdded":[{"library":"Shows","titles":(0..8).map(|i|{let mut c=card.clone();c["id"]=json!(i+1);c["name"]=json!(["The quiet orbit","Northbound","After the rain","Signals","The last garden","Low tide","Night trains","Paper skies"][i]);c}).collect::<Vec<_>>()}],"popularHere":[]}}));
    let cards = b.data(&Key::Home).unwrap().home.as_ref().unwrap().recently_added[0].titles.clone();
    // Reuse the selection's JSON rather than maintaining a second set of title fixtures.
    let titles = (0..cards.len())
        .map(|i| {
            let mut c = card.clone();
            c["id"] = json!(i + 1);
            c["name"] = json!(cards[i].name);
            c
        })
        .collect::<Vec<_>>();
    b.put(Key::Library("Shows".into()), json!({"library":{"titles":titles}}));
    b.put(Key::Title(1),json!({"title":{"id":1,"kind":"SHOW","name":"The quiet orbit","year":2026,"poster":"poster","backdrop":"poster","overview":"A crew follows a strange transmission across the last inhabited worlds. Every return home reveals something they missed.","genres":["Drama","Science fiction"],"rating":8.4,"videoCount":12,"watchedCount":3,"seasons":[{"number":1,"name":"Season 1","episodes":[video,video,video]}],"nextUp":{"resuming":true,"video":video}}}));
    b.put(Key::Series(1), json!({"title":{"series":series}}));
    b.put(Key::Similar(1), json!({"title":{"similar":{"alsoWatched":[],"recommendations":[]}}}));
    b.put(Key::Music, json!({"musicHome":{"recentlyPlayed":[],"recentlyAdded":[]}}));
    let entry = json!({"seriesId":5,"title":{"id":1},"show":"The quiet orbit","season":1,"episode":5,"name":"The other side","airAt":now()+3600,"state":"WANTED","monitor":"MISSING","poster":"poster","backdrop":"poster"});
    for key in [Key::Calendar(week_start(now())), Key::Soon, Key::Coming] {
        b.put(key, json!({"calendar":[entry]}));
    }
    b.put(Key::Wanted,json!({"wanted":[{"seriesId":5,"title":{"id":1},"show":"The quiet orbit","season":1,"episode":5,"name":"The other side","airAt":now()+3600,"aired":false,"state":"WANTED"}]}));
    b.put(Key::Requests,json!({"requests":[{"id":1,"user":{"id":2,"username":"Mira"},"name":"Paper skies","year":2026,"poster":"poster","library":"Shows","state":"PENDING","createdAt":now()-3600,"note":"Dubbed if there is one, please"}]}));
    b.put(Key::Downloads,json!({"downloads":[{"id":1,"name":"The quiet orbit - S01E04","source":"A configured source","size":1280000000i64,"poster":"poster","state":"DOWNLOADING","importState":"PENDING","live":{"stage":"DOWNLOADING","progress":0.42,"downloadRate":4200000,"uploadRate":128000,"peers":12,"seeds":21,"ratio":0.1,"eta":180}}]}));
    b.put(
        Key::Engine,
        json!({"downloadEngine":{"downloadRate":4200000,"uploadRate":128000,"active":1,"killSwitch":null,"version":"2.0.11","downloadPath":"/srv/media/downloads"}}),
    );
    b.put(Key::Inbox,json!({"notifications":{"unread":1,"items":[{"id":1,"title":"The quiet orbit is ready","body":"Episode 4 is ready to watch.","image":"poster","kind":"READY","createdAt":now()-120,"link":"/watch/10"}]}}));
    b.put(Key::Discover(String::new(),String::new()),json!({"discover":{"library":"Shows","results":[{"id":"20","provider":"TMDB","category":"EPISODES","library":"Shows","name":"Paper skies","year":2026,"poster":"poster","overview":"Two strangers travel north."}]}}));
    b.put(Key::ForYou(String::new()), json!({"forYou":{"shelves":[]}}));
    b.put(Key::Books("Books".into()),json!({"books":[{"id":21,"library":"Books","title":"Northbound","authors":["Mira Vale"],"series":"The northern road","seriesIndex":1,"cover":"poster","reading":{"position":200,"progress":0.4,"finished":false,"updatedAt":now()}},{"id":22,"library":"Books","title":"The last station","authors":["Mira Vale"],"series":"The northern road","seriesIndex":2,"cover":"poster"}]}));
    b.put(Key::Options,json!({"settings":{"libraries":[{"name":"Shows","metadataProvider":"TMDB","managed":true}],"profiles":[{"name":"Default"}],"sources":[],"automation":{"defaultMonitor":"MISSING"}}}));
    if scene == "add" {
        let result =
            b.data(&Key::Discover(String::new(), String::new())).unwrap().discover.as_ref().unwrap().results[0].clone();
        b.dialog = Some(super::Dialog::Pick(std::sync::Arc::new(result)));
        b.pick_library = "Shows".into();
        b.put(Key::Aired("TMDB".into(), "20".into()), json!({"airedEpisodes":12}));
    }
    if scene == "releases" {
        b.dialog =
            Some(super::Dialog::Releases { series: 5, season: 1, episodes: vec![4], name: "The quiet orbit".into() });
        b.put(Key::Releases(5,1,vec![4],String::new()),json!({"series":{"releases":[{"release":{"title":"The quiet orbit S01E04","source":"A configured source","link":"https://example.invalid/release","size":1280000000i64,"seeders":21},"attributes":{"resolution":1080,"codec":"AV1","group":"Group"},"episodes":[{"season":1,"episode":4}],"verdict":{"accepted":true,"score":120,"rejections":[],"warnings":[],"nonstandard":false},"batch":false}]}}));
    }
    if scene == "series-settings" {
        let series = b.data(&Key::Series(1)).unwrap().title.as_ref().unwrap().series.clone().unwrap();
        b.draft = super::SeriesDraft {
            numbering: "AUTO".into(),
            naming: series.style.file.clone(),
            seeding: "default".into(),
            ..Default::default()
        };
        b.put(
            Key::Naming(5, b.draft.naming.clone()),
            json!({"series":{"namingPreview":{"samples":["The quiet orbit - 04.mkv"],"error":null}}}),
        );
        b.dialog = Some(super::Dialog::SeriesSettings(std::sync::Arc::new(series)));
    }
    b.images.arrived("poster".into(), Some(crate::ui::gallery::artwork(3)), false);
    if scene == "notifications" {
        app.overlay = Some(Overlay::Notifications);
    }
    if scene == "search" {
        app.overlay = Some(Overlay::Search);
        app.query = "orbit".into();
        b.search_query = "orbit".into();
        b.put(Key::Search("orbit".into()), json!({"search":{"titles":[card],"videos":[video]}}));
    }
    true
}

impl Browsing {
    fn put(&mut self, key: Key, data: serde_json::Value) {
        self.cache.insert(
            key,
            Entry {
                data: Some(std::sync::Arc::new(
                    serde_json::from_value::<Document>(data).expect("browsing preview selection"),
                )),
                ..Default::default()
            },
        );
    }
}

/// Times scrolling the home page and a library headlessly, frame by frame as the runner draws
/// them. It measures release builds, which need debug assertions for this crate (`snapshot` is
/// only built with them):
///
/// `cargo test --locked --release --config
/// 'profile.release.package.tinystream-desktop.debug-assertions=true' --config 'profile.bench.
/// package.tinystream-desktop.debug-assertions=true' bench::scroll -- --ignored --nocapture`
///
/// `BENCH_SCENES` (`home,library`), `BENCH_TITLES` (400), `BENCH_FRAMES` (120) and `BENCH_EVENTS`
/// change what's measured: with events, every frame has that many wheel events from a touchpad;
/// without, a mouse wheel's notch comes every eight frames and is scrolled smoothly in between.
#[cfg(test)]
mod bench {
    use std::time::{Duration, Instant};

    use iced_core::{Event, Size, mouse, renderer, window};
    use iced_wgpu::graphics::{Shell, Viewport};
    use iced_wgpu::{Engine, Renderer, wgpu};
    use iced_winit::runtime::user_interface::{self, UserInterface};
    use serde_json::json;

    use super::*;
    use crate::app::Message as AppMessage;

    fn env(name: &str, default: usize) -> usize {
        std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    fn fill(app: &mut App, titles: usize) {
        let card = |i: usize| json!({"id":i+1,"kind":"SHOW","library":"Shows","name":format!("Title {i:03}"),"year":2000+i%26,"poster":format!("p{i}"),"backdrop":format!("p{i}"),"videoCount":12,"watchedCount":i%13,"progress":0.35,"freshCount":i%2});
        let entry = |i: usize| json!({"position":320,"upNext":false,"newEpisode":false,"video":{"id":100+i,"label":"S01E04","name":"An episode","duration":1440,"position":320,"still":format!("p{i}"),"title":card(i)}});
        let b = &mut app.browsing;
        b.put(Key::Library("Shows".into()), json!({"library":{"titles":(0..titles).map(card).collect::<Vec<_>>()}}));
        b.put(Key::Home, json!({"home":{
            "continueWatching":(0..12).map(entry).collect::<Vec<_>>(),
            "recentlyAdded":(0..4).map(|l| json!({"library":"Shows","titles":(0..20).map(|i| card(l*20+i)).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "popularHere":(0..20).map(|i| json!({"people":3,"title":card(100+i)})).collect::<Vec<_>>()}}));
        for i in 0..titles.max(120) {
            b.images.arrived(format!("p{i}"), Some(crate::ui::gallery::artwork(i as u32)), false);
        }
    }

    fn run(scene: &str) {
        let (titles, inputs, frames) = (env("BENCH_TITLES", 400), env("BENCH_EVENTS", 0), env("BENCH_FRAMES", 120));
        let mut app = App::new(Route::Home, crate::window_state::WindowState::default());
        tokio::runtime::Runtime::new().unwrap().block_on(app.session.preview(scene)).unwrap();
        prepare(&mut app, scene, crate::ui::Style::Layered, false, None);
        fill(&mut app, titles);

        let size = Size::new(2560u32, 1440);
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
        .unwrap();
        let (device, queue) = futures::executor::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits { max_non_sampler_bindings: 2048, ..wgpu::Limits::default() }
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            }),
        )
        .unwrap();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let antialiasing = Some(iced_wgpu::graphics::Antialiasing::MSAAx4);
        let engine = Engine::new(&adapter, device.clone(), queue.clone(), format, antialiasing, Shell::headless());
        let mut renderer = Renderer::new(
            engine,
            renderer::Settings {
                line_height: iced_core::text::LineHeight::Relative(1.5),
                ..renderer::Settings::default()
            },
        );
        let viewport = Viewport::with_physical_size(size, renderer::Scale { window: 1.0, application: app.ui_scale() });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        crate::ui::backdrop::set_frame(Some(texture.clone()));
        let target = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let cursor = mouse::Cursor::Available(iced_core::Point::new(2540.0, 1430.0));
        let mut cache = user_interface::Cache::new();
        let waker = iced_core::shell::Waker::new(|| {});

        let names = ["view", "build", "update", "draw", "drop", "present", "gpu"];
        let mut stages = [Duration::ZERO; 7];
        let mut worst = Duration::ZERO;
        let epoch = Instant::now();
        let warmup = 20;
        for frame in 0..warmup + frames {
            let mut times = [Duration::ZERO; 7];
            let mut time = |stage: usize, start: Instant| times[stage] += start.elapsed();
            let begun = Instant::now();
            let now = epoch + Duration::from_micros(6061 * frame as u64);
            let down = frame % 200 < 100;
            let delta = if inputs == 0 {
                mouse::ScrollDelta::Lines { x: 0.0, y: if down { -1.0 } else { 1.0 } }
            } else {
                mouse::ScrollDelta::Pixels { x: 0.0, y: if down { -30.0 } else { 30.0 } / inputs as f32 }
            };
            let count = if inputs == 0 { usize::from(frame % 8 == 0) } else { inputs };
            let mut events = vec![Event::Mouse(mouse::Event::WheelScrolled { delta }); count];
            events.push(Event::Window(window::Event::RedrawRequested(now)));
            let mut rebuilt = false;
            loop {
                let look = app.session.look();
                let start = Instant::now();
                let view = app.view();
                time(0, start);
                let start = Instant::now();
                let mut interface = UserInterface::build(view, viewport.logical_size(), cache, &mut renderer);
                time(1, start);
                let mut bus = iced_core::shell::Bus::new();
                let start = Instant::now();
                let _ = interface.update(&window::Headless, &waker, &events, cursor, &mut renderer, &mut bus);
                time(2, start);
                let messages = bus.into_iter().collect::<Vec<_>>();
                let again = !rebuilt && !messages.iter().all(|m| matches!(m, AppMessage::Scrolled(_)));
                if !again {
                    let start = Instant::now();
                    interface.draw(&mut renderer, &look.theme(), &renderer::Style { text_color: look.ink }, cursor);
                    time(3, start);
                }
                let start = Instant::now();
                cache = interface.into_cache();
                time(4, start);
                for message in messages {
                    app.update(message, &mut Vec::new());
                }
                if !again {
                    let start = Instant::now();
                    renderer.present(Some(look.canvas), format, &target, &viewport);
                    time(5, start);
                    let start = Instant::now();
                    let _ = device.poll(wgpu::PollType::wait_indefinitely());
                    time(6, start);
                    iced_core::Renderer::tick(&mut renderer);
                    break;
                }
                rebuilt = true;
                events = vec![Event::Window(window::Event::RedrawRequested(now))];
            }
            if frame >= warmup {
                for (stage, time) in stages.iter_mut().zip(times) {
                    *stage += time;
                }
                worst = worst.max(begun.elapsed());
            }
        }
        crate::ui::backdrop::set_frame(None);

        let ms = |time: Duration| time.as_secs_f64() * 1000.0 / frames as f64;
        let report = names.iter().zip(stages).map(|(n, t)| format!("{n} {:.2}", ms(t))).collect::<Vec<_>>().join("  ");
        println!(
            "{scene} ({titles} titles, {inputs} events a frame): {:.2} ms a frame (worst {:.2}): {report}",
            ms(stages.iter().sum()),
            worst.as_secs_f64() * 1000.0,
        );
    }

    #[test]
    #[ignore = "requires a Vulkan adapter; benchmarks scrolling, in release builds"]
    fn scroll() {
        for scene in std::env::var("BENCH_SCENES").as_deref().unwrap_or("home,library").split(',') {
            run(scene);
        }
    }
}
