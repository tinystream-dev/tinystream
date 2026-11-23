// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { EllipsisVertical, Search } from 'lucide-react'
import { useMemo, useState } from 'react'
import { DeleteItems, ReleaseDialog, useDeleteDownloaded } from '../components/downloads'
import { Empty, Page, PageTitle } from '../components/Page'
import { ListSkeleton } from '../components/Skeleton'
import { Squircle } from '../components/Squircle'
import { Button, IconButton, Panel, Popover, Segmented, Swap } from '../components/ui'
import { graphql } from '../gql'
import type { WantedQuery } from '../gql/graphql'
import { request } from '../lib/api'

const WantedQueryDoc = graphql(`
  query Wanted {
    wanted {
      seriesId
      title {
        id
        poster
        backdrop
      }
      show
      season
      episode
      name
      airAt
      aired
      state
      attempts
      searchedAt
      nextSearch
    }
  }
`)

type WantedEntry = WantedQuery['wanted'][number]
import { airs, duration, episodeCode, relative, useNow } from '../lib/downloads'
import { useMe } from '../lib/hooks'
import { useTitle } from '../lib/title'

export const Route = createFileRoute('/wanted')({ component: WantedPage })

type Filter = 'all' | 'wanted' | 'upcoming' | 'missing' | 'grabbed'
const FILTERS: Filter[] = ['all', 'wanted', 'grabbed', 'upcoming', 'missing']
const FILTER_LABELS: Record<Filter, string> = { all: 'All', wanted: 'Searching', grabbed: 'Downloading', upcoming: 'Upcoming', missing: 'Missing' }

/** Wanted episodes that haven't aired aren't being searched for yet. */
const STATUS: Record<string, { text: string; dot: string; ink: string }> = {
  upcoming: { text: 'Upcoming', dot: 'bg-ink-3', ink: 'text-ink-2' },
  WANTED: { text: 'Searching', dot: 'bg-warn', ink: 'text-warn' },
  GRABBED: { text: 'Downloading', dot: 'bg-info', ink: 'text-info' },
  MISSING: { text: 'Missing', dot: 'bg-danger', ink: 'text-danger' },
  SKIPPED: { text: 'Skipped', dot: 'bg-ink-3', ink: 'text-ink-2' },
}
const status = (e: WantedEntry) => STATUS[filterOf(e) === 'upcoming' ? 'upcoming' : e.state] ?? { text: 'Waiting', dot: 'bg-ink-3', ink: 'text-ink-2' }

const filterOf = (e: WantedEntry): Filter => (e.state === 'WANTED' && !e.aired ? 'upcoming' : (e.state.toLowerCase() as Filter))

function when(e: WantedEntry, now: number) {
  if (filterOf(e) === 'upcoming') {
    if (!e.airAt) return ''
    const at = airs(e.airAt, now)
    return `Airs ${/^(Today|Tomorrow) /.test(at) ? at[0].toLowerCase() + at.slice(1) : at}`
  }
  if (e.state === 'WANTED' && e.nextSearch) {
    const left = e.nextSearch - Math.floor(now / 1000)
    return left <= 0 ? 'Looking now' : `Next look in ${duration(left >= 60 ? Math.ceil(left / 60) * 60 : left)}`
  }
  return e.searchedAt ? `Looked ${relative(e.searchedAt, now)}` : ''
}

function Poster({ src }: { src?: string | null }) {
  const [broken, setBroken] = useState(false)
  return (
    <Squircle radius={10} className="aspect-[2/3] w-16 shrink-0 bg-panel">
      {src && !broken && <img src={src} alt="" loading="lazy" onError={() => setBroken(true)} className="size-full object-cover" />}
    </Squircle>
  )
}

