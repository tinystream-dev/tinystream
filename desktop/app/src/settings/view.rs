// SPDX-License-Identifier: AGPL-3.0-or-later

use iced_core::{Alignment, Length};
use iced_widget::{Column, Element, Widget as _, column, container, row};
use serde_json::{Value, json};

use super::{Action, Editor, Message, Tab, fields};
use crate::app::{App, Message as AppMessage, Overlay};
use crate::ui::Look;
use crate::ui::controls::{button, owned_input, toggle};
use crate::ui::layout::{page, page_title};
use crate::ui::text::{self as type_, SEMIBOLD, text};

fn msg(message: Message) -> AppMessage {
    AppMessage::Settings(message)
}
fn action(label: impl Into<String>, a: Action, busy: bool, look: &Look) -> Element<'static, AppMessage> {
    let b = button(label.into(), look).small();
    if busy { b.boxed() } else { b.on_press(msg(Message::Act(a))).boxed() }
}
fn edit(label: impl Into<String>, editor: Editor, busy: bool, look: &Look) -> Element<'static, AppMessage> {
    let b = button(label.into(), look).small();
    if busy { b.boxed() } else { b.on_press(msg(Message::Edit(editor))).boxed() }
}
fn card<'a>(
    title: impl Into<String>,
    body: impl iced_core::Widget<AppMessage, iced_core::Theme, iced_widget::Renderer> + 'a,
    look: &Look,
) -> Element<'a, AppMessage> {
    crate::ui::squircle::squircle(
        column![text(title.into(), type_::S15, look.ink).font(SEMIBOLD), body.boxed()].spacing(16),
        16.0,
        look,
    )
    .fill(look.raised)
    .edge()
    .padding(20)
    .width(Length::Fill)
    .boxed()
}
fn label(name: &str) -> String {
    if name.chars().all(|c| !c.is_lowercase()) {
        return match name {
            "RSS" | "TORZNAB" | "TMDB" | "DHT" | "UPNP" => name.into(),
            _ => {
                let n = name.replace('_', " ").to_lowercase();
                let mut chars = n.chars();
                chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
            },
        };
    }
    match name {
        "analyzeLoudness" => "Even out volume".into(),
        "onlineLyrics" => "Look up lyrics online".into(),
        "maxStorage" => "Total storage limit (MB)".into(),
        "clipStorage" => "Space for clips (MB)".into(),
        "downloadLimit" | "slowDownloadLimit" => "Download limit (KB/s)".into(),
        "uploadLimit" | "slowUploadLimit" => "Upload limit (KB/s)".into(),
        "tmdbApiKey" => "TMDB API key".into(),
        "vaapiDevice" => "GPU device".into(),
        "cors" => "Allowed origins (CORS)".into(),
        "upnp" => "Open the port on the router (UPnP)".into(),
        "dht" => "Find peers without trackers (DHT)".into(),
        "clipMaxLength" => "Longest clip (seconds)".into(),
        _ => {
            let mut result = String::new();
            for (i, c) in name.chars().enumerate() {
                if i == 0 {
                    result.extend(c.to_uppercase());
                } else if c.is_uppercase() {
                    result.push(' ');
                    result.extend(c.to_lowercase());
                } else {
                    result.push(c);
                }
            }
            result
        },
    }
}
fn scalar(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    }
}
fn array<'a>(value: &'a Value, name: &str) -> &'a [Value] {
    value.get(name).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default()
}
fn name(value: &Value) -> String {
    value.get("name").or_else(|| value.get("username")).or_else(|| value.get("label")).map(scalar).unwrap_or_default()
}
fn project(value: &Value, kind: &str) -> Value {
    let mut out = serde_json::Map::new();
    for (key, ty) in fields::fields(kind) {
        let v = value.get(key).cloned().unwrap_or(Value::Null);
        let base = ty.trim_end_matches('!');
        out.insert((*key).into(), if !v.is_null() && !fields::fields(base).is_empty() { project(&v, base) } else { v });
    }
    Value::Object(out)
}

