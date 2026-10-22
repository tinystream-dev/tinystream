// SPDX-License-Identifier: AGPL-3.0-or-later
// Downloads settings (web's DownloadSettings.tsx): the torrent client,
// sources, quality profiles, automation, and renames. Sources are only ever
// what the admin adds.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { bytes, relative } from '@tinystream/shared/downloads'
import { ArrowRight, Check, CircleAlert, Plus, RefreshCw, Rss, Trash2, Undo2, Zap } from 'lucide-react-native'
import { type ReactNode, useMemo, useState } from 'react'
import { Pressable, ScrollView, Text, View } from 'react-native'
import { haptic } from '../../modules/haptics'
import { MonitorPicker } from '../components/downloads'
import { ask, toast, toastError } from '../components/Feedback'
import { Sheet } from '../components/Sheet'
import { ListSkeleton } from '../components/Skeleton'
import { Badge, Button, Checkbox, Chip, Divider, ErrorText, Field, Group, IconButton, Input, ListRow, Segmented, Select, Spinner, Toggle } from '../components/ui'
import { Squircle } from '../effects/Squircle'
import { graphql } from '../gql'
import type { DetectSourceQuery } from '../gql/graphql'
import { type ProfileConfig, type Seeding, type SourceConfig, useSettings } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { Labeled, csv, num, uncsv, useDraft, useSaved } from './kit'

const EngineQuery = graphql(`
  query SettingsEngine {
    downloadEngine {
      downloadPath
      killSwitch
    }
  }
`)

const AddSource = graphql(`
  mutation AddSource($input: SourceInput!) {
    addSource(input: $input) {
      raw
    }
  }
`)

const UpdateSource = graphql(`
  mutation UpdateSource($name: String!, $input: SourceInput!) {
    updateSource(name: $name, input: $input) {
      raw
    }
  }
`)

const RemoveSource = graphql(`
  mutation RemoveSource($name: String!) {
    removeSource(name: $name) {
      raw
    }
  }
`)

const DetectSource = graphql(`
  query DetectSource($url: String!, $apiKey: String) {
    detectSource(url: $url, apiKey: $apiKey) {
      kind
      url
      feed
      name
      searchable
      sample {
        link
        title
        size
        seeders
        published
      }
    }
  }
`)

const AddProfile = graphql(`
  mutation AddProfile($input: ProfileInput!) {
    addProfile(input: $input) {
      raw
    }
  }
`)

const UpdateProfile = graphql(`
  mutation UpdateProfile($name: String!, $input: ProfileInput!) {
    updateProfile(name: $name, input: $input) {
      raw
    }
  }
`)

const RemoveProfile = graphql(`
  mutation RemoveProfile($name: String!) {
    removeProfile(name: $name) {
      raw
    }
  }
`)

const RenameSuggestions = graphql(`
  query RenameSuggestions {
    renameSuggestions {
      id
      library
      managed
      root
      src
      dst
      reason
      confidence
    }
  }
`)

const FileHistory = graphql(`
  query FileHistory {
    fileHistory {
      batch
      label
      at
      count
      undone
      operations {
        kind
        src
        dst
      }
    }
  }
`)

const RefreshRenames = graphql(`
  mutation RefreshRenameSuggestions {
    refreshRenameSuggestions
  }
`)

const ApplyRenames = graphql(`
  mutation ApplyRenames($ids: [Int!]!) {
    applyRenames(ids: $ids) {
      renamed
      problems
    }
  }
`)

const DismissRenames = graphql(`
  mutation DismissRenames($ids: [Int!]!) {
    dismissRenames(ids: $ids)
  }
`)

const Undo = graphql(`
  mutation UndoFileChanges($batch: String!) {
    undoFileChanges(batch: $batch) {
      undone
      problems
    }
  }
`)

type Detected = DetectSourceQuery['detectSource']

const presets: { value: string; label: string; rules: Seeding }[] = [
  { value: 'none', label: "Don't seed", rules: { ratio: 0, time: null, idle: null, then: 'REMOVE' } },
  { value: 'ratio', label: 'Ratio 1.0', rules: { ratio: 1, time: null, idle: null, then: 'REMOVE' } },
  { value: 'week', label: '7 days', rules: { ratio: null, time: '7days', idle: null, then: 'REMOVE' } },
  { value: 'forever', label: 'Forever', rules: { ratio: null, time: null, idle: null, then: 'PAUSE' } },
]

function presetOf(s: Seeding) {
  return presets.find((p) => (p.rules.ratio ?? null) === (s.ratio ?? null) && (p.rules.time ?? null) === (s.time ?? null) && !s.idle)?.value ?? 'custom'
}

