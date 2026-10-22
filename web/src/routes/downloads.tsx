// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import {
  ArrowDown,
  ArrowUp,
  CircleAlert,
  CircleCheck,
  ChevronDown,
  EllipsisVertical,
  FolderInput,
  Pause,
  Play,
  RefreshCw,
  ShieldAlert,
  Sprout,
  Trash2,
  Turtle,
  Users,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { DeleteItems, useDeleteDownloaded } from '../components/downloads'
import { ask, toast, toastError } from '../components/feedback'
import { Empty, Page, PageTitle, Section } from '../components/Page'
import { ListSkeleton, useArrived } from '../components/Skeleton'
import { PieceMap, Sparkline, record } from '../components/Sparkline'
import { Squircle } from '../components/Squircle'
import { Ticker } from '../components/Ticker'
import { Badge, Button, IconButton, MenuItem, Panel, Popover, Select, Tip } from '../components/ui'
import { graphql } from '../gql'
import { type Download, type EngineOverview, request } from '../lib/api'

const DownloadsQuery = graphql(`
  query Downloads {
    downloads {
      ...DownloadFields
    }
  }
`)

const EngineQuery = graphql(`
  query Engine {
    downloadEngine {
      ...EngineFields
    }
  }
`)

const PauseDoc = graphql(`
  mutation DownloadsPause($ids: [Int!]!) {
    pauseDownloads(ids: $ids) {
      id
    }
  }
`)

const ResumeDoc = graphql(`
  mutation DownloadsResume($ids: [Int!]!) {
    resumeDownloads(ids: $ids) {
      id
    }
  }
`)

const RecheckDoc = graphql(`
  mutation DownloadsRecheck($ids: [Int!]!) {
    recheckDownloads(ids: $ids) {
      id
    }
  }
`)

const ImportDoc = graphql(`
  mutation ImportDownload($id: Int!) {
    importDownload(id: $id) {
      id
    }
  }
`)

const RemoveDoc = graphql(`
  mutation RemoveDownloads($ids: [Int!]!, $deleteFiles: Boolean!) {
    removeDownloads(ids: $ids, deleteFiles: $deleteFiles)
  }
`)

type Action = 'pause' | 'resume' | 'recheck' | 'import'

/** Does `action` to downloads. */
function run(action: Action, ids: number[]): Promise<unknown> {
  if (action === 'import') return Promise.all(ids.map((id) => request(ImportDoc, { id })))
  if (action === 'pause') return request(PauseDoc, { ids })
  if (action === 'resume') return request(ResumeDoc, { ids })
  return request(RecheckDoc, { ids })
}
import { bytes, clockTime, duration, episodeCode, relative, speed } from '../lib/downloads'
import { useMe } from '../lib/hooks'
import { mediaLabels, mediaOptions, ofMediaType } from '@tinystream/shared/media'
import { useTitle } from '../lib/title'

export const Route = createFileRoute('/downloads')({ component: DownloadsPage })

const POLL = 1500

type Where = 'active' | 'seeding' | 'finished' | 'gone'

/** Which list each download was in last time, so a move can be celebrated. */
const lastSeen = new Map<number, Where>()

