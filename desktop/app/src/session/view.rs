// SPDX-License-Identifier: AGPL-3.0-or-later

//! The signed-out screens, after the web's `Login.tsx`, `invite.tsx` and `reset.tsx`, with the
//! server list and connect screen of the mobile app (`connect.tsx`, `Switcher.tsx`).

use iced_core::{Alignment, Length, Padding};
use iced_widget::{Column, Element, Row, Widget as _, column, container, responsive, row, scrollable, space};

use super::{Message, Screen, Session, profile_avatar};
use crate::app::Message as AppMessage;
use crate::servers::Server;
use crate::ui::controls::{banner, button, field, icon_button, input, panel, segmented, spinner, toggle};
use crate::ui::icon::{Icon, icon};
use crate::ui::layout::{page, page_title};
use crate::ui::logo::logo;
use crate::ui::media::avatar;
use crate::ui::overlay::confirm;
use crate::ui::pressable::{Appearance, Status, pressable};
use crate::ui::squircle::{Paint, squircle};
use crate::ui::text::{self as type_, MEDIUM, SEMIBOLD, inherit, text, truncate};
use crate::ui::{Look, Style};
use crate::{api, updates};

fn session(message: Message) -> AppMessage {
    AppMessage::Session(message)
}

/// What a frame says at the top: the title, and what's under it.
enum Subtitle<'a> {
    None,
    Text(&'a str),
    /// The server's address, which opens the server list.
    Server(String),
}

impl Session {
    /// The server's appearance (the signed-in person's own once they're in), or Grey before
    /// there's a server.
    pub fn look(&self) -> Look {
        let mut look = match &self.connection {
            Some(data) => {
                let appearance = &data.appearance;
                let dark = self.system_dark && matches!(appearance.mode, api::connection::SchemeMode::SYSTEM);
                let tokens: Vec<(&str, &str)> = if dark {
                    appearance.dark.palette.tokens.iter().map(|t| (t.name.as_str(), t.value.as_str())).collect()
                } else {
                    appearance.light.palette.tokens.iter().map(|t| (t.name.as_str(), t.value.as_str())).collect()
                };
                let style = match appearance.style {
                    api::connection::ComponentStyle::FLAT => Style::Flat,
                    api::connection::ComponentStyle::GLASS => Style::Glass,
                    _ => Style::Layered,
                };
                Look::from_palette(tokens, style, appearance.media_tint)
            },
            None => Look::default(),
        };
        look.reduce_motion = self.config.reduce_motion.unwrap_or(self.reduce_motion);
        look
    }

