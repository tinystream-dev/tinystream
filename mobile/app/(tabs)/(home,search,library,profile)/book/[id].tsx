// SPDX-License-Identifier: AGPL-3.0-or-later
// A book (web/src/routes/book.$id.tsx): its cover leaning with the phone,
// where it was left, what it's about, and the rest of its series.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { percent } from '@tinystream/shared/books'
import { language } from '@tinystream/shared/format'
import { useLocalSearchParams } from 'expo-router'
import { BookOpen, Check, EllipsisVertical, ExternalLink, RefreshCw, Star, Undo2 } from 'lucide-react-native'
import { useState } from 'react'
import { Linking, Pressable, Text, View } from 'react-native'
import { type BookCard, BooksQuery, RefreshBook, SetBookFinished, bookQuery, byline, patchReading, volume } from '../../../../src/books'
import { BigCover, BookTile } from '../../../../src/components/BookTile'
import { toast, toastError } from '../../../../src/components/Feedback'
import { Ambient } from '../../../../src/components/media'
import { Menu } from '../../../../src/components/Menu'
import { Page, Section } from '../../../../src/components/Page'
import { Row } from '../../../../src/components/Row'
import { Button, Empty, IconButton, Progress, Spinner } from '../../../../src/components/ui'
import { Tilt } from '../../../../src/effects/Tilt'
import { useRead } from '../../../../src/nav'
import { useMe } from '../../../../src/queries'
import { useApi } from '../../../../src/session'
import { useTheme } from '../../../../src/theme/ThemeProvider'

const formats = { EPUB: 'EPUB', FB2: 'FictionBook', TXT: 'Plain text' }
const providers: Record<string, string> = { 'apple-books': 'Apple Books', 'open-library': 'Open Library' }

/** The rest of its series, or else more by whoever wrote it. */
function related(book: BookCard, all: BookCard[]): { title: string; books: BookCard[] } | null {
  const others = all.filter((b) => b.id !== book.id)
  if (book.series) {
    const series = others.filter((b) => b.series === book.series)
    if (series.length) return { title: `More in ${book.series}`, books: series }
  }
  const author = book.authors[0]
  const by = author ? others.filter((b) => b.authors.includes(author)) : []
  return by.length ? { title: `More by ${author}`, books: by } : null
}