function DownloadsPage() {
  useTitle('Downloads')
  const [category, setCategory] = useState('')
  const qc = useQueryClient()
  const { data, dataUpdatedAt } = useQuery({ queryKey: ['downloads'], queryFn: async () => (await request(DownloadsQuery)).downloads, refetchInterval: POLL })
  const { data: engine, dataUpdatedAt: engineAt, isPending: enginePending } = useQuery({
    queryKey: ['engine'],
    queryFn: async () => (await request(EngineQuery)).downloadEngine,
    refetchInterval: POLL,
  })
  const filtered = ofMediaType(data ?? [], category)
  const active = filtered.filter((d) => d.state === 'DOWNLOADING' || (d.state === 'PAUSED' && !d.finishedAt))
  const seeding = filtered.filter((d) => d.state === 'SEEDING' || (d.state === 'PAUSED' && d.finishedAt))
  const gone = filtered.filter((d) => d.state === 'FAILED' || d.state === 'REMOVED')
  const finished = filtered.filter((d) => d.state === 'DONE')
  const running = data?.filter((d) => d.state === 'DOWNLOADING' || d.state === 'SEEDING') ?? []
  const paused = data?.filter((d) => d.state === 'PAUSED') ?? []

  const bulk = useMutation({
    mutationFn: async (action: 'pause' | 'resume') => {
      const targets = action === 'pause' ? running : paused
      await run(action, targets.map((d) => d.id))
      return targets.length
    },
    onSuccess: (n, action) => toast({ title: `${action === 'pause' ? 'Paused' : 'Resumed'} ${n} download${n === 1 ? '' : 's'}`, tone: 'ok' }),
    onError: toastError,
    onSettled: () => void qc.invalidateQueries({ queryKey: ['downloads'] }),
  })
  const arrived = useArrived(!!data && !enginePending)

  if (!data || enginePending) return <ListSkeleton rows={3} />

  return (
    <Page arrive={arrived}>
      <PageTitle
        aside={
          (running.length > 0 || paused.length > 0) && (
            <div className="flex gap-1.5 pb-0.5">
              {running.length > 0 && (
                <Button size="sm" variant="plain" onClick={() => bulk.mutate('pause')} disabled={bulk.isPending}>
                  <Pause className="size-3.5" /> Pause all
                </Button>
              )}
              {paused.length > 0 && (
                <Button size="sm" variant="plain" onClick={() => bulk.mutate('resume')} disabled={bulk.isPending}>
                  <Play className="size-3.5" /> Resume all
                </Button>
              )}
            </div>
          )
        }
      >
        Downloads
      </PageTitle>

      <div className="mb-6 max-w-56">
        <Select value={category} options={mediaOptions} onChange={setCategory} />
      </div>

      {engine?.killSwitch && (
        <Squircle radius={14} edge className="mb-6 flex items-start gap-3 bg-danger/10 p-4">
          <ShieldAlert className="mt-0.5 size-5 shrink-0 text-danger" />
          <p className="text-sm font-medium text-danger">{engine.killSwitch} is down, so every torrent is paused</p>
        </Squircle>
      )}
      {engine?.listenError && (
        <Squircle radius={14} edge className="mb-6 flex items-start gap-3 bg-warn-deep/10 p-4">
          <CircleAlert className="mt-0.5 size-5 shrink-0 text-warn" />
          <p className="text-sm text-ink-2">{engine.listenError}</p>
        </Squircle>
      )}

      {engine && <Traffic engine={engine} stamp={engineAt} downloads={data} />}

      {filtered.length === 0 && <Empty title={category ? "No downloads of this type" : "No downloads yet"} />}

      {active.length > 0 && (
        <Section title="Downloading" aside={<span className="text-xs text-ink-3 tabular">{active.length}</span>}>
          <List items={active} where="active" stamp={dataUpdatedAt} />
        </Section>
      )}
      {seeding.length > 0 && (
        <Section title="Seeding" aside={<span className="text-xs text-ink-3 tabular">{seeding.length}</span>}>
          <List items={seeding} where="seeding" stamp={dataUpdatedAt} />
        </Section>
      )}
      {finished.length > 0 && (
        <Section title="Finished" aside={<span className="text-xs text-ink-3">last two weeks</span>}>
          <List items={finished} where="finished" stamp={dataUpdatedAt} />
        </Section>
      )}
      {gone.length > 0 && (
        <Section title={<span className="text-danger/90">Removed &amp; failed</span>} aside={<span className="text-xs text-ink-3">last two weeks</span>}>
          <List items={gone} where="gone" stamp={dataUpdatedAt} />
        </Section>
      )}
      {engine && (
        <p className="mt-6 text-xs text-ink-3">
          Downloading to {engine.downloadPath} · libtorrent {engine.version}
        </p>
      )}
    </Page>
  )
}

