// SPDX-License-Identifier: AGPL-3.0-or-later

//! The web's desktop layout (`web/src/components/Shell.tsx`): a 240 px sidebar beside a single
//! scrolling page, with dialogs over both. Focus screens take the whole window.

use iced_core::Length::{self, Fill};
use iced_core::font::Weight;
use iced_core::{Alignment, Background, Color, Font, Padding, Theme, border};
use iced_widget::{
    Column, Element, Widget, button, center_x, column, container, mouse_area, opaque, responsive, row, scrollable,
    space, stack, text, text_input,
};

use crate::app::{App, Message, Overlay, SCROLL};
use crate::chrome::{DRAG_STRIP, Side};
use crate::route::Route;
use crate::{keys, style};

pub const SEARCH: &str = "search";

const SEMIBOLD: Font = Font { weight: Weight::Semibold, ..Font::DEFAULT };
const MEDIUM: Font = Font { weight: Weight::Medium, ..Font::DEFAULT };

/// The pages the sidebar and the command palette can go to before anything is loaded from a server.
const PAGES: &[(&str, Route, &str)] = &[
    ("Home", Route::Home, "G H"),
    ("Clips", Route::Clips, "G X"),
    ("Calendar", Route::Calendar, "G C"),
    ("Add shows", Route::Discover, "G A"),
    ("Downloads", Route::Downloads, "G D"),
    ("Wanted", Route::Wanted, "G W"),
    ("Requests", Route::Requests, "G R"),
    ("Settings", Route::Settings, "G S"),
];

pub fn view(app: &App) -> Element<'_, Message> {
    let look = app.session.look();
    let route = &app.history.current().route;
    let signed_in = app.session.screen == crate::session::Screen::Ready;
    let focus = route.is_focus() || !signed_in;

    let base: Element<'_, Message> = if !signed_in {
        app.session.view()
    } else if focus {
        page(app, route)
    } else {
        row![
            sidebar(app, route),
            container(space().width(1).height(Fill)).style(style::hairline(look)),
            page(app, route)
        ]
        .boxed()
    };

    let canvas = app.session.look().canvas;
    let mut layers = vec![
        container(base)
            .style(move |_| container::Style { background: Some(canvas.into()), ..container::Style::default() })
            .width(Fill)
            .height(Fill)
            .boxed(),
    ];

    if let Some(pill) = app.chrome.pill(Side::Right, look) {
        layers.push(
            container(pill.map(Message::Chrome))
                .align_right(Fill)
                .padding(Padding { top: 8.0, right: 8.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    if focus && let Some(pill) = app.chrome.pill(Side::Left, look) {
        layers.push(
            container(pill.map(Message::Chrome)).padding(Padding { top: 8.0, left: 8.0, ..Padding::ZERO }).boxed(),
        );
    }
    if let Some(overlay) = app.overlay {
        layers.push(self::overlay(app, overlay));
    }
    layers.push(app.chrome.edges().map(Message::Chrome).boxed());

    stack(layers).width(Fill).height(Fill).boxed()
}

fn sidebar<'a>(app: &'a App, route: &Route) -> Element<'a, Message> {
    let look = app.session.look();
    let logo = row![
        crate::auth::logo(app.session.look().ink, 24.0),
        text("tinystream").size(15).font(SEMIBOLD),
        space().width(Fill),
        button(text("●").size(9).color(look.faint))
            .padding([6, 8])
            .style(style::menu_row(look))
            .on_press(Message::Overlay(Some(Overlay::Notifications))),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let logo_row: Element<'_, Message> = match app.chrome.controls(Side::Left, look) {
        Some(controls) => row![controls.map(Message::Chrome), logo].spacing(8).align_y(Alignment::Center).boxed(),
        None => container(logo).padding(Padding { left: 10.0, ..Padding::ZERO }).boxed(),
    };

    let search = button(
        row![text("⌕").size(14), text("Search").size(14).width(Fill), text("Ctrl K").size(11).color(look.faint),]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([7, 10])
    .style(style::search(look))
    .on_press(Message::Overlay(Some(Overlay::Search)));

    let nav = column![
        nav_item("Home", Route::Home, route, look),
        nav_item("Clips", Route::Clips, route, look),
        nav_item("Downloads", Route::Downloads, route, look),
    ]
    .spacing(2);

    let account = button(
        row![
            container(text("?").size(12).color(look.muted)).center(24).style(move |_: &Theme| container::Style {
                background: Some(Background::Color(look.panel)),
                border: border::rounded(999),
                ..container::Style::default()
            }),
            text(
                app.session
                    .config
                    .active()
                    .and_then(|server| server.user.as_ref())
                    .map(|user| user.username.as_str())
                    .unwrap_or("Sign in")
            )
            .size(14)
            .color(look.muted)
            .width(Fill),
            text("···").size(14).color(look.faint),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([8, 8])
    .style(style::menu_row(look))
    .on_press(Message::Overlay(Some(Overlay::Account)));

    container(
        column![
            app.chrome.drag_area(container(logo_row).height(36).align_y(Alignment::Center), Message::Chrome),
            space().height(6),
            search,
            space().height(12),
            nav,
            space().height(Fill),
            account,
        ]
        .height(Fill),
    )
    .width(style::SIDEBAR_WIDTH)
    .height(Fill)
    .padding([12, 10])
    .style(style::sidebar(look))
    .boxed()
}

fn nav_item<'a>(label: &'a str, to: Route, current: &Route, look: style::Look) -> Element<'a, Message> {
    let active = *current == to;

    button(text(label).size(14))
        .width(Fill)
        .padding([7, 10])
        .style(style::nav_item(active, look))
        .on_press(Message::Navigate(to))
        .boxed()
}

/// The page, in the window's single scroll container, under a strip that moves the window.
fn page<'a>(app: &'a App, route: &Route) -> Element<'a, Message> {
    let look = app.session.look();
    if *route == Route::Settings {
        return scrollable(app.session.settings()).width(Fill).height(Fill).boxed();
    }
    let title = route_title(route);
    let path = route.to_string();

    let content = responsive(move |size| {
        let width = size.width.min(1400.0);
        let body = column![
            text(title.clone()).size(28).font(SEMIBOLD),
            text(path.clone()).size(14).color(look.faint),
            space().height(24),
            text("Nothing here yet.").size(14).color(look.muted),
        ]
        .spacing(6)
        .padding(40)
        .width(width);

        center_x(body).boxed()
    });

    let mut notices = Column::new().spacing(8);
    if let Some(version) = app.session.mismatch() {
        notices = notices.push(
            row![
                text(format!("App {} and server {version} have different versions.", crate::updates::VERSION))
                    .size(13)
                    .width(Fill),
                button(text("Dismiss").size(12))
                    .style(style::menu_row(look))
                    .on_press(Message::Session(crate::session::Message::DismissMismatch)),
            ]
            .spacing(12)
            .boxed(),
        );
    }
    if let Some(release) = &app.session.config.release {
        notices = notices.push(
            button(text(format!("Version {} is available", release.version)).size(13))
                .style(style::menu_row(look))
                .on_press(Message::Session(crate::session::Message::OpenRelease))
                .boxed(),
        );
    }
    if !app.session.online {
        notices = notices.push(text("Reconnecting to live updates…").size(13).color(look.muted).boxed());
    }
    if let Some(error) = &app.session.error {
        notices = notices.push(text(error).size(13).color(look.danger).boxed());
    }
    let scroll = scrollable(column![
        container(notices).padding(Padding { top: 48.0, right: 40.0, bottom: 12.0, left: 40.0 }),
        content
    ])
    .id(SCROLL)
    .width(Fill)
    .height(Fill)
    .on_scroll(|scroll| Message::Scrolled(scroll.viewport.absolute_offset().y));

    let strip = column![app.chrome.drag_area(space().width(Fill).height(DRAG_STRIP), Message::Chrome)];

    stack![scroll, strip].width(Fill).height(Fill).boxed()
}

