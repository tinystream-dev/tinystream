// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every widget and effect on one page, in each component style, with the default scheme and a
//! custom one (`--gallery`, for development). Compare it with the web UI at the same size.

use std::time::Instant;

use iced_core::{Alignment, Length};
use iced_widget::{Column, Element, Row, Widget as _, column, container, image, row, scrollable, space, stack};

use super::controls::{
    Scale, Tone, badge, banner, button, checkbox, chip, divider, field, icon_button, input, menu_item, or, panel,
    progress, segmented, spinner, tip, toggle,
};
use super::effects::{Ambient, Entrance, ambient, bone, bone_rounded, enter, page_transition, veil};
use super::icon::{Icon, icon};
use super::layout::{empty, page, page_title, poster_grid, section, shelf};
use super::media::{Poster, avatar, poster};
use super::numbers::{sparkline, ticker};
use super::overlay::{self, Toast, confirm, select, toasts};
use super::squircle::squircle;
use super::text::{self as type_, MEDIUM, SEMIBOLD, text};
use super::theme::{Look, Rounded, Style};
use super::tilt::art;

/// A scheme that isn't built in: plum, from a shared code.
pub const CUSTOM: &str = "ts1.GxYi7ObyxLX98ouCj8eafdP8_NNN-ajUxLX9";

const PLACES: [&str; 4] = ["Home", "Library", "Calendar", "Settings"];

pub struct Gallery {
    pub style: Style,
    pub custom: bool,
    /// Overrides the desktop's reduced motion setting.
    pub still: Option<bool>,
    on: bool,
    checked: bool,
    segment: usize,
    chip: usize,
    name: String,
    password: String,
    place: Option<&'static str>,
    count: u32,
    dialog: bool,
    sheet: bool,
    shelf: Option<iced_widget::scrollable::Viewport>,
    toasts: Vec<Toast>,
    next: u64,
    pub transition: Option<Instant>,
    pub scroll: f32,
    artwork: Vec<image::Handle>,
    image: crate::images::Images,
}

#[derive(Debug, Clone)]
pub enum Message {
    Style(Style),
    Custom(bool),
    Still(bool),
    On(bool),
    Checked(bool),
    Segment(usize),
    Chip(usize),
    Name(String),
    Password(String),
    Place(&'static str),
    Count,
    Dialog(bool),
    Sheet(bool),
    Shelf(iced_widget::scrollable::Scroll),
    Page(f32),
    Toast,
    Dismiss(u64),
    Transition,
    Noop,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            style: Style::Layered,
            custom: false,
            still: None,
            on: true,
            checked: false,
            segment: 0,
            chip: 0,
            name: String::new(),
            password: String::new(),
            place: Some(PLACES[0]),
            count: 97,
            dialog: false,
            sheet: false,
            shelf: None,
            toasts: Vec::new(),
            next: 0,
            transition: None,
            scroll: 0.0,
            artwork: (0..6).map(artwork).collect(),
            image: {
                let mut images = crate::images::Images::default();
                images.arrived("sample".into(), Some(artwork(5)), false);
                images
            },
        }
    }
}

/// A made-up poster: a warm or cool wash with a sun in it, so there's something to tilt and blur.
fn artwork(seed: u32) -> image::Handle {
    let (w, h) = (300u32, 450u32);
    let hue = (seed * 67 % 360) as f32;
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let sun = ((u - 0.62).powi(2) + (v - 0.3).powi(2)).sqrt();
            let glow = (1.0 - sun / 0.22).clamp(0.0, 1.0);
            let base = super::media::oklch(0.35 + 0.35 * (1.0 - v), 0.12, (hue + v * 60.0) % 360.0);
            let light = super::media::oklch(0.92, 0.08, (hue + 40.0) % 360.0);
            let c = super::theme::mix(light, base, glow.powf(0.6));
            let band = if (v * 9.0 + u * 2.0).fract() < 0.04 && v > 0.6 { 0.85 } else { 1.0 };
            pixels.extend([(c.r * 255.0 * band) as u8, (c.g * 255.0 * band) as u8, (c.b * 255.0 * band) as u8, 255]);
        }
    }
    image::Handle::from_rgba(w, h, pixels)
}

