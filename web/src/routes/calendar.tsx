// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { ArrowDown, ChevronLeft, ChevronRight, Play, Radio } from 'lucide-react'
import { useMemo, useState } from 'react'
import { StateBadge } from '../components/downloads'
import { Empty, Page, PageTitle } from '../components/Page'
import { Squircle } from '../components/Squircle'
import { Ticker } from '../components/Ticker'
import { Button, IconButton } from '../components/ui'
import { graphql } from '../gql'
import { type CalendarEntry, request } from '../lib/api'

const CalendarQuery = graphql(`
  query Calendar($from: Int!, $to: Int!) {
    calendar(from: $from, to: $to) {
      ...CalendarEntryFields
    }
  }
`)

const calendar = async (from: number, to: number) => (await request(CalendarQuery, { from: Math.floor(from), to: Math.floor(to) })).calendar
import { airs, clockTime, countdown, duration, episodeCode, speed, startOfDay, useNow } from '../lib/downloads'
import { useTitle } from '../lib/title'
import { Img } from '../components/Img'

export const Route = createFileRoute('/calendar')({ component: CalendarPage })

const DAY = 86400000

function weekStart(d: Date) {
  const s = startOfDay(d)
  // Weeks start on Monday.
  s.setDate(s.getDate() - ((s.getDay() + 6) % 7))
  return s
}

function CalendarPage() {
  useTitle('Calendar')
  const [offset, setOffset] = useState(0)
  const now = useNow(1000)
  const start = useMemo(() => new Date(weekStart(new Date()).getTime() + offset * 7 * DAY), [offset])
  const end = new Date(start.getTime() + 7 * DAY)
  const { data } = useQuery({
    queryKey: ['calendar', start.getTime()],
    queryFn: () => calendar(start.getTime() / 1000, end.getTime() / 1000),
    refetchInterval: (q) => (q.state.data?.some((e) => e.download) ? 3000 : 60_000),
  })
  const { data: soon } = useQuery({
    queryKey: ['calendar', 'next'],
    queryFn: () => calendar(Date.now() / 1000 - 12 * 3600, Date.now() / 1000 + 30 * 86400),
    // Quick while the hero is following an episode that's being fetched.
    refetchInterval: (q) => {
      const t = Date.now()
      const hero = q.state.data?.find((e) => heroWorthy(e, t))
      if (!hero) return 60_000
      const stage = stageOf(hero, t)
      return stage === 'downloading' ? 2000 : stage === 'searching' ? 10_000 : 60_000
    },
  })
  const next = soon?.find((e) => heroWorthy(e, now))
  const later = next ? (soon ?? []).filter((e) => e.airAt > next.airAt).slice(0, 3) : []
  const days = Array.from({ length: 7 }, (_, i) => new Date(start.getTime() + i * DAY))
  const today = startOfDay(new Date(now)).getTime()
  const range = `${days[0].toLocaleDateString(undefined, { month: 'short', day: 'numeric' })} – ${days[6].toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}`

  return (
    <Page wide>
      <PageTitle
        aside={
          <div className="flex items-center gap-1 pb-0.5">
            <span className="mr-2 hidden text-sm text-ink-3 tabular sm:block">{range}</span>
            <IconButton label="Previous week" onClick={() => setOffset((o) => o - 1)}>
              <ChevronLeft className="size-4.5" />
            </IconButton>
            <Button size="sm" disabled={offset === 0} onClick={() => setOffset(0)}>
              This week
            </Button>
            <IconButton label="Next week" onClick={() => setOffset((o) => o + 1)}>
              <ChevronRight className="size-4.5" />
            </IconButton>
          </div>
        }
      >
        Calendar
      </PageTitle>

      {next && offset === 0 && <UpNext entry={next} now={now} later={later} />}

      {data && data.length === 0 && soon?.length === 0 ? (
        <Empty title="Nothing on the schedule" />
      ) : (
      <div className="grid gap-3 md:grid-cols-7">
        {days.map((d) => {
          const t = d.getTime()
          const list = (data ?? []).filter((e) => e.airAt * 1000 >= t && e.airAt * 1000 < t + DAY)
          const isToday = t === today
          const past = t < today
          return (
            <div key={t} className={`min-w-0 ${list.length === 0 ? 'hidden md:block' : ''}`}>
              <div className="mb-2 flex items-baseline gap-2 px-1">
                <span className={`text-xs font-medium tracking-wide uppercase ${isToday ? 'text-ink' : 'text-ink-3'}`}>
                  {d.toLocaleDateString(undefined, { weekday: 'short' })}
                </span>
                <span
                  className={`grid size-6 place-items-center rounded-full text-[13px] tabular ${isToday ? 'bg-ink font-semibold text-canvas' : past ? 'text-ink-3' : 'text-ink-2'}`}
                >
                  {d.getDate()}
                </span>
              </div>
              <Squircle radius={16} className={`min-h-28 space-y-1.5 p-1.5 md:min-h-[26rem] ${isToday ? 'bg-raised ring-1 ring-line-strong' : 'bg-raised/50'}`}>
                {list.map((e) => (
                  <DayCard key={`${e.seriesId}-${e.season}-${e.episode}`} e={e} now={now} />
                ))}
              </Squircle>
            </div>
          )
        })}
      </div>
      )}
    </Page>
  )
}