    pub fn view(&self) -> Element<'_, AppMessage> {
        let look = self.look();
        let base = responsive(move |size| self.signed_out(size, &look)).boxed();
        if let Some(server) = self.removing.and_then(|id| self.config.servers.iter().find(|s| s.id == id)) {
            return crate::ui::modal::modal(
                base,
                self.confirm_remove(server, &look),
                session(Message::AskRemove(None)),
            );
        }
        base
    }

    fn signed_out(&self, size: iced_core::Size, look: &Look) -> Element<'_, AppMessage> {
        let l = look;
        let host = self.api.as_ref().map(|api| host(api.origin.as_str()));
        let server = || host.clone().map_or(Subtitle::None, Subtitle::Server);
        let setup = self.connection.as_ref().is_some_and(|c| c.server.setup_required);
        let picker = self.screen == Screen::Login && self.show_profiles() && self.profile.is_none();

        if self.store.is_none() && self.busy || self.screen == Screen::Login && self.connection.is_none() && self.busy {
            return container(spinner(20.0, l)).center(Length::Fill).style(|_| container::Style::default()).boxed();
        }

        let (title, subtitle, body, footer): (
            Option<&str>,
            Subtitle,
            Element<'_, AppMessage>,
            Option<Element<'_, AppMessage>>,
        ) = match self.screen {
            Screen::Connect => (
                Some(if self.config.servers.is_empty() { "Welcome to tinystream" } else { "Add a server" }),
                Subtitle::None,
                self.connect(l),
                self.config.active.map(|_| back("Back", Message::Cancel, !self.busy, l)),
            ),
            Screen::Login if setup => (
                Some("Welcome to tinystream"),
                Subtitle::Text("Create the admin account."),
                self.username_form(true, l),
                None,
            ),
            Screen::Login if picker => (
                Some("Who’s watching?"),
                server(),
                self.picker(l),
                Some(
                    container(
                        button("Sign in with a username instead", l).plain().on_press(session(Message::ByName)).boxed(),
                    )
                    .center_x(Length::Fill)
                    .boxed(),
                ),
            ),
            Screen::Login if self.profile.is_some() => (None, Subtitle::None, self.unlock(l), None),
            Screen::Login => (
                Some("Sign in"),
                server(),
                self.username_form(false, l),
                self.has_profiles().then(|| back("Back to pictures", Message::Login, !self.busy, l)),
            ),
            Screen::Invite => (
                Some(if self.link_valid {
                    "Join tinystream"
                } else if self.busy {
                    "Checking your invite…"
                } else {
                    "This invite doesn’t work anymore"
                }),
                Subtitle::Text(if self.link_valid || self.busy {
                    "Pick a username and password."
                } else {
                    "Ask whoever sent it for a new one."
                }),
                if self.link_valid { self.invite(l) } else { space().boxed() },
                Some(back("Sign in instead", Message::Login, !self.busy, l)),
            ),
            Screen::Reset if self.notice.is_some() => (
                Some("Check your email"),
                Subtitle::Text("If that account has an email address, a link is on its way. It works for an hour."),
                space().boxed(),
                Some(back("Back to sign in", Message::Login, true, l)),
            ),
            Screen::Reset => (
                Some("Forgot your password?"),
                Subtitle::Text("We’ll email you a link to pick a new one."),
                self.reset_form(l),
                Some(back("Back to sign in", Message::Login, !self.busy, l)),
            ),
            Screen::NewPassword => (
                Some(if self.link_valid || self.busy {
                    "Pick a new password"
                } else {
                    "This link doesn’t work anymore"
                }),
                Subtitle::Text(match &self.notice {
                    Some(notice) if self.link_valid => notice.as_str(),
                    _ if self.busy => "Checking your link…",
                    _ => "Links work once, for an hour.",
                }),
                self.new_password(l),
                Some(back("Back to sign in", Message::Login, !self.busy, l)),
            ),
            Screen::Servers => (
                Some("Servers"),
                Subtitle::None,
                self.servers(l),
                self.config.active.map(|_| back("Back", Message::Cancel, !self.busy, l)),
            ),
            Screen::Link => (
                Some("Open a link"),
                Subtitle::Text("Paste an invite or a password reset link."),
                self.link(l),
                Some(back("Back", Message::Cancel, !self.busy, l)),
            ),
            Screen::Ready => (None, Subtitle::None, space().boxed(), None),
        };

        let wide = picker;
        let mut frame = Column::new().width(Length::Fill.max(if wide { 768 } else { 384 }));
        if let Some(title) = title {
            frame = frame.push(header(title, subtitle, wide, l));
        }
        if let Some(version) = self.mismatch() {
            frame = frame.push(
                container(self.mismatch_banner(version, l)).padding(Padding { bottom: 20.0, ..Padding::ZERO }).boxed(),
            );
        }
        frame = frame.push(body);
        if let Some(footer) = footer {
            frame = frame.push(container(footer).padding(Padding { top: 20.0, ..Padding::ZERO }).boxed());
        }
        scrollable(
            container(frame).padding([48, 20]).center_x(Length::Fill).center_y(Length::Fixed(size.height.max(560.0))),
        )
        .direction(iced_widget::scrollable::Direction::Vertical(iced_widget::scrollable::Scrollbar::hidden()))
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    fn connect(&self, l: &Look) -> Element<'_, AppMessage> {
        if self.store.is_none() {
            return panel(
                column![
                    error(self.error.as_deref().unwrap_or("Couldn't open your saved servers."), l),
                    button("Try again", l).primary().large().fill().on_press(session(Message::Start)).boxed(),
                ]
                .spacing(16),
                20.0,
                l,
            )
            .padding(20)
            .width(Length::Fill)
            .boxed();
        }
        let address = self.address.trim();
        let mut form = vec![field(
            "Server address",
            input("auth-address", &self.address, "tinystream.lan:3000", |v| session(Message::Address(v)), l)
                .on_submit(session(Message::Submit))
                .boxed(),
            Some("Like tinystream.lan:3000 or https://tv.example.com"),
            l,
        )];
        if address.starts_with("http://") {
            form.push(unencrypted(l));
        }
        if let Some(message) = &self.error {
            form.push(error(message, l));
        }
        form.push(primary(
            if self.busy { None } else { Some("Connect") },
            "Connecting…",
            !address.is_empty(),
            Message::Submit,
            self.busy,
            l,
        ));
        form_panel(form, l)
    }

    fn picker(&self, l: &Look) -> Element<'_, AppMessage> {
        let profiles = self.connection.iter().flat_map(|c| &c.sign_in_profiles).map(|profile| {
            let picture = self.images.get(&profile_avatar(&profile.key));
            let face = avatar(&profile.key, picture, 104.0, Some(person(104.0, l)), l);
            let ink = l.ink;
            pressable(crate::ui::effects::lift_on_hover(face, -4.0, 1.05, l), 52.0, l, move |_| Appearance {
                paint: Paint::NONE,
                ink,
            })
            .round()
            .on_press_maybe((!self.busy).then(|| session(Message::Profile(profile.key.clone()))))
            .boxed()
        });
        container(Row::with_children(profiles).spacing(28).wrap()).center_x(Length::Fill).padding([16, 0]).boxed()
    }

    fn unlock(&self, l: &Look) -> Element<'_, AppMessage> {
        let key = self.profile.as_deref().unwrap_or_default();
        let picture = self.images.get(&profile_avatar(key));
        let mut form = vec![field(
            "Password",
            input("auth-password", &self.password, "", |v| session(Message::Password(v)), l)
                .secure()
                .on_submit(session(Message::Submit))
                .boxed(),
            None,
            l,
        )];
        if self.http() {
            form.push(unencrypted(l));
        }
        if let Some(message) = &self.error {
            form.push(error(message, l));
        }
        form.push(primary(Some("Sign in"), "Signing in…", !self.password.is_empty(), Message::Submit, self.busy, l));
        if let Some(forgot) = self.forgot(l) {
            form.push(forgot);
        }
        column![
            container(avatar(key, picture, 128.0, Some(person(128.0, l)), l)).center_x(Length::Fill),
            container(
                button("Not you?", l)
                    .plain()
                    .small()
                    .icon(Icon::ArrowLeft)
                    .on_press_maybe((!self.busy).then(|| session(Message::Login)))
                    .boxed()
            )
            .center_x(Length::Fill)
            .padding(Padding { top: 12.0, bottom: 24.0, ..Padding::ZERO }),
            form_panel(form, l),
        ]
        .boxed()
    }

    fn username_form(&self, setup: bool, l: &Look) -> Element<'_, AppMessage> {
        let mut form = vec![
            field(
                "Username",
                input("auth-username", &self.username, "", |v| session(Message::Username(v)), l)
                    .on_submit(session(Message::Submit))
                    .boxed(),
                None,
                l,
            ),
            field(
                "Password",
                input("auth-password", &self.password, "", |v| session(Message::Password(v)), l)
                    .secure()
                    .on_submit(session(Message::Submit))
                    .boxed(),
                None,
                l,
            ),
        ];
        if self.http() {
            form.push(unencrypted(l));
        }
        if let Some(message) = &self.error {
            form.push(error(message, l));
        }
        let ready = !self.username.trim().is_empty() && !self.password.is_empty();
        form.push(primary(
            Some(if setup { "Create account" } else { "Sign in" }),
            "Signing in…",
            ready,
            Message::Submit,
            self.busy,
            l,
        ));
        if !setup && let Some(forgot) = self.forgot(l) {
            form.push(forgot);
        }
        form_panel(form, l)
    }

    fn invite(&self, l: &Look) -> Element<'_, AppMessage> {
        let mut form = vec![
            field(
                "Username",
                input("auth-username", &self.username, "", |v| session(Message::Username(v)), l)
                    .on_submit(session(Message::Submit))
                    .boxed(),
                None,
                l,
            ),
            field(
                "Password",
                input("auth-password", &self.password, "", |v| session(Message::Password(v)), l)
                    .secure()
                    .on_submit(session(Message::Submit))
                    .boxed(),
                None,
                l,
            ),
        ];
        if let Some(message) = &self.error {
            form.push(error(message, l));
        }
        let ready = !self.username.trim().is_empty() && !self.password.is_empty();
        form.push(primary(Some("Create account"), "Creating your account…", ready, Message::Submit, self.busy, l));
        form_panel(form, l)
    }

    fn reset_form(&self, l: &Look) -> Element<'_, AppMessage> {
        let mut form = vec![field(
            "Username or email",
            input("auth-username", &self.username, "", |v| session(Message::Username(v)), l)
                .on_submit(session(Message::Submit))
                .boxed(),
            None,
            l,
        )];
        if let Some(message) = &self.error {
            form.push(error(message, l));
        }
        form.push(primary(
            Some("Send link"),
            "Sending…",
            !self.username.trim().is_empty(),
            Message::Submit,
            self.busy,
            l,
        ));
        form_panel(form, l)
    }

    fn new_password(&self, l: &Look) -> Element<'_, AppMessage> {
        if !self.link_valid {
            if self.busy {
                return container(spinner(20.0, l)).center_x(Length::Fill).padding(20).boxed();
            }
            return button("Send a new link", l).primary().large().fill().on_press(session(Message::Reset)).boxed();
        }
        let mut form = vec![field(
            "New password",
            input("auth-password", &self.password, "", |v| session(Message::Password(v)), l)
                .secure()
                .on_submit(session(Message::Submit))
                .boxed(),
            None,
            l,
        )];
        if let Some(message) = &self.error {
            form.push(error(message, l));
        }
        form.push(primary(
            Some("Change password"),
            "Changing it…",
            !self.password.is_empty(),
            Message::Submit,
            self.busy,
            l,
        ));
        form_panel(form, l)
    }

    fn link(&self, l: &Look) -> Element<'_, AppMessage> {
        let mut form = vec![field(
            "Link",
            input(
                "auth-address",
                &self.address,
                "https://tv.example.com/invite#…",
                |v| session(Message::Address(v)),
                l,
            )
            .on_submit(session(Message::Submit))
            .boxed(),
            None,
            l,
        )];
        if let Some(message) = &self.error {
            form.push(error(message, l));
        }
        form.push(primary(Some("Open"), "Opening…", !self.address.trim().is_empty(), Message::Submit, self.busy, l));
        form_panel(form, l)
    }

    /// The server list: each server in its own row, the one in use ticked.
    fn servers(&self, l: &Look) -> Element<'_, AppMessage> {
        column![
            self.server_list(44.0, l),
            container(
                button("Add server", l)
                    .large()
                    .fill()
                    .icon(Icon::Plus)
                    .on_press_maybe((!self.busy).then(|| session(Message::Add)))
                    .boxed()
            )
            .padding(Padding { top: 16.0, ..Padding::ZERO }),
            container(
                button("Open an invite or reset link", l)
                    .fill()
                    .icon(Icon::Link)
                    .on_press_maybe((!self.busy).then(|| session(Message::EnterLink)))
                    .boxed()
            )
            .padding(Padding { top: 8.0, ..Padding::ZERO }),
        ]
        .boxed()
    }

    /// The saved servers as rows (the mobile `Switcher`'s), with sign out and remove on hover.
    pub fn server_list(&self, size: f32, l: &Look) -> Element<'_, AppMessage> {
        Column::with_children(self.config.servers.iter().map(|server| self.server_row(server, size, l)))
            .spacing(4)
            .boxed()
    }

    fn server_row<'a>(&'a self, server: &'a Server, size: f32, l: &Look) -> Element<'a, AppMessage> {
        let active = self.config.active == Some(server.id);
        let signed_in = server.user.is_some();
        let name = server.user.as_ref().map_or(server.name.as_str(), |u| u.username.as_str());
        let picture = server
            .user
            .as_ref()
            .and_then(|u| u.avatar.as_deref())
            .filter(|_| active)
            .and_then(|path| self.images.get(path));
        let face = avatar(name, picture, size, None, l);
        let status = match &server.user {
            Some(user) => format!("{} · signed in", user.username),
            None => "Signed out".to_owned(),
        };
        let action_space = if signed_in { 76.0 } else { 42.0 } + if active { 30.0 } else { 0.0 };
        let mut details = column![
            truncate(host(&server.url), if size > 32.0 { type_::S15 } else { type_::S13 }, l.ink).font(MEDIUM),
            truncate(
                status,
                if size > 32.0 { type_::S13 } else { type_::XS },
                if signed_in { l.ink_2 } else { l.ink_3 }
            ),
        ];
        if size > 32.0 {
            details = details.push(
                row![
                    truncate(server.url.as_str(), type_::XS, l.ink_3),
                    if server.url.starts_with("http://") {
                        container(icon(Icon::ShieldOff, 12.0).color(l.warn))
                            .padding(Padding { left: 6.0, top: 2.0, ..Padding::ZERO })
                            .boxed()
                    } else {
                        space().boxed()
                    },
                ]
                .boxed(),
            );
        }
        let mut line = Row::new().spacing(12).align_y(Alignment::Center).push(face).push(
            container(details).width(Length::Fill).padding(Padding { right: action_space, ..Padding::ZERO }).boxed(),
        );
        if active {
            line = line.push(icon(Icon::Check, 18.0).color(l.ink).boxed());
        }
        let look = *l;
        let switch = (!active || !signed_in).then(|| session(Message::Switch(server.id))).filter(|_| !self.busy);
        let base = pressable(line, 16.0, l, move |status| Appearance {
            paint: Paint {
                fill: Some(match status {
                    Status::Hovered => look.panel,
                    Status::Pressed => look.press,
                    _ => look.raised,
                }),
                edge: true,
                ..Paint::NONE
            },
            ink: look.ink,
        })
        .id(format!("server-{}", server.id))
        .width(Length::Fill)
        .padding([10, 10])
        .on_press_maybe(switch.or_else(|| (!self.busy).then(|| AppMessage::Overlay(None))));
        let mut actions = Row::new().spacing(2).align_y(Alignment::Center);
        if signed_in {
            actions = actions.push(icon_button(
                Icon::LogOut,
                "Sign out",
                (!self.busy).then(|| session(Message::SignOut(server.id))),
                l,
            ));
        }
        actions = actions.push(icon_button(
            Icon::Trash2,
            "Remove",
            (!self.busy).then(|| session(Message::AskRemove(Some(server.id)))),
            l,
        ));
        let floating = container(squircle(actions, 10.0, l).fill(l.float).edge().padding(2))
            .align_right(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding { right: if active { 40.0 } else { 10.0 }, ..Padding::ZERO });
        iced_widget::stack![base, floating].boxed()
    }

    pub fn confirm_remove(&self, server: &Server, l: &Look) -> Element<'_, AppMessage> {
        let id = server.id;
        confirm(
            "Remove this server?",
            Some("You’ll have to connect and sign in again to use it."),
            "Remove",
            true,
            move |yes| session(if yes { Message::Remove(id) } else { Message::AskRemove(None) }),
            l,
        )
    }

    fn mismatch_banner(&self, version: &str, l: &Look) -> Element<'_, AppMessage> {
        let older =
            match (semver::Version::parse(updates::VERSION), semver::Version::parse(version.trim_start_matches('v'))) {
                (Ok(app), Ok(server)) if app > server => "server",
                _ => "app",
            };
        banner(
            Icon::TriangleAlert,
            l.warn,
            text(
                format!(
                    "This app is {}, your server is {version}. Some things may not work. Update the {older}.",
                    updates::VERSION
                ),
                type_::SM,
                l.ink,
            ),
            Some(session(Message::DismissMismatch)),
            l,
        )
    }

    fn forgot(&self, l: &Look) -> Option<Element<'_, AppMessage>> {
        self.connection.as_ref().is_some_and(|c| c.server.email).then(|| {
            button("Forgot your password?", l)
                .plain()
                .small()
                .fill()
                .on_press_maybe((!self.busy).then(|| session(Message::Reset)))
                .boxed()
        })
    }

    fn http(&self) -> bool {
        self.api.as_ref().is_some_and(|api| api.origin.scheme() == "http")
    }

    fn has_profiles(&self) -> bool {
        self.connection.as_ref().is_some_and(|c| {
            matches!(c.server.sign_in_style, api::connection::SignInStyle::PROFILES) && !c.sign_in_profiles.is_empty()
        })
    }

    pub(super) fn show_profiles(&self) -> bool {
        !self.by_name && !self.connection.as_ref().is_some_and(|c| c.server.setup_required) && self.has_profiles()
    }

    /// The desktop's own settings, at the top of the Settings page.
    pub fn settings(&self, look: &Look) -> Element<'_, AppMessage> {
        let l = look;
        let motion = segmented(
            self.config.reduce_motion,
            &[(None, "System"), (Some(true), "On"), (Some(false), "Off")],
            |choice| session(Message::ReduceMotion(choice)),
            l,
        );
        let mut updates = column![setting(
            "Check for updates",
            Some("Once a day, from GitHub. Nothing about you is sent."),
            toggle(self.config.check_updates, |on| session(Message::Updates(on)), l),
            l
        )]
        .spacing(16);
        updates = updates.push(text(format!("This is version {}.", updates::VERSION), type_::SM, l.ink_3).boxed());
        if let Some(release) = &self.config.release {
            updates = updates.push(
                button(format!("Version {} is out", release.version), l)
                    .primary()
                    .small()
                    .icon(Icon::ArrowDownToLine)
                    .on_press(session(Message::OpenRelease))
                    .boxed(),
            );
        }
        let mut cards = vec![
            page_title("Settings", None, l),
            card(
                "Appearance",
                None,
                setting("Reduce motion", Some("Follows your desktop unless you pick."), motion, l),
                l,
            ),
            card("Updates", None, updates.boxed(), l),
            card(
                "Servers",
                Some("Switch between them from the menu under your name."),
                column![
                    self.server_list(44.0, l),
                    container(button("Add server", l).icon(Icon::Plus).on_press(session(Message::Add)).boxed())
                        .padding(Padding { top: 12.0, ..Padding::ZERO })
                ]
                .boxed(),
                l,
            ),
        ];
        let mpv = crate::player::config_file();
        let found = mpv.as_ref().is_some_and(|file| file.is_file());
        let path = mpv.as_ref().map_or_else(|| "No configuration folder".to_owned(), |file| file.display().to_string());
        cards.push(card(
            "Advanced",
            Some("Videos play through mpv. Its own options, like scalers, debanding or shaders, can go in an mpv.conf; what the app controls stays as it sets it."),
            setting(
                "mpv.conf",
                Some(if found { "Found, and read when a video starts." } else { "Not there, so mpv's defaults play, with debanding." }),
                text(path, type_::XS, l.ink_2).boxed(),
                l,
            ),
            l,
        ));
        cards.push(card(
            "About",
            None,
            column![
                text(format!("tinystream {}", updates::VERSION), type_::SM, l.ink),
                text("Geist Variable · SIL Open Font License 1.1", type_::XS, l.ink_2),
                text("Lucide 1.48.0 · ISC and MIT", type_::XS, l.ink_2),
                button("Licenses", l).on_press(AppMessage::Overlay(Some(crate::app::Overlay::Credits))).boxed(),
            ]
            .spacing(10)
            .boxed(),
            l,
        ));
        if self.fallback && !self.config.fallback_seen {
            cards.push(card(
                "Where you stay signed in",
                Some("No keyring (Secret Service) is running, so sign-ins are kept in files only you can read, in the app's state folder."),
                button("Got it", l).on_press(session(Message::AcknowledgeFallback)).boxed(),
                l,
            ));
        }
        page(Column::with_children(cards).width(Length::Fill), false)
    }
}