impl Gallery {
    pub fn update(&mut self, message: Message, system_still: bool) -> Option<Box<dyn iced_core::widget::Operation>> {
        match message {
            Message::Style(style) => self.style = style,
            Message::Custom(custom) => self.custom = custom,
            Message::Still(still) => self.still = Some(still),
            Message::On(on) => self.on = on,
            Message::Checked(checked) => self.checked = checked,
            Message::Segment(segment) => self.segment = segment,
            Message::Chip(chip) => self.chip = chip,
            Message::Name(name) => self.name = name,
            Message::Password(password) => self.password = password,
            Message::Place(place) => self.place = Some(place),
            Message::Count => self.count += 7,
            Message::Dialog(open) => self.dialog = open,
            Message::Sheet(open) => self.sheet = open,
            Message::Shelf(scroll) => self.shelf = Some(scroll.viewport),
            Message::Page(direction) => {
                let width = self.shelf.map_or(800.0, |v| v.bounds.width);
                let motion = if self.still.unwrap_or(system_still) {
                    iced_core::widget::operation::Animation::Instant
                } else {
                    iced_core::widget::operation::Animation::default()
                };
                return Some(Box::new(iced_core::widget::operation::scrollable::scroll_by(
                    "gallery-shelf".into(),
                    iced_core::widget::operation::scrollable::AbsoluteOffset { x: direction * width * 0.8, y: 0.0 },
                    motion,
                )));
            },
            Message::Toast => {
                self.next += 1;
                let tone = [overlay::Tone::Ok, overlay::Tone::Danger, overlay::Tone::Quiet][self.next as usize % 3];
                self.toasts.push(Toast {
                    id: self.next,
                    title: match tone {
                        overlay::Tone::Ok => "Added to your list".into(),
                        overlay::Tone::Danger => "That didn’t work".into(),
                        overlay::Tone::Quiet => "Scanning Anime".into(),
                    },
                    body: (tone == overlay::Tone::Danger).then(|| "The server returned HTTP 502.".into()),
                    tone,
                });
                if self.toasts.len() > 4 {
                    self.toasts.remove(0);
                }
            },
            Message::Dismiss(id) => self.toasts.retain(|t| t.id != id),
            Message::Transition => self.transition = Some(Instant::now()),
            Message::Noop => {},
        }
        None
    }

    /// The look the gallery is showing, over the desktop's own settings.
    pub fn look(&self, system_still: bool) -> Look {
        let mut look = if self.custom {
            Look::from_code(CUSTOM, self.style).expect("a valid code")
        } else {
            Look::new(&tinystream_theme::find_builtin("grey").expect("Grey").scheme.tokens(), self.style, false)
        };
        look.reduce_motion = self.still.unwrap_or(system_still);
        look
    }

    pub fn modal_open(&self) -> bool {
        self.dialog || self.sheet
    }

    pub fn animating(&self) -> bool {
        self.transition.is_some_and(|start| page_transition(start.elapsed()).is_some())
    }

    pub fn view(&self, look: &Look) -> Element<'_, Message> {
        let l = look;
        let controls = row![
            segmented(
                self.style,
                &[(Style::Layered, "Layered"), (Style::Flat, "Flat"), (Style::Glass, "Glass")],
                Message::Style,
                l
            ),
            segmented(self.custom, &[(false, "Grey"), (true, "Plum")], Message::Custom, l),
            row![text("Reduce motion", type_::SM, l.ink_2), toggle(l.reduce_motion, Message::Still, l)]
                .spacing(10)
                .align_y(Alignment::Center),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let body = column![
            page_title("Gallery", Some(controls.boxed()), l),
            section("Type", None, self.type_scale(l), l),
            section("Colours", None, self.swatches(l), l),
            section("Buttons", None, self.buttons(l), l),
            section("Fields and choices", None, self.fields(l), l),
            section("Surfaces and glass", None, self.surfaces(l), l),
            section("Artwork", None, self.artwork(l), l),
            section("Loading", None, self.loading(l), l),
            section("Feedback", None, self.feedback(l), l),
        ];

        let mut layers = vec![
            scrollable(page(body, false))
                .id("gallery")
                .direction(iced_widget::scrollable::Direction::Vertical(iced_widget::scrollable::Scrollbar::hidden()))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        ];
        if let Some((_, veiled)) =
            self.transition.filter(|_| !l.reduce_motion).and_then(|start| page_transition(start.elapsed()))
        {
            layers.push(veil(veiled, l));
        }
        if !self.toasts.is_empty() {
            layers.push(toasts(&self.toasts, Message::Dismiss, l));
        }
        let base = iced_widget::Stack::with_children(layers).width(Length::Fill).height(Length::Fill).boxed();
        if self.dialog {
            return super::modal::modal(
                base,
                confirm(
                    "Remove Anime?",
                    Some("Its files stay on disk. You can add the folder again later."),
                    "Remove",
                    true,
                    |_| Message::Dialog(false),
                    l,
                ),
                Message::Dialog(false),
            );
        }
        if self.sheet {
            return super::modal::modal(
                base,
                overlay::sheet(
                    column![
                        row![
                            text("Details", type_::LG, l.ink).font(SEMIBOLD).width(Length::Fill),
                            icon_button(Icon::X, "Close", Some(Message::Sheet(false)), l)
                        ],
                        field("Name", input("sheet-name", &self.name, "", Message::Name, l).boxed(), None, l),
                        button("Save changes", l).primary().on_press(Message::Sheet(false)).boxed()
                    ]
                    .spacing(24),
                    Message::Sheet(false),
                    l,
                ),
                Message::Sheet(false),
            );
        }
        base
    }

