// SPDX-License-Identifier: AGPL-3.0-or-later

//! The web's desktop layout (`web/src/components/Shell.tsx`): a 240 px sidebar beside a single
//! scrolling page, with dialogs over both. Focus screens take the whole window.

use std::time::Instant;

use iced_core::Length::Fill;
use iced_core::{Alignment, Background, Border, Color, Padding, Theme};
use iced_widget::{
    Column, Element, Row, Widget, center_x, column, container, mouse_area, opaque, responsive, row, scrollable, space,
    stack, text_input,
};

use crate::app::{App, Message, Overlay, SCROLL};
use crate::chrome::{DRAG_STRIP, Side};
use crate::keys;
use crate::route::Route;
use crate::ui::Look;
use crate::ui::controls::{banner, divider, icon_button, menu_item, panel};
use crate::ui::effects::{Entrance, enter, page_transition, veil};
use crate::ui::icon::{Icon, icon};
use crate::ui::layout::{empty, page as page_column};
use crate::ui::logo::logo;
use crate::ui::media::avatar;
use crate::ui::pressable::{Appearance, Status, pressable};
use crate::ui::squircle::{Paint, squircle};
use crate::ui::text::{self as type_, MEDIUM, SEMIBOLD, inherit, text, truncate};

pub const SEARCH: &str = "search";

/// How wide the sidebar is (`w-60`).
pub const SIDEBAR_WIDTH: f32 = 240.0;

/// The pages the sidebar and the command palette can go to before anything is loaded from a server.
const PAGES: &[(&str, Route, &str, Icon)] = &[
    ("Home", Route::Home, "G H", Icon::House),
    ("Clips", Route::Clips, "G X", Icon::Scissors),
    ("Calendar", Route::Calendar, "G C", Icon::CalendarDays),
    ("Add shows", Route::Discover, "G A", Icon::Plus),
    ("Downloads", Route::Downloads, "G D", Icon::ArrowDownToLine),
    ("Wanted", Route::Wanted, "G W", Icon::ListTodo),
    ("Requests", Route::Requests, "G R", Icon::Inbox),
    ("Settings", Route::Settings, "G S", Icon::Settings),
];

pub fn view(app: &App) -> Element<'_, Message> {
    if let Some(gallery) = &app.gallery {
        let look = gallery.look(app.reduce_motion);
        return chrome(app, gallery.view(&look).map(Message::Gallery).boxed(), true, &look);
    }
    let look = app.session.look();
    let route = &app.history.current().route;
    let signed_in = app.session.screen == crate::session::Screen::Ready;
    let focus = route.is_focus() || !signed_in;

    let base: Element<'_, Message> = if !signed_in {
        app.session.view()
    } else if focus {
        page(app, &look)
    } else {
        let line = look.line;
        row![
            sidebar(app, route, &look),
            container(space().width(1).height(Fill))
                .style(move |_| container::Style { background: Some(line.into()), ..container::Style::default() }),
            page(app, &look)
        ]
        .boxed()
    };
    let base = chrome(app, base, focus, &look);
    if signed_in
        && let Some(server) = app.session.removing.and_then(|id| app.session.config.servers.iter().find(|s| s.id == id))
    {
        return crate::ui::modal::modal(
            base,
            app.session.confirm_remove(server, &look),
            Message::Session(crate::session::Message::AskRemove(None)),
        );
    }
    base
}

/// Everything around the screen: the canvas under it, the window's controls and resize edges
/// over it, and whatever overlay is open.
fn chrome<'a>(app: &'a App, base: Element<'a, Message>, focus: bool, look: &Look) -> Element<'a, Message> {
    let canvas = look.canvas;
    let mut layers = vec![
        container(base)
            .style(move |_| container::Style { background: Some(canvas.into()), ..container::Style::default() })
            .width(Fill)
            .height(Fill)
            .boxed(),
    ];
    if focus {
        layers.push(app.chrome.drag_area(space().width(Fill).height(DRAG_STRIP), Message::Chrome));
    }
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
    layers.push(app.chrome.edges().map(Message::Chrome).boxed());
    let base = iced_widget::Stack::with_children(layers).width(Fill).height(Fill).boxed();
    if app.gallery.is_none()
        && let Some(overlay) = app.overlay
    {
        return crate::ui::modal::modal(base, self::overlay(app, overlay, look), Message::Overlay(None));
    }
    base
}

