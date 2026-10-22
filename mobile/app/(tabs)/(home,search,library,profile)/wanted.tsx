// SPDX-License-Identifier: AGPL-3.0-or-later
// Episodes being looked for (web/src/routes/wanted.tsx), by show and season.

import { useQuery } from '@tanstack/react-query'
import { airs, countdown, episodeCode, relative } from '@tinystream/shared/downloads'
import { EllipsisVertical, Search } from 'lucide-react-native'
import { useMemo, useState } from 'react'
import { Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { ReleaseSheet, StateBadge, deleteItems, useDeleteDownloaded } from '../../../src/components/downloads'
import { Menu } from '../../../src/components/Menu'
import { Page } from '../../../src/components/Page'
import { ListSkeleton, useArrived } from '../../../src/components/Skeleton'
import { Badge, Button, Empty, IconButton, Segmented, Swap } from '../../../src/components/ui'
import { Squircle } from '../../../src/effects/Squircle'
import { useMotion } from '../../../src/effects/motion'
import { graphql } from '../../../src/gql'
import type { WantedQuery } from '../../../src/gql/graphql'
import { useGo } from '../../../src/nav'
import { useMe, useNow } from '../../../src/queries'
import { useApi } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

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
type Filter = 'all' | 'wanted' | 'upcoming' | 'missing' | 'grabbed'
const FILTERS: Filter[] = ['all', 'wanted', 'grabbed', 'upcoming', 'missing']
const FILTER_LABELS: Record<Filter, string> = { all: 'All', wanted: 'Searching', grabbed: 'Downloading', upcoming: 'Upcoming', missing: 'Missing' }

/** Wanted episodes that haven't aired aren't being searched for yet. */
const filterOf = (e: WantedEntry): Filter => (e.state === 'WANTED' && !e.aired ? 'upcoming' : (e.state.toLowerCase() as Filter))

type Searching = { seriesId: number; show: string; season: number; episodes: number[] }

export default function Wanted() {
  const api = useApi()
  const now = useNow(1000)
  const motion = useMotion()
  const { data, refetch } = useQuery({ queryKey: ['wanted'], queryFn: async () => (await api.request(WantedQueryDoc)).wanted, refetchInterval: 10_000 })
  const arrived = useArrived(!!data)
  const [filter, setFilter] = useState<Filter>('all')
  const [searching, setSearching] = useState<Searching | null>(null)

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

  return (
    <Page
      title="Wanted"
      onRefresh={() => refetch()}
      header={data && <Segmented size="sm" value={filter} onChange={setFilter} options={FILTERS.map((f) => ({ value: f, label: `${FILTER_LABELS[f]} ${count(f)}` }))} />}
    >
      {!data && <ListSkeleton rows={3} height={140} />}
      {data && groups.length === 0 && <Empty title={filter === 'all' ? 'Nothing is missing' : 'Nothing here'} />}
      <Swap value={filter} order={FILTERS} style={{ gap: 12 }}>
        {groups.map((list, i) => (
          <Animated.View key={`${list[0].seriesId}:${list[0].season}`} style={arrived ? motion.developIn(i) : undefined}>
            <Group list={list} now={now} onSearch={setSearching} />
          </Animated.View>
        ))}
      </Swap>
      {searching && (
        <ReleaseSheet open seriesId={searching.seriesId} season={searching.season} episodes={searching.episodes} title={searching.show} onClose={() => setSearching(null)} />
      )}
    </Page>
  )
}

function Group({ list, now, onSearch }: { list: WantedEntry[]; now: number; onSearch: (s: Searching) => void }) {
  const go = useGo()
  const { tokens } = useTheme()
  const canDelete = !!useMe()?.permissions.downloads
  const deleteDownloaded = useDeleteDownloaded()
  const [menu, setMenu] = useState(false)
  const first = list[0]
  return (
    <Squircle radius={16} edge className="bg-raised p-4">
      <View className="mb-2 flex-row items-center gap-2">
        <View className="min-w-0 flex-1">
          <Text
            className="font-sans text-[15px] font-semibold tracking-tight text-ink"
            numberOfLines={2}
            onPress={first.title ? () => go(`title/${first.title!.id}`) : undefined}
          >
            {first.show}
          </Text>
          <Text className="font-sans text-sm text-ink-3">Season {first.season}</Text>
        </View>
        <Button
          size="sm"
          icon={<Search size={14} color={tokens.ink} />}
          onPress={() => onSearch({ seriesId: first.seriesId, show: first.show, season: first.season, episodes: list.length === 1 ? [first.episode] : [] })}
        >
          Search
        </Button>
        {canDelete && (
          <IconButton label="More" onPress={() => setMenu(true)}>
            <EllipsisVertical size={18} color={tokens['ink-2']} />
          </IconButton>
        )}
      </View>
      {list.map((e, i) => {
        const upcoming = filterOf(e) === 'upcoming'
        const when = upcoming ? '' : e.state === 'WANTED' && e.nextSearch ? `next look ${countdown(e.nextSearch, now).replace('airing now', 'now')}` : e.searchedAt ? `looked ${relative(e.searchedAt, now)}` : ''
        return (
          <View key={e.episode} className={`gap-1 py-2.5 ${i > 0 ? 'border-t border-line' : ''}`}>
            <View className="flex-row items-center gap-3">
              <Text className="font-sans w-16 text-xs text-ink-3">{episodeCode(e.season, e.episode)}</Text>
              <Text className="font-sans min-w-0 flex-1 text-sm text-ink" numberOfLines={1}>
                {e.name ?? `Episode ${e.episode}`}
              </Text>
              {upcoming ? <Badge>Not aired</Badge> : <StateBadge state={e.state} />}
            </View>
            {(e.airAt || when) && (
              <Text className="font-sans pl-[76px] text-xs text-ink-3" numberOfLines={1}>
                {[e.airAt ? airs(e.airAt, now) : null, when].filter(Boolean).join(' · ')}
              </Text>
            )}
          </View>
        )
      })}
      <Menu open={menu} onClose={() => setMenu(false)} items={deleteItems({ seriesId: first.seriesId, show: first.show, season: first.season }, deleteDownloaded)} />
    </Squircle>
  )
}
