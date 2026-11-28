// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_widget::{column, row};

use super::*;

pub(super) fn requests<'a>(b: &'a Browsing, look: &Look) -> View<'a> {
    let mut page = column![row![
        heading("Requests", look),
        space().width(Fill),
        button(if b.can().manage_shows { "Add a show" } else { "Request a show" }, look)
            .primary()
            .icon(Icon::Plus)
            .on_press(AppMessage::Navigate(Route::Discover))
            .boxed()
    ]]
    .width(Fill);
    let Some(d) = b.data(&Key::Requests) else { return page.push((pending(b, &Key::Requests, look)).boxed()).boxed() };
    if d.requests.is_empty() {
        return page.push((layout::empty("No requests yet", None, look)).boxed()).boxed();
    }
    for pending_group in [true, false] {
        let mut list = Column::<View<'a>>::new().spacing(8);
        for r in d.requests.iter().filter(|r| (r.state == "PENDING") == pending_group) {
            let mut details = column![
                row![
                    title_link(&r.name, r.title.as_ref().map(|t| Route::Title(t.id as u64)), look),
                    text(r.year.map(|y| y.to_string()).unwrap_or_default(), ty::XS, look.ink_3),
                    state(&r.state, look)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    if let Some(user) = r.user.as_ref().filter(|u| b.viewer().is_none_or(|v| v.id != u.id)) {
                        watch_image(
                            b,
                            user.avatar.as_deref(),
                            media::avatar(
                                &user.username,
                                user.avatar.as_deref().and_then(|p| b.images.get(p)),
                                16.0,
                                None,
                                look,
                            ),
                        )
                    } else {
                        space().boxed()
                    },
                    text(
                        format!(
                            "{} asked {}{}",
                            if r.user.as_ref().is_some_and(|u| b.viewer().is_some_and(|v| v.id == u.id)) {
                                "You"
                            } else {
                                r.user.as_ref().map_or("Someone", |u| u.username.as_str())
                            },
                            relative(r.created_at),
                            r.library.as_ref().map(|l| format!(" · for {l}")).unwrap_or_default()
                        ),
                        ty::XS,
                        look.ink_3
                    )
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            ]
            .spacing(4)
            .width(Fill);
            if let Some(note) = &r.note {
                details = details.push((text(format!("“{note}”"), ty::S13, look.ink_2)).boxed());
            }
            if r.state == "APPROVED"
                && let Some(aired) = r.aired.filter(|n| *n > 0)
            {
                details = details
                    .push((progress(Some(r.have.unwrap_or(0) as f32 / aired as f32), look.ok, look)).boxed())
                    .push((text(format!("{} of {aired} episodes", r.have.unwrap_or(0)), ty::XS, look.ink_3)).boxed());
            }
            let mut actions = Vec::new();
            if r.state == "PENDING" {
                if b.can().manage_requests {
                    actions.push(
                        button("Decline", look)
                            .plain()
                            .small()
                            .on_press_maybe((!b.busy).then(|| act(b, Action::Decline(r.id))))
                            .boxed(),
                    );
                    actions.push(
                        button("Approve", look)
                            .primary()
                            .small()
                            .on_press_maybe((!b.busy).then(|| act(b, Action::Approve(r.id))))
                            .boxed(),
                    );
                } else if r.user.as_ref().is_some_and(|u| b.viewer().is_some_and(|v| v.id == u.id)) {
                    actions.push(
                        button("Cancel", look).plain().small().on_press(act(b, Action::CancelRequest(r.id))).boxed(),
                    );
                }
            }
            list = list.push(
                (raised(
                    row![picture(b, r.poster.as_deref(), 56.0, 84.0, 10.0, look), details, line(actions)]
                        .spacing(16)
                        .align_y(Alignment::Start)
                        .boxed(),
                    look,
                ))
                .boxed(),
            );
        }
        if d.requests.iter().any(|r| (r.state == "PENDING") == pending_group) {
            page = page.push(
                (section(
                    if pending_group {
                        if b.can().manage_requests { "Waiting for you" } else { "Waiting for approval" }
                    } else {
                        "Earlier"
                    },
                    list.boxed(),
                    look,
                ))
                .boxed(),
            );
        }
    }
    page.boxed()
}
