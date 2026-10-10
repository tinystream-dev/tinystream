// SPDX-License-Identifier: AGPL-3.0-or-later
import { useInfiniteQuery, useQuery } from '@tanstack/react-query'
import { useRouter } from 'expo-router'
import { useDeferredValue, useState } from 'react'
import { Pressable, Text, View, useWindowDimensions } from 'react-native'
import { Page } from '../components/Page'
import { Empty, Input, Segmented, Select, Spinner, Button, ErrorText } from '../components/ui'
import { useApi } from '../session'
import { useMusic } from './context'
import { Albums, Artists, Songs, Playlists } from './queries'
import { Artwork, PlaylistPicker, TrackRow } from './components'
import type { AlbumSort } from '../gql/graphql'

export function MusicLibrary() {
  const [tab, setTab] = useState<'albums' | 'artists' | 'songs' | 'playlists'>('albums')
  return (
    <Library
      key={tab}
      tab={tab}
      header={
        <Segmented
          value={tab}
          onChange={setTab}
          options={['albums', 'artists', 'songs', 'playlists'].map((value) => ({ value: value as typeof tab, label: value[0].toUpperCase() + value.slice(1) }))}
        />
      }
    />
  )
}
function Library({ tab, header }: { tab: 'albums' | 'artists' | 'songs' | 'playlists'; header: React.ReactNode }) {
  const { width } = useWindowDimensions()
  const tile = Math.floor((width - 56) / 2)
  const api = useApi(),
    router = useRouter(),
    { music } = useMusic()
  const [q, setQ] = useState(''),
    query = useDeferredValue(q),
    [sort, setSort] = useState<AlbumSort>('NAME'),
    [making, setMaking] = useState(false)
  const albums = useInfiniteQuery({
    queryKey: ['music', 'albums', sort],
    initialPageParam: 0,
    queryFn: async ({ pageParam }) => (await api.request(Albums, { sort, offset: pageParam })).albums,
    getNextPageParam: (page, pages) => (page.length === 60 ? pages.flat().length : undefined),
    enabled: tab === 'albums',
  })
  const songs = useInfiniteQuery({
    queryKey: ['music', 'songs', query],
    initialPageParam: 0,
    queryFn: async ({ pageParam }) => (await api.request(Songs, { query, offset: pageParam })).songs,
    getNextPageParam: (page, pages) => (page.length === 60 ? pages.flat().length : undefined),
    enabled: tab === 'songs',
  })
  const artists = useQuery({
    queryKey: ['music', 'artists'],
    queryFn: async () => (await api.request(Artists, { library: null })).artists,
    enabled: tab === 'artists',
  })
  const playlists = useQuery({ queryKey: ['music', 'playlists'], queryFn: async () => (await api.request(Playlists)).playlists, enabled: tab === 'playlists' })
  const tracks = songs.data?.pages.flat() ?? []
  const rows: { id: number; name: string; caption: string; cover: string | null | undefined; kind: 'album' | 'artist' | 'playlist' }[] | undefined =
    tab === 'albums'
      ? albums.data?.pages.flat().map((a) => ({ id: a.id, name: a.name, caption: a.artist, cover: a.cover, kind: 'album' as const }))
      : tab === 'artists'
        ? artists.data?.map((a) => ({ id: a.id, name: a.name, caption: `${a.albumCount} albums`, cover: a.cover, kind: 'artist' as const }))
        : playlists.data?.map((p) => ({ id: p.id, name: p.name, caption: `${p.trackCount} songs`, cover: p.covers[0], kind: 'playlist' as const }))
  const controls = (
    <View className="mb-4 gap-3">
      {header}
      {(tab === 'albums' ? albums.error : tab === 'artists' ? artists.error : tab === 'songs' ? songs.error : playlists.error) && (
        <ErrorText>Couldn't load the music library. Pull down to retry.</ErrorText>
      )}
      {tab === 'songs' && <Input placeholder="Find a song" value={q} onChangeText={setQ} />}
      {tab === 'albums' && (
        <Select
          value={sort}
          onChange={setSort}
          options={[
            { value: 'NAME', label: 'A–Z' },
            { value: 'ARTIST', label: 'Artist' },
            { value: 'NEWEST', label: 'Added' },
            { value: 'YEAR', label: 'Year' },
            { value: 'RECENT', label: 'Played' },
          ]}
        />
      )}
      {tab === 'playlists' && (
        <>
          <Button onPress={() => setMaking(true)}>New playlist</Button>
          <PlaylistPicker tracks={[]} open={making} close={() => setMaking(false)} />
        </>
      )}
    </View>
  )
  if (tab === 'songs')
    return (
      <Page
        title="Music"
        header={controls}
        onRefresh={() => songs.refetch()}
        list={{
          data: tracks,
          contentContainerStyle: { paddingHorizontal: 20 },
          keyExtractor: (t) => String(t.id),
          renderItem: ({ item, index }) => <TrackRow track={item} onPress={() => void music.play(tracks, index)} />,
          onEndReached: () => {
            if (songs.hasNextPage && !songs.isFetchingNextPage) void songs.fetchNextPage()
          },
          ListEmptyComponent: songs.isPending ? <Spinner /> : <Empty title="No songs here yet" />,
        }}
      />
    )
  return (
    <Page
      title="Music"
      header={controls}
      onRefresh={() => (tab === 'albums' ? albums.refetch() : tab === 'artists' ? artists.refetch() : playlists.refetch())}
      list={{
        data: rows ?? [],
        contentContainerStyle: { paddingHorizontal: 20 },
        numColumns: 2,
        keyExtractor: (r) => String(r.id),
        columnWrapperStyle: { gap: 16 },
        renderItem: ({ item }) => (
          <Pressable className="mb-5 flex-1" onPress={() => router.push({ pathname: `/${item.kind}/[id]`, params: { id: item.id } })}>
            <View className="items-center">
              <Artwork src={item.cover} size={tile} />
            </View>
            <Text className="font-sans mt-2 text-sm font-medium text-ink" numberOfLines={1}>
              {item.name}
            </Text>
            <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
              {item.caption}
            </Text>
          </Pressable>
        ),
        onEndReached: () => {
          if (tab === 'albums' && albums.hasNextPage && !albums.isFetchingNextPage) void albums.fetchNextPage()
        },
        ListEmptyComponent: !rows ? <Spinner /> : <Empty title="Nothing here yet" />,
      }}
    />
  )
}