/** Live totals over a rolling graph of the last minute and a half. */
function Traffic({ engine, stamp, downloads }: { engine: EngineOverview; stamp: number; downloads: Download[] }) {
  const down = record('engine-down', engine.downloadRate, stamp)
  const up = record('engine-up', engine.uploadRate, stamp)
  const max = Math.max(1, ...down, ...up)
  const peers = downloads.reduce((n, d) => n + (d.live && !d.live.paused ? d.live.peers : 0), 0)
  const shared = downloads.reduce((n, d) => n + (d.live?.uploaded ?? 0), 0)
  return (
    <Squircle radius={18} edge className="relative mb-11 overflow-hidden bg-raised">
      <Sparkline values={down} interval={POLL} max={max} className="absolute inset-x-0 bottom-0 h-20 w-full" />
      <Sparkline values={up} interval={POLL} max={max} color="var(--color-ok)" className="absolute inset-x-0 bottom-0 h-20 w-full opacity-80" />
      <div className="relative flex flex-wrap items-end gap-x-10 gap-y-4 p-5 pb-8">
        <Stat label="Download" tone="text-info" icon={<ArrowDown className="size-5" />} value={speed(engine.downloadRate)} />
        <Stat label="Upload" tone="text-ok" icon={<ArrowUp className="size-5" />} value={speed(engine.uploadRate)} />
        <div className="flex-1" />
        <div className="flex gap-6 pb-1 text-xs text-ink-3 tabular">
          {engine.slowHours && (
            <Tip label="Slower limits apply right now" className="flex items-center gap-1.5">
              <Turtle className="size-3.5" /> slow hours
            </Tip>
          )}
          <span className="flex items-center gap-1.5">
            <Users className="size-3.5" /> <Ticker value={String(peers)} /> peers
          </span>
          <span className="flex items-center gap-1.5">
            <Sprout className="size-3.5" /> {bytes(shared)} shared
          </span>
        </div>
      </div>
    </Squircle>
  )
}

function Stat({ label, value, icon, tone }: { label: string; value: string; icon: React.ReactNode; tone: string }) {
  return (
    <div>
      <p className="text-xs text-ink-3">{label}</p>
      <p className={`mt-1 flex items-center gap-1.5 text-[28px] leading-none font-semibold tracking-tight ${tone}`}>
        {icon}
        <Ticker value={value} className="text-ink" />
      </p>
    </div>
  )
}

function List({ items, where, stamp }: { items: Download[]; where: Where; stamp: number }) {
  return (
    <Squircle
      radius={16}
      edge
      className={`divide-y ${where === 'gone' ? 'divide-danger/10 bg-danger/[0.06] [&_img]:grayscale [&_img]:opacity-60' : 'divide-line bg-raised'}`}
    >
      {bySeason(items).map((g) =>
        g.items.length === 1 ? (
          <Row key={g.items[0].id} d={g.items[0]} where={where} stamp={stamp} />
        ) : (
          <Group key={g.key} season={g.season} items={g.items} where={where} stamp={stamp} />
        ),
      )}
    </Squircle>
  )
}

type Bunch = { key: string; season: number; items: Download[] }

/** Downloads of the same season of a series gathered together, each bunch
 *  where its first download was. Season packs and odd ones stand alone. */
function bySeason(items: Download[]): Bunch[] {
  const out: Bunch[] = []
  const open = new Map<string, Bunch>()
  for (const d of items) {
    const seasons = new Set(d.episodes.map((e) => e.season))
    const series = d.seriesId ?? d.seriesName
    if (series == null || seasons.size !== 1) {
      out.push({ key: `d${d.id}`, season: 0, items: [d] })
      continue
    }
    const season = d.episodes[0].season
    const key = `s${series}:${season}:${d.category}`
    const bunch = open.get(key)
    if (bunch) bunch.items.push(d)
    else {
      const b = { key, season, items: [d] }
      open.set(key, b)
      out.push(b)
    }
  }
  for (const b of out) b.items.sort((a, z) => first(a) - first(z))
  return out
}

