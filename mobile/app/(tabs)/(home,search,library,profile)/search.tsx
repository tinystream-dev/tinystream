// SPDX-License-Identifier: AGPL-3.0-or-later
// Search: a field pinned at the top, results grouped as web's command palette
// groups them (titles, episodes, music, places, actions), and, with nothing
// typed, Discover (web/src/routes/discover.tsx): shows to add or request.

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { mediaOptions, ofMediaType } from '@tinystream/shared/media'
import { useScrollToTop } from 'expo-router/react-navigation'
import { type Href, useFocusEffect, useLocalSearchParams, useRouter } from 'expo-router'
import {
  ArrowDownToLine,
  Bell,
  CalendarDays,
  Clapperboard,
  Disc3,
  Film,
  House,
  Inbox,
  ListTodo,
  type LucideIcon,
  Mic2,
  Music,
  Pause,
  Play,
  Plus,
  Scissors,
  Search as SearchIcon,
  Settings,
  Tv,
  X,
} from 'lucide-react-native'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { haptic } from '../../../modules/haptics'
import { DiscoverShelf, PickSheet, ResultCard, useAction } from '../../../src/components/Discover'
import { toast, toastError } from '../../../src/components/Feedback'
import { Img } from '../../../src/components/Img'
import { BAR, TopFade } from '../../../src/components/Page'
import { PosterGridSkeleton, RowSkeleton, useColumnWidth } from '../../../src/components/Skeleton'
import { useTabBarSpace } from '../../../src/components/TabBar'
import { Chip, Empty, ErrorText, Select, Spinner } from '../../../src/components/ui'
import { Glass, ScreenBlurArea } from '../../../src/effects/Glass'
import { Squircle } from '../../../src/effects/Squircle'
import { useMotion } from '../../../src/effects/motion'
import { graphql } from '../../../src/gql'
import { useGo, useWatch } from '../../../src/nav'
import { type DiscoverResult, useClipsOn, useFeatures, useLibraries, useMe } from '../../../src/queries'
import { prune, recents } from '../../../src/recents'
import { useApi, useSession } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

const SearchQuery = graphql(`
  query Search($query: String!) {
    musicSearch(query: $query, limit: 5) {
      artists {
        id
        name
        cover
        albumCount
      }
      albums {
        id
        name
        artist
        cover
        year
      }
      tracks {
        id
        title
        artist
        album
        cover
      }
    }
    search(query: $query) {
      titles {
        ...Card
      }
      videos {
        id
        label
        name
        title {
          name
        }
      }
    }
  }
`)

const RecentTitles = graphql(`
  query RecentTitles($ids: [Int!]!) {
    titles(ids: $ids) {
      id
    }
  }
`)

const DownloadStates = graphql(`
  query DownloadStates {
    downloads {
      id
      state
    }
  }
`)

const PauseDownloads = graphql(`
  mutation PauseDownloads($ids: [Int!]!) {
    pauseDownloads(ids: $ids) {
      id
    }
  }
`)

const ResumeDownloads = graphql(`
  mutation ResumeDownloads($ids: [Int!]!) {
    resumeDownloads(ids: $ids) {
      id
    }
  }
`)

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

type Command = {
  key: string
  group: string
  title: string
  meta?: string
  icon?: LucideIcon
  image?: string | null
  /** Album art is square; posters are tall. */
  square?: boolean
  go: () => void
}