function describeSeeding(s: Seeding) {
  if (s.ratio === 0) return 'Torrents stop as soon as they finish.'
  const limits = [s.ratio != null && `ratio ${s.ratio}`, s.time && `${s.time}`, s.idle && `${s.idle} without anyone downloading`].filter(Boolean)
  if (!limits.length) return 'Torrents seed until you stop them.'
  const until = limits.length > 1 ? `${limits.join(', or ')}, whichever comes first` : limits[0]
  return `Torrents seed until ${until}; then they're ${s.then === 'PAUSE' ? 'paused' : 'removed'}.`
}

/** Pick a preset, or open up the rules behind them. */
function SeedingEditor({ value, onChange }: { value: Seeding; onChange: (s: Seeding) => void }) {
  const [custom, setCustom] = useState(presetOf(value) === 'custom')
  const current = custom ? 'custom' : presetOf(value)
  return (
    <View className="gap-3">
      <Segmented
        size="sm"
        value={current}
        onChange={(v) => {
          if (v === 'custom') return setCustom(true)
          setCustom(false)
          onChange(presets.find((p) => p.value === v)!.rules)
        }}
        options={[...presets.map((p) => ({ value: p.value, label: p.label })), { value: 'custom', label: 'Custom' }]}
      />
      {current === 'custom' && (
        <View className="gap-3">
          <View className="flex-row gap-2">
            <View className="flex-1">
              <Field label="Up to ratio">
                <Input value={value.ratio == null ? '' : String(value.ratio)} keyboardType="decimal-pad" placeholder="no limit" onChangeText={(v) => onChange({ ...value, ratio: v.trim() ? num(v) : null })} />
              </Field>
            </View>
            <View className="flex-1">
              <Field label="For at most">
                <Input value={value.time ?? ''} autoCapitalize="none" placeholder="e.g. 3days" onChangeText={(v) => onChange({ ...value, time: v.trim() || null })} />
              </Field>
            </View>
          </View>
          <View className="flex-row gap-2">
            <View className="flex-1">
              <Field label="Or idle for">
                <Input value={value.idle ?? ''} autoCapitalize="none" placeholder="e.g. 1day" onChangeText={(v) => onChange({ ...value, idle: v.trim() || null })} />
              </Field>
            </View>
            <View className="flex-1">
              <Field label="Then">
                <Select
                  title="Then"
                  value={value.then}
                  options={[
                    { value: 'REMOVE', label: 'Remove it' },
                    { value: 'PAUSE', label: 'Pause it' },
                  ]}
                  onChange={(then) => onChange({ ...value, then })}
                />
              </Field>
            </View>
          </View>
        </View>
      )}
      <Text className="font-sans text-xs leading-4 text-ink-3">{describeSeeding(value)}</Text>
    </View>
  )
}

const Row2 = ({ children }: { children: ReactNode }) => <View className="flex-row gap-2">{children}</View>
const Half = ({ children }: { children: ReactNode }) => <View className="flex-1">{children}</View>

