// SPDX-License-Identifier: AGPL-3.0-or-later
// The video and book libraries (web/src/routes/library.$name.tsx), one at a
// time: a switcher across them, filters, and every title or book as a
// cover. Music libraries are in the Music tab.

import { useQuery } from '@tanstack/react-query'
import { useLocalSearchParams } from 'expo-router'
import { type ReactNode, useCallback, useDeferredValue, useEffect, useMemo, useState } from 'react'
import { ScrollView, Text, View } from 'react-native'
import { BookLibrary } from '../../../src/components/BookLibrary'
import { Page } from '../../../src/components/Page'
import { Poster } from '../../../src/components/Poster'
import { PosterGridSkeleton, useArrived, useColumnWidth } from '../../../src/components/Skeleton'
import { Chip, Empty, Segmented } from '../../../src/components/ui'
import { graphql } from '../../../src/gql'
import { type Card, useLibraries } from '../../../src/queries'
import { useApi } from '../../../src/session'
import { stored } from '../../../src/storage'

const LibraryQuery = graphql(`
  query Library($name: String!) {
    library(name: $name) {
      titles {
        ...Card
      }
    }
  }
`)

type Filter = 'all' | 'unwatched' | 'watching' | 'watched'
type Sort = 'title' | 'year'

const filters: { value: Filter; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'watching', label: 'Watching' },
  { value: 'unwatched', label: 'Not started' },
  { value: 'watched', label: 'Watched' },
]

function matches(c: Card, f: Filter) {
  const done = c.videoCount > 0 && c.watchedCount >= c.videoCount
  const started = c.watchedCount > 0 || c.progress !== null
  if (f === 'watched') return done
  if (f === 'watching') return started && !done
  if (f === 'unwatched') return !started
  return true
}

/** The library last looked at, to open on next time. */
const lastLibrary = stored<string | null>('library.last', null)

const GAP = 12

export default function Library() {
  const params = useLocalSearchParams<{ name?: string }>()
  const { data: libraries } = useLibraries()
  const shown = (libraries ?? []).filter((l) => l.kind !== 'MUSIC')
  const last = lastLibrary.use()
  const [picked, setPicked] = useState<string | null>(params.name ?? null)
  useEffect(() => {
    if (params.name) setPicked(params.name)
  }, [params.name])
  const lib = shown.find((l) => l.name === picked) ?? shown.find((l) => l.name === last) ?? shown[0] ?? null

  const choose = (n: string) => {
    setPicked(n)
    lastLibrary.set(n)
  }

  const switcher =
    shown.length > 1 ? <Segmented value={lib?.name ?? ''} options={shown.map((l) => ({ value: l.name, label: l.name }))} onChange={choose} /> : null

  if (lib?.kind === 'BOOKS') return <BookLibrary key={lib.name} name={lib.name} switcher={switcher} />
  return <VideoLibrary name={lib?.name ?? null} switcher={switcher} empty={!!libraries && !lib} />
}

function VideoLibrary({ name, switcher, empty }: { name: string | null; switcher: ReactNode; empty: boolean }) {
  const api = useApi()
  const [filter, setFilter] = useState<Filter>('all')
  const [sort, setSort] = useState<Sort>('title')
  const width = useColumnWidth(3, 20, GAP)

  const { data, refetch } = useQuery({
    queryKey: ['library', name],
    queryFn: async () => (await api.request(LibraryQuery, { name: name! })).library?.titles ?? [],
    enabled: !!name,
  })
  useArrived(!!data)

  // The chips change straight away; the grid follows, after.
  const shownFilter = useDeferredValue(filter)
  const shownSort = useDeferredValue(sort)
  const cards = useMemo(() => {
    const list = (data ?? []).filter((c) => matches(c, shownFilter))
    if (shownSort === 'year') list.sort((a, b) => (b.year ?? 0) - (a.year ?? 0))
    return list
  }, [data, shownFilter, shownSort])
  const renderItem = useCallback(({ item }: { item: Card }) => <Poster card={item} width={width} />, [width])

  const header = (
    <View className="gap-3 pb-5">
      {switcher}
      <ScrollView horizontal showsHorizontalScrollIndicator={false} style={{ marginHorizontal: -20 }} contentContainerStyle={{ paddingHorizontal: 20, gap: 6 }}>
        {filters.map((f) => (
          <Chip key={f.value} on={filter === f.value} onPress={() => setFilter(f.value)}>
            {f.label}
          </Chip>
        ))}
        <View className="w-3" />
        <Chip on={sort === 'title'} onPress={() => setSort('title')}>
          A–Z
        </Chip>
        <Chip on={sort === 'year'} onPress={() => setSort('year')}>
          Newest
        </Chip>
      </ScrollView>
      {data && (
        <Text className="font-sans text-xs text-ink-3">
          {cards.length === data.length ? `${data.length} title${data.length === 1 ? '' : 's'}` : `${cards.length} of ${data.length}`}
        </Text>
      )}
      {!data && name && <PosterGridSkeleton />}
      {empty && <Empty title="No libraries yet">Ask an admin for access.</Empty>}
      {data && cards.length === 0 && <Empty title={data.length ? 'Nothing matches this filter' : 'This library is empty'} />}
    </View>
  )

  return (
    <Page<Card>
      title={name ?? 'Library'}
      header={header}
      onRefresh={() => refetch()}
      list={{
        data: cards,
        numColumns: 3,
        keyExtractor: (c) => String(c.id),
        renderItem,
        columnWrapperStyle: { gap: GAP, paddingHorizontal: 20 },
        contentContainerStyle: { rowGap: 20 },
        initialNumToRender: 12,
        windowSize: 7,
      }}
    />
  )
}
