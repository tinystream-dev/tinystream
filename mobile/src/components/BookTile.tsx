// SPDX-License-Identifier: AGPL-3.0-or-later

import { type Shelf, percent } from '@tinystream/shared/books'
import { BookOpen } from 'lucide-react-native'
import { memo, useEffect, useState } from 'react'
import { PixelRatio, Text, View } from 'react-native'
import { Squircle } from '../effects/Squircle'
import { Tilt } from '../effects/Tilt'
import { type BookCard, bookCover, byline, volume } from '../books'
import { useGo } from '../nav'
import { useTheme } from '../theme/ThemeProvider'
import { Img } from './Img'

/** A book's cover, or its title on a quiet surface when it has none. */
export function BookCover({ book, width }: { book: Pick<BookCard, 'cover' | 'title' | 'authors'>; width: number }) {
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
  useEffect(() => setBroken(false), [book.cover])
  if (book.cover && !broken)
    return <Img src={bookCover(book.cover, PixelRatio.getPixelSizeForLayoutSize(width))} style={{ flex: 1 }} onError={() => setBroken(true)} />
  return (
    <View className="flex-1 justify-between bg-panel p-3">
      <BookOpen size={16} color={tokens['ink-3']} />
      <View>
        <Text className="font-sans text-[15px] font-medium leading-5 text-ink-2" numberOfLines={4}>
          {book.title}
        </Text>
        {book.authors.length > 0 && (
          <Text className="font-sans mt-1 text-xs text-ink-3" numberOfLines={1}>
            {byline(book.authors)}
          </Text>
        )}
      </View>
    </View>
  )
}

/** How far into a book someone is, if they've started it. */
export function progress(book: BookCard): string | null {
  const r = book.reading
  if (r?.finished) return 'Read'
  if (r && r.position > 0) return `${percent(r.position, book.length)}% read`
  return null
}

/** What's under a book's cover: how far in, or which in its series. */
export function caption(book: BookCard): string {
  return progress(book) ?? ([volume(book.seriesIndex), byline(book.authors)].filter(Boolean).join(' · ') || 'Not started')
}

export type SeriesShelf = Extract<Shelf<BookCard>, { kind: 'series' }>

/** What's under a series' covers: the book it's at, or how much of it's been read. */
export function seriesCaption(s: SeriesShelf): string {
  const r = s.current.reading
  if (r && !r.finished && r.position > 0) return [volume(s.current.seriesIndex), progress(s.current)].filter(Boolean).join(' · ')
  const count = `${s.books.length} books`
  if (s.read === s.books.length) return `${count} · Read`
  if (s.read > 0) return `${count} · ${s.read} read`
  return [count, byline(s.current.authors)].filter(Boolean).join(' · ')
}

function ReadingBar({ book }: { book: BookCard }) {
  const r = book.reading
  if (!r || r.finished || r.position <= 0) return null
  return (
    <View className="absolute bottom-2 left-2 right-2 h-1 overflow-hidden rounded-full bg-media-shade/50">
      <View className="h-full bg-media-ink" style={{ width: `${Math.max(4, r.progress * 100)}%` }} />
    </View>
  )
}

export const BookTile = memo(function BookTile({ book, width, name, label }: { book: BookCard; width: number; name?: string; label?: string }) {
  const go = useGo()
  return (
    <View style={{ width }}>
      <Tilt radius={10} style={{ width, aspectRatio: 2 / 3 }} onPress={() => go(`book/${book.id}`)}>
        <View className="flex-1 bg-raised">
          <BookCover book={book} width={width} />
        </View>
        <ReadingBar book={book} />
      </Tilt>
      <Text className="font-sans mt-2 text-[13px] font-medium text-ink" numberOfLines={1}>
        {name ?? book.title}
      </Text>
      <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
        {label ?? caption(book)}
      </Text>
    </View>
  )
})

/** A series as one tile: the book it's at on top of the next two. */
export const SeriesTile = memo(function SeriesTile({ library, shelf, width }: { library: string; shelf: SeriesShelf; width: number }) {
  const go = useGo()
  const behind = shelf.books.filter((b) => b.id !== shelf.current.id).slice(0, 2)
  return (
    <View style={{ width }}>
      <View style={{ width, aspectRatio: 2 / 3 }}>
        {behind
          .map((b, i) => (
            <View
              key={b.id}
              className="absolute inset-0 overflow-hidden rounded-[10px] bg-raised"
              style={{ opacity: 0.55 - 0.2 * i, transform: [{ translateY: -5 * (i + 1) - (width * 1.5 * 0.06 * (i + 1)) / 2 }, { scale: 1 - 0.06 * (i + 1) }] }}
            >
              <BookCover book={b} width={width} />
            </View>
          ))
          .reverse()}
        <Tilt
          radius={10}
          style={{ width, aspectRatio: 2 / 3 }}
          onPress={() => go(`series?library=${encodeURIComponent(library)}&name=${encodeURIComponent(shelf.series)}`)}
        >
          <View className="flex-1 bg-raised">
            <BookCover book={shelf.current} width={width} />
          </View>
          <ReadingBar book={shelf.current} />
        </Tilt>
      </View>
      <Text className="font-sans mt-2 text-[13px] font-medium text-ink" numberOfLines={1}>
        {shelf.series}
      </Text>
      <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
        {seriesCaption(shelf)}
      </Text>
    </View>
  )
})

/** A cover as big as it's shown on the book screen. */
export function BigCover({ book, width }: { book: Pick<BookCard, 'cover' | 'title' | 'authors'>; width: number }) {
  return (
    <Squircle radius={12} edge className="bg-raised" style={{ width, aspectRatio: 2 / 3 }}>
      <BookCover book={book} width={width} />
    </Squircle>
  )
}
