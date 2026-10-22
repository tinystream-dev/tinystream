// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Check, CheckCheck, CircleCheck, Play, RefreshCw, RotateCcw, Search, Undo2, Wand2 } from 'lucide-react'
import { type CSSProperties, useEffect, useRef, useState } from 'react'
import { remaining, runtime } from '@tinystream/shared/format'
import { DiscoverShelf } from '../components/DiscoverCard'
import { ReleaseDialog, StateBadge, useLookForAgain } from '../components/downloads'
import { NextEpisode, SeriesPanel, useItemSeries } from '../components/SeriesPanel'
import { toast, toastError } from '../components/feedback'
import { Empty, Page, Section } from '../components/Page'
import { Img } from '../components/Img'
import { ArtworkEditor } from '../components/ArtworkEditor'
import { Overview } from '../components/Overview'
import { Poster, morphFrom } from '../components/Poster'
import { Row } from '../components/Row'
import { TitleSkeleton } from '../components/Skeleton'
import { Squircle } from '../components/Squircle'
import { Button, IconButton, Input, Panel, Spinner, Tip } from '../components/ui'
import { graphql } from '../gql'
import { type Download, type Episode, type Item, type Provider, type SeriesEpisode, request } from '../lib/api'
import { airs, duration, shortDate, speed, useFeatures } from '../lib/downloads'
import { useMe } from '../lib/hooks'
import { remember } from '../lib/recents'
import { preload, useFetching } from '../lib/refreshing'
import { awaitTint, tintPending, titleTint } from '../lib/tint'
import { useTitle } from '../lib/title'
import { queryClient } from '../router'

const TitleQuery = graphql(`
  query Title($id: Int!) {
    title(id: $id) {
      ...TitleDetail
    }
  }
`)

const SimilarQuery = graphql(`
  query Similar($id: Int!) {
    title(id: $id) {
      similar {
        recommendations {
          ...DiscoverResultFields
        }
        alsoWatched {
          ...Card
        }
      }
    }
  }
`)

const MatchCandidatesQuery = graphql(`
  query MatchCandidates($id: Int!, $query: String, $provider: Provider) {
    title(id: $id) {
      matchCandidates(query: $query, provider: $provider) {
        query
        results {
          provider
          id
          name
          year
          poster
          overview
        }
      }
    }
  }
`)

const DownloadsQuery = graphql(`
  query TitleDownloads {
    downloads {
      ...DownloadFields
    }
  }
`)

const SetWatched = graphql(`
  mutation SetWatched($videoIds: [Int!]!, $watched: Boolean!) {
    setWatched(videoIds: $videoIds, watched: $watched) {
      id
    }
  }
`)

const SetTitleWatched = graphql(`
  mutation SetTitleWatched($id: Int!, $watched: Boolean!) {
    setTitleWatched(id: $id, watched: $watched) {
      id
    }
  }
`)

const RefreshTitle = graphql(`
  mutation RefreshTitle($id: Int!) {
    refreshTitle(id: $id) {
      id
    }
  }
`)

const MatchTitle = graphql(`
  mutation MatchTitle($id: Int!, $provider: Provider!, $providerId: String!) {
    matchTitle(id: $id, provider: $provider, providerId: $providerId) {
      id
    }
  }
`)

/** A title's details, cached under ['item', id]; null when it's gone. */
export const titleQuery = (id: number) => ({
  queryKey: ['item', id],
  queryFn: async (): Promise<Item | null> => (await request(TitleQuery, { id })).title,
  refetchInterval: awaitTint<Item | null>(tintPending),
})

export const Route = createFileRoute('/title/$id')({
  // The navigation waits (briefly) for the details and the poster, so the
  // artwork that was clicked has a poster to morph into instead of a skeleton.
  loader: ({ params }) => {
    if (typeof window === 'undefined') return
    const ready = queryClient
      .ensureQueryData(titleQuery(Number(params.id)))
      .then((item) => preload([item?.poster], 1200))
      .catch(() => {})
    return Promise.race([ready, new Promise<void>((r) => setTimeout(r, 1500))])
  },
  component: TitlePage,
})

