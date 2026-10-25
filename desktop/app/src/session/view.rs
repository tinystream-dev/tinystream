// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::Length::Fill;
use iced_core::{Alignment, Background, ContentFit, Theme};
use iced_widget::{
    Column, Element, Widget, button, column, container, image, responsive, row, scrollable, text, text_input, toggler,
};

use super::{Message, Screen, Session};
use crate::app::Message as AppMessage;
use crate::auth::{self, action, plain};
use crate::style::Look;
use crate::{api, updates};

impl Session {
    pub fn look(&self) -> Look {
        let mut look = Look::default();
        if let Some(data) = &self.connection {
            let appearance = &data.appearance;
            let dark = self.system_dark && matches!(appearance.mode, api::connection::SchemeMode::SYSTEM);
            if dark {
                for token in &appearance.dark.palette.tokens {
                    look.token(&token.name, &token.value);
                }
            } else {
                for token in &appearance.light.palette.tokens {
                    look.token(&token.name, &token.value);
                }
            }
            match appearance.style {
                api::connection::ComponentStyle::FLAT => {
                    look.smoothing = 0.0;
                    look.radius_scale = 0.8;
                    look.elevated = false;
                },
                api::connection::ComponentStyle::GLASS => {
                    look.smoothing = 0.8;
                    look.radius_scale = 1.25;
                },
                _ => {},
            }
        }
        look
    }

