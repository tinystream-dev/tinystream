// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::Length::Fill;
use iced_core::{Alignment, Color, Padding};
use iced_widget::{Column, Element, Row, Widget, column, container, row, scrollable, space};
use serde_json::json;

use super::model::*;
use super::{Action, Browsing, Dialog, Key, Message, now, week_start};
use crate::app::{App, Message as AppMessage};
use crate::route::Route;
use crate::ui::controls::*;
use crate::ui::icon::Icon;
use crate::ui::squircle::squircle;
use crate::ui::text::{self as ty, MEDIUM, SEMIBOLD, text, truncate};
use crate::ui::{Look, effects, layout, media};

type View<'a> = Element<'a, AppMessage>;
fn msg(b: &Browsing, m: Message) -> AppMessage {
    AppMessage::Browsing(b.generation, m)
}
fn act(b: &Browsing, action: Action) -> AppMessage {
    msg(b, Message::Act(action))
}
struct CardButton<'a> {
    look: Look,
    content: Option<View<'a>>,
    message: Option<AppMessage>,
}
fn card_button<'a>(look: &Look) -> CardButton<'a> {
    CardButton { look: *look, content: None, message: None }
}
impl<'a> CardButton<'a> {
    fn content(
        mut self,
        content: impl iced_core::Widget<AppMessage, iced_core::Theme, iced_widget::Renderer> + 'a,
    ) -> Self {
        self.content = Some(content.boxed());
        self
    }

    fn plain(self) -> Self {
        self
    }

    fn on_press(mut self, message: AppMessage) -> Self {
        self.message = Some(message);
        self
    }

    fn on_press_maybe(mut self, message: Option<AppMessage>) -> Self {
        self.message = message;
        self
    }

    fn boxed(self) -> View<'a> {
        let ink = self.look.ink;
        crate::ui::pressable::pressable(self.content.unwrap_or_else(|| space().boxed()), 14.0, &self.look, move |_| {
            crate::ui::pressable::Appearance { paint: crate::ui::squircle::Paint::NONE, ink }
        })
        .width(Fill)
        .on_press_maybe(self.message)
        .boxed()
    }
}

fn heading<'a>(name: impl Into<String>, look: &Look) -> View<'a> {
    container(text(name.into(), ty::TITLE, look.ink).font(SEMIBOLD))
        .padding(Padding { bottom: 32.0, ..Padding::ZERO })
        .boxed()
}
fn section<'a>(name: impl Into<String>, content: View<'a>, look: &Look) -> View<'a> {
    column![text(name.into(), ty::S15, look.ink).font(SEMIBOLD), content]
        .spacing(14)
        .padding(Padding { bottom: 44.0, ..Padding::ZERO })
        .width(Fill)
        .boxed()
}
fn line<'a>(children: Vec<View<'a>>) -> View<'a> {
    Row::with_children(children).spacing(8).align_y(Alignment::Center).wrap().boxed()
}
fn shelf<'a>(children: Vec<View<'a>>) -> View<'a> {
    scrollable(Row::with_children(children).spacing(20))
        .direction(iced_widget::scrollable::Direction::Horizontal(iced_widget::scrollable::Scrollbar::hidden()))
        .width(Fill)
        .boxed()
}
fn title_link<'a>(name: &'a str, route: Option<Route>, look: &Look) -> View<'a> {
    if let Some(route) = route {
        card_button(look)
            .content(truncate(name, ty::SM, look.ink).font(MEDIUM))
            .on_press(AppMessage::Navigate(route))
            .boxed()
    } else {
        truncate(name, ty::SM, look.ink).font(MEDIUM).boxed()
    }
}
fn raised<'a>(content: View<'a>, look: &Look) -> View<'a> {
    squircle(content, 16.0, look).fill(look.raised).edge().padding(12).width(Fill).boxed()
}
fn state<'a>(value: &str, look: &Look) -> View<'a> {
    let (label, tone) = match value {
        "DONE" | "APPROVED" => (if value == "DONE" { "Ready" } else { "Approved" }, Tone::Ok),
        "GRABBED" | "DOWNLOADING" => ("Downloading", Tone::Live),
        "WANTED" => ("Searching", Tone::Warn),
        "MISSING" => ("Missing", Tone::Danger),
        "DECLINED" | "FAILED" => (if value == "FAILED" { "Failed" } else { "Declined" }, Tone::Danger),
        "PENDING" => ("Pending", Tone::Warn),
        "PAUSED" => ("Paused", Tone::Quiet),
        "SEEDING" => ("Seeding", Tone::Live),
        "SKIPPED" => ("Skipped", Tone::Quiet),
        _ => (value, Tone::Quiet),
    };
    badge(label.to_owned(), tone, look)
}
fn picture<'a>(b: &'a Browsing, path: Option<&str>, width: f32, height: f32, radius: f32, look: &Look) -> View<'a> {
    container(media::img(path.and_then(|p| b.images.picture(p)), radius, look)).width(width).height(height).boxed()
}
fn poster<'a>(b: &'a Browsing, c: &'a Card, caption: Option<String>, look: &Look) -> View<'a> {
    let done = c.video_count > 0 && c.watched_count >= c.video_count;
    let caption = caption.unwrap_or_else(|| {
        if done {
            "Watched".into()
        } else if c.kind == "SHOW" && c.watched_count > 0 {
            format!("{} of {} watched", c.watched_count, c.video_count)
        } else if c.kind == "SHOW" {
            format!("{} episode{}", c.video_count, if c.video_count == 1 { "" } else { "s" })
        } else {
            c.year.map(|y| y.to_string()).unwrap_or("Movie".into())
        }
    });
    let poster = media::poster(
        media::Poster {
            name: &c.name,
            picture: c.poster.as_deref().and_then(|p| b.images.get(p)),
            arrived: c.poster.as_deref().and_then(|p| b.images.picture(p)).and_then(|p| match p {
                crate::images::Picture::Ready(_, at) => *at,
                _ => None,
            }),
            caption,
            progress: c.progress.filter(|_| !done).map(|p| p as f32),
            fresh: (c.fresh_count > 0 && !done).then_some("New"),
        },
        AppMessage::Navigate(Route::Title(c.id as u64)),
        look,
    );
    {
        let mut items = vec![
            ("Open title".into(), AppMessage::Navigate(Route::Title(c.id as u64))),
            (if done { "Mark unwatched" } else { "Mark watched" }.into(), act(b, Action::TitleWatched(c.id, !done))),
        ];
        if b.can().edit_metadata {
            items.push(("Refresh metadata".into(), act(b, Action::Refresh(c.id))));
            items.push(("Fix match".into(), msg(b, Message::Dialog(Some(Dialog::Match(c.id))))));
        }
        crate::ui::context_menu::context_menu(poster, items, look)
    }
}
fn skeleton<'a>(look: &Look) -> View<'a> {
    let l = *look;
    column![
        effects::bone_rounded(180, 36, 8.0, look),
        space().height(32),
        layout::poster_grid(12, move |_| column![
            effects::bone_rounded(152, 228, 14.0, &l),
            effects::bone_rounded(120, 14, 4.0, &l)
        ]
        .spacing(10)
        .boxed())
    ]
    .boxed()
}
fn pending<'a>(b: &'a Browsing, key: &Key, look: &Look) -> View<'a> {
    if let Some(error) = b.cache.get(key).and_then(|e| e.error.as_ref()) {
        column![
            text(error.as_str(), ty::SM, look.danger),
            button("Try again", look).on_press(msg(b, Message::Retry(key.clone()))).boxed()
        ]
        .spacing(12)
        .boxed()
    } else {
        skeleton(look)
    }
}

pub fn view<'a>(app: &'a App, route: &'a Route, look: &Look) -> Option<View<'a>> {
    let b = &app.browsing;
    let content = match route {
        Route::Home => home(b, look),
        Route::Library(name) => library(b, route, name, look),
        Route::Series { library: name, series } => books(b, name, Some(series), look),
        Route::Title(id) => title(b, *id as i64, look),
        Route::Discover => discover(b, look),
        Route::Calendar => calendar(b, look),
        Route::Requests => requests(b, look),
        Route::Wanted => wanted(b, route, look),
        Route::Downloads => downloads(b, route, look),
        _ => return None,
    };
    let mut page = Column::<View<'a>>::new().width(Fill);
    if let Some(error) = &b.error {
        page = page.push((banner(Icon::CircleAlert, look.danger, text(error, ty::SM, look.ink), None, look)).boxed());
    }
    if let Some(notice) = &b.notice {
        page = page.push((text(notice, ty::SM, look.ok)).boxed());
    }
    page = page.push((content).boxed());
    let page = layout::page(page, matches!(route, Route::Calendar));
    if let Route::Title(id) = route
        && let Some(t) = b.data(&Key::Title(*id as i64)).and_then(|d| d.title.as_ref())
        && let Some(path) = &t.card.backdrop
    {
        let canvas = look.canvas;
        let mut layers = vec![
            container(media::img(b.images.picture(path), 0.0, look)).width(Fill).height(544).boxed(),
            container(space().width(Fill).height(544))
                .style(move |_| container::Style {
                    background: Some(
                        iced_core::Gradient::Linear(
                            iced_core::gradient::Linear::new(iced_core::Radians(0.0))
                                .add_stop(0.0, canvas.scale_alpha(0.55))
                                .add_stop(0.2, canvas.scale_alpha(0.55))
                                .add_stop(1.0, canvas),
                        )
                        .into(),
                    ),
                    ..Default::default()
                })
                .boxed(),
        ];
        if let Some(color) = t.card.backdrop_tint.as_deref().or(t.card.poster_tint.as_deref()).and_then(parse_tint) {
            layers.push(effects::ambient(effects::Ambient::title(color)));
        }
        layers.push(page);
        Some(iced_widget::Stack::with_children(layers).width(Fill).boxed())
    } else {
        Some(page)
    }
}

