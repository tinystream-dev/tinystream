// SPDX-License-Identifier: AGPL-3.0-or-later

//! The Clips page and a clip's dialog, laid out as `routes/clips.tsx` lays them out.

use iced_core::Length::Fill;
use iced_core::{Alignment, Color, ContentFit, Padding};
use iced_widget::{Column, Element, Row, Stack, Widget, column, container, image, responsive, row, space};

use super::{Clip, Kind, List, Message, Open, Person, Scope, bytes, length, relative, space as gb, stamp};
use crate::app::{App, Message as AppMessage};
use crate::browsing::{art, watch_image};
use crate::route::Route;
use crate::ui::controls::{
    Tone, badge, button, checkbox, icon_button, owned_input, progress, segmented, spinner, toggle,
};
use crate::ui::effects::bone_rounded;
use crate::ui::icon::{Icon, icon};
use crate::ui::pressable::{Appearance, Status, pressable};
use crate::ui::squircle::{Paint, squircle};
use crate::ui::text::{self as type_, MEDIUM, SEMIBOLD, text, truncate};
use crate::ui::{Look, layout, media};

type View<'a> = Element<'a, AppMessage>;

fn msg(app: &App, message: Message) -> AppMessage {
    AppMessage::Clips(app.session.generation, message)
}

const SCOPES: [(Scope, &str); 3] =
    [(Scope::Mine, "Yours"), (Scope::Received, "Shared with you"), (Scope::Sent, "Shared by you")];
const KINDS: [(Kind, &str); 4] =
    [(Kind::All, "All"), (Kind::Clips, "Clips"), (Kind::Screenshots, "Screenshots"), (Kind::Quotes, "Quotes")];

pub fn page<'a>(app: &'a App, look: &Look) -> View<'a> {
    let c = &app.clips;
    let generation = app.session.generation;
    let aside = c.allowance().and_then(|you| usage(you, look));
    let mut filters = Row::new().spacing(16).align_y(Alignment::Center).push(segmented(
        c.scope,
        &SCOPES,
        move |s| AppMessage::Clips(generation, Message::Scope(s)),
        look,
    ));
    filters = filters.push(match c.book {
        None => segmented(c.kind, &KINDS, move |k| AppMessage::Clips(generation, Message::Kind(k)), look),
        Some(_) => {
            let from = c.list().and_then(|(list, ..)| list.clips.first()).map(|clip| clip.source.name.clone());
            squircle(
                row![
                    icon(Icon::Quote, 14.0).color(look.ink_3),
                    text(from.map_or("Quotes".into(), |name| format!("Quotes from {name}")), type_::S13, look.ink),
                    icon_button(Icon::X, "Show everything", Some(msg(app, Message::AllQuotes)), look),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                10.0,
                look,
            )
            .fill(look.panel)
            .padding(Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 10.0 })
            .height(32)
            .boxed()
        },
    });
    let content: View<'a> = match c.list() {
        None if c.list_error().is_some() => column![
            text(c.list_error().unwrap_or_default(), type_::SM, look.danger),
            button("Try again", look).on_press(msg(app, Message::Retry)).boxed(),
        ]
        .spacing(12)
        .boxed(),
        None => skeleton(look),
        Some((list, (scope, kind, book), _)) => {
            if list.clips.is_empty() {
                match kind {
                    Kind::Quotes => {
                        layout::empty("No quotes here", Some("Select some text in a book to quote it."), look)
                    },
                    Kind::All if book.is_none() => layout::empty(
                        match scope {
                            Scope::Mine => "No clips or screenshots yet",
                            Scope::Received => "Nothing here yet",
                            Scope::Sent => "You haven’t sent any clips",
                        },
                        None,
                        look,
                    ),
                    _ => layout::empty("Nothing here yet", None, look),
                }
            } else if book.is_some() || kind == Kind::Quotes {
                quotes_by_book(app, list, scope, look)
            } else {
                let l = *look;
                grid(list.clips.len(), move |i| card(app, &list.clips[i], scope, &l))
            }
        },
    };
    column![
        layout::page_title("Clips", aside, look),
        container(filters).padding(Padding { bottom: 24.0, ..Padding::ZERO }),
        content,
    ]
    .width(Fill)
    .boxed()
}

