// SPDX-License-Identifier: AGPL-3.0-or-later
// Who and what tinystream is built on (web's Credits.tsx), and the licenses
// the APK carries.

import { useQuery } from '@tanstack/react-query'
import { ArrowUpRight, FileText } from 'lucide-react-native'
import { useState } from 'react'
import { Linking, Text, View } from 'react-native'
import { FlatList } from 'react-native-gesture-handler'
import { thirdPartyLicenses } from '../../modules/about'
import { Sheet } from '../components/Sheet'
import { Divider, Group, ListRow, Spinner } from '../components/ui'
import { useTheme } from '../theme/ThemeProvider'

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
  { name: 'Symphonia', license: 'MPL-2.0', href: 'https://github.com/pdeljanov/Symphonia', does: 'Music decoding' },
]

export function Credits() {
  const { tokens } = useTheme()
  const [open, setOpen] = useState(false)
  const arrow = <ArrowUpRight size={16} color={tokens['ink-3']} />
  return (
    <>
      <Group
        title="Open source"
        description="tinystream is built on the work of these projects, and of many Rust crates and JavaScript packages besides. The app carries their full licenses."
      >
        <ListRow icon={<FileText size={20} color={tokens['ink-2']} />} label="Licenses in this app" onPress={() => setOpen(true)} />
      </Group>

      <Group>
        <ListRow
          label="A huge thank you to FFmpeg"
          hint="Every video and song tinystream plays is probed, decoded, transcoded and muxed by FFmpeg. Without it, there would be no tinystream. Used under the GPL-2.0-or-later."
          right={arrow}
          onPress={() => void Linking.openURL('https://ffmpeg.org')}
        />
        {libraries.map((l) => (
          <View key={l.name}>
            <Divider />
            <ListRow label={`${l.name} · ${l.does}`} value={l.license} right={arrow} onPress={() => void Linking.openURL(l.href)} />
          </View>
        ))}
      </Group>

      <Group title="Metadata" description="Titles, artwork and episode details come from these services.">
        <ListRow
          label="AniList"
          hint="A huge thank you to AniList for all of tinystream's anime information. tinystream is not affiliated with or endorsed by AniList."
          right={arrow}
          onPress={() => void Linking.openURL('https://anilist.co')}
        />
        <Divider />
        <ListRow
          label="TMDB"
          hint="tinystream uses TMDB and the TMDB APIs but is not endorsed, certified, or otherwise approved by TMDB."
          right={arrow}
          onPress={() => void Linking.openURL('https://www.themoviedb.org')}
        />
      </Group>

      <Sheet open={open} onClose={() => setOpen(false)}>
        {open && <Licenses />}
      </Sheet>
    </>
  )
}

function Licenses() {
  const { data, isPending } = useQuery({ queryKey: ['third-party-licenses'], queryFn: thirdPartyLicenses, staleTime: Infinity })
  if (isPending)
    return (
      <View className="items-center py-10">
        <Spinner />
      </View>
    )
  if (!data) return <Text className="font-sans py-6 text-sm text-ink-2">The licenses are put in release builds; this one is for development.</Text>
  return (
    <FlatList
      style={{ maxHeight: 600 }}
      data={chunks(data)}
      keyExtractor={(_, i) => String(i)}
      initialNumToRender={4}
      renderItem={({ item }) => (
        <Text className="text-[11px] leading-4 text-ink-2" style={{ fontFamily: 'monospace' }}>
          {item}
        </Text>
      )}
    />
  )
}

/** The file in pieces small enough to lay out one at a time: it runs to megabytes. */
function chunks(text: string, lines = 80) {
  const all = text.split('\n')
  return Array.from({ length: Math.ceil(all.length / lines) }, (_, i) => all.slice(i * lines, (i + 1) * lines).join('\n'))
}