struct Hero<'a> {
    name: &'a str,
    label: &'a str,
    detail: String,
    image: Option<&'a str>,
    tint: Option<&'a str>,
    action: Option<(String, Route)>,
    value: Option<f32>,
    height: f32,
    metric: Option<String>,
}
fn hero<'a>(b: &'a Browsing, props: Hero<'_>, look: &Look) -> View<'a> {
    let Hero { name, label, detail, image, tint, action, value, height, metric } = props;
    let l = *look;
    let mut content = column![
        text(label.to_owned(), ty::XS, look.ink_2).font(MEDIUM),
        text(name.to_owned(), ty::Size { px: 38.0, line: 44.0 }, look.ink).font(SEMIBOLD),
        text(detail, ty::SM, look.ink_2)
    ]
    .spacing(8);
    if let Some(metric) = metric {
        content = content.push(crate::ui::numbers::ticker(&metric, ty::Size { px: 40.0, line: 44.0 }, look).boxed());
    }
    if let Some((label, route)) = action {
        content = content.push(
            (container(
                button(label, look).primary().large().icon(Icon::Play).on_press(AppMessage::Navigate(route)).boxed(),
            )
            .padding(Padding { top: 12.0, ..Padding::ZERO }))
            .boxed(),
        );
    }
    if let Some(value) = value {
        content = content.push((container(progress(Some(value), look.ink, look)).width(224)).boxed());
    }
    let mut layers = vec![media::img(image.and_then(|p| b.images.picture(p)), 22.0, look)];
    layers.push(
        container(space().width(Fill).height(Fill))
            .style(move |_| {
                use iced_core::{Gradient, Radians, gradient};
                container::Style {
                    background: Some(
                        Gradient::Linear(
                            gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_2))
                                .add_stop(0.0, l.canvas)
                                .add_stop(0.65, l.canvas.scale_alpha(0.75))
                                .add_stop(1.0, l.canvas.scale_alpha(0.15)),
                        )
                        .into(),
                    ),
                    ..Default::default()
                }
            })
            .boxed(),
    );
    if let Some(color) = tint.and_then(parse_tint) {
        layers.push(effects::ambient(effects::Ambient::corner(color)));
    }
    layers.push(container(content).align_bottom(Fill).padding(36).boxed());
    squircle(iced_widget::Stack::with_children(layers).height(height).width(Fill), 22.0, look)
        .width(Fill)
        .edge()
        .boxed()
}

struct WideCard<'a> {
    name: &'a str,
    subtitle: String,
    image: Option<&'a str>,
    route: Option<Route>,
    label: Option<String>,
    value: Option<f32>,
    width: f32,
}
fn wide_card<'a>(b: &'a Browsing, props: WideCard<'_>, look: &Look) -> View<'a> {
    let WideCard { name, subtitle, image, route, label, value, width } = props;
    let mut layers =
        vec![crate::ui::tilt::art(image.and_then(|p| b.images.get(p)).cloned(), 16.0, 9.0 / 16.0, look).boxed()];
    let shade = look.media_shade;
    layers.push(
        container(space().width(Fill).height(Fill))
            .style(move |_| container::Style {
                background: Some(
                    iced_core::Gradient::Linear(
                        iced_core::gradient::Linear::new(iced_core::Radians(0.0))
                            .add_stop(0.0, shade.scale_alpha(0.1))
                            .add_stop(1.0, shade.scale_alpha(0.85)),
                    )
                    .into(),
                ),
                ..Default::default()
            })
            .boxed(),
    );
    if let Some(label) = label {
        layers.push(container(badge(label, Tone::Strong, look)).padding(12).boxed());
    }
    layers.push(
        container(
            column![
                truncate(name.to_owned(), ty::SM, look.media_ink).font(MEDIUM),
                truncate(subtitle, ty::XS, look.media_ink.scale_alpha(0.6))
            ]
            .spacing(3),
        )
        .align_bottom(Fill)
        .padding(14)
        .boxed(),
    );
    if let Some(value) = value {
        layers.push(container(progress(Some(value), look.media_ink, look)).align_bottom(Fill).boxed());
    }
    container(
        card_button(look)
            .content(
                squircle(iced_widget::Stack::with_children(layers).width(Fill).height(width * 9.0 / 16.0), 16.0, look)
                    .edge()
                    .width(Fill),
            )
            .on_press_maybe(route.map(AppMessage::Navigate))
            .boxed(),
    )
    .width(width)
    .boxed()
}

fn home<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let Some(h) = b.data(&Key::Home).and_then(|d| d.home.as_ref()) else { return pending(b, &Key::Home, look) };
    let mut page = Column::<View<'a>>::new().width(Fill);
    let featured = h.continue_watching.iter().find(|e| e.new_episode).or_else(|| h.continue_watching.first());
    if let Some(e) = featured {
        let v = &e.video;
        let detail = [v.label.clone(), v.name.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        page = page.push(
            (container(hero(
                b,
                Hero {
                    name: &v.title.name,
                    label: if e.new_episode {
                        "New episode"
                    } else if e.up_next {
                        "Up next"
                    } else {
                        "Continue watching"
                    },
                    detail,
                    image: v.title.backdrop.as_deref().or(v.still.as_deref()),
                    tint: v.title.backdrop_tint.as_deref().or(v.title.poster_tint.as_deref()),
                    action: Some((if e.up_next { "Play" } else { "Resume" }.into(), Route::Watch(v.id as u64, None))),
                    value: v.duration.filter(|d| *d > 0.0).map(|d| (e.position / d) as f32),
                    height: 304.0,
                    metric: Some(if e.up_next {
                        v.label.clone().unwrap_or("Play".into())
                    } else {
                        remaining(e.position, v.duration)
                    }),
                },
                look,
            ))
            .padding(Padding { bottom: 44.0, ..Padding::ZERO }))
            .boxed(),
        );
    } else {
        page = page.push((heading("Home", look)).boxed());
    }
    let rest: Vec<_> = h
        .continue_watching
        .iter()
        .filter(|e| featured.is_none_or(|f| f.video.id != e.video.id))
        .map(|e| {
            let v = &e.video;
            wide_card(
                b,
                WideCard {
                    name: &v.title.name,
                    subtitle: format!(
                        "{} · {}",
                        v.label.as_deref().unwrap_or(""),
                        if e.up_next { "Up next".into() } else { remaining(e.position, v.duration) }
                    ),
                    image: v.still.as_deref().or(v.title.backdrop.as_deref()),
                    route: Some(Route::Watch(v.id as u64, None)),
                    label: if e.new_episode { Some("New episode".into()) } else { None },
                    value: v.duration.filter(|d| *d > 0.0).map(|d| (e.position / d) as f32),
                    width: 320.0,
                },
                look,
            )
        })
        .collect();
    if !rest.is_empty() {
        page = page.push((section("Continue watching", shelf(rest), look)).boxed());
    }
    if let Some(d) = b.data(&Key::Coming) {
        let entries: Vec<_> = d
            .calendar
            .iter()
            .filter(|e| e.state != "DONE")
            .take(12)
            .map(|e| {
                wide_card(
                    b,
                    WideCard {
                        name: &e.show,
                        subtitle: format!("{} · {}", code(e.season, e.episode), e.name.as_deref().unwrap_or("")),
                        image: e.backdrop.as_deref().or(e.poster.as_deref()),
                        route: e.title.as_ref().map(|t| Route::Title(t.id as u64)),
                        label: Some(
                            e.download
                                .as_ref()
                                .map(|d| format!("Downloading {}%", (d.progress * 100.0) as i64))
                                .unwrap_or_else(|| countdown(e.air_at)),
                        ),
                        value: e.download.as_ref().map(|d| d.progress as f32),
                        width: 288.0,
                    },
                    look,
                )
            })
            .collect();
        if !entries.is_empty() {
            page = page.push((section("Coming up", shelf(entries), look)).boxed());
        }
    }
    if !h.popular_here.is_empty() {
        page = page.push(
            (section(
                "Popular with people here",
                shelf(
                    h.popular_here
                        .iter()
                        .map(|p| {
                            container(poster(
                                b,
                                &p.title,
                                Some(if p.people > 1 {
                                    format!("{} people watching", p.people)
                                } else {
                                    "Someone’s watching".into()
                                }),
                                look,
                            ))
                            .width(152)
                            .boxed()
                        })
                        .collect(),
                ),
                look,
            ))
            .boxed(),
        );
    }
    for s in &h.recently_added {
        if !s.titles.is_empty() {
            let header = row![
                text(format!("New in {}", s.library), ty::S15, look.ink).font(SEMIBOLD).width(Fill),
                button("See all", look)
                    .plain()
                    .small()
                    .on_press(AppMessage::Navigate(Route::Library(s.library.clone())))
                    .boxed()
            ]
            .align_y(Alignment::Center);
            page = page.push(
                (column![
                    header,
                    shelf(s.titles.iter().map(|c| container(poster(b, c, None, look)).width(152).boxed()).collect())
                ]
                .spacing(14)
                .padding(Padding { bottom: 44.0, ..Padding::ZERO }))
                .boxed(),
            );
        }
    }
    if let Some(h) = b.data(&Key::Music).and_then(|d| d.music_home.as_ref()) {
        for (name, albums) in [("Jump back in", &h.recently_played), ("New music", &h.recently_added)] {
            if !albums.is_empty() {
                page = page.push(
                    (section(
                        name,
                        shelf(
                            albums
                                .iter()
                                .map(|a| {
                                    container(
                                        column![
                                            card_button(look)
                                                .content(picture(b, a.cover.as_deref(), 136.0, 136.0, 12.0, look))
                                                .on_press(AppMessage::Navigate(Route::Album(a.id as u64)))
                                                .boxed(),
                                            truncate(&a.name, ty::S13, look.ink),
                                            truncate(&a.artist, ty::XS, look.ink_3)
                                        ]
                                        .spacing(6),
                                    )
                                    .width(136)
                                    .boxed()
                                })
                                .collect(),
                        ),
                        look,
                    ))
                    .boxed(),
                );
            }
        }
    }
    if h.recently_added.iter().all(|r| r.titles.is_empty()) && featured.is_none() {
        page = page.push(
            (layout::empty(
                if b.libraries().is_empty() { "No libraries yet" } else { "Your libraries are empty so far" },
                if b.viewer().is_some_and(|v| v.is_admin) { None } else { Some("Ask an admin for access.") },
                look,
            ))
            .boxed(),
        );
        if b.viewer().is_some_and(|v| v.is_admin) {
            page = page.push(
                (button("Add a library", look).primary().on_press(AppMessage::Navigate(Route::Settings)).boxed())
                    .boxed(),
            );
        }
    }
    page.boxed()
}

