// SPDX-License-Identifier: AGPL-3.0-or-later
// The calendar (web/src/routes/calendar.tsx) as an agenda: what's next, big,
// then two weeks of episodes by day. The day at the top and its week stay
// pinned under the bar; tap a day to jump to it.

import { useQuery } from '@tanstack/react-query'
import { airs, clockTime, countdown, duration, episodeCode, speed, startOfDay } from '@tinystream/shared/downloads'
import { ArrowDown, ChevronLeft, ChevronRight, Play, Radio } from 'lucide-react-native'
import { useCallback, useMemo, useRef, useState } from 'react'
import { Pressable, Text, View, type ViewToken } from 'react-native'
import type { FlatList } from 'react-native-gesture-handler'
import Animated from 'react-native-reanimated'
import { haptic } from '../../../modules/haptics'
import { StateBadge } from '../../../src/components/downloads'
import { Img } from '../../../src/components/Img'
import { Ambient, Shade } from '../../../src/components/media'
import { Page } from '../../../src/components/Page'
import { ListSkeleton, useArrived } from '../../../src/components/Skeleton'
import { Empty, IconButton, Progress } from '../../../src/components/ui'
import { Squircle } from '../../../src/effects/Squircle'
import { Ticker } from '../../../src/effects/Ticker'
import { Tilt } from '../../../src/effects/Tilt'
import { useMotion } from '../../../src/effects/motion'
import { graphql } from '../../../src/gql'
import { useGo, useWatch } from '../../../src/nav'
import { type CalendarEntry, useNow } from '../../../src/queries'
import { useApi } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

const CalendarQuery = graphql(`
  query Calendar($from: Int!, $to: Int!) {
    calendar(from: $from, to: $to) {
      ...CalendarEntryFields
    }
  }
`)

const DAY = 86400000
/** How many days the agenda shows at once. */
const SPAN = 14
const PINNED = 92

function weekStart(d: Date) {
  const s = startOfDay(d)
  // Weeks start on Monday.
  s.setDate(s.getDate() - ((s.getDay() + 6) % 7))
  return s
}

type Line = { kind: 'day'; day: number } | { kind: 'entry'; e: CalendarEntry; day: number } | { kind: 'none'; day: number }

const keyOf = (e: CalendarEntry) => `${e.seriesId}-${e.season}-${e.episode}`

