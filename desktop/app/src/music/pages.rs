// SPDX-License-Identifier: AGPL-3.0-or-later

//! A music library's albums, artists, songs and playlists, and the pages of each (`Library.tsx`,
//! `components.tsx`, and the album, artist and playlist routes).

use iced_core::Length::Fill;
use iced_core::{Alignment, Color, Padding};
use iced_widget::{Column, Element, Row, Stack, Widget, column, container, responsive, row, space, stack};

use super::model::{self, Track};
use super::{Card, Dialog, Message, Music, Sort, Tab};
use crate::images::Picture;
use crate::route::Route;
use crate::ui::controls::{Tone, badge, button, chip, input, menu_item, segmented, tip};
use crate::ui::effects::{self, Ambient};
use crate::ui::icon::{Icon, icon};
use crate::ui::pressable::{Appearance, Status, pressable};
use crate::ui::squircle::{Paint, squircle};
use crate::ui::text::{self as ty, MEDIUM, SEMIBOLD, text, truncate};
use crate::ui::{Look, layout, media};

type View<'a> = Element<'a, Message>;

const HEADING: ty::Size = ty::Size { px: 44.0, line: 48.4 };
const NAME: ty::Size = ty::Size { px: 48.0, line: 52.8 };

/// The music page for the route: a library, an album, an artist or a playlist. `counts` are the
/// library's albums and songs.
pub fn page<'a>(
    music: &'a Music,
    kept: &crate::offline::Downloads,
    counts: Option<(i64, i64)>,
    look: &Look,
) -> View<'a> {
    let detail = music.detail.as_ref();
    let (content, glow) = match &music.route {
        Some(Route::Album(_)) => (album(music, kept, look), Some((640.0, (0.7, 0.6), (0.15, 0.0)))),
        Some(Route::Artist(_)) => (artist(music, look), Some((576.0, (0.6, 0.6), (0.5, 0.0)))),
        Some(Route::Playlist(_)) => (playlist(music, kept, look), None),
        Some(Route::Library(name)) => (library(music, name, counts, look), None),
        _ => (space().boxed(), None),
    };
    let page = layout::page(content, false);
    let tint = detail.and_then(|d| d.cover_tint.as_deref()).and_then(media::parse_tint);
    match glow.zip(tint) {
        Some(((height, size, center), color)) => {
            let glow = effects::ambient(Ambient { color: color.scale_alpha(0.24), size, center, reach: 0.7 });
            Stack::with_children([page])
                .push_under(container(glow).width(Fill).height(height).boxed())
                .width(Fill)
                .boxed()
        },
        None => page,
    }
}

/// What's asked over the page: a new playlist's name, or whether to delete one.
pub fn dialog<'a>(music: &'a Music, look: &Look) -> Option<View<'a>> {
    match music.dialog.as_ref()? {
        Dialog::AddTo(tracks) => {
            let new = Message::Dialog(Some(Dialog::NewPlaylist(tracks.clone())));
            let mut rows = column![menu_item(
                Some(Icon::Plus),
                ty::inherit("New playlist…", ty::S13),
                None,
                false,
                Some(new),
                look
            )];
            for (id, name) in &music.lists {
                let label = ty::inherit(name.as_str(), ty::S13).wrapping(iced_core::text::Wrapping::None);
                let add = Message::AddTo(*id, name.clone(), tracks.clone());
                rows = rows.push(menu_item(Some(Icon::ListPlus), label, None, false, Some(add), look));
            }
            let content = column![
                text("Add to playlist", ty::S15, look.ink).font(MEDIUM),
                iced_widget::scrollable(rows).height(iced_core::Length::Shrink.max(288.0))
            ]
            .spacing(12);
            Some(crate::ui::overlay::dialog(content, 320.0, Message::Dialog(None), look))
        },
        Dialog::NewPlaylist(_) => {
            let named = !music.name.trim().is_empty();
            let content = column![
                text("New playlist", ty::S15, look.ink).font(MEDIUM),
                row![
                    container(
                        input("new-playlist", &music.name, "Name", Message::Name, look)
                            .on_submit(Message::CreatePlaylist)
                            .boxed()
                    )
                    .width(Fill),
                    button("Create", look).primary().on_press_maybe(named.then_some(Message::CreatePlaylist)).boxed()
                ]
                .spacing(8)
            ]
            .spacing(16);
            Some(crate::ui::overlay::dialog(content, 384.0, Message::Dialog(None), look))
        },
        Dialog::DeletePlaylist(id, title) => {
            let id = *id;
            Some(crate::ui::overlay::confirm(
                title,
                Some("The songs stay; only the list goes."),
                "Delete",
                true,
                move |yes| if yes { Message::DeletePlaylist(id) } else { Message::Dialog(None) },
                look,
            ))
        },
    }
}