fn library<'a>(b: &'a Browsing, route: &Route, name: &'a str, look: &Look) -> View<'a> {
    if b.libraries().iter().any(|l| l.name == name && l.kind == "BOOKS") {
        return books(b, name, None, look);
    }
    let key = Key::Library(name.into());
    let Some(d) = b.data(&key) else { return pending(b, &key, look) };
    let cards = d.library.as_ref().map_or(&[][..], |l| l.titles.as_slice());
    let filter = b.filter.get(route).map_or("all", String::as_str);
    let mut list: Vec<_> = cards
        .iter()
        .filter(|c| {
            let done = c.video_count > 0 && c.watched_count >= c.video_count;
            let started = c.watched_count > 0 || c.progress.is_some();
            match filter {
                "watching" => started && !done,
                "unwatched" => !started,
                "watched" => done,
                _ => true,
            }
        })
        .collect();
    let newest = b.newest.contains(route);
    if newest {
        list.sort_by_key(|c| std::cmp::Reverse(c.year.unwrap_or(0)));
    } else {
        list.sort_by_key(|c| c.name.to_lowercase());
    }
    let count = list.len();
    let filters = line(
        [("all", "All"), ("watching", "Watching"), ("unwatched", "Not started"), ("watched", "Watched")]
            .into_iter()
            .map(|(v, l)| chip(l, filter == v, msg(b, Message::Filter(v.into())), look))
            .chain([
                chip("A–Z", !newest, msg(b, Message::Sort(false)), look),
                chip("Newest", newest, msg(b, Message::Sort(true)), look),
            ])
            .collect(),
    );
    column![
        row![heading(name.to_owned(), look), text(format!("{} titles", cards.len()), ty::SM, look.ink_3)],
        container(filters).padding(Padding { bottom: 28.0, ..Padding::ZERO }),
        if count == 0 {
            layout::empty(
                if cards.is_empty() { "This library is empty" } else { "Nothing matches this filter" },
                None,
                look,
            )
        } else {
            let l = *look;
            layout::poster_grid(count, move |i| poster(b, list[i], None, &l))
        }
    ]
    .boxed()
}

fn books<'a>(b: &'a Browsing, name: &str, series: Option<&str>, look: &Look) -> View<'a> {
    let key = Key::Books(name.into());
    let Some(d) = b.data(&key) else { return pending(b, &key, look) };
    let mut books: Vec<_> = d
        .books
        .iter()
        .filter(|book| series.is_none_or(|s| book.series.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(s))))
        .collect();
    books.sort_by(|a, b| a.series_index.unwrap_or(0.0).total_cmp(&b.series_index.unwrap_or(0.0)));
    let mut page = column![heading(series.unwrap_or(name).to_owned(), look)].width(Fill);
    if series.is_some()
        && let Some(current) =
            books.iter().find(|b| !b.reading.as_ref().is_some_and(|r| r.finished)).or_else(|| books.first())
    {
        page = page.push(
            (row![
                picture(b, current.cover.as_deref(), 176.0, 264.0, 14.0, look),
                column![
                    text(current.authors.join(", "), ty::SM, look.ink_2),
                    text(format!("{} books", books.len()), ty::SM, look.ink_3),
                    button("Read", look)
                        .primary()
                        .on_press(AppMessage::Navigate(Route::Read(current.id as u64)))
                        .boxed()
                ]
                .spacing(16)
            ]
            .spacing(32))
            .boxed(),
        );
    }
    let l = *look;
    page.push(
        (layout::poster_grid(books.len(), move |i| {
            let book = books[i];
            column![
                button("", &l)
                    .content(picture(b, book.cover.as_deref(), 152.0, 228.0, 14.0, &l))
                    .on_press(AppMessage::Navigate(Route::Book(book.id as u64)))
                    .boxed(),
                truncate(&book.title, ty::S13, l.ink),
                text(book.authors.join(", "), ty::XS, l.ink_3)
            ]
            .spacing(6)
            .boxed()
        }))
        .boxed(),
    )
    .boxed()
}

fn title<'a>(b: &'a Browsing, id: i64, look: &Look) -> View<'a> {
    let key = Key::Title(id);
    let Some(d) = b.data(&key) else { return pending(b, &key, look) };
    let Some(t) = &d.title else { return layout::empty("This title isn't here anymore", None, look) };
    let c = &t.card;
    let can = b.can();
    let mut page = Column::<View<'a>>::new().width(Fill);
    let mut actions = Vec::new();
    if let Some(n) = &t.next_up {
        actions.push(
            button(if n.resuming { "Resume" } else { "Play" }, look)
                .primary()
                .large()
                .icon(Icon::Play)
                .on_press(AppMessage::Navigate(Route::Watch(n.video.id as u64, None)))
                .boxed(),
        );
    } else if let Some(v) = &t.movie {
        actions.push(
            button("Play", look)
                .primary()
                .large()
                .icon(Icon::Play)
                .on_press(AppMessage::Navigate(Route::Watch(v.id as u64, None)))
                .boxed(),
        );
    }
    let done = c.video_count > 0 && c.watched_count >= c.video_count;
    actions.push(
        button(if done { "Mark unwatched" } else { "Mark watched" }, look)
            .on_press_maybe((!b.busy).then(|| act(b, Action::TitleWatched(id, !done))))
            .boxed(),
    );
    if can.edit_metadata {
        actions.push(
            button("Refresh metadata", look)
                .plain()
                .icon(Icon::RotateCcw)
                .on_press(act(b, Action::Refresh(id)))
                .boxed(),
        );
        actions
            .push(button("Fix match", look).plain().on_press(msg(b, Message::Dialog(Some(Dialog::Match(id))))).boxed());
    }
    let details = column![
        text(&c.name, ty::Size { px: 44.0, line: 49.0 }, look.ink).font(SEMIBOLD),
        text(
            [
                c.year.map(|y| y.to_string()),
                Some(if c.kind == "SHOW" {
                    format!("{} season{}", t.seasons.len(), if t.seasons.len() == 1 { "" } else { "s" })
                } else {
                    t.movie.as_ref().and_then(|v| v.duration).map(runtime).unwrap_or_default()
                }),
                t.rating.map(|r| format!("{r:.1} rating"))
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
            ty::SM,
            look.ink_2
        ),
        text(t.genres.join(" · "), ty::S13, look.ink_3),
        line(actions)
    ]
    .spacing(14)
    .width(Fill);
    let header = row![picture(b, c.poster.as_deref(), 208.0, 312.0, 14.0, look), container(details).align_bottom(312)]
        .spacing(32);
    page = page.push(container(header).padding(Padding { top: 40.0, ..Padding::ZERO }).boxed());
    if can.edit_metadata {
        page = page.push(
            (container(line(vec![
                artwork(b, id, Some("POSTER"), "poster", t.custom_poster, look),
                artwork(b, id, Some("BACKDROP"), "backdrop", t.custom_backdrop, look),
            ]))
            .padding([16, 0]))
            .boxed(),
        );
    }
    if let Some(overview) = &t.overview {
        page = page.push(
            (container(text(overview, ty::SM, look.ink_2).line_height(iced_core::text::LineHeight::Relative(1.7)))
                .width(Fill.max(768))
                .padding([24, 0]))
            .boxed(),
        );
    }
    if let Some(series) = b.data(&Key::Series(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.series.as_ref()) {
        if can.manage_shows {
            page = page.push((series_panel(b, series, look)).boxed());
        }
    } else if b.features() && can.manage_shows && c.kind == "SHOW" {
        page = page.push((button("Manage downloads", look).on_press(act(b, Action::Manage(id))).boxed()).boxed());
    }
    if !t.seasons.is_empty() {
        let season = b.seasons.get(&id).copied().unwrap_or_else(|| initial_season(t));
        page = page.push(
            (container(line(
                t.seasons
                    .iter()
                    .map(|s| chip(&s.name, s.number == season, msg(b, Message::Season(s.number)), look))
                    .collect(),
            ))
            .padding([20, 0]))
            .boxed(),
        );
        if let Some(s) = t.seasons.iter().find(|s| s.number == season) {
            let ids: Vec<_> = s.episodes.iter().map(|e| e.id).collect();
            let all = s.episodes.iter().all(|e| e.finished);
            page = page.push(
                (row![
                    text(s.title.as_deref().unwrap_or(&s.name), ty::S15, look.ink).font(SEMIBOLD).width(Fill),
                    button(if all { "Mark season unwatched" } else { "Mark season watched" }, look)
                        .plain()
                        .small()
                        .on_press(act(b, Action::Watched(ids, !all)))
                        .boxed()
                ]
                .align_y(Alignment::Center))
                .boxed(),
            );
            if let Some(o) = &s.overview {
                page = page.push((text(o, ty::SM, look.ink_2)).boxed());
            }
            for v in &s.episodes {
                page = page.push((episode(b, v, look)).boxed());
            }
        }
        if let Some(series) = b.data(&Key::Series(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.series.as_ref()) {
            for e in series
                .episodes
                .iter()
                .filter(|e| e.season == season && e.video.is_none() && (e.aired || e.air_at.is_some()))
            {
                page = page.push(
                    (raised(
                        row![
                            text(code(e.season, e.episode), ty::XS, look.ink_3),
                            text(e.name.as_deref().unwrap_or("Upcoming episode"), ty::SM, look.ink_2).width(Fill),
                            state(&e.state, look),
                            button("Search", look)
                                .small()
                                .on_press(msg(
                                    b,
                                    Message::Dialog(Some(Dialog::Releases {
                                        series: series.id,
                                        season: e.season,
                                        episodes: vec![e.episode],
                                        name: c.name.clone()
                                    }))
                                ))
                                .boxed()
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center)
                        .boxed(),
                        look,
                    ))
                    .boxed(),
                );
            }
        }
    }
    if let Some(s) = b.data(&Key::Similar(id)).and_then(|d| d.title.as_ref()).and_then(|t| t.similar.as_ref()) {
        if !s.also_watched.is_empty() {
            page = page.push(
                (section(
                    "People also watched",
                    shelf(
                        s.also_watched.iter().map(|c| container(poster(b, c, None, look)).width(152).boxed()).collect(),
                    ),
                    look,
                ))
                .boxed(),
            );
        }
        if !s.recommendations.is_empty() {
            page = page.push(
                (section(
                    "More like this",
                    shelf(
                        s.recommendations
                            .iter()
                            .map(|r| container(discover_card(b, r, look)).width(152).boxed())
                            .collect(),
                    ),
                    look,
                ))
                .boxed(),
            );
        }
    }
    page.spacing(8).boxed()
}
fn initial_season(t: &Title) -> i64 {
    let regular: Vec<_> = t.seasons.iter().filter(|s| s.number > 0).collect();
    if t.seasons.iter().all(|s| s.episodes.iter().all(|e| e.finished)) {
        return regular.last().copied().or_else(|| t.seasons.last()).map_or(0, |s| s.number);
    }
    t.next_up
        .as_ref()
        .and_then(|n| t.seasons.iter().find(|s| s.episodes.iter().any(|e| e.id == n.video.id)))
        .or_else(|| regular.first().copied())
        .or_else(|| t.seasons.first())
        .map_or(0, |s| s.number)
}
fn artwork<'a>(b: &'a Browsing, id: i64, kind: Option<&str>, label: &str, custom: bool, look: &Look) -> View<'a> {
    let mut buttons = vec![
        button(format!("Change {label}"), look)
            .small()
            .on_press_maybe(
                (!b.busy).then(|| act(b, Action::Artwork { id, kind: kind.map(str::to_owned), reset: false })),
            )
            .boxed(),
    ];
    if custom {
        buttons.push(
            button(format!("Reset {label}"), look)
                .plain()
                .small()
                .on_press_maybe(
                    (!b.busy).then(|| act(b, Action::Artwork { id, kind: kind.map(str::to_owned), reset: true })),
                )
                .boxed(),
        );
    }
    line(buttons)
}
fn episode<'a>(b: &'a Browsing, v: &'a Video, look: &Look) -> View<'a> {
    let mut content = column![
        text(v.label.as_deref().unwrap_or("Video"), ty::XS, look.ink_3),
        text(v.name.as_deref().unwrap_or(""), ty::SM, look.ink).font(MEDIUM)
    ]
    .spacing(4)
    .width(Fill);
    if let Some(o) = &v.overview {
        content = content.push((text(o, ty::S13, look.ink_2)).boxed());
    }
    let mut actions = vec![
        button(if v.finished { "Watched" } else { "Mark watched" }, look)
            .plain()
            .small()
            .on_press(act(b, Action::Watched(vec![v.id], !v.finished)))
            .boxed(),
    ];
    if b.can().edit_metadata {
        actions.push(artwork(b, v.id, None, "still", v.custom_still, look));
    }
    raised(
        row![
            card_button(look)
                .content(picture(b, v.still.as_deref(), 176.0, 99.0, 10.0, look))
                .on_press(AppMessage::Navigate(Route::Watch(v.id as u64, None)))
                .boxed(),
            content,
            column![text(v.duration.map(runtime).unwrap_or_default(), ty::XS, look.ink_3), line(actions)].spacing(8)
        ]
        .spacing(16)
        .align_y(Alignment::Center)
        .boxed(),
        look,
    )
}
fn series_panel<'a>(b: &'a Browsing, s: &'a Series, look: &Look) -> View<'a> {
    let monitors = line(
        [("NONE", "Off"), ("FUTURE", "New episodes"), ("MISSING", "Everything")]
            .into_iter()
            .map(|(v, l)| chip(l, s.monitor == v, act(b, Action::UpdateSeries(s.id, json!({"monitor":v}))), look))
            .collect(),
    );
    raised(
        column![
            row![text("Downloads", ty::S15, look.ink).font(SEMIBOLD).width(Fill), state(&s.status, look)],
            monitors,
            line(vec![
                button("Settings", look)
                    .small()
                    .on_press(msg(b, Message::Dialog(Some(Dialog::SeriesSettings(std::sync::Arc::new(s.clone()))))))
                    .boxed(),
                button("Look for skipped episodes again", look)
                    .plain()
                    .small()
                    .on_press(act(b, Action::LookAgain(s.id, None, None)))
                    .boxed(),
                button("Refresh schedule", look)
                    .plain()
                    .small()
                    .on_press(act(b, Action::RefreshSchedule(s.id)))
                    .boxed(),
                button("Search", look)
                    .small()
                    .on_press(msg(
                        b,
                        Message::Dialog(Some(Dialog::Releases {
                            series: s.id,
                            season: 1,
                            episodes: vec![],
                            name: s.name.clone()
                        }))
                    ))
                    .boxed(),
                button("Remove from managed shows", look)
                    .danger()
                    .small()
                    .on_press(confirm(
                        b,
                        "Remove show?",
                        "Existing files will stay in your library.",
                        Action::RemoveSeries(s.id)
                    ))
                    .boxed()
            ])
        ]
        .spacing(12)
        .boxed(),
        look,
    )
}

