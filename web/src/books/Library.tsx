// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { shelve } from '@tinystream/shared/books'
import { Empty, Page, PageTitle } from '../components/Page'
import { PosterGridSkeleton } from '../components/Skeleton'
import { Chip } from '../components/ui'
import { request } from '../lib/api'
import { type BookCard, BooksQuery } from './api'
import { BookGrid, BookTile, SeriesTile } from './components'

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

export function BookLibrary({ name }: { name: string }) {
  const { data } = useQuery({ queryKey: ['books', 'library', name], queryFn: async () => (await request(BooksQuery, { library: name })).books })
  const [filter, setFilter] = useState<Filter>('all')
  const [sort, setSort] = useState<Sort>('title')

  const shelves = useMemo(() => {
    const list = (data ?? []).filter((b) => matches(b, filter))
    // The server's order already keeps a series' books together.
    if (sort === 'author') list.sort((a, b) => surname(a).localeCompare(surname(b)))
    if (sort === 'added') list.sort((a, b) => b.addedAt - a.addedAt)
    return shelve(list)
  }, [data, filter, sort])

  return (
    <Page>
      <PageTitle aside={data && <span className="pb-1 text-sm text-ink-3 tabular">{data.length} book{data.length === 1 ? '' : 's'}</span>}>{name}</PageTitle>
      <div className="mb-7 flex flex-wrap items-center gap-1">
        {filters.map((f) => (
          <Chip key={f.value} active={filter === f.value} onClick={() => setFilter(f.value)}>
            {f.label}
          </Chip>
        ))}
        <div className="flex-1" />
        <Chip active={sort === 'title'} onClick={() => setSort('title')}>
          A–Z
        </Chip>
        <Chip active={sort === 'author'} onClick={() => setSort('author')}>
          Author
        </Chip>
        <Chip active={sort === 'added'} onClick={() => setSort('added')}>
          Newest
        </Chip>
      </div>
      {data && shelves.length === 0 && <Empty title={data.length ? 'Nothing matches this filter' : 'This library is empty'} />}
      {data ? (
        <BookGrid>
          {shelves.map((s) => (s.kind === 'book' ? <BookTile key={s.book.id} book={s.book} /> : <SeriesTile key={`series:${s.series}`} library={name} shelf={s} />))}
        </BookGrid>
      ) : (
        <PosterGridSkeleton />
      )}
    </Page>
  )
}
