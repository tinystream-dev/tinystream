// SPDX-License-Identifier: AGPL-3.0-or-later

//! The player screen, laid out and styled as `Player.tsx` is: the picture, the top bar and the
//! controls fading away while it plays, skip and up-next cards, menus, the HUD, screenshots, the
//! paused screen and the last episode's.

use std::time::{Duration, Instant};

use iced_core::Length::Fill;
use iced_core::gradient::Linear;
use iced_core::{Alignment, Background, Color, ContentFit, Gradient, Padding, Radians, mouse};
use iced_widget::{
    Column, Element, Row, Stack, Widget, column, container, image, mouse_area, opaque, row, space, stack,
};

use super::{Hover, HudKind, Menu, Message, Player, QUALITIES, Quality, RATES, number, skip_label, timeline};
use crate::ui::controls::{divider, menu_item, panel, spinner, tip};
use crate::ui::effects::{Entrance, enter};
use crate::ui::icon::{Icon, icon};
use crate::ui::pressable::{Appearance, Status, pressable};
use crate::ui::squircle::{Paint, squircle};
use crate::ui::text::{self as type_, MEDIUM, SEMIBOLD, Size, inherit, text, truncate};
use crate::ui::video::video;
use crate::ui::{Look, motion};

/// `text-[30px] leading-tight`, the last episode's title.
const FINALE: Size = Size { px: 30.0, line: 37.5 };
/// `text-[52px] leading-[1.05]`, the title while paused.
const RESTING: Size = Size { px: 52.0, line: 54.6 };

pub fn view<'a>(player: &'a Player, look: &Look) -> Element<'a, Message> {
    if let Some(editor) = &player.clipping {
        return super::clip::view(player, editor, look);
    }
    if player.pip {
        let placeholder = container(
            column![
                text("Playing in picture in picture", type_::S15, look.media_ink),
                chrome_button(
                    (Icon::Maximize, 20.0, false),
                    "Return to player (p)",
                    Some(Message::PictureInPicture),
                    false,
                    1.0,
                    look
                ),
            ]
            .spacing(16)
            .align_x(Alignment::Center),
        )
        .center(Fill)
        .boxed();
        return Stack::with_children([placeholder, top_bar(player, 1.0, look)]).width(Fill).height(Fill).boxed();
    }
    let l = *look;
    let alpha = if look.reduce_motion { f32::from(player.show_chrome()) } else { player.chrome_alpha() };
    let chrome = alpha > 0.0;
    let duration = player.duration();
    let mut layers: Vec<Element<'a, Message>> = Vec::new();

    if let Some(screen) = &player.screen {
        let picture = video(screen, player.aspect)
            .recede(if look.reduce_motion { f32::from(player.is_rested()) } else { player.recede() })
            .ambient(!look.reduce_motion, !player.playing);
        layers.push(picture.boxed());
    }

    // The still holds the frame until the video has one of its own.
    if !player.started
        && let Some(still) = player.pb.as_ref().and_then(|pb| player.pictures.get(&pb.still))
    {
        layers.push(image(still.clone()).width(Fill).height(Fill).content_fit(ContentFit::Contain).boxed());
    }

    if let Some(pb) = &player.pb
        && player.rested.is_some()
    {
        let shown = player.is_rested();
        let t = if look.reduce_motion {
            1.0
        } else {
            player.rested.map_or(0.0, |(_, since)| motion::progress(since, Duration::from_millis(700), Instant::now()))
        };
        let opacity = if shown { t } else { 1.0 - t };
        if opacity > 0.0 {
            layers.push(resting(player, pb, opacity, look));
        }
    }

    let failure = player.failure();
    if (player.waiting || player.pb.is_none()) && failure.is_none() {
        layers.push(container(media_spinner(look)).center(Fill).boxed());
    }

    if let Some(hud) = &player.hud {
        layers.push(hud_view(&hud.kind, hud.at, look));
    }

    if let Some(error) = failure {
        layers.push(error_panel(error, look));
    }

    if chrome {
        layers.push(top_bar(player, alpha, look));
    }

    // Skip and up next, over the controls' right end.
    let mut corner = Column::new().spacing(12).align_x(Alignment::End);
    if let Some(chapter) = player.skippable()
        && !player.near_end()
        && (player.show_skip())
    {
        corner = corner.push(skip_pill(skip_label(chapter.title.as_deref().unwrap_or_default()), look));
    }
    if let Some(next) = player.pb.as_ref().and_then(|pb| pb.next.as_ref())
        && (player.near_end() || player.ended)
    {
        corner = corner.push(up_next(player, next, look));
    }
    layers.push(
        container(corner)
            .align_right(Fill)
            .align_bottom(Fill)
            .padding(Padding { right: 24.0, bottom: 112.0, ..Padding::ZERO })
            .boxed(),
    );

    if player.ended
        && let Some(pb) = player
            .pb
            .as_ref()
            .filter(|pb| pb.next.is_none() && matches!(pb.title.kind, crate::api::playback::TitleKind::SHOW))
    {
        layers.push(finale(player, pb, look));
    }

    if chrome {
        layers.push(controls(player, alpha, look));
        if let Some(hover) = player.hover
            && duration > 0.0
        {
            layers.push(preview(player, hover, look));
        }
    }

    if let Some(shot) = &player.shot {
        layers.push(shot_card(player, shot, look));
    }

    if let Some(menu) = player.menu {
        let dismiss = mouse_area(space().width(Fill).height(Fill)).on_press(Message::Menu(None));
        let panel = match menu {
            Menu::Tracks => tracks_menu(player, look),
            Menu::Quality => quality_menu(player, look),
        };
        layers.push(dismiss.boxed());
        layers.push(
            container(opaque(enter(panel, Entrance::pop(look), look)))
                .align_right(Fill)
                .align_bottom(Fill)
                .padding(Padding { right: 20.0, bottom: 72.0, ..Padding::ZERO })
                .boxed(),
        );
    }

    let shade = l.media_shade;
    let stage = container(Stack::with_children(layers).width(Fill).height(Fill))
        .width(Fill)
        .height(Fill)
        .style(move |_| container::Style { background: Some(shade.into()), ..container::Style::default() });
    let stage = mouse_area(stage).on_press(Message::Toggle).on_double_click(Message::Fullscreen);
    if player.show_chrome() || player.menu.is_some() {
        stage.boxed()
    } else {
        stage.interaction(mouse::Interaction::Hidden).boxed()
    }
}