fn discover<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let can = b.can();
    let name = if can.manage_shows { "Add shows" } else { "Request shows" };
    if !can.manage_shows && !can.request {
        return column![heading(name, look), layout::empty("You don't have permission to request shows", None, look)]
            .boxed();
    }
    let mut page = column![
        heading(name, look),
        input("discover-query", &b.discover_input, "Search shows and movies", move |q| msg(b, Message::Query(q)), look)
            .boxed()
    ];
    page = page.push(
        (line(
            std::iter::once(chip("All libraries", b.library.is_empty(), msg(b, Message::Library(String::new())), look))
                .chain(
                    b.libraries()
                        .iter()
                        .filter(|l| l.kind == "VIDEO")
                        .map(|l| chip(&l.name, b.library == l.name, msg(b, Message::Library(l.name.clone())), look)),
                )
                .collect(),
        ))
        .boxed(),
    );
    page = page.push(
        (line(
            [
                ("", "All types"),
                ("EPISODES", "Episodes / Seasons"),
                ("SPECIALS", "Specials"),
                ("MOVIES", "Movies"),
                ("OTHER", "Other / Mixed"),
            ]
            .into_iter()
            .map(|(v, l)| chip(l, b.category == v, msg(b, Message::Category(v.into())), look))
            .collect(),
        ))
        .boxed(),
    );
    if b.query.is_empty()
        && let Some(f) = b.data(&Key::ForYou(b.library.clone())).and_then(|d| d.for_you.as_ref())
    {
        for s in &f.shelves {
            if !s.results.is_empty() {
                page = page.push(
                    (section(
                        &s.name,
                        shelf(
                            s.results
                                .iter()
                                .filter(|r| b.category.is_empty() || r.category == b.category)
                                .map(|r| container(discover_card(b, r, look)).width(152).boxed())
                                .collect(),
                        ),
                        look,
                    ))
                    .boxed(),
                );
            }
        }
    }
    let key = Key::Discover(b.library.clone(), b.query.clone());
    if let Some(d) =
        b.data(&key).or_else(|| b.previous_discover.as_ref().and_then(|k| b.data(k))).and_then(|d| d.discover.as_ref())
    {
        let results: Vec<_> = d.results.iter().filter(|r| b.category.is_empty() || r.category == b.category).collect();
        if results.is_empty() {
            page = page.push((layout::empty("Nothing found", None, look)).boxed());
        } else {
            let l = *look;
            page = page.push((layout::poster_grid(results.len(), move |i| discover_card(b, results[i], &l))).boxed());
        }
    } else if b.query.chars().count() != 1 {
        page = page.push((pending(b, &key, look)).boxed());
    }
    page.spacing(20).boxed()
}
fn discover_card<'a>(b: &'a Browsing, r: &'a Discover, look: &Look) -> View<'a> {
    let status = if r.title_id.is_some() {
        Some("In your library")
    } else if r.series_id.is_some() && r.monitor.as_deref().is_some_and(|m| m != "NONE") {
        Some("Downloading")
    } else if r.request_state.as_deref() == Some("PENDING") {
        Some("Requested")
    } else {
        None
    };
    let action = if let Some(id) = r.title_id {
        Some(AppMessage::Navigate(Route::Title(id as u64)))
    } else if status.is_none() && r.category != "MOVIES" && (b.can().manage_shows || b.can().request) {
        Some(msg(b, Message::Dialog(Some(Dialog::Pick(std::sync::Arc::new(r.clone()))))))
    } else {
        None
    };
    let art = crate::ui::tilt::art(r.poster.as_deref().and_then(|p| b.images.get(p)).cloned(), 14.0, 1.5, look);
    let mut layers = vec![art.boxed()];
    if let Some(status) = status {
        layers.push(container(badge(status, Tone::Strong, look)).align_right(Fill).padding(8).boxed());
    }
    let card = card_button(look)
        .content(
            column![
                iced_widget::Stack::with_children(layers),
                truncate(&r.name, ty::S13, look.ink).font(MEDIUM),
                text(
                    format!(
                        "{} · {}",
                        r.category,
                        r.because
                            .as_ref()
                            .map(|s| format!("Like {s}"))
                            .unwrap_or_else(|| r.year.map(|y| y.to_string()).unwrap_or("Upcoming".into()))
                    ),
                    ty::XS,
                    look.ink_3
                )
            ]
            .spacing(6),
        )
        .plain()
        .on_press_maybe(action)
        .boxed();
    iced_widget::tooltip(
        card,
        panel(
            column![
                text(&r.name, ty::S15, look.ink).font(SEMIBOLD),
                text(r.overview.as_deref().unwrap_or(""), ty::S13, look.ink_2)
            ]
            .spacing(8),
            14.0,
            look,
        )
        .padding(16)
        .width(320),
        iced_widget::tooltip::Position::FollowCursor,
    )
    .boxed()
}