export function Torrents() {
  const api = useApi()
  const { draft: d, set, bar } = useDraft(
    (s) => s.downloads,
    (downloads) => ({ downloads }),
  )
  const { data: engine } = useQuery({ queryKey: ['engine', 'path'], queryFn: async () => (await api.request(EngineQuery)).downloadEngine })
  if (!d) return { body: <ListSkeleton rows={4} height={120} /> }
  const kib = (v: number) => (v ? `${v}` : '')
  return {
    footer: bar,
    body: (
      <>
        <Group title="Storage">
          <Labeled label="Download folder">
            <Input value={d.path ?? ''} autoCapitalize="none" placeholder={engine?.downloadPath} onChangeText={(v) => set((c) => void (c.path = v.trim() || null))} />
          </Labeled>
          <Labeled
            label="Import method"
            hint={
              {
                AUTO: 'Hardlink on the same disk, otherwise copy while seeding or move.',
                HARDLINK: 'Same disk only.',
                COPY: 'Reflinks on btrfs and xfs.',
                MOVE: 'Can’t seed afterwards.',
              }[d.import]
            }
          >
            <Segmented
              size="sm"
              value={d.import}
              onChange={(v) => set((c) => void (c.import = v))}
              options={[
                { value: 'AUTO', label: 'Automatic' },
                { value: 'HARDLINK', label: 'Hardlink' },
                { value: 'COPY', label: 'Copy' },
                { value: 'MOVE', label: 'Move' },
              ]}
            />
          </Labeled>
        </Group>

        <Group title="Seeding">
          <View className="p-4">
            <SeedingEditor value={d.seeding} onChange={(s) => set((c) => void (c.seeding = s))} />
          </View>
        </Group>

        <Group title="Speed">
          <Labeled label="Download limit (KB/s)">
            <Input keyboardType="number-pad" value={kib(d.downloadLimit)} placeholder="unlimited" onChangeText={(v) => set((c) => void (c.downloadLimit = num(v)))} />
          </Labeled>
          <Labeled label="Upload limit (KB/s)">
            <Input keyboardType="number-pad" value={kib(d.uploadLimit)} placeholder="unlimited" onChangeText={(v) => set((c) => void (c.uploadLimit = num(v)))} />
          </Labeled>
          <Labeled label="Active downloads">
            <Input keyboardType="number-pad" value={d.maxActive ? String(d.maxActive) : ''} placeholder="no limit" onChangeText={(v) => set((c) => void (c.maxActive = num(v)))} />
          </Labeled>
        </Group>

        <Group title="Scheduled limits">
          <View className="gap-3 p-4">
            <Row2>
              <Half>
                <Field label="From">
                  <Input value={d.slowFrom ?? ''} placeholder="08:00" onChangeText={(v) => set((c) => void (c.slowFrom = v.trim() || null))} />
                </Field>
              </Half>
              <Half>
                <Field label="Until">
                  <Input value={d.slowTo ?? ''} placeholder="23:00" onChangeText={(v) => set((c) => void (c.slowTo = v.trim() || null))} />
                </Field>
              </Half>
            </Row2>
            <Row2>
              <Half>
                <Field label="Download (KB/s)">
                  <Input keyboardType="number-pad" value={kib(d.slowDownloadLimit)} placeholder="same" onChangeText={(v) => set((c) => void (c.slowDownloadLimit = num(v)))} />
                </Field>
              </Half>
              <Half>
                <Field label="Upload (KB/s)">
                  <Input keyboardType="number-pad" value={kib(d.slowUploadLimit)} placeholder="same" onChangeText={(v) => set((c) => void (c.slowUploadLimit = num(v)))} />
                </Field>
              </Half>
            </Row2>
          </View>
        </Group>

        <Group title="Network" description={engine?.killSwitch ? `${engine.killSwitch} is down right now; every torrent is paused.` : undefined}>
          <Labeled label="Bind to interface">
            <Input value={d.bindInterface ?? ''} autoCapitalize="none" placeholder="wg0" onChangeText={(v) => set((c) => void (c.bindInterface = v.trim() || null))} />
          </Labeled>
          <Labeled label="Proxy" hint="socks5://user:pass@host:1080 or http://host:8080">
            <Input value={d.proxy ?? ''} autoCapitalize="none" placeholder="none" onChangeText={(v) => set((c) => void (c.proxy = v.trim() || null))} />
          </Labeled>
          <Labeled label="Port">
            <Input keyboardType="number-pad" value={String(d.port)} onChangeText={(v) => set((c) => void (c.port = num(v)))} />
          </Labeled>
          <ListRow label="Open the port on the router (UPnP)" right={<Toggle label="UPnP" value={d.upnp} onChange={(v) => set((c) => void (c.upnp = v))} />} />
          <Divider />
          <ListRow label="Find peers without trackers (DHT)" right={<Toggle label="DHT" value={d.dht} onChange={(v) => set((c) => void (c.dht = v))} />} />
        </Group>
      </>
    ),
  }
}

export function Sources() {
  const api = useApi()
  const { tokens } = useTheme()
  const { data } = useSettings()
  const saved = useSaved()
  const [editing, setEditing] = useState<{ source: SourceConfig | null } | null>(null)
  const toggle = useMutation({
    mutationFn: (s: SourceConfig) => api.request(UpdateSource, { name: s.name, input: { ...s, enabled: !s.enabled } }),
    onSuccess: saved,
    onError: toastError,
  })
  if (!data) return <ListSkeleton rows={3} height={64} />
  return (
    <>
      <Button variant="primary" onPress={() => setEditing({ source: null })} icon={<Plus size={17} color={tokens['on-accent']} />}>
        Add a source
      </Button>
      {data.sources.length === 0 ? (
        <View className="items-center gap-3 py-8">
          <Rss size={24} color={tokens['ink-3']} />
          <Text className="font-sans text-sm text-ink-2">No sources yet</Text>
        </View>
      ) : (
        <Group>
          {data.sources.map((s, i) => (
            <View key={s.name}>
              {i > 0 && <Divider inset={60} />}
              <ListRow
                icon={
                  <View className="h-8 w-8 items-center justify-center rounded-lg bg-panel">
                    {s.kind === 'TORZNAB' ? <Zap size={16} color={tokens['ink-3']} /> : <Rss size={16} color={tokens['ink-3']} />}
                  </View>
                }
                label={
                  <Text className={`font-sans text-[15px] ${s.enabled ? 'text-ink' : 'text-ink-3'}`} numberOfLines={1}>
                    {s.name} <Text className="text-xs text-ink-3">{s.kind === 'TORZNAB' ? 'Torznab' : 'RSS'}</Text>
                  </Text>
                }
                hint={s.url.replace(/(apikey|passkey)=[^&]+/i, '$1=…')}
                onPress={() => setEditing({ source: s })}
                right={
                  <View className="flex-row items-center gap-2">
                    {s.seeding && <Badge>own seeding</Badge>}
                    <Toggle label={`Use ${s.name}`} value={s.enabled} onChange={() => toggle.mutate(s)} />
                  </View>
                }
              />
            </View>
          ))}
        </Group>
      )}
      <Sheet open={!!editing} onClose={() => setEditing(null)}>
        {editing && <SourceForm source={editing.source} onClose={() => setEditing(null)} />}
      </Sheet>
    </>
  )
}

