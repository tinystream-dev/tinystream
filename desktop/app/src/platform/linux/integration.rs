// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::{mpsc, watch};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use crate::platform::integration::{Command, Integration, Media, Notification};

const PATH: &str = "/org/mpris/MediaPlayer2";
const APP: &str = "dev.tinystream.app";
type Send = Arc<dyn Fn(Command) + std::marker::Send + Sync>;

struct Application(Send);

#[zbus::interface(name = "dev.tinystream.app")]
impl Application {
    fn open(&self, arguments: Vec<String>) {
        (self.0)(Command::Open(arguments));
    }
}

struct Root(Send);

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {
        (self.0)(Command::Show);
    }

    fn quit(&self) {
        (self.0)(Command::Quit);
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "tinystream"
    }

    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        APP
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<&str> {
        vec!["tinystream", "https", "http"]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<&str> {
        vec![]
    }
}

struct Player {
    send: Send,
    media: watch::Receiver<Media>,
}

fn valid(value: f64, min: f64, max: f64) -> zbus::fdo::Result<f64> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(zbus::fdo::Error::InvalidArgs("Value is outside the supported range".into()))
    }
}

fn micros(seconds: f64) -> i64 {
    (seconds * 1_000_000.0) as i64
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) {
        (self.send)(Command::Next);
    }

    fn previous(&self) {
        (self.send)(Command::Previous);
    }

    fn pause(&self) {
        (self.send)(Command::Pause);
    }

    fn play_pause(&self) {
        (self.send)(Command::Toggle);
    }

    fn stop(&self) {
        (self.send)(Command::Stop);
    }

    fn play(&self) {
        (self.send)(Command::Play);
    }

    fn seek(&self, offset: i64) {
        (self.send)(Command::Seek(offset as f64 / 1_000_000.0));
    }

    fn set_position(&self, track_id: ObjectPath<'_>, position: i64) {
        if position >= 0 {
            (self.send)(Command::Position(track_id.to_string(), position as f64 / 1_000_000.0));
        }
    }

    fn open_uri(&self, uri: String) {
        (self.send)(Command::Open(vec![uri]));
    }

    #[zbus(signal)]
    async fn seeked(emitter: &zbus::object_server::SignalEmitter<'_>, position: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> &str {
        let m = self.media.borrow();
        if m.track.is_empty() {
            "Stopped"
        } else if m.playing {
            "Playing"
        } else {
            "Paused"
        }
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.media.borrow().rate.max(0.01)
    }

    #[zbus(property)]
    fn set_rate(&self, rate: f64) -> zbus::fdo::Result<()> {
        (self.send)(Command::Rate(valid(rate, 0.25, 3.0)?));
        Ok(())
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.media.borrow().volume
    }

    #[zbus(property)]
    fn set_volume(&self, volume: f64) -> zbus::fdo::Result<()> {
        (self.send)(Command::Volume(valid(volume, 0.0, 1.0)?));
        Ok(())
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        micros(self.media.borrow().position)
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        0.25
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        3.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.media.borrow().next
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.media.borrow().previous
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        !self.media.borrow().track.is_empty()
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.can_play()
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.media.borrow().seek
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let m = self.media.borrow();
        let mut values = HashMap::new();
        if let Ok(track) = ObjectPath::try_from(m.track.as_str()) {
            values.insert("mpris:trackid".into(), OwnedValue::from(track));
            values.insert("mpris:length".into(), micros(m.length).into());
            for (key, value) in [("xesam:title", &m.title), ("xesam:album", &m.album), ("mpris:artUrl", &m.art)] {
                values.insert(key.into(), Value::from(value.as_str()).try_to_owned().unwrap());
            }
            values.insert("xesam:artist".into(), Value::from(vec![m.artist.as_str()]).try_to_owned().unwrap());
        }
        values
    }
}