/// `Card` in `SettingsKit.tsx`.
fn card<'a>(
    title: &'a str,
    description: Option<&'a str>,
    content: Element<'a, AppMessage>,
    l: &Look,
) -> Element<'a, AppMessage> {
    let mut heading = Column::new().push(text(title, type_::S15, l.ink).font(SEMIBOLD).boxed());
    if let Some(description) = description {
        heading = heading.push(
            container(text(description, type_::SM, l.ink_3).line_height(iced_core::text::LineHeight::Relative(1.625)))
                .padding(Padding { top: 4.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    container(
        squircle(column![container(heading).padding(Padding { bottom: 16.0, ..Padding::ZERO }), content], 16.0, l)
            .fill(l.raised)
            .edge()
            .padding(20)
            .width(Length::Fill),
    )
    .padding(Padding { bottom: 20.0, ..Padding::ZERO })
    .boxed()
}

/// `Row` in `SettingsKit.tsx`: a setting's name and hint, and its control.
fn setting<'a>(
    label: &'a str,
    hint: Option<&'a str>,
    control: Element<'a, AppMessage>,
    l: &Look,
) -> Element<'a, AppMessage> {
    let mut words = Column::new().push(text(label, type_::SM, l.ink).boxed());
    if let Some(hint) = hint {
        words = words
            .push(container(text(hint, type_::XS, l.ink_3)).padding(Padding { top: 2.0, ..Padding::ZERO }).boxed());
    }
    row![words.width(Length::Fill), control].spacing(16).align_y(Alignment::Center).boxed()
}

/// The frame's top: the logo and the title, with what it's about under it.
fn header<'a>(title: &'a str, subtitle: Subtitle<'a>, centered: bool, l: &Look) -> Element<'a, AppMessage> {
    let mut words = Column::new().push(text(title, type_::LG, l.ink).font(SEMIBOLD).boxed());
    match subtitle {
        Subtitle::None => {},
        Subtitle::Text(line) => words = words.push(text(line, type_::SM, l.ink_2).boxed()),
        Subtitle::Server(host) => {
            let look = *l;
            let switcher = pressable(
                row![inherit(host, type_::SM), icon(Icon::ChevronDown, 14.0)].spacing(4).align_y(Alignment::Center),
                6.0,
                l,
                move |status| Appearance {
                    paint: Paint::NONE,
                    ink: if matches!(status, Status::Hovered | Status::Pressed) { look.ink } else { look.ink_2 },
                },
            )
            .on_press(session(Message::Manage));
            words = words.push(switcher.boxed());
        },
    }
    let line = row![logo(32.0, l), words].spacing(12).align_y(Alignment::Center);
    let line = if centered { container(line).center_x(Length::Fill) } else { container(line) };
    line.padding(Padding { bottom: 28.0, ..Padding::ZERO }).boxed()
}

/// The form's panel (`Panel radius={20} className="p-5"` around `space-y-4`).
fn form_panel<'a>(items: Vec<Element<'a, AppMessage>>, l: &Look) -> Element<'a, AppMessage> {
    panel(Column::with_children(items).spacing(16), 20.0, l).padding(20).width(Length::Fill).boxed()
}