/// As many cards across as fit at 240 or more, 20 apart, rows 28 apart.
fn grid<'a>(count: usize, card: impl Fn(usize) -> View<'a> + 'a) -> View<'a> {
    responsive(move |size| {
        let columns = (((size.width + 20.0) / (240.0 + 20.0)).floor() as usize).max(1);
        let mut rows = Column::new().spacing(28).width(Fill);
        let mut cells = (0..count).map(&card).peekable();
        while cells.peek().is_some() {
            let mut line = Row::new().spacing(20).width(Fill);
            for _ in 0..columns {
                line = line.push(match cells.next() {
                    Some(cell) => container(cell).width(Fill).boxed(),
                    None => space().width(Fill).boxed(),
                });
            }
            rows = rows.push(line);
        }
        rows.boxed()
    })
    .height(iced_core::Length::Shrink)
    .boxed()
}

fn skeleton<'a>(look: &Look) -> View<'a> {
    let l = *look;
    grid(8, move |_| {
        column![aspect(bone_rounded(Fill, Fill, 14.0, &l)), bone_rounded(Fill, 16, 4.0, &l),].spacing(10).boxed()
    })
}

/// How much of your clip space is used of what you have (`Space`).
fn usage<'a>(you: &super::Allowance, look: &Look) -> Option<View<'a>> {
    if you.storage == 0 && you.limit == 0 {
        return (you.bytes > 0).then(|| {
            container(text(format!("{} rendered", bytes(you.bytes)), type_::S13, look.ink_3))
                .padding(Padding { bottom: 4.0, ..Padding::ZERO })
                .boxed()
        });
    }
    let storage = if you.storage > 0 { you.bytes as f64 / (you.storage as f64 * 1024.0 * 1024.0) } else { 0.0 };
    let count = if you.limit > 0 { you.rendered as f64 / you.limit as f64 } else { 0.0 };
    let share = storage.max(count) as f32;
    let mut words =
        if you.storage > 0 { format!("{} of {}", bytes(you.bytes), gb(you.storage)) } else { bytes(you.bytes) };
    if you.limit > 0 {
        words.push_str(&format!(" · {} of {} clips", you.rendered, you.limit));
    }
    Some(
        column![
            container(text(words, type_::XS, look.ink_3)).align_right(Fill),
            progress(Some(share), if share > 0.9 { look.info } else { look.ink }, look),
        ]
        .spacing(6)
        .width(208)
        .padding(Padding { bottom: 4.0, ..Padding::ZERO })
        .boxed(),
    )
}

/// A cover tint ("r g b") as a colour, `alpha` see-through.
fn tint(rgb: Option<&str>, alpha: f32) -> Option<Color> {
    let parts: Vec<f32> = rgb?.split_whitespace().filter_map(|n| n.parse().ok()).collect();
    (parts.len() == 3).then(|| Color::from_rgba(parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0, alpha))
}

/// People's faces overlapping in a row (`-space-x-1.5`), each ringed in the page's colour.
fn faces<'a>(app: &'a App, people: impl Iterator<Item = &'a Person>, size: f32, look: &Look) -> View<'a> {
    let l = *look;
    let layers: Vec<View<'a>> = people
        .enumerate()
        .map(|(i, person)| {
            let ring = container(avatar(app, person, size, look)).padding(2).style(move |_| container::Style {
                background: Some(l.canvas.into()),
                border: iced_core::Border::default().rounded(size),
                ..container::Style::default()
            });
            container(ring).padding(Padding { left: i as f32 * (size - 6.0), ..Padding::ZERO }).boxed()
        })
        .collect();
    if layers.is_empty() {
        return space().boxed();
    }
    Stack::with_children(layers).boxed()
}

fn avatar<'a>(app: &'a App, person: &'a Person, size: f32, look: &Look) -> View<'a> {
    let picture = person.avatar.as_deref().and_then(|path| app.browsing.images.get(path));
    watch_image(&app.browsing, person.avatar.as_deref(), media::avatar(&person.username, picture, size, None, look))
}

