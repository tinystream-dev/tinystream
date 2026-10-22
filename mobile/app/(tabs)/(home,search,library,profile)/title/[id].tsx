// SPDX-License-Identifier: AGPL-3.0-or-later
// A title (web/src/routes/title.$id.tsx): its backdrop drifting behind the
// page, the poster leaning with the phone, what to play, the seasons and
// their episodes, and what else people might like. Admin actions are in the
// overflow sheet.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { airs, duration, shortDate, speed } from '@tinystream/shared/downloads'
import { remaining, runtime } from '@tinystream/shared/format'
import { useLocalSearchParams } from 'expo-router'
import { Check, CheckCheck, CircleCheck, EllipsisVertical, ImageUp, Play, RefreshCw, RotateCcw, Search, Undo2, Wand2 } from 'lucide-react-native'
import { useEffect, useRef, useState } from 'react'
import { Pressable, ScrollView, Text, View, useWindowDimensions } from 'react-native'
import Animated, { interpolate, useAnimatedStyle } from 'react-native-reanimated'
import { haptic } from '../../../../modules/haptics'
import { ArtworkSheet, type ArtworkTarget } from '../../../../src/components/ArtworkSheet'
import { DiscoverShelf } from '../../../../src/components/Discover'
import { ReleaseSheet, StateBadge, useLookForAgain } from '../../../../src/components/downloads'
import { toast, toastError } from '../../../../src/components/Feedback'
import { Img } from '../../../../src/components/Img'
import { Ambient, Shade } from '../../../../src/components/media'
import { Menu } from '../../../../src/components/Menu'
import { Page, Section, usePageScroll } from '../../../../src/components/Page'
import { Poster } from '../../../../src/components/Poster'
import { Row } from '../../../../src/components/Row'
import { NextEpisode, SeriesPanel, useItemSeries } from '../../../../src/components/SeriesPanel'
import { Sheet } from '../../../../src/components/Sheet'
import { useArrived } from '../../../../src/components/Skeleton'
import { Button, Chip, Empty, ErrorText, IconButton, Input, Progress, Spinner, usePick } from '../../../../src/components/ui'
import { Bone } from '../../../../src/effects/Shimmer'
import { Squircle } from '../../../../src/effects/Squircle'
import { Tilt } from '../../../../src/effects/Tilt'
import { useMotion } from '../../../../src/effects/motion'
import { graphql } from '../../../../src/gql'
import type { Provider } from '../../../../src/gql/graphql'
import { awaitTint, tintPending, titleTint } from '../../../../src/lib/tint'
import { useWatch } from '../../../../src/nav'
import { type Download, type Episode, type Item, type SeriesEpisode, titleQuery, useFeatures, useMe } from '../../../../src/queries'
import { remember } from '../../../../src/recents'
import { useApi, useSession } from '../../../../src/session'
import { useTheme } from '../../../../src/theme/ThemeProvider'

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

const HERO = 380

