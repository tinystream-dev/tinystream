// SPDX-License-Identifier: AGPL-3.0-or-later
// A series of books (web/src/routes/library_.$name.series.$series.tsx): the
// book it's at, how much of it's been read, and every book in it in order.

import { useQuery } from '@tanstack/react-query'
import { currentOf, inOrder, percent } from '@tinystream/shared/books'
import { useLocalSearchParams } from 'expo-router'
import { BookOpen } from 'lucide-react-native'
import { useCallback, useMemo } from 'react'
import { Text, View } from 'react-native'
import { type BookCard, BooksQuery, byline, volume } from '../../../src/books'
import { BigCover, BookTile, progress } from '../../../src/components/BookTile'
import { Page } from '../../../src/components/Page'
import { useArrived, useColumnWidth } from '../../../src/components/Skeleton'
import { Button, Empty, Progress, Spinner } from '../../../src/components/ui'
import { Tilt } from '../../../src/effects/Tilt'
import { useGo, useRead } from '../../../src/nav'
import { useApi } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

const GAP = 12

export default function SeriesScreen() {
  const { library, name } = useLocalSearchParams<{ library: string; name: string }>()
  const api = useApi()
  const go = useGo()
  const read = useRead()
  const { tokens } = useTheme()
  const width = useColumnWidth(3, 20, GAP)
  const { data, refetch } = useQuery({
    queryKey: ['books', 'library', library],
    queryFn: async () => (await api.request(BooksQuery, { library })).books,
  })
  useArrived(!!data)
  const books = useMemo(() => inOrder((data ?? []).filter((b) => b.series?.toLowerCase() === name.toLowerCase())), [data, name])
  const renderItem = useCallback(
    ({ item }: { item: BookCard }) => <BookTile book={item} width={width} name={volume(item.seriesIndex) ?? item.title} label={progress(item) ?? 'Not started'} />,
    [width],
  )

  if (!data)
    return (
      <Page title={name}>
        <View className="items-center py-24">
          <Spinner />
        </View>
      </Page>
    )
  if (!books.length)
    return (
      <Page title={name}>
        <Empty title="This series isn't here anymore" />
      </Page>
    )

  const current = currentOf(books)
  const r = current.reading
  const started = !!r && !r.finished && r.position > 0
  const done = books.filter((b) => b.reading?.finished).length
  const authors = [...new Set(books.flatMap((b) => b.authors))]
  const facts = [`${books.length} books`, done > 0 && (done === books.length ? 'All read' : `${done} read`)].filter(Boolean)

  const header = (
    <View className="gap-6 pb-6 pt-2">
      <View className="items-center">
        <Tilt gyro radius={12} style={{ width: 160, aspectRatio: 2 / 3 }} onPress={() => go(`book/${current.id}`)}>
          <BigCover book={current} width={160} />
        </Tilt>
      </View>
      <View className="items-center gap-1.5">
        <Text className="font-sans text-xs font-medium uppercase tracking-wide text-ink-3">Series</Text>
        <Text className="font-sans text-center text-[26px] font-semibold leading-8 tracking-tight text-ink">{books[0].series}</Text>
        {authors.length > 0 && <Text className="font-sans text-[15px] font-medium text-ink-2">{byline(authors)}</Text>}
        <Text className="font-sans text-[13px] text-ink-3">{facts.join(' · ')}</Text>
      </View>
      {done > 0 && done < books.length && (
        <View className="flex-row items-center gap-3">
          <View className="flex-1">
            <Progress value={done / books.length} tone="ink" />
          </View>
          <Text className="font-sans text-xs text-ink-2">{percent(done, books.length)}%</Text>
        </View>
      )}
      <Button variant="primary" size="lg" onPress={() => read(current.id)} icon={<BookOpen size={18} color={tokens['on-accent']} />}>
        {[started ? 'Continue' : done === books.length ? 'Read again' : 'Read', volume(current.seriesIndex)].filter(Boolean).join(' ')}
      </Button>
    </View>
  )

  return (
    <Page<BookCard>
      title={books[0].series ?? name}
      large={false}
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