/// The small label over a cover's corner.
fn corner<'a>(glyph: Option<Icon>, words: String, look: &Look) -> View<'a> {
    let ink = look.media_ink;
    let mut content = Row::new().spacing(4).align_y(Alignment::Center);
    if let Some(glyph) = glyph {
        content = content.push(icon(glyph, 12.0).color(ink).boxed());
    }
    content = content.push(text(words, type_::XS2, ink).font(MEDIUM).boxed());
    squircle(content, 6.0, look).fill(look.media_shade.scale_alpha(0.65)).padding([2, 6]).boxed()
}

fn card<'a>(app: &'a App, c: &'a Clip, scope: Scope, look: &Look) -> View<'a> {
    let l = *look;
    let subtitle = match scope {
        Scope::Received => format!("{} · {}", c.owner.username, relative(c.shared_at.unwrap_or(c.created_at))),
        _ if !c.name.is_empty() => c.source_line(),
        _ => relative(c.created_at),
    };
    let mut caption = Row::new().spacing(10).align_y(Alignment::Start);
    if scope == Scope::Received {
        caption = caption.push(avatar(app, &c.owner, 26.0, look));
    }
    caption = caption.push(
        column![truncate(c.display_name(), type_::SM, l.ink).font(MEDIUM), truncate(subtitle, type_::XS, l.ink_3)]
            .width(Fill)
            .boxed(),
    );
    if scope == Scope::Sent {
        let people = c.recipients.iter().take(3).map(|r| &r.user);
        caption = caption
            .push(container(faces(app, people, 20.0, look)).padding(Padding { top: 2.0, ..Padding::ZERO }).boxed());
    }
    let id = c.id as u64;
    pressable(
        column![
            aspect(Stack::with_children(layers_of(app, c, look)).boxed()),
            container(caption).padding(Padding { top: 10.0, ..Padding::ZERO })
        ],
        14.0,
        look,
        move |_| Appearance { paint: Paint::NONE, ink: l.ink },
    )
    .width(Fill)
    .on_press(msg(app, Message::Open(id as i64)))
    .boxed()
}

/// What sits in a card's 16:9 frame: the cover, its label, and a link mark.
fn layers_of<'a>(app: &'a App, c: &'a Clip, look: &Look) -> Vec<View<'a>> {
    let l = *look;
    let label = if c.quote.is_some() {
        corner(Some(Icon::Quote), "Quote".into(), look)
    } else if c.screenshot {
        corner(Some(Icon::Camera), stamp(c.start), look)
    } else {
        corner(None, length(c.end - c.start), look)
    };
    let mut layers =
        vec![cover(app, c, look), container(label).align_right(Fill).align_bottom(Fill).padding(8).boxed()];
    if c.public && c.link_live {
        let link = container(icon(Icon::Link2, 12.0).color(l.media_ink)).center(24).style(move |_| container::Style {
            background: Some(l.media_shade.scale_alpha(0.6).into()),
            border: iced_core::Border::default().rounded(12),
            ..container::Style::default()
        });
        layers.push(container(link).align_right(Fill).padding(8).boxed());
    }
    layers
}

/// `content` at 16:9, as wide as it's given.
fn aspect<'a>(content: View<'a>) -> View<'a> {
    layout::aspect(content, 16.0 / 9.0)
}

/// The clip's picture, or where its render is at (`Cover`).
fn cover<'a>(app: &'a App, c: &'a Clip, look: &Look) -> View<'a> {
    let l = *look;
    if let Some(quote) = &c.quote {
        let fill = tint(c.source.book.as_ref().and_then(|b| b.cover_tint.as_deref()), 0.16).unwrap_or(l.raised);
        return squircle(
            container(text(quote.text.as_str(), type_::S15, l.ink)).height(Fill).center_y(Fill).clip(true),
            14.0,
            look,
        )
        .fill(fill)
        .edge()
        .padding([16, 20])
        .width(Fill)
        .height(Fill)
        .boxed();
    }
    if c.ready()
        && let Some(poster) = c.poster.as_deref()
    {
        return art(&app.browsing, Some(poster), 14.0, look);
    }
    let glyph = if c.screenshot { Icon::Camera } else { Icon::Scissors };
    let inside: View<'a> = match c.state.as_str() {
        "RENDERING" | "QUEUED" => column![
            icon(glyph, 20.0).color(l.highlight),
            container(progress((c.state == "RENDERING").then(|| c.progress.unwrap_or(0.0) as f32), l.ink, look))
                .width(iced_core::Length::FillPortion(2)),
        ]
        .spacing(12)
        .align_x(Alignment::Center)
        .width(Fill)
        .padding([0, 40])
        .boxed(),
        "FAILED" => row![icon(Icon::TriangleAlert, 14.0).color(l.danger), text("Didn’t render", type_::XS, l.danger)]
            .spacing(6)
            .align_y(Alignment::Center)
            .boxed(),
        _ => text("Renders when opened", type_::XS, l.ink_3).boxed(),
    };
    squircle(container(inside).center(Fill).padding(16), 14.0, look)
        .fill(l.raised)
        .edge()
        .width(Fill)
        .height(Fill)
        .boxed()
}