export default function Title() {
  const { id: raw } = useLocalSearchParams<{ id: string }>()
  const id = Number(raw)
  const api = useApi()
  const { server } = useSession()
  const me = useMe()
  const qc = useQueryClient()
  const motion = useMotion()
  const { tokens } = useTheme()
  const watch = useWatch()
  const key = ['item', id]
  const { data: item, error, refetch } = useQuery({
    ...titleQuery(api, id),
    refetchInterval: awaitTint<Item | null>(tintPending),
  })
  const [season, setSeason] = useState<number | null>(null)
  const [menu, setMenu] = useState(false)
  const [matching, setMatching] = useState(false)
  const [artwork, setArtwork] = useState<ArtworkTarget[] | null>(null)
  const [searchEpisode, setSearchEpisode] = useState<SeriesEpisode | null>(null)
  const features = useFeatures()
  const { data: series } = useItemSeries(id, !!features && item?.kind === 'SHOW', !!me?.permissions.manageShows)
  const lookAgain = useLookForAgain()
  const arrived = useArrived(!!item)

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

  useEffect(() => {
    if (item) remember(server.id, { id: item.id, title: item.name, poster: item.poster, kind: item.kind === 'SHOW' ? 'show' : 'movie' })
  }, [item, server.id])

  const settle = () => {
    void qc.invalidateQueries({ queryKey: key })
    void qc.invalidateQueries({ queryKey: ['home'] })
    void qc.invalidateQueries({ queryKey: ['library'] })
  }
  /** Marks episodes (by id) watched or not, showing it straight away. */
  const setEpisodes = useMutation({
    mutationFn: ({ ids, watched }: { ids: number[]; watched: boolean }) => api.request(SetWatched, { videoIds: ids, watched }),
    onMutate: ({ ids, watched }) => {
      const before = qc.getQueryData<Item>(key)
      if (before)
        qc.setQueryData<Item>(key, {
          ...before,
          seasons: before.seasons.map((s) => ({ ...s, episodes: s.episodes.map((e) => (ids.includes(e.id) ? { ...e, finished: watched, position: null } : e)) })),
        })
      return { before }
    },
    onError: (e, _, ctx) => {
      if (ctx?.before) qc.setQueryData(key, ctx.before)
      toastError(e)
    },
    onSettled: settle,
  })
  const watchAll = useMutation({ mutationFn: (w: boolean) => api.request(SetTitleWatched, { id, watched: w }), onError: toastError, onSettled: settle })
  const refresh = useMutation({ mutationFn: () => api.request(RefreshTitle, { id }), onError: toastError })
  const can = me?.permissions
  const { data: downloads } = useQuery({
    queryKey: ['downloads'],
    queryFn: async () => (await api.request(DownloadsQuery)).downloads,
    enabled: !!can?.downloads && !!series && 'counts' in series && series.counts.grabbed > 0,
    refetchInterval: 2000,
  })

  if (error || item === null)
    return (
      <Page title="Title">
        <Empty title="This title isn't here anymore" />
      </Page>
    )
  if (!item)
    return (
      <Page title="" hero={<View style={{ height: HERO }} />}>
        <View className="flex-row items-end gap-4" style={{ marginTop: -140 }}>
          <Bone radius={16} style={{ width: 120, aspectRatio: 2 / 3 }} />
          <View className="flex-1 gap-2 pb-1">
            <Bone radius={8} style={{ height: 28, width: '80%' }} />
            <Bone radius={6} style={{ height: 14, width: '50%' }} />
          </View>
        </View>
        <Bone radius={14} style={{ height: 48 }} />
        <Bone radius={8} style={{ height: 70 }} />
        {Array.from({ length: 4 }, (_, i) => (
          <View key={i} className="flex-row gap-3">
            <Bone radius={10} style={{ width: 128, aspectRatio: 16 / 9 }} />
            <View className="flex-1 gap-2 pt-1">
              <Bone radius={5} style={{ height: 13, width: '70%' }} />
              <Bone radius={5} style={{ height: 10, width: '90%' }} />
            </View>
          </View>
        ))}
      </Page>
    )

  const all = item.seasons.flatMap((s) => s.episodes)
  const watchedFrom = (episode: Episode) =>
    all
      .filter((e) => e.finished && ((e.season ?? 0) > (episode.season ?? 0) || ((e.season ?? 0) === (episode.season ?? 0) && (e.episode ?? 0) >= (episode.episode ?? 0))))
      .map((e) => e.id)
  const allWatched = all.length > 0 && all.every((e) => e.finished)
  const current = item.seasons.find((s) => s.number === season)
  const extras = current?.episodes.filter((e) => e.episode === null && e.season !== 0) ?? []
  const regular = current?.episodes.filter((e) => !extras.includes(e)) ?? []
  // Episodes the schedule knows about but the library doesn't have (yet).
  const scheduled = (series && 'episodes' in series ? series.episodes : undefined) ?? []
  const ghosts = scheduled.filter((e) => e.season === season && !e.video && (e.aired || e.airAt))
  const extraSeasons = [...new Set(scheduled.filter((e) => !e.video && (e.aired || e.airAt)).map((e) => e.season))].filter((n) => !item.seasons.some((s) => s.number === n))
  const unaired = new Map<number, number>()
  for (const e of scheduled) if (!e.video && !e.aired && e.airAt && e.airAt * 1000 > Date.now()) unaired.set(e.season, (unaired.get(e.season) ?? 0) + 1)
  const seasonTabs = [
    ...item.seasons.map((s) => ({ number: s.number, name: s.name, done: s.episodes.every((e) => e.finished), upcoming: unaired.get(s.number) ?? 0 })),
    ...extraSeasons.map((n) => ({
      number: n,
      name: n === 0 ? (scheduled.filter((e) => e.season === 0).length === 1 ? 'Special' : 'Specials') : `Season ${n}`,
      done: false,
      upcoming: unaired.get(n) ?? 0,
    })),
  ].sort((a, b) => (a.number === 0 ? 1 : b.number === 0 ? -1 : a.number - b.number))
  const next = item.nextUp
  const markAll = () => {
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
                await api.request(SetTitleWatched, { id: item.id, watched: false })
                if (was.length) await api.request(SetWatched, { videoIds: was, watched: true })
              } catch (e) {
                toastError(e)
              }
              settle()
            },
          },
        }),
    })
  }
  const episodes = (ids: number[], watched: boolean) =>
    setEpisodes.mutate(
      { ids, watched },
      {
        onSuccess: () =>
          toast({
            title: `Marked ${ids.length} episode${ids.length === 1 ? '' : 's'} ${watched ? 'watched' : 'not watched'}`,
            action: { label: 'Undo', run: () => setEpisodes.mutate({ ids, watched: !watched }) },
          }),
      },
    )
  const artworks: ArtworkTarget[] = [
    { target: { titleId: item.id, kind: 'POSTER' }, custom: item.customPoster, label: 'thumbnail' },
    { target: { titleId: item.id, kind: 'BACKDROP' }, custom: item.customBackdrop, label: 'banner' },
    ...(item.movie ? [{ target: { videoId: item.movie.id }, custom: item.movie.customStill, label: 'video thumbnail' }] : []),
  ]
  const order = (n: number) => (arrived ? motion.developIn(n) : undefined)

  return (
    <>
      <Page
        title={item.name}
        hero={<Hero item={item} />}
        onRefresh={() => refetch()}
        right={
          can?.editMetadata ? (
            <IconButton label="More" onPress={() => setMenu(true)}>
              <EllipsisVertical size={20} color={tokens.ink} />
            </IconButton>
          ) : undefined
        }
      >
        <Animated.View style={order(1)} className="gap-4">
          {series?.next && <NextEpisode series={series} />}
          <View className="flex-row flex-wrap gap-2">
            {next && (
              <Button variant="primary" size="lg" onPress={() => watch(next.video.id)} icon={<Play size={17} color={tokens['on-accent']} fill={tokens['on-accent']} />}>
                <Text className="font-sans text-[15px] font-medium text-on-accent">
                  {next.resuming ? 'Resume' : allWatched ? 'Watch again' : 'Play'}
                  {item.kind === 'SHOW' && next.video.label ? <Text className="font-normal opacity-60"> {next.video.label}</Text> : null}
                  {next.resuming ? <Text className="font-normal opacity-60"> {remaining(next.video.position, next.video.duration)}</Text> : null}
                </Text>
              </Button>
            )}
            <Button
              size="lg"
              onPress={markAll}
              icon={allWatched ? <CircleCheck size={18} color={tokens.ink} /> : <Check size={18} color={tokens.ink} />}
            >
              {allWatched ? 'Watched' : 'Mark watched'}
            </Button>
          </View>
        </Animated.View>
        {item.overview && (
          <Animated.View style={order(2)}>
            <Overview text={item.overview} />
          </Animated.View>
        )}
        {can?.manageShows && features && item.kind === 'SHOW' && series !== undefined && (
          <SeriesPanel item={item} series={series && 'counts' in series ? series : null} season={season} />
        )}
        {can?.editMetadata && item.matchState === 'UNMATCHED' && (
          <Text className="font-sans text-sm text-ink-3">
            No {item.libraryProvider === 'TMDB' ? 'TMDB' : 'AniList'} match was found for this folder.{' '}
            <Text className="text-ink-2 underline" onPress={() => setMatching(true)}>
              Pick one
            </Text>
          </Text>
        )}

        {item.kind === 'SHOW' && (
          <Animated.View style={order(3)} className="gap-3">
            {seasonTabs.length > 1 && season !== null && <Seasons seasons={seasonTabs} value={season} onChange={setSeason} />}
            {season === 0 && seasonTabs.length === 1 && <Text className="font-sans text-[17px] font-semibold text-ink">{current?.name ?? 'Specials'}</Text>}
            {current?.title && current.title !== item.name && <Text className="font-sans text-sm text-ink-3">{current.title}</Text>}
            <View style={{ marginHorizontal: -12 }}>
              {[
                ...regular.map((e, i, list) => ({
                  n: e.episode ?? Number.MAX_SAFE_INTEGER,
                  row: (
                    <EpisodeRow
                      key={e.id}
                      e={e}
                      next={!allWatched && next?.video.id === e.id}
                      onPlay={() => watch(e.id)}
                      onWatched={(w) => setEpisodes.mutate({ ids: [e.id], watched: w })}
                      onArtwork={can?.editMetadata ? () => setArtwork([{ target: { videoId: e.id }, custom: e.customStill, label: 'episode thumbnail' }]) : undefined}
                      onWatchedUpTo={
                        list.slice(0, i).some((x) => !x.finished) || !e.finished
                          ? () =>
                              episodes(
                                list
                                  .slice(0, i + 1)
                                  .filter((x) => !x.finished)
                                  .map((x) => x.id),
                                true,
                              )
                          : undefined
                      }
                      onUnwatchedFrom={watchedFrom(e).length > 0 ? () => episodes(watchedFrom(e), false) : undefined}
                    />
                  ),
                })),
                ...ghosts
                  .filter((g) => !current?.episodes.some((e) => e.episode === g.episode))
                  .map((g) => ({
                    n: g.episode,
                    row: (
                      <GhostRow
                        key={`g${g.episode}`}
                        e={g}
                        admin={!!can?.manageShows}
                        download={downloads?.find(
                          (d) => d.state === 'DOWNLOADING' && d.seriesId === series?.id && d.episodes.some((x) => x.season === g.season && x.episode === g.episode),
                        )}
                        onSearch={() => setSearchEpisode(g)}
                        onLookAgain={() => series && lookAgain.mutate({ seriesId: series.id, season: g.season, episode: g.episode })}
                      />
                    ),
                  })),
              ]
                .sort((a, b) => a.n - b.n)
                .map((x) => x.row)}
            </View>
            {extras.length > 0 && (
              <>
                <Text className="font-sans mt-4 text-[17px] font-semibold text-ink">Extras</Text>
                <View style={{ marginHorizontal: -12 }}>
                  {extras.map((e) => (
                    <EpisodeRow
                      key={e.id}
                      e={e}
                      next={next?.video.id === e.id}
                      onPlay={() => watch(e.id)}
                      onWatched={(w) => setEpisodes.mutate({ ids: [e.id], watched: w })}
                      onArtwork={can?.editMetadata ? () => setArtwork([{ target: { videoId: e.id }, custom: e.customStill, label: 'episode thumbnail' }]) : undefined}
                    />
                  ))}
                </View>
              </>
            )}
          </Animated.View>
        )}

        <MoreLikeThis item={item} actionable={!!features && item.kind === 'SHOW'} />
        {me?.isAdmin && item.path && (
          <Text className="font-sans text-xs text-ink-3" numberOfLines={2}>
            {item.path}
          </Text>
        )}
      </Page>

      <Menu
        open={menu}
        onClose={() => setMenu(false)}
        items={[
          { label: 'Fix match', icon: (c) => <Wand2 size={18} color={c} />, onPress: () => setMatching(true) },
          { label: 'Refresh details', icon: (c) => <RefreshCw size={18} color={c} />, onPress: () => refresh.mutate(), disabled: refresh.isPending },
          { label: 'Change artwork', icon: (c) => <ImageUp size={18} color={c} />, onPress: () => setArtwork(artworks) },
        ]}
      />
      <ArtworkSheet open={!!artwork} items={artwork ?? []} onClose={() => setArtwork(null)} />
      <Sheet open={matching} onClose={() => setMatching(false)}>
        {matching && <Match item={item} onClose={() => setMatching(false)} />}
      </Sheet>
      {series && searchEpisode && (
        <ReleaseSheet open seriesId={series.id} season={searchEpisode.season} episodes={[searchEpisode.episode]} title={item.name} onClose={() => setSearchEpisode(null)} />
      )}
    </>
  )
}