    fn type_scale(&self, l: &Look) -> Element<'_, Message> {
        let sample = |name: &'static str, size: type_::Size, font: iced_core::Font| {
            row![
                container(text(name, type_::XS, l.ink_3)).width(140),
                text("Who’s watching tonight?", size, l.ink).font(font),
            ]
            .align_y(Alignment::Center)
            .boxed()
        };
        Column::with_children([
            sample("28 semibold · title", type_::TITLE, SEMIBOLD),
            sample("18 semibold · lg", type_::LG, SEMIBOLD),
            sample("24 semibold · 2xl", type_::XL2, SEMIBOLD),
            sample("20 medium · xl", type_::XL, MEDIUM),
            sample("16 · base", type_::BASE, type_::REGULAR),
            sample("15 medium", type_::S15, MEDIUM),
            sample("14 · sm", type_::SM, type_::REGULAR),
            sample("13", type_::S13, type_::REGULAR),
            sample("12 · xs", type_::XS, type_::REGULAR),
            sample("11 · 2xs", type_::XS2, type_::REGULAR),
        ])
        .spacing(10)
        .boxed()
    }

    fn swatches(&self, l: &Look) -> Element<'_, Message> {
        let cells = tinystream_theme::TOKENS.iter().map(|&name| {
            let color = l.token(name).expect("a token");
            column![
                squircle(space().width(Length::Fill).height(44), 10.0, l).fill(color).edge().width(Length::Fill),
                text(name, type_::XS2, l.ink_3),
            ]
            .spacing(6)
            .width(Length::Fill)
            .boxed()
        });
        let cells: Vec<_> = cells.collect();
        let mut rows = Column::new().spacing(14);
        let mut cells = cells.into_iter().peekable();
        while cells.peek().is_some() {
            rows = rows.push(Row::with_children(cells.by_ref().take(7)).spacing(14).boxed());
        }
        rows.boxed()
    }

    fn buttons(&self, l: &Look) -> Element<'_, Message> {
        let line = |scale: Scale| {
            row![
                button("Primary", l).primary().scale(scale).on_press(Message::Noop).boxed(),
                button("Quiet", l).scale(scale).icon(Icon::Plus).on_press(Message::Noop).boxed(),
                button("Plain", l).plain().scale(scale).on_press(Message::Noop).boxed(),
                button("Danger", l).danger().scale(scale).icon(Icon::Trash2).on_press(Message::Noop).boxed(),
                button("Disabled", l).primary().scale(scale).boxed(),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .boxed()
        };
        let icons = Row::with_children(
            [
                (Icon::Search, "Search"),
                (Icon::Bell, "Notifications"),
                (Icon::Settings, "Settings"),
                (Icon::Keyboard, "Keyboard shortcuts"),
                (Icon::LogOut, "Sign out"),
                (Icon::Ellipsis, "More"),
            ]
            .into_iter()
            .map(|(glyph, label)| icon_button(glyph, label, Some(Message::Noop), l)),
        )
        .spacing(4);
        column![
            line(Scale::Sm),
            line(Scale::Md),
            line(Scale::Lg),
            row![icons, tip(text("Rest the pointer here", type_::SM, l.ink_2), "A tip, like the web’s", l)]
                .spacing(24)
                .align_y(Alignment::Center),
            container(
                button("Sign in with a passkey", l)
                    .radius(14.0)
                    .large()
                    .icon(Icon::KeyRound)
                    .fill()
                    .on_press(Message::Noop)
                    .boxed()
            )
            .width(384),
        ]
        .spacing(16)
        .boxed()
    }

    fn fields(&self, l: &Look) -> Element<'_, Message> {
        let left = column![
            field("Username", input("gallery-name", &self.name, "", Message::Name, l).boxed(), None, l),
            field(
                "Password",
                input("gallery-password", &self.password, "", Message::Password, l).secure().boxed(),
                Some("At least eight characters."),
                l
            ),
            field("Start page", select(self.place, &PLACES, |p| (*p).to_owned(), Message::Place, l), None, l),
        ]
        .spacing(16)
        .width(Length::Fill);
        let chips = Row::with_children(
            ["All", "Shows", "Movies", "Unwatched"]
                .iter()
                .enumerate()
                .map(|(i, name)| chip(name, self.chip == i, Message::Chip(i), l)),
        )
        .spacing(4);
        let right = column![
            row![toggle(self.on, Message::On, l), text("Check for updates", type_::SM, l.ink)]
                .spacing(12)
                .align_y(Alignment::Center),
            row![
                checkbox(self.checked, Some(Message::Checked(!self.checked)), l),
                text("Remember me", type_::SM, l.ink)
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            segmented(self.segment, &[(0, "Day"), (1, "Week"), (2, "Month")], Message::Segment, l),
            chips,
            row![
                badge("Quiet", Tone::Quiet, l),
                badge("Ready", Tone::Ok, l),
                badge("Failed", Tone::Danger, l),
                badge("Live", Tone::Live, l),
                badge("Waiting", Tone::Warn, l),
                badge("New", Tone::Strong, l),
            ]
            .spacing(6),
        ]
        .spacing(16)
        .width(Length::Fill);
        row![left, right].spacing(40).boxed()
    }

    fn surfaces(&self, l: &Look) -> Element<'_, Message> {
        // Glass needs something behind it to blur: artwork and an ambient glow.
        let backdrop = row![
            container(
                image(self.artwork[1].clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(iced_core::ContentFit::Cover)
            )
            .width(Length::Fill),
            container(
                image(self.artwork[3].clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(iced_core::ContentFit::Cover)
            )
            .width(Length::Fill),
            container(
                image(self.artwork[4].clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(iced_core::ContentFit::Cover)
            )
            .width(Length::Fill),
        ]
        .height(300);
        let menu = panel(
            column![
                menu_item(
                    Some(Icon::Settings),
                    type_::inherit("Settings", type_::S13),
                    Some("G S"),
                    false,
                    Some(Message::Noop),
                    l
                ),
                menu_item(
                    Some(Icon::Keyboard),
                    type_::inherit("Keyboard shortcuts", type_::S13),
                    Some("?"),
                    false,
                    Some(Message::Noop),
                    l
                ),
                container(divider(l)).padding([4, 0]),
                menu_item(
                    Some(Icon::LogOut),
                    type_::inherit("Sign out", type_::S13),
                    None,
                    false,
                    Some(Message::Noop),
                    l
                ),
            ],
            14.0,
            l,
        )
        .padding(6)
        .width(224);
        let floating = row![
            menu,
            panel(
                column![
                    text("A panel", type_::S15, l.ink).font(MEDIUM),
                    text("Layered lifts it, Flat lines it, Glass blurs what's behind.", type_::SM, l.ink_2),
                    button("Open a dialog", l).on_press(Message::Dialog(true)).boxed(),
                ]
                .spacing(10),
                20.0,
                l
            )
            .padding(20)
            .width(320),
        ]
        .spacing(24)
        .padding(28);
        let cards = row![
            squircle(
                column![
                    text("A card", type_::S15, l.ink).font(SEMIBOLD),
                    text("Settings sections sit in these.", type_::SM, l.ink_3)
                ]
                .spacing(4),
                16.0,
                l
            )
            .fill(l.raised)
            .edge()
            .padding(20)
            .width(Length::Fill),
            squircle(container(text("A placeholder", type_::SM, l.ink_3)).center(Length::Fill), 16.0, l)
                .dashed()
                .width(Length::Fill)
                .height(86),
        ]
        .spacing(16);
        column![
            stack![backdrop, ambient(Ambient::corner(l.highlight)), floating],
            cards,
            banner(
                Icon::TriangleAlert,
                l.warn,
                text(
                    "This app is 0.38.0, your server is 0.37.2. Some things may not work. Update the server.",
                    type_::SM,
                    l.ink
                ),
                Some(Message::Noop),
                l
            ),
        ]
        .spacing(20)
        .boxed()
    }

    fn artwork(&self, l: &Look) -> Element<'_, Message> {
        let look = *l;
        let posters = poster_grid(self.artwork.len(), move |i| {
            let l = &look;
            poster(
                Poster {
                    name: ["Frieren", "Dandadan", "Mushishi", "Odd Taxi", "Ping Pong", "Bocchi"][i],
                    picture: Some(&self.artwork[i]),
                    arrived: None,
                    caption: format!("{} episodes", 12 + i * 2),
                    progress: (i == 0).then_some(0.42),
                    fresh: (i == 1).then_some("2 new"),
                },
                Message::Noop,
                l,
            )
        });
        let loaded = container(super::media::img(self.image.picture("sample"), 14.0, l)).width(180).height(120);
        let people = Row::with_children(
            ["David", "Mira", "Kaz", "Juno"]
                .into_iter()
                .enumerate()
                .map(|(i, name)| avatar(name, (i == 0).then(|| &self.artwork[5]), 56.0, None, l)),
        )
        .spacing(16)
        .align_y(Alignment::Center);
        let tinted = stack![
            squircle(space().width(Length::Fill).height(160), 22.0, l).fill(l.raised),
            ambient(Ambient::title(l.info)),
            container(text("Ambient tint", type_::S15, l.ink).font(MEDIUM)).padding(20),
        ]
        .height(160);
        let cards = (0..12)
            .map(|i| container(art(Some(self.artwork[i % 6].clone()), 14.0, 1.5, l)).width(152).boxed())
            .collect();
        let bar = squircle(text("Glass bars blur the page below.", type_::SM, l.ink), 12.0, l)
            .fill(l.canvas)
            .material(l.material_bar())
            .padding(16);
        column![
            posters,
            shelf("gallery-shelf", cards, self.shelf, Message::Shelf, Message::Page, l),
            people,
            loaded,
            tinted,
            bar,
            button("Play a page transition", l).on_press(Message::Transition).boxed()
        ]
        .spacing(24)
        .boxed()
    }

    fn loading(&self, l: &Look) -> Element<'_, Message> {
        let skeleton = row![
            column![bone_rounded(152, 228, 14.0, l), bone(114, 12, Rounded::Xl, l), bone(50, 10, Rounded::Lg, l)]
                .spacing(8),
            column![bone_rounded(152, 228, 14.0, l), bone(114, 12, Rounded::Md, l), bone(50, 10, Rounded::Md, l)]
                .spacing(8),
            column![bone(Length::Fill, 240, Rounded::Xl2, l)].width(Length::Fill),
        ]
        .spacing(20);
        let values: Vec<f32> =
            (0..60).map(|i| (((i as f32) * 0.37).sin() * 0.5 + 0.6) * (1.0 + (i % 7) as f32 * 0.08)).collect();
        column![
            skeleton,
            row![
                spinner(20.0, l),
                container(progress(None, l.info, l)).width(200),
                container(progress(Some(0.62), l.ok, l)).width(200),
                sparkline(&values, l.info, 160, 40, std::time::Duration::from_secs(1), l),
            ]
            .spacing(24)
            .align_y(Alignment::Center),
            row![
                ticker(&self.count.to_string(), type_::XL2, l),
                button("Count up", l).small().on_press(Message::Count).boxed(),
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        ]
        .spacing(24)
        .boxed()
    }

    fn feedback(&self, l: &Look) -> Element<'_, Message> {
        column![
            row![
                button("Show a toast", l).on_press(Message::Toast).boxed(),
                button("Ask something", l).on_press(Message::Dialog(true)).boxed(),
                button("Open a sheet", l).on_press(Message::Sheet(true)).boxed(),
            ]
            .spacing(12),
            or(l),
            enter(text("This line rose into place.", type_::SM, l.ink_2), Entrance::arrive(0), l),
            empty("Nothing here yet", Some("Libraries show up here once the server has scanned them."), l),
            row![
                icon(Icon::Sparkles, 18.0).color(l.highlight),
                text("Icons take the colour they're given, or their text's.", type_::SM, l.ink_2)
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(20)
        .boxed()
    }
}