function TitlePage() {
  const { id } = Route.useParams()
  const me = useMe()
  const qc = useQueryClient()
  const { data: item, error } = useQuery(titleQuery(Number(id)))
  const [season, setSeason] = useState<number | null>(null)
  const [matching, setMatching] = useState(false)
  const [backdropBroken, setBackdropBroken] = useState(false)
  useEffect(() => setBackdropBroken(false), [item?.backdrop])
  const features = useFeatures()
  const { data: series } = useItemSeries(Number(id), !!features && item?.kind === 'SHOW', !!me?.permissions.manageShows)
  const [searchEpisode, setSearchEpisode] = useState<SeriesEpisode | null>(null)
  const lookAgain = useLookForAgain()
  const fetching = useFetching(Number(id), item?.matchState === 'PENDING' && !!item.libraryProvider)

  useEffect(() => {
    if (!item || season !== null) return
    const regular = item.seasons.filter((s) => s.number > 0)
    // Caught up: nextUp is only a rewatch from the start, so open where the show is at.
    if (regular.length && item.seasons.every((s) => s.episodes.every((e) => e.finished))) {
      setSeason(regular[regular.length - 1].number)
      return
    }
    const next = item.nextUp && item.seasons.find((s) => s.episodes.some((e) => e.id === item.nextUp!.video.id))
    setSeason(next?.number ?? regular[0]?.number ?? item.seasons[0]?.number ?? null)
  }, [item, season])

  const key = ['item', Number(id)]
  const settle = () => {
    void qc.invalidateQueries({ queryKey: key })
    void qc.invalidateQueries({ queryKey: ['home'] })
    void qc.invalidateQueries({ queryKey: ['library'] })
  }
  /** Marks episodes (by id) watched or not, showing it straight away. */
  const setEpisodes = useMutation({
    mutationFn: ({ ids, watched }: { ids: number[]; watched: boolean }) =>
      request(SetWatched, { videoIds: ids, watched }),
    onMutate: ({ ids, watched }) => {
      const before = qc.getQueryData<Item>(key)
      if (before)
        qc.setQueryData<Item>(key, {
          ...before,
          seasons: before.seasons.map((s) => ({
            ...s,
            episodes: s.episodes.map((e) => (ids.includes(e.id) ? { ...e, finished: watched, position: null } : e)),
          })),
        })
      return { before }
    },
    onError: (e, _, ctx) => {
      if (ctx?.before) qc.setQueryData(key, ctx.before)
      toastError(e)
    },
    onSettled: settle,
  })
  const watchAll = useMutation({
    mutationFn: (w: boolean) => request(SetTitleWatched, { id: Number(id), watched: w }),
    onError: toastError,
    onSettled: settle,
  })
  const refresh = useMutation({ mutationFn: () => request(RefreshTitle, { id: Number(id) }), onError: toastError })
  useTitle(item?.name)
  useEffect(() => {
    if (item) remember({ id: item.id, title: item.name, poster: item.poster, kind: item.kind === 'SHOW' ? 'show' : 'movie' })
  }, [item])
  const ambient = titleTint(item)
  const can = me?.permissions
  const { data: downloads } = useQuery({
    queryKey: ['downloads'],
    queryFn: async () => (await request(DownloadsQuery)).downloads,
    enabled: !!can?.downloads && !!series && 'counts' in series && series.counts.grabbed > 0,
    refetchInterval: 2000,
  })

  if (error || item === null) return <Page><Empty title="This title isn't here anymore" /></Page>
  if (!item) return <TitleSkeleton />

  const all = item.seasons.flatMap((s) => s.episodes)
  const watchedFrom = (episode: Episode) => all
    .filter((e) => e.finished && (
      (e.season ?? 0) > (episode.season ?? 0) ||
      ((e.season ?? 0) === (episode.season ?? 0) && (e.episode ?? 0) >= (episode.episode ?? 0))
    ))
    .map((e) => e.id)
  const allWatched = all.length > 0 && all.every((e) => e.finished)
  const current = item.seasons.find((s) => s.number === season)
  const extras = current?.episodes.filter((e) => e.episode === null && e.season !== 0) ?? []
  const regular = current?.episodes.filter((e) => !extras.includes(e)) ?? []
  // Episodes the schedule knows about but the library doesn't have (yet).
  const scheduled = (series && 'episodes' in series ? series.episodes : undefined) ?? []
  const ghosts = scheduled.filter((e) => e.season === season && !e.video && (e.aired || e.airAt))
  const extraSeasons = [...new Set(scheduled.filter((e) => !e.video && (e.aired || e.airAt)).map((e) => e.season))].filter(
    (n) => !item.seasons.some((s) => s.number === n),
  )
  // Episodes per season that the schedule says haven't aired yet.
  const unaired = new Map<number, number>()
  for (const e of scheduled)
    if (!e.video && !e.aired && e.airAt && e.airAt * 1000 > Date.now()) unaired.set(e.season, (unaired.get(e.season) ?? 0) + 1)
  const seasonTabs = [
    ...item.seasons.map((s) => ({ number: s.number, name: s.name, done: s.episodes.every((e) => e.finished), upcoming: unaired.get(s.number) ?? 0 })),
    ...extraSeasons.map((n) => ({ number: n, name: n === 0 ? (scheduled.filter((e) => e.season === 0).length === 1 ? 'Special' : 'Specials') : `Season ${n}`, done: false, upcoming: unaired.get(n) ?? 0 })),
  ].sort((a, b) => (a.number === 0 ? 1 : b.number === 0 ? -1 : a.number - b.number))
  const totalRuntime = item.kind === 'MOVIE' ? runtime(item.movie?.duration) : null
  const next = item.nextUp
  const meta = [
    item.year,
    item.kind === 'SHOW'
      ? `${item.seasons.filter((s) => s.number > 0).length || item.seasons.length} season${item.seasons.length === 1 ? '' : 's'}`
      : totalRuntime,
    item.rating ? `${item.rating.toFixed(1)} rating` : null,
  ].filter(Boolean)

  return (
    <div className="relative" data-fetching={fetching || undefined}>
      {/* The backdrop bleeds behind the page and fades into it. */}
      {item.backdrop && !backdropBroken && (
        <div className="pointer-events-none absolute inset-x-0 top-0 h-[34rem] overflow-hidden">
          <div className="refreshable size-full" style={refreshing(0, 28, 1.06)}>
            <div className="parallax size-full">
              <img
                key={item.backdrop}
                src={item.backdrop}
                alt=""
                onError={() => setBackdropBroken(true)}
                className="size-full animate-[fade_900ms_ease-out] object-cover opacity-45 [mask-image:linear-gradient(to_bottom,black_20%,transparent)]"
              />
            </div>
          </div>
          <div className="absolute inset-0 bg-linear-to-r from-canvas/90 via-canvas/40 to-transparent" />
        </div>
      )}
      <div
        className="ambient pointer-events-none absolute inset-x-0 top-0 h-[44rem]"
        style={{
          '--ambient': ambient ? `rgb(${ambient} / 0.22)` : 'transparent',
          background: 'radial-gradient(70% 60% at 15% 0%, var(--ambient), transparent 70%)',
        } as CSSProperties}
      />

      <TitleBar item={item} />

      <div className="relative">
      <Page>
        <div className="relative flex flex-col gap-8 pt-6 sm:flex-row sm:items-end md:pt-20">
          <div className="lift w-40 shrink-0 sm:w-52">
            <Squircle radius={18} edge className="aspect-[2/3] bg-panel" style={{ viewTransitionName: 'poster' }}>
              <div className="refreshable size-full" style={refreshing(0, 16, 1.08)}>
                {item.poster && <img src={item.poster} alt="" className="size-full object-cover" />}
              </div>
              <div className="sheen" />
            </Squircle>
          </div>
          <div className="min-w-0 flex-1 pb-1">
            <h1 style={refreshing(1, 14)} className="refreshable text-[34px] leading-[1.1] font-semibold tracking-[-0.025em] text-balance md:text-[44px]">
              <Resolve key={item.id} text={item.name} hold={fetching} />
            </h1>
            <p style={refreshing(2)} className="refreshable mt-2.5 flex flex-wrap gap-x-4 gap-y-1 text-sm text-ink-2">
              {meta.map((m) => (
                <span key={String(m)}>{m}</span>
              ))}
              {item.genres.length > 0 && <span className="text-ink-3">{item.genres.slice(0, 4).join(', ')}</span>}
            </p>
            {series?.next && <NextEpisode series={series} />}
            <div className="mt-6 flex flex-wrap items-center gap-2">
              {next && (
                <Link to="/watch/$id" params={{ id: String(next.video.id) }}>
                  <Button variant="primary" size="lg">
                    <Play className="size-4.5 fill-current" />
                    {next.resuming ? 'Resume' : allWatched ? 'Watch again' : 'Play'}
                    {item.kind === 'SHOW' && next.video.label && <span className="font-normal opacity-60">{next.video.label}</span>}
                    {next.resuming && (
                      <span className="font-normal opacity-60">{remaining(next.video.position, next.video.duration)}</span>
                    )}
                  </Button>
                </Link>
              )}
              <Button
                size="lg"
                onClick={() => {
                  // Remember exactly what was watched, so undo can put it back.
                  const was = all.filter((e) => e.finished).map((e) => e.id)
                  const target = !allWatched
                  watchAll.mutate(target, {
                    onSuccess: () =>
                      toast({
                        title: target ? `Marked ${item.name} watched` : `Marked ${item.name} not watched`,
                        image: item.poster,
                        action: {
                          label: 'Undo',
                          run: async () => {
                            try {
                              await request(SetTitleWatched, { id: item.id, watched: false })
                              if (was.length) await request(SetWatched, { videoIds: was, watched: true })
                            } catch (e) {
                              toastError(e)
                            }
                            settle()
                          },
                        },
                      }),
                  })
                }}
              >
                {allWatched ? <CircleCheck className="size-4.5" /> : <Check className="size-4.5" />}
                {allWatched ? 'Watched' : 'Mark watched'}
              </Button>
              {can?.editMetadata && (
                <>
                  <IconButton label="Fix match" className="size-11" onClick={() => setMatching(true)}>
                    <Wand2 className="size-4.5" />
                  </IconButton>
                  <IconButton
                    label="Refresh details"
                    className="size-11"
                    onClick={() => refresh.mutate()}
                    disabled={refresh.isPending}
                  >
                    <RefreshCw className={`size-4.5 ${refresh.isPending || fetching ? 'animate-spin' : ''}`} />
                  </IconButton>
                </>
              )}
            </div>
          </div>
        </div>

        {can?.editMetadata && (
          <div className="mt-5 flex flex-wrap gap-3">
            <ArtworkEditor target={{ titleId: item.id, kind: 'POSTER' }} custom={item.customPoster} label="thumbnail" />
            <ArtworkEditor target={{ titleId: item.id, kind: 'BACKDROP' }} custom={item.customBackdrop} label="banner" />
            {item.movie && <ArtworkEditor target={{ videoId: item.movie.id }} custom={item.movie.customStill} label="video thumbnail" />}
          </div>
        )}
        {item.overview && <Overview text={item.overview} style={refreshing(3)} />}
        {can?.manageShows && features && item.kind === 'SHOW' && series !== undefined && (
          <SeriesPanel item={item} series={series && 'counts' in series ? series : null} season={season} />
        )}
        {can?.editMetadata && item.matchState === 'UNMATCHED' && (
          <p className="mt-4 text-sm text-ink-3">
            No {item.libraryProvider === 'TMDB' ? 'TMDB' : 'AniList'} match was found for this folder.{' '}
            <button className="text-ink-2 underline underline-offset-4 hover:text-ink" onClick={() => setMatching(true)}>
              Pick one
            </button>
          </p>
        )}

        {item.kind === 'SHOW' && (
          <div className="mt-12">
            {seasonTabs.length > 1 && (
              <div style={refreshing(4, 6)} className="refreshable mb-5 flex flex-wrap gap-1">
                {seasonTabs.map((s) => {
                  const done = s.done && !s.upcoming
                  return (
                    <Squircle
                      key={s.number}
                      as="button"
                      radius={9}
                      onClick={() => setSeason(s.number)}
                      className={`flex h-8 items-center gap-1.5 px-3 text-sm transition-colors ${season === s.number ? 'bg-press text-ink' : 'text-ink-2 hover:bg-hover hover:text-ink'}`}
                    >
                      {s.name}
                      {done && <Check className="size-3.5 text-ink-3" />}
                      {s.upcoming > 0 && (
                        <Tip
                          label={`${s.upcoming} episode${s.upcoming === 1 ? '' : 's'} yet to air`}
                          className="size-1.5 rounded-full bg-warn"
                        />
                      )}
                    </Squircle>
                  )
                })}
              </div>
            )}
            {season === 0 && seasonTabs.length === 1 && (
              <h2 className="mb-4 text-[15px] font-semibold">{current?.name ?? 'Specials'}</h2>
            )}
            {current?.title && current.title !== item.name && (
              <p style={refreshing(4)} className="refreshable mb-4 text-sm text-ink-3">{current.title}</p>
            )}
            <div className="-mx-3 space-y-0.5">
              {[
                ...regular.map((e, i, list) => ({ n: e.episode ?? Number.MAX_SAFE_INTEGER, row: (
                  <EpisodeRow
                    key={e.id}
                    e={e}
                    canEditArtwork={!!can?.editMetadata}
                    order={5 + i}
                    next={!allWatched && next?.video.id === e.id}
                    scrollTo={!allWatched && next?.video.id === e.id && i > 4}
                    onWatched={(w) => setEpisodes.mutate({ ids: [e.id], watched: w })}
                    onWatchedUpTo={
                      list.slice(0, i).some((x) => !x.finished) || !e.finished
                        ? () => {
                            const ids = list.slice(0, i + 1).filter((x) => !x.finished).map((x) => x.id)
                            setEpisodes.mutate(
                              { ids, watched: true },
                              {
                                onSuccess: () =>
                                  toast({
                                    title: `Marked ${ids.length} episode${ids.length === 1 ? '' : 's'} watched`,
                                    action: { label: 'Undo', run: () => setEpisodes.mutate({ ids, watched: false }) },
                                  }),
                              },
                            )
                          }
                        : undefined
                    }
                    onUnwatchedFrom={
                      watchedFrom(e).length > 0
                        ? () => {
                            const ids = watchedFrom(e)
                            setEpisodes.mutate(
                              { ids, watched: false },
                              {
                                onSuccess: () =>
                                  toast({
                                    title: `Marked ${ids.length} episode${ids.length === 1 ? '' : 's'} not watched`,
                                    action: { label: 'Undo', run: () => setEpisodes.mutate({ ids, watched: true }) },
                                  }),
                              },
                            )
                          }
                        : undefined
                    }
                  />
                ) })),
                ...ghosts
                  .filter((g) => !current?.episodes.some((e) => e.episode === g.episode))
                  .map((g) => ({ n: g.episode, row: (
                    <GhostRow
                      key={`g${g.episode}`}
                      e={g}
                      admin={!!can?.manageShows}
                      download={downloads?.find((d) => d.state === 'DOWNLOADING' && d.seriesId === series?.id && d.episodes.some((x) => x.season === g.season && x.episode === g.episode))}
                      onSearch={() => setSearchEpisode(g)}
                      onLookAgain={() => series && lookAgain.mutate({ seriesId: series.id, season: g.season, episode: g.episode })}
                    />
                  ) })),
              ]
                .sort((a, b) => a.n - b.n)
                .map((x) => x.row)}
            </div>
            {extras.length > 0 && (
              <>
                <h2 className="mt-8 mb-4 text-[15px] font-semibold">Extras</h2>
                <div className="-mx-3 space-y-0.5">
                  {extras.map((e, i) => (
                    <EpisodeRow key={e.id} e={e} order={5 + regular.length + i} next={next?.video.id === e.id}
                      canEditArtwork={!!can?.editMetadata} scrollTo={false} onWatched={(w) => setEpisodes.mutate({ ids: [e.id], watched: w })} />
                  ))}
                </div>
              </>
            )}
          </div>
        )}

        <MoreLikeThis item={item} actionable={!!features && item.kind === 'SHOW'} />

        {me?.isAdmin && item.path && <p className="mt-14 truncate text-xs text-ink-3">{item.path}</p>}
      </Page>
      </div>

      {matching && <MatchDialog item={item} onClose={() => setMatching(false)} />}
      {searchEpisode && series && (
        <ReleaseDialog
          seriesId={series.id}
          season={searchEpisode.season}
          episodes={[searchEpisode.episode]}
          title={item.name}
          onClose={() => setSearchEpisode(null)}
        />
      )}
    </div>
  )
}