fn sidebar<'a>(app: &'a App, route: &Route, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let brand = row![
        logo(20.0, look),
        text("tinystream", type_::S15, look.ink).font(SEMIBOLD).width(Fill),
        icon_button(Icon::Bell, "Notifications", Some(Message::Overlay(Some(Overlay::Notifications))), look),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let logo_row: Element<'_, Message> = match app.chrome.controls(Side::Left, look) {
        Some(controls) => row![controls.map(Message::Chrome), brand].spacing(8).align_y(Alignment::Center).boxed(),
        None => container(brand).padding(Padding { left: 10.0, ..Padding::ZERO }).boxed(),
    };

    let search = pressable(
        row![
            icon(Icon::Search, 16.0),
            inherit("Search", type_::SM).width(Fill),
            text("Ctrl K", type_::XS2, look.ink_3),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        9.0,
        look,
        move |status| Appearance {
            paint: Paint { fill: Some(l.panel), edge: true, ..Paint::NONE },
            ink: if matches!(status, Status::Hovered | Status::Pressed) { l.ink_2 } else { l.ink_3 },
        },
    )
    .width(Fill)
    .height(32)
    .padding([0, 10])
    .on_press(Message::Overlay(Some(Overlay::Search)));

    let nav = column![
        nav_link("Home", Icon::House, Route::Home, route, look),
        nav_link("Clips", Icon::Scissors, Route::Clips, route, look),
        nav_link("Downloads", Icon::ArrowDownToLine, Route::Downloads, route, look),
    ]
    .spacing(2);

    let user = app.session.config.active().and_then(|server| server.user.as_ref());
    let name = user.map_or("Signed out", |u| u.username.as_str());
    let picture = user.and_then(|u| u.avatar.as_deref()).and_then(|path| app.session.images.get(path));
    let open = app.overlay == Some(Overlay::Account);
    let account = pressable(
        row![
            avatar(name, picture, 24.0, None, look),
            truncate(name, type_::SM, look.ink_2).width(Fill),
            icon(Icon::Ellipsis, 16.0).color(look.ink_3),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        10.0,
        look,
        move |status| Appearance {
            paint: Paint {
                fill: (open || matches!(status, Status::Hovered | Status::Pressed)).then_some(l.hover),
                ..Paint::NONE
            },
            ink: l.ink_2,
        },
    )
    .width(Fill)
    .height(40)
    .padding([0, 8])
    .on_press(Message::Overlay(if open { None } else { Some(Overlay::Account) }));

    let raised = look.raised.scale_alpha(0.6);
    container(
        column![
            app.chrome.drag_area(container(logo_row).height(36).align_y(Alignment::Center), Message::Chrome),
            container(search).padding(Padding { top: 6.0, bottom: 12.0, ..Padding::ZERO }),
            nav,
            space().height(Fill),
            account,
        ]
        .height(Fill),
    )
    .width(SIDEBAR_WIDTH)
    .height(Fill)
    .padding([12, 10])
    .style(move |_| container::Style { background: Some(raised.into()), ..container::Style::default() })
    .boxed()
}

/// `NavLink`: a page in the sidebar.
fn nav_link<'a>(label: &'a str, glyph: Icon, to: Route, current: &Route, look: &Look) -> Element<'a, Message> {
    let active = *current == to;
    let l = *look;
    pressable(
        row![icon(glyph, 16.0), inherit(label, type_::SM).width(Fill)].spacing(10).align_y(Alignment::Center),
        8.0,
        look,
        move |status| {
            let hovered = matches!(status, Status::Hovered | Status::Pressed);
            Appearance {
                paint: Paint { fill: if active { Some(l.press) } else { hovered.then_some(l.hover) }, ..Paint::NONE },
                ink: if active || hovered { l.ink } else { l.ink_2 },
            }
        },
    )
    .width(Fill)
    .height(32)
    .padding([0, 10])
    .on_press(Message::Navigate(to))
    .boxed()
}