export default function Search() {
  const params = useLocalSearchParams<{ q?: string }>()
  const insets = useSafeAreaInsets()
  const bottom = useTabBarSpace()
  const { tokens } = useTheme()
  const input = useRef<TextInput>(null)
  const scroll = useRef<ScrollView>(null)
  useScrollToTop(scroll)
  const [q, setQ] = useState(params.q ?? '')
  const [debounced, setDebounced] = useState(q)
  // Searching AniList or TMDB for what's typed, rather than what's here.
  const [discover, setDiscover] = useState(!!params.q)

  useEffect(() => {
    if (params.q === undefined) return
    setQ(params.q)
    setDiscover(true)
  }, [params.q])
  useEffect(() => {
    const t = setTimeout(() => setDebounced(q.trim()), 200)
    return () => clearTimeout(t)
  }, [q])
  useEffect(() => {
    if (!q.trim()) setDiscover(false)
  }, [q])

  // The field is ready to type in whenever the tab is opened.
  useFocusEffect(
    useCallback(() => {
      const t = setTimeout(() => input.current?.focus(), 250)
      // A field that keeps focus off screen gets pulled back into view by Android, over the other tabs.
      return () => {
        clearTimeout(t)
        input.current?.blur()
      }
    }, []),
  )

  const top = insets.top + BAR + 8
  return (
    <>
      <ScreenBlurArea style={{ flex: 1 }}>
        <ScrollView
          ref={scroll}
          keyboardShouldPersistTaps="handled"
          keyboardDismissMode="on-drag"
          contentContainerStyle={{ paddingTop: top + 8, paddingBottom: bottom + 32, paddingHorizontal: 20, gap: 24 }}
        >
          {!debounced ? (
            <Browse query="" />
          ) : discover ? (
            <>
              <View className="flex-row">
                <Chip on={false} onPress={() => setDiscover(false)}>
                  ‹ Back to results
                </Chip>
              </View>
              <Browse query={debounced} />
            </>
          ) : (
            <Results q={debounced} onDiscover={() => setDiscover(true)} />
          )}
        </ScrollView>
      </ScreenBlurArea>
      <View pointerEvents="box-none" style={{ position: 'absolute', top: 0, left: 0, right: 0 }}>
        <View pointerEvents="none" style={{ position: 'absolute', top: 0, left: 0, right: 0 }}>
          <TopFade height={insets.top + 40} />
        </View>
        <View style={{ paddingTop: insets.top + 8, paddingHorizontal: 16 }}>
          <Glass radius={24} style={{ height: 48, flexDirection: 'row', alignItems: 'center', paddingHorizontal: 16, gap: 10 }}>
            <SearchIcon size={18} color={tokens['ink-3']} />
            <TextInput
              ref={input}
              value={q}
              onChangeText={setQ}
              placeholder="Search, or jump anywhere"
              placeholderTextColor={tokens['ink-3']}
              selectionColor={tokens.accent}
              cursorColor={tokens.ink}
              returnKeyType="search"
              autoCorrect={false}
              style={{ flex: 1, color: tokens.ink, fontFamily: 'Geist', fontSize: 16, padding: 0 }}
            />
            {q.length > 0 && (
              <Pressable
                accessibilityLabel="Clear"
                hitSlop={10}
                onPress={() => {
                  haptic('tick')
                  setQ('')
                  input.current?.focus()
                }}
              >
                <X size={18} color={tokens['ink-3']} />
              </Pressable>
            )}
          </Glass>
        </View>
      </View>
    </>
  )
}

/** What's here that matches, grouped, then places and actions, then asking AniList or TMDB. */
function Results({ q, onDiscover }: { q: string; onDiscover: () => void }) {
  const api = useApi()
  const go = useGo()
  const watch = useWatch()
  const router = useRouter()
  const me = useMe()
  const features = useFeatures()
  const can = me?.permissions
  const { data, isFetching } = useQuery({
    queryKey: ['search', q],
    queryFn: async () => {
      const r = await api.request(SearchQuery, { query: q })
      return { ...r.search, music: r.musicSearch }
    },
    placeholderData: (prev) => prev,
  })
  const commands = useCommands()
  const needle = q.toLowerCase()
  const matches = (c: Command) => c.title.toLowerCase().includes(needle)
  const music = () => router.navigate('/(tabs)/(music)/music' as Href)
  const results: Command[] = [
    ...(data?.titles ?? []).map((c) => ({
      key: `i${c.id}`,
      group: 'Titles',
      title: c.name,
      meta: c.kind === 'SHOW' ? `Show · ${c.library}` : `Movie${c.year ? ` · ${c.year}` : ''}`,
      image: c.poster,
      go: () => go(`title/${c.id}`),
    })),
    ...(data?.videos ?? []).map((e) => ({
      key: `e${e.id}`,
      group: 'Episodes',
      title: e.name ?? e.label ?? '',
      meta: `${e.title.name} · ${e.label ?? ''}`,
      icon: Clapperboard,
      go: () => watch(e.id),
    })),
    ...(data?.music.artists ?? []).map((a) => ({
      key: `ar${a.id}`,
      group: 'Artists',
      title: a.name,
      meta: `${a.albumCount} album${a.albumCount === 1 ? '' : 's'}`,
      image: a.cover ? `${a.cover}?size=96` : null,
      square: true,
      icon: Mic2,
      go: music,
    })),
    ...(data?.music.albums ?? []).map((a) => ({
      key: `al${a.id}`,
      group: 'Albums',
      title: a.name,
      meta: `${a.artist}${a.year ? ` · ${a.year}` : ''}`,
      image: a.cover ? `${a.cover}?size=96` : null,
      square: true,
      icon: Disc3,
      go: music,
    })),
    ...(data?.music.tracks ?? []).map((t) => ({
      key: `tr${t.id}`,
      group: 'Songs',
      title: t.title,
      meta: `${t.artist} · ${t.album}`,
      image: t.cover ? `${t.cover}?size=96` : null,
      square: true,
      icon: Music,
      go: music,
    })),
    ...commands.filter(matches),
    ...(features && (can?.request || can?.manageShows)
      ? [{ key: 'discover', group: 'Elsewhere', title: `${can.manageShows ? 'Add' : 'Request'} “${q}”`, meta: 'Search AniList or TMDB', icon: Plus, go: onDiscover }]
      : []),
  ]
  if (!data && isFetching)
    return (
      <View className="items-center py-10">
        <Spinner />
      </View>
    )
  if (data && results.length === 0) return <Empty title={`Nothing called “${q}”.`} />
  return <CommandList commands={results} />
}

