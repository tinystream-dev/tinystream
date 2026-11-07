// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deterministic browsing scenes for the existing headless snapshot renderer.

use serde_json::json;

use super::{Browsing, Document, Entry, Key, now, week_start};
use crate::app::{App, Overlay};
use crate::route::{History, Route};

pub fn prepare(app: &mut App, scene: &str) -> bool {
    let route = match scene {
        "home" => Route::Home,
        "library" => Route::Library("Shows".into()),
        "title" => Route::Title(1),
        "discover" => Route::Discover,
        "calendar" => Route::Calendar,
        "requests" => Route::Requests,
        "wanted" => Route::Wanted,
        "downloads" => Route::Downloads,
        "notifications" | "search" => Route::Home,
        _ => return false,
    };
    app.session.screen = crate::session::Screen::Ready;
    app.history = History::new(route);
    let b = &mut app.browsing;
    let card = json!({"id":1,"kind":"SHOW","library":"Shows","name":"The quiet orbit","year":2026,"poster":"poster","backdrop":"poster","videoCount":12,"watchedCount":3,"progress":0.35,"freshCount":1});
    let video = json!({"id":10,"season":1,"episode":4,"label":"S01E04","name":"A signal from the shore","overview":"A distant transmission brings the crew back to a place they thought they had left behind.","duration":1440,"position":320,"finished":false,"still":"poster","title":card});
    let series = json!({"id":5,"name":"The quiet orbit","library":"Shows","managed":true,"monitor":"MISSING","status":"CONTINUING","numbering":"AUTO","style":{"file":"{show} - {episode:00}","samples":12},"counts":{"have":3,"total":12,"wanted":8,"upcoming":1},"episodes":[],"sources":[],"groups":[],"aliases":[]});
    b.put(Key::Shell,json!({"viewer":{"id":1,"username":"David","isAdmin":true,"permissions":{"request":true,"manageShows":true,"manageRequests":true,"downloads":true,"editMetadata":true}},"libraries":[{"name":"Shows","kind":"VIDEO","showCount":18}],"server":{"downloads":true}}));
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
        json!({"downloadEngine":{"downloadRate":4200000,"uploadRate":128000,"active":1,"killSwitch":null}}),
    );
    b.put(Key::Inbox,json!({"notifications":{"unread":1,"items":[{"id":1,"title":"The quiet orbit is ready","body":"Episode 4 is ready to watch.","image":"poster","kind":"READY","createdAt":now()-120,"link":"/watch/10"}]}}));
    b.put(Key::Discover(String::new(),String::new()),json!({"discover":{"library":"Shows","results":[{"id":"20","provider":"TMDB","category":"SHOWS","library":"Shows","name":"Paper skies","year":2026,"poster":"poster","overview":"Two strangers travel north."}]}}));
    b.put(Key::ForYou(String::new()), json!({"forYou":{"shelves":[]}}));
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
