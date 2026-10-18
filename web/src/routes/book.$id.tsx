// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { BookOpen, Check, Download, ExternalLink, Highlighter, RefreshCw, Star, Undo2 } from 'lucide-react'
import { type CSSProperties, useState } from 'react'
import { percent } from '@tinystream/shared/books'
import { language } from '@tinystream/shared/format'
import { type BookCard, BooksQuery, RefreshBook, SetBookFinished, bookQuery, byline, contentQuery, patchReading, volume } from '../books/api'
import { BookCover, BookGrid, BookTile } from '../books/components'
import { useQuotes } from '../books/quotes'
import { Empty, Page, Section } from '../components/Page'
import { TitleSkeleton } from '../components/Skeleton'
import { toast, toastError } from '../components/feedback'
import { Button, Progress } from '../components/ui'
import { request } from '../lib/api'
import { useMe } from '../lib/hooks'
import { useTitle } from '../lib/title'

export const Route = createFileRoute('/book/$id')({ component: BookPage })

const formats = { EPUB: 'EPUB', FB2: 'FictionBook', TXT: 'Plain text' }
const providers: Record<string, string> = { 'apple-books': 'Apple Books', 'open-library': 'Open Library' }

function size(bytes: number) {
  return bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} KB`
}

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

function BookPage() {
  const id = Number(Route.useParams().id)
  const qc = useQueryClient()
  const me = useMe()
  const { data: book, isPending } = useQuery(bookQuery(id))
  const { data: library } = useQuery({
    queryKey: ['books', 'library', book?.library],
    queryFn: async () => (await request(BooksQuery, { library: book!.library })).books,
    enabled: !!book,
  })
  const [more, setMore] = useState(false)
  useTitle(book?.title)

  const finish = useMutation({
    mutationFn: (finished: boolean) => request(SetBookFinished, { id, finished }),
    onSuccess: (r, finished) => {
      patchReading(qc, id, r.setBookFinished.reading ?? null)
      void qc.invalidateQueries({ queryKey: ['books', 'home'] })
      toast({ title: finished ? `Marked ${book?.title} read` : `Marked ${book?.title} not started` })
    },
    onError: toastError,
  })
  const refresh = useMutation({
    mutationFn: () => request(RefreshBook, { id }),
    onSuccess: () => toast({ title: 'Looking it up again', body: 'Its details will change here once something turns up.' }),
    onError: toastError,
  })

  if (isPending) return <TitleSkeleton />
  if (!book) return <Page><Empty title="This book isn't here anymore" /></Page>

  const tint = book.coverTint
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
  const long = (book.description?.length ?? 0) > 700
  // The text is most of the wait to start reading: fetched when someone's about to.
  const prefetch = () => void qc.prefetchQuery(contentQuery(book.content))

  return (
    <div className="relative">
      <div
        className="pointer-events-none absolute inset-x-0 top-0 h-[40rem] transition-[background] duration-700"
        style={{ background: tint ? `radial-gradient(70% 60% at 15% 0%, rgb(${tint} / 0.24), transparent 70%)` : undefined } as CSSProperties}
      />
      <Page>
        <div className="relative flex flex-col gap-8 pt-2 sm:flex-row sm:items-end md:pt-10">
          <div className="lift w-44 shrink-0 sm:w-52" style={{ '--lift-radius': '16px' } as CSSProperties}>
            <BookCover book={book} size={420} className="w-full [view-transition-name:cover]" />
          </div>
          <div className="min-w-0 flex-1 pb-1">
            {series && <p className="text-xs font-medium tracking-wide text-ink-3 uppercase">{series}</p>}
            <h1 className="mt-1.5 text-[34px] leading-[1.1] font-semibold tracking-[-0.025em] text-balance md:text-[44px]">{book.title}</h1>
            <p className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-ink-2">
              {book.authors.length > 0 && <span className="font-medium text-ink">{byline(book.authors)}</span>}
              {facts.map((f, i) => (
                <span key={i} className="tabular">
                  {f}
                </span>
              ))}
              {book.rating != null && (
                <span className="flex items-center gap-1 tabular">
                  <Star className="size-3.5 fill-current text-warn" />
                  {(book.rating / 2).toFixed(1)}
                </span>
              )}
            </p>
            {started && (
              <div className="mt-5 flex max-w-sm items-center gap-3">
                <Progress value={r.progress} className="flex-1" />
                <span className="text-xs text-ink-2 tabular">{percent(r.position, book.length)}%</span>
              </div>
            )}
            <div className="mt-6 flex flex-wrap items-center gap-2">
              <Link to="/read/$id" params={{ id: String(book.id) }} onPointerEnter={prefetch} onFocus={prefetch}>
                <Button variant="primary" size="lg">
                  <BookOpen className="size-4.5" />
                  {started ? 'Continue reading' : r?.finished ? 'Read again' : 'Read'}
                </Button>
              </Link>
              <Button size="lg" onClick={() => finish.mutate(!r?.finished)} disabled={finish.isPending}>
                {r?.finished ? <Undo2 className="size-4.5" /> : <Check className="size-4.5" />}
                {r?.finished ? 'Mark not started' : 'Mark read'}
              </Button>
              <a href={book.file} download>
                <Button size="lg" variant="plain">
                  <Download className="size-4.5" /> Download
                </Button>
              </a>
              {me?.permissions.editMetadata && (
                <Button size="lg" variant="plain" onClick={() => refresh.mutate()} disabled={refresh.isPending}>
                  <RefreshCw className="size-4.5" /> Refresh details
                </Button>
              )}
              <QuotesLink book={book.id} />
            </div>
          </div>
        </div>

        {book.description && (
          <div className="mt-10 max-w-[44rem]">
            <p className={`text-[15px] leading-relaxed whitespace-pre-line text-ink-2 ${long && !more ? 'line-clamp-6' : ''}`}>{book.description}</p>
            {long && (
              <button onClick={() => setMore((m) => !m)} className="mt-2 text-[13px] text-ink-3 transition-colors hover:text-ink">
                {more ? 'Less' : 'More'}
              </button>
            )}
          </div>
        )}

        {book.genres.length > 0 && (
          <div className="mt-6 flex flex-wrap gap-1.5">
            {book.genres.map((g) => (
              <span key={g} className="rounded-md bg-panel px-2 py-0.5 text-xs text-ink-2">
                {g}
              </span>
            ))}
          </div>
        )}

        <p className="mt-8 text-xs leading-relaxed text-ink-3">
          {[
            book.publisher,
            book.published && `Published ${book.published}`,
            book.language && language(book.language),
            book.isbn && `ISBN ${book.isbn}`,
            size(book.size),
          ]
            .filter(Boolean)
            .join(' · ')}
          {book.provider && book.providerUrl && (
            <>
              {' · '}
              <a href={book.providerUrl} target="_blank" rel="noreferrer noopener" className="inline-flex items-center gap-1 hover:text-ink">
                Details from {providers[book.provider] ?? book.provider} <ExternalLink className="size-3" />
              </a>
            </>
          )}
        </p>

        {others && (
          <div className="mt-12">
            <Section title={others.title}>
              <BookGrid>
                {others.books.slice(0, 12).map((b) => (
                  <BookTile key={b.id} book={b} />
                ))}
              </BookGrid>
            </Section>
          </div>
        )}
      </Page>
    </div>
  )
}

/** How many quotes you've taken from it, to see them in Clips. */
function QuotesLink({ book }: { book: number }) {
  const mine = useQuotes(book).filter((m) => m.quote.mine).length
  if (mine === 0) return null
  return (
    <Link to="/clips" search={{ tab: 'mine', book }}>
      <Button size="lg" variant="plain" tabIndex={-1}>
        <Highlighter className="size-4.5" /> {mine === 1 ? '1 quote' : `${mine} quotes`}
      </Button>
    </Link>
  )
}
