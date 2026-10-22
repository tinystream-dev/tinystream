// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { CircleAlert, Copy, GitFork, Pencil, Pin, PinOff, Plus, Scissors, Trash2, Upload, Users } from 'lucide-react'
import { type CSSProperties, useEffect, useRef, useState } from 'react'
import {
  SEEDS,
  type SeedName,
  TOKENS,
  type TokenName,
  type Tokens,
  type Warning,
  contrastWarnings,
  derive,
  parseColor,
  toRecord,
} from '@tinystream/shared/theme'
import { graphql } from '../gql'
import type { AppearanceSettingsInput, ComponentStyle, SchemeChoiceInput, SchemeMode } from '../gql/graphql'
import { request } from '../lib/api'
import type { Scheme } from '../lib/appearance'
import { useMe } from '../lib/hooks'
import { ask, toast, toastError } from './feedback'
import { Card, Row } from './SettingsKit'
import { Squircle } from './Squircle'
import { Badge, Button, Dialog, Field, IconButton, Input, Progress, Segmented, Select, Tip, Toggle } from './ui'

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

function useSchemes() {
  return useQuery({ queryKey: ['schemes'], queryFn: async () => (await request(SchemesQuery)).colorSchemes })
}

function useRefresh() {
  const qc = useQueryClient()
  return () => {
    for (const key of ['appearance', 'schemes', 'appearance-settings', 'server-appearance'])
      void qc.invalidateQueries({ queryKey: [key] })
  }
}

/** Settings → Appearance: colours and style for you, and (for admins) for everyone. */
export function AppearanceSettings() {
  const me = useMe()
  const { data: schemes } = useSchemes()
  const { data: mine } = useQuery({
    queryKey: ['appearance-settings'],
    queryFn: async () => (await request(SettingsQuery)).appearanceSettings,
  })
  const { data: server } = useQuery({
    queryKey: ['server-appearance'],
    queryFn: async () => (await request(ServerQuery)).serverAppearance,
  })
  const refresh = useRefresh()
  const save = useMutation({
    mutationFn: (input: AppearanceSettingsInput) => request(SetAppearance, { input }),
    onSuccess: refresh,
    onError: toastError,
  })
  const [editing, setEditing] = useState<Scheme | 'new' | null>(null)
  const [importing, setImporting] = useState(false)
  const [revealed, setRevealed] = useState<{ id: string; at: number } | null>(null)
  if (!schemes || !mine || !server) return null

  const name = (id: string) => schemes.find((s) => s.id === id)?.name ?? 'Grey'
  const describe = (c: SchemeChoiceInput) =>
    c.mode === 'SINGLE' || c.light === c.dark ? name(c.mode === 'SINGLE' ? c.single : c.light) : `${name(c.light)} or ${name(c.dark)}, matching the system`
  const set = (change: Partial<AppearanceSettingsInput>) =>
    save.mutate({ colors: mine.colors ?? null, style: mine.style ?? null, mediaTint: mine.mediaTint, ...change })
  const grey = schemes.find((s) => s.id === 'grey')

  return (
    <>
      <Card title="Colours" description={mine.colors ? undefined : `Following the server: ${describe(server.colors)}.`}>
        <div className="space-y-4">
          <Row label="Colours">
            <Segmented
              value={mine.colors ? 'own' : 'server'}
              options={[
                { value: 'server', label: 'Server default' },
                { value: 'own', label: 'Custom' },
              ]}
              onChange={(v) => set({ colors: v === 'own' ? { ...server.colors } : null })}
            />
          </Row>
          {mine.colors && <ChoiceEditor value={mine.colors} schemes={schemes} onChange={(colors) => set({ colors })} />}
        </div>
      </Card>

      <Card title="Style">
        <div className="space-y-4">
          <Field label="Surfaces">
            <Select
              value={mine.style ?? ''}
              options={[{ value: '', label: `Server default (${styleName(server.style)})` }, ...STYLES]}
              onChange={(v) => set({ style: (v || null) as ComponentStyle | null })}
            />
          </Field>
          <Row label="Tint controls over artwork and video">
            <Toggle label="Tint controls over artwork and video" checked={mine.mediaTint} onChange={(mediaTint) => set({ mediaTint })} />
          </Row>
        </div>
      </Card>

      <Card
        title="Colour schemes"
        aside={
          <div className="flex gap-2">
            <Button onClick={() => setImporting(true)}>
              <Upload className="size-3.5" /> Import
            </Button>
            <Button variant="primary" disabled={!grey} onClick={() => setEditing('new')}>
              <Plus className="size-4" /> New
            </Button>
          </div>
        }
      >
        <div className="-mx-2 space-y-0.5">
          {schemes.map((s) => (
            <SchemeRow
              key={s.id}
              scheme={s}
              admin={!!me?.isAdmin}
              flash={revealed?.id === s.id ? revealed.at : undefined}
              onEdit={setEditing}
              onReveal={(id) => setRevealed({ id, at: Date.now() })}
            />
          ))}
        </div>
      </Card>

      {me?.isAdmin && <ServerDefaults value={server} schemes={schemes.filter((s) => s.builtIn || s.published)} />}

      {editing && grey && (
        <SchemeEditor
          scheme={editing === 'new' ? null : editing}
          base={grey}
          onClose={() => setEditing(null)}
          onReveal={(id) => {
            setEditing(null)
            setRevealed({ id, at: Date.now() })
          }}
        />
      )}
      {importing && <ImportDialog onClose={() => setImporting(false)} onImported={(s) => (setImporting(false), setEditing(s))} />}
    </>
  )
}

