// SPDX-License-Identifier: AGPL-3.0-or-later
// The server's own settings (web's Server, RawConfig and Skipped tabs).

import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Text, TextInput, View } from 'react-native'
import { toast } from '../components/Feedback'
import { ListSkeleton } from '../components/Skeleton'
import { Divider, Group, Input, ListRow, Select, Toggle } from '../components/ui'
import { Squircle } from '../effects/Squircle'
import { graphql } from '../gql'
import { type Settings, useSettings } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { Labeled, SaveBar, uncsv, useDraft, useSaved } from './kit'

const ReplaceConfig = graphql(`
  mutation ReplaceConfig($text: String!) {
    replaceConfig(text: $text) {
      raw
    }
  }
`)

const SkippedQuery = graphql(`
  query SkippedFiles {
    skippedFiles {
      library
      path
      reason
    }
  }
`)

/** The parts of config.toml the General section edits. */
type ServerConfig = Pick<Settings, 'network' | 'log' | 'scan' | 'metadata' | 'transcode' | 'signIn'>
const serverPart = ({ network, log, scan, metadata, transcode, signIn }: Settings): ServerConfig => ({ network, log, scan, metadata, transcode, signIn })

export function General() {
  const { data, draft, set, bar } = useDraft(serverPart, (d) => d)
  if (!data || !draft) return { body: <ListSkeleton rows={5} height={120} /> }
  const t = data.transcoding
  return {
    footer: bar,
    body: (
      <>
        <Group title="Network">
          <Labeled label="Host">
            <Input value={draft.network.host} autoCapitalize="none" onChangeText={(v) => set((c) => void (c.network.host = v))} />
          </Labeled>
          <Labeled label="Port">
            <Input keyboardType="number-pad" value={String(draft.network.port)} onChangeText={(v) => set((c) => void (c.network.port = Number(v.replace(/\D/g, '')) || 0))} />
          </Labeled>
          <Labeled label="Allowed origins (CORS)" hint="Comma-separated, or *">
            <Input value={draft.network.cors.join(', ')} autoCapitalize="none" placeholder="https://example.com" onChangeText={(v) => set((c) => void (c.network.cors = uncsv(v)))} />
          </Labeled>
        </Group>

        <Group title="Sign-in">
          <Labeled label="Sign-in page">
            <Select
              title="Sign-in page"
              value={draft.signIn.style}
              options={[
                { value: 'PROFILES', label: 'Profile pictures' },
                { value: 'USERNAME', label: 'Username and password' },
              ]}
              onChange={(v) => set((c) => void (c.signIn.style = v))}
            />
          </Labeled>
        </Group>

        <Group title="Library scanning">
          <ListRow label="Watch folders for changes" right={<Toggle label="Watch folders" value={draft.scan.watch} onChange={(v) => set((c) => void (c.scan.watch = v))} />} />
          <Divider />
          <Labeled label="Rescan interval" hint="e.g. 6h">
            <Input value={draft.scan.interval ?? ''} placeholder="off" autoCapitalize="none" onChangeText={(v) => set((c) => void (c.scan.interval = v.trim() || null))} />
          </Labeled>
        </Group>

        <Group title="Metadata">
          <Labeled label="TMDB API key" hint="themoviedb.org → Settings → API">
            <Input secureTextEntry autoCapitalize="none" value={draft.metadata.tmdbApiKey ?? ''} onChangeText={(v) => set((c) => void (c.metadata.tmdbApiKey = v.trim() || null))} />
          </Labeled>
          <Labeled label="Language">
            <Input autoCapitalize="none" value={draft.metadata.language} onChangeText={(v) => set((c) => void (c.metadata.language = v))} />
          </Labeled>
        </Group>

        <Group title="Transcoding" description={t.vaapi ? `VA-API: ${t.vaapi}` : t.vaapiError ? `VA-API unavailable: ${t.vaapiError}` : undefined}>
          <Labeled label="Encoder">
            <Select
              title="Encoder"
              value={draft.transcode.hardware}
              options={[
                { value: 'AUTO', label: 'GPU, falling back to CPU' },
                { value: 'VAAPI', label: 'GPU (VA-API) only' },
                { value: 'SOFTWARE', label: 'CPU only' },
              ]}
              onChange={(v) => set((c) => void (c.transcode.hardware = v))}
            />
          </Labeled>
          <Labeled label="GPU device">
            <Input autoCapitalize="none" value={draft.transcode.vaapiDevice} onChangeText={(v) => set((c) => void (c.transcode.vaapiDevice = v))} />
          </Labeled>
        </Group>

        <Group title="Logs" description={data.paths.log}>
          <Labeled label="Level">
            <Select
              title="Log level"
              value={draft.log.level}
              options={['error', 'warn', 'info', 'debug', 'trace'].map((l) => ({ value: l, label: l }))}
              onChange={(v) => set((c) => void (c.log.level = v))}
            />
          </Labeled>
        </Group>
      </>
    ),
  }
}

export function ConfigFile() {
  const api = useApi()
  const { tokens } = useTheme()
  const { data } = useSettings()
  const [text, setText] = useState<string | null>(null)
  const saved = useSaved()
  const save = useMutation({
    mutationFn: () => api.request(ReplaceConfig, { text: text ?? '' }),
    onSuccess: () => {
      setText(null)
      toast({ title: 'Saved config.toml', tone: 'ok' })
      saved()
    },
  })
  if (!data) return { body: <ListSkeleton rows={1} height={400} /> }
  return {
    footer: (
      <SaveBar
        dirty={text !== null && text !== data.raw}
        error={(save.error as Error | null)?.message}
        busy={save.isPending}
        label="Save file"
        onSave={() => save.mutate()}
        onDiscard={() => (setText(null), save.reset())}
      />
    ),
    body: (
      <View className="gap-2">
        <Text className="font-sans px-1 text-xs text-ink-3">{data.paths.config}</Text>
        <Squircle radius={12} edge className="bg-canvas">
          <TextInput
            value={text ?? data.raw}
            onChangeText={setText}
            multiline
            autoCapitalize="none"
            autoCorrect={false}
            spellCheck={false}
            scrollEnabled={false}
            textAlignVertical="top"
            selectionColor={tokens.accent}
            style={{ minHeight: 420, padding: 14, color: tokens.ink, fontFamily: 'monospace', fontSize: 12.5, lineHeight: 19 }}
          />
        </Squircle>
      </View>
    ),
  }
}

export function Skipped() {
  const api = useApi()
  const { data } = useQuery({ queryKey: ['skipped'], queryFn: async () => (await api.request(SkippedQuery)).skippedFiles })
  const { data: settings } = useSettings()
  if (!data) return <ListSkeleton rows={3} />
  if (data.length === 0) return <Text className="font-sans text-sm text-ink-3">Nothing skipped.</Text>
  const roots = new Map(settings?.libraries.map((l) => [l.name, l.resolvedPath ?? '']))
  const groups = new Map<string, typeof data>()
  for (const s of data) groups.set(s.reason, [...(groups.get(s.reason) ?? []), s])
  return (
    <>
      {[...groups].map(([reason, items]) => (
        <Group key={reason} title={`${reason} · ${items.length}`}>
          {items.map((s, i) => (
            <View key={s.path}>
              {i > 0 && <Divider />}
              <View className="px-4 py-2.5">
                <Text className="font-sans text-xs text-ink-3" numberOfLines={3}>
                  <Text className="text-ink-2">{s.library}</Text>
                  {s.path.slice((roots.get(s.library) ?? '').length)}
                </Text>
              </View>
            </View>
          ))}
        </Group>
      ))}
    </>
  )
}
