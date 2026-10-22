// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Play, Radio, RotateCcw } from 'lucide-react'
import { type CSSProperties, useState } from 'react'
import { remaining } from '@tinystream/shared/format'
import { Empty, Page, PageTitle, Section } from '../components/Page'
import { Poster, morphFrom } from '../components/Poster'
import { Row } from '../components/Row'
import { HomeSkeleton, useArrived } from '../components/Skeleton'
import { Squircle } from '../components/Squircle'
import { Ticker } from '../components/Ticker'
import { Button } from '../components/ui'
import { ReadingNowQuery } from '../books/api'
import { BookTile } from '../books/components'
import { graphql } from '../gql'
import type { AlbumCard } from '../music/api'
import { AlbumTile } from '../music/components'
import type { HomeQuery } from '../gql/graphql'
import { type CalendarEntry, librariesQuery, request } from '../lib/api'
import { airs, countdown, episodeCode, useFeatures, useNow } from '../lib/downloads'
import { useMe } from '../lib/hooks'
import { useTilt } from '../lib/tilt'
import { useTitle } from '../lib/title'
import { titleTint } from '../lib/tint'
import { Img } from '../components/Img'

export const Route = createFileRoute('/')({ component: HomePage })

const HomeQueryDoc = graphql(`
  query Home {
    home {
      continueWatching {
        position
        upNext
        newEpisode
        watchedAt
        video {
          id
          label
          name
          still
          duration
          title {
            id
            name
            poster
            backdrop
            posterTint
            backdropTint
          }
        }
      }
      recentlyAdded {
        library
        titles {
          ...Card
        }
      }
      popularHere {
        people
        title {
          ...Card
        }
      }
    }
  }
`)

const ComingUpQuery = graphql(`
  query ComingUp($from: Int!, $to: Int!) {
    calendar(from: $from, to: $to) {
      ...CalendarEntryFields
    }
  }
`)

type ContinueEntry = HomeQuery['home']['continueWatching'][number]

function HomePage() {
  const me = useMe()
  useTitle('Home')
  const { data } = useQuery({ queryKey: ['home'], queryFn: async () => (await request(HomeQueryDoc)).home })
  const { data: libraries } = useQuery(librariesQuery)
  const arrived = useArrived(!!data)
  if (!data) return <HomeSkeleton />

  const nothing =
    data.recentlyAdded.every((r) => r.titles.length === 0) &&
    !libraries?.some((l) => (l.kind === 'MUSIC' && l.albumCount > 0) || (l.kind === 'BOOKS' && l.bookCount > 0))
  // A fresh episode takes the hero; otherwise whatever was watched last.
  const hero = data.continueWatching.find((e) => e.newEpisode) ?? data.continueWatching[0]
  const rest = data.continueWatching.filter((e) => e !== hero)
  return (
    <Page arrive={arrived}>
      {hero ? <Hero entry={hero} /> : <PageTitle>Home</PageTitle>}
      {nothing && (
        <Empty title={data.recentlyAdded.length ? 'Your libraries are empty so far' : 'No libraries yet'}>
          {me?.isAdmin ? (
            <Link to="/settings" search={{ tab: 'libraries' }} className="mt-2 inline-block">
              <Button variant="primary">Add a library</Button>
            </Link>
          ) : (
            <p>Ask an admin for access.</p>
          )}
        </Empty>
      )}

      {rest.length > 0 && (
        <Section title="Continue watching">
          <Row>
            {rest.map((e) => (
              <ContinueCard key={e.video.id} entry={e} />
            ))}
          </Row>
        </Section>
      )}

      <ComingUp />

      {data.popularHere.length > 0 && (
        <Section title="Popular with people here">
          <Row>
            {data.popularHere.map(({ title: c, people }) => (
              <div key={c.id} className="w-38 shrink-0 snap-start">
                <Poster card={c} caption={people > 1 ? `${people} people watching` : 'Someone’s watching'} />
              </div>
            ))}
          </Row>
        </Section>
      )}

      {data.recentlyAdded
        .filter((r) => r.titles.length)
        .map((r) => (
          <Section
            key={r.library}
            title={`New in ${r.library}`}
            aside={
              <Link
                to="/library/$name"
                params={{ name: r.library }}
                className="text-[13px] text-ink-3 transition-colors hover:text-ink"
              >
                See all
              </Link>
            }
          >
            <Row>
              {r.titles.map((c) => (
                <div key={c.id} className="w-38 shrink-0 snap-start">
                  <Poster card={c} />
                </div>
              ))}
            </Row>
          </Section>
        ))}

      {libraries?.some((l) => l.kind === 'BOOKS') && <BookShelves />}

      {libraries?.some((l) => l.kind === 'MUSIC') && <MusicShelves />}
    </Page>
  )
}