/// Quotes under the books they're from, in the order they're in each (`QuotesByBook`).
fn quotes_by_book<'a>(app: &'a App, list: &'a List, scope: Scope, look: &Look) -> View<'a> {
    let l = *look;
    let mut books: Vec<(String, Vec<&'a Clip>)> = Vec::new();
    for c in &list.clips {
        let key = match &c.source.book {
            Some(book) => format!("b{}", book.id),
            None => format!("n{}", c.source.name),
        };
        match books.iter_mut().find(|(k, _)| *k == key) {
            Some((_, clips)) => clips.push(c),
            None => books.push((key, vec![c])),
        }
    }
    let mut sections = Column::new().spacing(48).width(Fill);
    for (_, mut clips) in books {
        let first = clips[0];
        let book = first.source.book.as_ref();
        let cover: View<'a> = match book {
            Some(b) => {
                let cover = container(art(&app.browsing, b.cover.as_deref(), 6.0, look)).width(44).height(64);
                pressable(cover, 6.0, look, move |_| Appearance { paint: Paint::NONE, ink: l.ink })
                    .on_press(AppMessage::Navigate(Route::Book(b.id as u64)))
                    .boxed()
            },
            None => squircle(container(icon(Icon::BookOpen, 16.0).color(l.ink_3)).center(Fill), 8.0, look)
                .fill(l.raised)
                .width(44)
                .height(64)
                .boxed(),
        };
        let count = format!("{} {}", clips.len(), if clips.len() == 1 { "quote" } else { "quotes" });
        let detail = [first.quote.as_ref().and_then(|q| q.author.clone()), Some(count)]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        let mut heading = Row::new().spacing(14).align_y(Alignment::Center).push(cover).push(
            column![
                truncate(first.source.name.as_str(), type_::S15, l.ink).font(SEMIBOLD),
                truncate(detail, type_::S13, l.ink_3)
            ]
            .width(Fill)
            .boxed(),
        );
        if let Some(b) = book {
            heading = heading
                .push(button("Read", look).small().on_press(AppMessage::Navigate(Route::Read(b.id as u64))).boxed());
        }
        clips.sort_by(|a, b| a.start.total_cmp(&b.start));
        let cells: Vec<View<'a>> = clips.into_iter().map(|c| quote_card(app, c, scope, look)).collect();
        let mut pairs = Column::new().spacing(12).width(Fill);
        let mut cells = cells.into_iter().peekable();
        while cells.peek().is_some() {
            let left = cells.next().unwrap_or_else(|| space().boxed());
            let right = cells.next().unwrap_or_else(|| space().width(Fill).boxed());
            pairs = pairs.push(row![container(left).width(Fill), container(right).width(Fill)].spacing(12));
        }
        sections = sections.push(column![heading, pairs].spacing(16));
    }
    sections.boxed()
}

