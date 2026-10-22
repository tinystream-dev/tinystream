// SPDX-License-Identifier: AGPL-3.0-or-later
// The server's torrents (web/src/routes/downloads.tsx), polled live: the
// traffic overall, then what's downloading, seeding, finished or gone.
// Swipe a download right to pause or resume it, left to remove it; tap it
// for everything else.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { bytes, clockTime, duration, episodeCode, relative, speed } from '@tinystream/shared/downloads'
import { mediaLabels, mediaOptions, ofMediaType } from '@tinystream/shared/media'
import { ArrowDown, ArrowUp, ChevronDown, CircleAlert, CircleCheck, FolderInput, Pause, Play, RefreshCw, ShieldAlert, Sprout, Trash2, Turtle, Users } from 'lucide-react-native'
import { type ReactNode, useEffect, useRef, useState } from 'react'
import { Pressable, Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { haptic } from '../../../modules/haptics'
import { deleteItems, useDeleteDownloaded } from '../../../src/components/downloads'
import { ask, toast, toastError } from '../../../src/components/Feedback'
import { Img } from '../../../src/components/Img'
import { Menu, type MenuItem } from '../../../src/components/Menu'
import { Page, Section } from '../../../src/components/Page'
import { ListSkeleton, useArrived } from '../../../src/components/Skeleton'
import { PieceMap, Sparkline, record } from '../../../src/components/Sparkline'
import { SwipeRow } from '../../../src/components/SwipeRow'
import { Badge, Empty, IconButton, Progress, Select, type Tone } from '../../../src/components/ui'
import { Squircle } from '../../../src/effects/Squircle'
import { Ticker } from '../../../src/effects/Ticker'
import { useMotion } from '../../../src/effects/motion'
import { graphql } from '../../../src/gql'
import { useGo } from '../../../src/nav'
import { type Download, type EngineOverview, useMe } from '../../../src/queries'
import { useApi } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

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

function useRun() {
  const api = useApi()
  return (action: Action, ids: number[]): Promise<unknown> => {
    if (action === 'import') return Promise.all(ids.map((id) => api.request(ImportDoc, { id })))
    if (action === 'pause') return api.request(PauseDoc, { ids })
    if (action === 'resume') return api.request(ResumeDoc, { ids })
    return api.request(RecheckDoc, { ids })
  }
}

const POLL = 1500

type Where = 'active' | 'seeding' | 'finished' | 'gone'

/** Which list each download was in last time, so a move can be celebrated. */
const lastSeen = new Map<number, Where>()

export default function Downloads() {
  const api = useApi()
  const qc = useQueryClient()
  const run = useRun()
  const motion = useMotion()
  const { tokens } = useTheme()
  const [category, setCategory] = useState('')
  const { data, dataUpdatedAt, refetch } = useQuery({ queryKey: ['downloads'], queryFn: async () => (await api.request(DownloadsQuery)).downloads, refetchInterval: POLL })
  const engineQuery = useQuery({ queryKey: ['engine'], queryFn: async () => (await api.request(EngineQuery)).downloadEngine, refetchInterval: POLL })
  const engine = engineQuery.data
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
  const ready = !!data && !engineQuery.isPending
  const arrived = useArrived(ready)
  const order = (n: number) => (arrived ? motion.developIn(n) : undefined)

  return (
    <Page
      title="Downloads"
      onRefresh={() => Promise.all([refetch(), engineQuery.refetch()])}
      right={
        <>
          {running.length > 0 && (
            <IconButton label="Pause all" onPress={() => bulk.mutate('pause')} disabled={bulk.isPending}>
              <Pause size={20} color={tokens.ink} />
            </IconButton>
          )}
          {paused.length > 0 && (
            <IconButton label="Resume all" onPress={() => bulk.mutate('resume')} disabled={bulk.isPending}>
              <Play size={20} color={tokens.ink} />
            </IconButton>
          )}
        </>
      }
    >
      {!ready && <ListSkeleton rows={4} />}
      {ready && (
        <>
          <Select title="Type" value={category} options={mediaOptions} onChange={setCategory} />
          {engine?.killSwitch && (
            <Squircle radius={14} edge className="flex-row items-start gap-3 bg-danger/10 p-4">
              <ShieldAlert size={20} color={tokens.danger} />
              <Text className="font-sans flex-1 text-sm font-medium text-danger">{engine.killSwitch} is down, so every torrent is paused</Text>
            </Squircle>
          )}
          {engine?.listenError && (
            <Squircle radius={14} edge className="flex-row items-start gap-3 bg-warn-deep/10 p-4">
              <CircleAlert size={20} color={tokens.warn} />
              <Text className="font-sans flex-1 text-sm text-ink-2">{engine.listenError}</Text>
            </Squircle>
          )}
          {engine && (
            <Animated.View style={order(0)}>
              <Traffic engine={engine} stamp={engineQuery.dataUpdatedAt} downloads={data} />
            </Animated.View>
          )}
          {filtered.length === 0 && <Empty title={category ? 'No downloads of this type' : 'No downloads yet'} />}
          {active.length > 0 && (
            <Animated.View style={order(1)}>
              <Section title="Downloading" aside={<Text className="font-sans text-xs text-ink-3">{active.length}</Text>}>
                <List items={active} where="active" stamp={dataUpdatedAt} />
              </Section>
            </Animated.View>
          )}
          {seeding.length > 0 && (
            <Animated.View style={order(2)}>
              <Section title="Seeding" aside={<Text className="font-sans text-xs text-ink-3">{seeding.length}</Text>}>
                <List items={seeding} where="seeding" stamp={dataUpdatedAt} />
              </Section>
            </Animated.View>
          )}
          {finished.length > 0 && (
            <Section title="Finished" aside={<Text className="font-sans text-xs text-ink-3">last two weeks</Text>}>
              <List items={finished} where="finished" stamp={dataUpdatedAt} />
            </Section>
          )}
          {gone.length > 0 && (
            <Section title={<Text className="font-sans text-[17px] font-semibold text-danger">Removed & failed</Text>} aside={<Text className="font-sans text-xs text-ink-3">last two weeks</Text>}>
              <List items={gone} where="gone" stamp={dataUpdatedAt} />
            </Section>
          )}
          {engine && (
            <Text className="font-sans text-xs text-ink-3">
              Downloading to {engine.downloadPath} · libtorrent {engine.version}
            </Text>
          )}
        </>
      )}
    </Page>
  )
}

/** Live totals over a rolling graph of the last minute and a half. */
function Traffic({ engine, stamp, downloads }: { engine: EngineOverview; stamp: number; downloads: Download[] }) {
  const { tokens } = useTheme()
  const down = record('engine-down', engine.downloadRate, stamp)
  const up = record('engine-up', engine.uploadRate, stamp)
  const max = Math.max(1, ...down, ...up)
  const peers = downloads.reduce((n, d) => n + (d.live && !d.live.paused ? d.live.peers : 0), 0)
  const shared = downloads.reduce((n, d) => n + (d.live?.uploaded ?? 0), 0)
  return (
    <Squircle radius={18} edge className="overflow-hidden bg-raised">
      <Sparkline values={down} interval={POLL} max={max} color={tokens.info} style={{ position: 'absolute', left: 0, right: 0, bottom: 0, height: 72 }} />
      <Sparkline values={up} interval={POLL} max={max} color={tokens.ok} style={{ position: 'absolute', left: 0, right: 0, bottom: 0, height: 72, opacity: 0.8 }} />
      <View className="gap-4 p-5 pb-7">
        <View className="flex-row gap-8">
          <Stat label="Download" icon={<ArrowDown size={20} color={tokens.info} />} value={speed(engine.downloadRate)} />
          <Stat label="Upload" icon={<ArrowUp size={20} color={tokens.ok} />} value={speed(engine.uploadRate)} />
        </View>
        <View className="flex-row flex-wrap gap-x-5 gap-y-1">
          {engine.slowHours && (
            <View className="flex-row items-center gap-1.5">
              <Turtle size={13} color={tokens['ink-3']} />
              <Text className="font-sans text-xs text-ink-3">slow hours</Text>
            </View>
          )}
          <View className="flex-row items-center gap-1.5">
            <Users size={13} color={tokens['ink-3']} />
            <Ticker value={String(peers)} className="font-sans text-xs text-ink-3" />
            <Text className="font-sans text-xs text-ink-3">peers</Text>
          </View>
          <View className="flex-row items-center gap-1.5">
            <Sprout size={13} color={tokens['ink-3']} />
            <Text className="font-sans text-xs text-ink-3">{bytes(shared)} shared</Text>
          </View>
        </View>
      </View>
    </Squircle>
  )
}

function Stat({ label, value, icon }: { label: string; value: string; icon: ReactNode }) {
  return (
    <View>
      <Text className="font-sans text-xs text-ink-3">{label}</Text>
      <View className="mt-1 flex-row items-center gap-1.5">
        {icon}
        <Ticker value={value} className="font-sans text-[24px] font-semibold tracking-tight text-ink" />
      </View>
    </View>
  )
}

function List({ items, where, stamp }: { items: Download[]; where: Where; stamp: number }) {
  return (
    <Squircle radius={16} edge className={where === 'gone' ? 'bg-danger/5' : 'bg-raised'}>
      {bySeason(items).map((g, i) => (
        <View key={g.key} className={i > 0 ? `border-t ${where === 'gone' ? 'border-danger/10' : 'border-line'}` : ''}>
          {g.items.length === 1 ? <Row d={g.items[0]} where={where} stamp={stamp} /> : <Group season={g.season} items={g.items} where={where} stamp={stamp} />}
        </View>
      ))}
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

const urgency = { danger: 0, warn: 1, live: 2, quiet: 3, ok: 4, strong: 5 }

/** A season's downloads folded into one row that opens into them. */
function Group({ season, items, where, stamp }: { season: number; items: Download[]; where: Where; stamp: number }) {
  const qc = useQueryClient()
  const run = useRun()
  const { tokens } = useTheme()
  const motion = useMotion()
  const lead = items[0]
  const key = `${where}:${lead.seriesId ?? lead.seriesName}:${season}`
  const [open, setOpen] = useState(() => opened.has(key))
  const [menu, setMenu] = useState(false)
  if (!open) for (const d of items) lastSeen.set(d.id, where)
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
    haptic('tick')
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
    <View>
      <SwipeRow
        left={
          live
            ? {
                label: running ? 'Pause all' : 'Resume all',
                icon: running ? <Pause size={20} color={tokens.canvas} /> : <Play size={20} color={tokens.canvas} />,
                color: tokens.info,
                text: tokens.canvas,
                run: () => act.mutate(running ? 'pause' : 'resume'),
              }
            : undefined
        }
      >
        <Pressable onPress={toggle} onLongPress={del ? () => (haptic('longPressOpen'), setMenu(true)) : undefined}>
          <View className="flex-row gap-3.5 bg-raised px-4 py-3.5" style={where === 'gone' ? { backgroundColor: 'transparent' } : undefined}>
            <Thumb poster={lead.poster} dim={where === 'gone'} />
            <View className="min-w-0 flex-1 gap-1">
              <Text className="font-sans text-sm font-medium text-ink" numberOfLines={1}>
                {lead.seriesName ?? lead.name}
              </Text>
              <View className="flex-row flex-wrap gap-1">
                <Badge>{mediaLabels[lead.category]}</Badge>
                <Badge>{episodeRange(season, items)}</Badge>
                <Badge tone={s.tone}>{s.text}</Badge>
                {imported > 0 && (
                  <Badge tone="ok" icon={<CircleCheck size={11} color={tokens.ok} />}>
                    {imported === items.length ? 'In library' : `${imported} of ${items.length} in library`}
                  </Badge>
                )}
              </View>
              <View className="flex-row items-center gap-1">
                <Animated.View style={{ transform: [{ rotate: open ? '180deg' : '0deg' }] }}>
                  <ChevronDown size={14} color={tokens['ink-3']} />
                </Animated.View>
                <Text className="font-sans text-xs text-ink-3">{items.length} downloads</Text>
              </View>
              {where === 'active' && (
                <View className="mt-1.5 gap-1.5">
                  <Progress value={progress ?? 0} duration={POLL} />
                  <View className="flex-row flex-wrap gap-x-4">
                    <Text className="font-sans text-xs text-ink-2">{progress !== null ? `${(progress * 100).toFixed(1)}%` : '—'}</Text>
                    <Text className="font-sans text-xs text-ink-3">
                      {bytes(done)} of {bytes(size)}
                    </Text>
                    {rate > 0 && <Text className="font-sans text-xs text-info">↓ {speed(rate)}</Text>}
                  </View>
                </View>
              )}
              {where === 'seeding' && <Text className="font-sans text-xs text-ink-3">{bytes(shared)} shared</Text>}
            </View>
          </View>
        </Pressable>
      </SwipeRow>
      {open && (
        <Animated.View style={[motion.unfold, { paddingLeft: 40 }]}>
          {items.map((d) => (
            <View key={d.id} className="border-t border-line">
              <Row d={d} where={where} stamp={stamp} nested />
            </View>
          ))}
        </Animated.View>
      )}
      {del && <Menu open={menu} onClose={() => setMenu(false)} items={deleteItems(del.target, del.run)} />}
    </View>
  )
}

/** Deleting `d`'s season or show, for people who may. */
function useDeletion(d: Download, season: number | null) {
  const run = useDeleteDownloaded()
  const can = useMe()?.permissions
  if (d.seriesId == null || !can?.manageShows || !can.downloads) return null
  return { target: { seriesId: d.seriesId, show: d.seriesName ?? d.name, season }, run }
}

function status(d: Download): { text: string; tone: Tone } {
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

function Thumb({ poster, dim }: { poster: string | null | undefined; dim?: boolean }) {
  const [broken, setBroken] = useState(false)
  return (
    <Squircle radius={7} className="bg-panel" style={{ width: 40, aspectRatio: 2 / 3, alignSelf: 'flex-start' }}>
      {poster && !broken && <Img src={poster} style={{ flex: 1, opacity: dim ? 0.5 : 1 }} onError={() => setBroken(true)} />}
    </Squircle>
  )
}

function Row({ d, where, stamp, nested }: { d: Download; where: Where; stamp: number; nested?: boolean }) {
  const api = useApi()
  const qc = useQueryClient()
  const run = useRun()
  const go = useGo()
  const motion = useMotion()
  const { tokens } = useTheme()
  const [menu, setMenu] = useState(false)
  // Glow once when it arrives here from the list above.
  const [arrived] = useState(() => {
    const before = lastSeen.get(d.id)
    return before !== undefined && before !== where
  })
  lastSeen.set(d.id, where)
  const imported = useRef(d.importState === 'DONE')
  useEffect(() => {
    if (d.importState === 'DONE' && !imported.current) haptic('success')
    imported.current = d.importState === 'DONE'
  }, [d.importState])
  const act = useMutation({
    mutationFn: (action: Action) => run(action, [d.id]),
    onError: toastError,
    onSettled: () => void qc.invalidateQueries({ queryKey: ['downloads'] }),
  })
  const remove = useMutation({
    mutationFn: (files: boolean) => api.request(RemoveDoc, { ids: [d.id], deleteFiles: files }),
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
  // Aim the bar where it'll be at the next poll, and glide there, so it moves continuously.
  const predicted = progress !== null && l && d.size ? Math.min(1, (l.done + l.downloadRate * (POLL / 1000)) / d.size) : progress
  const rates = downloading && l ? record(`dl${d.id}`, l.downloadRate, stamp) : []
  const eps = d.episodes
  const label = eps.length === 0 ? null : eps.length === 1 ? episodeCode(eps[0].season, eps[0].episode) : `${eps.length} episodes`
  const error = d.error ?? (d.importState === 'FAILED' ? d.importError : null)
  const paused = d.state === 'PAUSED'
  const live = downloading || d.state === 'SEEDING' || paused
  const seeded = (d.state === 'SEEDING' || (paused && d.finishedAt)) && l
  const removable = d.state !== 'REMOVED' && d.state !== 'DONE'
  const importable = !!d.finishedAt && d.importState !== 'DONE' && d.state !== 'REMOVED'
  const deleteFiles = async () => {
    const ok = await ask({
      title: 'Delete this download?',
      body: 'Its files are deleted from the download folder. Anything already in your library stays.',
      confirm: 'Delete',
      danger: true,
    })
    if (ok) remove.mutate(true)
  }
  const items: (MenuItem | false)[] = [
    !!d.title && { label: 'Open the title', icon: (c) => <CircleCheck size={18} color={c} />, onPress: () => go(`title/${d.title!.id}`) },
    live && { label: paused ? 'Resume' : 'Pause', icon: (c) => (paused ? <Play size={18} color={c} /> : <Pause size={18} color={c} />), onPress: () => act.mutate(paused ? 'resume' : 'pause') },
    live && { label: 'Check files again', icon: (c) => <RefreshCw size={18} color={c} />, onPress: () => act.mutate('recheck') },
    importable && { label: 'Import into the library', icon: (c) => <FolderInput size={18} color={c} />, onPress: () => act.mutate('import') },
    removable && { label: 'Remove, keep its files', icon: (c) => <Trash2 size={18} color={c} />, onPress: () => remove.mutate(false) },
    removable && { label: 'Remove and delete download', icon: (c) => <Trash2 size={18} color={c} />, danger: true, onPress: () => void deleteFiles() },
    ...(del ? deleteItems(del.target, del.run) : []),
  ]

  return (
    <SwipeRow
      left={
        live
          ? {
              label: paused ? 'Resume' : 'Pause',
              icon: paused ? <Play size={20} color={tokens.canvas} /> : <Pause size={20} color={tokens.canvas} />,
              color: tokens.info,
              text: tokens.canvas,
              run: () => act.mutate(paused ? 'resume' : 'pause'),
            }
          : undefined
      }
      right={removable ? { label: 'Remove', icon: <Trash2 size={20} color={tokens.canvas} />, color: tokens.danger, text: tokens.canvas, run: () => remove.mutate(false) } : undefined}
    >
      <Pressable
        onPress={() => {
          haptic('press')
          setMenu(true)
        }}
      >
        {({ pressed }) => (
          <Animated.View
            className={`flex-row gap-3.5 px-4 py-3.5 ${pressed ? 'bg-press' : where === 'gone' ? '' : 'bg-raised'}`}
            style={arrived ? motion.arrive : undefined}
          >
            {!nested && <Thumb poster={d.poster} dim={where === 'gone'} />}
            <View className="min-w-0 flex-1 gap-1">
              <Text className="font-sans text-sm font-medium text-ink" numberOfLines={1}>
                {nested ? (label ?? d.name) : (d.seriesName ?? d.name)}
              </Text>
              <View className="flex-row flex-wrap gap-1">
                {!nested && <Badge>{mediaLabels[d.category]}</Badge>}
                {label && !nested && <Badge>{label}</Badge>}
                <Badge tone={s.tone}>{s.text}</Badge>
                {d.importState === 'DONE' && (
                  <Badge tone="ok" icon={<CircleCheck size={11} color={tokens.ok} />}>
                    In library
                  </Badge>
                )}
              </View>
              <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
                {d.name}
              </Text>
              {downloading && (
                <View className="mt-1.5 gap-1.5">
                  <Progress value={predicted ?? 0} duration={POLL} />
                  {l && l.pieces.length > 0 && progress !== null && <PieceMap pieces={l.pieces} color={tokens.info} />}
                  <View className="flex-row items-end gap-3">
                    <View className="min-w-0 flex-1 flex-row flex-wrap gap-x-3 gap-y-0.5">
                      {progress !== null ? (
                        <Ticker value={`${(progress * 100).toFixed(1)}%`} className="font-sans text-xs text-ink-2" />
                      ) : (
                        <Text className="font-sans text-xs text-ink-2">—</Text>
                      )}
                      <Text className="font-sans text-xs text-ink-3">
                        {bytes(l?.done)} of {bytes(d.size)}
                      </Text>
                      {l && l.downloadRate > 0 && <Ticker value={`↓ ${speed(l.downloadRate)}`} className="font-sans text-xs text-info" />}
                      {l?.eta != null && (
                        <Text className="font-sans text-xs text-ink-3">
                          {duration(l.eta)} left · done at {clockTime(Date.now() / 1000 + l.eta)}
                        </Text>
                      )}
                      {l && (
                        <Text className="font-sans text-xs text-ink-3">
                          {l.seeds} seeds, {l.peers} peers
                        </Text>
                      )}
                    </View>
                    {rates.length > 0 && <Sparkline values={rates} interval={POLL} color={tokens.info} style={{ width: 80, height: 24 }} />}
                  </View>
                </View>
              )}
              {seeded && <SeedLine d={d} />}
              {!live && (
                <Text className="font-sans text-xs text-ink-3">
                  Added {relative(d.addedAt)}
                  {d.importedAt && ` · imported ${relative(d.importedAt)}`}
                  {d.requestedBy && ` · by ${d.requestedBy.username}`}
                </Text>
              )}
              {error && (
                <View className="mt-1 flex-row items-start gap-1.5">
                  <CircleAlert size={13} color={tokens.danger} style={{ marginTop: 1 }} />
                  <Text className="font-sans flex-1 text-xs text-danger">{error}</Text>
                </View>
              )}
            </View>
          </Animated.View>
        )}
      </Pressable>
      <Menu open={menu} onClose={() => setMenu(false)} items={items} note="Nothing to do; it's finished." />
    </SwipeRow>
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
    <View className="mt-1.5 gap-1.5">
      {toward && <Progress value={Math.max(0.01, Math.min(1, toward.value))} tone={paused ? 'quiet' : 'ok'} />}
      <Text className="font-sans text-xs text-ink-3">
        {[
          toward ? toward.text : `ratio ${l.ratio.toFixed(2)}`,
          `${bytes(l.uploaded)} shared`,
          !paused && l.uploadRate > 0 ? `↑ ${speed(l.uploadRate)}` : null,
          paused ? `paused · seeded for ${duration(l.seedingSeconds)}` : `seeding for ${duration(l.seedingSeconds)}`,
          !toward && !paused ? 'no end set' : null,
        ]
          .filter(Boolean)
          .join(' · ')}
      </Text>
    </View>
  )
}
