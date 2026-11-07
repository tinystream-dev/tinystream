// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::Length;
use iced_widget::{column, row};

use super::*;

/// Where an episode is between airing and being playable.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Countdown,
    Searching,
    Downloading,
    Ready,
    Out,
}

fn stage_of(e: &CalendarEntry) -> Stage {
    if e.air_at > now() {
        Stage::Countdown
    } else if e.video.is_some() {
        Stage::Ready
    } else if e.download.is_some() || e.state == "GRABBED" {
        Stage::Downloading
    } else if e.monitor == "NONE" {
        Stage::Out
    } else {
        Stage::Searching
    }
}

/// Whether an entry still deserves the hero spot.
fn hero_worthy(e: &CalendarEntry) -> bool {
    match stage_of(e) {
        Stage::Countdown => true,
        Stage::Out => now() - e.air_at < 2 * 3600,
        _ => now() - e.air_at < 12 * 3600,
    }
}

pub(super) fn calendar<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let start = calendar_start(b.week);
    let key = Key::Calendar(start);
    let controls = line(vec![
        container(text(format!("{} – {}", date(start), date(shift_days(start, 6))), ty::SM, look.ink_3))
            .padding(Padding { right: 4.0, ..Padding::ZERO })
            .boxed(),
        icon_button(Icon::ChevronLeft, "Previous week", Some(msg(b, Message::Week(b.week - 1))), look),
        button("This week", look).small().on_press_maybe((b.week != 0).then(|| msg(b, Message::Week(0)))).boxed(),
        icon_button(Icon::ChevronRight, "Next week", Some(msg(b, Message::Week(b.week + 1))), look),
    ]);
    let mut page = column![page_title(
        "Calendar",
        Some(container(controls).padding(Padding { bottom: 2.0, ..Padding::ZERO }).boxed()),
        look
    )]
    .width(Fill);
    let soon = b.data(&Key::Soon).map(|d| d.calendar.as_slice());
    if b.week == 0
        && let Some(next) = soon.and_then(|s| s.iter().find(|e| hero_worthy(e)))
    {
        let later: Vec<_> = soon.unwrap_or_default().iter().filter(|e| e.air_at > next.air_at).take(3).collect();
        let l = *look;
        page = page.push(
            container(
                iced_widget::responsive(move |size| up_next(b, next, &later, size.width >= 704.0, &l))
                    .height(Length::Shrink),
            )
            .padding(Padding { bottom: 32.0, ..Padding::ZERO })
            .boxed(),
        );
    }
    let Some(d) = b.data(&key) else { return page.push((pending(b, &key, look)).boxed()).boxed() };
    if d.calendar.is_empty() && soon.is_some_and(<[_]>::is_empty) {
        return page.push((layout::empty("Nothing on the schedule", None, look)).boxed()).boxed();
    }
    let today = shift_days(now(), 0);
    let mut days = Row::<View<'a>>::new().spacing(12).width(Fill);
    for i in 0..7 {
        let day = shift_days(start, i);
        let end = shift_days(day, 1);
        let (is_today, past) = (day == today, day < today);
        let mut entries = Column::<View<'a>>::new().spacing(6).width(Fill);
        let mut count = 0;
        for e in d.calendar.iter().filter(|e| e.air_at >= day && e.air_at < end) {
            entries = entries.push(day_card(b, e, look));
            count += 1;
        }
        let number = text(
            civil(day).2.to_string(),
            ty::S13,
            if is_today {
                look.canvas
            } else if past {
                look.ink_3
            } else {
                look.ink_2
            },
        );
        let number = if is_today {
            squircle(container(number.font(SEMIBOLD)).center(24), 12.0, look).fill(look.ink).round().boxed()
        } else {
            container(number).center(24).boxed()
        };
        let mut column = squircle(entries, 16.0, look)
            .fill(if is_today { look.raised } else { look.raised.scale_alpha(0.5) })
            .padding(6)
            .height(416.0f32.max(count as f32 * 190.0))
            .width(Fill);
        if is_today {
            column = column.paint(crate::ui::squircle::Paint {
                fill: Some(look.raised),
                ring: Some(look.line_strong),
                ..crate::ui::squircle::Paint::NONE
            });
        }
        days = days.push(
            column![
                container(
                    row![
                        text(weekday(day)[..3].to_uppercase(), ty::XS, if is_today { look.ink } else { look.ink_3 })
                            .font(MEDIUM),
                        number
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                )
                .padding([0, 4]),
                column
            ]
            .spacing(8)
            .width(Fill)
            .boxed(),
        );
    }
    page.push(days.boxed()).boxed()
}