/// The form's main action, which waits with a spinner.
fn primary<'a>(
    label: Option<&'a str>,
    waiting: &'a str,
    ready: bool,
    message: Message,
    busy: bool,
    l: &Look,
) -> Element<'a, AppMessage> {
    let action = button(label.unwrap_or(waiting), l).primary().large().fill();
    let action = if busy {
        action.content(
            row![spinner(18.0, l), inherit(waiting, type_::S15).font(MEDIUM)].spacing(10).align_y(Alignment::Center),
        )
    } else {
        action
    };
    action.on_press_maybe((ready && !busy).then(|| session(message))).boxed()
}

/// A way back, under the frame (`<Button variant="plain"><ArrowLeft /> …</Button>`).
fn back<'a>(label: &'a str, message: Message, enabled: bool, l: &Look) -> Element<'a, AppMessage> {
    container(button(label, l).icon(Icon::ArrowLeft).on_press_maybe(enabled.then(|| session(message))).boxed())
        .center_x(Length::Fill)
        .boxed()
}

fn error<'a>(message: &str, l: &Look) -> Element<'a, AppMessage> {
    text(message.to_owned(), type_::SM, l.danger).boxed()
}

/// Shown wherever a password could be typed over plain http (the mobile `Unencrypted`).
fn unencrypted<'a>(l: &Look) -> Element<'a, AppMessage> {
    row![
        container(icon(Icon::ShieldOff, 16.0).color(l.warn)).padding(Padding { top: 2.0, ..Padding::ZERO }),
        iced_widget::rich_text::<(), _, _>([
            iced_widget::span("Unencrypted connection: ").color(l.warn).font(MEDIUM),
            iced_widget::span("your password and media travel in plain text.").color(l.ink_2),
        ])
        .size(type_::S13.px)
        .line_height(iced_core::text::LineHeight::Absolute(20.0.into()))
        .width(Length::Fill),
    ]
    .spacing(10)
    .boxed()
}

/// Someone without a picture, on the sign-in screens (`UserRound`, thin).
fn person<'a>(size: f32, l: &Look) -> Element<'a, AppMessage> {
    icon(Icon::UserRound, size * 0.46).stroke(1.5).color(l.ink_2).boxed()
}

/// A server's address without the scheme, as it's usually written.
pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.trim_end_matches('/').to_owned()
}