pub fn view<'a>(app: &'a App, look: &Look) -> Element<'a, AppMessage> {
    let look = *look;
    iced_widget::responsive(move |size| view_inner(app, &look, size.width)).boxed()
}
fn view_inner<'a>(app: &'a App, look: &Look, width: f32) -> Element<'a, AppMessage> {
    let compact = width < 760.0;
    let state = &app.settings;
    let admin = app.browsing.viewer().is_some_and(|u| u.is_admin);
    let downloads = app.browsing.features();
    let mut nav = Column::new().spacing(4).width(192);
    for (title, tabs) in [
        ("You", &[(Tab::Account, "Account"), (Tab::Appearance, "Appearance")][..]),
        (
            "Library",
            &[
                (Tab::Libraries, "Libraries"),
                (Tab::Music, "Music"),
                (Tab::Clips, "Clips"),
                (Tab::Skipped, "Skipped files"),
            ][..],
        ),
        (
            "Downloads",
            &[
                (Tab::Torrents, "Torrents"),
                (Tab::Sources, "Sources"),
                (Tab::Profiles, "Quality profiles"),
                (Tab::Automation, "Automation"),
                (Tab::Renames, "Renames"),
            ][..],
        ),
        ("Server", &[(Tab::General, "General"), (Tab::Users, "Users"), (Tab::File, "Config file")][..]),
        ("Desktop", &[(Tab::Desktop, "Desktop")][..]),
        ("About", &[(Tab::Credits, "Credits")][..]),
    ] {
        let visible: Vec<_> =
            tabs.iter().filter(|(tab, _)| (!tab.admin() || admin) && (!tab.downloads() || downloads)).collect();
        if visible.is_empty() {
            continue;
        }
        nav = nav.push((container(text(title, type_::XS, look.ink_3)).padding([12, 10])).boxed());
        for (tab, title) in visible {
            nav = nav.push(crate::ui::controls::menu_item(
                Some(tab.icon()),
                text(*title, type_::SM, look.ink_2),
                None,
                *tab == state.tab,
                (!state.busy).then(|| msg(Message::Tab(*tab))),
                look,
            ));
        }
    }
    let mut body = Column::new().spacing(20).width(Length::Fill);
    if let Some(error) = &state.error {
        body = body.push((text(error.clone(), type_::SM, look.danger)).boxed());
    }
    if let Some(notice) = &state.notice {
        body = body.push((text(notice.clone(), type_::SM, look.ok)).boxed());
    }
    for (query, entry) in &state.cache {
        if !state.tab.queries().contains(query)
            && !(*query == "Folders" && state.editor.as_ref().is_some_and(|e| e.kind == "LibraryInput"))
        {
            continue;
        }
        if let Some(error) = &entry.error {
            body = body.push(
                (row![
                    text(error.clone(), type_::SM, look.danger),
                    button("Retry", look).small().on_press(msg(Message::Retry)).boxed()
                ]
                .spacing(8))
                .boxed(),
            );
        }
    }
    if let Some(error) = state.data("Settings").and_then(|v| v.pointer("/settings/error")).filter(|v| !v.is_null()) {
        body = body.push(
            (card(
                "config.toml has a mistake, so your last edit wasn't applied",
                text(scalar(error), type_::SM, look.danger),
                look,
            ))
            .boxed(),
        );
    }
    if let Some(editor) = &state.editor {
        body = body.push(
            (if editor.kind == "RawConfig" {
                card(
                    "Config file",
                    column![
                        iced_widget::text_editor(&state.raw).on_action(|a| msg(Message::Raw(a))).height(480).boxed(),
                        row![
                            button("Cancel", look).on_press(msg(Message::Close)).boxed(),
                            button("Save config.toml", look).on_press(msg(Message::Save)).boxed()
                        ]
                        .spacing(8)
                    ]
                    .spacing(16),
                    look,
                )
            } else {
                editor_view(editor, state.busy, look, if compact { width - 80.0 } else { width - 304.0 })
            })
            .boxed(),
        );
        if editor.kind == "LibraryInput" {
            body = body.push(folder_picker(state, editor, look));
        }
    } else {
        body = body.push((content(app, look)).boxed());
    }
    if state.busy {
        body = body.push((text("Saving…", type_::SM, look.ink_3)).boxed());
    }
    if let Some(result) = &state.result {
        body = body.push((result_view(result, look)).boxed());
    }
    let layout: Element<'a, AppMessage> = if compact {
        const PERSONAL: &[Tab] = &[Tab::Account, Tab::Appearance, Tab::Desktop, Tab::Credits];
        const ADMIN: &[Tab] = &[
            Tab::Account,
            Tab::Appearance,
            Tab::Libraries,
            Tab::Music,
            Tab::Clips,
            Tab::Skipped,
            Tab::General,
            Tab::Users,
            Tab::File,
            Tab::Desktop,
            Tab::Credits,
        ];
        const ALL: &[Tab] = &[
            Tab::Account,
            Tab::Appearance,
            Tab::Libraries,
            Tab::Music,
            Tab::Clips,
            Tab::Skipped,
            Tab::Torrents,
            Tab::Sources,
            Tab::Profiles,
            Tab::Automation,
            Tab::Renames,
            Tab::General,
            Tab::Users,
            Tab::File,
            Tab::Desktop,
            Tab::Credits,
        ];
        let tabs = if !admin {
            PERSONAL
        } else if downloads {
            ALL
        } else {
            ADMIN
        };
        column![
            crate::ui::select::select(
                Some(state.tab),
                tabs,
                |tab| tab.label().into(),
                |tab| msg(Message::Tab(tab)),
                look
            ),
            body
        ]
        .spacing(24)
        .boxed()
    } else {
        row![nav, body].spacing(32).boxed()
    };
    let base = page(column![page_title("Settings", None, look), layout], false);
    if let Some(confirm) = &state.confirm {
        return crate::ui::modal::modal(
            base,
            iced_widget::center(card(
                "Confirm",
                column![
                    text(confirm.confirm.clone().unwrap_or_default(), type_::SM, look.ink),
                    row![
                        button("Cancel", look).on_press(msg(Message::Cancel)).boxed(),
                        button("Confirm", look).primary().on_press(msg(Message::Confirm)).boxed()
                    ]
                    .spacing(8)
                ]
                .spacing(16),
                look,
            ))
            .width(Length::Fill)
            .boxed(),
            msg(Message::Cancel),
        );
    }
    base
}

fn result_view<'a>(value: &Value, look: &Look) -> Element<'a, AppMessage> {
    let mut lines = Column::new().spacing(8);
    for (key, v) in value.as_object().into_iter().flatten() {
        if let Some(link) = v.get("link").and_then(Value::as_str) {
            lines = lines.push(
                (row![
                    text(link.to_owned(), type_::SM, look.ink),
                    button("Copy link", look).on_press(msg(Message::Copy(link.into()))).boxed()
                ]
                .spacing(8))
                .boxed(),
            );
        }
        if let Some(secret) = v.get("secret").and_then(Value::as_str) {
            lines = lines
                .push((text("This is the only time this password is shown.", type_::SM, look.ink_2)).boxed())
                .push(
                    (row![
                        text(secret.to_owned(), type_::SM, look.ink),
                        button("Copy password", look).on_press(msg(Message::Copy(secret.into()))).boxed()
                    ]
                    .spacing(8))
                    .boxed(),
                );
        }
        if let Some(problems) = v.get("problems").and_then(Value::as_array) {
            for p in problems {
                lines = lines.push((text(scalar(p), type_::SM, look.danger)).boxed());
            }
        }
        if key == "detectSource" {
            lines = lines
                .push((text(format!("{} · {}", name(v), scalar(&v["kind"])), type_::SM, look.ink)).boxed())
                .push((text(scalar(&v["url"]), type_::SM, look.ink_2)).boxed());
        }
    }
    lines.boxed()
}