function WantedPage() {
  const now = useNow(1000)
  const { data } = useQuery({ queryKey: ['wanted'], queryFn: async () => (await request(WantedQueryDoc)).wanted, refetchInterval: 10_000 })
  const [filter, setFilter] = useState<Filter>('all')
  const canDelete = !!useMe()?.permissions.downloads
  const deleteDownloaded = useDeleteDownloaded()
  const [searching, setSearching] = useState<{ seriesId: number; show: string; season: number; episodes: number[] } | null>(null)

  const groups = useMemo(() => {
    const map = new Map<string, WantedEntry[]>()
    for (const e of data ?? []) {
      if (filter !== 'all' && filterOf(e) !== filter) continue
      const k = `${e.seriesId}:${e.season}`
      map.set(k, [...(map.get(k) ?? []), e])
    }
    return [...map.values()].map((list) => list.sort((a, b) => a.episode - b.episode))
  }, [data, filter])
  const count = (s: Filter) => (data ?? []).filter((e) => s === 'all' || filterOf(e) === s).length
  useTitle('Wanted')
  if (!data) return <ListSkeleton rows={3} />

  return (
    <Page>
      <PageTitle
        aside={
          <Segmented
            size="sm"
            value={filter}
            onChange={setFilter}
            options={FILTERS.map((f) => ({ value: f, label: count(f) > 0 ? `${FILTER_LABELS[f]} ${count(f)}` : FILTER_LABELS[f] }))}
          />
        }
      >
        Wanted
      </PageTitle>
      <Swap value={filter} order={FILTERS}>
        {groups.length === 0 && <Empty title={filter === 'all' ? 'Nothing is missing' : 'Nothing here'} />}
        <div className="space-y-4">
          {groups.map((list) => {
            const first = list[0]
            return (
              <Squircle key={`${first.seriesId}:${first.season}`} radius={20} edge className="bg-raised">
                {(first.title?.backdrop ?? first.title?.poster) && (
                  <img
                    src={first.title?.backdrop ?? first.title?.poster ?? ''}
                    alt=""
                    loading="lazy"
                    className="pointer-events-none absolute inset-x-0 top-0 h-39 w-full object-cover opacity-55 [mask-composite:intersect] [mask-image:linear-gradient(to_right,transparent_35%,black_85%),linear-gradient(to_bottom,black_20%,transparent)]"
                  />
                )}
                <div className="relative flex items-center gap-4.5 p-5 pb-0">
                  <Poster src={first.title?.poster} />
                  <div className="min-w-0 flex-1">
                    {first.title ? (
                      <Link to="/title/$id" params={{ id: String(first.title.id) }} className="text-xl font-semibold tracking-tight hover:underline">
                        {first.show}
                      </Link>
                    ) : (
                      <span className="text-xl font-semibold tracking-tight">{first.show}</span>
                    )}
                    <div className="mt-0.5 text-[13px] text-ink-2">
                      Season {first.season} · {list.length} {list.length === 1 ? 'episode' : 'episodes'}
                    </div>
                  </div>
                  <Button
                    size="sm"
                    onClick={() =>
                      setSearching({
                        seriesId: first.seriesId,
                        show: first.show,
                        season: first.season,
                        episodes: list.length === 1 ? [first.episode] : [],
                      })
                    }
                  >
                    <Search className="size-3.5" /> Search
                  </Button>
                  {canDelete && (
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
                          <DeleteItems
                            target={{ seriesId: first.seriesId, show: first.show, season: first.season }}
                            onDelete={deleteDownloaded}
                            close={close}
                          />
                        </Panel>
                      )}
                    </Popover>
                  )}
                </div>
                <div className="relative divide-y divide-line px-5 pt-2 pb-2 sm:pl-[6.375rem]">
                  {list.map((e) => {
                    const s = status(e)
                    return (
                      <div key={e.episode} className="flex items-center gap-4 py-2.75">
                        <span className="w-16 shrink-0 text-xs text-ink-3 tabular">{episodeCode(e.season, e.episode)}</span>
                        <span className="min-w-0 flex-1 truncate text-sm">{e.name ?? `Episode ${e.episode}`}</span>
                        <span className="hidden shrink-0 text-xs text-ink-3 tabular sm:block">{when(e, now)}</span>
                        <span className={`flex w-28 shrink-0 items-center gap-2 text-[13px] ${s.ink}`}>
                          <span className={`size-1.75 rounded-full ${s.dot}`} />
                          {s.text}
                        </span>
                      </div>
                    )
                  })}
                </div>
              </Squircle>
            )
          })}
        </div>
      </Swap>
      {searching && (
        <ReleaseDialog
          seriesId={searching.seriesId}
          season={searching.season}
          episodes={searching.episodes}
          title={searching.show}
          onClose={() => setSearching(null)}
        />
      )}
    </Page>
  )
}