fn library<'a>(music: &'a Music, name: &'a str, counts: Option<(i64, i64)>, look: &Look) -> View<'a> {
    let aside = counts.map(|(albums, songs)| {
        container(text(format!("{albums} albums · {songs} songs"), ty::SM, look.ink_3))
            .padding(Padding { bottom: 4.0, ..Padding::ZERO })
            .boxed()
    });
    let tabs = segmented(
        music.tab,
        &[(Tab::Albums, "Albums"), (Tab::Artists, "Artists"), (Tab::Songs, "Songs"), (Tab::Playlists, "Playlists")],
        Message::Tab,
        look,
    );
    let mut page = column![layout::page_title(name, aside, look), container(tabs).padding(bottom(24.0))].width(Fill);
    if let Some(error) = &music.error {
        page = page.push(container(text(error, ty::SM, look.danger)).padding(bottom(16.0)).boxed());
    }
    let l = *look;
    let body = match music.tab {
        Tab::Albums => {
            let sorts = Row::with_children(
                [
                    (Sort::Name, "A–Z"),
                    (Sort::Artist, "Artist"),
                    (Sort::Newest, "Added"),
                    (Sort::Year, "Year"),
                    (Sort::Recent, "Played"),
                ]
                .map(|(sort, label)| chip(label, music.sort == sort, Message::Sort(sort), look)),
            )
            .spacing(4);
            let albums = if music.cards.is_empty() {
                waiting(
                    music,
                    if music.sort == Sort::Recent {
                        "You haven't played anything here yet"
                    } else {
                        "No albums here yet"
                    },
                    look,
                )
            } else {
                layout::poster_grid(music.cards.len(), move |i| {
                    let album = &music.cards[i];
                    let caption = match (music.sort, album.year) {
                        (Sort::Year, Some(year)) => format!("{} · {year}", album.artist),
                        _ => album.artist.clone(),
                    };
                    album_tile(music, album, caption, &l)
                })
            };
            column![container(sorts).padding(bottom(28.0)), albums].boxed()
        },
        Tab::Artists if music.cards.is_empty() => waiting(music, "No artists here yet", look),
        Tab::Artists => layout::poster_grid(music.cards.len(), move |i| artist_tile(music, &music.cards[i], &l)),
        Tab::Songs => {
            let search = container(input("song-search", &music.query, "Find a song", Message::Query, look).boxed())
                .width(Fill.max(448))
                .padding(bottom(20.0));
            let songs = if music.songs.is_empty() && !music.loading {
                if music.query.is_empty() {
                    layout::empty("No songs here yet", None, look)
                } else {
                    layout::empty(format!("Nothing called “{}”", music.query), None, look)
                }
            } else {
                track_list(music, &music.songs, List { album: true, ..List::default() }, look)
            };
            column![search, songs].boxed()
        },
        Tab::Playlists => layout::poster_grid(music.cards.len() + 1, move |i| match i {
            0 => new_playlist(&l),
            i => playlist_tile(music, &music.cards[i - 1], &l),
        }),
    };
    page.push(body).boxed()
}

/// Nothing to show yet: a gap while it loads, then `empty` if there's still nothing.
fn waiting<'a>(music: &Music, empty: &'a str, look: &Look) -> View<'a> {
    if music.loading { space().height(240).boxed() } else { layout::empty(empty, None, look) }
}