fn editor_view<'a>(editor: &'a Editor, busy: bool, look: &Look, width: f32) -> Element<'a, AppMessage> {
    let fields = if editor.kind == "SchemeInput" {
        scheme_fields(editor, look)
    } else {
        form(editor, editor.value(), editor.kind, &editor.path, look)
    };
    let mut save = button("Save", look).primary();
    if !busy && editor.invalid.is_empty() && editor.variables != editor.original {
        save = save.on_press(msg(Message::Save));
    }
    let mut cancel = button("Cancel", look);
    if !busy {
        cancel = cancel.on_press(msg(Message::Close));
    }
    let body = column![fields, row![cancel.boxed(), save.boxed()].spacing(8)].spacing(20);
    if editor.kind == "SchemeInput" {
        if width < 560.0 {
            return card(editor.title.clone(), column![scheme_preview(editor.value(), look), body].spacing(20), look);
        }
        return card(
            editor.title.clone(),
            row![container(body).width(Length::Fill), container(scheme_preview(editor.value(), look)).width(260)]
                .spacing(20),
            look,
        );
    }
    card(editor.title.clone(), body, look)
}
fn form<'a>(
    editor: &'a Editor,
    value: &'a Value,
    kind: &'static str,
    path: &str,
    look: &Look,
) -> Element<'a, AppMessage> {
    let mut content = Column::new().spacing(16);
    for (key, ty) in fields::fields(kind) {
        if !editor.downloads
            && ((kind == "LibraryInput" && ["managed", "profile", "downloadPath"].contains(key))
                || (["PermissionsInput", "PermissionOverridesInput"].contains(&kind)
                    && ["request", "autoApprove", "requestLimit", "manageRequests", "manageShows", "downloads"]
                        .contains(key)))
        {
            continue;
        }
        if kind == "LibraryInput"
            && value["kind"] != "VIDEO"
            && ["metadataProvider", "managed", "profile"].contains(key)
        {
            continue;
        }
        let path = format!("{path}/{key}");
        let v = value.get(key).unwrap_or(&Value::Null);
        let title = label(key);
        let mut line = column![text(title, type_::SM, look.ink)].spacing(6);
        let nullable = !ty.ends_with('!');
        if nullable {
            let reset = if v.is_null() { fields::default_value(ty) } else { Value::Null };
            let title = if v.is_null() { "Use a custom value" } else { "Use default" };
            line = line
                .push((button(title, look).small().on_press(msg(Message::Set(path.clone(), reset))).boxed()).boxed());
        }
        if !v.is_null() {
            line = line.push((field(editor, v, ty, &path, key, look)).boxed());
        }
        if let Some(error) = editor.invalid.get(&path) {
            line = line.push((text(error.clone(), type_::XS, look.danger)).boxed());
        }
        content = content.push((line).boxed());
    }
    content.boxed()
}
fn field<'a>(
    editor: &'a Editor,
    value: &'a Value,
    kind: &'static str,
    path: &str,
    key: &str,
    look: &Look,
) -> Element<'a, AppMessage> {
    let base = kind.trim_end_matches('!');
    if base.starts_with('[') {
        let inner = base.trim_start_matches('[').trim_end_matches(']');
        let mut list = Column::new().spacing(8);
        for (i, v) in value.as_array().into_iter().flatten().enumerate() {
            let p = format!("{path}/{i}");
            list = list.push(
                (row![
                    field(editor, v, inner, &p, key, look),
                    column![
                        button("↑", look).small().on_press(msg(Message::Move(path.into(), i, -1))).boxed(),
                        button("↓", look).small().on_press(msg(Message::Move(path.into(), i, 1))).boxed(),
                        button("Remove", look).small().on_press(msg(Message::Remove(path.into(), i))).boxed()
                    ]
                    .spacing(2)
                ]
                .spacing(8))
                .boxed(),
            );
        }
        return list
            .push((button("Add", look).small().on_press(msg(Message::Add(path.into(), inner))).boxed()).boxed())
            .boxed();
    }
    if !fields::fields(base).is_empty() {
        return form(editor, value, base, path, look);
    }
    let p = path.to_owned();
    if base == "Boolean" {
        return toggle(value.as_bool().unwrap_or(false), move |v| msg(Message::Set(p.clone(), json!(v))), look);
    }
    if let Some(choices) = editor.choices.get(key).filter(|c| !c.is_empty()) {
        return crate::ui::select::select(
            choices.iter().find(|(id, _)| Some(id.as_str()) == value.as_str()).cloned(),
            choices,
            |v| v.1.clone(),
            move |v| msg(Message::Set(p.clone(), json!(v.0))),
            look,
        );
    }
    let options = fields::options(base);
    if !options.is_empty() {
        return crate::ui::select::select(
            value.as_str(),
            options,
            |v| label(v),
            move |v| msg(Message::Set(p.clone(), json!(v))),
            look,
        );
    }
    let display = editor.text.get(path).cloned().unwrap_or_else(|| scalar(value));
    let input = owned_input(format!("settings:{path}"), display, move |v| msg(Message::Text(p.clone(), v, base)), look);
    if key.to_lowercase().contains("password")
        || key == "apiKey"
        || key == "tmdbApiKey"
        || key == "current"
        || key == "new"
    {
        input.secure().boxed()
    } else {
        input.boxed()
    }
}