export default function Calendar() {
  const api = useApi()
  const { tokens } = useTheme()
  const [offset, setOffset] = useState(0)
  const now = useNow(1000)
  const list = useRef<FlatList<Line>>(null)
  const start = useMemo(() => weekStart(new Date()).getTime() + offset * 7 * DAY, [offset])
  const end = start + SPAN * DAY
  const calendar = (from: number, to: number) => api.request(CalendarQuery, { from: Math.floor(from), to: Math.floor(to) }).then((r) => r.calendar)
  const { data, refetch } = useQuery({
    queryKey: ['calendar', start],
    queryFn: () => calendar(start / 1000, end / 1000),
    refetchInterval: (q) => (q.state.data?.some((e) => e.download) ? 3000 : 60_000),
  })
  const { data: soon } = useQuery({
    queryKey: ['calendar', 'next'],
    queryFn: () => calendar(Date.now() / 1000 - 12 * 3600, Date.now() / 1000 + 30 * 86400),
    refetchInterval: (q) => {
      const t = Date.now()
      const hero = q.state.data?.find((e) => heroWorthy(e, t))
      if (!hero) return 60_000
      const stage = stageOf(hero, t)
      return stage === 'downloading' ? 2000 : stage === 'searching' ? 10_000 : 60_000
    },
  })
  useArrived(!!data)
  const next = soon?.find((e) => heroWorthy(e, now))
  const today = startOfDay(new Date(now)).getTime()
  const days = Array.from({ length: SPAN }, (_, i) => start + i * DAY)

  const lines = useMemo(() => {
    const out: Line[] = []
    for (const day of days) {
      const entries = (data ?? []).filter((e) => e.airAt * 1000 >= day && e.airAt * 1000 < day + DAY)
      // Days before today with nothing on them are left out; today always shows.
      if (entries.length === 0 && day !== today) continue
      out.push({ kind: 'day', day })
      if (entries.length) for (const e of entries) out.push({ kind: 'entry', e, day })
      else out.push({ kind: 'none', day })
    }
    return out
  }, [data, start, today])

  const [visible, setVisible] = useState<number | null>(null)
  const onViewable = useRef(({ viewableItems }: { viewableItems: ViewToken<Line>[] }) => {
    const first = viewableItems.find((v) => v.item)
    if (first?.item) setVisible(first.item.day)
  }).current
  const shownDay = visible && visible >= start && visible < end ? visible : (days.find((d) => d >= today) ?? start)
  const week = weekStart(new Date(shownDay)).getTime()
  const strip = Array.from({ length: 7 }, (_, i) => week + i * DAY)
  const busy = new Set((data ?? []).map((e) => startOfDay(new Date(e.airAt * 1000)).getTime()))

  const jump = useCallback(
    (day: number) => {
      const i = lines.findIndex((l) => l.kind === 'day' && l.day >= day)
      haptic('tick')
      if (i >= 0) list.current?.scrollToIndex({ index: i, viewOffset: 4, animated: true })
    },
    [lines],
  )

  const pinned = (
    <View style={{ flex: 1, paddingHorizontal: 12, paddingTop: 2 }}>
      <View className="flex-row items-center">
        <IconButton label="Earlier" size={34} onPress={() => setOffset((o) => o - 1)}>
          <ChevronLeft size={18} color={tokens['ink-2']} />
        </IconButton>
        <Text className="font-sans flex-1 text-center text-sm font-medium text-ink">
          {new Date(shownDay).toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric' })}
        </Text>
        <IconButton label="Later" size={34} onPress={() => setOffset((o) => o + 1)}>
          <ChevronRight size={18} color={tokens['ink-2']} />
        </IconButton>
      </View>
      <View className="mt-1 flex-row justify-between px-1">
        {strip.map((d) => {
          const on = startOfDay(new Date(d)).getTime() === startOfDay(new Date(shownDay)).getTime()
          const isToday = d === today
          return (
            <Pressable key={d} onPress={() => (d >= start && d < end ? jump(d) : setOffset((o) => o + Math.round((d - start) / (7 * DAY))))} hitSlop={4}>
              <View className="items-center gap-0.5" style={{ width: 40 }}>
                <Text className={`font-sans text-2xs uppercase ${isToday ? 'text-ink' : 'text-ink-3'}`}>{new Date(d).toLocaleDateString(undefined, { weekday: 'narrow' })}</Text>
                <View className={`h-7 w-7 items-center justify-center rounded-full ${on ? 'bg-ink' : ''}`}>
                  <Text className={`font-sans text-[13px] ${on ? 'font-semibold text-canvas' : d < today ? 'text-ink-3' : 'text-ink-2'}`}>{new Date(d).getDate()}</Text>
                </View>
                <View className="h-1 w-1 rounded-full" style={{ backgroundColor: busy.has(d) ? tokens.info : 'transparent' }} />
              </View>
            </Pressable>
          )
        })}
      </View>
    </View>
  )

  return (
    <Page<Line>
      title="Calendar"
      large={false}
      pinned={pinned}
      pinnedHeight={PINNED}
      scrollRef={list}
      onRefresh={() => refetch()}
      header={
        <View className="gap-4 pb-2 pt-4">
          {next && offset === 0 && <UpNext entry={next} now={now} />}
          {!data && <ListSkeleton rows={5} height={72} />}
          {data && data.length === 0 && soon?.length === 0 && <Empty title="Nothing on the schedule" />}
        </View>
      }
      list={{
        data: data && !(data.length === 0 && soon?.length === 0) ? lines : [],
        keyExtractor: (l) => (l.kind === 'entry' ? keyOf(l.e) : `${l.kind}${l.day}`),
        renderItem: ({ item }) => <LineView line={item} now={now} today={today} />,
        onViewableItemsChanged: onViewable,
        viewabilityConfig: { itemVisiblePercentThreshold: 10 },
        onScrollToIndexFailed: () => {},
        contentContainerStyle: { paddingHorizontal: 20 },
      }}
    />
  )
}

function LineView({ line, now, today }: { line: Line; now: number; today: number }) {
  if (line.kind === 'day') {
    const d = new Date(line.day)
    return (
      <View className="flex-row items-baseline gap-2 pb-2 pt-5">
        <Text className={`font-sans text-[15px] font-semibold ${line.day === today ? 'text-ink' : line.day < today ? 'text-ink-3' : 'text-ink-2'}`}>
          {line.day === today ? 'Today' : line.day === today + DAY ? 'Tomorrow' : d.toLocaleDateString(undefined, { weekday: 'long' })}
        </Text>
        <Text className="font-sans text-xs text-ink-3">{d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}</Text>
      </View>
    )
  }
  if (line.kind === 'none') return <Text className="font-sans pb-2 text-sm text-ink-3">Nothing today</Text>
  return <DayCard e={line.e} now={now} />
}

function useOpen() {
  const watch = useWatch()
  const go = useGo()
  return (e: CalendarEntry) => (e.video ? () => watch(e.video!.id) : e.title ? () => go(`title/${e.title!.id}`) : undefined)
}

function DayCard({ e, now }: { e: CalendarEntry; now: number }) {
  const open = useOpen()(e)
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
  const aired = e.airAt * 1000 <= now
  const soon = !aired && e.airAt * 1000 - now < 3600 * 1000
  return (
    <Pressable
      disabled={!open}
      onPress={() => {
        haptic('press')
        open?.()
      }}
      style={{ marginBottom: 8 }}
    >
      {({ pressed }) => (
        <Squircle radius={14} edge className={pressed ? 'bg-press' : 'bg-raised'} style={{ flexDirection: 'row', gap: 12, padding: 8, overflow: 'hidden' }}>
          <Ambient tint={e.backdropTint} alpha={0.2} at="0% 50%" size="60% 140%" />
          <Squircle radius={8} className="bg-panel" style={{ width: 44, aspectRatio: 2 / 3 }}>
            {e.poster && !broken && <Img src={e.poster} style={{ flex: 1 }} onError={() => setBroken(true)} />}
            {e.video && (
              <View className="absolute inset-0 items-center justify-center bg-media-shade/30">
                <Play size={16} color={tokens['media-ink']} fill={tokens['media-ink']} />
              </View>
            )}
          </Squircle>
          <View className="min-w-0 flex-1 justify-center gap-0.5">
            <Text className="font-sans text-sm font-medium text-ink" numberOfLines={1}>
              {e.show}
            </Text>
            <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
              {episodeCode(e.season, e.episode)} · {clockTime(e.airAt)}
              {e.name && ` · ${e.name}`}
            </Text>
            {e.download && (
              <View className="mt-1 flex-row items-center gap-2">
                <View className="flex-1">
                  <Progress value={e.download.progress} duration={3000} />
                </View>
                <Text className="font-sans text-2xs text-info">{Math.floor(e.download.progress * 100)}%</Text>
              </View>
            )}
            <View className="mt-0.5 flex-row">
              {e.download ? null : soon ? (
                <Ticker value={countdown(e.airAt, now)} className="font-sans text-2xs font-medium text-warn" />
              ) : (e.monitor !== 'NONE' || e.state === 'DONE') && (aired || e.state === 'DONE') ? (
                <StateBadge state={e.state} />
              ) : null}
            </View>
          </View>
        </Squircle>
      )}
    </Pressable>
  )
}

type Stage = 'countdown' | 'searching' | 'downloading' | 'ready' | 'out'

/** Where an episode is between airing and being playable. */
function stageOf(e: CalendarEntry, now: number): Stage {
  if (e.airAt * 1000 > now) return 'countdown'
  if (e.video) return 'ready'
  if (e.download || e.state === 'GRABBED') return 'downloading'
  if (e.monitor === 'NONE') return 'out'
  return 'searching'
}

/** Whether an entry still deserves the hero spot. */
function heroWorthy(e: CalendarEntry, now: number) {
  const since = now - e.airAt * 1000
  const stage = stageOf(e, now)
  if (stage === 'countdown') return true
  if (stage === 'out') return since < 2 * 3600e3
  return since < 12 * 3600e3
}

/** "13:04:22" inside the last day, "1 d 13 h" before that. */
function bigCountdown(unix: number, now: number) {
  const s = Math.max(0, Math.round(unix - now / 1000))
  if (s >= 86400) return countdown(unix, now).replace('in ', '')
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const pad = (n: number) => String(n).padStart(2, '0')
  return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`
}

const stageLabels: Record<Stage, string> = {
  countdown: 'Up next',
  searching: 'Just aired',
  downloading: 'Downloading',
  ready: 'Ready to watch',
  out: 'Out now',
}

function UpNext({ entry: e, now }: { entry: CalendarEntry; now: number }) {
  const open = useOpen()(e)
  const { tokens } = useTheme()
  const motion = useMotion()
  const [broken, setBroken] = useState(false)
  const image = e.backdrop ?? e.poster
  const stage = stageOf(e, now)
  const dl = e.download
  const pct = dl ? dl.progress : null
  const big = 'font-sans text-[30px] font-semibold tracking-tight'
  return (
    <Tilt gyro max={4} radius={22} style={{ minHeight: 220 }} onPress={open}>
      <View className="flex-1 bg-raised">
        {image && !broken && <Img src={image} style={{ position: 'absolute', inset: 0, opacity: 0.55 }} onError={() => setBroken(true)} />}
        <Ambient tint={e.backdropTint} alpha={0.35} />
        <Shade to="top" stops={[0.95, 0.6, 0.1]} />
        {stage === 'downloading' && (
          <View className="absolute bottom-0 left-0 right-0">
            <Progress value={pct ?? 0} duration={2000} />
          </View>
        )}
        <View className="flex-1 justify-end p-5" style={{ minHeight: 220 }}>
          <View className="flex-row items-center gap-2">
            {stage === 'ready' ? (
              <Play size={13} color={tokens.ok} fill={tokens.ok} />
            ) : stage === 'downloading' ? (
              <ArrowDown size={13} color={tokens.info} />
            ) : (
              <Radio size={13} color={stage === 'countdown' ? tokens['ink-2'] : tokens.warn} />
            )}
            <Text className="font-sans text-xs font-medium uppercase tracking-wider text-ink-2">{stageLabels[stage]}</Text>
          </View>
          <Text className="font-sans mt-2 text-[26px] font-semibold leading-tight tracking-tight text-ink" numberOfLines={2}>
            {e.show}
          </Text>
          <Text className="font-sans mt-1 text-sm text-ink-2" numberOfLines={1}>
            {episodeCode(e.season, e.episode)}
            {e.name && ` · ${e.name}`} · {airs(e.airAt, now)}
          </Text>
          <Animated.View key={stage} style={motion.pop} className="mt-3 flex-row items-center gap-4">
            {stage === 'countdown' && <Ticker value={bigCountdown(e.airAt, now)} className={`${big} text-ink`} />}
            {stage === 'searching' && (
              <View className="flex-row items-center gap-3">
                <Animated.View style={[{ width: 12, height: 12, borderRadius: 6, backgroundColor: tokens.warn }, motion.pulseDot]} />
                <Text className={`${big} text-ink`}>Searching…</Text>
              </View>
            )}
            {stage === 'downloading' && (
              <>
                {pct === null ? <Text className={`${big} text-info`}>Starting…</Text> : <Ticker value={`${(pct * 100).toFixed(1)}%`} className={`${big} text-info`} />}
                {dl && (
                  <View>
                    {dl.downloadRate > 0 && <Text className="font-sans text-sm text-ink-2">↓ {speed(dl.downloadRate)}</Text>}
                    {dl.eta != null && <Text className="font-sans text-sm text-ink-3">{duration(dl.eta)} left</Text>}
                  </View>
                )}
              </>
            )}
            {stage === 'ready' && (
              <Squircle radius={14} className="flex-row items-center gap-2 bg-accent" style={{ height: 46, paddingHorizontal: 20 }}>
                <Play size={18} color={tokens['on-accent']} fill={tokens['on-accent']} />
                <Text className="font-sans text-base font-semibold text-on-accent">Play {episodeCode(e.season, e.episode)}</Text>
              </Squircle>
            )}
            {stage === 'out' && <Text className={`${big} text-ink`}>Out now</Text>}
          </Animated.View>
        </View>
      </View>
    </Tilt>
  )
}