const MusicHomeQuery = graphql(`
  query MusicHome {
    musicHome {
      recentlyPlayed {
        ...AlbumCard
      }
      recentlyAdded {
        ...AlbumCard
      }
    }
  }
`)

/** What's being read, and what's new to read. */
function BookShelves() {
  const { data } = useQuery({ queryKey: ['books', 'home'], queryFn: () => request(ReadingNowQuery) })
  if (!data) return null
  return (
    <>
      {data.readingNow.length > 0 && (
        <Section title="Continue reading">
          <Row>
            {data.readingNow.map((b) => (
              <div key={b.id} className="w-34 shrink-0 snap-start">
                <BookTile book={b} />
              </div>
            ))}
          </Row>
        </Section>
      )}
      {data.newBooks.map((s) => (
        <Section
          key={s.library}
          title={`New in ${s.library}`}
          aside={
            <Link to="/library/$name" params={{ name: s.library }} className="text-[13px] text-ink-3 transition-colors hover:text-ink">
              See all
            </Link>
          }
        >
          <Row>
            {s.books.map((b) => (
              <div key={b.id} className="w-34 shrink-0 snap-start">
                <BookTile book={b} />
              </div>
            ))}
          </Row>
        </Section>
      ))}
    </>
  )
}

/** Music, a step quieter than the shows and movies above it. */
function MusicShelves() {
  const { data } = useQuery({ queryKey: ['music', 'home'], queryFn: async () => (await request(MusicHomeQuery)).musicHome })
  if (!data) return null
  const shelf = (title: string, albums: AlbumCard[]) =>
    albums.length > 0 && (
      <Section title={title}>
        <Row>
          {albums.map((a) => (
            <div key={a.id} className="w-34 shrink-0 snap-start">
              <AlbumTile album={a} />
            </div>
          ))}
        </Row>
      </Section>
    )
  return (
    <>
      {shelf('Jump back in', data.recentlyPlayed)}
      {shelf('New music', data.recentlyAdded)}
    </>
  )
}