/** The backdrop drifting slower than the page, the poster leaning with the phone, and the name. */
function Hero({ item }: { item: Item }) {
  const y = usePageScroll()
  const { width } = useWindowDimensions()
  const [broken, setBroken] = useState(false)
  useEffect(() => setBroken(false), [item.backdrop])
  const drift = useAnimatedStyle(() => {
    const v = y?.value ?? 0
    return { transform: [{ translateY: v < 0 ? v : v * 0.45 }, { scale: v < 0 ? 1 - v / HERO : 1 }] }
  })
  const fade = useAnimatedStyle(() => ({ opacity: interpolate(y?.value ?? 0, [0, HERO * 0.7], [1, 0.2], 'clamp') }))
  const meta = [
    item.year,
    item.kind === 'SHOW' ? `${item.seasons.filter((s) => s.number > 0).length || item.seasons.length} season${item.seasons.length === 1 ? '' : 's'}` : runtime(item.movie?.duration),
    item.rating ? `${item.rating.toFixed(1)} rating` : null,
  ].filter(Boolean)
  return (
    <View style={{ height: HERO + 40 }}>
      <Animated.View style={[{ position: 'absolute', top: 0, left: 0, width, height: HERO }, drift]}>
        {item.backdrop && !broken && <Img src={item.backdrop} style={{ flex: 1, opacity: 0.5 }} transition={600} onError={() => setBroken(true)} />}
        <Shade to="top" stops={[1, 0.35, 0]} />
      </Animated.View>
      <Ambient tint={titleTint(item)} alpha={0.28} at="15% 0%" size="90% 70%" />
      <Animated.View style={[{ position: 'absolute', left: 20, right: 20, bottom: 0, flexDirection: 'row', alignItems: 'flex-end', gap: 16 }, fade]}>
        <Tilt gyro max={6} radius={16} style={{ width: 124, aspectRatio: 2 / 3 }}>
          <View className="flex-1 bg-panel">{item.poster && <Img src={item.poster} style={{ flex: 1 }} />}</View>
        </Tilt>
        <View className="min-w-0 flex-1 pb-1">
          <Text className="font-sans text-[28px] font-semibold leading-tight tracking-tight text-ink" numberOfLines={4}>
            {item.name}
          </Text>
          <Text className="font-sans mt-1.5 text-sm text-ink-2">{meta.join(' · ')}</Text>
          {item.genres.length > 0 && (
            <Text className="font-sans mt-0.5 text-xs text-ink-3" numberOfLines={1}>
              {item.genres.slice(0, 4).join(', ')}
            </Text>
          )}
        </View>
      </Animated.View>
    </View>
  )
}

