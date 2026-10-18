// SPDX-License-Identifier: AGPL-3.0-or-later
// A book library (web/src/books/Library.tsx): filters by how far they've
// been read, and every book as a cover, a series' books together.

import { useQuery } from '@tanstack/react-query'
import { type ReactNode, useCallback, useDeferredValue, useMemo, useState } from 'react'
import { ScrollView, Text, View } from 'react-native'
import { type BookCard, BooksQuery } from '../books'
import { useApi } from '../session'
import { BookTile } from './BookTile'
import { Page } from './Page'
import { PosterGridSkeleton, useArrived, useColumnWidth } from './Skeleton'
import { Chip, Empty } from './ui'

type Filter = 'all' | 'reading' | 'unread' | 'read'
type Sort = 'title' | 'author' | 'added'

const filters: { value: Filter; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'reading', label: 'Reading' },
  { value: 'unread', label: 'Not started' },
  { value: 'read', label: 'Read' },
]

function matches(b: BookCard, f: Filter) {
  const r = b.reading
  if (f === 'read') return !!r?.finished
  if (f === 'reading') return !!r && !r.finished && r.position > 0
  if (f === 'unread') return !r || (!r.finished && r.position === 0)
  return true
}

const surname = (b: BookCard) => (b.authors[0] ?? '').split(' ').pop()?.toLowerCase() ?? ''

const GAP = 12

export function BookLibrary({ name, switcher }: { name: string; switcher: ReactNode }) {
  const api = useApi()
  const width = useColumnWidth(3, 20, GAP)
  const [filter, setFilter] = useState<Filter>('all')
  const [sort, setSort] = useState<Sort>('title')
  const { data, refetch } = useQuery({
    queryKey: ['books', 'library', name],
    queryFn: async () => (await api.request(BooksQuery, { library: name })).books,
  })
  useArrived(!!data)

  const shownFilter = useDeferredValue(filter)
  const shownSort = useDeferredValue(sort)
  const books = useMemo(() => {
    const list = (data ?? []).filter((b) => matches(b, shownFilter))
    if (shownSort === 'author') list.sort((a, b) => surname(a).localeCompare(surname(b)))
    if (shownSort === 'added') list.sort((a, b) => b.addedAt - a.addedAt)
    return list
  }, [data, shownFilter, shownSort])
  const renderItem = useCallback(({ item }: { item: BookCard }) => <BookTile book={item} width={width} />, [width])

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
        <Chip on={sort === 'author'} onPress={() => setSort('author')}>
          Author
        </Chip>
        <Chip on={sort === 'added'} onPress={() => setSort('added')}>
          Newest
        </Chip>
      </ScrollView>
      {data && (
        <Text className="font-sans text-xs text-ink-3">
          {books.length === data.length ? `${data.length} book${data.length === 1 ? '' : 's'}` : `${books.length} of ${data.length}`}
        </Text>
      )}
      {!data && <PosterGridSkeleton />}
      {data && books.length === 0 && <Empty title={data.length ? 'Nothing matches this filter' : 'This library is empty'} />}
    </View>
  )

  return (
    <Page<BookCard>
      title={name}
      header={header}
      onRefresh={() => refetch()}
      list={{
        data: books,
        numColumns: 3,
        keyExtractor: (b) => String(b.id),
        renderItem,
        columnWrapperStyle: { gap: GAP, paddingHorizontal: 20 },
        contentContainerStyle: { rowGap: 20 },
        initialNumToRender: 12,
        windowSize: 7,
      }}
    />
  )
}