function ChoiceEditor({ value, schemes, onChange }: { value: SchemeChoiceInput; schemes: Scheme[]; onChange: (v: SchemeChoiceInput) => void }) {
  const options = schemes.map((s) => ({ value: s.id, label: s.name }))
  // A choice whose scheme is gone still shows something sensible.
  const known = (id: string) => (schemes.some((s) => s.id === id) ? id : 'grey')
  return (
    <>
      <Row label="Mode">
        <Segmented<SchemeMode>
          value={value.mode}
          options={[
            { value: 'SINGLE', label: 'One scheme' },
            { value: 'SYSTEM', label: 'Match system' },
          ]}
          onChange={(mode) => onChange({ ...value, mode })}
        />
      </Row>
      {value.mode === 'SINGLE' ? (
        <Field label="Scheme">
          <Select value={known(value.single)} options={options} onChange={(single) => onChange({ ...value, single })} />
        </Field>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="When the system is light">
            <Select value={known(value.light)} options={options} onChange={(light) => onChange({ ...value, light })} />
          </Field>
          <Field label="When the system is dark">
            <Select value={known(value.dark)} options={options} onChange={(dark) => onChange({ ...value, dark })} />
          </Field>
        </div>
      )}
    </>
  )
}

function ServerDefaults({ value, schemes }: { value: { colors: SchemeChoiceInput; style: ComponentStyle }; schemes: Scheme[] }) {
  const refresh = useRefresh()
  const save = useMutation({
    mutationFn: (input: { colors: SchemeChoiceInput; style: ComponentStyle }) => request(SetServerAppearance, { input }),
    onSuccess: refresh,
    onError: toastError,
  })
  return (
    <Card title="Server default">
      <div className="space-y-4">
        <ChoiceEditor value={value.colors} schemes={schemes} onChange={(colors) => save.mutate({ ...value, colors })} />
        <Field label="Style">
          <Select value={value.style} options={STYLES} onChange={(style) => save.mutate({ ...value, style })} />
        </Field>
      </div>
    </Card>
  )
}

const SWATCH: TokenName[] = ['canvas', 'raised', 'ink', 'accent', 'info', 'warn', 'highlight', 'social', 'danger', 'ok']

function Swatches({ tokens }: { tokens: Record<string, string> }) {
  return (
    <Squircle radius={7} edge className="flex h-6 w-32 shrink-0 overflow-hidden">
      {SWATCH.map((n) => (
        <span key={n} className="flex-1" style={{ background: tokens[n] }} />
      ))}
    </Squircle>
  )
}