function SourceForm({ source, onClose }: { source: SourceConfig | null; onClose: () => void }) {
  const api = useApi()
  const { tokens } = useTheme()
  const saved = useSaved()
  const { data: settings } = useSettings()
  const [s, setS] = useState<SourceConfig>(source ?? { name: '', kind: 'RSS', url: '', feed: null, apiKey: null, categories: [], enabled: true, downloadPath: null, seeding: null })
  const [link, setLink] = useState(source?.url ?? '')
  const [detected, setDetected] = useState<Detected | null>(null)
  const set = (patch: Partial<SourceConfig>) => setS((x) => ({ ...x, ...patch }))
  const detect = useMutation({
    mutationFn: async () => (await api.request(DetectSource, { url: link, apiKey: s.apiKey || null })).detectSource,
    onSuccess: (d) => {
      setDetected(d)
      setS((x) => ({
        ...x,
        kind: d.kind,
        url: d.url,
        feed: d.feed,
        name: x.name || d.name?.replace(/\s*[-–|].*$/, '').trim() || new URL(link.replace('{query}', '')).hostname,
      }))
    },
  })
  const done = () => {
    saved()
    onClose()
  }
  const save = useMutation({
    mutationFn: async () => void (source === null ? await api.request(AddSource, { input: s }) : await api.request(UpdateSource, { name: source.name, input: s })),
    onSuccess: done,
  })
  const remove = useMutation({ mutationFn: () => api.request(RemoveSource, { name: source!.name }), onSuccess: done, onError: toastError })
  const ready = !!s.name.trim() && !!s.url.trim()
  const global = settings?.downloads.seeding

  return (
    <ScrollView style={{ maxHeight: 640 }} keyboardShouldPersistTaps="handled">
      <View className="gap-4 pb-2">
        <Text className="font-sans text-lg font-semibold text-ink">{source ? `Edit ${source.name}` : 'Add a source'}</Text>
        <Field label="Link" hint="A Torznab API URL, an RSS search URL with {query}, or an RSS feed.">
          <View className="flex-row gap-2">
            <View className="flex-1">
              <Input value={link} onChangeText={setLink} placeholder="https://…" autoCapitalize="none" autoCorrect={false} keyboardType="url" />
            </View>
            <Button className="self-center" onPress={() => detect.mutate()} disabled={!link.trim() || detect.isPending}>
              {detect.isPending ? <Spinner /> : 'Check'}
            </Button>
          </View>
        </Field>
        <Field label="API key" hint="Torznab only.">
          <Input secureTextEntry autoCapitalize="none" value={s.apiKey ?? ''} onChangeText={(v) => set({ apiKey: v.trim() || null })} />
        </Field>
        {detect.error && (
          <View className="flex-row items-start gap-2">
            <CircleAlert size={16} color={tokens.danger} />
            <Text className="font-sans flex-1 text-sm text-danger">{(detect.error as Error).message}</Text>
          </View>
        )}
        {detected && (
          <Squircle radius={12} className="gap-1.5 bg-canvas p-3">
            <View className="flex-row items-center gap-2">
              <Check size={16} color={tokens.ok} />
              <Text className="font-sans flex-1 text-sm text-ink">
                {detected.kind === 'TORZNAB' ? 'A Torznab API' : detected.searchable ? 'An RSS feed that can search' : 'An RSS feed (watched, not searched)'}
                {detected.name && <Text className="text-ink-3"> · {detected.name}</Text>}
              </Text>
            </View>
            {detected.sample.slice(0, 5).map((r) => (
              <View key={r.link} className="flex-row gap-3">
                <Text className="font-sans flex-1 text-xs text-ink-3" numberOfLines={1}>
                  {r.title}
                </Text>
                <Text className="font-sans text-xs text-ink-3">{bytes(r.size)}</Text>
              </View>
            ))}
            {detected.sample.length === 0 && <Text className="font-sans text-xs text-ink-3">No results</Text>}
          </Squircle>
        )}
        {(detected || source) && (
          <>
            <Field label="Name">
              <Input value={s.name} onChangeText={(name) => set({ name })} />
            </Field>
            <Field label="Type">
              <Segmented
                value={s.kind}
                onChange={(kind) => set({ kind })}
                options={[
                  { value: 'RSS', label: 'RSS' },
                  { value: 'TORZNAB', label: 'Torznab' },
                ]}
              />
            </Field>
            <Field label={s.kind === 'TORZNAB' ? 'API URL' : 'Search URL'}>
              <Input value={s.url} onChangeText={(url) => set({ url })} autoCapitalize="none" autoCorrect={false} />
            </Field>
            {s.kind === 'RSS' && (
              <Field label="Feed of new releases">
                <Input value={s.feed ?? ''} onChangeText={(v) => set({ feed: v.trim() || null })} autoCapitalize="none" autoCorrect={false} />
              </Field>
            )}
            {s.kind === 'TORZNAB' && (
              <Field label="Categories" hint="Comma-separated, e.g. 5070 for anime. Empty searches everything.">
                <Input
                  keyboardType="numbers-and-punctuation"
                  value={(s.categories ?? []).join(', ')}
                  onChangeText={(v) => set({ categories: v.split(',').map((x) => Number(x.trim())).filter((x) => x > 0) })}
                />
              </Field>
            )}
            <Field label="Download folder">
              <Input value={s.downloadPath ?? ''} onChangeText={(v) => set({ downloadPath: v.trim() || null })} autoCapitalize="none" />
            </Field>
            <View className="flex-row items-center gap-3">
              <Text className="font-sans flex-1 text-[15px] text-ink">Custom seeding rules</Text>
              <Toggle
                label="Own seeding rules"
                value={!!s.seeding}
                onChange={(v) => set({ seeding: v ? (global ?? { ratio: 1, time: null, idle: null, then: 'REMOVE' }) : null })}
              />
            </View>
            {s.seeding && <SeedingEditor value={s.seeding} onChange={(seeding) => set({ seeding })} />}
          </>
        )}
        {save.error && <ErrorText>{(save.error as Error).message}</ErrorText>}
        <Button variant="primary" size="lg" disabled={!ready || save.isPending} onPress={() => save.mutate()}>
          {source === null ? 'Add source' : 'Save'}
        </Button>
        {source !== null && (
          <Button
            variant="danger"
            size="lg"
            icon={<Trash2 size={17} color={tokens.danger} />}
            onPress={async () => (await ask({ title: `Remove ${source.name}?`, confirm: 'Remove', danger: true })) && remove.mutate()}
          >
            Remove
          </Button>
        )}
      </View>
    </ScrollView>
  )
}