export default function BookScreen() {
  const id = Number(useLocalSearchParams<{ id: string }>().id)
  const api = useApi()
  const qc = useQueryClient()
  const read = useRead()
  const can = useMe()?.permissions
  const { tokens } = useTheme()
  const [menu, setMenu] = useState(false)
  const [more, setMore] = useState(false)
  const { data: book, isPending, refetch } = useQuery(bookQuery(api, id))
  const { data: library } = useQuery({
    queryKey: ['books', 'library', book?.library],
    queryFn: async () => (await api.request(BooksQuery, { library: book!.library })).books,
    enabled: !!book,
  })

  const finish = useMutation({
    mutationFn: (finished: boolean) => api.request(SetBookFinished, { id, finished }),
    onSuccess: (r, finished) => {
      patchReading(qc, id, r.setBookFinished.reading ?? null)
      void qc.invalidateQueries({ queryKey: ['books', 'home'] })
      toast({ title: finished ? `Marked ${book?.title} read` : `Marked ${book?.title} not started` })
    },
    onError: toastError,
  })
  const refresh = useMutation({
    mutationFn: () => api.request(RefreshBook, { id }),
    onSuccess: () => toast({ title: 'Looking it up again' }),
    onError: toastError,
  })

  if (isPending)
    return (
      <Page title="Book">
        <View className="items-center py-24">
          <Spinner />
        </View>
      </Page>
    )
  if (!book)
    return (
      <Page title="Book">
        <Empty title="This book isn't here anymore" />
      </Page>
    )

  const r = book.reading
  const started = !!r && !r.finished && r.position > 0
  const series = [book.series, volume(book.seriesIndex)].filter(Boolean).join(' · ')
  const facts = [
    book.year,
    book.chapterCount > 0 && `${book.chapterCount} chapter${book.chapterCount === 1 ? '' : 's'}`,
    book.pages && `${book.pages} pages`,
    formats[book.format],
  ].filter(Boolean)
  const others = library ? related(book, library) : null
  const long = (book.description?.length ?? 0) > 400

  return (
    <>
      <Page
        title={book.title}
        large={false}
        onRefresh={() => refetch()}
        right={
          can?.editMetadata ? (
            <IconButton label="More" onPress={() => setMenu(true)}>
              <EllipsisVertical size={20} color={tokens.ink} />
            </IconButton>
          ) : undefined
        }
      >
        <View className="gap-6 pt-2">
          <View className="items-center">
            <Ambient tint={book.coverTint} at="50% 40%" size="90% 70%" />
            <Tilt gyro radius={12} style={{ width: 200, aspectRatio: 2 / 3 }} onPress={() => read(book.id)}>
              <BigCover book={book} width={200} />
            </Tilt>
          </View>
          <View className="items-center gap-1.5">
            {series ? <Text className="font-sans text-xs font-medium uppercase tracking-wide text-ink-3">{series}</Text> : null}
            <Text className="font-sans text-center text-[26px] font-semibold leading-8 tracking-tight text-ink">{book.title}</Text>
            {book.authors.length > 0 && <Text className="font-sans text-[15px] font-medium text-ink-2">{byline(book.authors)}</Text>}
            <View className="flex-row flex-wrap justify-center gap-x-3 gap-y-1">
              {facts.map((f, i) => (
                <Text key={i} className="font-sans text-[13px] text-ink-3">
                  {f}
                </Text>
              ))}
              {book.rating != null && (
                <View className="flex-row items-center gap-1">
                  <Star size={12} color={tokens.warn} fill={tokens.warn} />
                  <Text className="font-sans text-[13px] text-ink-3">{(book.rating / 2).toFixed(1)}</Text>
                </View>
              )}
            </View>
          </View>
          {started && (
            <View className="flex-row items-center gap-3">
              <View className="flex-1">
                <Progress value={r.progress} tone="ink" />
              </View>
              <Text className="font-sans text-xs text-ink-2">{percent(r.position, book.length)}%</Text>
            </View>
          )}
          <View className="flex-row gap-2">
            <View className="flex-1">
              <Button variant="primary" size="lg" onPress={() => read(book.id)} icon={<BookOpen size={18} color={tokens['on-accent']} />}>
                {started ? 'Continue reading' : r?.finished ? 'Read again' : 'Read'}
              </Button>
            </View>
            <Button
              size="lg"
              disabled={finish.isPending}
              onPress={() => finish.mutate(!r?.finished)}
              icon={r?.finished ? <Undo2 size={18} color={tokens.ink} /> : <Check size={18} color={tokens.ink} />}
            >
              {r?.finished ? 'Not started' : 'Mark read'}
            </Button>
          </View>
          {book.description && (
            <Pressable onPress={() => long && setMore((m) => !m)}>
              <Text className="font-sans text-[15px] leading-6 text-ink-2" numberOfLines={long && !more ? 7 : undefined}>
                {book.description}
              </Text>
              {long && <Text className="font-sans mt-1.5 text-[13px] text-ink-3">{more ? 'Less' : 'More'}</Text>}
            </Pressable>
          )}
          {book.genres.length > 0 && (
            <View className="flex-row flex-wrap gap-1.5">
              {book.genres.map((g) => (
                <View key={g} className="rounded-md bg-panel px-2 py-0.5">
                  <Text className="font-sans text-xs text-ink-2">{g}</Text>
                </View>
              ))}
            </View>
          )}
          <View className="gap-1">
            <Text className="font-sans text-xs leading-5 text-ink-3">
              {[book.publisher, book.published && `Published ${book.published}`, book.language && language(book.language), book.isbn && `ISBN ${book.isbn}`]
                .filter(Boolean)
                .join(' · ')}
            </Text>
            {book.provider && book.providerUrl && (
              <Pressable onPress={() => void Linking.openURL(book.providerUrl!)} className="flex-row items-center gap-1" hitSlop={8}>
                <Text className="font-sans text-xs text-ink-3">Details from {providers[book.provider] ?? book.provider}</Text>
                <ExternalLink size={11} color={tokens['ink-3']} />
              </Pressable>
            )}
          </View>
          {others && (
            <Section title={others.title}>
              <Row data={others.books} width={112} keyOf={(b) => b.id} render={(b) => <BookTile book={b} width={112} />} />
            </Section>
          )}
        </View>
      </Page>
      <Menu
        open={menu}
        onClose={() => setMenu(false)}
        items={[can?.editMetadata && { label: 'Refresh details', icon: (c) => <RefreshCw size={18} color={c} />, onPress: () => refresh.mutate() }]}
      />
    </>
  )
}