function first(d: Download) {
  return Math.min(...d.episodes.map((e) => e.episode))
}

/** Which seasons are opened up, so polling doesn't fold them again. */
const opened = new Set<string>()

/** Episodes, like S01E01–E04, or S01 · 5 episodes when there are gaps. */
function episodeRange(season: number, items: Download[]) {
  const eps = [...new Set(items.flatMap((d) => d.episodes.map((e) => e.episode)))].sort((a, z) => a - z)
  if (season === 0) return `${eps.length} specials`
  const code = episodeCode(season, eps[0])
  if (eps[eps.length - 1] - eps[0] === eps.length - 1) return `${code}–E${String(eps[eps.length - 1]).padStart(2, '0')}`
  return `S${String(season).padStart(2, '0')} · ${eps.length} episodes`
}

const urgency = { danger: 0, warn: 1, live: 2, quiet: 3, ok: 4 }

/** A season's downloads folded into one row that opens into them. */
function Group({ season, items, where, stamp }: { season: number; items: Download[]; where: Where; stamp: number }) {
  const qc = useQueryClient()
  const lead = items[0]
  const key = `${where}:${lead.seriesId ?? lead.seriesName}:${season}`
  const [open, setOpen] = useState(() => opened.has(key))
  // Glow when one of them arrives from the list above, like a lone row would.
  const [arrived] = useState(() =>
    items.some((d) => {
      const before = lastSeen.get(d.id)
      return before !== undefined && before !== where
    }),
  )
  if (!open) for (const d of items) lastSeen.set(d.id, where)
  const [broken, setBroken] = useState(false)
  const act = useMutation({
    mutationFn: (action: 'pause' | 'resume') =>
      run(
        action,
        items.filter((d) => (action === 'pause' ? d.state === 'DOWNLOADING' || d.state === 'SEEDING' : d.state === 'PAUSED')).map((d) => d.id),
      ),
    onError: toastError,
    onSettled: () => void qc.invalidateQueries({ queryKey: ['downloads'] }),
  })
  const del = useDeletion(lead, season)

  const toggle = () => {
    if (open) opened.delete(key)
    else opened.add(key)
    setOpen(!open)
  }
  const s = items.map(status).sort((a, z) => urgency[a.tone] - urgency[z.tone])[0]
  const live = where === 'active' || where === 'seeding'
  const running = items.some((d) => d.state === 'DOWNLOADING' || d.state === 'SEEDING')
  const size = items.reduce((n, d) => n + (d.size ?? 0), 0)
  const done = items.reduce((n, d) => n + (d.live?.done ?? (d.finishedAt ? (d.size ?? 0) : 0)), 0)
  const rate = items.reduce((n, d) => n + (d.live?.downloadRate ?? 0), 0)
  const shared = items.reduce((n, d) => n + (d.live?.uploaded ?? 0), 0)
  const progress = size > 0 ? Math.min(1, done / size) : null
  const imported = items.filter((d) => d.importState === 'DONE').length

  return (
    <div className={arrived ? 'animate-[arrive_2.4s_ease-out]' : ''}>
      <div className="relative flex gap-3.5 px-4 py-3.5">
        <Squircle radius={7} className="aspect-[2/3] w-10 shrink-0 self-start bg-panel">
          {lead.poster && !broken && (
            <img src={lead.poster} alt="" loading="lazy" onError={() => setBroken(true)} className="size-full object-cover" />
          )}
        </Squircle>
        <div className="min-w-0 flex-1">
          <div className="flex items-start gap-3">
            <button type="button" aria-expanded={open} onClick={toggle} className="min-w-0 flex-1 text-left">
              <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                <span className="max-w-full truncate text-sm font-medium">{lead.seriesName ?? lead.name}</span>
                <Badge>{mediaLabels[lead.category]}</Badge>
                <Badge>{episodeRange(season, items)}</Badge>
                <Badge tone={s.tone}>{s.text}</Badge>
                {imported > 0 && (
                  <Badge tone="ok">
                    <CircleCheck className="size-3" /> {imported === items.length ? 'In library' : `${imported} of ${items.length} in library`}
                  </Badge>
                )}
              </div>
              <span className="mt-0.5 flex items-center gap-1 text-xs text-ink-3">
                <ChevronDown className={`size-3.5 transition-transform duration-150 ${open ? 'rotate-180' : ''}`} />
                {items.length} downloads
              </span>
            </button>
            {live && (
              <IconButton label={running ? 'Pause all' : 'Resume all'} onClick={() => act.mutate(running ? 'pause' : 'resume')}>
                {running ? <Pause className="size-4" /> : <Play className="size-4" />}
              </IconButton>
            )}
            {del && (
              <Popover
                portal
                trigger={({ toggle }) => (
                  <IconButton label="More" onClick={toggle}>
                    <EllipsisVertical className="size-4" />
                  </IconButton>
                )}
              >
                {(close) => (
                  <Panel className="w-60 p-1.5">
                    <DeleteItems target={del.target} onDelete={del.run} close={close} />
                  </Panel>
                )}
              </Popover>
            )}
          </div>
          {where === 'active' && (
            <div className="mt-3">
              <div className="relative h-1 overflow-hidden rounded-full bg-press">
                <div
                  className="h-full rounded-full bg-info transition-[width] ease-linear"
                  style={{ width: `${(progress ?? 0) * 100}%`, transitionDuration: `${POLL}ms` }}
                />
              </div>
              <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink-3 tabular">
                <span className="text-ink-2">{progress !== null ? <Ticker value={`${(progress * 100).toFixed(1)}%`} /> : '—'}</span>
                <span>
                  {bytes(done)} of {bytes(size)}
                </span>
                {rate > 0 && (
                  <span className="flex items-center gap-1 text-info">
                    <ArrowDown className="size-3" /> <Ticker value={speed(rate)} />
                  </span>
                )}
              </div>
            </div>
          )}
          {where === 'seeding' && (
            <p className="mt-1.5 flex items-center gap-1 text-xs text-ink-3 tabular">
              <Sprout className="size-3" /> {bytes(shared)} shared
            </p>
          )}
        </div>
      </div>
      {open && (
        <div className={`divide-y border-t pl-[54px] ${where === 'gone' ? 'divide-danger/10 border-danger/10' : 'divide-line border-line'}`}>
          {items.map((d) => (
            <Row key={d.id} d={d} where={where} stamp={stamp} nested />
          ))}
        </div>
      )}
    </div>
  )
}