/// The detached window uses the same screen and mpv instance as the main player.
pub fn pip_view<'a>(player: &'a Player, look: &Look) -> Element<'a, Message> {
    let mut layers = Vec::new();
    if let Some(screen) = &player.screen {
        layers.push(video(screen, player.aspect).boxed());
    }
    if player.waiting {
        layers.push(container(media_spinner(look)).center(Fill).boxed());
    }
    let alpha = if look.reduce_motion { f32::from(player.show_chrome()) } else { player.chrome_alpha() };
    if alpha > 0.0 {
        let controls = row![
            chrome_button(
                (if player.playing { Icon::Pause } else { Icon::Play }, 20.0, true),
                "Play or pause (k)",
                Some(Message::Toggle),
                false,
                1.0,
                look
            ),
            chrome_button(
                (Icon::RotateCcw, 18.0, false),
                "Back 10 seconds (j)",
                Some(Message::SeekBy(-10.0)),
                false,
                1.0,
                look
            ),
            chrome_button(
                (Icon::RotateCw, 18.0, false),
                "Forward 10 seconds (l)",
                Some(Message::SeekBy(10.0)),
                false,
                1.0,
                look
            ),
            space().width(Fill),
            chrome_button(
                (Icon::Maximize, 18.0, false),
                "Return to player (p)",
                Some(Message::PictureInPicture),
                false,
                1.0,
                look
            ),
        ]
        .spacing(2);
        layers.push(container(controls).align_bottom(Fill).padding(8).boxed());
    }
    let drag =
        mouse_area(container(space()).width(Fill).height(20)).on_press(Message::PipWindow(crate::chrome::Action::Drag));
    layers.push(container(drag).align_top(Fill).boxed());
    if alpha > 0.0 {
        layers.push(
            container(chrome_button(
                (Icon::X, 18.0, false),
                "Close picture in picture",
                Some(Message::PictureInPicture),
                false,
                alpha,
                look,
            ))
            .align_top(Fill)
            .align_right(Fill)
            .padding(6)
            .boxed(),
        );
    }
    layers.push(
        crate::chrome::Chrome::default()
            .edges()
            .map(|message| match message {
                crate::chrome::Message::Resize(direction) => {
                    Message::PipWindow(crate::chrome::Action::Resize(direction))
                },
                _ => Message::Poke,
            })
            .boxed(),
    );
    mouse_area(Stack::with_children(layers).width(Fill).height(Fill)).on_double_click(Message::PictureInPicture).boxed()
}

fn media_spinner<'a>(look: &Look) -> Element<'a, Message> {
    let mut l = *look;
    l.ink = look.media_ink.scale_alpha(0.8);
    l.line = look.media_ink.scale_alpha(0.15);
    spinner(36.0, &l)
}

/// A gradient from `from` at the top to `to` at the bottom (`bg-linear-to-b`).
fn downwards(stops: &[(f32, Color)]) -> Background {
    let mut linear = Linear::new(Radians(std::f32::consts::PI));
    for &(at, color) in stops {
        linear = linear.add_stop(at, color);
    }
    Background::Gradient(Gradient::Linear(linear))
}

/// `ChromeButton`: a 40 px square of an icon on the picture, with its label as a tip.
fn chrome_button<'a>(
    (glyph, size, filled): (Icon, f32, bool),
    label: &'a str,
    message: Option<Message>,
    active: bool,
    alpha: f32,
    look: &Look,
) -> Element<'a, Message> {
    let ink = look.media_ink;
    let mut glyph = icon(glyph, size);
    if filled {
        glyph = glyph.filled();
    }
    let disabled = message.is_none();
    let button = pressable(container(glyph).center(40), 10.0, look, move |status| {
        let hovered = matches!(status, Status::Hovered | Status::Pressed);
        let ink = if hovered || active { ink } else { ink.scale_alpha(0.85) };
        Appearance {
            paint: Paint { fill: (hovered || active).then(|| ink.scale_alpha(0.1 * alpha)), ..Paint::NONE },
            ink: ink.scale_alpha(if disabled { 0.4 * alpha } else { alpha }),
        }
    })
    .on_press_maybe(message);
    tip(button, label, look)
}