function EntryLink({ e, children, className }: { e: CalendarEntry; children: React.ReactNode; className?: string }) {
  if (e.video) {
    return (
      <Link to="/watch/$id" params={{ id: String(e.video.id) }} className={className}>
        {children}
      </Link>
    )
  }
  if (e.title) {
    return (
      <Link to="/title/$id" params={{ id: String(e.title.id) }} className={className}>
        {children}
      </Link>
    )
  }
  return <div className={className}>{children}</div>
}

function DayCard({ e, now }: { e: CalendarEntry; now: number }) {
  const [broken, setBroken] = useState(false)
  const aired = e.airAt * 1000 <= now
  const soon = !aired && e.airAt * 1000 - now < 3600 * 1000
  return (
    <EntryLink e={e} className="group block outline-none">
      <Squircle radius={12} className="flex gap-2.5 p-2 transition-colors group-hover:bg-hover md:flex-col md:gap-2">
        <Squircle radius={8} className="relative aspect-[2/3] w-11 shrink-0 bg-panel md:aspect-video md:w-full">
          {e.poster && (
            <Img src={e.poster} loading="lazy" className="absolute inset-0 size-full object-cover md:hidden" />
          )}
          {(e.backdrop || e.poster) && !broken && (
            <Img
              src={e.backdrop ?? e.poster!}
              loading="lazy"
              onError={() => setBroken(true)}
              className="absolute inset-0 hidden size-full object-cover md:block"
            />
          )}
          {e.video && (
            <div className="absolute inset-0 grid place-items-center bg-media-shade/35 opacity-0 transition-opacity group-hover:opacity-100">
              <Play className="size-5 fill-media-ink text-media-ink" />
            </div>
          )}
          <span className="absolute top-1.5 left-1.5 hidden rounded-md bg-media-shade/60 px-1.5 py-0.5 text-2xs font-medium text-media-ink tabular backdrop-blur-md md:block">
            {clockTime(e.airAt)}
          </span>
        </Squircle>
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] leading-snug font-medium">{e.show}</p>
          <p className="truncate text-xs text-ink-3">
            <span className="tabular">{episodeCode(e.season, e.episode)}</span>
            <span className="md:hidden"> · {clockTime(e.airAt)}</span>
            {e.name && <span> · {e.name}</span>}
          </p>
          {e.download && (
            <div className="mt-2 flex items-center gap-2">
              <div className="h-1 flex-1 overflow-hidden rounded-full bg-press">
                <div className="h-full rounded-full bg-info transition-[width] duration-[3s] ease-linear" style={{ width: `${e.download.progress * 100}%` }} />
              </div>
              <span className="text-2xs text-info tabular">{Math.floor(e.download.progress * 100)}%</span>
            </div>
          )}
          <div className="mt-1.5 flex flex-wrap gap-1">
            {e.download ? null : soon ? (
              <span className="text-2xs font-medium text-warn"><Ticker value={countdown(e.airAt, now)} /></span>
            ) : e.monitor !== 'NONE' || e.state === 'DONE' ? (
              aired || e.state === 'DONE' ? (
                <StateBadge state={e.state} />
              ) : null
            ) : null}
          </div>
        </div>
      </Squircle>
    </EntryLink>
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

function UpNext({ entry: e, now, later }: { entry: CalendarEntry; now: number; later: CalendarEntry[] }) {
  const [broken, setBroken] = useState(false)
  const image = e.backdrop ?? e.poster
  const ambient = e.backdropTint
  const stage = stageOf(e, now)
  const dl = e.download
  const pct = dl ? dl.progress : null
  return (
    <EntryLink e={e} className="group mb-8 block rounded-[23px] outline-offset-3 outline-none focus-visible:outline-2 focus-visible:outline-glow/70">
      <Squircle radius={22} edge className="relative min-h-52 overflow-hidden bg-raised md:h-60">
        {image && !broken && (
          <img
            src={image}
            alt=""
            onError={() => setBroken(true)}
            className="absolute inset-0 size-full object-cover opacity-55 transition-transform duration-700 ease-out group-hover:scale-[1.015]"
          />
        )}
        {ambient && (
          <div
            className="absolute inset-0"
            style={{ background: `radial-gradient(120% 90% at 0% 100%, rgb(${ambient} / 0.35), transparent 60%)` }}
          />
        )}
        <div className="absolute inset-0 bg-linear-to-r from-canvas via-canvas/70 to-transparent" />
        {/* The bar fills the bottom edge while it downloads. */}
        {stage === 'downloading' && (
          <div className="absolute inset-x-0 bottom-0 h-1 bg-media-ink/10">
            {pct === null ? (
              <div className="absolute inset-y-0 w-1/3 animate-[slide_1.2s_ease-in-out_infinite] bg-info" />
            ) : (
              <div className="h-full bg-info transition-[width] duration-[2s] ease-linear" style={{ width: `${pct * 100}%` }} />
            )}
          </div>
        )}
        <div className="relative flex h-full min-h-52 items-end gap-8 p-6 md:p-8">
          <div className="min-w-0 flex-1">
            <p className="flex items-center gap-2 text-xs font-medium tracking-wide text-ink-2 uppercase">
              {stage === 'ready' ? (
                <Play className="size-3.5 fill-current text-ok" />
              ) : stage === 'downloading' ? (
                <ArrowDown className="size-3.5 text-info" />
              ) : (
                <Radio className={`size-3.5 ${stage === 'countdown' ? '' : 'text-warn'}`} />
              )}
              {stageLabels[stage]}
            </p>
            <p className="mt-2 truncate text-[28px] leading-tight font-semibold tracking-[-0.02em] md:text-[34px]">{e.show}</p>
            <p className="mt-1 truncate text-sm text-ink-2">
              <span className="tabular">{episodeCode(e.season, e.episode)}</span>
              {e.name && ` · ${e.name}`} · {airs(e.airAt, now)}
            </p>
            <div key={stage} className="mt-4 flex animate-[pop_240ms_ease-out] items-center gap-4">
              {stage === 'countdown' && (
                <span className="text-[32px] leading-none font-semibold tracking-tight whitespace-nowrap md:text-[40px]">
                  <Ticker value={bigCountdown(e.airAt, now)} />
                </span>
              )}
              {stage === 'searching' && (
                <span className="flex items-center gap-3 text-[32px] leading-none font-semibold tracking-tight md:text-[40px]">
                  <span className="size-3 animate-[pulse-dot_1.6s_ease-in-out_infinite] rounded-full bg-warn" />
                  Searching…
                </span>
              )}
              {stage === 'downloading' && (
                <>
                  <span className="text-[32px] leading-none font-semibold tracking-tight whitespace-nowrap text-info md:text-[40px]">
                    {pct === null ? 'Starting…' : <Ticker value={`${(pct * 100).toFixed(1)}%`} />}
                  </span>
                  {dl && (
                    <span className="flex flex-col text-sm text-ink-2 tabular">
                      {dl.downloadRate > 0 && (
                        <span className="flex items-center gap-1">
                          <ArrowDown className="size-3.5" /> <Ticker value={speed(dl.downloadRate)} />
                        </span>
                      )}
                      {dl.eta != null && <span className="text-ink-3">{duration(dl.eta)} left</span>}
                    </span>
                  )}
                </>
              )}
              {stage === 'ready' && (
                <Squircle
                  radius={14}
                  className="inline-flex h-12 items-center gap-2.5 bg-accent px-6 text-base font-semibold text-on-accent transition-colors group-hover:bg-accent-hover"
                >
                  <Play className="size-5 fill-current" /> Play {episodeCode(e.season, e.episode)}
                </Squircle>
              )}
              {stage === 'out' && (
                <span className="text-[32px] leading-none font-semibold tracking-tight md:text-[40px]">Out now</span>
              )}
            </div>
          </div>
          {later.length > 0 && (
            <div className="hidden w-64 shrink-0 space-y-2 lg:block">
              <p className="text-2xs font-medium tracking-wide text-ink-3 uppercase">Then</p>
              {later.map((l) => (
                <div key={`${l.seriesId}-${l.season}-${l.episode}`} className="min-w-0">
                  <p className="truncate text-[13px] font-medium">{l.show}</p>
                  <p className="truncate text-xs text-ink-3 tabular">
                    {episodeCode(l.season, l.episode)} · {airs(l.airAt, now)}
                  </p>
                </div>
              ))}
            </div>
          )}
        </div>
      </Squircle>
    </EntryLink>
  )
}
