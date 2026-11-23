// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { EllipsisVertical, Search } from 'lucide-react'
import { useMemo, useState } from 'react'
import { DeleteItems, ReleaseDialog, StateBadge, useDeleteDownloaded } from '../components/downloads'
import { Empty, Page, PageTitle } from '../components/Page'
import { ListSkeleton } from '../components/Skeleton'
import { Squircle } from '../components/Squircle'
import { Badge, Button, IconButton, Panel, Popover, Segmented, Swap } from '../components/ui'
import { graphql } from '../gql'
import type { WantedQuery } from '../gql/graphql'
import { request } from '../lib/api'

const WantedQueryDoc = graphql(`
  query Wanted {
    wanted {
      seriesId
      title {
        id
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
import { airs, countdown, episodeCode, relative, useNow } from '../lib/downloads'
import { useMe } from '../lib/hooks'
import { useTitle } from '../lib/title'

export const Route = createFileRoute('/wanted')({ component: WantedPage })

type Filter = 'all' | 'wanted' | 'upcoming' | 'missing' | 'grabbed'
const FILTERS: Filter[] = ['all', 'wanted', 'grabbed', 'upcoming', 'missing']
const FILTER_LABELS: Record<Filter, string> = { all: 'All', wanted: 'Searching', grabbed: 'Downloading', upcoming: 'Upcoming', missing: 'Missing' }

/** Wanted episodes that haven't aired aren't being searched for yet. */
const filterOf = (e: WantedEntry): Filter => (e.state === 'WANTED' && !e.aired ? 'upcoming' : (e.state.toLowerCase() as Filter))

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
              <Squircle key={`${first.seriesId}:${first.season}`} radius={16} edge className="bg-raised p-4">
                <div className="mb-3 flex items-center gap-3">
                  <div className="min-w-0 flex-1">
                    {first.title ? (
                      <Link to="/title/$id" params={{ id: String(first.title.id) }} className="text-[15px] font-semibold tracking-tight hover:underline">
                        {first.show}
                      </Link>
                    ) : (
                      <span className="text-[15px] font-semibold tracking-tight">{first.show}</span>
                    )}
                    <span className="ml-2 text-sm text-ink-3">Season {first.season}</span>
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
                <div className="-mx-2 divide-y divide-line">
                  {list.map((e) => (
                    <div key={e.episode} className="flex items-center gap-3 px-2 py-2">
                      <span className="w-16 shrink-0 text-xs text-ink-3 tabular">{episodeCode(e.season, e.episode)}</span>
                      <span className="min-w-0 flex-1 truncate text-sm">{e.name ?? `Episode ${e.episode}`}</span>
                      <span className="hidden shrink-0 text-xs text-ink-3 sm:block">{e.airAt ? airs(e.airAt, now) : ''}</span>
                      <span className="w-36 shrink-0 text-right text-xs text-ink-3 tabular">
                        {filterOf(e) === 'upcoming'
                          ? ''
                          : e.state === 'WANTED' && e.nextSearch
                            ? `next look ${countdown(e.nextSearch, now).replace('airing now', 'now')}`
                            : e.searchedAt
                              ? `looked ${relative(e.searchedAt, now)}`
                              : ''}
                      </span>
                      {filterOf(e) === 'upcoming' ? <Badge>Not aired</Badge> : <StateBadge state={e.state} />}
                    </div>
                  ))}
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