fn calendar<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let start = week_start(now()) + b.week * 7 * 86400;
    let key = Key::Calendar(start);
    let controls = line(vec![
        text(format!("{} – {}", date(start), date(start + 6 * 86400)), ty::SM, look.ink_3).boxed(),
        icon_button(Icon::ChevronLeft, "Previous week", Some(msg(b, Message::Week(b.week - 1))), look),
        button("This week", look).small().on_press(msg(b, Message::Week(0))).boxed(),
        icon_button(Icon::ChevronRight, "Next week", Some(msg(b, Message::Week(b.week + 1))), look),
    ]);
    let mut page =
        column![row![heading("Calendar", look), space().width(Fill), controls].align_y(Alignment::Center)].width(Fill);
    let Some(d) = b.data(&key) else { return page.push((pending(b, &key, look)).boxed()).boxed() };
    if b.week == 0
        && let Some(next) = b.data(&Key::Soon).and_then(|d| d.calendar.iter().find(|e| e.air_at > now() - 12 * 3600))
    {
        let route = entry_route(next);
        let label = if next.air_at > now() {
            "Up next"
        } else if next.video.is_some() {
            "Ready to watch"
        } else if next.download.is_some() {
            "Downloading"
        } else {
            "Just aired"
        };
        let detail = format!(
            "{} · {} · {}",
            code(next.season, next.episode),
            next.name.as_deref().unwrap_or(""),
            countdown(next.air_at)
        );
        page = page.push(
            (container(hero(
                b,
                Hero {
                    name: &next.show,
                    label,
                    detail,
                    image: next.backdrop.as_deref().or(next.poster.as_deref()),
                    tint: next.backdrop_tint.as_deref(),
                    action: route.map(|r| ("Play".into(), r)),
                    value: next.download.as_ref().map(|d| d.progress as f32),
                    height: 240.0,
                    metric: Some(if next.air_at > now() {
                        countdown(next.air_at)
                    } else if let Some(d) = &next.download {
                        format!("{:.1}%", d.progress * 100.0)
                    } else if next.video.is_some() {
                        "Ready to watch".into()
                    } else {
                        "Searching…".into()
                    }),
                },
                look,
            ))
            .padding(Padding { bottom: 32.0, ..Padding::ZERO }))
            .boxed(),
        );
    }
    if d.calendar.is_empty() {
        return page.push((layout::empty("Nothing on the schedule", None, look)).boxed()).boxed();
    }
    let mut days = Row::<View<'a>>::new().spacing(12).width(Fill);
    for (i, label) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].iter().enumerate() {
        let day = start + i as i64 * 86400;
        let mut entries = Column::<View<'a>>::new().spacing(6).width(Fill);
        let count = d.calendar.iter().filter(|e| e.air_at >= day && e.air_at < day + 86400).count();
        for e in d.calendar.iter().filter(|e| e.air_at >= day && e.air_at < day + 86400) {
            entries = entries.push((calendar_card(b, e, look)).boxed());
        }
        days = days.push(
            (column![
                text(format!("{label} {}", date(day)), ty::XS, look.ink_3),
                squircle(entries, 16.0, look)
                    .fill(look.raised)
                    .padding(6)
                    .height(416.0f32.max(count as f32 * 160.0))
                    .width(Fill)
            ]
            .spacing(8)
            .width(Fill))
            .boxed(),
        );
    }
    page.push((days).boxed()).boxed()
}
fn entry_route(e: &CalendarEntry) -> Option<Route> {
    e.video
        .as_ref()
        .map(|v| Route::Watch(v.id as u64, None))
        .or_else(|| e.title.as_ref().map(|t| Route::Title(t.id as u64)))
}
fn calendar_card<'a>(b: &'a Browsing, e: &'a CalendarEntry, look: &Look) -> View<'a> {
    let mut content = column![
        container(media::img(
            e.backdrop.as_deref().or(e.poster.as_deref()).and_then(|p| b.images.picture(p)),
            8.0,
            look
        ))
        .height(72)
        .width(Fill),
        text(&e.show, ty::S13, look.ink).font(MEDIUM),
        text(format!("{} · {}", code(e.season, e.episode), e.name.as_deref().unwrap_or("")), ty::XS, look.ink_3)
    ]
    .spacing(6);
    if let Some(d) = &e.download {
        content = content
            .push((progress(Some(d.progress as f32), look.info, look)).boxed())
            .push((text(format!("{}%", (d.progress * 100.0) as u32), ty::XS, look.info)).boxed());
    } else {
        content = content.push((state(&e.state, look)).boxed());
    }
    card_button(look).plain().content(content).on_press_maybe(entry_route(e).map(AppMessage::Navigate)).boxed()
}
fn requests<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let mut page = column![row![
        heading("Requests", look),
        space().width(Fill),
        button(if b.can().manage_shows { "Add a show" } else { "Request a show" }, look)
            .primary()
            .icon(Icon::Plus)
            .on_press(AppMessage::Navigate(Route::Discover))
            .boxed()
    ]]
    .width(Fill);
    let Some(d) = b.data(&Key::Requests) else { return page.push((pending(b, &Key::Requests, look)).boxed()).boxed() };
    if d.requests.is_empty() {
        return page.push((layout::empty("No requests yet", None, look)).boxed()).boxed();
    }
    for pending_group in [true, false] {
        let mut list = Column::<View<'a>>::new().spacing(8);
        for r in d.requests.iter().filter(|r| (r.state == "PENDING") == pending_group) {
            let mut details = column![
                row![
                    title_link(&r.name, r.title.as_ref().map(|t| Route::Title(t.id as u64)), look),
                    text(r.year.map(|y| y.to_string()).unwrap_or_default(), ty::XS, look.ink_3),
                    state(&r.state, look)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                text(
                    format!(
                        "{} asked {} · for {}",
                        r.user.as_ref().map_or("Someone", |u| u.username.as_str()),
                        relative(r.created_at),
                        r.library.as_deref().unwrap_or("")
                    ),
                    ty::XS,
                    look.ink_3
                )
            ]
            .spacing(4)
            .width(Fill);
            if let Some(note) = &r.note {
                details = details.push((text(format!("“{note}”"), ty::S13, look.ink_2)).boxed());
            }
            if r.state == "APPROVED"
                && let Some(aired) = r.aired.filter(|n| *n > 0)
            {
                details = details
                    .push((progress(Some(r.have.unwrap_or(0) as f32 / aired as f32), look.ok, look)).boxed())
                    .push((text(format!("{} of {aired} episodes", r.have.unwrap_or(0)), ty::XS, look.ink_3)).boxed());
            }
            let mut actions = Vec::new();
            if r.state == "PENDING" {
                if b.can().manage_requests {
                    actions.push(
                        button("Decline", look)
                            .plain()
                            .small()
                            .on_press_maybe((!b.busy).then(|| act(b, Action::Decline(r.id))))
                            .boxed(),
                    );
                    actions.push(
                        button("Approve", look)
                            .primary()
                            .small()
                            .on_press_maybe((!b.busy).then(|| act(b, Action::Approve(r.id))))
                            .boxed(),
                    );
                } else if r.user.as_ref().is_some_and(|u| b.viewer().is_some_and(|v| v.id == u.id)) {
                    actions.push(
                        button("Cancel", look).plain().small().on_press(act(b, Action::CancelRequest(r.id))).boxed(),
                    );
                }
            }
            list = list.push(
                (raised(
                    row![picture(b, r.poster.as_deref(), 56.0, 84.0, 10.0, look), details, line(actions)]
                        .spacing(16)
                        .align_y(Alignment::Start)
                        .boxed(),
                    look,
                ))
                .boxed(),
            );
        }
        if d.requests.iter().any(|r| (r.state == "PENDING") == pending_group) {
            page = page.push(
                (section(
                    if pending_group {
                        if b.can().manage_requests { "Waiting for you" } else { "Waiting for approval" }
                    } else {
                        "Earlier"
                    },
                    list.boxed(),
                    look,
                ))
                .boxed(),
            );
        }
    }
    page.boxed()
}
fn wanted<'a>(b: &'a Browsing, route: &Route, look: &Look) -> View<'a> {
    let mut page = column![heading("Wanted", look)].width(Fill);
    let Some(d) = b.data(&Key::Wanted) else { return page.push((pending(b, &Key::Wanted, look)).boxed()).boxed() };
    let filter = b.filter.get(route).map_or("all", String::as_str);
    let filter_of =
        |e: &Wanted| if e.state == "WANTED" && !e.aired { "upcoming".to_owned() } else { e.state.to_lowercase() };
    page = page.push(
        (line(
            [
                ("all", "All"),
                ("wanted", "Searching"),
                ("grabbed", "Downloading"),
                ("upcoming", "Upcoming"),
                ("missing", "Missing"),
            ]
            .into_iter()
            .map(|(v, l)| chip(l, filter == v, msg(b, Message::Filter(v.into())), look))
            .collect(),
        ))
        .boxed(),
    );
    let mut groups: std::collections::BTreeMap<(i64, i64), Vec<&Wanted>> = Default::default();
    for e in &d.wanted {
        if filter == "all" || filter_of(e) == filter {
            groups.entry((e.series_id, e.season)).or_default().push(e);
        }
    }
    if groups.is_empty() {
        return page
            .push(
                (layout::empty(if filter == "all" { "Nothing is missing" } else { "Nothing here" }, None, look))
                    .boxed(),
            )
            .boxed();
    }
    for ((series, season), mut episodes) in groups {
        episodes.sort_by_key(|e| e.episode);
        let first = episodes[0];
        let mut header = row![
            button(&first.show, look)
                .plain()
                .on_press_maybe(first.title.as_ref().map(|t| AppMessage::Navigate(Route::Title(t.id as u64))))
                .boxed(),
            text(format!("Season {season}"), ty::SM, look.ink_3),
            space().width(Fill),
            button("Search", look)
                .small()
                .on_press(msg(
                    b,
                    Message::Dialog(Some(Dialog::Releases {
                        series,
                        season,
                        episodes: if episodes.len() == 1 { vec![first.episode] } else { vec![] },
                        name: first.show.clone()
                    }))
                ))
                .boxed()
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if b.can().downloads {
            header = header.push(
                (button("Delete downloaded files", look)
                    .danger()
                    .small()
                    .on_press(confirm(
                        b,
                        "Delete downloaded files?",
                        "This permanently deletes the season's downloaded files.",
                        Action::DeleteDownloaded(series, Some(season)),
                    ))
                    .boxed())
                .boxed(),
            );
        }
        let mut content = Column::<View<'a>>::new().push((header).boxed()).spacing(12);
        for e in episodes {
            content = content.push(
                (row![
                    text(code(e.season, e.episode), ty::XS, look.ink_3).width(70),
                    text(e.name.clone().unwrap_or_else(|| format!("Episode {}", e.episode)), ty::SM, look.ink)
                        .width(Fill),
                    text(e.air_at.map(date).unwrap_or_default(), ty::XS, look.ink_3),
                    text(
                        e.next_search
                            .map(|t| format!("next look {}", countdown(t)))
                            .or_else(|| e.searched_at.map(|t| format!("looked {}", relative(t))))
                            .unwrap_or_default(),
                        ty::XS,
                        look.ink_3
                    ),
                    if filter_of(e) == "upcoming" {
                        badge("Not aired", Tone::Quiet, look)
                    } else {
                        state(&e.state, look)
                    }
                ]
                .spacing(12)
                .align_y(Alignment::Center))
                .boxed(),
            );
        }
        page = page.push((raised(content.boxed(), look)).boxed());
    }
    page.spacing(16).boxed()
}
fn downloads<'a>(b: &'a Browsing, route: &Route, look: &Look) -> View<'a> {
    if !b.can().downloads {
        return column![
            heading("Downloads", look),
            layout::empty("You don't have permission to manage downloads", None, look)
        ]
        .boxed();
    }
    let Some(d) = b.data(&Key::Downloads) else { return pending(b, &Key::Downloads, look) };
    let running = d
        .downloads
        .iter()
        .filter(|d| matches!(d.state.as_str(), "DOWNLOADING" | "SEEDING"))
        .map(|d| d.id)
        .collect::<Vec<_>>();
    let paused = d.downloads.iter().filter(|d| d.state == "PAUSED").map(|d| d.id).collect::<Vec<_>>();
    let mut page = column![row![
        heading("Downloads", look),
        space().width(Fill),
        button("Pause all", look)
            .plain()
            .small()
            .on_press_maybe((!running.is_empty() && !b.busy).then(|| act(b, Action::Pause(running))))
            .boxed(),
        button("Resume all", look)
            .plain()
            .small()
            .on_press_maybe((!paused.is_empty() && !b.busy).then(|| act(b, Action::Resume(paused))))
            .boxed()
    ]]
    .width(Fill);
    if let Some(e) = b.data(&Key::Engine).and_then(|d| d.download_engine.as_ref()) {
        if let Some(interface) = &e.kill_switch {
            page = page.push(
                (banner(
                    Icon::CircleAlert,
                    look.danger,
                    text(format!("{interface} is down, so every torrent is paused"), ty::SM, look.ink),
                    None,
                    look,
                ))
                .boxed(),
            );
        }
        if let Some(error) = &e.listen_error {
            page = page.push((text(error, ty::SM, look.warn)).boxed());
        }
        let down = b.rates.get("download").map_or(&[][..], Vec::as_slice);
        let up = b.rates.get("upload").map_or(&[][..], Vec::as_slice);
        let peers =
            d.downloads.iter().filter_map(|d| d.live.as_ref()).filter(|l| !l.paused).map(|l| l.peers).sum::<i64>();
        let shared = d.downloads.iter().filter_map(|d| d.live.as_ref()).map(|l| l.uploaded).sum::<i64>();
        let stats = line(vec![
            column![
                text("Download", ty::XS, look.ink_3),
                crate::ui::numbers::ticker(
                    &format!("{}/s", bytes(e.download_rate)),
                    ty::Size { px: 32.0, line: 40.0 },
                    look
                )
            ]
            .spacing(4)
            .boxed(),
            column![
                text("Upload", ty::XS, look.ink_3),
                crate::ui::numbers::ticker(
                    &format!("{}/s", bytes(e.upload_rate)),
                    ty::Size { px: 32.0, line: 40.0 },
                    look
                )
            ]
            .spacing(4)
            .boxed(),
            text(
                format!("{peers} peers · {} shared{}", bytes(shared), if e.slow_hours { " · slow hours" } else { "" }),
                ty::XS,
                look.ink_3,
            )
            .boxed(),
        ]);
        let chart = iced_widget::stack![
            container(crate::ui::numbers::sparkline(
                down,
                look.info,
                Fill,
                80,
                std::time::Duration::from_secs(2),
                look
            ))
            .align_bottom(Fill),
            container(crate::ui::numbers::sparkline(up, look.ok, Fill, 80, std::time::Duration::from_secs(2), look))
                .align_bottom(Fill),
            container(stats).padding(20)
        ];
        page = page.push(squircle(chart, 18.0, look).fill(look.raised).edge().width(Fill).height(140).boxed());
    }
    page = page.push(line(
        [
            ("", "All types"),
            ("EPISODES", "Episodes / Seasons"),
            ("SPECIALS", "Specials"),
            ("MOVIES", "Movies"),
            ("OTHER", "Other / Mixed"),
        ]
        .into_iter()
        .map(|(v, l)| chip(l, b.download_category == v, msg(b, Message::DownloadCategory(v.into())), look))
        .collect(),
    ));
    let filter = b.filter.get(route).map_or("all", String::as_str);
    page = page.push(
        (line(
            [
                ("all", "All"),
                ("DOWNLOADING", "Downloading"),
                ("SEEDING", "Seeding"),
                ("DONE", "Finished"),
                ("FAILED", "Failed"),
            ]
            .into_iter()
            .map(|(v, l)| chip(l, filter == v, msg(b, Message::Filter(v.into())), look))
            .collect(),
        ))
        .boxed(),
    );
    if !b.selected.is_empty() {
        let ids = b.selected.iter().copied().collect::<Vec<_>>();
        page = page.push(
            (line(vec![
                button("Clear selection", look).plain().small().on_press(msg(b, Message::SelectAll(false))).boxed(),
                button("Pause", look).small().on_press(act(b, Action::Pause(ids.clone()))).boxed(),
                button("Resume", look).small().on_press(act(b, Action::Resume(ids.clone()))).boxed(),
                button("Remove", look)
                    .danger()
                    .small()
                    .on_press(confirm(
                        b,
                        "Remove downloads?",
                        "The downloaded files will stay on disk.",
                        Action::Remove(ids, false),
                    ))
                    .boxed(),
            ]))
            .boxed(),
        );
    }
    if d.downloads.is_empty() {
        return page.push((layout::empty("No downloads yet", None, look)).boxed()).boxed();
    }
    for group in ["DOWNLOADING", "SEEDING", "DONE", "FAILED"] {
        if filter != "all" && filter != group {
            continue;
        }
        let mut list = Column::<View<'a>>::new().spacing(8);
        let mut count = 0;
        for dl in
            d.downloads.iter().filter(|dl| b.download_category.is_empty() || dl.category == b.download_category).filter(
                |dl| match group {
                    "DOWNLOADING" => dl.state == "DOWNLOADING" || (dl.state == "PAUSED" && dl.finished_at.is_none()),
                    "SEEDING" => dl.state == "SEEDING" || (dl.state == "PAUSED" && dl.finished_at.is_some()),
                    "FAILED" => matches!(dl.state.as_str(), "FAILED" | "REMOVED"),
                    _ => dl.state == group,
                },
            )
        {
            count += 1;
            let live = dl.live.as_ref();
            let mut details = column![
                row![
                    title_link(&dl.name, dl.title.as_ref().map(|t| Route::Title(t.id as u64)), look),
                    space().width(Fill),
                    state(&dl.state, look)
                ]
                .align_y(Alignment::Center),
                text(
                    format!("{} · {}", dl.source.as_deref().unwrap_or(""), bytes(dl.size.unwrap_or(0))),
                    ty::XS,
                    look.ink_3
                )
            ]
            .spacing(6)
            .width(Fill);
            if let Some(l) = live {
                details = details.push((progress(Some(l.progress as f32), look.info, look)).boxed()).push(
                    (text(
                        format!(
                            "{:.1}% · ↓ {}/s · ↑ {}/s · {} peers · {} seeds · ratio {:.2}{}",
                            l.progress * 100.0,
                            bytes(l.download_rate),
                            bytes(l.upload_rate),
                            l.peers,
                            l.seeds,
                            l.ratio,
                            l.eta.map(|t| format!(" · {} left", runtime(t as f64))).unwrap_or_default()
                        ),
                        ty::XS,
                        look.ink_3,
                    ))
                    .boxed(),
                );
            }
            if let Some(error) = dl.error.as_ref().or(dl.import_error.as_ref()) {
                details = details.push((text(error, ty::S13, look.danger)).boxed());
            }
            let paused = dl.state == "PAUSED" || live.is_some_and(|l| l.paused);
            let mut actions =
                vec![
                    button(if paused { "Resume" } else { "Pause" }, look)
                        .small()
                        .on_press_maybe((!b.busy).then(|| {
                            act(b, if paused { Action::Resume(vec![dl.id]) } else { Action::Pause(vec![dl.id]) })
                        }))
                        .boxed(),
                    button("Recheck", look).plain().small().on_press(act(b, Action::Recheck(vec![dl.id]))).boxed(),
                    button("Remove", look)
                        .danger()
                        .small()
                        .on_press(confirm(
                            b,
                            "Remove download?",
                            "The downloaded files will stay on disk.",
                            Action::Remove(vec![dl.id], false),
                        ))
                        .boxed(),
                    button("Delete files", look)
                        .danger()
                        .small()
                        .on_press(confirm(
                            b,
                            "Delete download and files?",
                            "This permanently deletes the downloaded files.",
                            Action::Remove(vec![dl.id], true),
                        ))
                        .boxed(),
                ];
            if dl.finished_at.is_some() && dl.import_state != "DONE" && dl.state != "REMOVED" {
                actions.insert(0, button("Import", look).small().on_press(act(b, Action::Import(dl.id))).boxed());
            }
            list = list.push(
                (raised(
                    row![
                        button(if b.selected.contains(&dl.id) { "☑" } else { "☐" }, look)
                            .plain()
                            .small()
                            .on_press(msg(b, Message::Select(dl.id, !b.selected.contains(&dl.id))))
                            .boxed(),
                        picture(b, dl.poster.as_deref(), 48.0, 72.0, 8.0, look),
                        column![details, line(actions)].spacing(8).width(Fill)
                    ]
                    .spacing(12)
                    .boxed(),
                    look,
                ))
                .boxed(),
            );
        }
        if count > 0 {
            page = page.push(
                (section(
                    match group {
                        "DOWNLOADING" => "Downloading",
                        "SEEDING" => "Seeding",
                        "DONE" => "Finished",
                        _ => "Failed",
                    },
                    list.boxed(),
                    look,
                ))
                .boxed(),
            );
        }
    }
    page.spacing(16).boxed()
}