const RESOLUTIONS = ['2160p', '1080p', '720p', '576p', '480p']
const CODECS = ['hevc', 'av1', 'h264']

const blankProfile: ProfileConfig = {
  name: '',
  resolutions: ['1080p', '720p'],
  groups: [],
  require: [],
  reject: [],
  minSize: null,
  maxSize: null,
  codecs: [],
  preferDualAudio: false,
  batches: true,
  minSeeders: 1,
}

export function Profiles() {
  const { tokens } = useTheme()
  const { data } = useSettings()
  const [editing, setEditing] = useState<{ existing: boolean; profile: ProfileConfig } | null>(null)
  if (!data) return <ListSkeleton rows={3} height={64} />
  return (
    <>
      <Button variant="primary" onPress={() => setEditing({ existing: false, profile: blankProfile })} icon={<Plus size={17} color={tokens['on-accent']} />}>
        Add a profile
      </Button>
      {data.profiles.length === 0 ? (
        <Text className="font-sans text-sm text-ink-3">Built-in: 1080p › 720p › 2160p › 480p</Text>
      ) : (
        <Group>
          {data.profiles.map((p, i) => (
            <View key={p.name}>
              {i > 0 && <Divider />}
              <ListRow
                label={p.name}
                hint={[p.resolutions.join(' › ') || 'any resolution', p.groups.length ? p.groups.join(' › ') : null, p.preferDualAudio && 'dual audio', !p.batches && 'no packs']
                  .filter(Boolean)
                  .join(' · ')}
                right={
                  <View className="flex-row gap-1">
                    {data.libraries
                      .filter((l) => l.profile === p.name)
                      .map((l) => (
                        <Badge key={l.name}>{l.name}</Badge>
                      ))}
                  </View>
                }
                chevron
                onPress={() => setEditing({ existing: true, profile: p })}
              />
            </View>
          ))}
        </Group>
      )}
      <Sheet open={!!editing} onClose={() => setEditing(null)}>
        {editing && <ProfileForm existing={editing.existing} initial={editing.profile} onClose={() => setEditing(null)} />}
      </Sheet>
    </>
  )
}

/** Toggle chips whose order is the order you tapped them in. */
function OrderedChips({ all, value, onChange, label }: { all: string[]; value: string[]; onChange: (v: string[]) => void; label: (s: string) => string }) {
  return (
    <View className="flex-row flex-wrap gap-1.5">
      {all.map((x) => {
        const i = value.indexOf(x)
        return (
          <Chip key={x} on={i >= 0} onPress={() => onChange(i >= 0 ? value.filter((v) => v !== x) : [...value, x])}>
            {i >= 0 ? `${i + 1}  ${label(x)}` : label(x)}
          </Chip>
        )
      })}
    </View>
  )
}

