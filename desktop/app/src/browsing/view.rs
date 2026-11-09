// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::Length::Fill;
use iced_core::{Alignment, Color, Padding};
use iced_widget::{Column, Element, Row, Widget, column, container, row, scrollable, space};
use serde_json::json;

use super::model::*;
use super::{Action, Browsing, Dialog, Key, Message, calendar_start, now, shift_days};
use crate::app::{App, Message as AppMessage};
use crate::route::Route;
use crate::ui::controls::*;
use crate::ui::icon::Icon;
use crate::ui::squircle::squircle;
use crate::ui::text::{self as ty, MEDIUM, SEMIBOLD, text, truncate};
use crate::ui::{Look, effects, layout, media};

mod calendar;
mod discover;
mod downloads;
mod home;
mod library;
mod overlays;
mod requests;
mod title;
mod wanted;
use calendar::calendar;
use discover::{discover, discover_card};
use downloads::downloads;
use home::home;
use library::{book_tile, books, library};
pub use overlays::{dialog, notifications, palette, search};
use requests::requests;
use title::title;
use wanted::wanted;

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
    hover: Option<f32>,
}
fn card_button<'a>(look: &Look) -> CardButton<'a> {
    CardButton { look: *look, content: None, message: None, hover: None }
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

    /// Fills in under the pointer (`group-hover:bg-hover`), with corners of `radius`.
    fn hover(mut self, radius: f32) -> Self {
        self.hover = Some(radius);
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
        use crate::ui::pressable::{Appearance, Status};
        use crate::ui::squircle::Paint;
        let (ink, fill, hover) = (self.look.ink, self.look.hover, self.hover.is_some());
        let radius = self.hover.unwrap_or(14.0);
        crate::ui::pressable::pressable(
            self.content.unwrap_or_else(|| space().boxed()),
            radius,
            &self.look,
            move |status| Appearance {
                paint: if hover && matches!(status, Status::Hovered | Status::Pressed) {
                    Paint::fill(fill)
                } else {
                    Paint::NONE
                },
                ink,
            },
        )
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
/// `mediaOptions`: every kind of release, then each on its own.
const MEDIA_OPTIONS: &[&str] = &["", "EPISODES", "SPECIALS", "MOVIES", "OTHER"];
fn media_label(category: &str) -> &str {
    match category {
        "" => "All types",
        "EPISODES" => "Episodes / Seasons",
        "SPECIALS" => "Specials",
        "MOVIES" => "Movies",
        "OTHER" => "Other / Mixed",
        other => other,
    }
}
fn media_select<'a>(value: &str, on_change: impl Fn(String) -> AppMessage + 'a, look: &Look) -> View<'a> {
    let value = MEDIA_OPTIONS.iter().copied().find(|o| *o == value);
    crate::ui::select::select(value, MEDIA_OPTIONS, |v| media_label(v).to_owned(), move |v| on_change(v.into()), look)
}
/// `<PageTitle>`: the page's name, with `aside` at its end, their bottoms lined up.
fn page_title<'a>(name: impl Into<String>, aside: Option<View<'a>>, look: &Look) -> View<'a> {
    let name = truncate(name.into(), ty::TITLE, look.ink).font(SEMIBOLD).width(Fill).boxed();
    container(Row::with_children([name].into_iter().chain(aside)).spacing(16).align_y(Alignment::End))
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
fn shelf<'a>(b: &'a Browsing, id: impl Into<String>, children: Vec<View<'a>>, look: &Look) -> View<'a> {
    let id = id.into();
    let page_id = id.clone();
    layout::shelf(
        id.clone(),
        children,
        b.shelves.get(&id).copied(),
        move |scroll| msg(b, Message::Shelf(id.clone(), scroll.viewport)),
        move |direction| msg(b, Message::PageShelf(page_id.clone(), direction)),
        look,
        0.0,
    )
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
pub fn watch_image<'a>(b: &'a Browsing, path: Option<&str>, content: View<'a>) -> View<'a> {
    if let Some(path) = path {
        crate::ui::visible::visible(
            content,
            format!("{}/{path}/{}", b.generation, b.images.picture(path).is_some()),
            msg(b, Message::WantImage(path.to_owned())),
        )
    } else {
        content
    }
}
pub fn art<'a>(b: &'a Browsing, path: Option<&str>, radius: f32, look: &Look) -> View<'a> {
    let content = if let Some(path) = path {
        media::img(b.images.picture(path), radius, look)
    } else {
        squircle(space().width(Fill).height(Fill), radius, look).fill(look.raised).boxed()
    };
    watch_image(b, path, content)
}
fn picture<'a>(b: &'a Browsing, path: Option<&str>, width: f32, height: f32, radius: f32, look: &Look) -> View<'a> {
    container(art(b, path, radius, look)).width(width).height(height).boxed()
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
            corner: if c.fresh_count > 0 && !done {
                Some((
                    if c.kind == "SHOW" && c.fresh_count > 1 { format!("{} new", c.fresh_count) } else { "New".into() },
                    true,
                ))
            } else {
                let unwatched = if c.kind == "SHOW" { c.video_count - c.watched_count } else { 0 };
                (c.kind == "SHOW" && c.watched_count > 0 && unwatched > 0).then(|| (unwatched.to_string(), false))
            },
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
        crate::ui::context_menu::context_menu(watch_image(b, c.poster.as_deref(), poster), items, look)
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
        Route::Downloads => downloads(b, look),
        _ => return None,
    };
    let mut page = Column::<View<'a>>::new().width(Fill);
    if let Some(error) = &b.error {
        page = page.push((banner(Icon::CircleAlert, look.danger, text(error, ty::SM, look.ink), None, look)).boxed());
    }
    if let Some(notice) = &b.notice {
        page = page.push(line(vec![
            text(notice, ty::SM, look.ok).boxed(),
            if let Some(undo) = &b.undo {
                button("Undo", look).plain().small().on_press(act(b, undo.clone())).boxed()
            } else {
                space().boxed()
            },
        ]));
    }
    page = page.push((content).boxed());
    let page = layout::page(page, matches!(route, Route::Calendar));
    let loaded = match route {
        Route::Home => b.data(&Key::Home).is_some(),
        Route::Title(id) => b.data(&Key::Title(*id as i64)).is_some(),
        Route::Library(name) | Route::Series { library: name, .. } => {
            b.data(&Key::Library(name.clone())).or_else(|| b.data(&Key::Books(name.clone()))).is_some()
        },
        Route::Discover => b.data(&Key::Discover(b.library.clone(), b.query.clone())).is_some(),
        Route::Calendar => b.data(&Key::Calendar(calendar_start(b.week))).is_some(),
        Route::Requests => b.data(&Key::Requests).is_some(),
        Route::Wanted => b.data(&Key::Wanted).is_some(),
        Route::Downloads => b.data(&Key::Downloads).is_some(),
        _ => false,
    };
    let page = if loaded { effects::enter(page, effects::Entrance::arrive(0), look) } else { page };

    if let Route::Title(id) = route
        && let Some(t) = b.data(&Key::Title(*id as i64)).and_then(|d| d.title.as_ref())
        && let Some(path) = &t.card.backdrop
    {
        let canvas = look.canvas;
        let mut layers = vec![
            container(art(b, Some(path), 0.0, look)).width(Fill).height(544).boxed(),
            container(crate::ui::squircle::gradient(
                0.0,
                std::f32::consts::PI,
                vec![(0.0, canvas.scale_alpha(0.55)), (0.2, canvas.scale_alpha(0.55)), (1.0, canvas)],
                look,
            ))
            .height(544)
            .width(Fill)
            .boxed(),
        ];
        if let Some(color) = t.card.backdrop_tint.as_deref().or(t.card.poster_tint.as_deref()).and_then(parse_tint) {
            layers.push(effects::ambient(effects::Ambient::title(color)));
        }
        let mut stack = iced_widget::Stack::with_children([page]);
        for layer in layers.into_iter().rev() {
            stack = stack.push_under(layer);
        }
        Some(stack.width(Fill).boxed())
    } else {
        Some(page)
    }
}

