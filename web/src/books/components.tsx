// SPDX-License-Identifier: AGPL-3.0-or-later

import { Link } from '@tanstack/react-router'
import { BookOpen } from 'lucide-react'
import { type CSSProperties, type ReactNode, useEffect, useState } from 'react'
import { type Shelf, percent } from '@tinystream/shared/books'
import { Img } from '../components/Img'
import { morphFrom } from '../components/Poster'
import { Squircle } from '../components/Squircle'
import { useTilt } from '../lib/tilt'
import { type BookCard, bookCover, byline, volume } from './api'

/** A book's cover, or its title on a quiet surface when it has none. */
export function BookCover({ book, size, className = '', morph }: { book: Pick<BookCard, 'cover' | 'title' | 'authors'>; size: number; className?: string; morph?: boolean }) {
  const [broken, setBroken] = useState(false)
  useEffect(() => setBroken(false), [book.cover])
  return (
    <Squircle radius={Math.max(6, Math.round(size / 28))} edge className={`aspect-[2/3] bg-raised ${className}`} data-morph={morph || undefined}>
      {book.cover && !broken ? (
        <Img src={bookCover(book.cover, size)} loading="lazy" decoding="async" onError={() => setBroken(true)} className="size-full object-cover" />
      ) : (
        <div className="flex size-full flex-col justify-between bg-gradient-to-br from-panel to-float p-3">
          <BookOpen className="size-4 text-ink-3" />
          <div className="min-w-0">
            <p className="line-clamp-4 text-[15px] leading-snug font-medium text-ink-2">{book.title}</p>
            {book.authors.length > 0 && <p className="mt-1 truncate text-xs text-ink-3">{byline(book.authors)}</p>}
          </div>
        </div>
      )}
    </Squircle>
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

/** What's under a series' covers: the book it's at, or how much of it's been read. */
export function seriesCaption(s: Extract<Shelf<BookCard>, { kind: 'series' }>): string {
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
    <div className="absolute inset-x-2.5 bottom-2.5 h-1 overflow-hidden rounded-full bg-media-shade/50">
      <div className="h-full bg-media-ink" style={{ width: `${Math.max(4, r.progress * 100)}%` } as CSSProperties} />
    </div>
  )
}

export function BookTile({ book, name, label }: { book: BookCard; name?: string; label?: string }) {
  const tilt = useTilt<HTMLDivElement>()
  return (
    <Link
      to="/book/$id"
      params={{ id: String(book.id) }}
      viewTransition
      onClick={(e) => morphFrom(e, 'cover')}
      className="group block min-w-0 outline-none"
      aria-label={book.title}
      {...tilt.handlers}
    >
      <div className="rounded-[15px] outline-offset-3 transition-transform duration-200 ease-out group-hover:-translate-y-0.5 group-focus-visible:-translate-y-0.5 group-focus-visible:outline-2 group-focus-visible:outline-glow/70">
        <div ref={tilt.ref} className="tilt relative">
          <BookCover book={book} size={180} morph />
          <div className="glare pointer-events-none" />
          <ReadingBar book={book} />
        </div>
      </div>
      <p className="mt-2 truncate text-[13px] font-medium text-ink">{name ?? book.title}</p>
      <p className="truncate text-xs text-ink-3 tabular">{label ?? caption(book)}</p>
    </Link>
  )
}

/** A series as one tile: the book it's at on top of the next two. */
export function SeriesTile({ library, shelf }: { library: string; shelf: Extract<Shelf<BookCard>, { kind: 'series' }> }) {
  const tilt = useTilt<HTMLDivElement>()
  const behind = shelf.books.filter((b) => b.id !== shelf.current.id).slice(0, 2)
  return (
    <Link
      to="/library/$name/series/$series"
      params={{ name: library, series: shelf.series }}
      viewTransition
      onClick={(e) => morphFrom(e, 'cover')}
      className="group block min-w-0 outline-none"
      aria-label={`${shelf.series}, ${shelf.books.length} books`}
      {...tilt.handlers}
    >
      <div className="rounded-[15px] outline-offset-3 transition-transform duration-200 ease-out group-hover:-translate-y-0.5 group-focus-visible:-translate-y-0.5 group-focus-visible:outline-2 group-focus-visible:outline-glow/70">
        <div ref={tilt.ref} className="tilt relative">
          {behind
            .map((b, i) => (
              <div
                key={b.id}
                className="absolute inset-0 origin-top transition-transform duration-200 ease-out"
                style={{ transform: `translateY(${-6 * (i + 1)}px) scale(${1 - 0.07 * (i + 1)})`, filter: `brightness(${0.7 - 0.2 * i})` }}
              >
                <BookCover book={b} size={180} />
              </div>
            ))
            .reverse()}
          <BookCover book={shelf.current} size={180} morph className="relative" />
          <div className="glare pointer-events-none" />
          <ReadingBar book={shelf.current} />
        </div>
      </div>
      <p className="mt-2 truncate text-[13px] font-medium text-ink">{shelf.series}</p>
      <p className="truncate text-xs text-ink-3 tabular">{seriesCaption(shelf)}</p>
    </Link>
  )
}

export function BookGrid({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-x-5 gap-y-7">{children}</div>
}
