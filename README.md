<div align="center">

# tinystream

A small self-hosted media server for your shows, movies and music.

[![CI](https://github.com/tinystream-dev/tinystream/actions/workflows/ci.yml/badge.svg)](https://github.com/tinystream-dev/tinystream/actions/workflows/ci.yml)
[![Release](https://github.com/tinystream-dev/tinystream/actions/workflows/release.yml/badge.svg)](https://github.com/tinystream-dev/tinystream/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/tinystream-dev/tinystream?sort=semver)](https://github.com/tinystream-dev/tinystream/releases/latest)
[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust 1.94+](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org)

</div>

## Installation

### Prebuilt binaries

Download the archive for your architecture from the [latest release](https://github.com/tinystream-dev/tinystream/releases/latest), unpack it and run it:

```sh
tar xzf tinystream-*-x86_64.tar.gz
cd tinystream-*/
./tinystream
```

FFmpeg and libtorrent are linked in statically. At runtime tinystream needs a few common system libraries: OpenSSL, libstdc++, zlib, bzip2 and xz. GPU transcoding also needs libva and a VA-API driver; without them, tinystream transcodes on the CPU.

Music apps that speak Subsonic (Feishin, Symfonium, Tempo and others) connect to the same address. Each person makes a password per app under Settings → Account → Music apps.

On first run tinystream writes a commented config to `~/.config/tinystream/config.toml` and serves the UI on <http://localhost:3000>. The first account you create is the admin.

### Building from source

You need Rust 1.94 or newer, bun or npm (for the web UI), the `wasm32-unknown-unknown` target and the wasm-bindgen CLI at the version `web/decoder` pins (for the music player's decoder), and what the bundled FFmpeg and libtorrent builds use: curl, tar, git, make, meson, ninja, autotools, a C/C++17 compiler, libclang, and the OpenSSL, libdrm, zlib, bzip2 and xz headers.

```sh
git clone https://github.com/tinystream-dev/tinystream
cd tinystream
cargo build --release
```

The result is `target/release/tinystream`.

The first build compiles FFmpeg (with x264, dav1d, libva, libass, libopus and LAME) and libtorrent from source into `.native/`, which takes a few minutes. Each library is a crate under `native/`, so cargo's progress bar shows which one it's on, and each one's output goes to `.native/<library>-<version>/build.log`. Later builds reuse it, even after `cargo clean`.

| Variable | Effect |
| --- | --- |
| `TS_FFMPEG_NATIVE=1` | Tunes FFmpeg for your CPU (the result isn't portable). |
| `TINYSTREAM_NATIVE_DIR` | Builds the native libraries somewhere other than `.native/`. |
| `TINYSTREAM_SKIP_WEB_BUILD=1` | Embeds the UI already in `web/dist` instead of building it. |

### Cargo features

All on by default:

| Feature | What it adds |
| --- | --- |
| `web-ui` | Embeds the web UI into the binary. |
| `metadata` | Titles, artwork, descriptions and airing schedules from AniList, TMDB and TVmaze. Without it, you won't have metadata fetching |
| `torrent` | Downloads and library management: the torrent client, sources, monitoring, imports and renames. Implies `metadata`. |

Build with `--no-default-features` for a plain media server, or pick what you want, e.g. `--no-default-features --features web-ui,metadata`.

## Specials and season extras

Put special episodes in `Show/S00/`, `Show/Season 00/`, or `Show/Specials/`.
Numbered files such as `Show S00E01.mkv` keep their episode numbers; unnumbered
videos such as `OVA.mkv` are also playable. A single special is labelled
**Special**; multiple videos are labelled **Specials**.

Videos in subdirectories of a season are shown under **Extras** for that season:

```text
Show/
  Season 01/
    Show S01E01.mkv
    extras/
      OP.mkv
      ED.mkv
    extra/
      Show S01E01 Director's cut.mkv
  Specials/
    OVA.mkv
```

Extras use their filenames as titles and don't take an episode number or replace
a numbered episode. Hidden files, samples, and non-video files are ignored.
Subdirectories are scanned recursively without following directory symlinks.
Extras can be played and resumed, while ordinary episode playback skips them.

**Add shows** and **Downloads** have filters and labels for **Episodes / Seasons**,
**Specials**, **Movies**, and **Other / Mixed**. AniList's OVA and SPECIAL formats
are classified as specials. Movie discovery is shown separately; automatic movie
acquisition is not supported by the existing show downloader. Unidentified or
mixed downloads stay in Other / Mixed instead of being assumed to be movies.

## License

Released under the [AGPL-3.0-or-later](./LICENSE) open-source license.

### Invite links

Admins can create reusable invite links in **Settings → Users → Invite links**. Each link has a name, a maximum number of accounts (1–1000), and a lifetime (1 hour–365 days). A link stops working when either limit is reached, and admins can revoke it sooner. The default is 5 uses and 7 days.

Recipients choose their own username and password, then sign in automatically. New accounts are members and follow the server's default permissions. A failed sign-up does not consume a use. Copy the link when it is created: the database stores only its hash, so it cannot be shown again after leaving the page.

### Custom artwork

In a show's or movie's folder, `thumbnail.jpg` (also `.jpeg`, `.png`, `.webp`, or `.gif`) supplies its thumbnail, and `banner.*` supplies its banner. Existing `poster.*`, `folder.*`, `cover.*`, `backdrop.*`, `fanart.*`, and `background.*` names continue to work; `poster.*` and `backdrop.*` take precedence over the new aliases.

For an episode such as `Episode 1.mkv`, use `Episode 1.thumbnail.png` or `Episode 1.png` beside the video. The same image formats are supported; the `.thumbnail.*` form takes precedence. Existing same-name `.jpg` episode images remain supported.

Users with **Edit metadata** permission can change title thumbnails, banners, and episode thumbnails on the title page. Uploads accept PNG, JPEG, WebP, and GIF up to 8 MB. Corrupt images are rejected; decoding is limited to 8192 pixels per side and 64 MB. Uploaded artwork takes precedence over local files and provider artwork, survives metadata refreshes and library rescans, and is stored in tinystream's database without changing library files. **Reset** removes the uploaded override and restores the local, provider, or generated image.