/** The one thing to watch next, big. */
function Hero({ entry: e }: { entry: ContinueEntry }) {
  const [broken, setBroken] = useState(false)
  const v = e.video
  const image = v.title.backdrop ?? v.still
  const ambient = titleTint(v.title)
  const resuming = !e.upNext
  const progress = resuming && v.duration ? e.position / v.duration : 0
  const left = remaining(e.position, v.duration)
  const label = e.newEpisode ? 'New episode' : resuming ? 'Continue watching' : 'Up next'
  return (
    <Link to="/watch/$id" params={{ id: String(v.id) }} className="group mb-11 block rounded-[23px] outline-offset-3 outline-none focus-visible:outline-2 focus-visible:outline-glow/70">
      <Squircle radius={22} edge className="relative min-h-64 overflow-hidden bg-raised md:h-76">
        {image && !broken && (
          <Img
            src={image}
            onError={() => setBroken(true)}
            className="absolute inset-0 size-full object-cover opacity-60 transition-transform duration-700 ease-out group-hover:scale-[1.015]"
          />
        )}
        <div
          className="ambient absolute inset-0"
          style={{
            '--ambient': ambient ? `rgb(${ambient} / 0.38)` : 'transparent',
            background: 'radial-gradient(120% 90% at 0% 100%, var(--ambient), transparent 60%)',
          } as CSSProperties}
        />
        <div className="absolute inset-0 bg-linear-to-r from-canvas via-canvas/65 to-transparent" />
        <div className="relative flex h-full min-h-64 flex-col justify-end p-6 md:p-9">
          <p className="flex items-center gap-2 text-xs font-medium tracking-wide text-ink-2 uppercase">
            {e.newEpisode ? <Radio className="size-3.5 text-warn" /> : <RotateCcw className="size-3.5" />}
            {label}
          </p>
          <p className="mt-2 max-w-2xl text-[28px] leading-tight font-semibold tracking-[-0.02em] text-balance md:text-[38px]">{v.title.name}</p>
          <p className="mt-1 truncate text-sm text-ink-2">
            {[v.label, v.name].filter(Boolean).join(' · ')}
          </p>
          <div className="mt-5 flex flex-wrap items-end gap-x-6 gap-y-4">
            <div>
              <span className="block text-[32px] leading-none font-semibold tracking-tight whitespace-nowrap md:text-[40px]">
                {resuming && left ? <Ticker value={left} /> : (v.label ?? 'Play')}
              </span>
              {progress > 0 && (
                <div className="mt-3 h-1 w-56 overflow-hidden rounded-full bg-media-ink/15">
                  <div className="h-full rounded-full bg-media-ink" style={{ width: `${progress * 100}%` }} />
                </div>
              )}
            </div>
            <Squircle
              radius={14}
              className="inline-flex h-11 items-center gap-2.5 bg-accent px-5 text-[15px] font-medium text-on-accent transition-colors group-hover:bg-accent-hover"
            >
              <Play className="size-4.5 fill-current" />
              {resuming ? 'Resume' : 'Play'}
            </Squircle>
          </div>
        </div>
      </Squircle>
    </Link>
  )
}

function ContinueCard({ entry }: { entry: ContinueEntry }) {
  const [broken, setBroken] = useState(false)
  const v = entry.video
  const image = v.still ?? v.title.backdrop
  const progress = v.duration && !entry.upNext ? entry.position / v.duration : 0
  const tilt = useTilt<HTMLDivElement>(4)
  return (
    <Link
      to="/watch/$id"
      params={{ id: String(v.id) }}
      viewTransition
      onClick={(e) => morphFrom(e, 'still')}
      className="group block w-80 shrink-0 snap-start rounded-[17px] outline-offset-3 outline-none focus-visible:outline-2 focus-visible:outline-glow/70"
      {...tilt.handlers}
    >
      <div ref={tilt.ref} className="tilt">
      <Squircle radius={16} edge className="aspect-video overflow-hidden rounded-2xl bg-raised" data-morph>
        {image && !broken ? (
          <Img
            src={image}
            loading="lazy"
            onError={() => setBroken(true)}
            className="size-full object-cover transition-transform duration-500 ease-out group-hover:scale-[1.02]"
          />
        ) : (
          <div className="size-full bg-panel" />
        )}
        <div className="absolute inset-0 bg-linear-to-t from-media-shade/75 via-media-shade/10 to-transparent" />
        {entry.newEpisode && (
          <span className="absolute top-3 left-3 flex items-center gap-1.5 rounded-md bg-media-shade/60 px-2 py-1 text-2xs font-medium text-media-ink backdrop-blur-md">
            <span className="size-1.5 rounded-full bg-warn" /> New episode
          </span>
        )}
        <div className="absolute inset-x-3.5 bottom-3 flex items-end gap-3">
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium text-media-ink">{v.title.name}</p>
            <p className="truncate text-xs text-media-ink/60">
              {[v.label, entry.upNext ? 'Up next' : remaining(entry.position, v.duration)]
                .filter(Boolean)
                .join(' · ')}
            </p>
          </div>
          <span className="grid size-9 shrink-0 place-items-center rounded-full bg-media-shade/45 text-media-ink opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
            <Play className="size-4 fill-current" />
          </span>
        </div>
        {progress > 0 && (
          <div className="absolute inset-x-0 bottom-0 h-[3px] bg-media-ink/15">
            <div className="h-full bg-media-ink" style={{ width: `${progress * 100}%` }} />
          </div>
        )}
        <div className="glare" />
      </Squircle>
      </div>
    </Link>
  )
}