pub struct PaletteItem {
    pub label: String,
    pub route: Route,
    pub group: &'static str,
    pub image: Option<String>,
}

pub fn palette(app: &App) -> Vec<PaletteItem> {
    let b = &app.browsing;
    let mut items = crate::shell::commands(&app.query)
        .into_iter()
        .filter(|(_, route)| allowed(b, route))
        .map(|(label, route)| PaletteItem { label: label.into(), route, group: "Go to", image: None })
        .collect::<Vec<_>>();
    let needle = app.query.trim().to_lowercase();
    for l in b.libraries().iter().filter(|l| l.name.to_lowercase().contains(&needle)) {
        items.push(PaletteItem {
            label: l.name.clone(),
            route: Route::Library(l.name.clone()),
            group: "Go to",
            image: None,
        });
    }
    if needle.is_empty() {
        for c in &b.recent {
            items.push(PaletteItem {
                label: c.name.clone(),
                route: Route::Title(c.id as u64),
                group: "Recent",
                image: c.poster.clone(),
            });
        }
    } else if b.search_query.to_lowercase() == needle
        && let Some(d) = b.data(&Key::Search(b.search_query.clone()))
    {
        if let Some(s) = &d.search {
            for c in &s.titles {
                items.push(PaletteItem {
                    label: c.name.clone(),
                    route: Route::Title(c.id as u64),
                    group: "Titles",
                    image: c.poster.clone(),
                });
            }
            for v in &s.videos {
                items.push(PaletteItem {
                    label: format!(
                        "{} · {} · {}",
                        v.title.name,
                        v.label.as_deref().unwrap_or(""),
                        v.name.as_deref().unwrap_or("")
                    ),
                    route: Route::Watch(v.id as u64, None),
                    group: "Episodes",
                    image: None,
                });
            }
        }
        if let Some(s) = &d.music_search {
            for a in &s.artists {
                items.push(PaletteItem {
                    label: a.name.clone(),
                    route: Route::Artist(a.id as u64),
                    group: "Artists",
                    image: a.cover.clone(),
                });
            }
            for a in &s.albums {
                items.push(PaletteItem {
                    label: format!("{} · {}", a.name, a.artist),
                    route: Route::Album(a.id as u64),
                    group: "Albums",
                    image: a.cover.clone(),
                });
            }
        }
    }
    items
}