/// The page, in the window's single scroll container, under a strip that moves the window. While
/// the route changes, the old page blurs away and the new one sharpens in.
fn page<'a>(app: &'a App, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let transition = app.transition.as_ref().filter(|_| !look.reduce_motion).and_then(|(start, from)| {
        page_transition(Instant::now().saturating_duration_since(*start)).map(|(arrived, veil)| (arrived, veil, from))
    });
    let route = match transition {
        Some((false, _, from)) => from,
        _ => &app.history.current().route,
    };
    let content: Element<'a, Message> = if *route == Route::Settings {
        app.session.settings(look)
    } else {
        let title = route_title(route);
        responsive(move |_| {
            page_column(
                column![
                    page_title_owned(title.clone(), &l),
                    empty("Nothing here yet", Some("This part of the app is on its way."), &l)
                ],
                false,
            )
        })
        .boxed()
    };

    let mut notices = Column::new().spacing(8);
    let any = app.session.mismatch().is_some()
        || app.session.config.release.is_some()
        || !app.session.online
        || app.session.error.is_some();
    if let Some(version) = app.session.mismatch() {
        notices = notices.push(banner(
            Icon::TriangleAlert,
            look.warn,
            text(
                format!("This app is {}, your server is {version}. Some things may not work.", crate::updates::VERSION),
                type_::SM,
                look.ink,
            ),
            Some(Message::Session(crate::session::Message::DismissMismatch)),
            look,
        ));
    }
    if let Some(release) = &app.session.config.release {
        notices = notices.push(banner(
            Icon::Sparkles,
            look.info,
            row![
                text(format!("Version {} of tinystream is out.", release.version), type_::SM, look.ink).width(Fill),
                crate::ui::controls::button("See what's new", look)
                    .small()
                    .on_press(Message::Session(crate::session::Message::OpenRelease))
                    .boxed(),
            ]
            .align_y(Alignment::Center),
            None,
            look,
        ));
    }
    if !app.session.online {
        notices = notices.push(
            row![
                crate::ui::controls::spinner(14.0, look),
                text("Reconnecting to live updates…", type_::S13, look.ink_2)
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    if let Some(error) = &app.session.error {
        notices =
            notices.push(banner(Icon::CircleAlert, look.danger, text(error.as_str(), type_::SM, look.ink), None, look));
    }
    let notices: Element<'a, Message> = if !any {
        space().boxed()
    } else {
        center_x(container(notices).width(Fill.max(1400)).padding(Padding {
            top: 40.0,
            right: 40.0,
            bottom: 0.0,
            left: 40.0,
        }))
        .boxed()
    };

    let scroll = scrollable(column![notices, content])
        .id(SCROLL)
        .direction(iced_widget::scrollable::Direction::Vertical(iced_widget::scrollable::Scrollbar::hidden()))
        .width(Fill)
        .height(Fill)
        .on_scroll(|scroll| Message::Scrolled(scroll.viewport.absolute_offset().y));

    let strip = column![app.chrome.drag_area(space().width(Fill).height(DRAG_STRIP), Message::Chrome)];
    let mut layers = vec![scroll.boxed(), strip.boxed()];
    if let Some((_, veiled, _)) = transition {
        layers.push(veil(veiled, look));
    }
    iced_widget::Stack::with_children(layers).width(Fill).height(Fill).boxed()
}

fn page_title_owned<'a>(title: String, look: &Look) -> Element<'a, Message> {
    container(type_::truncate(title, type_::TITLE, look.ink).font(SEMIBOLD))
        .padding(Padding { bottom: 32.0, ..Padding::ZERO })
        .boxed()
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
        .map(|(title, route, ..)| (*title, route.clone()))
        .collect()
}

fn overlay<'a>(app: &'a App, overlay: Overlay, look: &Look) -> Element<'a, Message> {
    let (content, dim, at): (Element<'_, Message>, bool, Placement) = match overlay {
        Overlay::Credits => (crate::ui::credits::credits(Message::Overlay(None), look), true, Placement::Dialog),
        Overlay::Search => (search(app, look), true, Placement::Palette),
        Overlay::Shortcuts => (shortcuts(look), true, Placement::Dialog),
        Overlay::Notifications => (notifications(look), false, Placement::TopLeft),
        Overlay::Account => (account_menu(app, look), false, Placement::BottomLeft),
    };
    let content = enter(content, Entrance::pop(look), look);

    let placed: Element<'_, Message> = match at {
        Placement::Palette => container(opaque(content))
            .center_x(Fill)
            .padding(Padding { top: 96.0, left: 16.0, right: 16.0, ..Padding::ZERO })
            .height(Fill)
            .boxed(),
        Placement::Dialog => container(opaque(content))
            .center_x(Fill)
            .padding(Padding { top: 64.0, left: 16.0, right: 16.0, ..Padding::ZERO })
            .height(Fill)
            .boxed(),
        Placement::TopLeft => {
            container(opaque(content)).padding(Padding { top: 52.0, left: 12.0, ..Padding::ZERO }).boxed()
        },
        Placement::BottomLeft => container(opaque(content))
            .align_bottom(Fill)
            .padding(Padding { bottom: 60.0, left: 10.0, ..Padding::ZERO })
            .boxed(),
    };

    let scrim = look.scrim();
    let backdrop = mouse_area(container(space().width(Fill).height(Fill)).style(move |_: &Theme| {
        if dim {
            container::Style { background: Some(scrim.into()), ..container::Style::default() }
        } else {
            container::Style::default()
        }
    }))
    .on_press(Message::Overlay(None));

    stack![backdrop, placed].width(Fill).height(Fill).boxed()
}