fn top_bar<'a>(player: &'a Player, alpha: f32, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let pb = player.pb.as_ref();
    let title = pb.map_or("", |pb| pb.title.name.as_str());
    let line = pb.and_then(subtitle_line);
    let mut words = Column::new().push(truncate(title, type_::S15, l.media_ink.scale_alpha(alpha)).font(MEDIUM));
    if let Some(line) = line {
        words = words.push(truncate(line, type_::S13, l.media_ink.scale_alpha(0.6 * alpha)));
    }
    let shade = l.media_shade;
    container(
        row![
            chrome_button((Icon::ArrowLeft, 20.0, false), "Back", Some(Message::Back), false, alpha, look),
            container(words).width(Fill).padding(Padding { top: 4.0, ..Padding::ZERO }),
        ]
        .spacing(12)
        .align_y(Alignment::Start),
    )
    .width(Fill)
    .padding(Padding { top: 16.0, right: 20.0, bottom: 64.0, left: 20.0 })
    .style(move |_| container::Style {
        background: Some(downwards(&[(0.0, shade.scale_alpha(0.7 * alpha)), (1.0, Color::TRANSPARENT)])),
        ..container::Style::default()
    })
    .boxed()
}

/// The episode's code and name, under the show's (`subtitleLine`).
fn subtitle_line(pb: &crate::api::playback::Playback) -> Option<String> {
    if !matches!(pb.title.kind, crate::api::playback::TitleKind::SHOW) {
        return None;
    }
    let parts: Vec<&str> =
        [pb.label.as_deref(), pb.name.as_deref()].into_iter().flatten().filter(|s| !s.is_empty()).collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn controls<'a>(player: &'a Player, alpha: f32, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let ink = l.media_ink;
    let duration = player.duration();
    let pb = player.pb.as_ref();
    let shown = player.drag.unwrap_or(player.time);
    let chapters: Vec<(f64, f64)> =
        pb.map(|pb| pb.media.chapters.iter().map(|c| (c.start, c.end)).collect()).unwrap_or_default();
    let bar = timeline::timeline(
        shown,
        duration,
        (player.time.min(player.cached), player.cached),
        &chapters,
        ink.scale_alpha(alpha),
        (Message::Hover, Message::Drag, Message::Seek),
    );

    let mut buttons = Row::new().spacing(4).align_y(Alignment::Center);
    buttons = buttons.push(chrome_button(
        (if player.playing { Icon::Pause } else { Icon::Play }, 20.0, true),
        if player.playing { "Pause (k)" } else { "Play (k)" },
        Some(Message::Toggle),
        false,
        alpha,
        look,
    ));
    if let Some(previous) = pb.and_then(|pb| pb.previous.as_ref()) {
        buttons = buttons.push(chrome_button(
            (Icon::SkipBack, 18.0, true),
            "Previous",
            Some(Message::Go(previous.id)),
            false,
            alpha,
            look,
        ));
    }
    if let Some(next) = pb.and_then(|pb| pb.next.as_ref()) {
        buttons = buttons.push(chrome_button(
            (Icon::SkipForward, 18.0, true),
            "Next (n)",
            Some(Message::Go(next.id)),
            false,
            alpha,
            look,
        ));
    }
    buttons = buttons.push(chrome_button(
        (Icon::RotateCcw, 20.0, false),
        "Back 10 seconds (j)",
        Some(Message::SeekBy(-10.0)),
        false,
        alpha,
        look,
    ));
    buttons = buttons.push(chrome_button(
        (Icon::RotateCw, 20.0, false),
        "Forward 10 seconds (l)",
        Some(Message::SeekBy(10.0)),
        false,
        alpha,
        look,
    ));
    buttons = buttons.push(volume_control(player, alpha, look));
    buttons = buttons.push(
        container(
            row![
                text(clock(player.time), type_::S13, ink.scale_alpha(0.8 * alpha)),
                text(format!("/ {}", clock(duration)), type_::S13, ink.scale_alpha(0.4 * alpha)),
            ]
            .spacing(4),
        )
        .padding(Padding { left: 8.0, ..Padding::ZERO })
        .boxed(),
    );
    if let Some(title) = player.chapter().and_then(|c| c.title.as_deref()) {
        buttons = buttons.push(
            container(truncate(title, type_::S13, ink.scale_alpha(0.5 * alpha)))
                .padding(Padding { left: 12.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    if player.rate != 1.0 {
        let badge =
            squircle(text(format!("{}×", number(player.rate)), type_::XS2, ink.scale_alpha(0.8 * alpha)), 6.0, look)
                .fill(ink.scale_alpha(0.1 * alpha))
                .padding([2, 6]);
        buttons = buttons.push(container(badge).padding(Padding { left: 12.0, ..Padding::ZERO }).boxed());
    }
    buttons = buttons.push(space().width(Fill).boxed());
    if pb.is_some() {
        let open = player.menu == Some(Menu::Tracks);
        let toggle = Message::Menu(if open { None } else { Some(Menu::Tracks) });
        buttons = buttons.push(chrome_button(
            (Icon::Captions, 20.0, false),
            "Audio & subtitles (c)",
            Some(toggle),
            open,
            alpha,
            look,
        ));
        let open = player.menu == Some(Menu::Quality);
        let toggle = Message::Menu(if open { None } else { Some(Menu::Quality) });
        buttons =
            buttons.push(chrome_button((Icon::Settings2, 20.0, false), "Quality", Some(toggle), open, alpha, look));
    }
    if player.can_clip && pb.is_some() {
        buttons = buttons.push(chrome_button(
            (Icon::Scissors, 20.0, false),
            "Clip (x)",
            Some(Message::MakeClip),
            false,
            alpha,
            look,
        ));
        buttons = buttons.push(chrome_button(
            (Icon::Camera, 20.0, false),
            "Screenshot (shift+s)",
            Some(Message::Screenshot),
            false,
            alpha,
            look,
        ));
    }
    buttons = buttons.push(chrome_button(
        (Icon::PictureInPicture2, 20.0, false),
        "Picture in picture (p)",
        Some(Message::PictureInPicture),
        false,
        alpha,
        look,
    ));
    let fullscreen = player.fullscreen;
    buttons = buttons.push(chrome_button(
        (if fullscreen { Icon::Minimize } else { Icon::Maximize }, 20.0, false),
        if fullscreen { "Exit full screen (f)" } else { "Full screen (f)" },
        Some(Message::Fullscreen),
        false,
        alpha,
        look,
    ));

    let shade = l.media_shade;
    container(
        container(column![bar.boxed(), buttons].spacing(8))
            .width(Fill)
            .padding(Padding { top: 80.0, right: 20.0, bottom: 16.0, left: 20.0 })
            .style(move |_| container::Style {
                background: Some(downwards(&[
                    (0.0, Color::TRANSPARENT),
                    (0.5, shade.scale_alpha(0.4 * alpha)),
                    (1.0, shade.scale_alpha(0.8 * alpha)),
                ])),
                ..container::Style::default()
            }),
    )
    .align_bottom(Fill)
    .boxed()
}

/// `VolumeControl`: mute, with the slider sliding out beside it under the pointer.
fn volume_control<'a>(player: &'a Player, alpha: f32, look: &Look) -> Element<'a, Message> {
    let glyph = if player.muted || player.volume == 0.0 {
        Icon::VolumeX
    } else if player.volume < 0.5 {
        Icon::Volume1
    } else {
        Icon::Volume2
    };
    let mut group = Row::new().align_y(Alignment::Center);
    group = group.push(chrome_button((glyph, 20.0, false), "Mute (m)", Some(Message::Mute), false, alpha, look));
    if player.volume_open {
        let value = if player.muted { 0.0 } else { player.volume };
        let slider = timeline::volume(value, look.media_ink.scale_alpha(alpha), Message::Volume);
        group =
            group.push(container(slider.boxed()).width(80).padding(Padding { right: 4.0, ..Padding::ZERO }).boxed());
    }
    mouse_area(group).on_enter(Message::VolumeHover(true)).on_exit(Message::VolumeHover(false)).boxed()
}

/// A frame from around the hovered time, and the time, above the timeline (`Preview`).
fn preview<'a>(player: &'a Player, hover: Hover, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let chapter = player
        .pb
        .as_ref()
        .and_then(|pb| pb.media.chapters.iter().find(|c| hover.at >= c.start && hover.at < c.end))
        .and_then(|c| c.title.as_deref());
    let mut label = Row::new().spacing(6).push(text(clock(hover.at), type_::XS, l.media_ink));
    if let Some(chapter) = chapter {
        label = label.push(text(chapter, type_::XS, l.media_ink.scale_alpha(0.55)));
    }
    let label = squircle(label, 8.0, look).fill(l.media_panel.scale_alpha(0.9)).padding([4, 8]);
    let mut stacked = Column::new().spacing(6).align_x(Alignment::Center);
    if let Some(frame) = player.preview_shown.and_then(|at| player.previews.get(&at)) {
        stacked = stacked.push(
            squircle(image(frame.clone()).width(176).height(99).content_fit(ContentFit::Cover), 10.0, look)
                .fill(l.media_panel.scale_alpha(0.9))
                .edge(),
        );
    }
    stacked = stacked.push(label);
    // Centred on the pointer, but kept inside the bar (`clamp(5.5rem, …, 100% - 5.5rem)`).
    let center = 20.0 + hover.x.clamp(88.0, (hover.width - 88.0).max(88.0));
    container(container(stacked).width(176).center_x(176))
        .align_bottom(Fill)
        .padding(Padding { left: (center - 88.0).max(0.0), bottom: 88.0, ..Padding::ZERO })
        .boxed()
}

/// `SkipPill`.
fn skip_pill<'a>(label: &'a str, look: &Look) -> Element<'a, Message> {
    let ink = look.media_ink;
    pressable(
        row![inherit(label, type_::S13).font(MEDIUM), text("S", type_::XS2, ink.scale_alpha(0.5))]
            .spacing(8)
            .align_y(Alignment::Center),
        12.0,
        look,
        move |status| {
            let hovered = matches!(status, Status::Hovered | Status::Pressed);
            Appearance {
                paint: Paint {
                    fill: Some(ink.scale_alpha(if hovered { 0.2 } else { 0.1 })),
                    edge: true,
                    ..Paint::NONE
                },
                ink: if hovered { ink } else { ink.scale_alpha(0.85) },
            }
        },
    )
    .height(36)
    .padding([0, 14])
    .on_press(Message::Skip)
    .boxed()
}