fn config_editor(settings: &Value, section: &'static str, kind: &'static str) -> Editor {
    let value = project(&settings[section], kind);
    Editor::new(label(section), "SaveSection", kind, &format!("/patch/{section}"), json!({"patch":{section:value}}))
}
fn content<'a>(app: &'a App, look: &Look) -> Element<'a, AppMessage> {
    let s = &app.settings;
    let settings = s.data("Settings").map(|v| &v["settings"]);
    match s.tab {
        Tab::Desktop => app.session.desktop_settings(look),
        Tab::Credits => card(
            "Credits",
            column![
                text(format!("tinystream {} · AGPL-3.0-or-later", crate::updates::VERSION), type_::SM, look.ink),
                button("Third-party licenses", look).on_press(AppMessage::Overlay(Some(Overlay::Credits))).boxed()
            ]
            .spacing(12),
            look,
        ),
        Tab::Account => account(app, look),
        Tab::Appearance => appearance(app, look),
        Tab::Libraries | Tab::Sources | Tab::Profiles => {
            let Some(settings) = settings else {
                return loading(look);
            };
            let (title, key, kind, add, update, remove) = match s.tab {
                Tab::Libraries => {
                    ("Libraries", "libraries", "LibraryInput", "AddLibrary", "UpdateLibrary", "RemoveLibrary")
                },
                Tab::Sources => ("Sources", "sources", "SourceInput", "AddSource", "UpdateSource", "RemoveSource"),
                _ => ("Quality profiles", "profiles", "ProfileInput", "AddProfile", "UpdateProfile", "RemoveProfile"),
            };
            let mut list = Column::new().spacing(16);
            if array(settings, key).is_empty() {
                list = list.push((text("None yet. Add one to get started.", type_::SM, look.ink_3)).boxed());
            }
            for v in array(settings, key) {
                let n = name(v);
                let editor =
                    Editor::new(format!("Edit {n}"), update, kind, "/input", json!({"name":n,"input":project(v,kind)}));
                let mut item = column![
                    row![
                        text(n.clone(), type_::SM, look.ink).width(Length::Fill),
                        edit("Edit", editor, s.busy, look),
                        action(
                            "Remove",
                            Action::new(remove, json!({"name":n})).confirm(format!("Remove {n}?")),
                            s.busy,
                            look
                        )
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                ]
                .spacing(6);
                if let Some(path) = v.get("path").or_else(|| v.get("url")) {
                    item = item.push((text(scalar(path), type_::XS, look.ink_2)).boxed());
                }
                if let Some(error) = v.get("error").filter(|v| !v.is_null()) {
                    item = item.push((text(scalar(error), type_::XS, look.danger)).boxed());
                }
                if s.tab == Tab::Libraries {
                    item = item.push(
                        (row![
                            text(
                                format!("{} titles · {} skipped", scalar(&v["titleCount"]), scalar(&v["skippedCount"])),
                                type_::XS,
                                look.ink_3
                            ),
                            action("Scan", Action::new("Scan", json!({"library":n})), s.busy, look)
                        ]
                        .spacing(8))
                        .boxed(),
                    );
                }
                if s.tab == Tab::Sources {
                    item = item.push((action(if v["enabled"] == true {"Disable"} else {"Enable"},Action::new("UpdateSource",json!({"name":n,"input":({ let mut p=project(v,kind); p["enabled"]=json!(v["enabled"]!=true); p })})),s.busy,look)).boxed());
                }
                list = list.push((item).boxed());
            }
            let mut blank = fields::default_value(kind);
            if kind == "SourceInput" {
                blank["enabled"] = json!(true);
            }
            if kind == "ProfileInput" {
                blank = json!({"name":"","resolutions":["1080p","720p"],"groups":[],"require":[],"reject":[],"minSize":null,"maxSize":null,"codecs":[],"preferDualAudio":false,"batches":true,"minSeeders":1});
            }
            list = list.push(
                (edit(
                    format!("Add {}", title.to_lowercase()),
                    Editor::new(format!("Add {}", title.to_lowercase()), add, kind, "/input", json!({"input":blank})),
                    s.busy,
                    look,
                ))
                .boxed(),
            );
            if s.tab == Tab::Sources {
                list = list.push(
                    (edit(
                        "Detect source",
                        Editor::new(
                            "Detect source",
                            "DetectSource",
                            "SourceDetection",
                            "",
                            json!({"url":"","apiKey":null}),
                        ),
                        s.busy,
                        look,
                    ))
                    .boxed(),
                );
            }
            card(title, list, look)
        },
        Tab::Music | Tab::Torrents | Tab::Automation | Tab::General => {
            let Some(settings) = settings else {
                return loading(look);
            };
            let sections: &[(&str, &str)] = match s.tab {
                Tab::Music => &[("music", "MusicConfigInput")],
                Tab::Torrents => &[("downloads", "DownloadsConfigInput")],
                Tab::Automation => &[("automation", "AutomationConfigInput"), ("requests", "RequestsConfigInput")],
                _ => &[
                    ("network", "NetworkConfigInput"),
                    ("tailscale", "TailscaleConfigInput"),
                    ("signIn", "SignInConfigInput"),
                    ("smtp", "SmtpConfigInput"),
                    ("scan", "ScanConfigInput"),
                    ("metadata", "MetadataConfigInput"),
                    ("transcode", "TranscodeConfigInput"),
                    ("log", "LogConfigInput"),
                ],
            };
            let mut cards = Column::new().spacing(20);
            for &(section, kind) in sections {
                cards = cards.push(
                    (card(
                        label(section),
                        column![
                            summary(&settings[section], look),
                            edit("Edit", config_editor(settings, section, kind), s.busy, look)
                        ]
                        .spacing(16),
                        look,
                    ))
                    .boxed(),
                );
            }
            if s.tab == Tab::General {
                cards = cards.push(
                    (card(
                        "Email",
                        edit(
                            "Send test email",
                            Editor::new("Send test email", "SendTestEmail", "TestEmail", "", json!({"to":""})),
                            s.busy,
                            look,
                        ),
                        look,
                    ))
                    .boxed(),
                );
            }
            if s.tab == Tab::General {
                cards = cards.push(card("Tailscale status", summary(&settings["tailscaleStatus"], look), look));
                if let Some(data) = s.data("Settings") {
                    cards = cards.push(card("Available encoders", summary(&data["server"]["transcoding"], look), look));
                }
            }
            if s.tab == Tab::Torrents
                && let Some(data) = s.data("SettingsEngine")
            {
                cards = cards.push(card("Download engine", summary(&data["downloadEngine"], look), look));
            }
            cards.boxed()
        },
        Tab::File => {
            let Some(settings) = settings else {
                return loading(look);
            };
            card(
                "Config file",
                column![
                    text(scalar(&settings["paths"]["config"]), type_::XS, look.ink_3),
                    text(scalar(&settings["raw"]), type_::SM, look.ink_2),
                    edit(
                        "Edit config.toml",
                        Editor::new("Config file", "ReplaceConfig", "RawConfig", "", json!({"text":settings["raw"]})),
                        s.busy,
                        look
                    )
                ]
                .spacing(16),
                look,
            )
        },
        Tab::Skipped => {
            let Some(data) = s.data("SkippedFiles") else {
                return loading(look);
            };
            let mut rows = Column::new().spacing(16);
            if array(data, "skippedFiles").is_empty() {
                rows = rows.push((text("No skipped files.", type_::SM, look.ink_3)).boxed());
            }
            for v in array(data, "skippedFiles") {
                rows = rows.push(
                    (column![
                        text(format!("{} · {}", scalar(&v["library"]), scalar(&v["path"])), type_::SM, look.ink),
                        text(scalar(&v["reason"]), type_::XS, look.ink_3)
                    ]
                    .spacing(4))
                    .boxed(),
                );
            }
            card("Skipped files", rows, look)
        },
        Tab::Users => users(app, look),
        Tab::Clips => clips(app, look),
        Tab::Renames => renames(app, look),
    }
}
fn loading<'a>(look: &Look) -> Element<'a, AppMessage> {
    text("Loading…", type_::SM, look.ink_3).boxed()
}
fn summary<'a>(value: &Value, look: &Look) -> Element<'a, AppMessage> {
    let mut rows = Column::new().spacing(8);
    for (key, value) in value.as_object().into_iter().flatten() {
        if key.to_lowercase().contains("password") || key.to_lowercase().contains("apikey") {
            continue;
        }
        if value.is_object() || value.is_array() {
            continue;
        }
        rows = rows.push(
            (row![
                text(label(key), type_::SM, look.ink_2).width(Length::Fill),
                text(if value.is_null() { "Default".into() } else { scalar(value) }, type_::SM, look.ink)
            ]
            .spacing(16))
            .boxed(),
        );
    }
    rows.boxed()
}

