// SPDX-License-Identifier: AGPL-3.0-or-later

//! The Downloaded page, and the download buttons and settings around the app.

use iced_core::Length::Fill;
use iced_core::{Alignment, Padding};
use iced_widget::{Column, Element, Row, Widget, column, container, row, space};
use serde_json::{Value, json};

use super::{Downloads, Index, Key, Message, Status, store};
use crate::app::{App, Message as AppMessage};
use crate::route::Route;
use crate::ui::Look;
use crate::ui::controls::{Tone, badge, button, icon_button, progress};
use crate::ui::icon::Icon;
use crate::ui::text::{self as ty, MEDIUM, SEMIBOLD, text, truncate};

type View<'a> = Element<'a, AppMessage>;

fn send(message: Message) -> AppMessage {
    AppMessage::Offline(message)
}

/// "1.4 GB", as the web writes sizes.
pub fn size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", units[unit]) }
}

/// What the server said about `key`, from what's kept.
fn answer(index: &Index, key: Key) -> Option<&Value> {
    let (operation, field, variables) = match key {
        Key::Video(id) => ("Playback", "video", json!({ "id": id })),
        Key::Track(id) => ("MusicTrack", "track", json!({ "id": id })),
        Key::Album(id) => ("MusicAlbum", "album", json!({ "id": id })),
        Key::Playlist(id) => ("MusicPlaylist", "playlist", json!({ "id": id })),
    };
    let item = index.items.get(&key)?;
    item.answers.get(&store::answer_key(operation, &variables)).map(|data| &data[field]).filter(|v| v.is_object())
}

/// What a download is called, and a line about it, once the server's said.
fn describe(index: &Index, key: Key) -> (String, String, Option<String>) {
    let Some(v) = answer(index, key) else {
        let what = match key {
            Key::Video(_) => "Video",
            Key::Track(_) => "Song",
            Key::Album(_) => "Album",
            Key::Playlist(_) => "Playlist",
        };
        return (what.into(), "Waiting for its details".into(), None);
    };
    let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
    match key {
        Key::Video(_) => {
            let title = s(&v["title"]["name"]);
            let name = v["name"].as_str().or(v["label"].as_str()).map_or_else(|| title.clone(), str::to_owned);
            let about = match v["label"].as_str() {
                Some(label) if title != name => format!("{title} · {label}"),
                _ => title,
            };
            (name, about, v["still"].as_str().map(str::to_owned))
        },
        Key::Track(_) => (s(&v["title"]), s(&v["artist"]), v["cover"].as_str().map(str::to_owned)),
        Key::Album(_) => (s(&v["name"]), s(&v["artist"]), v["cover"].as_str().map(str::to_owned)),
        Key::Playlist(_) => (
            s(&v["name"]),
            format!("{} songs", v["trackCount"].as_i64().unwrap_or(0)),
            v["covers"][0].as_str().map(str::to_owned),
        ),
    }
}

fn picture<'a>(app: &'a App, path: Option<String>, width: f32, height: f32, radius: f32, look: &Look) -> View<'a> {
    let path = path.filter(|p| !p.is_empty());
    container(crate::browsing::art(&app.browsing, path.as_deref(), radius, look)).width(width).height(height).boxed()
}

/// The ⏸/▶/✕ beside a download, by how it's doing.
fn controls<'a>(key: Key, status: Status, look: &Look) -> View<'a> {
    let mut buttons = Row::<View<'a>>::new().spacing(2).align_y(Alignment::Center);
    match status {
        Status::Queued | Status::Downloading(_) => {
            buttons = buttons.push(icon_button(Icon::Pause, "Pause", Some(send(Message::Pause(key))), look));
        },
        Status::Paused | Status::Failed => {
            buttons = buttons.push(icon_button(Icon::Play, "Resume", Some(send(Message::Resume(key))), look));
        },
        Status::Done | Status::Missing => {},
    }
    buttons.push(icon_button(Icon::Trash2, "Remove download", Some(send(Message::Remove(key))), look)).boxed()
}