/// `UpNext`: the next episode, with the countdown sweeping across its button.
fn up_next<'a>(player: &'a Player, next: &'a crate::api::playback::PlaybackNext, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let still: Element<'a, Message> = match player.pictures.get(&next.still) {
        Some(still) => image(still.clone())
            .width(112)
            .height(63)
            .content_fit(ContentFit::Cover)
            .border_radius(l.radius(9.0))
            .boxed(),
        None => squircle(space().width(112).height(63), 9.0, look).fill(l.panel).boxed(),
    };
    let label = next.label.as_deref().unwrap_or_default();
    let name = next.name.as_deref().unwrap_or(label);
    let words = column![
        text(format!("Up next · {label}"), type_::XS, l.ink_3),
        text(name, type_::SM, l.ink).font(MEDIUM).line_height(iced_core::text::LineHeight::Relative(1.375)),
    ]
    .spacing(2)
    .width(Fill);
    let countdown = player.countdown_left();
    let sweep =
        player.countdown.map(|since| motion::progress(since, Duration::from_secs(super::COUNTDOWN), Instant::now()));
    let button_label = || {
        row![
            icon(Icon::Play, 16.0).filled().boxed(),
            inherit(if countdown.is_some() { "Playing next" } else { "Play now" }, type_::SM).font(MEDIUM).boxed()
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    };
    let (accent, on_accent) = (l.accent, l.on_accent);
    let portion = |t: f32| iced_core::Length::FillPortion((t * 1000.0) as u16 + 1);
    let fill: Element<'a, Message> = match sweep {
        Some(t) => stack![
            squircle(space().width(Fill).height(36), 10.0, look).fill(accent.scale_alpha(0.4)),
            row![
                squircle(space().width(Fill).height(36), 10.0, look).fill(accent).width(portion(t)),
                space().width(portion(1.0 - t))
            ],
            container(button_label()).center_x(Fill).center_y(36),
        ]
        .boxed(),
        None => squircle(container(button_label()).center_x(Fill).center_y(36), 10.0, look)
            .fill(accent.scale_alpha(0.85))
            .width(Fill)
            .boxed(),
    };
    let body = column![row![still, words].spacing(12), container(fill).padding(Padding { top: 10.0, ..Padding::ZERO })];
    let panel_fill = l.media_panel;
    let card = pressable(body, 16.0, look, move |status| Appearance {
        paint: Paint {
            fill: Some(if matches!(status, Status::Hovered | Status::Pressed) {
                panel_fill
            } else {
                panel_fill.scale_alpha(0.85)
            }),
            edge: true,
            lift: true,
            ..Paint::NONE
        },
        ink: on_accent,
    })
    .width(320)
    .padding(10)
    .on_press(Message::Go(next.id));
    enter(card, Entrance::pop(look), look)
}