pub fn search<'a>(app: &'a App, look: &Look) -> View<'a> {
    let b = &app.browsing;
    let mut results = Column::<View<'a>>::new().spacing(2).width(Fill);
    let mut group = "";
    let items = palette(app);
    let searching = !app.query.trim().is_empty()
        && (b.search_query != app.query.trim() || b.data(&Key::Search(b.search_query.clone())).is_none());
    for (index, item) in items.iter().enumerate() {
        if item.group != group {
            group = item.group;
            results = results.push(container(text(group, ty::XS2, look.ink_3)).padding([8, 10]).boxed());
        }
        let content = row![
            picture(b, item.image.as_deref(), 28.0, 42.0, 6.0, look),
            text(item.label.clone(), ty::SM, look.ink).width(Fill)
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        results = results.push(menu_item(
            None,
            content,
            None,
            b.search_index == index,
            Some(AppMessage::Navigate(item.route.clone())),
            look,
        ));
    }
    if searching {
        results = results.push(container(spinner(16.0, look)).padding(16).boxed());
    } else if items.is_empty() {
        results = results.push(
            container(text(format!("Nothing called “{}”.", app.query.trim()), ty::SM, look.ink_3)).padding(24).boxed(),
        );
    }
    let l = *look;
    let field = row![
        crate::ui::icon::icon(Icon::Search, 18.0).color(look.ink_3),
        iced_widget::text_input("Search, or jump anywhere", &app.query)
            .id(crate::shell::SEARCH)
            .on_input(AppMessage::Query)
            .on_submit(AppMessage::Submit)
            .size(15)
            .padding([16, 0])
            .style(move |_, _| iced_widget::text_input::Style {
                background: iced_core::Background::Color(Color::TRANSPARENT),
                border: iced_core::Border::default(),
                placeholder: l.ink_3,
                value: l.ink,
                selection: l.selection
            }),
        text("Esc", ty::XS2, look.ink_3)
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([0, 16]);
    panel(
        column![
            field,
            divider(look),
            scrollable(results).id("palette-results").height(iced_core::Length::Shrink.max(440)),
            container(text("↑ ↓ Navigate    Enter Open    Esc Close", ty::XS2, look.ink_3)).padding([8, 16])
        ],
        16.0,
        look,
    )
    .width(640)
    .boxed()
}

pub fn notifications<'a>(app: &'a App, look: &Look) -> View<'a> {
    let b = &app.browsing;
    let inbox = b.data(&Key::Inbox).and_then(|d| d.notifications.as_ref());
    let unread = inbox.map_or(0, |i| i.unread);
    let mut items = Column::<View<'a>>::new().spacing(2);
    if let Some(i) = inbox {
        for fresh in [true, false] {
            if i.items.iter().any(|n| n.read_at.is_none() == fresh) {
                items = items.push(
                    (container(text(if fresh { "New" } else { "Earlier" }, ty::XS, look.ink_3)).padding([6, 10]))
                        .boxed(),
                );
            }
            for n in i.items.iter().filter(|n| n.read_at.is_none() == fresh) {
                let mut details =
                    column![text(&n.title, ty::S13, if fresh { look.ink } else { look.ink_2 }).font(if fresh {
                        MEDIUM
                    } else {
                        ty::REGULAR
                    })]
                    .spacing(4)
                    .width(Fill);
                if let Some(body) = &n.body {
                    details = details.push((text(body, ty::XS, look.ink_3)).boxed());
                }
                details = details.push((text(relative(n.created_at), ty::XS2, look.ink_3)).boxed());
                items = items.push(
                    (row![
                        card_button(look)
                            .plain()
                            .content(row![picture(b, n.image.as_deref(), 36.0, 54.0, 8.0, look), details].spacing(12))
                            .on_press(msg(b, Message::Follow(n.clone())))
                            .boxed(),
                        icon_button(Icon::X, "Remove", Some(act(b, Action::DeleteNotice(Some(n.id)))), look)
                    ]
                    .align_y(Alignment::Start))
                    .boxed(),
                );
            }
        }
        if i.items.is_empty() {
            items = items.push((layout::empty("Nothing yet", None, look)).boxed());
        }
    } else {
        items = items.push((pending(b, &Key::Inbox, look)).boxed());
    }
    panel(
        column![
            row![
                text("Notifications", ty::SM, look.ink).font(MEDIUM).width(Fill),
                button(if unread > 0 { "Mark all read" } else { "Clear" }, look)
                    .plain()
                    .small()
                    .on_press_maybe(
                        (!b.busy)
                            .then(|| act(b, if unread > 0 { Action::Read(None) } else { Action::DeleteNotice(None) }))
                    )
                    .boxed()
            ]
            .spacing(8)
            .padding([8, 10]),
            divider(look),
            scrollable(items).height(iced_core::Length::Shrink.max(544))
        ],
        16.0,
        look,
    )
    .padding(6)
    .width(384)
    .boxed()
}

