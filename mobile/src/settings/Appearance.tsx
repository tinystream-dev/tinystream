// SPDX-License-Identifier: AGPL-3.0-or-later
// Colours and style (web's AppearanceSettings.tsx), for you and, for admins,
// for everyone. Changes come back through the server's appearance, so the
// app takes them on as soon as they're saved.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { SEEDS, type SeedName, TOKENS, type TokenName, type Tokens, type Warning, contrastWarnings, derive, parseColor, toRecord } from '@tinystream/shared/theme'
import * as Clipboard from 'expo-clipboard'
import { CircleAlert, Copy, GitFork, Pencil, Pin, PinOff, Plus, Send, Trash2, Upload } from 'lucide-react-native'
import { useEffect, useState } from 'react'
import { Pressable, ScrollView, Text, View } from 'react-native'
import { haptic } from '../../modules/haptics'
import { ask, toast, toastError } from '../components/Feedback'
import { Menu } from '../components/Menu'
import { Sheet } from '../components/Sheet'
import { ListSkeleton } from '../components/Skeleton'
import { Badge, Button, Divider, ErrorText, Field, Group, IconButton, Input, ListRow, Progress, Segmented, Select, Toggle } from '../components/ui'
import { Squircle } from '../effects/Squircle'
import { graphql } from '../gql'
import type { AppearanceSettingsInput, ComponentStyle, SchemeChoiceInput, SchemeFieldsFragment, SchemeMode } from '../gql/graphql'
import { useMe } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { Labeled } from './kit'

type Scheme = SchemeFieldsFragment

graphql(`
  fragment SchemeFields on ColorScheme {
    id
    name
    builtIn
    published
    editable
    code
    shareCode
    forkedFrom {
      id
      name
    }
    palette {
      seeds {
        name
        value
      }
      overrides {
        name
        value
      }
      tokens {
        name
        value
      }
      warnings {
        foreground
        background
        ratio
        minimum
      }
    }
  }
`)

const SchemesQuery = graphql(`
  query ColorSchemes {
    colorSchemes {
      ...SchemeFields
    }
  }
`)

const SettingsQuery = graphql(`
  query AppearanceSettings {
    appearanceSettings {
      colors {
        mode
        single
        light
        dark
      }
      style
      mediaTint
    }
  }
`)

const ServerQuery = graphql(`
  query ServerAppearance {
    serverAppearance {
      colors {
        mode
        single
        light
        dark
      }
      style
    }
  }
`)

const SetAppearance = graphql(`
  mutation SetAppearance($input: AppearanceSettingsInput!) {
    setAppearance(input: $input) {
      mode
    }
  }
`)

const SetServerAppearance = graphql(`
  mutation SetServerAppearance($input: ServerAppearanceInput!) {
    setServerAppearance(input: $input) {
      style
    }
  }
`)

const SaveScheme = graphql(`
  mutation SaveScheme($id: String, $input: SchemeInput!) {
    saveScheme(id: $id, input: $input) {
      ...SchemeFields
    }
  }
`)

const ForkScheme = graphql(`
  mutation ForkScheme($id: String!) {
    forkScheme(id: $id) {
      ...SchemeFields
    }
  }
`)

const ImportScheme = graphql(`
  mutation ImportScheme($code: String!, $name: String) {
    importScheme(code: $code, name: $name) {
      ...SchemeFields
    }
  }
`)

const DeleteScheme = graphql(`
  mutation DeleteScheme($id: String!) {
    deleteScheme(id: $id)
  }
`)

const PublishScheme = graphql(`
  mutation PublishScheme($id: String!, $published: Boolean!) {
    publishScheme(id: $id, published: $published) {
      id
    }
  }
`)

const DecodeScheme = graphql(`
  query DecodeScheme($code: String!) {
    decodeScheme(code: $code) {
      name
      code
      palette {
        tokens {
          name
          value
        }
        warnings {
          foreground
          background
          ratio
          minimum
        }
      }
    }
  }
`)