/** The seasons' chips: the one pressed lights up straight away, its episodes come after. */
function Seasons({
  seasons,
  value,
  onChange,
}: {
  seasons: { number: number; name: string; done: boolean; upcoming: number }[]
  value: number
  onChange: (n: number) => void
}) {
  const { tokens } = useTheme()
  const [season, pick] = usePick(value, onChange)
  return (
    <ScrollView horizontal showsHorizontalScrollIndicator={false} style={{ marginHorizontal: -20 }} contentContainerStyle={{ paddingHorizontal: 20, gap: 6 }}>
      {seasons.map((s) => (
        <Chip key={s.number} on={season === s.number} onPress={() => pick(s.number)}>
          <Text className={`font-sans text-[13px] font-medium ${season === s.number ? 'text-canvas' : 'text-ink-2'}`}>{s.name}</Text>
          {s.done && !s.upcoming && <Check size={13} color={season === s.number ? tokens.canvas : tokens['ink-3']} />}
          {s.upcoming > 0 && <View style={{ width: 6, height: 6, borderRadius: 3, backgroundColor: tokens.warn }} />}
        </Chip>
      ))}
    </ScrollView>
  )
}

function Overview({ text }: { text: string }) {
  const [open, setOpen] = useState(false)
  const [long, setLong] = useState(false)
  return (
    <Pressable
      disabled={!long}
      onPress={() => {
        haptic('tick')
        setOpen((o) => !o)
      }}
    >
      <Text
        className="font-sans text-[15px] leading-6 text-ink-2"
        numberOfLines={open ? undefined : 4}
        onTextLayout={(e) => !open && setLong((l) => l || e.nativeEvent.lines.length >= 4)}
      >
        {text}
      </Text>
      {long && <Text className="font-sans mt-1.5 text-sm text-ink-3">{open ? 'Less' : 'More'}</Text>}
    </Pressable>
  )
}

