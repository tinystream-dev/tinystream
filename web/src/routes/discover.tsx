// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Search } from 'lucide-react'
import { useEffect, useState } from 'react'
import { mediaOptions, ofMediaType } from '@tinystream/shared/media'
import { DiscoverShelf, PickDialog, ResultCard, useAction } from '../components/DiscoverCard'
import { Empty, Page, PageTitle } from '../components/Page'
import { PosterGrid } from '../components/PosterGrid'
import { PosterGridSkeleton, RowSkeleton } from '../components/Skeleton'
import { Squircle } from '../components/Squircle'
import { Select, Spinner } from '../components/ui'
import { graphql } from '../gql'
import { type DiscoverResult, librariesQuery, request } from '../lib/api'
import { useMe } from '../lib/hooks'
import { useTitle } from '../lib/title'

export const Route = createFileRoute('/discover')({
  validateSearch: (s: Record<string, unknown>): { q?: string } => ({ q: typeof s.q === 'string' ? s.q : undefined }),
  component: DiscoverPage,
})

const DiscoverQuery = graphql(`
  query Discover($library: String, $query: String) {
    discover(library: $library, query: $query) {
      library
      results {
        ...DiscoverResultFields
      }
    }
  }
`)

const ForYouQuery = graphql(`
  query ForYou($library: String) {
    forYou(library: $library) {
      library
      shelves {
        key
        name
        results {
          ...DiscoverResultFields
        }
      }
    }
  }
`)

function DiscoverPage() {
  const me = useMe()
  const { q: initial } = Route.useSearch()
  const navigate = useNavigate()
  const [q, setQ] = useState(initial ?? '')
  const [query, setQuery] = useState(initial ?? '')
  const [library, setLibrary] = useState<string>('')
  const [category, setCategory] = useState('')
  const [picked, setPicked] = useState<DiscoverResult | null>(null)
  const { data: libraries } = useQuery(librariesQuery)
  const manage = !!me?.permissions.manageShows
  const action = useAction()
  useTitle(manage ? 'Add shows' : 'Request shows')

  useEffect(() => {
    const t = setTimeout(() => {
      setQuery(q.trim())
      void navigate({ to: '/discover', search: { q: q.trim() || undefined }, replace: true })
    }, 350)
    return () => clearTimeout(t)
  }, [q, navigate])

  const { data, isFetching, error } = useQuery({
    queryKey: ['discover', query, library],
    queryFn: async () => (await request(DiscoverQuery, { query, library: library || null })).discover,
    // Nothing typed shows what's popular; one letter isn't worth a search yet.
    enabled: query.length !== 1 && (manage || !!me?.permissions.request),
    placeholderData: (p) => p,
  })
  const forYou = useQuery({
    queryKey: ['discover', 'for-you', library],
    queryFn: async () => (await request(ForYouQuery, { library: library || null })).forYou,
    enabled: !query && !!action,
    placeholderData: (p) => p,
    staleTime: 5 * 60_000,
  })
  const admin = manage

  return (
    <Page>
      <PageTitle>{admin ? 'Add shows' : 'Request shows'}</PageTitle>
      <div className="mb-8 flex flex-col gap-2 sm:flex-row sm:items-center">
        <Squircle radius={14} edge className="flex h-12 min-w-0 flex-none items-center gap-3 bg-raised px-4 sm:flex-1">
          <Search className="size-4.5 shrink-0 text-ink-3" />
          <input
            autoFocus
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search AniList or TMDB"
            className="h-full min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-ink-3"
          />
          {isFetching && <Spinner className="size-4 shrink-0 text-ink-3" />}
        </Squircle>
        {libraries && libraries.length > 1 && (
          <div className="sm:w-48">
            <Select
              value={library || data?.library || ''}
              options={libraries.map((l) => ({ value: l.name, label: l.name }))}
              onChange={setLibrary}
            />
          </div>
        )}
        <div className="sm:w-48">
          <Select value={category} options={mediaOptions} onChange={setCategory} />
        </div>
      </div>

      {me && !manage && !me.permissions.request && (
        <Empty title="You can't request shows" />
      )}
      {error && <p className="text-sm text-danger">{(error as Error).message}</p>}
      {!query && action && (forYou.data ? forYou.data.shelves.filter((row) => ofMediaType(row.results, category).length > 0).map((row) => <DiscoverShelf key={row.key} title={row.name} results={ofMediaType(row.results, category)} />) : forYou.isPending && <RowSkeleton />)}
      {!query && data && ofMediaType(data.results, category).length > 0 && (
        <h2 className="mb-4 text-[15px] font-semibold tracking-tight">Popular right now</h2>
      )}
      {!data && !error && <PosterGridSkeleton count={12} />}
      {data && ofMediaType(data.results, category).length === 0 && <Empty title={query ? `Nothing called “${query}”${category ? ' of this type' : ''}` : 'No titles of this type'} />}

      {data && (
        <PosterGrid>
          {ofMediaType(data.results, category).map((r) => (
            <ResultCard key={`${r.provider}${r.category}${r.id}`} r={r} action={action} onPick={() => setPicked(r)} />
          ))}
        </PosterGrid>
      )}

      {picked && <PickDialog r={picked} action={action} onClose={() => setPicked(null)} />}
    </Page>
  )
}
