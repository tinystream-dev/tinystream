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
    let route = &app.history.current().route;
    let focus = route.is_focus();

    let base: Element<'_, Message> = if focus {
        page(app, route)
    } else {
        row![sidebar(app, route), container(space().width(1).height(Fill)).style(style::hairline), page(app, route)]
            .boxed()
    };

    let mut layers = vec![container(base).style(style::canvas).width(Fill).height(Fill).boxed()];

    if let Some(pill) = app.chrome.pill(Side::Right) {
        layers.push(
            container(pill.map(Message::Chrome))
                .align_right(Fill)
                .padding(Padding { top: 8.0, right: 8.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    if focus && let Some(pill) = app.chrome.pill(Side::Left) {
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
    let logo = row![
        container(space().width(20).height(20)).style(|_: &Theme| container::Style {
            background: Some(Background::Color(style::INK)),
            border: border::rounded(6),
            ..container::Style::default()
        }),
        text("tinystream").size(15).font(SEMIBOLD),
        space().width(Fill),
        button(text("●").size(9).color(style::INK_3))
            .padding([6, 8])
            .style(style::menu_row)
            .on_press(Message::Overlay(Some(Overlay::Notifications))),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let logo_row: Element<'_, Message> = match app.chrome.controls(Side::Left) {
        Some(controls) => row![controls.map(Message::Chrome), logo].spacing(8).align_y(Alignment::Center).boxed(),
        None => container(logo).padding(Padding { left: 10.0, ..Padding::ZERO }).boxed(),
    };

    let search = button(
        row![text("⌕").size(14), text("Search").size(14).width(Fill), text("Ctrl K").size(11).color(style::INK_3),]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([7, 10])
    .style(style::search)
    .on_press(Message::Overlay(Some(Overlay::Search)));

    let nav = column![
        nav_item("Home", Route::Home, route),
        nav_item("Clips", Route::Clips, route),
        nav_item("Downloads", Route::Downloads, route),
    ]
    .spacing(2);

    let account = button(
        row![
            container(text("?").size(12).color(style::INK_2)).center(24).style(|_: &Theme| container::Style {
                background: Some(Background::Color(style::PANEL)),
                border: border::rounded(999),
                ..container::Style::default()
            }),
            text("Not signed in").size(14).color(style::INK_2).width(Fill),
            text("···").size(14).color(style::INK_3),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([8, 8])
    .style(style::menu_row)
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
    .style(style::sidebar)
    .boxed()
}

fn nav_item<'a>(label: &'a str, to: Route, current: &Route) -> Element<'a, Message> {
    let active = *current == to;

    button(text(label).size(14))
        .width(Fill)
        .padding([7, 10])
        .style(style::nav_item(active))
        .on_press(Message::Navigate(to))
        .boxed()
}

/// The page, in the window's single scroll container, under a strip that moves the window.
fn page<'a>(app: &'a App, route: &Route) -> Element<'a, Message> {
    let title = route_title(route);
    let path = route.to_string();

    let content = responsive(move |size| {
        let width = size.width.min(1400.0);
        let body = column![
            text(title.clone()).size(28).font(SEMIBOLD),
            text(path.clone()).size(14).color(style::INK_3),
            space().height(24),
            text("Nothing here yet.").size(14).color(style::INK_2),
        ]
        .spacing(6)
        .padding(40)
        .width(width);

        center_x(body).boxed()
    });

    let scroll = scrollable(content)
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
    let (content, dim, at): (Element<'_, Message>, bool, Placement) = match overlay {
        Overlay::Search => (search(app), true, Placement::Palette),
        Overlay::Shortcuts => (shortcuts(), true, Placement::Center),
        Overlay::Notifications => (notifications(), false, Placement::TopLeft),
        Overlay::Account => (account_menu(), false, Placement::BottomLeft),
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

    let backdrop = mouse_area(container(space().width(Fill).height(Fill)).style(if dim {
        style::backdrop
    } else {
        |_: &Theme| container::Style::default()
    }))
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
    let input = text_input("Search, or jump anywhere", &app.query)
        .id(SEARCH)
        .on_input(Message::Query)
        .on_submit(Message::Submit)
        .size(15)
        .padding([14, 16])
        .style(|_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: border::rounded(0),
            placeholder: style::INK_3,
            value: style::INK,
            selection: style::LINE_STRONG,
        });

    let results = commands(&app.query);
    let list: Element<'_, Message> = if results.is_empty() {
        container(text(format!("Nothing called “{}”.", app.query.trim())).size(14).color(style::INK_3))
            .center_x(Fill)
            .padding(24)
            .boxed()
    } else {
        let rows = results.into_iter().map(|(title, route)| {
            let hint = PAGES.iter().find(|(t, ..)| *t == title).map(|(.., hint)| *hint).unwrap_or_default();
            button(
                row![text(title).size(14).width(Fill), text(hint).size(11).color(style::INK_3)]
                    .align_y(Alignment::Center),
            )
            .width(Fill)
            .padding([8, 12])
            .style(style::menu_row)
            .on_press(Message::Navigate(route))
            .boxed()
        });

        column![
            container(text("GO TO").size(10).font(MEDIUM).color(style::INK_3)).padding([6, 12]),
            Column::with_children(rows),
        ]
        .padding(6)
        .boxed()
    };

    container(column![input, container(space().width(Fill).height(1)).style(style::hairline), list])
        .width(576)
        .style(style::dialog)
        .boxed()
}

fn shortcuts<'a>() -> Element<'a, Message> {
    let groups = keys::SHEET.iter().map(|(group, keys)| {
        let rows = keys.iter().map(|(combo, what)| {
            let parts = combo.split_whitespace().map(|part| -> Element<'a, Message> {
                if part == "then" {
                    text("then").size(11).color(style::INK_3).boxed()
                } else {
                    container(text(part).size(11)).padding([2, 6]).style(style::kbd).boxed()
                }
            });

            row![text(*what).size(14).color(style::INK_2).width(Fill), row(parts).spacing(4).align_y(Alignment::Center)]
                .align_y(Alignment::Center)
                .boxed()
        });

        column![text(*group).size(12).color(style::INK_3), Column::with_children(rows).spacing(6)]
            .spacing(8)
            .width(Fill)
            .boxed()
    });

    container(column![text("Keyboard shortcuts").size(15).font(MEDIUM), row(groups).spacing(32)].spacing(20))
        .width(672)
        .padding(24)
        .style(style::dialog)
        .boxed()
}

fn notifications<'a>() -> Element<'a, Message> {
    container(text("Nothing new.").size(14).color(style::INK_3)).width(320).padding(16).style(style::dialog).boxed()
}

fn account_menu<'a>() -> Element<'a, Message> {
    let item = |label: &'a str, hint: &'a str, message: Message| {
        button(
            row![text(label).size(13).width(Fill), text(hint).size(11).color(style::INK_3)].align_y(Alignment::Center),
        )
        .width(Fill)
        .padding([6, 10])
        .style(style::menu_row)
        .on_press(message)
    };

    container(
        column![
            item("Settings", "G S", Message::Navigate(Route::Settings)),
            item("Keyboard shortcuts", "?", Message::Overlay(Some(Overlay::Shortcuts))),
        ]
        .spacing(2),
    )
    .width(Length::Fixed(224.0))
    .padding(6)
    .style(style::dialog)
    .boxed()
}