function EpisodeRow({
  e,
  next,
  onPlay,
  onWatched,
  onArtwork,
  onWatchedUpTo,
  onUnwatchedFrom,
}: {
  e: Episode
  next: boolean
  onPlay: () => void
  onWatched: (w: boolean) => void
  onArtwork?: () => void
  onWatchedUpTo?: () => void
  onUnwatchedFrom?: () => void
}) {
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
  const [menu, setMenu] = useState(false)
  useEffect(() => setBroken(false), [e.still])
  const progress = !e.finished && e.position && e.duration ? e.position / e.duration : 0
  // The check pops when it's marked here, not every time the page opens.
  const before = useRef(e.finished)
  const motion = useMotion()
  const drawn = e.finished && !before.current
  return (
    <>
      <Pressable
        onPress={() => {
          haptic('press')
          onPlay()
        }}
        onLongPress={() => {
          haptic('longPressOpen')
          setMenu(true)
        }}
      >
        {({ pressed }) => (
          <Squircle radius={14} edge={next} className={pressed ? 'bg-press' : next ? 'bg-raised' : ''} style={{ flexDirection: 'row', gap: 12, padding: 12 }}>
            <Squircle radius={10} edge className="bg-panel" style={{ width: 128, aspectRatio: 16 / 9, alignItems: 'center', justifyContent: 'center' }}>
              {e.still && !broken ? (
                <Img src={e.still} style={{ position: 'absolute', inset: 0, opacity: e.finished ? 0.45 : 1 }} onError={() => setBroken(true)} />
              ) : (
                <Text className="font-sans text-sm text-ink-3">{e.episode}</Text>
              )}
              {progress > 0 && (
                <View className="absolute bottom-0 left-0 right-0 h-[3px] bg-media-shade/50">
                  <View className="h-full bg-media-ink" style={{ width: `${progress * 100}%` }} />
                </View>
              )}
            </Squircle>
            <View className="min-w-0 flex-1">
              <View className="flex-row items-center gap-2">
                <Text className="font-sans text-xs text-ink-3">{e.label}</Text>
                {next && (
                  <View className="rounded-md bg-ink px-1.5 py-0.5">
                    <Text className="font-sans text-2xs font-medium text-canvas">{progress > 0 ? 'Resume' : 'Up next'}</Text>
                  </View>
                )}
              </View>
              <Text className={`font-sans text-[15px] font-medium ${e.finished ? 'text-ink-2' : 'text-ink'}`} numberOfLines={2}>
                {e.name ?? `Episode ${e.episode}`}
              </Text>
              {e.overview && (
                <Text className="font-sans mt-0.5 text-[13px] leading-5 text-ink-3" numberOfLines={2}>
                  {e.overview}
                </Text>
              )}
              <Text className="font-sans mt-1 text-xs text-ink-3">{progress > 0 ? remaining(e.position, e.duration) : runtime(e.duration)}</Text>
            </View>
            <Pressable
              accessibilityLabel={e.finished ? 'Mark as not watched' : 'Mark watched'}
              hitSlop={10}
              onPress={() => {
                haptic(e.finished ? 'toggleOff' : 'toggleOn')
                before.current = e.finished
                onWatched(!e.finished)
              }}
              style={{ paddingTop: 2 }}
            >
              <Animated.View key={String(e.finished)} style={drawn ? motion.pop : undefined}>
                {e.finished ? <CircleCheck size={20} color={tokens.ink} /> : <Check size={20} color={tokens['ink-3']} />}
              </Animated.View>
            </Pressable>
          </Squircle>
        )}
      </Pressable>
      <Menu
        open={menu}
        onClose={() => setMenu(false)}
        header={
          <Text className="font-sans px-1 text-[15px] font-semibold text-ink" numberOfLines={2}>
            {e.label} · {e.name ?? `Episode ${e.episode}`}
          </Text>
        }
        items={[
          { label: 'Play', icon: (c) => <Play size={18} color={c} />, onPress: onPlay },
          { label: e.finished ? 'Mark as not watched' : 'Mark watched', icon: (c) => (e.finished ? <Undo2 size={18} color={c} /> : <Check size={18} color={c} />), onPress: () => onWatched(!e.finished) },
          onWatchedUpTo && { label: 'Mark watched up to here', icon: (c) => <CheckCheck size={18} color={c} />, onPress: onWatchedUpTo },
          onUnwatchedFrom && { label: 'Mark not watched from here, including later seasons', icon: (c) => <Undo2 size={18} color={c} />, onPress: onUnwatchedFrom },
          onArtwork && { label: 'Change episode thumbnail', icon: (c) => <ImageUp size={18} color={c} />, onPress: onArtwork },
        ]}
      />
    </>
  )
}