    pub fn view(&self) -> Element<'_, AppMessage> {
        responsive(move |size| self.frame(size)).boxed()
    }

    fn frame(&self, size: iced_core::Size) -> Element<'_, AppMessage> {
        let look = self.look();
        let picker = self.screen == Screen::Login && self.show_profiles() && self.profile.is_none();
        let unlock = self.screen == Screen::Login && self.profile.is_some();
        let setup = self.screen == Screen::Login && self.connection.as_ref().is_some_and(|c| c.server.setup_required);
        let title = match self.screen {
            Screen::Connect => "Connect to a server",
            Screen::Login if setup => "Welcome to tinystream",
            Screen::Login if picker => "Who's watching?",
            Screen::Login => "Sign in",
            Screen::Invite => "Join tinystream",
            Screen::Reset if self.notice.is_some() => "Check your inbox",
            Screen::Reset => "Forgot your password?",
            Screen::NewPassword => "Pick a new password",
            Screen::Servers => "Your servers",
            Screen::Link => "Open a link",
            Screen::Ready => "tinystream",
        };
        let mut frame = Column::new().width(if picker { 768 } else { 384 }).spacing(28);
        if unlock {
            let avatar = self.avatar(self.profile.as_deref().unwrap_or_default(), 128.0, look);
            frame = frame.push(
                container(
                    column![avatar, plain("Not you?", Message::Login, !self.busy, look)]
                        .spacing(12)
                        .align_x(Alignment::Center),
                )
                .center_x(Fill)
                .boxed(),
            );
        } else {
            let header = row![auth::logo(look.ink, 32.0), text(title).size(18).font(auth::SEMIBOLD).color(look.ink)]
                .spacing(12)
                .align_y(Alignment::Center);
            frame = frame.push(if picker { container(header).center_x(Fill).boxed() } else { header.boxed() });
        }
        if picker {
            let profiles = self.connection.iter().flat_map(|c| &c.sign_in_profiles).map(|profile| {
                button(self.avatar(&profile.key, 104.0, look))
                    .padding(0)
                    .on_press_maybe((!self.busy).then(|| AppMessage::Session(Message::Profile(profile.key.clone()))))
                    .style(move |_, status| button::Style {
                        background: (status == button::Status::Hovered).then_some(look.line.into()),
                        border: iced_core::border::rounded(999),
                        ..button::Style::default()
                    })
                    .boxed()
            });
            frame = frame.push(container(row(profiles).spacing(28).wrap()).center_x(Fill).padding([16, 0]).boxed());
        } else {
            let mut form = Column::new().width(Fill).spacing(16);
            match self.screen {
                Screen::Servers => {
                    form = form.push(self.server_list()).push(action("Add server", Message::Add, !self.busy, look));
                },
                Screen::Connect | Screen::Link => {
                    form = form.push(field(
                        if self.screen == Screen::Connect { "Server address" } else { "Invite or reset link" },
                        &self.address,
                        Message::Address,
                        false,
                        look,
                    ));
                    if self.screen == Screen::Connect && self.address.trim().starts_with("http://") {
                        form = form.push(
                            text("HTTP is unencrypted. Use HTTPS outside a trusted network.")
                                .size(13)
                                .color(look.muted)
                                .boxed(),
                        );
                    }
                    form = form.push(action(
                        if self.busy {
                            "Connecting…"
                        } else if self.screen == Screen::Link {
                            "Open link"
                        } else if self.store.is_none() {
                            "Retry"
                        } else {
                            "Connect"
                        },
                        if self.screen == Screen::Connect && self.store.is_none() {
                            Message::Start
                        } else {
                            Message::Submit
                        },
                        !self.busy && (self.store.is_none() || !self.address.trim().is_empty()),
                        look,
                    ));
                },
                Screen::Reset if self.notice.is_some() => {
                    form = form
                        .push(text(self.notice.as_deref().unwrap_or_default()).size(14).color(look.muted).boxed())
                        .push(action("Back to sign in", Message::Login, !self.busy, look));
                },
                Screen::Invite | Screen::NewPassword if !self.link_valid => {
                    form = form.push(
                        text(if self.busy {
                            "Checking link…"
                        } else {
                            self.error.as_deref().unwrap_or("This link doesn't work anymore. Ask for a new one.")
                        })
                        .size(14)
                        .color(look.muted)
                        .boxed(),
                    );
                },
                _ => {
                    if matches!(self.screen, Screen::Login | Screen::Invite | Screen::Reset) && !unlock {
                        form = form.push(field(
                            if self.screen == Screen::Reset { "Username or email" } else { "Username" },
                            &self.username,
                            Message::Username,
                            false,
                            look,
                        ));
                    }
                    if self.screen != Screen::Reset {
                        form = form.push(field("Password", &self.password, Message::Password, true, look));
                    }
                    let valid = if self.screen == Screen::Reset {
                        !self.username.trim().is_empty()
                    } else {
                        !self.password.is_empty()
                            && (unlock || self.screen == Screen::NewPassword || !self.username.trim().is_empty())
                    };
                    form = form.push(action(
                        if self.busy {
                            "Please wait…"
                        } else {
                            match self.screen {
                                Screen::Invite => "Create account",
                                Screen::Reset => "Send link",
                                Screen::NewPassword => "Change password",
                                _ if setup => "Create account",
                                _ => "Sign in",
                            }
                        },
                        Message::Submit,
                        !self.busy && valid,
                        look,
                    ));
                    if self.screen == Screen::Login
                        && !setup
                        && self.connection.as_ref().is_some_and(|c| c.server.email)
                    {
                        form = form.push(
                            container(plain("Forgot your password?", Message::Reset, !self.busy, look))
                                .center_x(Fill)
                                .boxed(),
                        );
                    }
                },
            }
            if let Some(error) = &self.error
                && !(matches!(self.screen, Screen::Invite | Screen::NewPassword) && !self.link_valid)
            {
                form = form.push(text(error).size(13).color(look.danger).boxed());
            }
            frame = frame.push(auth::panel(form.boxed(), look));
        }
        let mut footer = Column::new().align_x(Alignment::Center).spacing(8).width(Fill);
        if picker {
            footer = footer.push(plain("Sign in with a username instead", Message::ByName, !self.busy, look));
        } else if self.screen == Screen::Login && self.by_name {
            footer = footer.push(plain("Back to pictures", Message::Login, !self.busy, look));
        } else if matches!(self.screen, Screen::Link | Screen::Servers)
            || (self.screen == Screen::Connect && self.config.active.is_some())
        {
            footer = footer.push(plain("Back", Message::Cancel, !self.busy, look));
        } else if matches!(self.screen, Screen::Invite | Screen::NewPassword | Screen::Reset)
            && !(self.screen == Screen::Reset && self.notice.is_some())
        {
            footer = footer.push(plain("Back to sign in", Message::Login, !self.busy, look));
        }
        if matches!(self.screen, Screen::Connect | Screen::Login) {
            let mut links =
                row![plain("Open an invite or reset link", Message::EnterLink, !self.busy, look)].spacing(8);
            if !self.config.servers.is_empty() {
                links = links.push(plain("Servers", Message::Manage, !self.busy, look));
            }
            footer = footer.push(links.boxed());
        }
        if let Some(version) = self.mismatch() {
            footer = footer
                .push(text(format!("App {} · Server {version}", updates::VERSION)).size(12).color(look.muted).boxed())
                .push(plain("Dismiss version warning", Message::DismissMismatch, true, look));
        }
        frame = frame.push(footer.boxed());
        scrollable(container(frame).padding([48, 24]).center_x(Fill).center_y(size.height.max(640.0)))
            .width(Fill)
            .height(Fill)
            .boxed()
    }

    fn avatar(&self, key: &str, size: f32, look: Look) -> Element<'_, AppMessage> {
        match self.avatars.get(key) {
            Some(handle) => image(handle.clone())
                .width(size)
                .height(size)
                .content_fit(ContentFit::Cover)
                .border_radius(999.0)
                .boxed(),
            None => container(auth::logo(look.faint, size * 0.4))
                .center(size)
                .style(move |_| container::Style {
                    background: Some(look.raised.into()),
                    border: iced_core::border::rounded(999),
                    ..container::Style::default()
                })
                .boxed(),
        }
    }

    pub(super) fn show_profiles(&self) -> bool {
        !self.by_name
            && self.connection.as_ref().is_some_and(|c| {
                matches!(c.server.sign_in_style, api::connection::SignInStyle::PROFILES)
                    && !c.server.setup_required
                    && !c.sign_in_profiles.is_empty()
            })
    }

    pub fn server_list(&self) -> Element<'_, AppMessage> {
        let look = self.look();
        Column::with_children(self.config.servers.iter().map(|server| {
            let active = self.config.active == Some(server.id);
            let account = server.user.as_ref().map(|u| u.username.as_str()).unwrap_or("Sign in");
            let label = format!("{}{}", if active { "✓  " } else { "" }, server.name);
            column![
                button(
                    column![
                        row![
                            text(label).size(14).color(look.ink).width(Fill),
                            text(if server.url.starts_with("http://") { "HTTP" } else { "" })
                                .size(10)
                                .color(look.faint)
                        ]
                        .align_y(Alignment::Center),
                        text(account).size(12).color(look.muted)
                    ]
                    .spacing(4)
                )
                .width(Fill)
                .padding([10, 8])
                .on_press_maybe((!self.busy).then_some(AppMessage::Session(Message::Switch(server.id))))
                .style(move |_, status| button::Style {
                    background: (status == button::Status::Hovered).then_some(look.line.into()),
                    border: iced_core::border::rounded(9),
                    ..button::Style::default()
                }),
                row![
                    plain("Sign out", Message::SignOut(server.id), !self.busy && server.user.is_some(), look),
                    plain("Remove", Message::Remove(server.id), !self.busy, look)
                ]
                .spacing(8),
            ]
            .spacing(2)
            .boxed()
        }))
        .spacing(12)
        .boxed()
    }

    pub fn settings(&self) -> Element<'_, AppMessage> {
        let look = self.look();
        let mut body = column![
            text("Settings").size(28).font(auth::SEMIBOLD).color(look.ink),
            toggler(self.config.check_updates)
                .label("Check for updates")
                .on_toggle(|enabled| AppMessage::Session(Message::Updates(enabled))),
            text(format!("App version {}", updates::VERSION)).size(14).color(look.muted),
            self.server_list(),
            plain("Add server", Message::Add, !self.busy, look),
        ]
        .spacing(20)
        .padding(40)
        .width(560);
        if self.fallback && !self.config.fallback_seen {
            body = body.push(text("No Secret Service is running. Sessions are saved in private files (0600) in the state directory.").size(14).color(look.muted).boxed())
                .push(plain("Got it", Message::AcknowledgeFallback, true, look));
        }
        if let Some(release) = &self.config.release {
            body = body.push(plain(
                &format!("Version {} is available", release.version),
                Message::OpenRelease,
                true,
                look,
            ));
        }
        body.boxed()
    }
}

fn field<'a>(
    label: &'a str,
    value: &'a str,
    message: fn(String) -> Message,
    secure: bool,
    look: Look,
) -> Element<'a, AppMessage> {
    let input = text_input("", value)
        .id(if secure {
            "auth-password"
        } else if label == "Username" || label == "Username or email" {
            "auth-username"
        } else {
            "auth-address"
        })
        .secure(secure)
        .on_input(move |value| AppMessage::Session(message(value)))
        .on_submit(AppMessage::Session(Message::Submit))
        .line_height(iced_core::text::LineHeight::Absolute(20.0.into()))
        .padding([8, 12])
        .size(14)
        .style(move |_: &Theme, status| text_input::Style {
            background: Background::Color(iced_core::Color::TRANSPARENT),
            border: iced_core::border::rounded(9)
                .width(if matches!(status, text_input::Status::Focused { .. }) { 1 } else { 0 })
                .color(look.focus),
            placeholder: look.faint,
            value: look.ink,
            selection: look.focus,
        });
    column![
        text(label).size(13).color(look.muted),
        auth::surface(container(input).height(36).center_y(36).boxed(), look, look.raised, 9.0, false)
    ]
    .spacing(6)
    .boxed()
}
