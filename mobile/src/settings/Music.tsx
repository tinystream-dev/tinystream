// SPDX-License-Identifier: AGPL-3.0-or-later
import { View } from 'react-native'
import { Group, Input, Select, Toggle } from '../components/ui'
import { useMusic } from '../music/context'
import type { GainMode } from '@tinystream/shared/music-player'
import { useMe } from '../queries'
import { Labeled, useDraft } from './kit'
import { AppPasswords } from './Account'
export function MusicSettings() {
  const admin = useMe()?.isAdmin
  const { music, usePlayer } = useMusic(),
    settings = usePlayer((s) => s.settings)
  return (
    <View className="gap-6">
      <Group title="Playback">
        <Select
          value={settings.gain}
          onChange={(gain: GainMode) => music.settings({ gain })}
          options={[
            { value: 'off', label: 'ReplayGain off' },
            { value: 'track', label: 'Track gain' },
            { value: 'album', label: 'Album gain' },
            { value: 'auto', label: 'Automatic gain' },
          ]}
        />
        <Input
          accessibilityLabel="Crossfade seconds"
          keyboardType="numeric"
          value={String(settings.crossfade)}
          onChangeText={(value) => {
            const n = Number(value)
            if (Number.isFinite(n)) music.settings({ crossfade: Math.max(0, Math.min(12, n)) })
          }}
        />
      </Group>
      {admin && <ServerMusicSettings />}
      <AppPasswords />
    </View>
  )
}

function ServerMusicSettings() {
  const { draft, set, bar } = useDraft(
    (s) => s.music,
    (music) => ({ music }),
  )
  if (!draft) return null
  return (
    <View className="gap-3">
      <Group title="Server music">
        <Labeled label="Even out volume" hint="Measure songs without ReplayGain tags">
          <Toggle
            label="Even out volume"
            value={draft.analyzeLoudness}
            onChange={(v) =>
              set((d) => {
                d.analyzeLoudness = v
              })
            }
          />
        </Labeled>
        <Labeled label="Look up lyrics online">
          <Toggle
            label="Look up lyrics online"
            value={draft.onlineLyrics}
            onChange={(v) =>
              set((d) => {
                d.onlineLyrics = v
              })
            }
          />
        </Labeled>
        {draft.onlineLyrics && (
          <Labeled label="Lyrics server">
            <Input
              value={draft.lyricsUrl}
              onChangeText={(v) =>
                set((d) => {
                  d.lyricsUrl = v
                })
              }
            />
          </Labeled>
        )}
      </Group>
      {bar}
    </View>
  )
}
