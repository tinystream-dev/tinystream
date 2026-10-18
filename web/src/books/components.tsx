// SPDX-License-Identifier: AGPL-3.0-or-later

import { Link } from '@tanstack/react-router'
import { BookOpen } from 'lucide-react'
import { type CSSProperties, type ReactNode, useEffect, useState } from 'react'
import { percent } from '@tinystream/shared/books'
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

/** What's under a book's cover: how far in, or which in its series. */
export function caption(book: BookCard): string {
  const r = book.reading
  if (r?.finished) return 'Read'
  if (r && r.position > 0) return `${percent(r.position, book.length)}% read`
  return [volume(book.seriesIndex), byline(book.authors)].filter(Boolean).join(' · ') || 'Not started'
}

export function BookTile({ book, label }: { book: BookCard; label?: string }) {
  const tilt = useTilt<HTMLDivElement>()
  const r = book.reading
  const reading = r && !r.finished && r.position > 0
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
          {reading && (
            <div className="absolute inset-x-2.5 bottom-2.5 h-1 overflow-hidden rounded-full bg-media-shade/50">
              <div className="h-full bg-media-ink" style={{ width: `${Math.max(4, r.progress * 100)}%` } as CSSProperties} />
            </div>
          )}
        </div>
      </div>
      <p className="mt-2 truncate text-[13px] font-medium text-ink">{book.title}</p>
      <p className="truncate text-xs text-ink-3 tabular">{label ?? caption(book)}</p>
    </Link>
  )
}

export function BookGrid({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-x-5 gap-y-7">{children}</div>
}