fn quote_card<'a>(app: &'a App, c: &'a Clip, scope: Scope, look: &Look) -> View<'a> {
    let l = *look;
    let quote = text(c.quote.as_ref().map_or("", |q| q.text.as_str()), type_::Size { px: 15.0, line: 24.4 }, l.ink)
        .ellipsis(iced_core::text::Ellipsis::End);
    let mut body = Column::new().spacing(8).push(layout::at_most(quote, 24.4 * 6.0));
    if !c.name.is_empty() {
        body = body.push(text(c.name.as_str(), type_::S13, l.ink_2).boxed());
    }
    let when = relative(if scope == Scope::Received { c.shared_at.unwrap_or(c.created_at) } else { c.created_at });
    let detail = [(!c.mine).then(|| c.owner.username.clone()), c.source.label.clone(), Some(when)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let mut line = Row::new().spacing(6).align_y(Alignment::Center);
    if !c.mine {
        line = line.push(avatar(app, &c.owner, 16.0, look));
    }
    line = line.push(truncate(detail, type_::XS, l.ink_3).boxed());
    if c.public && c.link_live {
        line = line.push(icon(Icon::Link2, 12.0).color(l.ink_3).boxed());
    }
    body = body.push(line.boxed());
    let id = c.id as u64;
    pressable(body.padding([14, 16]).width(Fill), 14.0, look, move |status| Appearance {
        paint: Paint {
            fill: Some(if matches!(status, Status::Hovered | Status::Pressed) { l.hover } else { l.panel }),
            edge: true,
            ..Paint::NONE
        },
        ink: l.ink,
    })
    .width(Fill)
    .on_press(msg(app, Message::Open(id as i64)))
    .boxed()
}

/// The open clip's dialog (`ClipDialog`), or what it asks first.
pub fn dialog<'a>(app: &'a App, look: &Look) -> Option<View<'a>> {
    if !matches!(app.history.current().route, Route::Clip(_)) || app.clips.expanded {
        return None;
    }
    let open = app.clips.open.as_ref()?;
    let close = msg(app, Message::Close);
    if open.confirm
        && let Some(Some(c)) = open.clip()
    {
        let title = if c.quote.is_some() {
            "Delete this quote?"
        } else if c.screenshot {
            "Delete this screenshot?"
        } else {
            "Delete this clip?"
        };
        let body = (!c.recipients.is_empty()).then_some("Also removes it for everyone it was sent to.");
        let generation = app.session.generation;
        return Some(crate::ui::overlay::confirm(
            title,
            body,
            "Delete",
            true,
            move |yes| AppMessage::Clips(generation, Message::Confirm(yes)),
            look,
        ));
    }
    let content: View<'a> = match open.clip() {
        _ if open.failed() => text("This clip isn’t here anymore", type_::S15, look.ink).font(MEDIUM).boxed(),
        Some(None) => text("This clip isn’t here anymore", type_::S15, look.ink).font(MEDIUM).boxed(),
        None => aspect(container(spinner(28.0, look)).center(Fill).boxed()),
        Some(Some(c)) => clip_dialog(app, open, c, look),
    };
    Some(crate::ui::overlay::dialog(content, 768.0, close, look))
}