/// `Finale`: the newest episode is over; when the next one comes, if it's known.
fn finale<'a>(player: &'a Player, pb: &'a crate::api::playback::Playback, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let next = player.series.as_ref().and_then(|series| series.next.as_ref());
    let airs = next.and_then(|n| n.air_at.map(|at| (n, at)));
    let heading = if airs.is_some() { "YOU’RE ALL CAUGHT UP" } else { "THAT’S THE LAST ONE" };
    let line = match airs {
        Some((n, at)) => {
            let name = n.name.as_deref().map(|name| format!(" “{name}”")).unwrap_or_default();
            format!("{}{name} airs {}.", episode_code(n.season, n.episode), lowercase_day(&super::airs(at)))
        },
        None if player.series.as_ref().and_then(|s| s.status.as_deref()) == Some("finished") => {
            "You’ve finished the whole show.".into()
        },
        None => match &pb.label {
            Some(label) => format!("You’ve watched every episode there is, up to {label}."),
            None => "You’ve watched every episode there is.".into(),
        },
    };
    let ink = l.media_ink;
    let shade = l.media_shade;
    let back = pressable(
        row![icon(Icon::ArrowLeft, 18.0), inherit("Back to the show", type_::S15).font(MEDIUM)]
            .spacing(8)
            .align_y(Alignment::Center),
        14.0,
        look,
        move |status| Appearance {
            paint: Paint::fill(if matches!(status, Status::Hovered | Status::Pressed) {
                ink.scale_alpha(0.9)
            } else {
                ink
            }),
            ink: shade,
        },
    )
    .height(44)
    .padding([0, 20])
    .on_press(Message::Back);
    let content = column![
        text(heading, type_::XS, ink.scale_alpha(0.6)).font(MEDIUM),
        text(pb.title.name.as_str(), FINALE, ink).font(SEMIBOLD).align_x(Alignment::Center),
        text(line, type_::S15, ink.scale_alpha(0.7)).align_x(Alignment::Center),
        container(back).padding(Padding { top: 16.0, ..Padding::ZERO }),
    ]
    .spacing(8)
    .align_x(Alignment::Center)
    .width(Fill.max(448));
    container(enter(content, Entrance::rise(), look))
        .center(Fill)
        .padding(24)
        .style(move |_| container::Style {
            background: Some(shade.scale_alpha(0.6).into()),
            ..container::Style::default()
        })
        .boxed()
}