/** An episode that isn't in the library: coming up, being fetched, or missing. */
function GhostRow({ e, admin, download, onSearch, onLookAgain }: { e: SeriesEpisode; admin: boolean; download?: Download; onSearch: () => void; onLookAgain: () => void }) {
  const [menu, setMenu] = useState(false)
  const l = download?.live
  const upcoming = !e.aired && !!e.airAt && e.airAt * 1000 > Date.now()
  const actionable = admin && !upcoming
  return (
    <>
      <Pressable
        disabled={!actionable}
        onLongPress={() => {
          haptic('longPressOpen')
          setMenu(true)
        }}
        onPress={() => {
          haptic('press')
          setMenu(true)
        }}
      >
        {({ pressed }) => (
          <Squircle radius={14} className={pressed ? 'bg-press' : ''} style={{ flexDirection: 'row', gap: 12, padding: 12 }}>
            <Squircle radius={10} dashed style={{ width: 128, aspectRatio: 16 / 9, alignItems: 'center', justifyContent: 'center' }}>
              <Text className="font-sans text-sm text-ink-3">{e.episode}</Text>
            </Squircle>
            <View className="min-w-0 flex-1 gap-1">
              <Text className="font-sans text-xs text-ink-3">
                S{String(e.season).padStart(2, '0')}E{String(e.episode).padStart(2, '0')}
              </Text>
              <Text className="font-sans text-[15px] font-medium text-ink-2" numberOfLines={2}>
                {e.name ?? `Episode ${e.episode}`}
              </Text>
              <Text className="font-sans text-xs text-ink-3">
                {e.airAt ? (upcoming ? `Airs ${airs(e.airAt).replace(/^(Today|Tomorrow|Yesterday)/, (w) => w.toLowerCase())}` : `Aired ${shortDate(e.airAt)}`) : 'Aired'}
              </Text>
              <View className="flex-row items-center gap-2">
                {e.state !== 'IDLE' && !upcoming && <StateBadge state={e.state} />}
                {upcoming && e.state === 'WANTED' && <Text className="font-sans text-xs text-ink-3">Downloads when it airs</Text>}
              </View>
              {l && l.stage !== 'METADATA' && (
                <View className="mt-1 gap-1">
                  <Progress value={l.progress} />
                  <Text className="font-sans text-xs text-ink-3">
                    {(l.progress * 100).toFixed(1)}%{l.downloadRate > 0 && ` · ${speed(l.downloadRate)}`}
                    {l.eta != null && ` · ${duration(l.eta)} left`}
                  </Text>
                </View>
              )}
            </View>
          </Squircle>
        )}
      </Pressable>
      <Menu
        open={menu}
        onClose={() => setMenu(false)}
        items={[
          { label: 'Search for this episode', icon: (c) => <Search size={18} color={c} />, onPress: onSearch },
          e.state === 'SKIPPED' && { label: 'Look for it again', icon: (c) => <RotateCcw size={18} color={c} />, onPress: onLookAgain },
        ]}
      />
    </>
  )
}