fn state<'a>(status: Status, error: Option<&str>, look: &Look) -> View<'a> {
    match status {
        Status::Queued => badge("Waiting", Tone::Quiet, look),
        Status::Paused => badge("Paused", Tone::Quiet, look),
        Status::Downloading(_) => badge("Downloading", Tone::Live, look),
        Status::Failed => badge(error.map_or_else(|| "Failed".to_owned(), str::to_owned), Tone::Danger, look),
        Status::Done => badge("Downloaded", Tone::Ok, look),
        Status::Missing => space().boxed(),
    }
}

fn row_for<'a>(app: &'a App, key: Key, look: &Look) -> View<'a> {
    let d = &app.downloads;
    let index = &d.index;
    let (name, about, art) = describe(index, key);
    let status = d.status(key);
    let item = &index.items[&key];
    let video = matches!(key, Key::Video(_));
    let mut details = column![
        row![truncate(name, ty::S15, look.ink).font(MEDIUM), state(status, item.error.as_deref(), look)]
            .spacing(10)
            .align_y(Alignment::Center),
        truncate(about, ty::XS, look.ink_3)
    ]
    .spacing(6)
    .width(Fill);
    let mut words = Vec::new();
    match (status, d.active.as_ref().filter(|a| a.key == key)) {
        (Status::Downloading(value), Some(active)) => {
            details = details.push(progress(Some(value), look.info, look));
            words.push(match active.total {
                Some(total) => format!("{} of {}", size(active.done), size(total)),
                None => size(active.done),
            });
        },
        (Status::Downloading(value), None) => {
            details = details.push(progress(Some(value), look.info, look));
            let parts = &item.parts;
            let done = parts.iter().filter(|p| d.status(**p) == Status::Done).count();
            words.push(format!("{done} of {} songs", parts.len()));
        },
        _ if item.size > 0 => words.push(size(parts_size(index, key))),
        _ => {},
    }
    if video && let Some(duration) = answer(index, key).and_then(|v| v["media"]["duration"].as_f64()) {
        words.push(crate::music::model::length(duration));
    }
    if !words.is_empty() {
        details = details.push(text(words.join(" · "), ty::XS, look.ink_3).boxed());
    }
    let (width, height) = if video { (128.0, 72.0) } else { (64.0, 64.0) };
    let open = match key {
        Key::Video(id) => Some(Route::Watch(id as u64, None)),
        Key::Album(id) => Some(Route::Album(id as u64)),
        Key::Playlist(id) => Some(Route::Playlist(id as u64)),
        Key::Track(_) => None,
    };
    let l = *look;
    let body = crate::ui::pressable::pressable(
        row![
            picture(app, art, width, height, 10.0, look),
            container(details).padding(Padding { right: 96.0, ..Padding::ZERO })
        ]
        .spacing(16)
        .align_y(Alignment::Center),
        14.0,
        look,
        move |status| crate::ui::pressable::Appearance {
            paint: if matches!(status, crate::ui::pressable::Status::Hovered | crate::ui::pressable::Status::Pressed) {
                crate::ui::squircle::Paint::fill(l.hover)
            } else {
                crate::ui::squircle::Paint::NONE
            },
            ink: l.ink,
        },
    )
    .padding(12)
    .width(Fill)
    .on_press_maybe(open.filter(|_| status == Status::Done).map(AppMessage::Navigate))
    .boxed();
    iced_widget::Stack::with_children([
        body,
        container(controls(key, status, look)).align_right(Fill).center_y(Fill).padding(12).boxed(),
    ])
    .width(Fill)
    .boxed()
}

/// Bytes kept for `key` and the tracks it holds.
fn parts_size(index: &Index, key: Key) -> u64 {
    let Some(item) = index.items.get(&key) else { return 0 };
    item.size + item.parts.iter().filter_map(|p| index.items.get(p)).map(|p| p.size).sum::<u64>()
}

fn section<'a>(name: &'a str, rows: Vec<View<'a>>, look: &Look) -> View<'a> {
    column![text(name, ty::S15, look.ink).font(SEMIBOLD), Column::with_children(rows).spacing(4).width(Fill)]
        .spacing(10)
        .padding(Padding { bottom: 36.0, ..Padding::ZERO })
        .width(Fill)
        .boxed()
}