const LABELS: Record<TokenName, string> = {
  canvas: 'Page',
  raised: 'Cards',
  panel: 'Fields',
  float: 'Menus',
  ink: 'Text',
  'ink-2': 'Secondary text',
  'ink-3': 'Faint text',
  line: 'Lines',
  'line-strong': 'Strong lines',
  hover: 'Hovered',
  press: 'Pressed',
  accent: 'Primary',
  'accent-hover': 'Primary, hovered',
  'on-accent': 'On primary',
  danger: 'Danger',
  ok: 'Done',
  info: 'Live and downloading',
  'info-deep': 'Live, tinted',
  warn: 'Waiting and warnings',
  'warn-soft': 'Warnings, soft',
  'warn-deep': 'Warnings, tinted',
  highlight: 'Clips',
  social: 'People',
  focus: 'Focus ring',
  selection: 'Selected text',
  scrollbar: 'Scrollbars',
  glow: 'Light overlays',
  shade: 'Shadows',
}

const STYLES: { value: ComponentStyle; label: string }[] = [
  { value: 'LAYERED', label: 'Layered' },
  { value: 'FLAT', label: 'Flat' },
  { value: 'GLASS', label: 'Liquid Glass' },
]

const styleName = (s: ComponentStyle) => STYLES.find((o) => o.value === s)?.label ?? s

function useRefresh() {
  const qc = useQueryClient()
  return () => {
    for (const key of ['appearance', 'schemes', 'appearance-settings', 'server-appearance']) void qc.invalidateQueries({ queryKey: [key] })
  }
}

export function Appearance() {
  const api = useApi()
  const me = useMe()
  const { tokens } = useTheme()
  const { data: schemes } = useQuery({ queryKey: ['schemes'], queryFn: async () => (await api.request(SchemesQuery)).colorSchemes })
  const { data: mine } = useQuery({ queryKey: ['appearance-settings'], queryFn: async () => (await api.request(SettingsQuery)).appearanceSettings })
  const { data: server } = useQuery({ queryKey: ['server-appearance'], queryFn: async () => (await api.request(ServerQuery)).serverAppearance })
  const refresh = useRefresh()
  const save = useMutation({ mutationFn: (input: AppearanceSettingsInput) => api.request(SetAppearance, { input }), onSuccess: refresh, onError: toastError })
  const [editing, setEditing] = useState<Scheme | 'new' | null>(null)
  const [importing, setImporting] = useState(false)
  if (!schemes || !mine || !server) return <ListSkeleton rows={4} height={100} />

  const name = (id: string) => schemes.find((s) => s.id === id)?.name ?? 'Grey'
  const describe = (c: SchemeChoiceInput) =>
    c.mode === 'SINGLE' || c.light === c.dark ? name(c.mode === 'SINGLE' ? c.single : c.light) : `${name(c.light)} or ${name(c.dark)}, matching the system`
  const set = (change: Partial<AppearanceSettingsInput>) => save.mutate({ colors: mine.colors ?? null, style: mine.style ?? null, mediaTint: mine.mediaTint, ...change })
  const grey = schemes.find((s) => s.id === 'grey')

  return (
    <>
      <Group title="Colours" description={mine.colors ? undefined : `Following the server: ${describe(server.colors)}.`}>
        <View className="p-4">
          <Segmented
            value={mine.colors ? 'own' : 'server'}
            options={[
              { value: 'server', label: 'Server default' },
              { value: 'own', label: 'Custom' },
            ]}
            onChange={(v) => set({ colors: v === 'own' ? { ...server.colors } : null })}
          />
        </View>
        {mine.colors && <ChoiceEditor value={mine.colors} schemes={schemes} onChange={(colors) => set({ colors })} />}
      </Group>

      <Group title="Style">
        <Labeled label="Surfaces">
          <Select
            title="Surfaces"
            value={mine.style ?? ''}
            options={[{ value: '', label: `Server default (${styleName(server.style)})` }, ...STYLES]}
            onChange={(v) => set({ style: (v || null) as ComponentStyle | null })}
          />
        </Labeled>
        <Divider />
        <ListRow
          label="Tint controls over artwork and video"
          right={<Toggle label="Tint controls over artwork and video" value={mine.mediaTint} onChange={(mediaTint) => set({ mediaTint })} />}
        />
      </Group>

      <Group
        title="Colour schemes"
        aside={
          <View className="flex-row gap-1">
            <IconButton label="Import" size={32} onPress={() => setImporting(true)}>
              <Upload size={16} color={tokens['ink-2']} />
            </IconButton>
            <IconButton label="New" size={32} disabled={!grey} onPress={() => setEditing('new')}>
              <Plus size={18} color={tokens['ink-2']} />
            </IconButton>
          </View>
        }
      >
        {schemes.map((s, i) => (
          <View key={s.id}>
            {i > 0 && <Divider />}
            <SchemeRow scheme={s} admin={!!me?.isAdmin} onEdit={setEditing} />
          </View>
        ))}
      </Group>

      {me?.isAdmin && <ServerDefaults value={server} schemes={schemes.filter((s) => s.builtIn || s.published)} />}

      <Sheet open={!!editing && !!grey} onClose={() => setEditing(null)}>
        {editing && grey && <SchemeEditor key={editing === 'new' ? 'new' : editing.id} scheme={editing === 'new' ? null : editing} base={grey} onClose={() => setEditing(null)} />}
      </Sheet>
      <Sheet open={importing} onClose={() => setImporting(false)}>
        {importing && <ImportForm onImported={(s) => (setImporting(false), setEditing(s))} />}
      </Sheet>
    </>
  )
}

