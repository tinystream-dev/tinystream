// SPDX-License-Identifier: AGPL-3.0-or-later

import { ArrowUpRight } from 'lucide-react'
import type { ReactNode } from 'react'
import { Avatar } from './Avatar'
import { Card } from './SettingsKit'

const contributors = __CONTRIBUTORS__

const libraries = [
  { name: 'x264', license: 'GPL-2.0-or-later', href: 'https://www.videolan.org/developers/x264.html', does: 'H.264 encoding' },
  { name: 'dav1d', license: 'BSD-2-Clause', href: 'https://code.videolan.org/videolan/dav1d', does: 'AV1 decoding' },
  { name: 'LAME', license: 'LGPL-2.0-or-later', href: 'https://lame.sourceforge.io', does: 'MP3 encoding' },
  { name: 'Opus', license: 'BSD-3-Clause', href: 'https://opus-codec.org', does: 'Opus audio' },
  { name: 'zimg', license: 'WTFPL', href: 'https://github.com/sekrit-twc/zimg', does: 'Scaling and color conversion' },
  { name: 'libva', license: 'MIT', href: 'https://github.com/intel/libva', does: 'Hardware transcoding' },
  { name: 'rustyass', license: 'LGPL-3.0-or-later', href: 'https://github.com/tinystream-dev/rustyass', does: 'Subtitles' },
  { name: 'HarfRust', license: 'MIT', href: 'https://github.com/harfbuzz/harfrust', does: 'Text shaping' },
  { name: 'Fontations', license: 'MIT OR Apache-2.0', href: 'https://github.com/googlefonts/fontations', does: 'Font reading' },
  { name: 'unicode-bidi', license: 'MIT OR Apache-2.0', href: 'https://github.com/servo/unicode-bidi', does: 'Right-to-left text' },
  { name: 'libtorrent', license: 'BSD-3-Clause', href: 'https://libtorrent.org', does: 'Torrent downloads' },
  { name: 'Boost', license: 'BSL-1.0', href: 'https://www.boost.org', does: 'What libtorrent is built on' },
  { name: 'Noto Sans', license: 'OFL-1.1', href: 'https://notofonts.github.io', does: 'Default subtitle font' },
  { name: 'Symphonia', license: 'MPL-2.0', href: 'https://github.com/pdeljanov/Symphonia', does: 'Music decoding in the browser' },
]

export function Credits() {
  return (
    <>
      <Card title="Contributors" description="Everyone who has written code for this build of tinystream.">
        {contributors.length ? (
          <ul className="grid gap-1 sm:grid-cols-2">
            {contributors.map((c) => {
              const body = (
                <>
                  <Avatar user={{ id: null, username: c.name }} src={null} size={28} />
                  <span className="min-w-0 flex-1 truncate text-sm">{c.name}</span>
                  <span className="shrink-0 text-xs text-ink-3 tabular">
                    {c.commits} {c.commits === 1 ? 'commit' : 'commits'}
                  </span>
                </>
              )
              return (
                <li key={c.name}>
                  {c.github ? (
                    <a href={`https://github.com/${c.github}`} target="_blank" rel="noreferrer" className="flex items-center gap-3 rounded-lg p-1.5 transition-colors hover:bg-hover">
                      {body}
                    </a>
                  ) : (
                    <div className="flex items-center gap-3 p-1.5">{body}</div>
                  )}
                </li>
              )
            })}
          </ul>
        ) : (
          <p className="text-sm text-ink-3">This build was made without its git history, so there's no one to list.</p>
        )}
      </Card>
      <Card
        title="Open source"
        description={
          <>
            tinystream is built on the work of these projects, and of many Rust crates and web packages besides. Every release
            carries their full licenses in THIRD_PARTY_LICENSES; the web UI's are also{' '}
            <a href="/third-party-licenses.txt" target="_blank" rel="noreferrer" className="text-ink-2 underline underline-offset-2 hover:text-ink">
              here
            </a>
            .
          </>
        }
      >
        <a
          href="https://ffmpeg.org"
          target="_blank"
          rel="noreferrer"
          className="mb-4 flex flex-col gap-1 rounded-xl border border-line p-4 transition-colors hover:bg-hover"
        >
          <span className="inline-flex items-center gap-1 text-sm font-semibold">
            A huge thank you to FFmpeg
            <ArrowUpRight className="size-3.5 text-ink-3" />
          </span>
          <span className="text-xs leading-relaxed text-ink-3">
            Every video and song tinystream plays is probed, decoded, transcoded and muxed by FFmpeg. Without it, there would be no
            tinystream. Used under the GPL-2.0-or-later.
          </span>
        </a>
        <ul className="grid gap-1 sm:grid-cols-2">
          {libraries.map((l) => (
            <li key={l.name}>
              <a href={l.href} target="_blank" rel="noreferrer" className="flex items-baseline gap-3 rounded-lg p-1.5 transition-colors hover:bg-hover">
                <span className="min-w-0 flex-1 truncate text-sm">
                  {l.name} <span className="text-ink-3">· {l.does}</span>
                </span>
                <span className="shrink-0 text-xs text-ink-3">{l.license}</span>
              </a>
            </li>
          ))}
        </ul>
      </Card>
      <Card title="Metadata" description="Titles, artwork and episode details come from these services.">
        <div className="flex flex-col gap-5">
          <Source href="https://anilist.co" logo={<span className="text-sm font-semibold">AniList</span>}>
            A huge thank you to AniList for all of tinystream's anime information. tinystream is not affiliated with or endorsed by
            AniList.
          </Source>
          <Source href="https://www.themoviedb.org" logo={<img src="/tmdb.svg" alt="TMDB" className="h-3.5" />}>
            tinystream uses TMDB and the TMDB APIs but is not endorsed, certified, or otherwise approved by TMDB.
          </Source>
        </div>
      </Card>
    </>
  )
}

function Source({ href, logo, children }: { href: string; logo: ReactNode; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <a href={href} target="_blank" rel="noreferrer" className="inline-flex w-fit items-center gap-1 text-ink-2 hover:text-ink">
        {logo}
        <ArrowUpRight className="size-3.5 text-ink-3" />
      </a>
      <p className="text-xs leading-relaxed text-ink-3">{children}</p>
    </div>
  )
}