/**
 * Styles something the next fetch replaces: `order` places it in the cascade
 * that sharpens it, `blur` and `scale` set how far it goes while fetching.
 */
function refreshing(order: number, blur?: number, scale?: number): CSSProperties {
  return {
    '--i': order,
    ...(blur !== undefined && { '--blur': `${blur}px` }),
    ...(scale !== undefined && { '--scale': scale }),
  } as CSSProperties
}

/**
 * Text that resolves a letter at a time when it changes. While `hold` is
 * set (the details are being fetched, and blurred) the old text stays, so
 * the new one resolves as they sharpen rather than unseen under the blur.
 */
function Resolve({ text, hold }: { text: string; hold: boolean }) {
  const [shown, setShown] = useState({ text, fresh: false })
  if (!hold && shown.text !== text) setShown({ text, fresh: true })
  if (!shown.fresh) return shown.text
  const total = [...shown.text.replace(/\s+/g, '')].length
  let n = 0
  return (
    <span role="text" aria-label={shown.text}>
      {shown.text.split(/(\s+)/).map((word, w) =>
        /^\s+$/.test(word) ? (
          word
        ) : (
          <span key={w} aria-hidden className="inline-block whitespace-nowrap">
            {[...word].map((c, i) => {
              const at = n++
              const last = at === total - 1
              return (
                <span
                  key={i}
                  className="inline-block animate-[resolve_800ms_cubic-bezier(.16,1,.3,1)_both]"
                  style={{ animationDelay: `${Math.min(at, 40) * 30}ms` }}
                  onAnimationEnd={last ? () => setShown({ text: shown.text, fresh: false }) : undefined}
                >
                  {c}
                </span>
              )
            })}
          </span>
        ),
      )}
    </span>
  )
}