/// Paused for a while: what you're watching, big, over the receded frame (`Resting`).
fn resting<'a>(
    player: &'a Player,
    pb: &'a crate::api::playback::Playback,
    opacity: f32,
    look: &Look,
) -> Element<'a, Message> {
    let l = *look;
    let ink = l.media_ink;
    let mut words = Column::new()
        .width(Fill.max(576))
        .push(text("PAUSED", type_::XS, ink.scale_alpha(0.5 * opacity)).font(MEDIUM).boxed());
    words = words.push(
        container(text(pb.title.name.as_str(), RESTING, ink.scale_alpha(opacity)).font(SEMIBOLD))
            .padding(Padding { top: 12.0, ..Padding::ZERO })
            .boxed(),
    );
    if let Some(line) = subtitle_line(pb) {
        words = words.push(
            container(text(line, type_::LG, ink.scale_alpha(0.75 * opacity)))
                .padding(Padding { top: 10.0, ..Padding::ZERO })
                .boxed(),
        );
    }
    if let Some(overview) = &player.overview {
        let clipped: String = overview.chars().take(320).collect();
        let clipped = if clipped.len() < overview.len() { format!("{}…", clipped.trim_end()) } else { clipped };
        words = words.push(
            container(
                text(clipped, type_::S15, ink.scale_alpha(0.6 * opacity))
                    .line_height(iced_core::text::LineHeight::Relative(1.625)),
            )
            .padding(Padding { top: 16.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    let duration = player.duration();
    if duration > 0.0 {
        words = words.push(
            container(text(
                format!("{} of {}", clock(player.time), clock(duration)),
                type_::SM,
                ink.scale_alpha(0.45 * opacity),
            ))
            .padding(Padding { top: 24.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    let shade = l.media_shade;
    let mut gradient = Linear::new(Radians(std::f32::consts::FRAC_PI_2));
    for (at, a) in [(0.0, 0.65), (0.5, 0.25), (1.0, 0.0)] {
        gradient = gradient.add_stop(at, shade.scale_alpha(a * opacity));
    }
    // Rises into place as it comes (`translate-y-4`, over 1.2 s).
    let t = player.rested.map_or(1.0, |(_, since)| {
        motion::SETTLE.at(motion::progress(since, Duration::from_millis(1200), Instant::now()))
    });
    let rise = if player.is_rested() { 16.0 * (1.0 - t) } else { 0.0 };
    container(container(words).padding(Padding { top: rise * 2.0, ..Padding::ZERO }))
        .width(Fill)
        .height(Fill)
        .align_y(Alignment::Center)
        .padding(Padding { left: 96.0, right: 96.0, ..Padding::ZERO })
        .style(move |_| container::Style {
            background: Some(Background::Gradient(Gradient::Linear(gradient))),
            ..container::Style::default()
        })
        .boxed()
}

/// `HudView`: a quiet confirmation of what just happened.
fn hud_view<'a>(kind: &HudKind, at: Instant, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let ink = l.media_ink;
    let shade = l.media_shade;
    let round = |content: Element<'a, Message>, size: f32, alpha: f32| -> Element<'a, Message> {
        squircle(container(content).center(size), size / 2.0, look).round().fill(shade.scale_alpha(alpha)).boxed()
    };
    match kind {
        HudKind::Seek(amount) => {
            let back = *amount < 0.0;
            let glyph = icon(if back { Icon::RotateCcw } else { Icon::RotateCw }, 28.0).color(ink);
            let label = text(format!("{}{} s", if back { "−" } else { "+" }, number(amount.abs())), type_::SM, ink)
                .font(MEDIUM);
            let content = enter(
                column![round(glyph.boxed(), 64.0, 0.45), label].spacing(4).align_x(Alignment::Center),
                Entrance { duration: Duration::from_millis(160), ..Entrance::pop(look) },
                look,
            );
            let side = Row::new()
                .push(space().width(iced_core::Length::FillPortion(if back { 12 } else { 88 })).boxed())
                .push(content)
                .push(space().width(iced_core::Length::FillPortion(if back { 88 } else { 12 })).boxed());
            container(side).width(Fill).height(Fill).align_y(Alignment::Center).boxed()
        },
        HudKind::Volume(volume) => {
            let glyph = if *volume == 0.0 {
                Icon::VolumeX
            } else if *volume < 0.5 {
                Icon::Volume1
            } else {
                Icon::Volume2
            };
            let level = (*volume as f32).clamp(0.0, 1.0);
            let bar = stack![
                squircle(space().width(144).height(4), 2.0, look).round().fill(ink.scale_alpha(0.2)),
                squircle(space().width(144.0 * level).height(4), 2.0, look).round().fill(ink),
            ];
            let pill = squircle(
                row![
                    icon(glyph, 18.0).color(ink),
                    bar,
                    container(text(format!("{}", (volume * 100.0).round()), type_::XS, ink))
                        .width(32)
                        .align_right(32)
                        .boxed()
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                20.0,
                look,
            )
            .round()
            .fill(shade.scale_alpha(0.55))
            .padding([10, 16]);
            top_center(pill.boxed())
        },
        HudKind::Speed(words) | HudKind::Notice(words) => {
            let pill = squircle(text(words.clone(), type_::SM, ink).font(MEDIUM), 18.0, look)
                .round()
                .fill(shade.scale_alpha(0.55))
                .padding([8, 16]);
            top_center(pill.boxed())
        },
        HudKind::Play | HudKind::Pause => {
            // `flash`: in over the first fifth, then out.
            let t = motion::progress(at, Duration::from_millis(600), Instant::now());
            let alpha = if t < 0.2 { t / 0.2 } else { 1.0 - (t - 0.2) / 0.8 };
            let glyph = icon(if matches!(kind, HudKind::Play) { Icon::Play } else { Icon::Pause }, 32.0)
                .filled()
                .color(ink.scale_alpha(alpha));
            container(round(glyph.boxed(), 80.0, 0.45 * alpha)).center(Fill).boxed()
        },
    }
}

/// Something near the top, centred (`top-[12%]`).
fn top_center<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    column![
        space().height(iced_core::Length::FillPortion(12)),
        container(content).center_x(Fill),
        space().height(iced_core::Length::FillPortion(88))
    ]
    .height(Fill)
    .boxed()
}

/// The panel saying the video can't play, with what to try.
fn error_panel<'a>(error: String, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let link = |label: &'a str, message: Message| {
        pressable(inherit(label, type_::SM), 4.0, &l, move |status| Appearance {
            paint: Paint::NONE,
            ink: if matches!(status, Status::Hovered | Status::Pressed) { l.ink } else { l.ink.scale_alpha(0.9) },
        })
        .on_press(message)
    };
    let content = column![
        text("This video can't play right now", type_::S15, l.ink).font(MEDIUM),
        text(error, type_::SM, l.ink_2).line_height(iced_core::text::LineHeight::Relative(1.625)),
        container(
            row![
                link("Try again", Message::Retry),
                text("or", type_::SM, l.ink_3),
                link("convert it to 720p", Message::Convert)
            ]
            .spacing(8)
        )
        .padding(Padding { top: 10.0, ..Padding::ZERO }),
    ]
    .spacing(6);
    container(container(opaque(panel(content, 16.0, look).padding(20))).width(Fill.max(448)))
        .center(Fill)
        .padding(24)
        .boxed()
}

/// `ShotCard`: the shutter's flash, then what was saved, in the corner.
fn shot_card<'a>(player: &'a Player, shot: &'a super::Shot, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let flash = 0.55 * (1.0 - motion::progress(shot.at, Duration::from_millis(420), Instant::now()));
    let ink = l.media_ink;
    let shutter = container(space().width(Fill).height(Fill)).style(move |_| container::Style {
        background: Some(ink.scale_alpha(flash).into()),
        ..container::Style::default()
    });
    let body: Element<'a, Message> = match (&shot.error, &shot.clip) {
        (Some(error), _) => column![
            text("Couldn’t take a screenshot", type_::SM, l.ink).font(MEDIUM),
            text(error.as_str(), type_::XS, l.ink_2),
        ]
        .spacing(2)
        .padding(12)
        .boxed(),
        (None, clip) => {
            let poster =
                clip.as_ref().and_then(|(_, poster)| poster.as_ref()).and_then(|poster| player.pictures.get(poster));
            let picture: Element<'a, Message> = match poster {
                Some(poster) => image(poster.clone()).width(Fill).height(144).content_fit(ContentFit::Cover).boxed(),
                None => container(spinner(20.0, look)).center_x(Fill).center_y(144).boxed(),
            };
            let shade = l.media_shade;
            let picture = container(picture)
                .width(Fill)
                .style(move |_| container::Style { background: Some(shade.into()), ..container::Style::default() });
            let words = row![
                icon(Icon::Camera, 14.0).color(l.highlight),
                text(if clip.is_some() { "Saved to Clips" } else { "Taking a screenshot…" }, type_::S13, l.ink)
                    .font(MEDIUM),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([8, 12]);
            let card = column![picture, words];
            let open = clip.is_some().then_some(Message::OpenShot);
            pressable(card, 14.0, look, move |_| Appearance { paint: Paint::NONE, ink: l.ink })
                .width(Fill)
                .on_press_maybe(open)
                .boxed()
        },
    };
    let card = squircle(body, 14.0, look).fill(l.float).edge().lift().width(256);
    stack![
        shutter,
        container(enter(card, Entrance::rise(), look)).align_right(Fill).align_bottom(Fill).padding(Padding {
            right: 20.0,
            bottom: 112.0,
            ..Padding::ZERO
        }),
    ]
    .boxed()
}

/// The audio and subtitles menu (`Captions`).
fn tracks_menu<'a>(player: &'a Player, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let Some(pb) = &player.pb else { return space().boxed() };
    let heading = |label: &'a str| {
        container(text(label, type_::XS, l.ink_3))
            .padding(Padding { top: 6.0, right: 10.0, bottom: 4.0, left: 10.0 })
            .boxed()
    };
    let mut audio = Column::new().width(Fill).push(heading("Audio"));
    for track in &pb.media.audio {
        let hint = (track.channels > 2).then(|| channels(track.channels));
        let name = super::tracks::track_name(
            track.title.as_deref(),
            track.language.as_deref(),
            &format!("Track {}", track.index),
        );
        audio = audio.push(menu_item(
            None,
            inherit(name, type_::S13),
            hint,
            Some(track.index) == player.audio,
            Some(Message::Audio(track.index)),
            look,
        ));
    }
    let mut subtitles = Column::new().width(Fill).push(heading("Subtitles"));
    subtitles = subtitles.push(menu_item(
        None,
        inherit("Off", type_::S13),
        None,
        player.subtitle.is_none(),
        Some(Message::Subtitle(None)),
        look,
    ));
    for track in &pb.media.subtitles {
        let hint = if !track.supported {
            Some("image")
        } else if track.forced {
            Some("forced")
        } else {
            None
        };
        let fallback = if track.id.starts_with('x') {
            "External".to_owned()
        } else {
            format!("Track {}", &track.id[1.min(track.id.len())..])
        };
        let name = super::tracks::track_name(track.title.as_deref(), track.language.as_deref(), &fallback);
        let message = super::tracks::usable(track).then(|| Message::Subtitle(Some(track.id.clone())));
        subtitles = subtitles.push(menu_item(
            None,
            inherit(name, type_::S13),
            hint,
            player.subtitle.as_ref() == Some(&track.id),
            message,
            look,
        ));
    }
    panel(row![audio.boxed(), subtitles.boxed()].spacing(4), 14.0, look).padding(6).width(544).boxed()
}

/// The speed and quality menu (`Settings2`).
fn quality_menu<'a>(player: &'a Player, look: &Look) -> Element<'a, Message> {
    let l = *look;
    let mut speeds = Row::new().spacing(2).padding([0, 4]);
    for rate in RATES {
        let on = player.rate == rate;
        speeds = speeds.push(
            pressable(
                container(inherit(format!("{}×", number(rate)), type_::XS)).center_x(Fill).center_y(28),
                8.0,
                look,
                move |status| {
                    let hovered = matches!(status, Status::Hovered | Status::Pressed);
                    Appearance {
                        paint: Paint { fill: if on { Some(l.ink) } else { hovered.then_some(l.hover) }, ..Paint::NONE },
                        ink: if on { l.canvas } else { l.ink_2 },
                    }
                },
            )
            .width(Fill)
            .on_press(Message::Rate(rate))
            .boxed(),
        );
    }
    let mut menu = Column::new()
        .push(
            container(text("Speed", type_::XS, l.ink_3))
                .padding(Padding { top: 6.0, right: 10.0, bottom: 4.0, left: 10.0 })
                .boxed(),
        )
        .push(container(speeds).padding(Padding { bottom: 6.0, ..Padding::ZERO }).boxed())
        .push(container(divider(look)).padding([4, 0]).boxed())
        .push(
            container(
                text(player.describe(), type_::XS, l.ink_3).line_height(iced_core::text::LineHeight::Relative(1.625)),
            )
            .padding(Padding { top: 6.0, right: 10.0, bottom: 8.0, left: 10.0 })
            .boxed(),
        );
    for quality in QUALITIES {
        let (label, hint) = match quality {
            Quality::Auto => ("Automatic".to_owned(), Some("the original file")),
            Quality::Height(height) => (format!("{height}p"), None),
        };
        menu = menu.push(menu_item(
            None,
            inherit(label, type_::S13),
            hint,
            player.quality == quality,
            Some(Message::Quality(quality)),
            look,
        ));
    }
    panel(menu, 14.0, look).padding(6).width(256).boxed()
}