function ChoiceEditor({ value, schemes, onChange }: { value: SchemeChoiceInput; schemes: Scheme[]; onChange: (v: SchemeChoiceInput) => void }) {
  const options = schemes.map((s) => ({ value: s.id, label: s.name }))
  // A choice whose scheme is gone still shows something sensible.
  const known = (id: string) => (schemes.some((s) => s.id === id) ? id : 'grey')
  return (
    <>
      <View className="px-4 pb-1">
        <Segmented<SchemeMode>
          value={value.mode}
          options={[
            { value: 'SINGLE', label: 'One scheme' },
            { value: 'SYSTEM', label: 'Match system' },
          ]}
          onChange={(mode) => onChange({ ...value, mode })}
        />
      </View>
      {value.mode === 'SINGLE' ? (
        <Labeled label="Scheme">
          <Select title="Scheme" value={known(value.single)} options={options} onChange={(single) => onChange({ ...value, single })} />
        </Labeled>
      ) : (
        <>
          <Labeled label="When the system is light">
            <Select title="When the system is light" value={known(value.light)} options={options} onChange={(light) => onChange({ ...value, light })} />
          </Labeled>
          <Labeled label="When the system is dark">
            <Select title="When the system is dark" value={known(value.dark)} options={options} onChange={(dark) => onChange({ ...value, dark })} />
          </Labeled>
        </>
      )}
    </>
  )
}

function ServerDefaults({ value, schemes }: { value: { colors: SchemeChoiceInput; style: ComponentStyle }; schemes: Scheme[] }) {
  const api = useApi()
  const refresh = useRefresh()
  const save = useMutation({
    mutationFn: (input: { colors: SchemeChoiceInput; style: ComponentStyle }) => api.request(SetServerAppearance, { input }),
    onSuccess: refresh,
    onError: toastError,
  })
  return (
    <Group title="Server default" description="What everyone sees who hasn't picked their own.">
      <View className="pt-3">
        <ChoiceEditor value={value.colors} schemes={schemes} onChange={(colors) => save.mutate({ ...value, colors })} />
      </View>
      <Labeled label="Style">
        <Select title="Style" value={value.style} options={STYLES} onChange={(style) => save.mutate({ ...value, style })} />
      </Labeled>
    </Group>
  )
}

const SWATCH: TokenName[] = ['canvas', 'raised', 'ink', 'accent', 'info', 'warn', 'highlight', 'social', 'danger', 'ok']

function Swatches({ tokens }: { tokens: Record<string, string> }) {
  return (
    <Squircle radius={7} edge style={{ width: 96, height: 24, flexDirection: 'row', overflow: 'hidden' }}>
      {SWATCH.map((n) => (
        <View key={n} style={{ flex: 1, backgroundColor: tokens[n] }} />
      ))}
    </Squircle>
  )
}