fn account<'a>(app: &'a App, look: &Look) -> Element<'a, AppMessage> {
    let s = &app.settings;
    let mut cards = Column::new().spacing(20);
    cards = cards.push(card(
        "Notifications",
        row![
            text("System notifications", type_::SM, look.ink).width(Length::Fill),
            toggle(
                app.session.config.notifications,
                |on| AppMessage::Session(crate::session::Message::Notifications(on)),
                look
            )
        ]
        .spacing(16),
        look,
    ));
    if let Some(user) = app.browsing.viewer() {
        cards = cards.push(
            (card(
                "Profile",
                column![
                    crate::ui::media::avatar(
                        &user.username,
                        app.session
                            .config
                            .active()
                            .and_then(|s| s.user.as_ref())
                            .and_then(|u| u.avatar.as_deref())
                            .and_then(|path| app.session.images.get(path)),
                        64.0,
                        None,
                        look
                    ),
                    text(user.username.clone(), type_::SM, look.ink),
                    row![
                        action("Change picture", Action::new("Avatar", json!({"reset":false})), s.busy, look),
                        action(
                            "Remove picture",
                            Action::new("Avatar", json!({"reset":true})).confirm("Remove your profile picture?"),
                            s.busy,
                            look
                        )
                    ]
                    .spacing(8)
                ]
                .spacing(12),
                look,
            ))
            .boxed(),
        );
    }
    cards = cards.push(
        (card(
            "Password",
            edit(
                "Change password",
                Editor::new("Change password", "ChangePassword", "PasswordChange", "", json!({"current":"","new":""})),
                s.busy,
                look,
            ),
            look,
        ))
        .boxed(),
    );
    let email = s.data("MyEmail").map(|v| v["viewer"]["email"].clone()).unwrap_or(Value::Null);
    if app.session.connection.as_ref().is_some_and(|c| c.server.email) {
        cards = cards.push(
            (card(
                "Email",
                column![
                    text(
                        if email.is_null() { "No email address".into() } else { scalar(&email) },
                        type_::SM,
                        look.ink_2
                    ),
                    edit(
                        "Change email",
                        Editor::new("Email", "SetEmail", "EmailChange", "", json!({"email":email,"password":""})),
                        s.busy,
                        look
                    )
                ]
                .spacing(12),
                look,
            ))
            .boxed(),
        );
    }
    let mut keys = Column::new().spacing(10);
    if let Some(data) = s.data("Passkeys") {
        for k in array(&data["viewer"], "passkeys") {
            keys = keys.push(
                (column![
                    text(name(k), type_::SM, look.ink),
                    text(
                        format!("Added {} · last used {}", scalar(&k["createdAt"]), scalar(&k["lastUsed"])),
                        type_::XS,
                        look.ink_3
                    )
                ]
                .spacing(4))
                .boxed(),
            );
        }
    }
    keys = keys
        .push((text("Passkeys are read-only. Adding keys comes in a later version.", type_::SM, look.ink_3)).boxed());
    cards = cards.push((card("Passkeys", keys, look)).boxed());
    let mut passwords = Column::new().spacing(12);
    if let Some(data) = s.data("AppPasswords") {
        for p in array(data, "appPasswords") {
            passwords = passwords.push(
                (row![
                    column![
                        text(name(p), type_::SM, look.ink),
                        text(
                            format!("Created {} · last used {}", scalar(&p["createdAt"]), scalar(&p["lastUsed"])),
                            type_::XS,
                            look.ink_3
                        )
                    ]
                    .width(Length::Fill),
                    action(
                        "Revoke",
                        Action::new("DeleteAppPassword", json!({"id":p["id"]}))
                            .confirm(format!("Revoke {}? The app using it will be signed out.", name(p))),
                        s.busy,
                        look
                    )
                ]
                .spacing(8))
                .boxed(),
            );
        }
    }
    passwords = passwords.push(
        (edit(
            "New password",
            Editor::new("Music app password", "CreateAppPassword", "AppPasswordName", "", json!({"name":""})),
            s.busy,
            look,
        ))
        .boxed(),
    );
    cards.push((card("Music apps", passwords, look)).boxed()).boxed()
}

