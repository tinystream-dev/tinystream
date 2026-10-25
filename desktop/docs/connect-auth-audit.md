# Connect and auth dependency audit

Audited 2026-10-25 for #83, following #81. David approved all twelve versions in the implementation conversation before compilation.

The lockfile preserves the existing desktop pins and reuses exact root/mobile versions for
HTTP, WebSockets, TLS, URL handling, anyhow and their dependencies. Resolution used only
`cargo metadata` (no crate code execution). `cargo metadata --locked` validates the result.
Secret Service uses the already audited zbus 5.19.0 directly, avoiding keyring and its dependencies.

Profile avatar rendering enables iced’s image feature. Its image decoder dependencies reuse root pins; four small runtime crates are new.

Twelve new registry crate/version pairs were audited and approved. The final build uses eleven:
`graphql_query_derive` was removed when code generation moved to the app build script so it
can reuse the shared Person and NotificationFields fragments. All remaining pairs are below:

| Crate | Version | Published | Build-time code | Findings |
| --- | --- | --- | --- | --- |
| graphql_client | 0.14.0 | 2024-03-26 | No | Publisher surma, unchanged; owner. Runtime response types; default derive feature disabled. |
| graphql_client_codegen | 0.14.0 | 2024-03-26 | Yes, used by app build script | Schema/query parsing and token generation; file access confined to supplied input paths. No network, subprocesses, environment scraping or obfuscation found. Publisher surma, unchanged; owner. |
| graphql-introspection-query | 0.2.0 | 2021-07-04 | No | Serde data types. Publisher tomhoule, unchanged; sole owner. |
| graphql-parser | 0.4.1 | 2024-12-03 | No | Parser used by codegen. Publisher tailhook → LegNeato; both remain owners. No I/O found. |
| guillotiere | 0.6.2 | 2021-08-10 | No | Texture atlas allocation. Publisher nical, unchanged; sole owner. |
| kamadak-exif | 0.6.1 | 2024-11-06 | No | Image metadata parsing. Publisher kamadak, unchanged; sole owner. |
| mutate_once | 0.1.2 | 2025-08-16 | No | Interior mutability for exif. Publisher kamadak, unchanged; sole owner. |
| svg_fmt | 0.4.5 | 2025-04-17 | No | Formatting for the texture atlas. Publisher nical, unchanged; sole owner. |
| heck | 0.4.1 | 2023-02-01 | No | Pure case conversion; older line required by codegen. Publisher jplatte, unchanged; owner. |
| syn | 1.0.109 | 2023-02-24 | Yes | Full build.rs read: invokes RUSTC --version and emits compatibility cfgs only. Older line required by codegen. Publisher dtolnay, unchanged; sole owner. |
| webpki-roots | 0.26.11 | 2025-05-06 | No | Tiny reexport of the root-locked 1.0.10 trust roots, required by tokio-tungstenite. Publisher ctz, unchanged; owner. |

All twelve versions are not yanked and older than 30 days. Dates, publishers, previous-version
publishers and current owners were checked against the crates.io API. The RustSec advisory-db
clone was refreshed before scanning: no advisories for these twelve versions. Existing findings
remain: tokio 1.53.1 RUSTSEC-2026-0386 (AsyncFd I/O safety; fixes require a separately audited
upgrade), and the previously accepted zbus_names/zvariant/zvariant_derive unmaintained notices.
No existing dependency was upgraded to address unrelated advisories in this change.

The chosen graphql_client 0.14 line is deliberately old and keeps its codegen stack small.
The app-owned build script reads the schema, shared fragments, session documents and shared logo,
then writes generated Rust into OUT_DIR. getrandom 0.3.4, used for server IDs, was already
locked and audited in the desktop foundation; enabling image formats also reuses root pins.
Network and Secret Service protocol implementations are app code, not additional dependencies.

The UI repair adds no crates. Geist is embedded from the web workspace’s installed
`@fontsource-variable/geist` Latin font, converted to TrueType; its OFL is included
in `app/assets/Geist-OFL.txt`. The shared vector logo is read by the build script.