fn clip_dialog<'a>(app: &'a App, open: &'a Open, c: &'a Clip, look: &Look) -> View<'a> {
    let l = *look;
    let media: View<'a> = if let Some(quote) = &c.quote {
        let fill = tint(c.source.book.as_ref().and_then(|b| b.cover_tint.as_deref()), 0.14).unwrap_or(l.panel);
        squircle(
            iced_widget::scrollable(text(quote.text.as_str(), type_::Size { px: 17.0, line: 27.6 }, l.ink))
                .height(iced_core::Length::Shrink),
            14.0,
            look,
        )
        .fill(fill)
        .padding([20, 24])
        .width(Fill)
        .boxed()
    } else {
        aspect(media_view(app, open, c, look))
    };

    let title: View<'a> = if c.can_manage {
        let placeholder = if c.quote.is_some() {
            "Add a note".to_owned()
        } else {
            Clip { name: String::new(), source: c.source.clone(), ..Clip::default() }.display_name()
        };
        owned_input("clip-title", open.title.clone(), move |s| msg(app, Message::Title(s)), look)
            .placeholder(placeholder)
            .on_submit(msg(app, Message::SaveTitle))
            .boxed()
    } else {
        text(c.display_name(), type_::LG, l.ink).font(SEMIBOLD).boxed()
    };
    let source: View<'a> = match (&c.source.title, &c.source.book) {
        (Some(title), _) => link(c.source_line(), AppMessage::Navigate(Route::Title(title.id as u64)), look),
        (None, Some(book)) => link(c.source_line(), AppMessage::Navigate(Route::Book(book.id as u64)), look),
        _ => text(c.source_line(), type_::S13, l.ink_2).boxed(),
    };
    let when = match &c.quote {
        Some(quote) => quote.author.as_ref().map(|author| format!("· {author}")),
        None if c.screenshot => Some(format!("· {}", stamp(c.start))),
        None => Some(format!("· {}–{}", stamp(c.start), stamp(c.end))),
    };
    let mut source_line = Row::new().spacing(4).align_y(Alignment::Center).push(source);
    if let Some(when) = when {
        source_line = source_line.push(text(when, type_::S13, l.ink_3).boxed());
    }
    let what = if c.quote.is_some() {
        "Quote".to_owned()
    } else if c.screenshot {
        match (c.width, c.height) {
            (Some(w), Some(h)) => format!("{w}×{h} PNG"),
            _ => "PNG".into(),
        }
    } else {
        match (c.width, c.height) {
            (Some(w), Some(h)) => format!("{w}×{h} · {} fps", c.fps.unwrap_or(0.0).round()),
            _ => format!("{}p", c.quality.height),
        }
    };
    let size = match c.bytes {
        Some(n) if c.ready() && n > 0 => format!(" · {}", bytes(n)),
        _ => String::new(),
    };
    let mut meta = Row::new().spacing(6).align_y(Alignment::Center);
    if !c.mine {
        meta = meta.push(avatar(app, &c.owner, 16.0, look));
        meta = meta.push(text(format!("{} ·", c.owner.username), type_::XS, l.ink_3).boxed());
    }
    meta = meta.push(text(format!("{what}{size} · {}", relative(c.created_at)), type_::XS, l.ink_3).boxed());
    let mut info = column![title, source_line, meta].spacing(4).width(Fill);
    if c.source.status == "CHANGED" && c.can_manage && !c.screenshot && c.quote.is_none() {
        info = info.push(
            row![
                icon(Icon::TriangleAlert, 14.0).color(l.warn_soft),
                text("Source video was replaced; check the range.", type_::XS, l.warn_soft)
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }

    let mut actions = Row::new().spacing(6).align_y(Alignment::Center);
    if c.quote.is_some() {
        actions = actions
            .push(button("Copy", look).primary().icon(Icon::Copy).on_press(msg(app, Message::CopyQuote)).boxed());
    }
    if let Some(book) = &c.source.book {
        actions = actions.push(icon_button(
            Icon::BookOpen,
            "Read from here",
            Some(AppMessage::Navigate(Route::Read(book.id as u64))),
            look,
        ));
    }
    if c.ready() && c.quote.is_none() {
        actions = actions.push(
            button("Download", look).primary().icon(Icon::Download).on_press(msg(app, Message::Download)).boxed(),
        );
    }
    if let Some(video) = &c.source.video {
        actions = actions.push(icon_button(
            Icon::Play,
            "Watch from here",
            Some(AppMessage::Navigate(Route::Watch(video.id as u64, Some(c.start as u32)))),
            look,
        ));
    }
    let can_clip = app.browsing.can().clip
        && app.browsing.data(&crate::browsing::Key::Shell).and_then(|d| d.server.as_ref()).is_some_and(|s| s.clips);
    let editable = !c.screenshot && c.can_manage && can_clip && c.source.video.is_some() && c.source.status != "GONE";
    if editable && let Some(video) = &c.source.video {
        actions = actions.push(icon_button(
            Icon::Pencil,
            "Change the range, tracks or quality",
            Some(AppMessage::EditClip(video.id, c.id)),
            look,
        ));
    }
    actions = actions.push(if c.can_manage {
        icon_button(Icon::Trash2, "Delete", Some(msg(app, Message::Delete)), look)
    } else {
        icon_button(Icon::X, "Remove from your list", Some(msg(app, Message::Hide)), look)
    });

    let mut body = column![media, row![info, actions].spacing(12).align_y(Alignment::Start)].spacing(20).width(Fill);
    if c.can_manage {
        body = body.push(manage(app, open, c, look));
    }
    body.boxed()
}

fn link<'a>(label: String, message: AppMessage, look: &Look) -> View<'a> {
    let l = *look;
    pressable(text(label, type_::S13, l.ink_2), 4.0, look, move |status| Appearance {
        paint: Paint::NONE,
        ink: if matches!(status, Status::Hovered | Status::Pressed) { l.ink } else { l.ink_2 },
    })
    .on_press(message)
    .boxed()
}

/// The picture, the player, or where the render is at, in the dialog's 16:9 frame.
fn media_view<'a>(app: &'a App, open: &'a Open, c: &'a Clip, look: &Look) -> View<'a> {
    let l = *look;
    let shade = l.media_shade;
    let frame = |content: View<'a>| {
        container(content)
            .width(Fill)
            .height(Fill)
            .style(move |_| container::Style {
                background: Some(shade.into()),
                border: iced_core::Border::default().rounded(l.radius(14.0)),
                ..container::Style::default()
            })
            .boxed()
    };
    if c.ready() && c.screenshot {
        let path = format!("{}?v={}", c.file, c.rendered_at.unwrap_or_default());
        let picture: View<'a> = match app.browsing.images.get(&path) {
            Some(handle) => image(handle.clone()).width(Fill).height(Fill).content_fit(ContentFit::Contain).boxed(),
            None => container(spinner(28.0, look)).center(Fill).boxed(),
        };
        return frame(watch_image(&app.browsing, Some(&path), picture));
    }
    if c.ready()
        && let Some(player) = &open.player
    {
        let id = open.id;
        let generation = app.session.generation;
        if player.pip {
            return frame(
                container(
                    button("Return from picture in picture", look)
                        .on_press(AppMessage::Clips(
                            generation,
                            Message::Player(id, crate::player::clip_player::Message::PictureInPicture),
                        ))
                        .boxed(),
                )
                .center(Fill)
                .boxed(),
            );
        }
        let poster = c.poster.as_deref().and_then(|p| app.browsing.images.get(p));
        let view = player
            .view(poster, app.chrome.fullscreen, look)
            .map(move |message| AppMessage::Clips(generation, Message::Player(id, message)));
        return frame(watch_image(&app.browsing, c.poster.as_deref(), view.boxed()));
    }
    let glyph = if c.screenshot { Icon::Camera } else { Icon::Scissors };
    let inside: View<'a> = if c.state == "FAILED" {
        let mut failed = column![
            icon(Icon::TriangleAlert, 24.0).color(l.danger),
            text(c.error.clone().unwrap_or_else(|| "It didn’t render.".into()), type_::SM, l.ink_2).center(),
        ]
        .spacing(12)
        .align_x(Alignment::Center)
        .width(Fill.max(384));
        if c.can_manage && c.source.status == "OK" {
            failed = failed.push(
                container(button("Try again", look).icon(Icon::RotateCw).on_press(msg(app, Message::Rerender)).boxed())
                    .padding(Padding { top: 4.0, ..Padding::ZERO })
                    .boxed(),
            );
        }
        failed.boxed()
    } else {
        let words = match c.state.as_str() {
            "QUEUED" => "Waiting its turn to render".to_owned(),
            "RENDERING" => format!("Rendering · {}%", (c.progress.unwrap_or(0.0) * 100.0).round()),
            _ => "Getting it ready".to_owned(),
        };
        column![
            icon(glyph, 24.0).color(l.highlight),
            container(progress((c.state == "RENDERING").then(|| c.progress.unwrap_or(0.0) as f32), l.ink, look))
                .width(240),
            text(words, type_::S13, l.ink_3),
        ]
        .spacing(14)
        .align_x(Alignment::Center)
        .boxed()
    };
    frame(container(inside).center(Fill).padding(24).boxed())
}