enum Placement {
    Palette,
    Dialog,
    TopLeft,
    BottomLeft,
}

/// `CommandPalette`.
fn search<'a>(app: &'a App, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let input = text_input("Search, or jump anywhere", &app.query)
        .id(SEARCH)
        .on_input(Message::Query)
        .on_submit(Message::Submit)
        .size(15)
        .padding([16, 0])
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: l.ink_3,
            value: l.ink,
            selection: l.selection,
        });
    let field = row![icon(Icon::Search, 18.0).color(look.ink_3), input, text("Esc", type_::XS2, look.ink_3)]
        .spacing(12)
        .align_y(Alignment::Center)
        .padding([0, 16]);

    let results = commands(&app.query);
    let list: Element<'_, Message> = if results.is_empty() {
        container(text(format!("Nothing called “{}”.", app.query.trim()), type_::SM, look.ink_3))
            .center_x(Fill)
            .padding([24, 12])
            .boxed()
    } else {
        let rows = results.into_iter().enumerate().map(|(i, (title, route))| {
            let (hint, glyph) = PAGES
                .iter()
                .find(|(t, ..)| *t == title)
                .map(|(.., hint, glyph)| (*hint, *glyph))
                .unwrap_or(("", Icon::House));
            let first = i == 0;
            pressable(
                row![
                    container(icon(glyph, 16.0).color(l.ink_3)).center_x(24),
                    inherit(title, type_::SM).width(Fill),
                    text(hint, type_::XS, l.ink_3),
                    if first {
                        icon(Icon::CornerDownLeft, 14.0).color(l.ink_3).boxed()
                    } else {
                        space().width(14).boxed()
                    },
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                10.0,
                look,
                move |status| Appearance {
                    paint: Paint {
                        fill: (first || matches!(status, Status::Hovered | Status::Pressed)).then_some(l.hover),
                        ..Paint::NONE
                    },
                    ink: l.ink,
                },
            )
            .width(Fill)
            .padding([8, 12])
            .on_press(Message::Navigate(route))
            .boxed()
        });

        column![
            container(text("GO TO", type_::XS2, look.ink_3).font(MEDIUM)).padding(Padding {
                top: 10.0,
                right: 12.0,
                bottom: 4.0,
                left: 12.0
            }),
            Column::with_children(rows),
        ]
        .padding(6)
        .boxed()
    };

    container(
        squircle(column![field, divider(look), list], 18.0, look)
            .paint(Paint { fill: Some(look.float), material: look.material(), edge: true, lift: true, ..Paint::NONE })
            .width(Fill),
    )
    .width(Fill.max(576))
    .boxed()
}