function ProfileForm({ existing, initial, onClose }: { existing: boolean; initial: ProfileConfig; onClose: () => void }) {
  const api = useApi()
  const { tokens } = useTheme()
  const saved = useSaved()
  const [p, setP] = useState<ProfileConfig>(initial)
  const [groups, setGroups] = useState(csv(initial.groups))
  const [require, setRequire] = useState(csv(initial.require))
  const [reject, setReject] = useState(csv(initial.reject))
  const set = (x: Partial<ProfileConfig>) => setP((o) => ({ ...o, ...x }))
  const body = () => ({ ...p, groups: uncsv(groups), require: uncsv(require), reject: uncsv(reject) })
  const done = () => {
    saved()
    onClose()
  }
  const save = useMutation({
    mutationFn: async () => void (existing ? await api.request(UpdateProfile, { name: initial.name, input: body() }) : await api.request(AddProfile, { input: body() })),
    onSuccess: done,
  })
  const remove = useMutation({ mutationFn: () => api.request(RemoveProfile, { name: initial.name }), onSuccess: done })
  return (
    <ScrollView style={{ maxHeight: 640 }} keyboardShouldPersistTaps="handled">
      <View className="gap-4 pb-2">
        <Text className="font-sans text-lg font-semibold text-ink">{existing ? `Edit ${initial.name}` : 'New profile'}</Text>
        <Field label="Name">
          <Input value={p.name} onChangeText={(name) => set({ name })} placeholder="1080p subs" />
        </Field>
        <Field label="Resolutions, best first">
          <OrderedChips all={RESOLUTIONS} value={p.resolutions} onChange={(resolutions) => set({ resolutions })} label={(x) => x} />
        </Field>
        <Field label="Release groups, best first">
          <Input value={groups} onChangeText={setGroups} placeholder="SubsPlease, Erai-raws" autoCapitalize="none" />
        </Field>
        <Field label="Must contain" hint="Words or regular expressions.">
          <Input value={require} onChangeText={setRequire} autoCapitalize="none" />
        </Field>
        <Field label="Must not contain">
          <Input value={reject} onChangeText={setReject} placeholder="cam, hardsub" autoCapitalize="none" />
        </Field>
        <Row2>
          <Half>
            <Field label="Min size per episode (MB)">
              <Input keyboardType="number-pad" value={p.minSize == null ? '' : String(p.minSize)} placeholder="none" onChangeText={(v) => set({ minSize: v.trim() ? num(v) : null })} />
            </Field>
          </Half>
          <Half>
            <Field label="Max size per episode (MB)">
              <Input keyboardType="number-pad" value={p.maxSize == null ? '' : String(p.maxSize)} placeholder="none" onChangeText={(v) => set({ maxSize: v.trim() ? num(v) : null })} />
            </Field>
          </Half>
        </Row2>
        <Field label="Min seeders">
          <Input keyboardType="number-pad" value={String(p.minSeeders)} onChangeText={(v) => set({ minSeeders: num(v) })} />
        </Field>
        <Field label="Codecs, best first">
          <OrderedChips all={CODECS} value={p.codecs} onChange={(codecs) => set({ codecs })} label={(x) => x.toUpperCase()} />
        </Field>
        <View className="flex-row items-center gap-3">
          <Text className="font-sans flex-1 text-[15px] text-ink">Prefer dual audio</Text>
          <Toggle label="Prefer dual audio" value={p.preferDualAudio} onChange={(v) => set({ preferDualAudio: v })} />
        </View>
        <View className="flex-row items-center gap-3">
          <Text className="font-sans flex-1 text-[15px] text-ink">Allow season packs</Text>
          <Toggle label="Season packs" value={p.batches} onChange={(v) => set({ batches: v })} />
        </View>
        {save.error && <ErrorText>{(save.error as Error).message}</ErrorText>}
        {remove.error && <ErrorText>{(remove.error as Error).message}</ErrorText>}
        <Button variant="primary" size="lg" disabled={!p.name.trim() || save.isPending} onPress={() => save.mutate()}>
          Save
        </Button>
        {existing && (
          <Button variant="danger" size="lg" icon={<Trash2 size={17} color={tokens.danger} />} onPress={() => remove.mutate()}>
            Remove
          </Button>
        )}
      </View>
    </ScrollView>
  )
}