fn users<'a>(app: &'a App, look: &Look) -> Element<'a, AppMessage> {
    let s = &app.settings;
    let mut cards = Column::new().spacing(20);
    let mut list = Column::new().spacing(12);
    if let Some(data) = s.data("Users") {
        for u in array(data, "users") {
            let mut editor = Editor::new(
                format!("Edit {}", name(u)),
                "UpdateUser",
                "UserPatch",
                "/input",
                json!({"id":u["id"],"input":{"username":u["username"],"isAdmin":u["isAdmin"],"password":null,"permissions":u["overrides"]}}),
            );
            editor.confirmation=Some("Save this user's role, permissions and account changes? Admins can do everything, including removing you.".into());
            let mut controls = row![
                edit("Edit", editor, s.busy, look),
                action("Picture", Action::new("Avatar", json!({"userId":u["id"],"reset":false})), s.busy, look),
                action(
                    "Reset permissions",
                    Action::new("UpdateUser", json!({"id":u["id"],"input":{"permissions":{}}}))
                        .confirm("Reset all custom permissions to the defaults?"),
                    s.busy,
                    look
                )
            ];
            if app.browsing.viewer().is_none_or(|me| Some(me.id) != u["id"].as_i64()) {
                controls = controls.push(
                    (action(
                        "Remove",
                        Action::new("DeleteUser", json!({"id":u["id"]}))
                            .confirm(format!("Remove {}? Their watch history and requests go too.", name(u))),
                        s.busy,
                        look,
                    ))
                    .boxed(),
                );
            }
            list = list.push(
                (row![
                    column![
                        text(name(u), type_::SM, look.ink),
                        text(if u["isAdmin"] == true { "Admin" } else { "Member" }, type_::XS, look.ink_3)
                    ]
                    .width(Length::Fill),
                    controls.spacing(8).wrap()
                ]
                .spacing(12))
                .boxed(),
            );
        }
    }
    list=list.push((edit("Add person",Editor::new("Add person","CreateUser","NewUser","/input",json!({"input":{"username":"","password":"","isAdmin":false,"permissions":fields::default_value("PermissionOverridesInput")}})),s.busy,look)).boxed());
    cards = cards.push((card("People", list, look)).boxed());
    if let Some(data) = s.data("PermissionDefaults") {
        cards = cards.push(
            (card(
                "Default permissions",
                edit(
                    "Edit defaults",
                    Editor::new(
                        "Default permissions",
                        "SetPermissionDefaults",
                        "PermissionsInput",
                        "/permissions",
                        json!({"permissions":data["permissionDefaults"]}),
                    ),
                    s.busy,
                    look,
                ),
                look,
            ))
            .boxed(),
        );
    }
    let mut invites = Column::new().spacing(12);
    if let Some(data) = s.data("Invites") {
        for i in array(data, "invites") {
            invites = invites.push(
                (row![
                    column![
                        text(name(i), type_::SM, look.ink),
                        text(
                            format!(
                                "{} / {} accounts · expires {}{}",
                                scalar(&i["uses"]),
                                scalar(&i["maxUses"]),
                                scalar(&i["expiresAt"]),
                                if i["revoked"] == true { " · revoked" } else { "" }
                            ),
                            type_::XS,
                            look.ink_3
                        )
                    ]
                    .width(Length::Fill),
                    action(
                        "Revoke",
                        Action::new("RevokeInvite", json!({"id":i["id"]})).confirm("Revoke this invite?"),
                        s.busy,
                        look
                    )
                ]
                .spacing(8))
                .boxed(),
            );
        }
    }
    invites = invites.push(
        (edit(
            "New invite",
            Editor::new(
                "New invite",
                "CreateInvite",
                "InviteFields",
                "",
                json!({"label":"","maxUses":1,"expiresInHours":168,"email":null}),
            ),
            s.busy,
            look,
        ))
        .boxed(),
    );
    cards.push((card("Invites", invites, look)).boxed()).boxed()
}

fn clips<'a>(app: &'a App, look: &Look) -> Element<'a, AppMessage> {
    let s = &app.settings;
    let mut cards = Column::new().spacing(20);
    if let Some(data) = s.data("Settings") {
        cards = cards.push(
            (card(
                "General",
                column![
                    summary(&data["settings"]["clips"], look),
                    edit("Edit", config_editor(&data["settings"], "clips", "ClipsConfigInput"), s.busy, look)
                ]
                .spacing(16),
                look,
            ))
            .boxed(),
        );
    }
    if let Some(data) = s.data("ClipStorage") {
        let data = &data["clipStorage"];
        let mut usage = column![text(
            format!("{} bytes in {}", scalar(&data["bytes"]), scalar(&data["dir"])),
            type_::SM,
            look.ink_2
        )]
        .spacing(12);
        for u in array(data, "usage") {
            usage = usage.push(
                (text(
                    format!(
                        "{} · {} clips · {} rendered · {} bytes / {} MB",
                        name(&u["user"]),
                        scalar(&u["clips"]),
                        scalar(&u["rendered"]),
                        scalar(&u["bytes"]),
                        scalar(&u["storage"])
                    ),
                    type_::SM,
                    look.ink,
                ))
                .boxed(),
            );
        }
        usage = usage.push(
            (action(
                "Drop every render",
                Action::new("DropClipRenders", json!({}))
                    .confirm("Drop every rendered clip? Clips re-render when opened."),
                s.busy,
                look,
            ))
            .boxed(),
        );
        cards = cards.push((card("Storage", usage, look)).boxed());
        let mut public = Column::new().spacing(12);
        for c in array(data, "publicClips") {
            let mut controls = row![
                action("Turn link off", Action::new("UnpublishClip", json!({"id":c["id"]})), s.busy, look),
                action(
                    "Delete",
                    Action::new("AdminDeleteClip", json!({"id":c["id"]}))
                        .confirm(format!("Delete {}'s clip?", name(&c["owner"]))),
                    s.busy,
                    look
                )
            ]
            .spacing(8);
            if let Some(link) = c["link"].as_str().filter(|_| c["linkLive"] == true) {
                let link = app
                    .session
                    .api
                    .as_ref()
                    .and_then(|api| api.origin.join(link).ok())
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| link.into());
                controls = controls
                    .push((button("Copy link", look).small().on_press(msg(Message::Copy(link))).boxed()).boxed());
            }
            public = public.push(
                (column![text(format!("{} · {}", name(c), name(&c["owner"])), type_::SM, look.ink), controls]
                    .spacing(8))
                .boxed(),
            );
        }
        cards = cards.push((card("Public links", public, look)).boxed());
    }
    cards.boxed()
}