/// How many channels a track has, as its menu hint.
fn channels(count: i64) -> &'static str {
    const NAMES: [&str; 14] = [
        "3 ch", "4 ch", "5 ch", "6 ch", "7 ch", "8 ch", "9 ch", "10 ch", "11 ch", "12 ch", "13 ch", "14 ch", "15 ch",
        "16 ch",
    ];
    NAMES.get((count - 3).clamp(0, 13) as usize).copied().unwrap_or("16 ch")
}

/// `S01E02`, or `Special 3` (`episodeCode`).
fn episode_code(season: i64, episode: i64) -> String {
    if season == 0 { format!("Special {episode}") } else { format!("S{season:02}E{episode:02}") }
}

/// "Today at…" mid-sentence.
fn lowercase_day(airs: &str) -> String {
    for day in ["Today", "Tomorrow"] {
        if let Some(rest) = airs.strip_prefix(day) {
            return format!("{}{rest}", day.to_lowercase());
        }
    }
    airs.to_owned()
}

/// `1:02:03` or `2:03` (`clock` in `format.ts`).
pub fn clock(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "0:00".into();
    }
    let s = seconds.max(0.0).floor() as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_and_codes_like_the_web() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(65.9), "1:05");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(episode_code(1, 2), "S01E02");
        assert_eq!(episode_code(0, 3), "Special 3");
        assert_eq!(lowercase_day("Tomorrow at 17:30"), "tomorrow at 17:30");
    }
}