function SchemeRow({ scheme: s, admin, onEdit }: { scheme: Scheme; admin: boolean; onEdit: (s: Scheme) => void }) {
  const api = useApi()
  const refresh = useRefresh()
  const { tokens } = useTheme()
  const [menu, setMenu] = useState(false)
  const fork = useMutation({
    mutationFn: async () => (await api.request(ForkScheme, { id: s.id })).forkScheme,
    onSuccess: (forked) => (refresh(), onEdit(forked)),
    onError: toastError,
  })
  const remove = useMutation({ mutationFn: () => api.request(DeleteScheme, { id: s.id }), onSuccess: refresh, onError: toastError })
  const publish = useMutation({ mutationFn: () => api.request(PublishScheme, { id: s.id, published: !s.published }), onSuccess: refresh, onError: toastError })
  const warnings = s.palette.warnings.length
  return (
    <>
      <ListRow
        icon={undefined}
        onPress={() => setMenu(true)}
        chevron={false}
        label={
          <View className="flex-row items-center gap-3">
            <Swatches tokens={toRecord(s.palette.tokens)} />
            <View className="min-w-0 flex-1">
              <View className="flex-row flex-wrap items-center gap-1.5">
                <Text className="font-sans text-[15px] text-ink" numberOfLines={1}>
                  {s.name}
                </Text>
                {s.builtIn && <Badge>Built in</Badge>}
                {s.published && <Badge tone="live">Published</Badge>}
                {warnings > 0 && <CircleAlert size={14} color={tokens.warn} />}
              </View>
              {s.forkedFrom && <Text className="font-sans text-xs text-ink-3">Forked from {s.forkedFrom.name}</Text>}
            </View>
          </View>
        }
      />
      <Menu
        open={menu}
        onClose={() => setMenu(false)}
        header={<Text className="font-sans px-1 text-[15px] font-semibold text-ink">{s.name}</Text>}
        items={[
          s.editable && { label: 'Edit', icon: (c) => <Pencil size={18} color={c} />, onPress: () => onEdit(s) },
          { label: 'Fork', icon: (c) => <GitFork size={18} color={c} />, onPress: () => fork.mutate() },
          {
            label: 'Copy code',
            icon: (c) => <Copy size={18} color={c} />,
            onPress: () => void Clipboard.setStringAsync(s.shareCode).then(() => toast({ title: 'Code copied', tone: 'ok' })),
          },
          admin && !s.builtIn && s.editable && { label: s.published ? 'Unpublish' : 'Publish', icon: (c) => <Send size={18} color={c} />, onPress: () => publish.mutate() },
          s.editable && {
            label: 'Delete',
            danger: true,
            icon: (c) => <Trash2 size={18} color={c} />,
            onPress: async () =>
              (await ask({
                title: `Delete ${s.name}?`,
                body: s.published ? 'Everyone using it goes back to the server default.' : 'If you use it, you go back to the server default.',
                confirm: 'Delete',
                danger: true,
              })) && remove.mutate(),
          },
        ]}
      />
    </>
  )
}

function Warnings({ warnings }: { warnings: Warning[] }) {
  const { tokens } = useTheme()
  if (!warnings.length) return null
  return (
    <Squircle radius={12} className="gap-1 bg-warn-deep/10 p-3">
      <View className="flex-row items-center gap-2">
        <CircleAlert size={15} color={tokens.warn} />
        <Text className="font-sans text-[13px] font-medium text-warn">Some colours are hard to read together</Text>
      </View>
      {warnings.map((w) => (
        <Text key={`${w.foreground}/${w.background}`} className="font-sans pl-6 text-xs text-ink-2">
          {LABELS[w.foreground as TokenName] ?? w.foreground} on {(LABELS[w.background as TokenName] ?? w.background).toLowerCase()}: {w.ratio}:1, needs {w.minimum}:1
        </Text>
      ))}
    </Squircle>
  )
}