fn album<'a>(music: &'a Music, kept: &crate::offline::Downloads, look: &Look) -> View<'a> {
    let Some(album) = &music.detail else { return missing(music, "This album isn't here anymore", look) };
    let tracks = &music.songs;
    let best =
        tracks.iter().max_by_key(|t| t.bit_depth.unwrap_or(0) as u64 * 1_000_000 + t.sample_rate.unwrap_or(0) as u64);
    let year = album
        .original_date
        .as_deref()
        .or(album.release_date.as_deref())
        .map(|d| d.chars().take(4).collect::<String>())
        .or(album.year.map(|y| y.to_string()));
    let here = music.current().is_some_and(|e| e.track.album_id == Some(album.id));

    let mut meta = Row::new().spacing(12).align_y(Alignment::Center);
    meta = meta.push(if album.artists.is_empty() || album.compilation {
        text(&album.artist, ty::SM, look.ink).font(MEDIUM).boxed()
    } else {
        Row::with_children(album.artists.iter().enumerate().map(|(i, a)| {
            let name = if i == 0 { a.name.clone() } else { format!(", {}", a.name) };
            let ink = look.ink;
            pressable(text(name, ty::SM, ink).font(MEDIUM), 0.0, look, move |_| Appearance { paint: Paint::NONE, ink })
                .on_press(Message::Go(Route::Artist(a.id as u64)))
                .boxed()
        }))
        .boxed()
    });
    if let Some(year) = year {
        meta = meta.push(text(year, ty::SM, look.ink_2).boxed());
    }
    meta = meta.push(
        text(
            format!("{} song{}, {}", album.track_count, plural(album.track_count), model::length(album.duration)),
            ty::SM,
            look.ink_2,
        )
        .boxed(),
    );
    if let Some(best) = best {
        meta = meta.push(quality(best, look));
    }
    if !album.genres.is_empty() {
        meta = meta.push(
            text(album.genres.iter().take(3).cloned().collect::<Vec<_>>().join(", "), ty::SM, look.ink_3).boxed(),
        );
    }
    let actions = row![
        button(
            if here && music.player.playing {
                "Pause"
            } else if here {
                "Resume"
            } else {
                "Play"
            },
            look
        )
        .primary()
        .large()
        .icon(Icon::Play)
        .on_press(if here { Message::Toggle } else { Message::PlayFrom(0, tracks.len()) })
        .boxed(),
        button("Shuffle", look).large().icon(Icon::Shuffle).on_press(Message::ShuffleSongs).boxed(),
        button("Listen together", look).large().plain().icon(Icon::Headphones).on_press(Message::StartRoom).boxed(),
        star(album.starred, Message::StarDetail(!album.starred), look),
        menu(tracks, None, look),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .extend(download(kept, crate::offline::Key::Album(album.id), look));
    let info = column![
        text(kind(&album.release_types, album.compilation).to_uppercase(), ty::XS, look.ink_3).font(MEDIUM),
        container(text(&album.name, HEADING, look.ink).font(SEMIBOLD)).padding(top(6.0)),
        container(meta.wrap().vertical_spacing(4.0)).padding(top(10.0)),
        container(actions).padding(top(24.0)),
    ];
    let artwork =
        squircle(container(cover(music, album.cover.as_deref(), 20.0, 64.0, look)).width(240).height(240), 20.0, look)
            .lift();
    let mut page = column![
        hero(artwork.boxed(), info.boxed()),
        container(track_list(
            music,
            tracks,
            List { numbers: true, discs: &album.disc_titles, ..List::default() },
            look
        ))
        .padding(top(40.0))
    ]
    .width(Fill);

    let formats = tracks.iter().map(model::quality).collect::<std::collections::BTreeSet<_>>();
    let notes = album
        .release_date
        .iter()
        .map(|d| format!("Released {d}"))
        .chain((!album.labels.is_empty()).then(|| album.labels.join(", ")))
        .chain((formats.len() > 1).then(|| formats.into_iter().collect::<Vec<_>>().join(", ")))
        .chain(best.filter(|t| model::hi_res(t)).map(|_| "Hi-res".to_owned()))
        .collect::<Vec<_>>();
    if !notes.is_empty() {
        page = page.push(container(text(notes.join(" · "), ty::XS, look.ink_3)).padding(top(32.0)).boxed());
    }
    if !music.more_by.is_empty() {
        let name = album.artists.first().map_or(album.artist.as_str(), |a| a.name.as_str());
        page = page.push(
            container(section(format!("More by {name}"), None, albums(music, &music.more_by, look), look))
                .padding(top(48.0))
                .boxed(),
        );
    }
    page.boxed()
}