fn route_title(route: &Route) -> String {
    match route {
        Route::Home => "Home".into(),
        Route::Calendar => "Calendar".into(),
        Route::Clips => "Clips".into(),
        Route::Discover => "Add shows".into(),
        Route::Downloads => "Downloads".into(),
        Route::Requests => "Requests".into(),
        Route::Settings => "Settings".into(),
        Route::Wanted => "Wanted".into(),
        Route::Library(name) => name.clone(),
        Route::Series { series, .. } => series.clone(),
        Route::Title(id) => format!("Title {id}"),
        Route::Watch(id) => format!("Watching {id}"),
        Route::Artist(id) => format!("Artist {id}"),
        Route::Album(id) => format!("Album {id}"),
        Route::Playlist(id) => format!("Playlist {id}"),
        Route::Book(id) => format!("Book {id}"),
        Route::Read(id) => format!("Reading {id}"),
        Route::Together(code) => format!("Watch Together {code}"),
        Route::Listen(code) => format!("Listen Together {code}"),
    }
}

/// The command palette's pages matching `query`, best first.
pub fn commands(query: &str) -> Vec<(&'static str, Route)> {
    let needle = query.trim().to_lowercase();

    PAGES
        .iter()
        .filter(|(title, ..)| title.to_lowercase().contains(&needle))
        .map(|(title, route, _)| (*title, route.clone()))
        .collect()
}