fn renames<'a>(app: &'a App, look: &Look) -> Element<'a, AppMessage> {
    let s = &app.settings;
    let mut list = Column::new().spacing(16);
    let selected: Vec<_> = s.selected_renames.iter().copied().collect();
    list = list.push(
        row![
            action("Look again", Action::new("RefreshRenameSuggestions", json!({})), s.busy, look),
            button("Select all / none", look).small().on_press(msg(Message::SelectAllRenames)).boxed(),
            action(
                format!("Rename {}", selected.len()),
                Action::new("ApplyRenames", json!({"ids":selected})).confirm("Rename the selected files?"),
                s.busy || selected.is_empty(),
                look
            ),
            action(
                "Dismiss selected",
                Action::new("DismissRenames", json!({"ids":selected})),
                s.busy || selected.is_empty(),
                look
            )
        ]
        .spacing(8)
        .boxed(),
    );
    list = list.push(
        row![
            text("Show less certain suggestions", type_::SM, look.ink_2).width(Length::Fill),
            toggle(s.show_low, |v| msg(Message::ShowLow(v)), look)
        ]
        .spacing(12)
        .boxed(),
    );
    if let Some(data) = s.data("RenameSuggestions") {
        let visible: Vec<_> =
            array(data, "renameSuggestions").iter().filter(|r| s.show_low || r["confidence"] == "HIGH").collect();
        if visible.is_empty() {
            list = list.push(text("Nothing to fix.", type_::SM, look.ink_3).boxed());
        }
        for r in visible {
            let id = r["id"].as_i64().unwrap_or_default();
            let mut item = column![
                row![
                    text(scalar(&r["library"]), type_::XS, look.ink_3).width(Length::Fill),
                    toggle(s.selected_renames.contains(&id), move |_| msg(Message::SelectRename(id)), look)
                ]
                .spacing(8),
                text(scalar(&r["src"]), type_::SM, look.ink_3),
                text(scalar(&r["dst"]), type_::SM, look.ink),
                text(
                    format!("{}{}", scalar(&r["reason"]), if r["confidence"] == "LOW" { " · guess" } else { "" }),
                    type_::XS,
                    look.ink_2
                )
            ]
            .spacing(6);
            if r["managed"] != true {
                item = item.push(text("Not managed; can't rename.", type_::XS, look.warn).boxed());
            }
            list = list.push(item.boxed());
        }
    }
    let mut history = Column::new().spacing(12);
    if let Some(data) = s.data("FileHistory") {
        for h in array(data, "fileHistory") {
            let mut item =
                column![text(format!("{} · {} files", scalar(&h["label"]), scalar(&h["count"])), type_::SM, look.ink)]
                    .spacing(8);
            for op in array(h, "operations") {
                item = item.push(
                    (text(format!("{} → {}", scalar(&op["src"]), scalar(&op["dst"])), type_::XS, look.ink_3)).boxed(),
                );
            }
            if h["undone"] != true {
                item = item.push(
                    (action(
                        "Undo",
                        Action::new("UndoFileChanges", json!({"batch":h["batch"]}))
                            .confirm("Undo this batch of file changes?"),
                        s.busy,
                        look,
                    ))
                    .boxed(),
                );
            }
            history = history.push((item).boxed());
        }
    }
    column![card("Rename suggestions", list, look), card("File history", history, look)].spacing(20).boxed()
}