fn artist<'a>(music: &'a Music, look: &Look) -> View<'a> {
    let Some(artist) = &music.detail else { return missing(music, "This artist isn't here anymore", look) };
    let popular = &music.songs;
    let playable = !popular.is_empty();
    let header = column![
        container(cover_round(music, artist.cover.as_deref(), look)).width(208).height(208),
        container(text(&artist.name, NAME, look.ink).font(SEMIBOLD).center()).padding(top(24.0)),
        container(text(
            format!(
                "{} album{} · {} song{}",
                artist.album_count,
                plural(artist.album_count),
                artist.track_count,
                plural(artist.track_count)
            ),
            ty::SM,
            look.ink_2
        ))
        .padding(top(4.0)),
        container(
            row![
                button("Play", look)
                    .primary()
                    .large()
                    .icon(Icon::Play)
                    .on_press_maybe(playable.then_some(Message::PlayFrom(0, popular.len())))
                    .boxed(),
                button("Shuffle", look)
                    .large()
                    .icon(Icon::Shuffle)
                    .on_press_maybe(playable.then_some(Message::ShuffleSongs))
                    .boxed(),
                star(artist.starred, Message::StarDetail(!artist.starred), look),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .padding(top(24.0)),
    ]
    .align_x(Alignment::Center)
    .width(Fill);
    let mut page = column![container(header).padding(top(48.0))].width(Fill);
    if playable {
        let shown = popular.len().min(if music.popular { 20 } else { 5 });
        let more = (popular.len() > 5).then(|| {
            button(if music.popular { "Fewer" } else { "More" }, look)
                .plain()
                .small()
                .on_press(Message::Popular(!music.popular))
                .boxed()
        });
        let list = section(
            "Popular".into(),
            more,
            track_list(music, &popular[..shown], List { album: true, ..List::default() }, look),
            look,
        );
        page = page.push(container(list).padding(top(48.0)).boxed());
    }
    let single =
        |a: &&Card| a.release_types.iter().any(|t| t.eq_ignore_ascii_case("single") || t.eq_ignore_ascii_case("ep"));
    let (singles, full): (Vec<&Card>, Vec<&Card>) = artist.albums.iter().partition(|a| single(a));
    for (name, list) in [("Albums", full), ("Singles and EPs", singles)] {
        if !list.is_empty() {
            let l = *look;
            let grid = layout::poster_grid(list.len(), move |i| {
                album_tile(music, list[i], list[i].year.map(|y| y.to_string()).unwrap_or_default(), &l)
            });
            page = page.push(section(name.into(), None, grid, look));
        }
    }
    if !artist.appears_on.is_empty() {
        let l = *look;
        let grid = layout::poster_grid(artist.appears_on.len(), move |i| {
            let album = &artist.appears_on[i];
            album_tile(music, album, album.artist.clone(), &l)
        });
        page = page.push(section("Appears on".into(), None, grid, look));
    }
    page.boxed()
}

/// Keeps an album or a playlist to play offline (#97).
fn download<'a>(kept: &crate::offline::Downloads, key: crate::offline::Key, look: &Look) -> Option<View<'a>> {
    crate::offline::view::download_button(kept, vec![key], "Download", true, Message::Offline, Message::Go, look)
}

fn playlist<'a>(music: &'a Music, kept: &crate::offline::Downloads, look: &Look) -> View<'a> {
    let Some(list) = &music.detail else { return missing(music, "This playlist isn't here anymore", look) };
    let tracks = &music.songs;
    let playable = !tracks.is_empty();
    let title: View<'a> = if list.mine && music.renaming {
        container(
            input("playlist-name", &music.name, "Name", Message::Name, look).on_submit(Message::SavePlaylist).boxed(),
        )
        .width(Fill.max(576))
        .boxed()
    } else if list.mine {
        let l = *look;
        pressable(text(&list.name, HEADING, look.ink).font(SEMIBOLD), 0.0, look, move |_| Appearance {
            paint: Paint::NONE,
            ink: l.ink,
        })
        .on_press(Message::Rename(true))
        .boxed()
    } else {
        text(&list.name, HEADING, look.ink).font(SEMIBOLD).boxed()
    };
    let owner = if list.mine { String::new() } else { format!("{} · ", list.owner.name) };
    let mut actions = row![
        button("Play", look)
            .primary()
            .large()
            .icon(Icon::Play)
            .on_press_maybe(playable.then_some(Message::PlayFrom(0, tracks.len())))
            .boxed(),
        button("Shuffle", look)
            .large()
            .icon(Icon::Shuffle)
            .on_press_maybe(playable.then_some(Message::ShuffleSongs))
            .boxed(),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .extend(download(kept, crate::offline::Key::Playlist(list.id), look));
    if list.mine {
        actions = actions
            .push(
                button(if list.public { "Everyone here sees it" } else { "Only you see it" }, look)
                    .large()
                    .plain()
                    .icon(if list.public { Icon::Globe } else { Icon::Lock })
                    .on_press(Message::PlaylistPublic)
                    .boxed(),
            )
            .push(
                button("", look)
                    .large()
                    .danger()
                    .icon(Icon::Trash2)
                    .on_press(Message::Dialog(Some(Dialog::DeletePlaylist(
                        list.id,
                        format!("Delete “{}”?", list.name),
                    ))))
                    .boxed(),
            );
    }
    let info = column![
        text(if list.public { "PUBLIC PLAYLIST" } else { "PLAYLIST" }, ty::XS, look.ink_3).font(MEDIUM),
        container(title).padding(top(6.0)),
        container(text(
            format!("{owner}{} song{}, {}", list.track_count, plural(list.track_count), model::length(list.duration)),
            ty::SM,
            look.ink_2
        ))
        .padding(top(10.0)),
        container(actions).padding(top(24.0)),
    ];
    let artwork =
        squircle(container(mosaic(music, &list.covers, 20.0, look)).width(240).height(240), 20.0, look).lift();
    let body = if playable {
        track_list(music, tracks, List { album: true, remove: list.mine, ..List::default() }, look)
    } else {
        layout::empty("Nothing in here yet", Some("Add songs from any album or list with their ••• menu."), look)
    };
    column![hero(artwork.boxed(), info.boxed()), container(body).padding(top(40.0))].width(Fill).boxed()
}

