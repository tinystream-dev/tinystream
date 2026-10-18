// SPDX-License-Identifier: AGPL-3.0-or-later

import { percent } from '@tinystream/shared/books'
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

/** What's under a book's cover: how far in, or which in its series. */
export function caption(book: BookCard): string {
  const r = book.reading
  if (r?.finished) return 'Read'
  if (r && r.position > 0) return `${percent(r.position, book.length)}% read`
  return [volume(book.seriesIndex), byline(book.authors)].filter(Boolean).join(' · ') || 'Not started'
}

export const BookTile = memo(function BookTile({ book, width }: { book: BookCard; width: number }) {
  const go = useGo()
  const r = book.reading
  const reading = r && !r.finished && r.position > 0
  return (
    <View style={{ width }}>
      <Tilt radius={10} style={{ width, aspectRatio: 2 / 3 }} onPress={() => go(`book/${book.id}`)}>
        <View className="flex-1 bg-raised">
          <BookCover book={book} width={width} />
        </View>
        {reading && (
          <View className="absolute bottom-2 left-2 right-2 h-1 overflow-hidden rounded-full bg-media-shade/50">
            <View className="h-full bg-media-ink" style={{ width: `${Math.max(4, r.progress * 100)}%` }} />
          </View>
        )}
      </Tilt>
      <Text className="font-sans mt-2 text-[13px] font-medium text-ink" numberOfLines={1}>
        {book.title}
      </Text>
      <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
        {caption(book)}
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