/// `Shortcuts`.
fn shortcuts<'a>(look: &Look) -> Element<'a, Message> {
    let l = *look;
    let groups = keys::SHEET.iter().map(|(group, keys)| {
        let rows = keys.iter().map(|(combo, what)| {
            let parts = combo.split_whitespace().map(|part| -> Element<'a, Message> {
                if part == "then" {
                    text("then", type_::XS2, l.ink_3).boxed()
                } else {
                    container(text(part, type_::XS2, l.ink))
                        .padding([2, 6])
                        .style(move |_| container::Style {
                            background: Some(l.panel.into()),
                            border: Border {
                                color: l.line_strong,
                                width: 1.0,
                                radius: l.rounded(crate::ui::theme::Rounded::Md).into(),
                            },
                            ..container::Style::default()
                        })
                        .boxed()
                }
            });
            row![
                text(*what, type_::SM, l.ink_2).width(Fill),
                Row::with_children(parts).spacing(4).align_y(Alignment::Center)
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .boxed()
        });
        column![text(*group, type_::XS, l.ink_3), Column::with_children(rows).spacing(6)].spacing(8).width(Fill).boxed()
    });

    container(
        panel(
            column![
                text("Keyboard shortcuts", type_::S15, look.ink).font(MEDIUM),
                Row::with_children(groups).spacing(32)
            ]
            .spacing(20),
            20.0,
            look,
        )
        .padding(20)
        .width(Fill),
    )
    .width(Fill.max(672))
    .boxed()
}

fn notifications<'a>(look: &Look) -> Element<'a, Message> {
    panel(
        column![
            row![text("Notifications", type_::SM, look.ink).font(MEDIUM)].padding([4, 4]),
            empty("Nothing new", Some("What happens on your server shows up here."), look),
        ],
        14.0,
        look,
    )
    .padding(12)
    .width(320)
    .boxed()
}

/// `AccountMenu`, with the server list from the mobile app's switcher at the top.
fn account_menu<'a>(app: &'a App, look: &Look) -> Element<'a, Message> {
    let item = |glyph: Icon, label: &'a str, hint: Option<&'a str>, message: Message| {
        menu_item(Some(glyph), inherit(label, type_::S13), hint, false, Some(message), look)
    };
    let mut menu = Column::new().spacing(0).width(Fill);
    menu = menu.push(
        container(text("Servers", type_::XS, look.ink_3))
            .padding(Padding { top: 4.0, right: 10.0, bottom: 4.0, left: 10.0 })
            .boxed(),
    );
    menu = menu.push(app.session.server_list(28.0, look));
    menu = menu.push(item(Icon::Plus, "Add server", None, Message::Session(crate::session::Message::Add)));
    menu = menu.push(container(divider(look)).padding([4, 0]).boxed());
    menu = menu.push(item(Icon::Settings, "Settings", Some("G S"), Message::Navigate(Route::Settings)));
    menu = menu.push(item(Icon::Keyboard, "Keyboard shortcuts", Some("?"), Message::Overlay(Some(Overlay::Shortcuts))));
    menu = menu.push(container(divider(look)).padding([4, 0]).boxed());
    if let Some(id) = app.session.config.active {
        menu = menu.push(item(Icon::LogOut, "Sign out", None, Message::Session(crate::session::Message::SignOut(id))));
    }
    panel(menu, 14.0, look).padding(6).width(360).boxed()
}