/** Deleting `d`'s season or show, for people who may. */
function useDeletion(d: Download, season: number | null) {
  const run = useDeleteDownloaded()
  const can = useMe()?.permissions
  if (d.seriesId == null || !can?.manageShows || !can.downloads) return null
  return { target: { seriesId: d.seriesId, show: d.seriesName ?? d.name, season }, run }
}

function status(d: Download): { text: string; tone: 'quiet' | 'ok' | 'live' | 'warn' | 'danger' } {
  if (d.state === 'FAILED') return { text: 'Failed', tone: 'danger' }
  if (d.state === 'REMOVED') return { text: 'Removed', tone: 'quiet' }
  if (d.importState === 'FAILED') return { text: 'Not imported', tone: 'danger' }
  const l = d.live
  if (d.state === 'PAUSED') return { text: 'Paused', tone: 'quiet' }
  if (l?.stage === 'METADATA') return { text: 'Finding peers', tone: 'warn' }
  if (l?.stage === 'CHECKING') return { text: 'Checking', tone: 'warn' }
  if (d.state === 'DOWNLOADING') return { text: l && l.downloadRate > 0 ? 'Downloading' : 'Waiting for peers', tone: 'live' }
  if (d.state === 'SEEDING') return { text: 'Seeding', tone: 'ok' }
  return { text: 'Done', tone: 'ok' }
}