/** Once the hero has scrolled away, a slim bar keeps the title in view. */
function TitleBar({ item }: { item: Item }) {
  return (
    <div className="sticky top-12 z-20 h-0 md:top-0">
      <div className="scrolled-in absolute inset-x-0 top-0 material-bar border-b border-line bg-canvas/80 backdrop-blur-xl">
        <button
          onClick={() => window.scrollTo({ top: 0, behavior: 'smooth' })}
          className="mx-auto flex h-14 w-full max-w-[1400px] items-center gap-3 px-5 text-left md:px-10"
        >
          <Squircle radius={6} className="aspect-[2/3] w-7 shrink-0 bg-panel">
            {item.poster && <img src={item.poster} alt="" className="size-full object-cover" />}
          </Squircle>
          <span className="truncate text-[15px] font-semibold tracking-tight">{item.name}</span>
          {item.year && <span className="shrink-0 text-sm text-ink-3">{item.year}</span>}
        </button>
      </div>
    </div>
  )
}

/** Shows like this one from the provider, and what else people here watched. */
function MoreLikeThis({ item, actionable }: { item: Item; actionable: boolean }) {
  const { data } = useQuery({
    queryKey: ['discover', 'similar', item.id],
    queryFn: async () => (await request(SimilarQuery, { id: item.id })).title?.similar ?? null,
    staleTime: 5 * 60_000,
  })
  if (!data || (data.recommendations.length === 0 && data.alsoWatched.length === 0)) return null
  return (
    <div className="mt-16">
      {data.recommendations.length > 0 && <DiscoverShelf title="More like this" results={data.recommendations} actionable={actionable} />}
      {data.alsoWatched.length > 0 && (
        <Section title="People here who watched this also watched">
          <Row>
            {data.alsoWatched.map((c) => (
              <div key={c.id} className="w-38 shrink-0 snap-start">
                <Poster card={c} />
              </div>
            ))}
          </Row>
        </Section>
      )}
    </div>
  )
}