/// The link, and who it was sent to, for whoever owns it.
fn manage<'a>(app: &'a App, open: &'a Open, c: &'a Clip, look: &Look) -> View<'a> {
    let l = *look;
    let mut section = Column::new().spacing(16).width(Fill);
    let can_link = app.clips.allowance().is_some_and(|you| you.can_link) || c.public;
    if can_link {
        let mut words = Column::new().width(Fill).push(text("Anyone with the link", type_::SM, l.ink).boxed());
        if c.public && !c.link_live {
            words = words.push(text("Links are off for the owner", type_::XS, l.ink_3).boxed());
        }
        let mut line = Row::new()
            .spacing(12)
            .align_y(Alignment::Center)
            .push(toggle(c.public, |on| msg(app, Message::Public(on)), look))
            .push(words.boxed());
        if c.public && c.link.is_some() && c.link_live {
            line = line.push(
                button("Copy link", look)
                    .small()
                    .icon(if open.copied { Icon::Check } else { Icon::Copy })
                    .on_press(msg(app, Message::CopyLink))
                    .boxed(),
            );
        }
        if c.public && !c.link_live {
            line = line.push(badge("Off", Tone::Warn, look));
        }
        section = section.push(line.boxed());
    }
    let mut sent = Column::new().spacing(8).push(text("Sent to", type_::XS, l.ink_3).boxed());
    if !c.recipients.is_empty() {
        let mut chips = Row::new().spacing(6);
        for recipient in &c.recipients {
            let person = &recipient.user;
            let mut chip = Row::new()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(avatar(app, person, 20.0, look))
                .push(text(person.username.as_str(), type_::S13, l.ink).boxed());
            if recipient.hidden {
                chip = chip.push(text("removed it", type_::XS2, l.ink_3).boxed());
            }
            let id = person.id;
            chip = chip.push(icon_button(Icon::X, "Take it back", Some(msg(app, Message::Unshare(id))), look));
            chips = chips.push(
                squircle(chip, 10.0, look)
                    .fill(l.panel)
                    .padding(Padding { top: 0.0, right: 2.0, bottom: 0.0, left: 6.0 })
                    .height(32)
                    .boxed(),
            );
        }
        sent = sent.push(chips.wrap().boxed());
    }
    sent = sent.push(send_to(app, open, look));
    section = section.push(sent.boxed());
    column![crate::ui::controls::divider(look), section].spacing(16).boxed()
}