function Row({ d, where, stamp, nested }: { d: Download; where: Where; stamp: number; nested?: boolean }) {
  const qc = useQueryClient()
  // Glow once when it arrives here from the list above.
  const [arrived] = useState(() => {
    const before = lastSeen.get(d.id)
    return before !== undefined && before !== where
  })
  lastSeen.set(d.id, where)
  // A light crosses the row once when it finishes, or lands in the library.
  const [glint, setGlint] = useState(arrived && (where === 'seeding' || where === 'finished') ? 1 : 0)
  const imported = useRef(d.importState === 'DONE')
  useEffect(() => {
    if (d.importState === 'DONE' && !imported.current) setGlint((g) => g + 1)
    imported.current = d.importState === 'DONE'
  }, [d.importState])
  const [broken, setBroken] = useState(false)
  const act = useMutation({
    mutationFn: (action: Action) => run(action, [d.id]),
    onError: toastError,
    onSettled: () => void qc.invalidateQueries({ queryKey: ['downloads'] }),
  })
  const remove = useMutation({
    mutationFn: (files: boolean) => request(RemoveDoc, { ids: [d.id], deleteFiles: files }),
    onSuccess: (_, files) => toast({ title: files ? 'Download deleted' : 'Download removed', body: d.seriesName ?? d.name, tone: 'ok' }),
    onError: toastError,
    onSettled: () => void qc.invalidateQueries({ queryKey: ['downloads'] }),
  })
  const seasons = [...new Set(d.episodes.map((e) => e.season))]
  const del = useDeletion(d, seasons.length === 1 ? seasons[0] : null)
  const l = d.live
  const s = status(d)
  const downloading = d.state === 'DOWNLOADING'
  const progress = downloading ? (l && l.stage !== 'METADATA' ? l.progress : null) : null
  // Aim the bar where it'll be at the next poll, and glide there, so it
  // moves continuously instead of jumping every poll.
  const predicted =
    progress !== null && l && d.size ? Math.min(1, (l.done + l.downloadRate * (POLL / 1000)) / d.size) : progress
  const rates = downloading && l ? record(`dl${d.id}`, l.downloadRate, stamp) : []
  const eps = d.episodes
  const label =
    eps.length === 0 ? null : eps.length === 1 ? episodeCode(eps[0].season, eps[0].episode) : `${eps.length} episodes`
  const error = d.error ?? (d.importState === 'FAILED' ? d.importError : null)
  const paused = d.state === 'PAUSED'
  const live = downloading || d.state === 'SEEDING' || paused
  const seeded = (d.state === 'SEEDING' || (paused && d.finishedAt)) && l
  const removable = d.state !== 'REMOVED' && d.state !== 'DONE'
  const importable = !!d.finishedAt && d.importState !== 'DONE' && d.state !== 'REMOVED'

  return (
    <div className={`group relative flex gap-3.5 overflow-hidden px-4 py-3.5 ${arrived ? 'animate-[arrive_2.4s_ease-out]' : ''}`}>
      {glint > 0 && <span key={glint} className="glint" />}
      {!nested && (
        <Squircle radius={7} className="aspect-[2/3] w-10 shrink-0 self-start bg-panel">
          {d.poster && !broken && <img src={d.poster} alt="" loading="lazy" onError={() => setBroken(true)} className="size-full object-cover" />}
        </Squircle>
      )}
      <div className="min-w-0 flex-1">
        <div className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
              {nested ? (
                <span className="max-w-full truncate text-sm font-medium">{label ?? d.name}</span>
              ) : d.seriesName && d.title ? (
                <Link to="/title/$id" params={{ id: String(d.title.id) }} className="max-w-full truncate text-sm font-medium hover:underline">
                  {d.seriesName}
                </Link>
              ) : (
                <span className="max-w-full truncate text-sm font-medium">{d.seriesName ?? d.name}</span>
              )}
              {!nested && <Badge>{mediaLabels[d.category]}</Badge>}
              {label && !nested && <Badge>{label}</Badge>}
              <Badge tone={s.tone}>{s.text}</Badge>
              {d.importState === 'DONE' &&
                (d.title ? (
                  <Link to="/title/$id" params={{ id: String(d.title.id) }} className="rounded-md hover:brightness-125">
                    <Badge tone="ok" title={`Put into the library by ${d.importMode}`}>
                      <CircleCheck className="size-3" /> In library
                    </Badge>
                  </Link>
                ) : (
                  <Badge tone="ok" title={`Put into the library by ${d.importMode}`}>
                    <CircleCheck className="size-3" /> In library
                  </Badge>
                ))}
            </div>
            <Tip label={d.name} className="mt-0.5 block truncate text-xs text-ink-3">
              {d.name}
            </Tip>
          </div>
          {live && (
            <IconButton label={paused ? 'Resume' : 'Pause'} onClick={() => act.mutate(paused ? 'resume' : 'pause')}>
              {paused ? <Play className="size-4" /> : <Pause className="size-4" />}
            </IconButton>
          )}
          <Popover
            portal
            trigger={({ toggle }) => (
              <IconButton label="More" onClick={toggle}>
                <EllipsisVertical className="size-4" />
              </IconButton>
            )}
          >
            {(close) => (
              <Panel className="w-60 p-1.5">
                {live && (
                  <MenuItem onClick={() => (close(), act.mutate('recheck'))}>
                    <span className="flex items-center gap-2">
                      <RefreshCw className="size-3.5 shrink-0" /> Check files again
                    </span>
                  </MenuItem>
                )}
                {importable && (
                  <MenuItem onClick={() => (close(), act.mutate('import'))}>
                    <span className="flex items-center gap-2">
                      <FolderInput className="size-3.5 shrink-0" /> Import into the library
                    </span>
                  </MenuItem>
                )}
                {removable && (
                  <>
                    <MenuItem onClick={() => (close(), remove.mutate(false))}>
                      <span className="flex items-center gap-2">
                        <Trash2 className="size-3.5 shrink-0" /> Remove, keep its files
                      </span>
                    </MenuItem>
                    <MenuItem
                      onClick={async () => {
                        close()
                        const ok = await ask({
                          title: 'Delete this download?',
                          body: 'Its files are deleted from the download folder. Anything already in your library stays.',
                          confirm: 'Delete',
                          danger: true,
                        })
                        if (ok) remove.mutate(true)
                      }}
                    >
                      <span className="flex items-center gap-2 text-danger">
                        <Trash2 className="size-3.5 shrink-0" /> Remove and delete download
                      </span>
                    </MenuItem>
                  </>
                )}
                {!removable && !importable && !del && (
                  <p className="px-2.5 py-1.5 text-xs text-ink-3">Nothing to do; it's finished.</p>
                )}
                {del && (
                  <>
                    {(removable || importable) && <div className="my-1 h-px bg-line" />}
                    <DeleteItems target={del.target} onDelete={del.run} close={close} />
                  </>
                )}
              </Panel>
            )}
          </Popover>
        </div>

        {downloading && (
          <div className="mt-3">
            <div className="relative h-1 overflow-hidden rounded-full bg-press">
              {predicted === null ? (
                <div className="absolute inset-y-0 w-1/3 animate-[slide_1.2s_ease-in-out_infinite] rounded-full bg-info" />
              ) : (
                <div
                  className="h-full rounded-full bg-info transition-[width] ease-linear"
                  style={{ width: `${predicted * 100}%`, transitionDuration: `${POLL}ms` }}
                />
              )}
            </div>
            {l && l.pieces.length > 0 && progress !== null && <PieceMap pieces={l.pieces} className="mt-1" />}
            <div className="mt-1.5 flex items-end gap-4">
              <div className="flex min-w-0 flex-1 flex-wrap gap-x-4 gap-y-1 text-xs text-ink-3 tabular">
                <span className="text-ink-2">{progress !== null ? <Ticker value={`${(progress * 100).toFixed(1)}%`} /> : '—'}</span>
                <span>
                  {bytes(l?.done)} of {bytes(d.size)}
                </span>
                {l && l.downloadRate > 0 && (
                  <span className="flex items-center gap-1 text-info">
                    <ArrowDown className="size-3" /> <Ticker value={speed(l.downloadRate)} />
                  </span>
                )}
                {l?.eta != null && (
                  <span>
                    {duration(l.eta)} left <span className="text-ink-3/70">· done at {clockTime(Date.now() / 1000 + l.eta)}</span>
                  </span>
                )}
                {l && (
                  <span>
                    {l.seeds} seeds, {l.peers} peers
                  </span>
                )}
              </div>
              {rates.length > 0 && <Sparkline values={rates} interval={POLL} fade className="hidden h-7 w-32 shrink-0 sm:block" />}
            </div>
          </div>
        )}
        {seeded && <SeedLine d={d} />}
        {!live && (
          <p className="mt-1 text-xs text-ink-3">
            Added {relative(d.addedAt)}
            {d.importedAt && ` · imported ${relative(d.importedAt)}`}
            {d.requestedBy && ` · by ${d.requestedBy.username}`}
          </p>
        )}
        {error && (
          <p className="mt-2 flex items-start gap-1.5 text-xs text-danger">
            <CircleAlert className="mt-px size-3.5 shrink-0" /> {error}
          </p>
        )}
      </div>
    </div>
  )
}