function EpisodeRow({
  e,
  order,
  next,
  scrollTo,
  onWatched,
  canEditArtwork,
  onWatchedUpTo,
  onUnwatchedFrom,
}: {
  e: Episode
  canEditArtwork: boolean
  order: number
  next: boolean
  scrollTo: boolean
  onWatched: (w: boolean) => void
  onWatchedUpTo?: () => void
  onUnwatchedFrom?: () => void
}) {
  const [broken, setBroken] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => setBroken(false), [e.still])
  const progress = !e.finished && e.position && e.duration ? e.position / e.duration : 0
  // The check draws itself when it's marked here, not every time the page opens.
  const finishedBefore = useRef(e.finished)
  useEffect(() => {
    if (!e.finished) finishedBefore.current = false
  }, [e.finished])
  const drawCheck = e.finished && !finishedBefore.current
  // Long seasons: bring the episode you're up to into view.
  useEffect(() => {
    if (scrollTo) ref.current?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [scrollTo])
  return (
    <div ref={ref} style={refreshing(order)} className="refreshable group relative">
      <Link
        to="/watch/$id"
        params={{ id: String(e.id) }}
        viewTransition
        onClick={(ev) => morphFrom(ev, 'still')}
        className="block rounded-[15px] outline-none focus-visible:outline-2 focus-visible:outline-glow/70"
      >
        <Squircle radius={14} edge={next} className={`flex gap-4 p-3 transition-colors group-hover:bg-hover ${next ? 'bg-raised' : ''}`}>
          <Squircle radius={10} edge className="relative aspect-video w-36 shrink-0 bg-panel md:w-44" data-morph>
            {e.still && !broken ? (
              <Img src={e.still} loading="lazy" onError={() => setBroken(true)} className="size-full object-cover" />
            ) : (
              <div className="grid size-full place-items-center text-sm text-ink-3 tabular">{e.episode}</div>
            )}
            <div className="drain" data-on={e.finished || undefined} />
            <div className="absolute inset-0 grid place-items-center bg-media-shade/30 opacity-0 transition-opacity group-hover:opacity-100">
              <Play className="size-6 fill-media-ink text-media-ink" />
            </div>
            {progress > 0 && (
              <div className="absolute inset-x-0 bottom-0 h-[3px] bg-media-shade/50">
                <div className="h-full bg-media-ink" style={{ width: `${progress * 100}%` }} />
              </div>
            )}
          </Squircle>
          <div className="min-w-0 flex-1 py-0.5 pr-28">
            <p className="flex items-baseline gap-2.5">
              <span className="shrink-0 text-xs text-ink-3 tabular">{e.label}</span>
              <span className={`truncate text-[15px] font-medium ${e.finished ? 'text-ink-2' : ''}`}>
                {e.name ?? `Episode ${e.episode}`}
              </span>
              {next && (
                <span className="shrink-0 rounded-md bg-ink px-1.5 py-0.5 text-2xs font-medium text-canvas">
                  {progress > 0 ? 'Resume' : 'Up next'}
                </span>
              )}
            </p>
            {e.overview && <p className="mt-1 line-clamp-2 text-[13px] leading-relaxed text-ink-3">{e.overview}</p>}
            <p className="mt-1.5 text-xs text-ink-3">
              {progress > 0 ? remaining(e.position, e.duration) : runtime(e.duration)}
            </p>
          </div>
        </Squircle>
      </Link>
      {canEditArtwork && (
        <div className="absolute right-3 bottom-3">
          <ArtworkEditor target={{ videoId: e.id }} custom={e.customStill} label="episode thumbnail" compact />
        </div>
      )}
      <div className="absolute top-3 right-3 flex gap-0.5">
        {onWatchedUpTo && (
          <IconButton
            label="Mark watched up to here"
            onClick={onWatchedUpTo}
            className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          >
            <CheckCheck className="size-4.5" />
          </IconButton>
        )}
        {onUnwatchedFrom && (
          <IconButton
            label="Mark not watched from here, including later seasons"
            onClick={onUnwatchedFrom}
            className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          >
            <Undo2 className="size-4.5" />
          </IconButton>
        )}
        <IconButton
          label={e.finished ? 'Mark as not watched' : 'Mark watched'}
          onClick={() => onWatched(!e.finished)}
          className={e.finished ? 'text-ink' : 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100'}
        >
          {e.finished ? <CircleCheck className={`size-4.5 ${drawCheck ? 'draw' : ''}`} /> : <Check className="size-4.5" />}
        </IconButton>
      </div>
    </div>
  )
}

/** An episode that isn't in the library: coming up, being fetched, or missing. */
function GhostRow({
  e,
  admin,
  download,
  onSearch,
  onLookAgain,
}: {
  e: SeriesEpisode
  admin: boolean
  download?: Download
  onSearch: () => void
  onLookAgain: () => void
}) {
  const l = download?.live
  const upcoming = !e.aired && e.airAt && e.airAt * 1000 > Date.now()
  return (
    <Squircle radius={14} className="group flex gap-4 p-3 transition-colors hover:bg-hover/50">
      <Squircle
        radius={10}
        dashed
        className="grid aspect-video w-36 shrink-0 place-items-center text-sm text-ink-3 tabular md:w-44"
      >
        {e.episode}
      </Squircle>
      <div className="min-w-0 flex-1 py-0.5">
        <p className="flex items-baseline gap-2.5">
          <span className="shrink-0 text-xs text-ink-3 tabular">
            S{String(e.season).padStart(2, '0')}E{String(e.episode).padStart(2, '0')}
          </span>
          <span className="truncate text-[15px] font-medium text-ink-2">{e.name ?? `Episode ${e.episode}`}</span>
        </p>
        <p className="mt-1.5 text-xs text-ink-3">
          {e.airAt ? (upcoming ? `Airs ${airs(e.airAt).replace(/^(Today|Tomorrow|Yesterday)/, (w) => w.toLowerCase())}` : `Aired ${shortDate(e.airAt)}`) : 'Aired'}
        </p>
        <div className="mt-2 flex items-center gap-2">
          {e.state !== 'IDLE' && !upcoming && <StateBadge state={e.state} />}
          {upcoming && e.state === 'WANTED' && <span className="text-xs text-ink-3">Downloads when it airs</span>}
        </div>
        {l && l.stage !== 'METADATA' && (
          <div className="mt-2.5 max-w-sm">
            <div className="h-1 overflow-hidden rounded-full bg-press">
              <div className="h-full rounded-full bg-info transition-[width] duration-[2s] ease-linear" style={{ width: `${l.progress * 100}%` }} />
            </div>
            <p className="mt-1 text-xs text-ink-3 tabular">
              {(l.progress * 100).toFixed(1)}%{l.downloadRate > 0 && ` · ${speed(l.downloadRate)}`}
              {l.eta != null && ` · ${duration(l.eta)} left`}
            </p>
          </div>
        )}
      </div>
      {admin && !upcoming && (
        <div className="flex self-start">
          {e.state === 'SKIPPED' && (
            <IconButton label="Look for it again" onClick={onLookAgain} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100">
              <RotateCcw className="size-4.5" />
            </IconButton>
          )}
          <IconButton label="Search for this episode" onClick={onSearch} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100">
            <Search className="size-4.5" />
          </IconButton>
        </div>
      )}
    </Squircle>
  )
}

function MatchDialog({ item, onClose }: { item: Item; onClose: () => void }) {
  const qc = useQueryClient()
  const [provider, setProvider] = useState<Provider>(item.libraryProvider ?? item.provider ?? 'ANILIST')
  const [q, setQ] = useState('')
  const [query, setQuery] = useState('')
  const { data, isFetching, error } = useQuery({
    queryKey: ['match', item.id, provider, query],
    queryFn: async () => {
      const r = (await request(MatchCandidatesQuery, { id: item.id, provider, query: query || null })).title?.matchCandidates
      if (!r) throw new Error('This title isn’t here anymore.')
      return r
    },
  })
  useEffect(() => {
    if (data && !q) setQ(data.query)
  }, [data, q])
  const apply = useMutation({
    mutationFn: (c: { provider: Provider; id: string }) => request(MatchTitle, { id: item.id, provider: c.provider, providerId: c.id }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['item', item.id] })
      onClose()
    },
  })

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-shade/50 px-4 pt-[10vh] animate-[fade_120ms_ease-out]"
      onPointerDown={(e) => e.target === e.currentTarget && onClose()}
      onKeyDown={(e) => e.key === 'Escape' && onClose()}
    >
      <div className="w-full max-w-xl animate-[pop_140ms_ease-out]">
        <Panel radius={18} className="p-4">
          <p className="text-[15px] font-medium">Which one is “{item.name}”?</p>
          <form
            className="mt-4 flex gap-2"
            onSubmit={(e) => {
              e.preventDefault()
              setQuery(q)
            }}
          >
            <div className="flex-1">
              <Input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search by title" />
            </div>
            <Button type="button" onClick={() => setProvider(provider === 'ANILIST' ? 'TMDB' : 'ANILIST')}>
              {provider === 'ANILIST' ? 'AniList' : 'TMDB'}
            </Button>
            <Button variant="primary" type="submit">
              <Search className="size-4" />
            </Button>
          </form>
          <div className="mt-3 max-h-[55vh] space-y-0.5 overflow-y-auto">
            {isFetching && (
              <div className="grid place-items-center py-10 text-ink-3">
                <Spinner />
              </div>
            )}
            {error && <p className="py-6 text-center text-sm text-danger">{(error as Error).message}</p>}
            {!isFetching && data?.results.length === 0 && (
              <p className="py-6 text-center text-sm text-ink-3">No results.</p>
            )}
            {!isFetching &&
              data?.results.map((c) => (
                <button key={c.id} className="block w-full text-left" onClick={() => apply.mutate(c)} disabled={apply.isPending}>
                  <Squircle radius={12} className="flex gap-3 p-2 transition-colors hover:bg-hover">
                    <Squircle radius={8} className="aspect-[2/3] w-12 shrink-0 bg-panel">
                      {c.poster && <img src={c.poster} alt="" className="size-full object-cover" />}
                    </Squircle>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium">
                        {c.name} {c.year && <span className="font-normal text-ink-3">{c.year}</span>}
                      </p>
                      {c.overview && <p className="mt-0.5 line-clamp-2 text-xs leading-relaxed text-ink-3">{c.overview}</p>}
                    </div>
                  </Squircle>
                </button>
              ))}
          </div>
        </Panel>
      </div>
    </div>
  )
}