struct Tray {
    send: Send,
    media: watch::Receiver<Media>,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Tray {
    fn activate(&self, _x: i32, _y: i32) {
        (self.send)(Command::ToggleWindow);
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {
        (self.send)(Command::Toggle);
    }

    fn context_menu(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        APP
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "tinystream"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        if self.media.borrow().tray { "Active" } else { "Passive" }
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        APP
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        static PIXMAP: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
        let pixels = PIXMAP.get_or_init(|| {
            let image =
                image::load_from_memory(include_bytes!("../../../resources/tray.png")).expect("tray icon").to_rgba8();
            image.pixels().flat_map(|p| [p[3], p[0], p[1], p[2]]).collect()
        });
        vec![(64, 64, pixels.clone())]
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> ObjectPath<'_> {
        ObjectPath::from_static_str_unchecked("/Menu")
    }

    #[zbus(signal)]
    async fn new_status(emitter: &zbus::object_server::SignalEmitter<'_>, status: &str) -> zbus::Result<()>;
}

struct Menu {
    send: Send,
    media: watch::Receiver<Media>,
    revision: std::sync::atomic::AtomicU32,
}
type Properties = HashMap<String, OwnedValue>;
type Layout = (i32, Properties, Vec<OwnedValue>);

impl Menu {
    fn item(&self, id: i32) -> Properties {
        let m = self.media.borrow();
        let label = match id {
            1 => {
                if m.title.is_empty() {
                    "Nothing playing"
                } else {
                    &m.title
                }
            },
            2 => {
                if m.playing {
                    "Pause"
                } else {
                    "Play"
                }
            },
            3 => "Previous",
            4 => "Next",
            5 => "Show tinystream",
            6 => "Quit",
            _ => "",
        };
        let enabled = match id {
            1 => false,
            2 => !m.track.is_empty(),
            3 => m.previous,
            4 => m.next,
            _ => true,
        };
        HashMap::from([
            ("label".into(), Value::from(label).try_to_owned().unwrap()),
            ("enabled".into(), enabled.into()),
            ("visible".into(), true.into()),
        ])
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(&self, parent_id: i32, recursion_depth: i32, _property_names: Vec<String>) -> (u32, Layout) {
        let children = if parent_id == 0 && recursion_depth != 0 {
            (1..=6)
                .map(|id| Value::from((id, self.item(id), Vec::<OwnedValue>::new())).try_to_owned().unwrap())
                .collect()
        } else {
            vec![]
        };
        (self.revision.load(std::sync::atomic::Ordering::Relaxed), (parent_id, self.item(parent_id), children))
    }

    fn get_group_properties(&self, ids: Vec<i32>, _property_names: Vec<String>) -> Vec<(i32, Properties)> {
        ids.into_iter().map(|id| (id, self.item(id))).collect()
    }

    fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<OwnedValue> {
        self.item(id).remove(name).ok_or_else(|| zbus::fdo::Error::InvalidArgs("Unknown property".into()))
    }

    fn event(&self, id: i32, event_id: &str, _data: Value<'_>, _timestamp: u32) {
        if event_id == "clicked" {
            let command = match id {
                2 => Command::Toggle,
                3 => Command::Previous,
                4 => Command::Next,
                5 => Command::Show,
                6 => Command::Quit,
                _ => return,
            };
            (self.send)(command);
        }
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        for (id, event, data, timestamp) in events {
            self.event(id, &event, data.into(), timestamp);
        }
        vec![]
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        vec![]
    }

    #[zbus(signal)]
    async fn layout_updated(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}

pub async fn start(arguments: Vec<String>, send: Send) -> Result<Option<Integration>, String> {
    start_inner(arguments, send).await.map_err(|e| e.to_string())
}

async fn start_inner(arguments: Vec<String>, send: Send) -> zbus::Result<Option<Integration>> {
    let connection = zbus::Connection::session().await?;
    use zbus::fdo::{DBusProxy, RequestNameFlags, RequestNameReply};
    let bus = DBusProxy::new(&connection).await?;
    connection.object_server().at("/dev/tinystream/app", Application(send.clone())).await?;
    let reply = bus.request_name(APP.try_into()?, RequestNameFlags::DoNotQueue.into()).await?;
    if reply == RequestNameReply::Exists {
        let app = zbus::Proxy::new(&connection, APP, "/dev/tinystream/app", APP).await?;
        app.call::<_, _, ()>("Open", &(arguments,)).await?;
        return Ok(None);
    }
    let (media, state) = watch::channel(Media::default());
    let (notifications, inbox) = mpsc::unbounded_channel();
    connection.object_server().at(PATH, Root(send.clone())).await?;
    connection.object_server().at(PATH, Player { send: send.clone(), media: state.clone() }).await?;
    connection.request_name("org.mpris.MediaPlayer2.tinystream").await?;
    connection.object_server().at("/StatusNotifierItem", Tray { send: send.clone(), media: state.clone() }).await?;
    connection
        .object_server()
        .at("/Menu", Menu { send: send.clone(), media: state.clone(), revision: std::sync::atomic::AtomicU32::new(1) })
        .await?;
    let task = tokio::spawn(async move {
        let notifications = notifications_loop(connection.clone(), inbox, send.clone());
        let tray = tray_loop(connection.clone(), send);
        let changes = media_loop(connection.clone(), state);
        futures::future::join3(notifications, tray, changes).await;
    });
    Ok(Some(Integration { media, notifications, task }))
}

async fn media_loop(connection: zbus::Connection, mut state: watch::Receiver<Media>) {
    let mut previous = state.borrow().clone();
    while state.changed().await.is_ok() {
        let current = state.borrow_and_update().clone();
        if let Ok(player) = connection.object_server().interface::<_, Player>(PATH).await {
            let p = player.get().await;
            let emitter = player.signal_emitter();
            if current.track != previous.track
                || current.title != previous.title
                || current.artist != previous.artist
                || current.album != previous.album
                || current.art != previous.art
                || current.length != previous.length
            {
                let _ = p.metadata_changed(emitter).await;
            }
            if current.playing != previous.playing || current.track != previous.track {
                let _ = p.playback_status_changed(emitter).await;
            }
            if current.rate != previous.rate {
                let _ = p.rate_changed(emitter).await;
            }
            if current.volume != previous.volume {
                let _ = p.volume_changed(emitter).await;
            }
            if current.next != previous.next {
                let _ = p.can_go_next_changed(emitter).await;
            }
            if current.previous != previous.previous {
                let _ = p.can_go_previous_changed(emitter).await;
            }
            if current.seek != previous.seek {
                let _ = p.can_seek_changed(emitter).await;
            }
            if current.track != previous.track {
                let _ = p.can_play_changed(emitter).await;
                let _ = p.can_pause_changed(emitter).await;
            }
            if current.discontinuities != previous.discontinuities {
                let _ = Player::seeked(emitter, micros(current.position)).await;
            }
        }
        if current.tray != previous.tray
            && let Ok(tray) = connection.object_server().interface::<_, Tray>("/StatusNotifierItem").await
        {
            let _ = Tray::new_status(tray.signal_emitter(), if current.tray { "Active" } else { "Passive" }).await;
        }
        if (current.title != previous.title
            || current.playing != previous.playing
            || current.next != previous.next
            || current.previous != previous.previous)
            && let Ok(menu) = connection.object_server().interface::<_, Menu>("/Menu").await
        {
            let revision = menu.get().await.revision.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = Menu::layout_updated(menu.signal_emitter(), revision, 0).await;
        }
        previous = current;
    }
}

async fn tray_loop(connection: zbus::Connection, send: Send) {
    let Ok(bus) = zbus::fdo::DBusProxy::new(&connection).await else { return };
    let Ok(mut changes) = bus.receive_name_owner_changed().await else { return };
    loop {
        let registered = match zbus::Proxy::new(
            &connection,
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
        )
        .await
        {
            Ok(watcher) => {
                watcher.call::<_, _, ()>("RegisterStatusNotifierItem", &("/StatusNotifierItem",)).await.is_ok()
                    && watcher.get_property::<bool>("IsStatusNotifierHostRegistered").await.unwrap_or(false)
            },
            Err(_) => false,
        };
        send(Command::Tray(registered));
        let mut changed = false;
        while let Some(change) = changes.next().await {
            if change.args().is_ok_and(|args| args.name().as_str() == "org.kde.StatusNotifierWatcher") {
                changed = true;
                break;
            }
        }
        if !changed {
            send(Command::Tray(false));
            return;
        }
    }
}

async fn notifications_loop(
    connection: zbus::Connection,
    mut inbox: mpsc::UnboundedReceiver<Notification>,
    send: Send,
) {
    let Ok(proxy) = zbus::Proxy::new(
        &connection,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    else {
        return;
    };
    let Ok(mut actions) = proxy.receive_signal("ActionInvoked").await else { return };
    let Ok(mut closed) = proxy.receive_signal("NotificationClosed").await else { return };
    let mut links = HashMap::<u32, String>::new();
    loop {
        enum Event {
            Notice(Option<Notification>),
            Action(Option<zbus::Message>),
            Closed(Option<zbus::Message>),
        }
        let event = {
            let notice = Box::pin(async { Event::Notice(inbox.recv().await) });
            let action = Box::pin(async { Event::Action(actions.next().await) });
            let closed = Box::pin(async { Event::Closed(closed.next().await) });
            match futures::future::select(notice, futures::future::select(action, closed)).await {
                futures::future::Either::Left((event, _)) => event,
                futures::future::Either::Right((futures::future::Either::Left((event, _)), _)) => event,
                futures::future::Either::Right((futures::future::Either::Right((event, _)), _)) => event,
            }
        };
        match event {
            Event::Notice(Some(n)) => {
                let hints = HashMap::from([("desktop-entry", Value::from(APP))]);
                match proxy
                    .call::<_, _, u32>(
                        "Notify",
                        &(
                            "tinystream",
                            0u32,
                            APP,
                            n.title,
                            n.body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"),
                            vec!["default", "Open"],
                            hints,
                            -1i32,
                        ),
                    )
                    .await
                {
                    Ok(id) => {
                        links.insert(id, n.link);
                    },
                    Err(error) => tracing::debug!("desktop notification failed: {error}"),
                }
            },
            Event::Action(Some(signal)) => {
                if let Ok((id, action)) = signal.body().deserialize::<(u32, String)>()
                    && action == "default"
                    && let Some(link) = links.remove(&id)
                {
                    send(Command::Notification(link));
                }
            },
            Event::Closed(Some(signal)) => {
                if let Ok((id, _)) = signal.body().deserialize::<(u32, u32)>() {
                    links.remove(&id);
                }
            },
            _ => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Notifications(mpsc::UnboundedSender<(String, String)>);
    #[zbus::interface(name = "org.freedesktop.Notifications")]
    impl Notifications {
        #[allow(clippy::too_many_arguments)]
        fn notify(
            &self,
            _app_name: &str,
            _replaces_id: u32,
            _icon: &str,
            summary: String,
            body: String,
            _actions: Vec<String>,
            _hints: HashMap<String, OwnedValue>,
            _timeout: i32,
        ) -> u32 {
            self.0.send((summary, body)).unwrap();
            1
        }

        #[zbus(signal)]
        async fn action_invoked(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            id: u32,
            action: &str,
        ) -> zbus::Result<()>;

        #[zbus(signal)]
        async fn notification_closed(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            id: u32,
            reason: u32,
        ) -> zbus::Result<()>;
    }

    #[test]
    #[ignore = "run inside dbus-run-session; owns the application's bus names"]
    fn services_export_controls_and_forward_a_second_launch() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mock = zbus::Connection::session().await.unwrap();
            let (notifications, mut arrivals) = mpsc::unbounded_channel();
            mock.object_server().at("/org/freedesktop/Notifications", Notifications(notifications)).await.unwrap();
            mock.request_name("org.freedesktop.Notifications").await.unwrap();
            let (tx, mut rx) = mpsc::unbounded_channel();
            let send: Send = Arc::new(move |command| {
                let _ = tx.send(command);
            });
            let integration = start(vec![], send.clone()).await.unwrap().unwrap();
            integration
                .media
                .send(Media {
                    track: "/dev/tinystream/track/1".into(),
                    title: "Episode".into(),
                    artist: "Series".into(),
                    length: 120.0,
                    position: 15.0,
                    playing: true,
                    rate: 1.0,
                    volume: 0.8,
                    seek: true,
                    next: true,
                    ..Default::default()
                })
                .unwrap();
            let connection = zbus::Connection::session().await.unwrap();
            let player = zbus::Proxy::new(
                &connection,
                "org.mpris.MediaPlayer2.tinystream",
                PATH,
                "org.mpris.MediaPlayer2.Player",
            )
            .await
            .unwrap();
            assert_eq!(player.get_property::<String>("PlaybackStatus").await.unwrap(), "Playing");
            assert_eq!(player.get_property::<i64>("Position").await.unwrap(), 15_000_000);
            let metadata: HashMap<String, OwnedValue> = player.get_property("Metadata").await.unwrap();
            assert_eq!(<&str>::try_from(&metadata["xesam:title"]).unwrap(), "Episode");
            player.call::<_, _, ()>("Pause", &()).await.unwrap();
            player.call::<_, _, ()>("Seek", &(-5_000_000i64,)).await.unwrap();
            player.set_property("Rate", 1.5f64).await.unwrap();
            assert!(player.set_property("Rate", f64::NAN).await.is_err());
            let track = ObjectPath::try_from("/dev/tinystream/track/1").unwrap();
            player.call::<_, _, ()>("SetPosition", &(track, 20_000_000i64)).await.unwrap();
            assert!(start(vec!["tinystream://localhost/together/ABCD".into()], send).await.unwrap().is_none());
            let menu = zbus::Proxy::new(&connection, APP, "/Menu", "com.canonical.dbusmenu").await.unwrap();
            let (_, layout): (u32, Layout) =
                menu.call("GetLayout", &(0i32, -1i32, Vec::<String>::new())).await.unwrap();
            assert_eq!(layout.2.len(), 6);
            menu.call::<_, _, ()>("Event", &(5i32, "clicked", Value::from(0i32), 0u32)).await.unwrap();
            let commands = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let mut commands = Vec::new();
                while commands.len() < 6 {
                    let command = rx.recv().await.unwrap();
                    if !matches!(command, Command::Tray(_)) {
                        commands.push(command);
                    }
                }
                commands
            })
            .await
            .unwrap();
            assert!(matches!(commands[0], Command::Pause));
            assert!(matches!(commands[1], Command::Seek(-5.0)));
            assert!(matches!(commands[2], Command::Rate(1.5)));
            assert!(matches!(&commands[3], Command::Position(_, at) if *at == 20.0));
            assert!(matches!(&commands[4], Command::Open(args) if args == &["tinystream://localhost/together/ABCD"]));
            assert!(matches!(commands[5], Command::Show));
            integration
                .notifications
                .send(Notification { title: "Ready".into(), body: "A < B".into(), link: "/title/7".into() })
                .unwrap();
            let notice =
                tokio::time::timeout(std::time::Duration::from_secs(5), arrivals.recv()).await.unwrap().unwrap();
            assert_eq!(notice, ("Ready".into(), "A &lt; B".into()));
            let notifications =
                mock.object_server().interface::<_, Notifications>("/org/freedesktop/Notifications").await.unwrap();
            Notifications::action_invoked(notifications.signal_emitter(), 1, "default").await.unwrap();
            let command = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let command = rx.recv().await.unwrap();
                    if !matches!(command, Command::Tray(_)) {
                        break command;
                    }
                }
            })
            .await
            .unwrap();
            assert!(matches!(command, Command::Notification(link) if link == "/title/7"));
        });
    }
}