function SchemeRow({
  scheme: s,
  admin,
  flash,
  onEdit,
  onReveal,
}: {
  scheme: Scheme
  admin: boolean
  flash?: number
  onEdit: (s: Scheme) => void
  onReveal: (id: string) => void
}) {
  const refresh = useRefresh()
  const ref = useRef<HTMLDivElement>(null)
  const fork = useMutation({
    mutationFn: async () => (await request(ForkScheme, { id: s.id })).forkScheme,
    onSuccess: (forked) => {
      refresh()
      onEdit(forked)
    },
    onError: toastError,
  })
  const remove = useMutation({ mutationFn: () => request(DeleteScheme, { id: s.id }), onSuccess: refresh, onError: toastError })
  const publish = useMutation({
    mutationFn: () => request(PublishScheme, { id: s.id, published: !s.published }),
    onSuccess: refresh,
    onError: toastError,
  })
  useEffect(() => {
    if (flash) ref.current?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [flash])
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(s.shareCode)
      toast({ title: 'Code copied', tone: 'ok' })
    } catch (e) {
      toastError(e)
    }
  }
  const warnings = s.palette.warnings.length
  return (
    <div
      ref={ref}
      key={flash}
      className={`flex items-center gap-3 rounded-xl px-2 py-2 hover:bg-hover ${flash ? 'animate-[arrive_1600ms_ease-out]' : ''}`}
    >
      <Swatches tokens={toRecord(s.palette.tokens)} />
      <div className="min-w-0 flex-1">
        <p className="flex items-center gap-2 text-sm">
          <span className="truncate">{s.name}</span>
          {s.builtIn && <Badge>Built in</Badge>}
          {s.published && <Badge tone="live">Published</Badge>}
          {warnings > 0 && (
            <Tip label={`${warnings} hard-to-read pair${warnings === 1 ? '' : 's'}`}>
              <CircleAlert className="size-3.5 text-warn" />
            </Tip>
          )}
        </p>
        {s.forkedFrom && <ForkedFrom origin={s.forkedFrom} onReveal={onReveal} />}
      </div>
      {admin && !s.builtIn && s.editable && (
        <Button size="sm" variant="plain" onClick={() => publish.mutate()} disabled={publish.isPending}>
          {s.published ? 'Unpublish' : 'Publish'}
        </Button>
      )}
      <IconButton label="Copy code" onClick={copy}>
        <Copy className="size-4" />
      </IconButton>
      <IconButton label="Fork" onClick={() => fork.mutate()} disabled={fork.isPending}>
        <GitFork className="size-4" />
      </IconButton>
      {s.editable && (
        <>
          <IconButton label="Edit" onClick={() => onEdit(s)}>
            <Pencil className="size-4" />
          </IconButton>
          <IconButton
            label="Delete"
            onClick={async () =>
              (await ask({
                title: `Delete ${s.name}?`,
                body: s.published ? 'Everyone using it goes back to the server default.' : 'If you use it, you go back to the server default.',
                confirm: 'Delete',
                danger: true,
              })) && remove.mutate()
            }
          >
            <Trash2 className="size-4" />
          </IconButton>
        </>
      )}
    </div>
  )
}

function ForkedFrom({ origin, onReveal }: { origin: { id?: string | null; name: string }; onReveal: (id: string) => void }) {
  const id = origin.id
  return (
    <p className="text-xs text-ink-3">
      Forked from{' '}
      {id ? (
        <button onClick={() => onReveal(id)} className="text-ink-2 underline decoration-line-strong underline-offset-2 hover:text-ink">
          {origin.name}
        </button>
      ) : (
        origin.name
      )}
    </p>
  )
}

function Warnings({ warnings }: { warnings: Warning[] }) {
  if (!warnings.length) return null
  return (
    <Squircle radius={12} className="bg-warn-deep/10 p-3">
      <p className="flex items-center gap-2 text-[13px] font-medium text-warn">
        <CircleAlert className="size-4" /> Some colours are hard to read together
      </p>
      <ul className="mt-1.5 space-y-0.5 pl-6 text-xs text-ink-2">
        {warnings.map((w) => (
          <li key={`${w.foreground}/${w.background}`}>
            {LABELS[w.foreground as TokenName] ?? w.foreground} on {(LABELS[w.background as TokenName] ?? w.background).toLowerCase()}:{' '}
            <span className="tabular">
              {w.ratio}:1, needs {w.minimum}:1
            </span>
          </li>
        ))}
      </ul>
    </Squircle>
  )
}