/** Shows like this one from the provider, and what else people here watched. */
function MoreLikeThis({ item, actionable }: { item: Item; actionable: boolean }) {
  const api = useApi()
  const { data } = useQuery({
    queryKey: ['discover', 'similar', item.id],
    queryFn: async () => (await api.request(SimilarQuery, { id: item.id })).title?.similar ?? null,
    staleTime: 5 * 60_000,
  })
  if (!data || (data.recommendations.length === 0 && data.alsoWatched.length === 0)) return null
  return (
    <>
      {data.recommendations.length > 0 && <DiscoverShelf title="More like this" results={data.recommendations} actionable={actionable} />}
      {data.alsoWatched.length > 0 && (
        <Section title="People here who watched this also watched">
          <Row data={data.alsoWatched} width={124} keyOf={(c) => c.id} render={(c) => <Poster card={c} width={124} />} />
        </Section>
      )}
    </>
  )
}

function Match({ item, onClose }: { item: Item; onClose: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const [provider, setProvider] = useState<Provider>(item.libraryProvider ?? item.provider ?? 'ANILIST')
  const [q, setQ] = useState('')
  const [query, setQuery] = useState('')
  const { data, isFetching, error } = useQuery({
    queryKey: ['match', item.id, provider, query],
    queryFn: async () => {
      const r = (await api.request(MatchCandidatesQuery, { id: item.id, provider, query: query || null })).title?.matchCandidates
      if (!r) throw new Error('This title isn’t here anymore.')
      return r
    },
  })
  useEffect(() => {
    if (data && !q) setQ(data.query)
  }, [data, q])
  const apply = useMutation({
    mutationFn: (c: { provider: Provider; id: string }) => api.request(MatchTitle, { id: item.id, provider: c.provider, providerId: c.id }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['item', item.id] })
      toast({ title: 'Matched', tone: 'ok' })
      onClose()
    },
    onError: toastError,
  })
  return (
    <View style={{ maxHeight: 620 }}>
      <Text className="font-sans text-lg font-semibold text-ink">Which one is “{item.name}”?</Text>
      <View className="mt-3 flex-row gap-2">
        <View className="flex-1">
          <Input value={q} onChangeText={setQ} onSubmitEditing={() => setQuery(q)} returnKeyType="search" placeholder="Search by title" />
        </View>
        <Button onPress={() => setProvider(provider === 'ANILIST' ? 'TMDB' : 'ANILIST')} className="self-center">
          {provider === 'ANILIST' ? 'AniList' : 'TMDB'}
        </Button>
        <Button variant="primary" onPress={() => setQuery(q)} icon={<Search size={16} color={tokens['on-accent']} />} className="self-center" />
      </View>
      <ScrollView className="mt-3" style={{ flexGrow: 0 }}>
        {isFetching && (
          <View className="items-center py-10">
            <Spinner />
          </View>
        )}
        {error && <ErrorText>{(error as Error).message}</ErrorText>}
        {!isFetching && data?.results.length === 0 && <Text className="font-sans py-6 text-center text-sm text-ink-3">No results.</Text>}
        {!isFetching &&
          data?.results.map((c) => (
            <Pressable key={c.id} disabled={apply.isPending} onPress={() => (haptic('press'), apply.mutate(c))}>
              {({ pressed }) => (
                <Squircle radius={12} className={pressed ? 'bg-press' : ''} style={{ flexDirection: 'row', gap: 12, padding: 8 }}>
                  <Squircle radius={8} className="bg-panel" style={{ width: 48, aspectRatio: 2 / 3 }}>
                    {c.poster && <Img src={c.poster} style={{ flex: 1 }} />}
                  </Squircle>
                  <View className="min-w-0 flex-1">
                    <Text className="font-sans text-sm font-medium text-ink">
                      {c.name} {c.year ? <Text className="font-normal text-ink-3">{c.year}</Text> : null}
                    </Text>
                    {c.overview && (
                      <Text className="font-sans mt-0.5 text-xs leading-4 text-ink-3" numberOfLines={2}>
                        {c.overview}
                      </Text>
                    )}
                  </View>
                </Squircle>
              )}
            </Pressable>
          ))}
      </ScrollView>
    </View>
  )
}