/// An album, artist or playlist's artwork beside what it is, their bottoms lined up.
fn hero<'a>(artwork: View<'a>, info: View<'a>) -> View<'a> {
    container(row![artwork, container(info).width(Fill).padding(bottom(4.0))].spacing(32).align_y(Alignment::End))
        .padding(top(40.0))
        .boxed()
}

/// Loading, or gone.
fn missing<'a>(music: &'a Music, gone: &'a str, look: &Look) -> View<'a> {
    if let Some(error) = &music.error {
        return container(text(error, ty::SM, look.danger)).padding(top(40.0)).boxed();
    }
    waiting(music, gone, look)
}

fn section<'a>(name: String, aside: Option<View<'a>>, content: View<'a>, look: &Look) -> View<'a> {
    let heading =
        Row::with_children([text(name, ty::S15, look.ink).font(SEMIBOLD).width(Fill).boxed()].into_iter().chain(aside))
            .spacing(12)
            .align_y(Alignment::Center);
    column![container(heading).padding(bottom(14.0)), content].padding(bottom(44.0)).width(Fill).boxed()
}

fn albums<'a>(music: &'a Music, list: &'a [Card], look: &Look) -> View<'a> {
    let l = *look;
    layout::poster_grid(list.len(), move |i| {
        album_tile(music, &list[i], list[i].year.map(|y| y.to_string()).unwrap_or_default(), &l)
    })
}

/// `<AlbumTile>`: the cover, leaning toward the pointer, with a play button that rises in over it.
fn album_tile<'a>(music: &'a Music, album: &'a Card, caption: String, look: &Look) -> View<'a> {
    let route = Route::Album(album.id as u64);
    let picture = album.cover.as_deref().and_then(|p| music.images.picture(p));
    let (handle, arrived) = match picture {
        Some(Picture::Ready(handle, arrived)) => (Some(handle.clone()), *arrived),
        _ => (None, None),
    };
    let mut art = Stack::new().push(crate::ui::tilt::art(handle.clone(), 14.0, 1.0, look).arrived(arrived).boxed());
    if handle.is_none() {
        art = art.push(container(icon(Icon::Music, 40.0).color(look.ink_3)).center(Fill).boxed());
    }
    let art = link(art.width(Fill), route.clone(), look);
    let here = music.current().is_some_and(|e| e.track.album_id == Some(album.id));
    let playing = here && music.player.playing;
    let l = *look;
    let play = pressable(
        container(icon(if playing { Icon::Pause } else { Icon::Play }, 18.0).filled()).center(40),
        20.0,
        look,
        move |status| Appearance {
            paint: Paint::fill(if matches!(status, Status::Hovered | Status::Pressed) {
                l.accent_hover
            } else {
                l.accent
            }),
            ink: l.on_accent,
        },
    )
    .round()
    .on_press(if here { Message::Toggle } else { Message::PlayAlbum(album.id) });
    let play = container(tip(play, if playing { "Pause" } else { "Play" }, look))
        .align_right(Fill)
        .align_bottom(Fill)
        .padding(10);
    let art = if here { stack![art, play].boxed() } else { iced_widget::hover(art, play).boxed() };
    column![
        art,
        link(
            column![
                container(truncate(&album.name, ty::S13, look.ink).font(MEDIUM)).padding(top(8.0)),
                truncate(caption, ty::XS, look.ink_3)
            ]
            .width(Fill),
            route,
            look
        )
    ]
    .width(Fill)
    .boxed()
}

