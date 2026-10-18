// SPDX-License-Identifier: AGPL-3.0-or-later
// Home (web/src/routes/index.tsx): the one thing to watch next, big, then
// shelves of what to continue, what's coming, and what's new.

import { useQuery } from '@tanstack/react-query'
import { remaining } from '@tinystream/shared/format'
import { airs, countdown, episodeCode } from '@tinystream/shared/downloads'
import { type Href, useRouter } from 'expo-router'
import { Play, Radio, RotateCcw } from 'lucide-react-native'
import { useState } from 'react'
import { Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { Bell } from '../../../src/components/Bell'
import { Img } from '../../../src/components/Img'
import { MismatchBanner } from '../../../src/components/MismatchBanner'
import { Page, Section, SectionLink } from '../../../src/components/Page'
import { Poster } from '../../../src/components/Poster'
import { Row } from '../../../src/components/Row'
import { RowSkeleton, useArrived } from '../../../src/components/Skeleton'
import { Ambient, Bar, Shade, Tag } from '../../../src/components/media'
import { Button, Empty } from '../../../src/components/ui'
import { Bone } from '../../../src/effects/Shimmer'
import { Squircle } from '../../../src/effects/Squircle'
import { Ticker } from '../../../src/effects/Ticker'
import { Tilt } from '../../../src/effects/Tilt'
import { useMotion } from '../../../src/effects/motion'
import { ReadingNowQuery } from '../../../src/books'
import { BookTile } from '../../../src/components/BookTile'
import { graphql } from '../../../src/gql'
import type { HomeQuery } from '../../../src/gql/graphql'
import { titleTint } from '../../../src/lib/tint'
import { useGo, useWatch } from '../../../src/nav'
import { type CalendarEntry, useFeatures, useLibraries, useMe, useNow } from '../../../src/queries'
import { useApi } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

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

const MusicHomeQuery = graphql(`
  query MusicHome {
    musicHome {
      recentlyPlayed {
        id
        name
        artist
        cover
      }
      recentlyAdded {
        id
        name
        artist
        cover
      }
    }
  }
`)

type ContinueEntry = HomeQuery['home']['continueWatching'][number]

const POSTER = 124

export default function Home() {
  const api = useApi()
  const me = useMe()
  const go = useGo()
  const motion = useMotion()
  const home = useQuery({ queryKey: ['home'], queryFn: async () => (await api.request(HomeQueryDoc)).home })
  const libraries = useLibraries()
  const data = home.data
  const arrived = useArrived(!!data)

  const refresh = () => Promise.all([home.refetch(), libraries.refetch()])

  if (!data)
    return (
      <Page title="Home" right={<Bell />}>
        <MismatchBanner />
        <Bone radius={22} style={{ height: 260 }} />
        <RowSkeleton wide />
        <RowSkeleton />
      </Page>
    )

  const nothing =
    data.recentlyAdded.every((r) => r.titles.length === 0) &&
    !libraries.data?.some((l) => (l.kind === 'MUSIC' && l.albumCount > 0) || (l.kind === 'BOOKS' && l.bookCount > 0))
  // A fresh episode takes the hero; otherwise whatever was watched last.
  const hero = data.continueWatching.find((e) => e.newEpisode) ?? data.continueWatching[0]
  const rest = data.continueWatching.filter((e) => e !== hero)
  const shown = (n: number) => (arrived ? motion.developIn(n) : undefined)

  return (
    <Page title="Home" right={<Bell />} onRefresh={refresh}>
      <MismatchBanner />
      {hero && (
        <Animated.View style={shown(0)}>
          <Hero entry={hero} />
        </Animated.View>
      )}
      {nothing && (
        <Empty title={data.recentlyAdded.length ? 'Your libraries are empty so far' : 'No libraries yet'}>
          {me?.isAdmin ? (
            <Button variant="primary" className="mt-3" onPress={() => go('settings/libraries', '(profile)')}>
              Add a library
            </Button>
          ) : (
            'Ask an admin for access.'
          )}
        </Empty>
      )}

      {rest.length > 0 && (
        <Animated.View style={shown(1)}>
          <Section title="Continue watching">
            <Row data={rest} width={280} keyOf={(e) => e.video.id} render={(e) => <ContinueCard entry={e} />} />
          </Section>
        </Animated.View>
      )}

      <ComingUp />

      {data.popularHere.length > 0 && (
        <Animated.View style={shown(2)}>
          <Section title="Popular with people here">
            <Row
              data={data.popularHere}
              width={POSTER}
              keyOf={(p) => p.title.id}
              render={({ title, people }) => <Poster card={title} width={POSTER} caption={people > 1 ? `${people} people watching` : 'Someone’s watching'} />}
            />
          </Section>
        </Animated.View>
      )}

      {data.recentlyAdded
        .filter((r) => r.titles.length)
        .map((r, i) => (
          <Animated.View key={r.library} style={shown(3 + i)}>
            <Section title={`New in ${r.library}`} aside={<SectionLink label="See all" onPress={() => go(`library?name=${encodeURIComponent(r.library)}`, '(library)')} />}>
              <Row data={r.titles} width={POSTER} keyOf={(c) => c.id} render={(c) => <Poster card={c} width={POSTER} />} />
            </Section>
          </Animated.View>
        ))}

      {libraries.data?.some((l) => l.kind === 'BOOKS') && <BookShelves />}

      {libraries.data?.some((l) => l.kind === 'MUSIC') && <MusicShelves />}
    </Page>
  )
}

/** What's being read, and what's new to read. */
function BookShelves() {
  const api = useApi()
  const go = useGo()
  const { data } = useQuery({ queryKey: ['books', 'home'], queryFn: () => api.request(ReadingNowQuery) })
  if (!data) return null
  return (
    <>
      {data.readingNow.length > 0 && (
        <Section title="Continue reading">
          <Row data={data.readingNow} width={BOOK} keyOf={(b) => b.id} render={(b) => <BookTile book={b} width={BOOK} />} />
        </Section>
      )}
      {data.newBooks.map((s) => (
        <Section key={s.library} title={`New in ${s.library}`} aside={<SectionLink label="See all" onPress={() => go(`library?name=${encodeURIComponent(s.library)}`, '(library)')} />}>
          <Row data={s.books} width={BOOK} keyOf={(b) => b.id} render={(b) => <BookTile book={b} width={BOOK} />} />
        </Section>
      ))}
    </>
  )
}

const BOOK = 108

/** Music, a step quieter than the shows and movies above it. Albums open in the Music tab. */
function MusicShelves() {
  const api = useApi()
  const router = useRouter()
  const { data } = useQuery({ queryKey: ['music', 'home'], queryFn: async () => (await api.request(MusicHomeQuery)).musicHome })
  if (!data) return null
  const shelf = (title: string, albums: typeof data.recentlyAdded) =>
    albums.length > 0 && (
      <Section title={title}>
        <Row
          data={albums}
          width={112}
          keyOf={(a) => a.id}
          render={(a) => (
            <View style={{ width: 112 }}>
              <Tilt radius={12} style={{ width: 112, aspectRatio: 1 }} onPress={() => router.navigate('/(tabs)/(music)/music' as Href)}>
                <View className="flex-1 bg-panel">{a.cover && <Img src={`${a.cover}?size=256`} style={{ flex: 1 }} />}</View>
              </Tilt>
              <Text className="font-sans mt-2 text-[13px] font-medium text-ink" numberOfLines={1}>
                {a.name}
              </Text>
              <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
                {a.artist}
              </Text>
            </View>
          )}
        />
      </Section>
    )
  return (
    <>
      {shelf('Jump back in', data.recentlyPlayed)}
      {shelf('New music', data.recentlyAdded)}
    </>
  )
}

/** The one thing to watch next, big, leaning with the phone. */
function Hero({ entry: e }: { entry: ContinueEntry }) {
  const watch = useWatch()
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
  const v = e.video
  const image = v.title.backdrop ?? v.still
  const resuming = !e.upNext
  const progress = resuming && v.duration ? e.position / v.duration : 0
  const left = remaining(e.position, v.duration)
  const label = e.newEpisode ? 'New episode' : resuming ? 'Continue watching' : 'Up next'
  return (
    <Tilt gyro max={4} radius={22} style={{ minHeight: 280 }} onPress={() => watch(v.id)}>
      <View className="flex-1 bg-raised">
        {image && !broken && <Img src={image} style={[{ position: 'absolute', inset: 0, opacity: 0.6 }]} onError={() => setBroken(true)} />}
        <Ambient tint={titleTint(v.title)} />
        <Shade to="top" stops={[0.95, 0.55, 0]} />
        <View className="flex-1 justify-end p-5" style={{ minHeight: 280 }}>
          <View className="flex-row items-center gap-2">
            {e.newEpisode ? <Radio size={14} color={tokens.warn} /> : <RotateCcw size={14} color={tokens['ink-2']} />}
            <Text className="font-sans text-xs font-medium uppercase tracking-wider text-ink-2">{label}</Text>
          </View>
          <Text className="font-sans mt-2 text-[28px] font-semibold leading-tight tracking-tight text-ink" numberOfLines={2}>
            {v.title.name}
          </Text>
          <Text className="font-sans mt-1 text-sm text-ink-2" numberOfLines={1}>
            {[v.label, v.name].filter(Boolean).join(' · ')}
          </Text>
          <View className="mt-4 flex-row items-end justify-between gap-4">
            <View className="flex-1">
              {resuming && left ? (
                <Ticker value={left} className="font-sans text-[30px] font-semibold tracking-tight text-ink" />
              ) : (
                <Text className="font-sans text-[30px] font-semibold tracking-tight text-ink">{v.label ?? 'Play'}</Text>
              )}
              {progress > 0 && (
                <View className="mt-3 h-1 w-40 overflow-hidden rounded-full bg-ink/15">
                  <View className="h-full rounded-full bg-ink" style={{ width: `${progress * 100}%` }} />
                </View>
              )}
            </View>
            <Squircle radius={14} className="flex-row items-center gap-2 bg-accent" style={{ height: 44, paddingHorizontal: 18 }}>
              <Play size={17} color={tokens['on-accent']} fill={tokens['on-accent']} />
              <Text className="font-sans text-[15px] font-medium text-on-accent">{resuming ? 'Resume' : 'Play'}</Text>
            </Squircle>
          </View>
        </View>
      </View>
    </Tilt>
  )
}

function ContinueCard({ entry }: { entry: ContinueEntry }) {
  const watch = useWatch()
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
  const v = entry.video
  const image = v.still ?? v.title.backdrop
  const progress = v.duration && !entry.upNext ? entry.position / v.duration : 0
  return (
    <Tilt max={4} radius={16} style={{ width: 280, aspectRatio: 16 / 9 }} onPress={() => watch(v.id)}>
      <View className="flex-1 bg-panel">{image && !broken && <Img src={image} style={{ flex: 1 }} onError={() => setBroken(true)} />}</View>
      <Shade to="top" media stops={[0.75, 0.1, 0]} />
      {entry.newEpisode && (
        <Tag dot={tokens.warn} style={{ position: 'absolute', top: 10, left: 10 }}>
          New episode
        </Tag>
      )}
      <View className="absolute bottom-3 left-3.5 right-3.5">
        <Text className="font-sans text-sm font-medium text-media-ink" numberOfLines={1}>
          {v.title.name}
        </Text>
        <Text className="font-sans text-xs text-media-ink/60" numberOfLines={1}>
          {[v.label, entry.upNext ? 'Up next' : remaining(entry.position, v.duration)].filter(Boolean).join(' · ')}
        </Text>
      </View>
      {progress > 0 && <Bar value={progress} />}
    </Tilt>
  )
}

/** The next week's episodes of shows being followed. */
function ComingUp() {
  const api = useApi()
  const go = useGo()
  const features = useFeatures()
  const now = useNow(30_000)
  const { data } = useQuery({
    queryKey: ['calendar', 'home'],
    queryFn: () => {
      const t = Math.floor(Date.now() / 1000)
      return api.request(ComingUpQuery, { from: t - 3600, to: t + 7 * 86400 }).then((r) => r.calendar)
    },
    enabled: !!features,
    refetchInterval: (q) => (q.state.data?.some((e) => e.download) ? 3000 : 5 * 60_000),
  })
  const list = (data ?? []).filter((e) => e.state !== 'DONE').slice(0, 12)
  if (!features || list.length === 0) return null
  return (
    <Section title="Coming up" aside={<SectionLink label="Calendar" onPress={() => go('calendar')} />}>
      <Row data={list} width={260} keyOf={(e) => `${e.seriesId}-${e.season}-${e.episode}`} render={(e) => <ComingCard e={e} now={now} />} />
    </Section>
  )
}

function ComingCard({ e, now }: { e: CalendarEntry; now: number }) {
  const go = useGo()
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
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
  return (
    <Tilt max={4} radius={16} style={{ width: 260, aspectRatio: 16 / 9 }} onPress={e.title ? () => go(`title/${e.title!.id}`) : undefined}>
      <View className="flex-1 bg-panel">{image && !broken && <Img src={image} style={{ flex: 1, opacity: 0.8 }} onError={() => setBroken(true)} />}</View>
      <Shade to="top" media stops={[0.85, 0.2, 0]} />
      <Tag color={dl ? tokens.info : undefined} style={{ position: 'absolute', top: 10, left: 10 }}>
        {badge}
      </Tag>
      <View className="absolute bottom-3 left-3.5 right-3.5">
        <Text className="font-sans text-sm font-medium text-media-ink" numberOfLines={1}>
          {e.show}
        </Text>
        <Text className="font-sans text-xs text-media-ink/60" numberOfLines={1}>
          {episodeCode(e.season, e.episode)}
          {e.name && ` · ${e.name}`}
        </Text>
      </View>
      {dl && <Bar value={dl.progress} color={tokens.info} />}
    </Tilt>
  )
}