pub fn page<'a>(app: &'a App, look: &Look) -> View<'a> {
    let d = &app.downloads;
    let index = &d.index;
    let mut title = Row::<View<'a>>::new()
        .push(truncate("Downloaded", ty::TITLE, look.ink).font(SEMIBOLD).width(Fill).boxed())
        .spacing(8)
        .align_y(Alignment::End);
    let mut page = Column::<View<'a>>::new().width(Fill);
    if !d.on {
        page = page.push(container(title).padding(Padding { bottom: 32.0, ..Padding::ZERO }).boxed());
        page = page.push(crate::ui::layout::empty(
            "Offline downloads are off",
            Some("Turn them on in Settings → Desktop to keep episodes, films and music on this computer."),
            look,
        ));
        page = page.push(
            container(button("Open Settings", look).on_press(AppMessage::Navigate(Route::Settings)).boxed())
                .center_x(Fill)
                .boxed(),
        );
        return crate::ui::layout::page(page, false);
    }

    let kept: Vec<Key> = index
        .items
        .iter()
        .filter(|(key, item)| item.kept && (!key.is_track() || !index.held(**key, None)))
        .map(|(key, _)| *key)
        .collect();
    let (waiting, done): (Vec<Key>, Vec<Key>) = kept.iter().partition(|key| d.status(**key) != Status::Done);
    let total: u64 = index.items.values().map(|item| item.size).sum();
    if total > 0 {
        title = title.push(text(size(total), ty::SM, look.ink_3).boxed());
    }
    if waiting.iter().any(|k| matches!(d.status(*k), Status::Queued | Status::Downloading(_))) {
        title = title.push(
            button("Pause all", look).plain().small().icon(Icon::Pause).on_press(send(Message::PauseAll)).boxed(),
        );
    } else if waiting.iter().any(|k| matches!(d.status(*k), Status::Paused | Status::Failed)) {
        title = title.push(
            button("Resume all", look).plain().small().icon(Icon::Play).on_press(send(Message::ResumeAll)).boxed(),
        );
    }
    page = page.push(container(title).padding(Padding { bottom: 32.0, ..Padding::ZERO }).boxed());
    if !d.allowed && !app.session.offline && app.session.connection.is_some() {
        page = page.push(
            container(crate::ui::controls::banner(
                Icon::ShieldAlert,
                look.warn,
                text("Your server doesn't let you download anymore. What's here still plays.", ty::SM, look.ink),
                None,
                look,
            ))
            .padding(Padding { bottom: 24.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    if kept.is_empty() {
        page = page.push(crate::ui::layout::empty(
            "Nothing downloaded yet",
            Some("Download episodes, films, albums and playlists from their pages to watch and listen offline."),
            look,
        ));
        return crate::ui::layout::page(page, false);
    }
    if !waiting.is_empty() {
        let mut waiting = waiting;
        waiting.sort_by_key(|key| index.items[key].order);
        page =
            page.push(section("Downloading", waiting.into_iter().map(|key| row_for(app, key, look)).collect(), look));
    }

    let mut shows: Vec<(String, Vec<(String, Key)>)> = Vec::new();
    let mut music = Vec::new();
    for key in done {
        match key {
            Key::Video(_) => {
                let v = answer(index, key);
                let title = v.and_then(|v| v["title"]["name"].as_str()).unwrap_or_default().to_owned();
                let order = v.and_then(|v| v["label"].as_str()).unwrap_or_default().to_owned();
                match shows.iter_mut().find(|(name, _)| *name == title) {
                    Some((_, videos)) => videos.push((order, key)),
                    None => shows.push((title, vec![(order, key)])),
                }
            },
            _ => music.push(key),
        }
    }
    shows.sort_by_key(|(name, _)| name.to_lowercase());
    for (name, mut videos) in shows {
        videos.sort();
        page =
            page.push(section_owned(name, videos.into_iter().map(|(_, key)| row_for(app, key, look)).collect(), look));
    }
    if !music.is_empty() {
        page = page.push(section("Music", music.into_iter().map(|key| row_for(app, key, look)).collect(), look));
    }
    crate::ui::layout::page(page, false)
}

fn section_owned<'a>(name: String, rows: Vec<View<'a>>, look: &Look) -> View<'a> {
    column![
        truncate(if name.is_empty() { "Videos".to_owned() } else { name }, ty::S15, look.ink).font(SEMIBOLD),
        Column::with_children(rows).spacing(4).width(Fill)
    ]
    .spacing(10)
    .padding(Padding { bottom: 36.0, ..Padding::ZERO })
    .width(Fill)
    .boxed()
}

/// A button for downloading `keys` (an episode, a season, a film, an album or a playlist), saying
/// how that's going; `label` names what it downloads. `wrap` and `go` make its messages, for pages
/// with messages of their own.
pub fn download_button<'a, M: Clone + 'a>(
    downloads: &Downloads,
    keys: Vec<Key>,
    label: &str,
    large: bool,
    wrap: impl Fn(Message) -> M,
    go: impl Fn(Route) -> M,
    look: &Look,
) -> Option<Element<'a, M>> {
    let statuses: Vec<Status> = keys.iter().map(|k| downloads.status(*k)).collect();
    let missing: Vec<Key> =
        keys.iter().zip(&statuses).filter(|(_, s)| **s == Status::Missing).map(|(k, _)| *k).collect();
    if keys.is_empty() || !downloads.on || (!downloads.ready() && missing.len() == keys.len()) {
        return None;
    }
    let done = statuses.iter().filter(|s| **s == Status::Done).count();
    let going = statuses.iter().any(|s| matches!(s, Status::Queued | Status::Downloading(_)));
    let (words, icon, message) = if done == keys.len() {
        ("Downloaded".to_owned(), Icon::CircleCheck, go(Route::Offline))
    } else if !missing.is_empty() && downloads.ready() {
        (label.to_owned(), Icon::Download, wrap(Message::Download(missing)))
    } else if going {
        let words = if keys.len() > 1 { format!("Downloading {done}/{}", keys.len()) } else { "Downloading".into() };
        (words, Icon::HardDriveDownload, go(Route::Offline))
    } else {
        ("Download paused".to_owned(), Icon::CirclePause, go(Route::Offline))
    };
    let button = button(words, look).icon(icon).on_press(message);
    Some(if large { button.large().boxed() } else { button.plain().small().boxed() })
}