/** A bit of the app in the scheme being edited, whatever the page itself uses. */
function Preview({ tokens }: { tokens: Tokens }) {
  const vars = Object.fromEntries(TOKENS.map((n) => [`--color-${n}`, tokens[n]])) as CSSProperties
  return (
    <Squircle radius={14} edge className="bg-canvas p-4 text-ink" style={vars}>
      <Squircle radius={12} edge className="bg-raised p-3.5">
        <p className="text-[15px] font-semibold tracking-tight">Frieren</p>
        <p className="mt-0.5 text-sm text-ink-2">Season 2 · Episode 4</p>
        <p className="mt-0.5 text-xs text-ink-3">Airs in 3 days</p>
        <div className="mt-3 flex flex-wrap items-center gap-1.5">
          <Badge tone="live">Downloading</Badge>
          <Badge tone="warn">Wanted</Badge>
          <Badge tone="ok">Done</Badge>
          <Badge tone="danger">Failed</Badge>
          <Scissors className="ml-1 size-3.5 text-highlight" />
          <Users className="size-3.5 text-social" />
        </div>
        <Progress value={0.6} tone="live" className="mt-3" />
      </Squircle>
      <div className="mt-3 flex items-center gap-2">
        <Button variant="primary">Play</Button>
        <Button>Later</Button>
        <Button variant="danger">Remove</Button>
        <div className="flex-1" />
        <Toggle label="Example" checked onChange={() => {}} />
      </div>
    </Squircle>
  )
}

function ColorInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [text, setText] = useState(value)
  useEffect(() => setText(value), [value])
  const parsed = parseColor(text)
  const hex = parsed ? '#' + [parsed.r, parsed.g, parsed.b].map((v) => v.toString(16).padStart(2, '0')).join('') : '#000000'
  return (
    <div className="flex items-center gap-2">
      <label className="relative size-7 shrink-0 cursor-pointer overflow-hidden rounded-md inset-ring inset-ring-line-strong" style={{ background: value }}>
        <input
          type="color"
          value={hex}
          onChange={(e) => {
            // Keeps a see-through colour's alpha.
            const a = parsed && parsed.a !== 1000 ? parsed.a : 1000
            const v = e.target.value
            const next =
              a === 1000
                ? v
                : `rgb(${parseInt(v.slice(1, 3), 16)} ${parseInt(v.slice(3, 5), 16)} ${parseInt(v.slice(5, 7), 16)} / ${a / 1000})`
            setText(next)
            onChange(next)
          }}
          className="absolute inset-0 opacity-0"
        />
      </label>
      <div className="w-48">
        <Input
          value={text}
          spellCheck={false}
          className={`font-mono text-xs ${parsed ? '' : 'text-danger'}`}
          onChange={(e) => {
            setText(e.target.value)
            if (parseColor(e.target.value)) onChange(e.target.value.trim())
          }}
        />
      </div>
    </div>
  )
}