/** Seeding stats, with how far along it is towards its goal. */
function SeedLine({ d }: { d: Download }) {
  const l = d.live!
  const paused = d.state === 'PAUSED'
  const goal = d.seedGoal
  const toward =
    goal.ratio != null && goal.ratio > 0
      ? { value: l.ratio / goal.ratio, text: `ratio ${l.ratio.toFixed(2)} of ${goal.ratio.toFixed(1)}` }
      : goal.seconds
        ? { value: l.seedingSeconds / goal.seconds, text: `${duration(l.seedingSeconds)} of ${duration(goal.seconds)}` }
        : null
  return (
    <div className="mt-2.5">
      {toward && (
        <div className="mb-1.5 h-1 max-w-md overflow-hidden rounded-full bg-press">
          <div
            className={`h-full rounded-full transition-[width] duration-700 ${paused ? 'bg-ink-3' : 'bg-ok'}`}
            style={{ width: `${Math.max(1, Math.min(1, toward.value) * 100)}%` }}
          />
        </div>
      )}
      <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink-3 tabular">
        <span className="flex items-center gap-1">
          <Sprout className="size-3" /> {toward ? toward.text : `ratio ${l.ratio.toFixed(2)}`}
        </span>
        <span>{bytes(l.uploaded)} shared</span>
        {!paused && l.uploadRate > 0 && (
          <span className="flex items-center gap-1 text-ok">
            <ArrowUp className="size-3" /> <Ticker value={speed(l.uploadRate)} />
          </span>
        )}
        <span>{paused ? `paused · seeded for ${duration(l.seedingSeconds)}` : `seeding for ${duration(l.seedingSeconds)}`}</span>
        {!toward && !paused && <span>no end set</span>}
      </div>
    </div>
  )
}