fn up_next<'a>(
    b: &'a Browsing,
    e: &'a CalendarEntry,
    later: &[&'a CalendarEntry],
    wide: bool,
    look: &Look,
) -> View<'a> {
    let stage = stage_of(e);
    let dl = e.download.as_ref();
    let pct = dl.map(|d| d.progress as f32);
    let mark = match stage {
        Stage::Ready => crate::ui::icon::icon(Icon::Play, 14.0).filled().color(look.ok),
        Stage::Downloading => crate::ui::icon::icon(Icon::ArrowDown, 14.0).color(look.info),
        Stage::Countdown => crate::ui::icon::icon(Icon::Radio, 14.0).color(look.ink_2),
        _ => crate::ui::icon::icon(Icon::Radio, 14.0).color(look.warn),
    };
    let label = match stage {
        Stage::Countdown => "Up next",
        Stage::Searching => "Just aired",
        Stage::Downloading => "Downloading",
        Stage::Ready => "Ready to watch",
        Stage::Out => "Out now",
    };
    let big = |s: String| text(s, BIG, look.ink).font(SEMIBOLD).boxed();
    let body: View<'a> = match stage {
        Stage::Countdown => crate::ui::numbers::styled_ticker(&big_countdown(e.air_at), BIG, SEMIBOLD, None, look),
        Stage::Searching => row![effects::pulse_dot(12.0, look.warn, look), big("Searching…".into())]
            .spacing(12)
            .align_y(Alignment::Center)
            .boxed(),
        Stage::Downloading => {
            let mut parts = row![match pct {
                Some(p) => crate::ui::numbers::styled_ticker(
                    &format!("{:.1}%", p * 100.0),
                    BIG,
                    SEMIBOLD,
                    Some(look.info),
                    look
                ),
                None => text("Starting…", BIG, look.info).font(SEMIBOLD).boxed(),
            }]
            .spacing(16)
            .align_y(Alignment::Center);
            if let Some(dl) = dl {
                let mut rates = Column::<View<'a>>::new();
                if dl.download_rate > 0 {
                    rates = rates.push(
                        row![
                            crate::ui::icon::icon(Icon::ArrowDown, 14.0).color(look.ink_2),
                            crate::ui::numbers::styled_ticker(
                                &speed(dl.download_rate),
                                ty::SM,
                                ty::REGULAR,
                                Some(look.ink_2),
                                look
                            )
                        ]
                        .spacing(4)
                        .align_y(Alignment::Center)
                        .boxed(),
                    );
                }
                if let Some(eta) = dl.eta {
                    rates = rates.push(text(format!("{} left", duration(eta)), ty::SM, look.ink_3).boxed());
                }
                parts = parts.push(rates.boxed());
            }
            parts.boxed()
        },
        Stage::Ready => button(format!("Play {}", code(e.season, e.episode)), look)
            .primary()
            .large()
            .icon(Icon::Play)
            .on_press_maybe(entry_route(e).map(AppMessage::Navigate))
            .boxed(),
        Stage::Out => big("Out now".into()),
    };
    let aside = (wide && !later.is_empty()).then(|| {
        let mut list = column![text("THEN", ty::XS2, look.ink_3).font(MEDIUM)].spacing(8).width(Fill);
        for l in later {
            list = list.push(
                column![
                    truncate(l.show.as_str(), ty::S13, look.ink).font(MEDIUM),
                    truncate(format!("{} · {}", code(l.season, l.episode), airs(l.air_at)), ty::XS, look.ink_3)
                ]
                .boxed(),
            );
        }
        list.boxed()
    });
    let detail = [Some(code(e.season, e.episode)), e.name.clone(), Some(airs(e.air_at))]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    hero(
        b,
        Hero {
            name: &e.show,
            label,
            mark,
            detail,
            image: e.backdrop.as_deref().or(e.poster.as_deref()),
            tint: e.backdrop_tint.as_deref(),
            route: entry_route(e),
            height: 240.0,
            name_size: 34.0,
            padding: 32.0,
            body: container(body).height(48).align_y(Alignment::Center).boxed(),
            aside,
            bar: (stage == Stage::Downloading).then_some(pct),
        },
        look,
    )
}

fn entry_route(e: &CalendarEntry) -> Option<Route> {
    e.video
        .as_ref()
        .map(|v| Route::Watch(v.id as u64, None))
        .or_else(|| e.title.as_ref().map(|t| Route::Title(t.id as u64)))
}

fn day_card<'a>(b: &'a Browsing, e: &'a CalendarEntry, look: &Look) -> View<'a> {
    let aired = e.air_at <= now();
    let soon = !aired && e.air_at - now() < 3600;
    let image = e.backdrop.as_deref().or(e.poster.as_deref());
    let thumb = iced_widget::Stack::with_children([
        watch_image(
            b,
            image,
            crate::ui::tilt::art(image.and_then(|p| b.images.get(p)).cloned(), 8.0, 9.0 / 16.0, look).boxed(),
        ),
        container(badge(clock_time(e.air_at), Tone::Media, look)).padding(6).boxed(),
    ])
    .width(Fill);
    let mut content = column![
        thumb,
        column![
            truncate(e.show.as_str(), ty::S13, look.ink).font(MEDIUM),
            truncate(
                [Some(code(e.season, e.episode)), e.name.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · "),
                ty::XS,
                look.ink_3
            )
        ]
    ]
    .spacing(8)
    .width(Fill);
    if let Some(d) = &e.download {
        content = content.push(
            row![
                container(progress(Some(d.progress as f32), look.info, look)).width(Fill),
                text(format!("{}%", (d.progress * 100.0).floor() as i64), ty::XS2, look.info)
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .boxed(),
        );
    } else if soon {
        content = content.push(crate::ui::numbers::styled_ticker(
            &countdown(e.air_at),
            ty::XS2,
            MEDIUM,
            Some(look.warn),
            look,
        ));
    } else if (e.monitor != "NONE" || e.state == "DONE") && (aired || e.state == "DONE") {
        content = content.push(state(&e.state, look));
    }
    card_button(look)
        .hover(12.0)
        .content(container(content).padding(8))
        .on_press_maybe(entry_route(e).map(AppMessage::Navigate))
        .boxed()
}