function SchemeEditor({ scheme, base, onClose, onReveal }: { scheme: Scheme | null; base: Scheme; onClose: () => void; onReveal: (id: string) => void }) {
  const from = scheme ?? base
  const [name, setName] = useState(scheme?.name ?? 'My scheme')
  const [seeds, setSeeds] = useState(() => toRecord(from.palette.seeds) as Record<SeedName, string>)
  const [overrides, setOverrides] = useState(() => (scheme ? toRecord(scheme.palette.overrides) : {}) as Partial<Record<TokenName, string>>)
  const tokens = derive(seeds, overrides)
  const refresh = useRefresh()
  const save = useMutation({
    mutationFn: () =>
      request(SaveScheme, {
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
    <Dialog onClose={onClose} width="max-w-3xl">
      <p className="text-[15px] font-medium">{scheme ? `Edit ${scheme.name}` : 'New colour scheme'}</p>
      {scheme?.forkedFrom && <ForkedFrom origin={scheme.forkedFrom} onReveal={onReveal} />}
      <div className="mt-5 grid gap-6 md:grid-cols-[1fr_18rem]">
        <div className="min-w-0 space-y-5">
          <Field label="Name">
            <Input value={name} maxLength={48} onChange={(e) => setName(e.target.value)} />
          </Field>
          <div>
            <p className="mb-2 text-[13px] text-ink-2">Base colours</p>
            <div className="space-y-2">
              {SEEDS.map((n) => (
                <div key={n} className="flex items-center gap-3">
                  <span className="min-w-0 flex-1 truncate text-sm">{LABELS[n]}</span>
                  <ColorInput value={seeds[n]} onChange={(v) => parseColor(v)?.a === 1000 && setSeeds((s) => ({ ...s, [n]: v }))} />
                </div>
              ))}
            </div>
          </div>
          <div>
            <p className="mb-2 text-[13px] text-ink-2">All colours</p>
            <div className="space-y-1">
              {TOKENS.map((n) => (
                <div key={n} className="flex min-h-9 items-center gap-3">
                  <span className="min-w-0 flex-1 truncate text-sm">
                    {LABELS[n]} <span className="text-xs text-ink-3">{n}</span>
                  </span>
                  {overrides[n] !== undefined ? (
                    <ColorInput value={overrides[n]} onChange={(v) => pin(n, v)} />
                  ) : (
                    <span className="flex items-center gap-2">
                      <span className="size-7 rounded-md inset-ring inset-ring-line-strong" style={{ background: tokens?.[n] }} />
                      <span className="w-48 px-3 font-mono text-xs text-ink-3">{tokens?.[n]}</span>
                    </span>
                  )}
                  <IconButton label={overrides[n] !== undefined ? 'Unpin' : 'Pin'} onClick={() => pin(n, overrides[n] !== undefined ? null : (tokens?.[n] ?? '#000000'))}>
                    {overrides[n] !== undefined ? <PinOff className="size-4" /> : <Pin className="size-4" />}
                  </IconButton>
                </div>
              ))}
            </div>
          </div>
        </div>
        <div className="space-y-4 md:sticky md:top-0 md:self-start">
          {tokens && <Preview tokens={tokens} />}
          {tokens && <Warnings warnings={contrastWarnings(tokens)} />}
        </div>
      </div>
      <div className="mt-6 flex items-center gap-2">
        <div className="flex-1" />
        <Button variant="plain" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" disabled={!name.trim() || !tokens || save.isPending} onClick={() => save.mutate()}>
          {scheme ? 'Save' : 'Create'}
        </Button>
      </div>
    </Dialog>
  )
}

function ImportDialog({ onClose, onImported }: { onClose: () => void; onImported: (s: Scheme) => void }) {
  const [code, setCode] = useState('')
  const [name, setName] = useState<string | null>(null)
  const trimmed = code.trim()
  const { data, error, isFetching } = useQuery({
    queryKey: ['decode', trimmed],
    queryFn: async () => (await request(DecodeScheme, { code: trimmed })).decodeScheme,
    enabled: trimmed.length > 4,
    retry: false,
  })
  const refresh = useRefresh()
  const save = useMutation({
    mutationFn: async () => (await request(ImportScheme, { code: trimmed, name: name ?? data?.name ?? null })).importScheme,
    onSuccess: (s) => {
      refresh()
      toast({ title: `Imported ${s.name}`, tone: 'ok' })
      onImported(s)
    },
    onError: toastError,
  })
  const shown = data && !error ? data : null
  return (
    <Dialog onClose={onClose}>
      <p className="text-[15px] font-medium">Import a colour scheme</p>
      <div className="mt-5 space-y-4">
        <Field label="Code" hint="Starts with ts1.">
          <Input value={code} autoFocus spellCheck={false} className="font-mono text-xs" onChange={(e) => setCode(e.target.value)} />
        </Field>
        {error && trimmed.length > 4 && <p className="text-sm text-danger">{(error as Error).message}</p>}
        {shown && (
          <>
            <Field label="Name">
              <Input value={name ?? shown.name ?? 'Imported scheme'} maxLength={48} onChange={(e) => setName(e.target.value)} />
            </Field>
            <Swatches tokens={toRecord(shown.palette.tokens)} />
            <Warnings warnings={shown.palette.warnings} />
          </>
        )}
      </div>
      <div className="mt-6 flex items-center gap-2">
        <div className="flex-1" />
        <Button variant="plain" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" disabled={!shown || isFetching || save.isPending} onClick={() => save.mutate()}>
          Import
        </Button>
      </div>
    </Dialog>
  )
}
