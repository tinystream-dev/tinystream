// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { BookOpen } from 'lucide-react'
import { type CSSProperties, useMemo } from 'react'
import { currentOf, inOrder, percent } from '@tinystream/shared/books'
import { BooksQuery, bookQuery, byline, contentQuery, volume } from '../books/api'
import { BookCover, BookGrid, BookTile, progress } from '../books/components'
import { Empty, Page, Section } from '../components/Page'
import { TitleSkeleton } from '../components/Skeleton'
import { Button, Progress } from '../components/ui'
import { request } from '../lib/api'
import { useTitle } from '../lib/title'

export const Route = createFileRoute('/library_/$name/series/$series')({ component: SeriesPage })

function SeriesPage() {
  const { name, series } = Route.useParams()
  const qc = useQueryClient()
  const { data } = useQuery({ queryKey: ['books', 'library', name], queryFn: async () => (await request(BooksQuery, { library: name })).books })
  const books = useMemo(() => inOrder((data ?? []).filter((b) => b.series?.toLowerCase() === series.toLowerCase())), [data, series])
  useTitle(series)

  if (!data) return <TitleSkeleton />
  if (!books.length) return <Page><Empty title="This series isn't here anymore" /></Page>

  const current = currentOf(books)
  const r = current.reading
  const started = !!r && !r.finished && r.position > 0
  const read = books.filter((b) => b.reading?.finished).length
  const authors = [...new Set(books.flatMap((b) => b.authors))]
  const which = volume(current.seriesIndex)
  // The text is most of the wait to start reading: fetched when someone's about to.
  const prefetch = async () => {
    const book = await qc.fetchQuery(bookQuery(current.id))
    if (book) void qc.prefetchQuery(contentQuery(book.content))
  }

  return (
    <Page>
      <div className="relative flex flex-col gap-8 pt-2 sm:flex-row sm:items-end md:pt-10">
        <Link to="/book/$id" params={{ id: String(current.id) }} className="lift w-36 shrink-0 sm:w-44" style={{ '--lift-radius': '14px' } as CSSProperties}>
          <BookCover book={current} size={360} className="w-full [view-transition-name:cover]" />
        </Link>
        <div className="min-w-0 flex-1 pb-1">
          <p className="text-xs font-medium tracking-wide text-ink-3 uppercase">Series</p>
          <h1 className="mt-1.5 text-[34px] leading-[1.1] font-semibold tracking-[-0.025em] text-balance md:text-[44px]">{books[0].series}</h1>
          <p className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-ink-2">
            {authors.length > 0 && <span className="font-medium text-ink">{byline(authors)}</span>}
            <span className="tabular">{books.length} books</span>
            {read > 0 && <span className="tabular">{read === books.length ? 'All read' : `${read} read`}</span>}
          </p>
          {read > 0 && read < books.length && (
            <div className="mt-5 flex max-w-sm items-center gap-3">
              <Progress value={read / books.length} className="flex-1" />
              <span className="text-xs text-ink-2 tabular">{percent(read, books.length)}%</span>
            </div>
          )}
          <div className="mt-6 flex flex-wrap items-center gap-2">
            <Link to="/read/$id" params={{ id: String(current.id) }} onPointerEnter={prefetch} onFocus={prefetch}>
              <Button variant="primary" size="lg">
                <BookOpen className="size-4.5" />
                {[started ? 'Continue' : read === books.length ? 'Read again' : 'Read', which].filter(Boolean).join(' ')}
              </Button>
            </Link>
          </div>
        </div>
      </div>

      <div className="mt-12">
        <Section title="Books">
          <BookGrid>
            {books.map((b) => (
              <BookTile key={b.id} book={b} name={volume(b.seriesIndex) ?? b.title} label={progress(b) ?? 'Not started'} />
            ))}
          </BookGrid>
        </Section>
      </div>
    </Page>
  )
}