/// `<ArtistTile>`: a round picture and how many albums.
fn artist_tile<'a>(music: &'a Music, artist: &'a Card, look: &Look) -> View<'a> {
    let content = column![
        layout::aspect(cover_round(music, artist.cover.as_deref(), look), 1.0),
        container(truncate(&artist.name, ty::S13, look.ink).font(MEDIUM)).padding(top(10.0)),
        text(format!("{} album{}", artist.album_count, plural(artist.album_count)), ty::XS, look.ink_3),
    ]
    .align_x(Alignment::Center)
    .width(Fill);
    link(content, Route::Artist(artist.id as u64), look)
}

fn new_playlist<'a>(look: &Look) -> View<'a> {
    let l = *look;
    let tile = column![
        layout::aspect(
            squircle(container(icon(Icon::Plus, 32.0)).center(Fill), 14.0, look).dashed().width(Fill).height(Fill),
            1.0
        ),
        container(text("New playlist", ty::S13, look.ink).font(MEDIUM)).padding(top(8.0)),
    ]
    .width(Fill);
    pressable(tile, 14.0, look, move |status| Appearance {
        paint: Paint::NONE,
        ink: if matches!(status, Status::Hovered | Status::Pressed) { l.ink } else { l.ink_3 },
    })
    .width(Fill)
    .on_press(Message::Dialog(Some(Dialog::NewPlaylist(vec![]))))
    .boxed()
}

fn playlist_tile<'a>(music: &'a Music, list: &'a Card, look: &Look) -> View<'a> {
    let owner = if list.mine { String::new() } else { format!("{} · ", list.owner.name) };
    let length = if list.track_count > 0 { format!(", {}", model::length(list.duration)) } else { String::new() };
    let content = column![
        layout::aspect(mosaic(music, &list.covers, 14.0, look), 1.0),
        container(truncate(&list.name, ty::S13, look.ink).font(MEDIUM)).padding(top(8.0)),
        truncate(format!("{owner}{} songs{length}", list.track_count), ty::XS, look.ink_3),
    ]
    .width(Fill);
    link(content, Route::Playlist(list.id as u64), look)
}

/// `<Cover>`: artwork filling its space, or a quiet note when there's none.
fn cover<'a>(music: &Music, path: Option<&str>, radius: f32, note: f32, look: &Look) -> View<'a> {
    match path.and_then(|p| music.images.picture(p)) {
        picture @ Some(Picture::Ready(..) | Picture::Loading) => media::img(picture, radius, look),
        _ => squircle(container(icon(Icon::Music, note).color(look.ink_3)).center(Fill), radius, look)
            .fill(look.raised)
            .width(Fill)
            .height(Fill)
            .boxed(),
    }
}

fn cover_round<'a>(music: &'a Music, path: Option<&'a str>, look: &Look) -> View<'a> {
    let l = *look;
    responsive(move |size| cover(music, path, size.width / 2.0, size.width / 4.0, &l)).boxed()
}

/// `<Mosaic>`: up to four covers in a square, or one when that's all there is.
fn mosaic<'a>(music: &'a Music, covers: &'a [String], radius: f32, look: &Look) -> View<'a> {
    if covers.len() < 4 {
        return cover(music, covers.first().map(String::as_str), radius, 40.0, look);
    }
    if let Some(Some(handle)) = music.mosaics.get(&super::mosaic_key(covers)) {
        return media::img(Some(&Picture::Ready(handle.clone(), None)), radius, look);
    }
    let cell = |i: usize| container(cover(music, Some(&covers[i]), 0.0, 24.0, look)).width(Fill).height(Fill);
    squircle(column![row![cell(0), cell(1)], row![cell(2), cell(3)]], radius, look)
        .fill(look.raised)
        .width(Fill)
        .height(Fill)
        .boxed()
}