/** The next week's episodes of shows being followed. */
function ComingUp() {
  const features = useFeatures()
  const now = useNow(30_000)
  const { data } = useQuery({
    queryKey: ['calendar', 'home'],
    queryFn: () => {
      const t = Math.floor(Date.now() / 1000)
      return request(ComingUpQuery, { from: t - 3600, to: t + 7 * 86400 }).then((r) => r.calendar)
    },
    enabled: !!features,
    refetchInterval: (q) => (q.state.data?.some((e) => e.download) ? 3000 : 5 * 60_000),
  })
  const list = (data ?? []).filter((e) => e.state !== 'DONE').slice(0, 12)
  if (!features || list.length === 0) return null
  return (
    <Section
      title="Coming up"
      aside={
        <Link to="/calendar" className="text-[13px] text-ink-3 transition-colors hover:text-ink">
          Calendar
        </Link>
      }
    >
      <Row>
        {list.map((e) => (
          <ComingCard key={`${e.seriesId}-${e.season}-${e.episode}`} e={e} now={now} />
        ))}
      </Row>
    </Section>
  )
}

function ComingCard({ e, now }: { e: CalendarEntry; now: number }) {
  const [broken, setBroken] = useState(false)
  const tilt = useTilt<HTMLDivElement>(4)
  const image = e.backdrop ?? e.poster
  const dl = e.download
  const badge = dl
    ? `Downloading ${Math.floor(dl.progress * 100)}%`
    : e.airAt * 1000 <= now
      ? e.state === 'WANTED'
        ? 'Searching…'
        : 'Out now'
      : e.airAt * 1000 - now < 86400000
        ? countdown(e.airAt, now)
        : airs(e.airAt, now)
  const body = (
    <div ref={tilt.ref} className="tilt">
    <Squircle radius={16} edge className="aspect-video overflow-hidden bg-raised">
      {image && !broken ? (
        <Img
          src={image}
          loading="lazy"
          onError={() => setBroken(true)}
          className="size-full object-cover opacity-80 transition-transform duration-500 ease-out group-hover:scale-[1.02]"
        />
      ) : (
        <div className="size-full bg-panel" />
      )}
      <div className="absolute inset-0 bg-linear-to-t from-media-shade/85 via-media-shade/20 to-transparent" />
      <span
        className={`absolute top-3 left-3 rounded-md bg-media-shade/60 px-2 py-1 text-2xs font-medium tabular backdrop-blur-md ${dl ? 'text-info' : 'text-media-ink'}`}
      >
        {badge}
      </span>
      <div className="absolute inset-x-3.5 bottom-3">
        <p className="truncate text-sm font-medium text-media-ink">{e.show}</p>
        <p className="truncate text-xs text-media-ink/60">
          {episodeCode(e.season, e.episode)}
          {e.name && ` · ${e.name}`}
        </p>
      </div>
      {dl && (
        <div className="absolute inset-x-0 bottom-0 h-[3px] bg-media-ink/15">
          <div className="h-full bg-info transition-[width] duration-[3s] ease-linear" style={{ width: `${dl.progress * 100}%` }} />
        </div>
      )}
      <div className="glare" />
    </Squircle>
    </div>
  )
  const cls = 'group block w-72 shrink-0 snap-start rounded-[17px] outline-offset-3 outline-none focus-visible:outline-2 focus-visible:outline-glow/70'
  return e.title ? (
    <Link to="/title/$id" params={{ id: String(e.title.id) }} className={cls} {...tilt.handlers}>
      {body}
    </Link>
  ) : (
    <div className={cls} {...tilt.handlers}>
      {body}
    </div>
  )
}