pub fn dialog<'a>(app: &'a App, look: &Look) -> Option<View<'a>> {
    let b = &app.browsing;
    let dialog = b.dialog.as_ref()?;
    let close = msg(b, Message::Dialog(None));
    let mut content = Column::<View<'a>>::new().spacing(16).width(Fill);
    match dialog {
        Dialog::Confirm { title, body, action } => {
            return Some(crate::ui::overlay::confirm(
                title,
                Some(body),
                "Confirm",
                true,
                move |yes| if yes { act(b, action.as_ref().clone()) } else { close.clone() },
                look,
            ));
        },
        Dialog::Pick(r) => {
            content = content.push(
                (row![
                    picture(b, r.poster.as_deref(), 80.0, 120.0, 10.0, look),
                    column![
                        text(&r.name, ty::S15, look.ink).font(SEMIBOLD),
                        text(r.overview.as_deref().unwrap_or(""), ty::S13, look.ink_2)
                    ]
                    .spacing(8)
                    .width(Fill)
                ]
                .spacing(16))
                .boxed(),
            );
            if b.can().manage_shows {
                let options = b.data(&Key::Options).and_then(|d| d.settings.as_ref());
                if let Some(options) = options {
                    content = content.push((text("Library", ty::S13, look.ink_2)).boxed()).push(
                        (line(
                            options
                                .libraries
                                .iter()
                                .filter(|l| l.metadata_provider.as_deref() == Some(&r.provider))
                                .map(|l| {
                                    chip(
                                        &l.name,
                                        b.pick_library == l.name,
                                        msg(b, Message::PickLibrary(l.name.clone())),
                                        look,
                                    )
                                })
                                .collect(),
                        ))
                        .boxed(),
                    );
                    if options.libraries.iter().find(|l| l.name == b.pick_library).is_some_and(|l| !l.managed) {
                        content = content.push(
                            (text(
                                format!("{} isn't managed; can't download into it.", b.pick_library),
                                ty::XS,
                                look.warn,
                            ))
                            .boxed(),
                        );
                    }
                    content = content.push((text("Quality", ty::S13, look.ink_2)).boxed()).push(
                        (line(
                            std::iter::once(chip(
                                "Library default",
                                b.profile.is_empty(),
                                msg(b, Message::Profile(String::new())),
                                look,
                            ))
                            .chain(options.profiles.iter().map(|p| {
                                chip(&p.name, b.profile == p.name, msg(b, Message::Profile(p.name.clone())), look)
                            }))
                            .collect(),
                        ))
                        .boxed(),
                    );
                } else {
                    content = content.push((pending(b, &Key::Options, look)).boxed());
                }
                content = content.push((text("Download", ty::S13, look.ink_2)).boxed()).push(
                    (line(
                        [("NONE", "Off"), ("FUTURE", "New episodes"), ("MISSING", "Everything")]
                            .into_iter()
                            .map(|(v, l)| chip(l, b.chosen_monitor() == v, msg(b, Message::Monitor(v.into())), look))
                            .collect(),
                    ))
                    .boxed(),
                );
                if b.chosen_monitor() == "MISSING"
                    && let Some(aired) = b
                        .data(&Key::Aired(r.provider.clone(), r.id.clone()))
                        .and_then(|d| d.aired_episodes)
                        .filter(|n| *n >= 100)
                {
                    content=content.push((text(format!("{} has {aired} aired episodes, and all of them will be downloaded. Pick Future to only get new ones.",r.name),ty::XS,look.warn)).boxed());
                }
                content = content.push(
                    (button("Add show", look)
                        .primary()
                        .on_press_maybe((!b.busy && options.is_some() && !b.pick_library.is_empty()).then(|| {
                            act(
                                b,
                                Action::Add(
                                    r.clone(),
                                    b.pick_library.clone(),
                                    b.chosen_monitor().to_owned(),
                                    b.profile.clone(),
                                ),
                            )
                        }))
                        .boxed())
                    .boxed(),
                );
            } else {
                content = content
                    .push(
                        (field(
                            "A note for the admin (optional)",
                            input(
                                "request-note",
                                &b.note,
                                "Dubbed if there is one, please",
                                move |n| msg(b, Message::Note(n)),
                                look,
                            )
                            .boxed(),
                            None,
                            look,
                        ))
                        .boxed(),
                    )
                    .push(
                        (button("Request", look)
                            .primary()
                            .on_press_maybe((!b.busy).then(|| act(b, Action::Request(r.clone(), b.note.clone()))))
                            .boxed())
                        .boxed(),
                    );
            }
        },
        Dialog::Releases { series, season, episodes, name } => {
            content = content
                .push((text(format!("Search releases · {name}"), ty::S15, look.ink).font(SEMIBOLD)).boxed())
                .push(
                    (input(
                        "release-query",
                        &b.release_query,
                        "Search releases",
                        move |q| msg(b, Message::ReleaseQuery(q)),
                        look,
                    )
                    .boxed())
                    .boxed(),
                );
            let key = Key::Releases(*series, *season, episodes.clone(), b.release_query.clone());
            if let Some(s) = b.data(&key).and_then(|d| d.series.as_ref()) {
                let mut releases = Column::<View<'a>>::new().spacing(12);
                for r in &s.releases {
                    releases = releases.push(
                        (raised(
                            column![
                                text(&r.release.title, ty::S13, look.ink).font(MEDIUM),
                                text(
                                    format!(
                                        "{} · {} · {} seeds · score {}",
                                        r.release.source,
                                        bytes(r.release.size.unwrap_or(0)),
                                        r.release.seeders.unwrap_or(0),
                                        r.verdict.score
                                    ),
                                    ty::XS,
                                    look.ink_3
                                ),
                                text(
                                    r.verdict
                                        .rejections
                                        .iter()
                                        .chain(&r.verdict.warnings)
                                        .cloned()
                                        .collect::<Vec<_>>()
                                        .join(" · "),
                                    ty::XS,
                                    if r.verdict.accepted { look.ink_3 } else { look.warn }
                                ),
                                button("Grab", look)
                                    .primary()
                                    .small()
                                    .on_press_maybe((!b.busy).then(|| if r.verdict.accepted {
                                        act(b, Action::Grab(*series, std::sync::Arc::new(r.clone())))
                                    } else {
                                        confirm(
                                            b,
                                            "Grab rejected release?",
                                            "This release doesn't meet the quality profile.",
                                            Action::Grab(*series, std::sync::Arc::new(r.clone())),
                                        )
                                    }))
                                    .boxed()
                            ]
                            .spacing(6)
                            .boxed(),
                            look,
                        ))
                        .boxed(),
                    );
                }
                if s.releases.is_empty() {
                    releases = releases.push((layout::empty("No releases found", None, look)).boxed());
                }
                content = content.push((scrollable(releases).height(iced_core::Length::Shrink.max(420))).boxed());
            } else {
                content = content.push((pending(b, &key, look)).boxed());
            }
        },
        Dialog::SeriesSettings(s) => {
            let draft = &b.draft;
            content = content.push(text(&s.name, ty::S15, look.ink).font(MEDIUM).boxed());
            let mut form = Column::<View<'a>>::new().spacing(16);
            form = form.push(text("Quality profile", ty::S13, look.ink_2).boxed());
            if let Some(options) = b.data(&Key::Options).and_then(|d| d.settings.as_ref()) {
                form = form.push(line(
                    std::iter::once(chip(
                        "Library default",
                        draft.profile.is_empty(),
                        msg(b, Message::Draft("profile", String::new())),
                        look,
                    ))
                    .chain(options.profiles.iter().map(|p| {
                        chip(&p.name, draft.profile == p.name, msg(b, Message::Draft("profile", p.name.clone())), look)
                    }))
                    .collect(),
                ));
                form = form.push(text("Sources", ty::S13, look.ink_2).boxed()).push(line(
                    std::iter::once(chip(
                        "All sources",
                        draft.sources.is_empty(),
                        msg(b, Message::Source(String::new())),
                        look,
                    ))
                    .chain(options.sources.iter().map(|source| {
                        chip(
                            &source.name,
                            draft.sources.contains(&source.name),
                            msg(b, Message::Source(source.name.clone())),
                            look,
                        )
                    }))
                    .collect(),
                ));
            }
            for (id, label, value) in [
                ("groups", "Preferred groups", &draft.groups),
                ("aliases", "Also known as", &draft.aliases),
                ("naming", "File names", &draft.naming),
            ] {
                form = form.push(field(
                    label,
                    input(id, value, "", move |value| msg(b, Message::Draft(id, value)), look).boxed(),
                    None,
                    look,
                ));
            }
            if let Some(preview) = b
                .data(&Key::Naming(s.id, draft.naming.clone()))
                .and_then(|d| d.series.as_ref())
                .and_then(|s| s.naming_preview.as_ref())
            {
                for sample in &preview.samples {
                    form = form.push(text(sample, ty::XS, look.ink_3).boxed());
                }
                if let Some(error) = &preview.error {
                    form = form.push(text(error, ty::XS, look.danger).boxed());
                }
            }
            form = form.push(text("Episode numbers in releases", ty::S13, look.ink_2).boxed()).push(line(
                [("AUTO", "Work it out"), ("SEASONAL", "Per season"), ("ABSOLUTE", "From the first episode")]
                    .into_iter()
                    .map(|(v, l)| chip(l, draft.numbering == v, msg(b, Message::Draft("numbering", v.into())), look))
                    .collect(),
            ));
            form = form.push(text("Seeding", ty::S13, look.ink_2).boxed()).push(line(
                [
                    ("default", "Default"),
                    ("none", "Don't seed"),
                    ("ratio", "Ratio 1.0"),
                    ("week", "7 days"),
                    ("forever", "Forever"),
                ]
                .into_iter()
                .map(|(v, l)| chip(l, draft.seeding == v, msg(b, Message::Draft("seeding", v.into())), look))
                .collect(),
            ));
            content = content.push(scrollable(form).height(iced_core::Length::Shrink.max(440)).boxed()).push(
                button("Save", look)
                    .primary()
                    .on_press_maybe((!b.busy).then(|| act(b, Action::UpdateSeries(s.id, draft.patch()))))
                    .boxed(),
            );
        },
        Dialog::Match(id) => {
            content = content
                .push((text("Fix match", ty::S15, look.ink).font(SEMIBOLD)).boxed())
                .push(
                    (input(
                        "match-query",
                        &b.match_query,
                        "Search titles",
                        move |q| msg(b, Message::MatchQuery(q)),
                        look,
                    )
                    .boxed())
                    .boxed(),
                )
                .push(
                    (line(
                        [("", "Library provider"), ("TMDB", "TMDB"), ("ANILIST", "AniList")]
                            .into_iter()
                            .map(|(v, l)| chip(l, b.provider == v, msg(b, Message::Provider(v.into())), look))
                            .collect(),
                    ))
                    .boxed(),
                );
            let key = Key::Match(*id, b.match_query.clone(), b.provider.clone());
            if let Some(c) = b.data(&key).and_then(|d| d.title.as_ref()).and_then(|t| t.match_candidates.as_ref()) {
                let mut list = Column::<View<'a>>::new().spacing(8);
                for r in &c.results {
                    list = list.push(
                        (card_button(look)
                            .plain()
                            .content(
                                row![
                                    picture(b, r.poster.as_deref(), 48.0, 72.0, 8.0, look),
                                    column![
                                        text(&r.name, ty::SM, look.ink),
                                        text(r.year.map(|y| y.to_string()).unwrap_or_default(), ty::XS, look.ink_3)
                                    ]
                                ]
                                .spacing(12),
                            )
                            .on_press(act(b, Action::Match(*id, r.provider.clone(), r.id.clone())))
                            .boxed())
                        .boxed(),
                    );
                }
                content = content.push((scrollable(list).height(iced_core::Length::Shrink.max(420))).boxed());
            } else {
                content = content.push((pending(b, &key, look)).boxed());
            }
        },
    }
    if let Some(error) = &b.error {
        content = content.push((text(error, ty::SM, look.danger)).boxed());
    }
    if b.busy {
        content = content.push((spinner(20.0, look)).boxed());
    }
    content = content.push((button("Close", look).plain().on_press(close.clone()).boxed()).boxed());
    Some(crate::ui::overlay::dialog(content, 640.0, close, look))
}
fn confirm(b: &Browsing, title: &str, body: &str, action: Action) -> AppMessage {
    msg(
        b,
        Message::Dialog(Some(Dialog::Confirm {
            title: title.into(),
            body: body.into(),
            action: std::sync::Arc::new(action),
        })),
    )
}
pub fn allowed(b: &Browsing, route: &Route) -> bool {
    let can = b.can();
    match route {
        Route::Discover => b.features() && (can.manage_shows || can.request),
        Route::Wanted => b.features() && can.manage_shows,
        Route::Requests => b.features() && (can.manage_requests || can.request),
        Route::Downloads => can.downloads,
        Route::Calendar => b.features(),
        _ => true,
    }
}
fn code(season: i64, episode: i64) -> String {
    format!("S{season:02}E{episode:02}")
}
fn runtime(seconds: f64) -> String {
    let m = (seconds / 60.0).round() as i64;
    if m >= 60 { format!("{} h {} min", m / 60, m % 60) } else { format!("{m} min") }
}
fn remaining(position: f64, duration: Option<f64>) -> String {
    duration.map(|d| format!("{} left", runtime((d - position).max(0.0)))).unwrap_or_default()
}
fn bytes(n: i64) -> String {
    let n = n.max(0) as f64;
    for (unit, size) in [("GB", 1e9), ("MB", 1e6), ("KB", 1e3)] {
        if n >= size {
            return format!("{:.1} {unit}", n / size);
        }
    }
    format!("{n:.0} B")
}
fn relative(at: i64) -> String {
    let s = (now() - at).max(0);
    if s < 60 {
        "just now".into()
    } else if s < 3600 {
        format!("{} min ago", s / 60)
    } else if s < 86400 {
        format!("{} h ago", s / 3600)
    } else {
        format!("{} d ago", s / 86400)
    }
}
fn countdown(at: i64) -> String {
    let s = (at - now()).max(0);
    if s == 0 {
        "Out now".into()
    } else if s >= 86400 {
        format!("in {} d {} h", s / 86400, s % 86400 / 3600)
    } else {
        format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
    }
}
fn date(at: i64) -> String {
    use crate::platform::{Current, Desktop};
    let days = (at + Current::utc_offset(at)).div_euclid(86400) + 719468;
    let era = days.div_euclid(146097);
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    format!(
        "{} {day}",
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][(month - 1) as usize]
    )
}
fn parse_tint(tint: &str) -> Option<Color> {
    let rgb = tint.split_whitespace().map(str::parse::<f32>).collect::<Result<Vec<_>, _>>().ok()?;
    if rgb.len() != 3 {
        return None;
    }
    Some(Color::from_rgb(rgb[0] / 255.0, rgb[1] / 255.0, rgb[2] / 255.0))
}