/** A bit of the app in the scheme being edited, whatever the app itself uses. */
function Preview({ tokens: t }: { tokens: Tokens }) {
  const chip = (text: string, color: string) => (
    <View style={{ backgroundColor: color + '26', borderRadius: 6, paddingHorizontal: 6, paddingVertical: 2 }}>
      <Text className="font-sans text-2xs font-medium" style={{ color }}>
        {text}
      </Text>
    </View>
  )
  return (
    <Squircle radius={14} edge style={{ backgroundColor: t.canvas, padding: 14, gap: 12 }}>
      <Squircle radius={12} style={{ backgroundColor: t.raised, padding: 12, gap: 2 }}>
        <Text className="font-sans text-[15px] font-semibold" style={{ color: t.ink }}>
          Frieren
        </Text>
        <Text className="font-sans text-sm" style={{ color: t['ink-2'] }}>
          Season 2 · Episode 4
        </Text>
        <Text className="font-sans text-xs" style={{ color: t['ink-3'] }}>
          Airs in 3 days
        </Text>
        <View className="mt-2 flex-row flex-wrap gap-1.5">
          {chip('Downloading', t.info)}
          {chip('Wanted', t.warn)}
          {chip('Done', t.ok)}
          {chip('Failed', t.danger)}
        </View>
        <View style={{ height: 4, borderRadius: 2, backgroundColor: t.press, marginTop: 10, overflow: 'hidden' }}>
          <View style={{ width: '60%', height: '100%', backgroundColor: t.info }} />
        </View>
      </Squircle>
      <View className="flex-row items-center gap-2">
        <View style={{ backgroundColor: t.accent, borderRadius: 10, paddingHorizontal: 14, paddingVertical: 8 }}>
          <Text className="font-sans text-sm font-medium" style={{ color: t['on-accent'] }}>
            Play
          </Text>
        </View>
        <View style={{ backgroundColor: t.panel, borderRadius: 10, paddingHorizontal: 14, paddingVertical: 8 }}>
          <Text className="font-sans text-sm" style={{ color: t.ink }}>
            Later
          </Text>
        </View>
        <Text className="font-sans text-sm" style={{ color: t.danger }}>
          Remove
        </Text>
      </View>
    </Squircle>
  )
}

/** A colour: its swatch, and what it is as text, which is how it's changed. */
function ColorInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [text, setText] = useState(value)
  useEffect(() => setText(value), [value])
  const valid = !!parseColor(text)
  return (
    <View className="flex-row items-center gap-2">
      <View style={{ width: 28, height: 28, borderRadius: 7, backgroundColor: valid ? text : value }} />
      <View style={{ width: 150 }}>
        <Input
          value={text}
          autoCapitalize="none"
          autoCorrect={false}
          style={{ fontFamily: 'monospace', fontSize: 12, height: 38 }}
          onChangeText={(v) => {
            setText(v)
            if (parseColor(v)) onChange(v.trim())
          }}
        />
      </View>
    </View>
  )
}