fn overlay(app: &App, overlay: Overlay) -> Element<'_, Message> {
    let look = app.session.look();
    let (content, dim, at): (Element<'_, Message>, bool, Placement) = match overlay {
        Overlay::Search => (search(app), true, Placement::Palette),
        Overlay::Shortcuts => (shortcuts(look), true, Placement::Center),
        Overlay::Notifications => (notifications(look), false, Placement::TopLeft),
        Overlay::Account => (account_menu(app), false, Placement::BottomLeft),
    };

    let placed: Element<'_, Message> = match at {
        Placement::Palette => container(opaque(content))
            .center_x(Fill)
            .padding(Padding { top: 96.0, ..Padding::ZERO })
            .height(Fill)
            .boxed(),
        Placement::Center => container(opaque(content)).center(Fill).boxed(),
        Placement::TopLeft => {
            container(opaque(content)).padding(Padding { top: 52.0, left: 12.0, ..Padding::ZERO }).boxed()
        },
        Placement::BottomLeft => container(opaque(content))
            .align_bottom(Fill)
            .padding(Padding { bottom: 64.0, left: 12.0, ..Padding::ZERO })
            .boxed(),
    };

    let backdrop = mouse_area(
        container(space().width(Fill).height(Fill))
            .style(move |theme: &Theme| if dim { style::backdrop(look)(theme) } else { container::Style::default() }),
    )
    .on_press(Message::Overlay(None));

    stack![backdrop, placed].width(Fill).height(Fill).boxed()
}

enum Placement {
    Palette,
    Center,
    TopLeft,
    BottomLeft,
}

fn search(app: &App) -> Element<'_, Message> {
    let look = app.session.look();
    let input = text_input("Search, or jump anywhere", &app.query)
        .id(SEARCH)
        .on_input(Message::Query)
        .on_submit(Message::Submit)
        .size(15)
        .padding([14, 16])
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: border::rounded(0),
            placeholder: look.faint,
            value: look.ink,
            selection: look.line_strong,
        });

    let results = commands(&app.query);
    let list: Element<'_, Message> = if results.is_empty() {
        container(text(format!("Nothing called “{}”.", app.query.trim())).size(14).color(look.faint))
            .center_x(Fill)
            .padding(24)
            .boxed()
    } else {
        let rows = results.into_iter().map(|(title, route)| {
            let hint = PAGES.iter().find(|(t, ..)| *t == title).map(|(.., hint)| *hint).unwrap_or_default();
            button(
                row![text(title).size(14).width(Fill), text(hint).size(11).color(look.faint)]
                    .align_y(Alignment::Center),
            )
            .width(Fill)
            .padding([8, 12])
            .style(style::menu_row(look))
            .on_press(Message::Navigate(route))
            .boxed()
        });

        column![
            container(text("GO TO").size(10).font(MEDIUM).color(look.faint)).padding([6, 12]),
            Column::with_children(rows),
        ]
        .padding(6)
        .boxed()
    };

    container(column![input, container(space().width(Fill).height(1)).style(style::hairline(look)), list])
        .width(576)
        .style(style::dialog(look))
        .boxed()
}

fn shortcuts<'a>(look: style::Look) -> Element<'a, Message> {
    let groups = keys::SHEET.iter().map(|(group, keys)| {
        let rows = keys.iter().map(|(combo, what)| {
            let parts = combo.split_whitespace().map(|part| -> Element<'a, Message> {
                if part == "then" {
                    text("then").size(11).color(look.faint).boxed()
                } else {
                    container(text(part).size(11)).padding([2, 6]).style(style::kbd(look)).boxed()
                }
            });

            row![text(*what).size(14).color(look.muted).width(Fill), row(parts).spacing(4).align_y(Alignment::Center)]
                .align_y(Alignment::Center)
                .boxed()
        });

        column![text(*group).size(12).color(look.faint), Column::with_children(rows).spacing(6)]
            .spacing(8)
            .width(Fill)
            .boxed()
    });

    container(column![text("Keyboard shortcuts").size(15).font(MEDIUM), row(groups).spacing(32)].spacing(20))
        .width(672)
        .padding(24)
        .style(style::dialog(look))
        .boxed()
}

fn notifications<'a>(look: style::Look) -> Element<'a, Message> {
    container(text("Nothing new.").size(14).color(look.faint)).width(320).padding(16).style(style::dialog(look)).boxed()
}

fn account_menu<'a>(app: &'a App) -> Element<'a, Message> {
    let look = app.session.look();
    let item = |label: &'a str, hint: &'a str, message: Message| {
        button(row![text(label).size(13).width(Fill), text(hint).size(11).color(look.faint)].align_y(Alignment::Center))
            .width(Fill)
            .padding([6, 10])
            .style(style::menu_row(look))
            .on_press(message)
    };

    container(
        column![
            app.session.server_list(),
            item("Add server", "", Message::Session(crate::session::Message::Add)).boxed(),
            item("Settings", "G S", Message::Navigate(Route::Settings)).boxed(),
            item("Keyboard shortcuts", "?", Message::Overlay(Some(Overlay::Shortcuts))).boxed(),
        ]
        .spacing(2),
    )
    .width(Length::Fixed(224.0))
    .padding(6)
    .style(style::dialog(look))
    .boxed()
}