/// Whatever `content` is, going to `route` when pressed.
fn link<'a>(
    content: impl Widget<Message, iced_core::Theme, iced_widget::Renderer> + 'a,
    route: Route,
    look: &Look,
) -> View<'a> {
    let ink = look.ink;
    pressable(content, 14.0, look, move |_| Appearance { paint: Paint::NONE, ink })
        .width(Fill)
        .on_press(Message::Go(route))
        .boxed()
}

fn star<'a>(starred: bool, message: Message, look: &Look) -> View<'a> {
    let l = *look;
    let glyph = icon(Icon::Star, 16.0);
    let button =
        pressable(container(if starred { glyph.filled() } else { glyph }).center(32), 16.0, look, move |status| {
            Appearance {
                paint: Paint::NONE,
                ink: if starred {
                    l.warn
                } else if matches!(status, Status::Hovered | Status::Pressed) {
                    l.ink
                } else {
                    l.ink_3
                },
            }
        })
        .round()
        .on_press(message);
    tip(button, if starred { "Unstar" } else { "Star" }, look)
}

pub(super) fn quality<'a>(track: &Track, look: &Look) -> View<'a> {
    badge(model::quality(track), if model::hi_res(track) { Tone::Warn } else { Tone::Quiet }, look)
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Next,
    Queue,
    Playlist,
    Album,
    Artist(usize),
}

/// `<TrackMenu>`: queueing some tracks, keeping them, or going to where one's from (`one`).
fn menu<'a>(tracks: &'a [Track], one: Option<(&'a Track, bool)>, look: &Look) -> View<'a> {
    use Action::*;
    let chosen = move || one.map_or_else(|| tracks.to_vec(), |(t, _)| vec![t.clone()]);
    let mut actions = vec![Next, Queue, Playlist];
    if one.is_some_and(|(t, album)| album && t.album_id.is_some()) {
        actions.push(Album);
    }
    let artists = one.map_or(&[][..], |(t, _)| &t.artists[..t.artists.len().min(3)]);
    actions.extend((0..artists.len()).map(Artist));
    let menu = crate::ui::select::icon_menu(
        Icon::Ellipsis,
        actions,
        None,
        |action| match action {
            Next => "Play next".into(),
            Queue => "Add to queue".into(),
            Playlist => "Add to playlist".into(),
            Album => "Go to album".into(),
            Artist(i) => artists[*i].name.clone(),
        },
        move |action| match action {
            Next | Queue => Message::Add(chosen(), action == Next),
            Playlist => Message::Dialog(Some(Dialog::AddTo(chosen().iter().map(|t| t.id).collect()))),
            Album => Message::Go(Route::Album(one.and_then(|(t, _)| t.album_id).unwrap_or_default() as u64)),
            Artist(i) => Message::Go(Route::Artist(artists[i].id as u64)),
        },
        look,
    );
    tip(menu, "More", look)
}

/// How a `<TrackList>` shows its tracks.
#[derive(Clone, Copy, Default)]
struct List<'a> {
    /// Track numbers (an album's) instead of covers.
    numbers: bool,
    /// The album after the artists.
    album: bool,
    /// Headers between discs, with their titles, once there's more than one.
    discs: &'a [super::DiscTitle],
    /// A button taking each out (a playlist of yours).
    remove: bool,
}

/// `<TrackList>`: tracks one per row; pressing one plays the list from there.
fn track_list<'a>(music: &'a Music, tracks: &'a [Track], list: List<'a>, look: &Look) -> View<'a> {
    let numbers = list.numbers;
    let multi_disc = numbers && tracks.iter().any(|t| t.disc.unwrap_or(1) != tracks[0].disc.unwrap_or(1));
    let mut rows = Column::new().width(Fill);
    for (i, track) in tracks.iter().enumerate() {
        let disc = track.disc.unwrap_or(1);
        if multi_disc && (i == 0 || tracks[i - 1].disc.unwrap_or(1) != disc) {
            let mut header = row![
                icon(Icon::Disc3, 14.0).color(look.ink_3),
                text(format!("Disc {disc}"), ty::XS, look.ink_3).font(MEDIUM)
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            if let Some(title) = list.discs.iter().find(|d| d.disc == disc && !d.title.is_empty()) {
                header = header.push(text(format!("· {}", title.title), ty::XS, look.ink_2).font(MEDIUM).boxed());
            }
            rows = rows.push(
                container(header)
                    .padding(Padding { top: if i == 0 { 0.0 } else { 20.0 }, bottom: 6.0, left: 8.0, right: 8.0 })
                    .boxed(),
            );
        }
        rows = rows.push(track_row(music, track, i, tracks.len(), list, look));
    }
    rows.boxed()
}