function SchemeEditor({ scheme, base, onClose }: { scheme: Scheme | null; base: Scheme; onClose: () => void }) {
  const api = useApi()
  const refresh = useRefresh()
  const { tokens: theme } = useTheme()
  const from = scheme ?? base
  const [name, setName] = useState(scheme?.name ?? 'My scheme')
  const [seeds, setSeeds] = useState(() => toRecord(from.palette.seeds) as Record<SeedName, string>)
  const [overrides, setOverrides] = useState(() => (scheme ? toRecord(scheme.palette.overrides) : {}) as Partial<Record<TokenName, string>>)
  const tokens = derive(seeds, overrides)
  const save = useMutation({
    mutationFn: () =>
      api.request(SaveScheme, {
        id: scheme?.id ?? null,
        input: {
          name,
          seeds: SEEDS.map((n) => ({ name: n, value: seeds[n] })),
          overrides: TOKENS.filter((n) => overrides[n]).map((n) => ({ name: n, value: overrides[n]! })),
        },
      }),
    onSuccess: () => {
      refresh()
      toast({ title: scheme ? 'Saved' : `Created ${name.trim()}`, tone: 'ok' })
      onClose()
    },
    onError: toastError,
  })
  const pin = (n: TokenName, v: string | null) =>
    setOverrides((o) => {
      const next = { ...o }
      if (v === null) delete next[n]
      else next[n] = v
      return next
    })
  return (
    <ScrollView style={{ maxHeight: 660 }} keyboardShouldPersistTaps="handled">
      <View className="gap-4 pb-2">
        <Text className="font-sans text-lg font-semibold text-ink">{scheme ? `Edit ${scheme.name}` : 'New colour scheme'}</Text>
        {tokens && <Preview tokens={tokens} />}
        {tokens && <Warnings warnings={contrastWarnings(tokens)} />}
        <Field label="Name">
          <Input value={name} maxLength={48} onChangeText={setName} />
        </Field>
        <Text className="font-sans text-[13px] text-ink-2">Base colours</Text>
        {SEEDS.map((n) => (
          <View key={n} className="flex-row items-center gap-3">
            <Text className="font-sans flex-1 text-sm text-ink" numberOfLines={1}>
              {LABELS[n]}
            </Text>
            <ColorInput value={seeds[n]} onChange={(v) => parseColor(v)?.a === 1000 && setSeeds((s) => ({ ...s, [n]: v }))} />
          </View>
        ))}
        <Text className="font-sans mt-2 text-[13px] text-ink-2">All colours</Text>
        {TOKENS.map((n) => (
          <View key={n} className="flex-row items-center gap-2" style={{ minHeight: 40 }}>
            <View className="min-w-0 flex-1">
              <Text className="font-sans text-sm text-ink" numberOfLines={1}>
                {LABELS[n]}
              </Text>
              <Text className="font-sans text-2xs text-ink-3">{n}</Text>
            </View>
            {overrides[n] !== undefined ? (
              <ColorInput value={overrides[n]} onChange={(v) => pin(n, v)} />
            ) : (
              <View className="flex-row items-center gap-2">
                <View style={{ width: 28, height: 28, borderRadius: 7, backgroundColor: tokens?.[n] }} />
                <Text className="text-xs text-ink-3" style={{ fontFamily: 'monospace', width: 150 }} numberOfLines={1}>
                  {tokens?.[n]}
                </Text>
              </View>
            )}
            <Pressable
              accessibilityLabel={overrides[n] !== undefined ? 'Unpin' : 'Pin'}
              hitSlop={8}
              onPress={() => {
                haptic('tick')
                pin(n, overrides[n] !== undefined ? null : (tokens?.[n] ?? '#000000'))
              }}
            >
              {overrides[n] !== undefined ? <PinOff size={17} color={theme.ink} /> : <Pin size={17} color={theme['ink-3']} />}
            </Pressable>
          </View>
        ))}
        <Button variant="primary" size="lg" disabled={!name.trim() || !tokens || save.isPending} onPress={() => save.mutate()}>
          {scheme ? 'Save' : 'Create'}
        </Button>
      </View>
    </ScrollView>
  )
}

function ImportForm({ onImported }: { onImported: (s: Scheme) => void }) {
  const api = useApi()
  const refresh = useRefresh()
  const [code, setCode] = useState('')
  const [name, setName] = useState<string | null>(null)
  const trimmed = code.trim()
  const { data, error, isFetching } = useQuery({
    queryKey: ['decode', trimmed],
    queryFn: async () => (await api.request(DecodeScheme, { code: trimmed })).decodeScheme,
    enabled: trimmed.length > 4,
    retry: false,
  })
  const save = useMutation({
    mutationFn: async () => (await api.request(ImportScheme, { code: trimmed, name: name ?? data?.name ?? null })).importScheme,
    onSuccess: (s) => {
      refresh()
      toast({ title: `Imported ${s.name}`, tone: 'ok' })
      onImported(s)
    },
    onError: toastError,
  })
  const shown = data && !error ? data : null
  return (
    <View className="gap-4">
      <Text className="font-sans text-lg font-semibold text-ink">Import a colour scheme</Text>
      <Field label="Code" hint="Starts with ts1.">
        <Input value={code} autoCapitalize="none" autoCorrect={false} style={{ fontFamily: 'monospace', fontSize: 12 }} onChangeText={setCode} />
      </Field>
      {error && trimmed.length > 4 && <ErrorText>{(error as Error).message}</ErrorText>}
      {shown && (
        <>
          <Field label="Name">
            <Input value={name ?? shown.name ?? 'Imported scheme'} maxLength={48} onChangeText={setName} />
          </Field>
          <Swatches tokens={toRecord(shown.palette.tokens)} />
          <Warnings warnings={shown.palette.warnings} />
        </>
      )}
      {isFetching && <Progress value={0.5} tone="quiet" />}
      <Button variant="primary" size="lg" disabled={!shown || isFetching || save.isPending} onPress={() => save.mutate()}>
        Import
      </Button>
    </View>
  )
}
