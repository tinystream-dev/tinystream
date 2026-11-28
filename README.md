<div align="center">

# tinystream

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logos/logo-inverted.svg">
  <img src="assets/logos/logo.svg" alt="tinystream logo" width="128">
</picture>

A small self-hosted media server for your shows, movies, music and books.

[![Latest release](https://img.shields.io/github/v/release/tinystream-dev/tinystream?sort=semver)](https://github.com/tinystream-dev/tinystream/releases/latest)
[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE)

<br>

<img src="assets/screenshots/home.webp" alt="The tinystream home page, continuing Sintel" width="100%">

<sub>Films by <a href="https://studio.blender.org/films/">Blender Studio</a>, CC BY.</sub>

</div>

## Features

- Shows, movies, music and books, all from one binary with the web app built in.
- Subtitles that look the way they were made to, styled ASS included, wherever you watch.
- Hardware transcoding with VA-API or NVENC, and the CPU when there's no GPU to use.
- Watch Together and Listen Together, for movie nights and shared albums with friends.
- Clips of any moment, shared with a link.
- Lyrics and loudness normalization, plus OpenSubsonic so the music apps you already like keep working.
- Requests, a calendar, and downloads from the RSS and Torznab indexers you add.
- Invite links for friends and family.
- Tailscale Serve and Funnel, set up for you.
- Apps for Android and the desktop.

## Apps

The web app comes with the server, so you don't need anything else to start watching. If you'd like something native:

- **Desktop** (Linux for now). It plays nearly everything as-is through mpv, so the server rarely has to transcode, and subtitles keep up with the picture far better than in a browser. It also has picture-in-picture, media keys, a tray icon, and downloads for when you're away from your server.
- **Android**, with the same library and subtitles in your pocket.

Both are on the [releases page](https://github.com/tinystream-dev/tinystream/releases/latest).

## Getting started

Point `/path/to/media` in [`docker-compose.yaml`](docker-compose.yaml) at your media, then:

```sh
mkdir -p config data && sudo chown 1000:1000 config data
docker compose up -d
```

Open <http://localhost:3000> and create the admin account. If you'd rather skip Docker, Linux binaries for x86_64 and arm64 are on the releases page too.

## License

Released under the [AGPL-3.0-or-later](./LICENSE) open-source license. Subtitles are drawn by [rustyass](https://github.com/tinystream-dev/rustyass), our pure-Rust take on libass, under the LGPL.