/** The places and actions the palette offers, for this person on this server. */
function useCommands(): Command[] {
  const api = useApi()
  const qc = useQueryClient()
  const go = useGo()
  const router = useRouter()
  const me = useMe()
  const features = useFeatures()
  const clipsOn = useClipsOn()
  const { data: libraries } = useLibraries()
  const can = me?.permissions
  return useMemo(() => {
    const bulk = (action: 'pause' | 'resume') => async () => {
      try {
        const all = (await api.request(DownloadStates)).downloads
        const targets = all.filter((d) => (action === 'pause' ? d.state === 'DOWNLOADING' || d.state === 'SEEDING' : d.state === 'PAUSED'))
        const ids = targets.map((d) => d.id)
        if (action === 'pause') await api.request(PauseDownloads, { ids })
        else await api.request(ResumeDownloads, { ids })
        void qc.invalidateQueries({ queryKey: ['downloads'] })
        toast({ title: `${action === 'pause' ? 'Paused' : 'Resumed'} ${targets.length} download${targets.length === 1 ? '' : 's'}`, tone: 'ok' })
      } catch (e) {
        toastError(e)
      }
    }
    const place = (key: string, title: string, icon: LucideIcon, to: () => void): Command => ({ key: `p/${key}`, group: 'Go to', title, icon, go: to })
    const list: (Command | false | undefined | null)[] = [
      place('home', 'Home', House, () => router.navigate('/(tabs)/(home)' as Href)),
      clipsOn && place('clips', 'Clips', Scissors, () => go('clips')),
      features && place('calendar', 'Calendar', CalendarDays, () => go('calendar')),
      features && can?.downloads && place('downloads', 'Downloads', ArrowDownToLine, () => go('downloads')),
      features && can?.manageShows && place('wanted', 'Wanted', ListTodo, () => go('wanted')),
      features && (can?.request || can?.manageRequests) && place('requests', 'Requests', Inbox, () => go('requests')),
      ...(libraries ?? [])
        .filter((l) => l.kind === 'VIDEO')
        .map((l) => ({ ...place(`l${l.name}`, l.name, l.showCount >= l.movieCount ? Tv : Film, () => go(`library?name=${encodeURIComponent(l.name)}`, '(library)')), meta: 'Library' })),
      place('settings', 'Settings', Settings, () => go('settings')),
      features && can?.downloads && { key: 'a/pause', group: 'Actions', title: 'Pause all downloads', icon: Pause, go: bulk('pause') },
      features && can?.downloads && { key: 'a/resume', group: 'Actions', title: 'Resume all downloads', icon: Play, go: bulk('resume') },
      { key: 'a/inbox', group: 'Actions', title: 'Notifications', icon: Bell, go: () => go('notifications') },
    ]
    return list.filter((c): c is Command => !!c)
  }, [api, qc, go, router, features, clipsOn, can, libraries])
}

function CommandList({ commands }: { commands: Command[] }) {
  const { tokens } = useTheme()
  const motion = useMotion()
  return (
    <Animated.View style={motion.fade}>
      {commands.map((c, i) => (
        <View key={c.key}>
          {(i === 0 || commands[i - 1].group !== c.group) && (
            <Text className={`font-sans ${i === 0 ? '' : 'mt-4'} mb-1 px-1 text-2xs font-medium uppercase tracking-wider text-ink-3`}>{c.group}</Text>
          )}
          <Pressable
            onPress={() => {
              haptic('press')
              c.go()
            }}
          >
            {({ pressed }) => (
              <Squircle radius={12} className={pressed ? 'bg-press' : ''} style={{ flexDirection: 'row', alignItems: 'center', gap: 12, paddingHorizontal: 8, paddingVertical: 9 }}>
                {c.image !== undefined ? (
                  <Squircle radius={6} className="bg-panel" style={{ width: 30, aspectRatio: c.square ? 1 : 2 / 3, alignItems: 'center', justifyContent: 'center' }}>
                    {c.image ? <Img src={c.image} style={{ position: 'absolute', inset: 0 }} /> : c.icon && <c.icon size={14} color={tokens['ink-3']} />}
                  </Squircle>
                ) : (
                  <View style={{ width: 30, alignItems: 'center' }}>{c.icon && <c.icon size={18} color={tokens['ink-3']} />}</View>
                )}
                <Text className="font-sans min-w-0 flex-1 text-[15px] text-ink" numberOfLines={1}>
                  {c.title}
                </Text>
                {c.meta && (
                  <Text className="font-sans max-w-[40%] text-xs text-ink-3" numberOfLines={1}>
                    {c.meta}
                  </Text>
                )}
              </Squircle>
            )}
          </Pressable>
        </View>
      ))}
    </Animated.View>
  )
}