const BIG: ty::Size = ty::Size { px: 40.0, line: 40.0 };

struct Hero<'a> {
    name: &'a str,
    label: &'a str,
    mark: crate::ui::icon::Glyph,
    detail: String,
    image: Option<&'a str>,
    tint: Option<&'a str>,
    route: Option<Route>,
    height: f32,
    name_size: f32,
    padding: f32,
    /// What sits under the detail line: the big number and the button.
    body: View<'a>,
    aside: Option<View<'a>>,
    /// A bar filling the bottom edge, `None` inside while it can't tell how far along it is.
    bar: Option<Option<f32>>,
}
fn hero<'a>(b: &'a Browsing, props: Hero<'a>, look: &Look) -> View<'a> {
    let Hero { name, label, mark, detail, image, tint, route, height, name_size, padding, body, aside, bar } = props;
    let l = *look;
    let content = column![
        row![mark, text(label.to_uppercase(), ty::XS, look.ink_2).font(MEDIUM)].spacing(8).align_y(Alignment::Center),
        container(text(name.to_owned(), ty::Size { px: name_size, line: name_size * 1.25 }, look.ink).font(SEMIBOLD))
            .padding(Padding { top: 8.0, ..Padding::ZERO }),
        container(truncate(detail, ty::SM, look.ink_2)).padding(Padding { top: 4.0, ..Padding::ZERO }),
        container(body).padding(Padding { top: 20.0, ..Padding::ZERO })
    ]
    .width(Fill);
    let mut layers = vec![art(b, image, 22.0, look)];
    layers.push(crate::ui::squircle::gradient(
        22.0,
        std::f32::consts::FRAC_PI_2,
        vec![(0.0, l.canvas), (0.5, l.canvas.scale_alpha(0.68)), (1.0, Color::TRANSPARENT)],
        look,
    ));
    if let Some(color) = tint.and_then(parse_tint) {
        layers.push(effects::ambient(effects::Ambient::corner(color)));
    }
    if let Some(value) = bar {
        layers.push(container(progress(value, look.info, look)).align_bottom(Fill).boxed());
    }
    let content = match aside {
        Some(aside) => row![content, container(aside).width(256)].spacing(32).align_y(Alignment::End).boxed(),
        None => content.boxed(),
    };
    layers.push(container(content).align_bottom(Fill).padding(padding).boxed());
    let destination = route;
    card_button(look)
        .content(crate::ui::tilt::card(
            iced_widget::Stack::with_children(layers).height(height).width(Fill),
            22.0,
            0.0,
            look,
        ))
        .on_press_maybe(destination.map(AppMessage::Navigate))
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
    let mut layers = vec![match image {
        Some(path) => media::img(b.images.picture(path), 16.0, look),
        None => squircle(space().width(Fill).height(Fill), 16.0, look).fill(look.raised).boxed(),
    }];
    let shade = look.media_shade;
    layers.push(crate::ui::squircle::gradient(
        16.0,
        std::f32::consts::PI,
        vec![(0.0, shade.scale_alpha(0.1)), (1.0, shade.scale_alpha(0.85))],
        look,
    ));
    if let Some(label) = label {
        layers.push(container(badge(label, Tone::Media, look)).padding(12).boxed());
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
    watch_image(
        b,
        image,
        container(
            card_button(look)
                .content(crate::ui::tilt::card(
                    iced_widget::Stack::with_children(layers).width(Fill).height(width * 9.0 / 16.0),
                    16.0,
                    4.0,
                    look,
                ))
                .on_press_maybe(route.map(AppMessage::Navigate))
                .boxed(),
        )
        .width(width)
        .boxed(),
    )
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
        Route::Downloads => b.features() && can.downloads,
        Route::Clips | Route::Clip(_) => b.data(&Key::Shell).and_then(|d| d.server.as_ref()).is_some_and(|s| s.clips),
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
fn duration(seconds: i64) -> String {
    let s = seconds.max(0);
    let (d, h, m) = (s / 86400, s % 86400 / 3600, s % 3600 / 60);
    match (d, h, m) {
        (0, 0, 0) => format!("{s} s"),
        (0, 0, m) => format!("{m} min"),
        (0, h, 0) => format!("{h} h"),
        (0, h, m) => format!("{h} h {m} min"),
        (d, 0, _) => format!("{d} d"),
        (d, h, _) => format!("{d} d {h} h"),
    }
}
fn speed(n: i64) -> String {
    if n == 0 { "0 KB/s".into() } else { format!("{}/s", bytes(n)) }
}
fn countdown(at: i64) -> String {
    let s = at - now();
    if s <= 0 {
        "airing now".into()
    } else if s < 3600 {
        format!("in {}:{:02}", s / 60, s % 60)
    } else {
        format!("in {}", duration(s))
    }
}
/// "13:04:22" inside the last day, "1 d 13 h" before that.
fn big_countdown(at: i64) -> String {
    let s = (at - now()).max(0);
    if s >= 86400 {
        countdown(at).replacen("in ", "", 1)
    } else if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
fn clock_time(at: i64) -> String {
    use crate::platform::{Current, Desktop};
    let minutes = (at + Current::utc_offset(at)).rem_euclid(86400) / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    format!("{}:{m:02} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" })
}
fn airs(at: i64) -> String {
    use crate::platform::{Current, Desktop};
    let day = |t: i64| (t + Current::utc_offset(t)).div_euclid(86400);
    let days = day(at) - day(now());
    let clock = clock_time(at);
    match days {
        0 => format!("Today at {clock}"),
        1 => format!("Tomorrow at {clock}"),
        -1 => format!("Yesterday at {clock}"),
        2..=6 => format!("{} at {clock}", weekday(at)),
        _ if days.abs() > 180 => format!("{}, {}", date(at), civil(at).0),
        _ => format!("{} at {clock}", date(at)),
    }
}
fn weekday(at: i64) -> &'static str {
    use crate::platform::{Current, Desktop};
    let day = (at + Current::utc_offset(at)).div_euclid(86400);
    ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"][day.rem_euclid(7) as usize]
}
/// The local (year, month, day) `at` falls on.
fn civil(at: i64) -> (i64, i64, i64) {
    use crate::platform::{Current, Desktop};
    let days = (at + Current::utc_offset(at)).div_euclid(86400) + 719468;
    let era = days.div_euclid(146097);
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}
fn date(at: i64) -> String {
    let (_, month, day) = civil(at);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_like_the_web() {
        assert_eq!(duration(45), "45 s");
        assert_eq!(duration(5 * 60), "5 min");
        assert_eq!(duration(3 * 3600), "3 h");
        assert_eq!(duration(3 * 3600 + 12 * 60), "3 h 12 min");
        assert_eq!(duration(2 * 86400 + 4 * 3600 + 59), "2 d 4 h");
        assert_eq!(duration(86400 + 30 * 60), "1 d");
    }

    #[test]
    fn civil_dates_cross_years_and_leap_days() {
        use crate::platform::{Current, Desktop};
        let local = |utc: i64| utc - Current::utc_offset(utc) + 12 * 3600;
        assert_eq!(civil(local(0)), (1970, 1, 1));
        assert_eq!(civil(local(951_782_400)), (2000, 2, 29));
        assert_eq!(civil(local(1_798_675_200)), (2026, 12, 31));
        assert_eq!(civil(local(1_798_761_600)), (2027, 1, 1));
    }
}