fn track_row<'a>(
    music: &'a Music,
    track: &'a Track,
    index: usize,
    end: usize,
    list: List<'a>,
    look: &Look,
) -> View<'a> {
    let here = music.current().is_some_and(|e| e.track.id == track.id);
    let l = *look;
    let slot = |hovered: bool| -> View<'a> {
        let content: View<'a> = if list.numbers {
            if here {
                bars(l.ink)
            } else if hovered {
                icon(Icon::Play, 14.0).filled().color(l.ink).boxed()
            } else {
                text(track.number.unwrap_or(index as i64 + 1).to_string(), ty::S13, l.ink_3).boxed()
            }
        } else {
            let art = container(cover(music, track.cover.as_deref(), 6.0, 12.0, &l)).width(32).height(32);
            if here {
                stack![
                    art,
                    squircle(container(bars(l.media_ink)).center(32), 6.0, &l).fill(l.media_shade.scale_alpha(0.55))
                ]
                .boxed()
            } else {
                art.boxed()
            }
        };
        container(content).center_x(32).boxed()
    };
    let titles = || -> View<'a> {
        let artists = if track.artists.len() > 1 {
            track.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")
        } else {
            track.artist.clone()
        };
        let detail = if list.album { format!("{artists} · {}", track.album) } else { artists };
        column![
            truncate(&track.title, ty::SM, l.ink).font(if here { MEDIUM } else { ty::REGULAR }),
            truncate(detail, ty::XS, l.ink_3)
        ]
        .width(Fill)
        .boxed()
    };
    let length = || container(text(model::duration(track.duration), ty::S13, l.ink_3)).align_right(44).boxed();
    let gap = || space().width(32).boxed();
    let mut rest = row![slot(false), titles()].spacing(12).align_y(Alignment::Center);
    rest = rest.push(if track.starred { star(true, Message::Star(track.id, false), look) } else { gap().boxed() });
    rest = rest.push(length()).push(gap());
    if list.remove {
        rest = rest.push(gap());
    }
    let mut hovered = row![
        slot(true),
        titles(),
        quality(track, look),
        star(track.starred, Message::Star(track.id, !track.starred), look),
        length(),
        container(menu(std::slice::from_ref(track), Some((track, !list.numbers)), look)).center_x(32),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if list.remove {
        let x = pressable(container(icon(Icon::X, 16.0)).center(32), 16.0, look, move |status| Appearance {
            paint: Paint::NONE,
            ink: if matches!(status, Status::Hovered | Status::Pressed) { l.ink } else { l.ink_3 },
        })
        .round()
        .on_press(Message::PlaylistRemove(index));
        hovered = hovered.push(tip(x, "Remove from the playlist", look));
    }
    let hovered = pressable(hovered, 10.0, look, move |status| Appearance {
        paint: Paint::fill(if status == Status::Pressed { l.press } else { l.hover }),
        ink: l.ink,
    })
    .width(Fill)
    .height(48)
    .padding([0, 8])
    .on_press(if here { Message::Toggle } else { Message::PlayFrom(index, end) });
    crate::ui::reveal::reveal(container(rest).width(Fill).center_y(48).padding([0, 8]), hovered)
}

/// `<Playing>`: three bars, for the track that's on.
pub(super) fn bars<'a>(color: Color) -> View<'a> {
    Row::with_children([12.0, 7.0, 10.0].map(|height| {
        container(space().width(3).height(height))
            .style(move |_| container::Style {
                background: Some(color.into()),
                border: iced_core::Border::default().rounded(1.5),
                ..container::Style::default()
            })
            .boxed()
    }))
    .spacing(2)
    .height(12)
    .align_y(Alignment::End)
    .boxed()
}

fn kind(types: &[String], compilation: bool) -> &'static str {
    let has = |name: &str| types.iter().any(|t| t.eq_ignore_ascii_case(name));
    if compilation || has("compilation") {
        "Compilation"
    } else if has("ep") {
        "EP"
    } else if has("single") {
        "Single"
    } else if has("live") {
        "Live album"
    } else {
        "Album"
    }
}

pub(super) fn plural(n: i64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn top(px: f32) -> Padding {
    Padding { top: px, ..Padding::ZERO }
}

fn bottom(px: f32) -> Padding {
    Padding { bottom: px, ..Padding::ZERO }
}