fn appearance<'a>(app: &'a App, look: &Look) -> Element<'a, AppMessage> {
    let s = &app.settings;
    let mut cards = Column::new().spacing(20);
    if let Some(data) = s.data("AppearanceSettings") {
        let value = project(&data["appearanceSettings"], "AppearanceSettingsInput");
        cards = cards.push(
            (card(
                "Your appearance",
                column![
                    summary(&data["appearanceSettings"], look),
                    edit(
                        "Edit appearance",
                        Editor::new(
                            "Your appearance",
                            "SetAppearance",
                            "AppearanceSettingsInput",
                            "/input",
                            json!({"input":value})
                        ),
                        s.busy,
                        look
                    )
                ]
                .spacing(16),
                look,
            ))
            .boxed(),
        );
    }
    if app.browsing.viewer().is_some_and(|u| u.is_admin)
        && let Some(data) = s.data("ServerAppearance")
    {
        cards = cards.push(
            (card(
                "Server defaults",
                edit(
                    "Edit server appearance",
                    Editor::new(
                        "Server appearance",
                        "SetServerAppearance",
                        "ServerAppearanceInput",
                        "/input",
                        json!({"input":project(&data["serverAppearance"],"ServerAppearanceInput")}),
                    ),
                    s.busy,
                    look,
                ),
                look,
            ))
            .boxed(),
        );
    }
    let mut schemes = Column::new().spacing(16);
    if let Some(data) = s.data("ColorSchemes") {
        for scheme in array(data, "colorSchemes") {
            let id = scheme["id"].clone();
            let mut controls=row![action("Use scheme",Action::new("SetAppearance",json!({"input":{"colors":{"mode":"SINGLE","single":id,"light":id,"dark":id},"style":s.data("AppearanceSettings").map(|v|v["appearanceSettings"]["style"].clone()).unwrap_or(Value::Null),"mediaTint":s.data("AppearanceSettings").map(|v|v["appearanceSettings"]["mediaTint"].clone()).unwrap_or(json!(true))}})),s.busy,look),action("Fork",Action::new("ForkScheme",json!({"id":id})),s.busy,look)].spacing(8);
            if let Some(code) = scheme["shareCode"].as_str() {
                controls = controls.push(
                    (button("Copy ts1 code", look).small().on_press(msg(Message::Copy(code.into()))).boxed()).boxed(),
                );
            }
            if scheme["editable"] == true {
                let input = json!({"name":scheme["name"],"seeds":scheme["palette"]["seeds"],"overrides":scheme["palette"]["overrides"]});
                controls = controls
                    .push(
                        (edit(
                            "Edit",
                            Editor::new(
                                "Edit scheme",
                                "SaveScheme",
                                "SchemeInput",
                                "/input",
                                json!({"id":id,"input":input}),
                            ),
                            s.busy,
                            look,
                        ))
                        .boxed(),
                    )
                    .push(
                        (action(
                            if scheme["published"] == true { "Unpublish" } else { "Publish" },
                            Action::new("PublishScheme", json!({"id":id,"published":scheme["published"]!=true})),
                            s.busy,
                            look,
                        ))
                        .boxed(),
                    )
                    .push(
                        (action(
                            "Delete",
                            Action::new("DeleteScheme", json!({"id":id})).confirm(format!("Delete {}?", name(scheme))),
                            s.busy,
                            look,
                        ))
                        .boxed(),
                    );
            }
            schemes =
                schemes.push((column![text(name(scheme), type_::SM, look.ink), controls.wrap()].spacing(8)).boxed());
        }
    }
    let builtin = tinystream_theme::find_builtin("grey").expect("built-in Grey");
    let seeds: Vec<Value> = tinystream_theme::SEEDS
        .iter()
        .zip(builtin.scheme.seeds)
        .map(|(name, color)| json!({"name":name,"value":format!("#{:02x}{:02x}{:02x}",color.r,color.g,color.b)}))
        .collect();
    schemes = schemes.push(
        (row![
            edit(
                "New scheme",
                Editor::new(
                    "New scheme",
                    "SaveScheme",
                    "SchemeInput",
                    "/input",
                    json!({"id":null,"input":{"name":"","seeds":seeds,"overrides":[]}})
                ),
                s.busy,
                look
            ),
            edit(
                "Import ts1 code",
                Editor::new("Import scheme", "ImportScheme", "SchemeImport", "", json!({"code":"","name":null})),
                s.busy,
                look
            )
        ]
        .spacing(8))
        .boxed(),
    );
    cards.push((card("Color schemes", schemes, look)).boxed()).boxed()
}
fn scheme_preview<'a>(value: &Value, look: &Look) -> Element<'a, AppMessage> {
    let mut scheme = tinystream_theme::find_builtin("grey").expect("built-in Grey").scheme;
    let mut errors = Vec::new();
    for seed in array(value, "seeds") {
        let name = seed["name"].as_str().unwrap_or_default();
        match (
            tinystream_theme::SEEDS.iter().position(|s| *s == name),
            seed["value"].as_str().and_then(tinystream_theme::Color::parse),
        ) {
            (Some(i), Some(color)) if color.opaque() => scheme.seeds[i] = color,
            _ => errors.push(format!("Invalid seed: {name}")),
        }
    }
    for token in array(value, "overrides") {
        let name = token["name"].as_str().unwrap_or_default();
        match (tinystream_theme::token_index(name), token["value"].as_str().and_then(tinystream_theme::Color::parse)) {
            (Some(i), Some(color)) => {
                scheme.overrides.insert(i, color);
            },
            _ => errors.push(format!("Invalid token: {name}")),
        }
    }
    let tokens = scheme.tokens();
    let mut preview = Look::new(&tokens, look.style, true);
    preview.reduce_motion = look.reduce_motion;
    let mut body = column![
        text("Live preview", type_::LG, preview.ink),
        text("The quick brown fox jumps over the lazy dog.", type_::SM, preview.ink_2),
        row![
            button("Primary", &preview).primary().on_press(msg(Message::Cancel)).boxed(),
            button("Secondary", &preview).on_press(msg(Message::Cancel)).boxed(),
            toggle(true, |_| msg(Message::Cancel), &preview)
        ]
        .spacing(8)
    ]
    .spacing(16);
    for warning in tinystream_theme::contrast_warnings(&tokens) {
        body = body.push(
            (text(
                format!(
                    "Contrast: {} / {} · {:.2}:1 (minimum {:.1}:1)",
                    warning.foreground, warning.background, warning.ratio, warning.minimum
                ),
                type_::XS,
                preview.warn,
            ))
            .boxed(),
        );
    }
    for error in errors {
        body = body.push((text(error, type_::XS, preview.danger)).boxed());
    }
    card("Preview", body, &preview)
}

fn folder_picker<'a>(state: &'a super::Settings, editor: &Editor, look: &Look) -> Element<'a, AppMessage> {
    let mut body = Column::new().spacing(8);
    body = body.push(
        button("Browse folders on server", look)
            .small()
            .on_press(msg(Message::BrowseFolder(editor.value()["path"].as_str().unwrap_or_default().into())))
            .boxed(),
    );
    if let Some(data) = state.data("Folders") {
        let f = &data["folders"];
        let path = f["path"].as_str().unwrap_or_default();
        body = body.push(
            row![
                text(path.to_owned(), type_::SM, look.ink).width(Length::Fill),
                button("Use this folder", look)
                    .small()
                    .on_press(msg(Message::Set(format!("{}/path", editor.path), json!(path))))
                    .boxed()
            ]
            .spacing(8)
            .boxed(),
        );
        if let Some(parent) = f["parent"].as_str() {
            body = body
                .push(button("Up a folder", look).small().on_press(msg(Message::BrowseFolder(parent.into()))).boxed());
        }
        for folder in array(f, "folders") {
            body = body.push(
                button(name(folder), look)
                    .small()
                    .on_press(msg(Message::BrowseFolder(scalar(&folder["path"]))))
                    .boxed(),
            );
        }
    }
    card("Folders", body, look)
}

fn scheme_fields<'a>(editor: &'a Editor, look: &Look) -> Element<'a, AppMessage> {
    let value = editor.value();
    let mut body = column![
        text("Name", type_::SM, look.ink),
        field(editor, &value["name"], "String!", &format!("{}/name", editor.path), "name", look)
    ]
    .spacing(12);
    for (i, seed) in array(value, "seeds").iter().enumerate() {
        let path = format!("{}/seeds/{i}/value", editor.path);
        let display = editor.text.get(&path).cloned().unwrap_or_else(|| scalar(&seed["value"]));
        body = body.push(
            row![
                text(label(seed["name"].as_str().unwrap_or_default()), type_::SM, look.ink).width(Length::Fill),
                owned_input(
                    format!("settings:{path}"),
                    display,
                    move |v| msg(Message::Text(path.clone(), v, "Color")),
                    look
                )
                .boxed()
            ]
            .spacing(12)
            .boxed(),
        );
    }
    body = body.push(text("Token overrides", type_::SM, look.ink).boxed()).push(field(
        editor,
        &value["overrides"],
        "[TokenInput!]!",
        &format!("{}/overrides", editor.path),
        "overrides",
        look,
    ));
    for error in editor.invalid.values() {
        body = body.push(text(error.clone(), type_::XS, look.danger).boxed());
    }
    body.boxed()
}