export function Automation() {
  const { tokens } = useTheme()
  const auto = useDraft(
    (s) => s.automation,
    (automation) => ({ automation }),
  )
  const req = useDraft(
    (s) => s.requests,
    (requests) => ({ requests }),
  )
  if (!auto.draft || !req.draft) return { body: <ListSkeleton rows={3} height={120} /> }
  const a = auto.draft
  const r = req.draft
  return {
    // Each part saves on its own, as on web; whichever was changed shows its bar.
    footer: auto.dirty ? auto.bar : req.bar,
    body: (
      <>
        <Group title="Default monitoring">
          <View className="p-4">
            <MonitorPicker value={a.defaultMonitor} onChange={(m) => auto.set((c) => void (c.defaultMonitor = m))} />
          </View>
        </Group>
        <Group title="Episode search">
          <View className="gap-2 p-4">
            {a.retry.map((step, i) => (
              <View key={i} className="flex-row items-center gap-2">
                <Text className="font-sans text-sm text-ink-3">every</Text>
                <View className="flex-1">
                  <Input value={step.every} autoCapitalize="none" onChangeText={(v) => auto.set((c) => void (c.retry[i].every = v))} />
                </View>
                <Text className="font-sans text-sm text-ink-3">until</Text>
                <View className="flex-1">
                  <Input value={step.until} autoCapitalize="none" onChangeText={(v) => auto.set((c) => void (c.retry[i].until = v))} />
                </View>
                <IconButton label="Remove step" onPress={() => auto.set((c) => void c.retry.splice(i, 1))}>
                  <Trash2 size={17} color={tokens['ink-3']} />
                </IconButton>
              </View>
            ))}
            <Text className="font-sans text-xs text-ink-3">…after it airs</Text>
            <View className="flex-row">
              <Button size="sm" variant="plain" icon={<Plus size={14} color={tokens['ink-2']} />} onPress={() => auto.set((c) => void c.retry.push({ every: '1day', until: '14days' }))}>
                Add a step
              </Button>
            </View>
          </View>
          <Labeled label="RSS check interval" hint="e.g. 15m">
            <Input value={a.rssInterval} autoCapitalize="none" onChangeText={(v) => auto.set((c) => void (c.rssInterval = v))} />
          </Labeled>
        </Group>
        <Group title="Renames">
          <ListRow
            label="Suggest renames for badly named files"
            right={<Toggle label="Rename suggestions" value={a.renameSuggestions} onChange={(v) => auto.set((c) => void (c.renameSuggestions = v))} />}
          />
        </Group>
        <Group title="Requests" description="Monitoring for approved requests">
          <View className="p-4">
            <MonitorPicker size="sm" value={r.monitor} onChange={(m) => req.set((c) => void (c.monitor = m))} />
          </View>
        </Group>
      </>
    ),
  }
}

/** The part of a path that changes, struck out and replaced. */
function Diff({ from, to, root }: { from: string; to: string; root: string | null }) {
  const { tokens } = useTheme()
  const strip = (p: string) => (root && p.startsWith(root) ? p.slice(root.length + 1) : p)
  const a = strip(from)
  const b = strip(to)
  let i = 0
  while (i < a.length && i < b.length && a[i] === b[i]) i++
  const cut = a.lastIndexOf('/', i) + 1
  const mono = { fontFamily: 'monospace', fontSize: 11.5, lineHeight: 17 }
  return (
    <View className="min-w-0 flex-1">
      <Text style={mono} className="text-ink-3">
        {a.slice(0, cut)}
        <Text className="text-danger line-through">{a.slice(cut)}</Text>
      </Text>
      <View className="flex-row items-start gap-1">
        <ArrowRight size={12} color={tokens['ink-3']} style={{ marginTop: 3 }} />
        <Text style={mono} className="flex-1 text-ink-2">
          {b.slice(0, cut)}
          <Text className="text-ok">{b.slice(cut)}</Text>
        </Text>
      </View>
    </View>
  )
}