/// Picks people on this server to send the clip to (`SendTo`).
fn send_to<'a>(app: &'a App, open: &'a Open, look: &Look) -> View<'a> {
    let l = *look;
    let me = app.browsing.viewer().map(|v| v.id);
    let Some(people) = app.clips.people(me) else { return space().boxed() };
    if people.is_empty() {
        return space().boxed();
    }
    let mut line = Row::new()
        .spacing(8)
        .align_y(Alignment::Center)
        .push(button("Send to", look).icon(Icon::Send).on_press(msg(app, Message::Picker(!open.picking))).boxed());
    line = line.push(faces(app, open.send_to.iter().take(6), 24.0, look));
    if !open.send_to.is_empty() {
        let who = if open.send_to.len() == 1 {
            open.send_to[0].username.clone()
        } else {
            format!("{} people", open.send_to.len())
        };
        line = line.push(truncate(who, type_::XS, l.ink_3).boxed());
        line = line.push(button("Send", look).primary().small().on_press(msg(app, Message::Send)).boxed());
    }
    let mut picker = Column::new().spacing(8).push(line.boxed());
    if open.picking {
        let mut list = Column::new().spacing(2);
        for person in people {
            let on = open.send_to.iter().any(|p| p.id == person.id);
            let row = row![
                checkbox(on, Some(msg(app, Message::Pick(person.clone(), !on))), look),
                avatar(app, person, 22.0, look),
                truncate(person.username.as_str(), type_::S13, l.ink).width(Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            list = list.push(
                pressable(row, 8.0, look, move |status| Appearance {
                    paint: Paint {
                        fill: matches!(status, Status::Hovered | Status::Pressed).then_some(l.hover),
                        ..Paint::NONE
                    },
                    ink: l.ink,
                })
                .width(Fill)
                .padding([6, 8])
                .on_press(msg(app, Message::Pick(person.clone(), !on))),
            );
        }
        picker = picker.push(
            crate::ui::controls::panel(layout::at_most(iced_widget::scrollable(list), 288.0), 14.0, look)
                .padding(6)
                .width(256)
                .boxed(),
        );
    }
    picker.boxed()
}

/// The open clip's player filling the window, while it's full screen.
pub fn expanded<'a>(app: &'a App, look: &Look) -> Option<View<'a>> {
    if app.clips.pip() || !app.clips.expanded || !matches!(app.history.current().route, Route::Clip(_)) {
        return None;
    }
    let open = app.clips.open.as_ref()?;
    let player = open.player.as_ref()?;
    let (id, generation) = (open.id, app.session.generation);
    let poster = open.clip().flatten().and_then(|c| c.poster.as_deref()).and_then(|p| app.browsing.images.get(p));
    Some(
        player
            .view(poster, true, look)
            .map(move |message| AppMessage::Clips(generation, Message::Player(id, message)))
            .boxed(),
    )
}