/** Discover: recommendations and what's popular, or what AniList and TMDB have for `query`. */
function Browse({ query }: { query: string }) {
  const api = useApi()
  const me = useMe()
  const go = useGo()
  const { server } = useSession()
  const column = useColumnWidth()
  const { data: libraries } = useLibraries()
  const [library, setLibrary] = useState('')
  const [category, setCategory] = useState('')
  const [picked, setPicked] = useState<DiscoverResult | null>(null)
  const features = useFeatures()
  const manage = !!me?.permissions.manageShows
  const action = useAction()
  const allowed = !!features && (manage || !!me?.permissions.request)

  const { data, error } = useQuery({
    queryKey: ['discover', query, library],
    queryFn: async () => (await api.request(DiscoverQuery, { query, library: library || null })).discover,
    enabled: allowed,
    placeholderData: (p) => p,
  })
  const forYou = useQuery({
    queryKey: ['discover', 'for-you', library],
    queryFn: async () => (await api.request(ForYouQuery, { library: library || null })).forYou,
    enabled: !query && !!action && allowed,
    placeholderData: (p) => p,
    staleTime: 5 * 60_000,
  })
  const [stored] = useState(() => recents(server.id))
  const { data: recent } = useQuery({
    queryKey: ['recents', stored.map((r) => r.id)],
    queryFn: async () => prune(server.id, new Set((await api.request(RecentTitles, { ids: stored.map((r) => r.id) })).titles.map((t) => t.id))),
    enabled: !query && stored.length > 0,
    gcTime: 0,
  })

  const results = ofMediaType(data?.results ?? [], category)
  const videoLibraries = (libraries ?? []).filter((l) => l.kind === 'VIDEO')
  return (
    <>
      {!query && (
        <CommandList
          commands={(recent ?? []).map((r) => ({ key: `r${r.id}`, group: 'Recent', title: r.title, meta: r.kind === 'show' ? 'Show' : 'Movie', image: r.poster, go: () => go(`title/${r.id}`) }))}
        />
      )}
      {allowed && (
        <>
          <View>
            <Text className="font-sans text-[22px] font-semibold tracking-tight text-ink">{manage ? 'Add shows' : 'Request shows'}</Text>
            <View className="mt-3 flex-row gap-2">
              {videoLibraries.length > 1 && (
                <View className="flex-1">
                  <Select
                    title="Library"
                    value={library || data?.library || ''}
                    options={videoLibraries.map((l) => ({ value: l.name, label: l.name }))}
                    onChange={setLibrary}
                  />
                </View>
              )}
              <View className="flex-1">
                <Select title="Type" value={category} options={mediaOptions} onChange={setCategory} />
              </View>
            </View>
          </View>
          {error && <ErrorText>{(error as Error).message}</ErrorText>}
          {!query &&
            action &&
            (forYou.data
              ? forYou.data.shelves
                  .filter((row) => ofMediaType(row.results, category).length > 0)
                  .map((row) => <DiscoverShelf key={row.key} title={row.name} results={ofMediaType(row.results, category)} />)
              : forYou.isPending && <RowSkeleton />)}
          {!query && data && results.length > 0 && <Text className="font-sans text-[17px] font-semibold tracking-tight text-ink">Popular right now</Text>}
          {!data && !error && <PosterGridSkeleton count={9} />}
          {data && results.length === 0 && <Empty title={query ? `Nothing called “${query}”${category ? ' of this type' : ''}` : 'No titles of this type'} />}
          {data && (
            <View className="flex-row flex-wrap" style={{ gap: 12, rowGap: 22 }}>
              {results.map((r) => (
                <ResultCard key={`${r.provider}${r.category}${r.id}`} r={r} width={column} action={action} onPick={() => setPicked(r)} />
              ))}
            </View>
          )}
          <PickSheet r={picked} action={action} onClose={() => setPicked(null)} />
        </>
      )}
      {!allowed && !query && !recent?.length && (
        <Empty title="Search your libraries">{features && me ? "You can't request shows" : 'Titles, episodes and music, as you type.'}</Empty>
      )}
    </>
  )
}