export function Renames() {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const { data } = useQuery({ queryKey: ['renames'], queryFn: async () => (await api.request(RenameSuggestions)).renameSuggestions })
  const { data: history } = useQuery({ queryKey: ['history'], queryFn: async () => (await api.request(FileHistory)).fileHistory })
  const [selected, setSelected] = useState<Set<number>>(new Set())
  const [showLow, setShowLow] = useState(false)
  const done = () => {
    setSelected(new Set())
    void qc.invalidateQueries({ queryKey: ['renames'] })
    void qc.invalidateQueries({ queryKey: ['history'] })
  }
  const refresh = useMutation({ mutationFn: () => api.request(RefreshRenames), onSuccess: done, onError: toastError })
  const apply = useMutation({
    mutationFn: async (ids: number[]) => (await api.request(ApplyRenames, { ids })).applyRenames,
    onSuccess: (r) => (done(), r.problems.length === 0 && toast({ title: `Renamed ${r.renamed} file${r.renamed === 1 ? '' : 's'}`, tone: 'ok' })),
    onError: toastError,
  })
  const dismiss = useMutation({ mutationFn: (ids: number[]) => api.request(DismissRenames, { ids }), onSuccess: done, onError: toastError })
  const undo = useMutation({ mutationFn: async (batch: string) => (await api.request(Undo, { batch })).undoFileChanges, onSuccess: done, onError: toastError })
  const visible = useMemo(() => (data ?? []).filter((s) => showLow || s.confidence === 'HIGH'), [data, showLow])
  const low = (data ?? []).filter((s) => s.confidence === 'LOW').length
  const libraries = [...new Set(visible.map((s) => s.library))]
  const toggle = (id: number) => {
    haptic('tick')
    setSelected((s) => {
      const n = new Set(s)
      if (n.has(id)) n.delete(id)
      else n.add(id)
      return n
    })
  }
  const ids = [...selected]
  if (!data) return <ListSkeleton rows={3} height={80} />
  return (
    <>
      <Group
        title="Suggestions"
        aside={
          <IconButton label="Look again" size={32} onPress={() => refresh.mutate()} disabled={refresh.isPending}>
            <RefreshCw size={16} color={tokens['ink-2']} />
          </IconButton>
        }
      >
        <View className="gap-3 p-4">
          {visible.length === 0 && <Text className="font-sans text-sm text-ink-3">{data.length === 0 ? 'Nothing to fix.' : 'Only guesses left; show them below.'}</Text>}
          {visible.length > 0 && (
            <View className="flex-row flex-wrap items-center gap-2">
              <Button size="sm" onPress={() => setSelected(selected.size === visible.length ? new Set() : new Set(visible.map((s) => s.id)))}>
                {selected.size === visible.length ? 'Select none' : 'Select all'}
              </Button>
              <View className="flex-1" />
              <Button size="sm" variant="plain" disabled={!ids.length} onPress={() => dismiss.mutate(ids)}>
                Dismiss
              </Button>
              <Button size="sm" variant="primary" disabled={!ids.length || apply.isPending} onPress={() => apply.mutate(ids)}>
                {`Rename ${ids.length || ''}`}
              </Button>
            </View>
          )}
          {apply.data?.problems.map((p) => (
            <ErrorText key={p}>{p}</ErrorText>
          ))}
          {libraries.map((lib) => (
            <View key={lib} className="gap-1">
              <Text className="font-sans text-xs font-medium text-ink-3">{lib}</Text>
              {visible.some((s) => s.library === lib && !s.managed) && <Text className="font-sans text-xs text-warn">Not managed; can't rename.</Text>}
              {visible
                .filter((s) => s.library === lib)
                .map((s) => (
                  <Pressable key={s.id} onPress={() => toggle(s.id)} className="flex-row items-start gap-3 py-2">
                    <Checkbox checked={selected.has(s.id)} />
                    <View className="min-w-0 flex-1 gap-1">
                      <Diff from={s.src} to={s.dst} root={s.root} />
                      <View className="flex-row items-center gap-2">
                        <Text className="font-sans text-xs text-ink-3">{s.reason}</Text>
                        {s.confidence === 'LOW' && <Badge tone="warn">guess</Badge>}
                      </View>
                    </View>
                  </Pressable>
                ))}
            </View>
          ))}
          {low > 0 && (
            <Pressable onPress={() => setShowLow((v) => !v)}>
              <Text className="font-sans text-xs text-ink-3">
                {showLow ? 'Hide' : 'Show'} {low} less certain suggestion{low === 1 ? '' : 's'}
              </Text>
            </Pressable>
          )}
        </View>
      </Group>

      <Group title="History">
        {history?.length === 0 && <Text className="font-sans p-4 text-sm text-ink-3">Nothing yet.</Text>}
        {history?.map((b, i) => (
          <View key={b.batch}>
            {i > 0 && <Divider />}
            <ListRow
              label={
                <Text className={`font-sans text-[15px] ${b.undone ? 'text-ink-3 line-through' : 'text-ink'}`} numberOfLines={2}>
                  {b.label}
                </Text>
              }
              hint={`${relative(b.at)} · ${b.count} change${b.count === 1 ? '' : 's'}`}
              right={
                !b.undone ? (
                  <Button
                    size="sm"
                    variant="plain"
                    disabled={undo.isPending}
                    icon={<Undo2 size={14} color={tokens['ink-2']} />}
                    onPress={async () =>
                      (await ask(
                        b.batch.startsWith('import')
                          ? { title: 'Undo this import?', body: 'The imported files are removed from the library (the download keeps its copy).', confirm: 'Undo import' }
                          : { title: 'Put these files back the way they were?', confirm: 'Undo' },
                      )) && undo.mutate(b.batch)
                    }
                  >
                    Undo
                  </Button>
                ) : undefined
              }
            />
          </View>
        ))}
      </Group>
      {undo.data && undo.data.problems.length > 0 && <ErrorText>{undo.data.problems.join(' ')}</ErrorText>}
    </>
  )
}
