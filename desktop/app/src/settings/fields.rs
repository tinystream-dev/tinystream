// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::{Value, json};

pub fn fields(name: &str) -> &'static [(&'static str, &'static str)] {
    match name {
        "AppearanceSettingsInput" => {
            &[("colors", "SchemeChoiceInput"), ("style", "ComponentStyle"), ("mediaTint", "Boolean!")]
        },
        "AutomationConfigInput" => &[
            ("defaultMonitor", "Monitor!"),
            ("rssInterval", "Duration!"),
            ("retry", "[RetryStepInput!]!"),
            ("renameSuggestions", "Boolean!"),
        ],
        "ClipsConfigInput" => &[
            ("enabled", "Boolean!"),
            ("path", "String"),
            ("publicLinks", "Boolean!"),
            ("concurrency", "Int!"),
            ("maxStorage", "Int!"),
            ("fontsDir", "String"),
            ("defaultFont", "String"),
        ],
        "DownloadsConfigInput" => &[
            ("path", "String"),
            ("import", "ImportMode!"),
            ("port", "Int!"),
            ("upnp", "Boolean!"),
            ("dht", "Boolean!"),
            ("maxActive", "Int!"),
            ("downloadLimit", "Int!"),
            ("uploadLimit", "Int!"),
            ("slowDownloadLimit", "Int!"),
            ("slowUploadLimit", "Int!"),
            ("slowFrom", "String"),
            ("slowTo", "String"),
            ("bindInterface", "String"),
            ("proxy", "String"),
            ("seeding", "SeedingInput!"),
        ],
        "LibraryInput" => &[
            ("name", "String!"),
            ("path", "String!"),
            ("kind", "LibraryKind!"),
            ("metadataProvider", "Provider"),
            ("managed", "Boolean!"),
            ("profile", "String"),
            ("downloadPath", "String"),
        ],
        "LogConfigInput" => &[("level", "String!")],
        "MetadataConfigInput" => &[("tmdbApiKey", "String"), ("language", "String!")],
        "MusicConfigInput" => {
            &[("onlineLyrics", "Boolean!"), ("lyricsUrl", "String!"), ("analyzeLoudness", "Boolean!")]
        },
        "NetworkConfigInput" => &[
            ("host", "String!"),
            ("port", "Int!"),
            ("cors", "[String!]!"),
            ("trustedProxies", "[String!]!"),
            ("publicUrl", "String"),
        ],
        "NewUser" => &[
            ("username", "String!"),
            ("password", "String!"),
            ("isAdmin", "Boolean!"),
            ("permissions", "PermissionOverridesInput!"),
        ],
        "PermissionOverridesInput" => &[
            ("allLibraries", "Boolean"),
            ("libraries", "[String!]"),
            ("request", "Boolean"),
            ("autoApprove", "Boolean"),
            ("requestLimit", "Int"),
            ("manageRequests", "Boolean"),
            ("manageShows", "Boolean"),
            ("downloads", "Boolean"),
            ("editMetadata", "Boolean"),
            ("watchTogether", "Boolean"),
            ("shareLinks", "Boolean"),
            ("clip", "Boolean"),
            ("clipMaxLength", "Int"),
            ("clipLimit", "Int"),
            ("clipStorage", "Int"),
            ("clipLinks", "Boolean"),
        ],
        "PermissionsInput" => &[
            ("allLibraries", "Boolean!"),
            ("libraries", "[String!]!"),
            ("request", "Boolean!"),
            ("autoApprove", "Boolean!"),
            ("requestLimit", "Int!"),
            ("manageRequests", "Boolean!"),
            ("manageShows", "Boolean!"),
            ("downloads", "Boolean!"),
            ("editMetadata", "Boolean!"),
            ("watchTogether", "Boolean!"),
            ("shareLinks", "Boolean!"),
            ("clip", "Boolean!"),
            ("clipMaxLength", "Int!"),
            ("clipLimit", "Int!"),
            ("clipStorage", "Int!"),
            ("clipLinks", "Boolean!"),
        ],
        "ProfileInput" => &[
            ("name", "String!"),
            ("resolutions", "[String!]!"),
            ("groups", "[String!]!"),
            ("require", "[String!]!"),
            ("reject", "[String!]!"),
            ("minSize", "Int"),
            ("maxSize", "Int"),
            ("codecs", "[String!]!"),
            ("preferDualAudio", "Boolean!"),
            ("batches", "Boolean!"),
            ("minSeeders", "Int!"),
        ],
        "RequestsConfigInput" => &[("monitor", "Monitor!")],
        "RetryStepInput" => &[("every", "Duration!"), ("until", "Duration!")],
        "ScanConfigInput" => &[("watch", "Boolean!"), ("interval", "Duration")],
        "SchemeChoiceInput" => {
            &[("mode", "SchemeMode!"), ("single", "String!"), ("light", "String!"), ("dark", "String!")]
        },
        "SchemeInput" => &[("name", "String!"), ("seeds", "[TokenInput!]!"), ("overrides", "[TokenInput!]!")],
        "SeedingInput" => &[("ratio", "Float"), ("time", "Duration"), ("idle", "Duration"), ("then", "SeedAction!")],
        "ServerAppearanceInput" => &[("colors", "SchemeChoiceInput!"), ("style", "ComponentStyle!")],
        "SignInConfigInput" => &[("style", "SignInStyle!"), ("publicProfiles", "Boolean!")],
        "SmtpConfigInput" => &[
            ("host", "String"),
            ("port", "Int"),
            ("security", "SmtpSecurity!"),
            ("username", "String"),
            ("password", "String"),
            ("from", "String"),
        ],
        "SourceInput" => &[
            ("name", "String!"),
            ("kind", "SourceKind!"),
            ("url", "String!"),
            ("feed", "String"),
            ("apiKey", "String"),
            ("categories", "[Int!]!"),
            ("enabled", "Boolean!"),
            ("downloadPath", "String"),
            ("seeding", "SeedingInput"),
        ],
        "TailscaleConfigInput" => {
            &[("detect", "Boolean!"), ("socket", "String"), ("expose", "Expose!"), ("httpsPort", "Int!")]
        },
        "TokenInput" => &[("name", "String!"), ("value", "String!")],
        "TranscodeConfigInput" => &[("hardware", "Hardware!"), ("vaapiDevice", "String!"), ("cudaDevice", "Int!")],
        "UserPatch" => &[
            ("username", "String"),
            ("isAdmin", "Boolean"),
            ("password", "String"),
            ("permissions", "PermissionOverridesInput"),
        ],
        "SourceDetection" => &[("url", "String!"), ("apiKey", "String")],
        "TestEmail" => &[("to", "String!")],
        "UserName" => &[("username", "String!")],
        "PasswordChange" => &[("current", "String!"), ("new", "String!")],
        "EmailChange" => &[("email", "String"), ("password", "String!")],
        "AppPasswordName" => &[("name", "String!")],
        "InviteFields" => &[("label", "String!"), ("maxUses", "Int!"), ("expiresInHours", "Int!"), ("email", "String")],
        "RawConfig" => &[("text", "String!")],
        "SchemeImport" => &[("code", "String!"), ("name", "String")],
        _ => &[],
    }
}
pub fn options(name: &str) -> &'static [&'static str] {
    match name {
        "ComponentStyle" => &["LAYERED", "FLAT", "GLASS"],
        "Expose" => &["MANUAL", "TAILNET", "PUBLIC"],
        "Hardware" => &["AUTO", "VAAPI", "NVENC", "SOFTWARE"],
        "ImportMode" => &["AUTO", "HARDLINK", "COPY", "MOVE"],
        "LibraryKind" => &["VIDEO", "MUSIC", "BOOKS"],
        "Monitor" => &["NONE", "FUTURE", "MISSING"],
        "Provider" => &["ANILIST", "TMDB"],
        "SchemeMode" => &["SINGLE", "SYSTEM"],
        "SeedAction" => &["PAUSE", "REMOVE"],
        "SignInStyle" => &["PROFILES", "USERNAME"],
        "SmtpSecurity" => &["STARTTLS", "TLS", "NONE"],
        "SourceKind" => &["TORZNAB", "RSS"],
        _ => &[],
    }
}
pub fn default_value(kind: &str) -> Value {
    let base = kind.trim_end_matches('!');
    if base.starts_with('[') {
        return json!([]);
    }
    match base {
        "SchemeChoiceInput" => json!({"mode":"SINGLE","single":"grey","light":"grey","dark":"grey"}),
        "Boolean" => json!(false),
        "Int" | "Float" => json!(0),
        "String" | "Duration" => json!(""),
        _ if !options(base).is_empty() => json!(options(base)[0]),
        _ => {
            let mut value = serde_json::Map::new();
            for (key, ty) in fields(base) {
                value.insert((*key).into(), if ty.ends_with('!') { default_value(ty) } else { Value::Null });
            }
            Value::Object(value)
        },
    }
}