/// The download button on an episode's row: download it, or see how it's doing.
pub fn episode_button<'a>(downloads: &Downloads, key: Key, look: &Look) -> Option<View<'a>> {
    if !downloads.on {
        return None;
    }
    let (glyph, label, message) = match downloads.status(key) {
        Status::Missing if downloads.ready() => (Icon::Download, "Download", send(Message::Download(vec![key]))),
        Status::Missing => return None,
        Status::Done => (Icon::HardDriveDownload, "Downloaded", AppMessage::Navigate(Route::Offline)),
        Status::Queued | Status::Downloading(_) => {
            (Icon::ArrowDownToLine, "Downloading", AppMessage::Navigate(Route::Offline))
        },
        Status::Paused | Status::Failed => (Icon::CirclePause, "Download paused", AppMessage::Navigate(Route::Offline)),
    };
    Some(icon_button(glyph, label, Some(message), look))
}

/// What's in Settings → Desktop's Offline downloads card: the switch, and what each saved
/// server's downloads take up.
pub fn settings<'a>(app: &'a App, look: &Look) -> View<'a> {
    let d = &app.downloads;
    let words = column![
        text("Keep downloads", ty::SM, look.ink),
        container(text(
            "Download episodes, films, albums and playlists to play without the server, and fetch the next few songs as you listen. Your server has to allow it too.",
            ty::XS,
            look.ink_3
        ))
        .padding(Padding { top: 2.0, ..Padding::ZERO })
    ]
    .width(Fill);
    let switch = crate::ui::controls::toggle(
        app.session.config.offline_downloads,
        |on| AppMessage::Session(crate::session::Message::OfflineDownloads(on)),
        look,
    );
    let mut content = column![row![words, switch].spacing(16).align_y(Alignment::Center)].spacing(16);
    for server in &app.session.config.servers {
        let Some(origin) = super::origin(&server.url) else { continue };
        let Some(bytes) = d.usage.get(&origin).copied().filter(|b| *b > 0) else { continue };
        content = content.push(
            row![
                column![text(server.name.as_str(), ty::SM, look.ink), text(size(bytes), ty::XS, look.ink_3)]
                    .spacing(2)
                    .width(Fill),
                button("Remove downloads", look).small().on_press(send(Message::Forget(origin))).boxed()
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    content.boxed()
}
