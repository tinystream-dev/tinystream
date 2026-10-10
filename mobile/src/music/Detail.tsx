// SPDX-License-Identifier: AGPL-3.0-or-later
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useLocalSearchParams, useRouter } from 'expo-router'
import { useState } from 'react'
import { Text, View } from 'react-native'
import { Page } from '../components/Page'
import { ask, toastError } from '../components/Feedback'
import { Button, Empty, Input, Spinner, Toggle, ErrorText } from '../components/ui'
import { useApi } from '../session'
import { rgba } from '../lib/tint'
import { Album, Artist, Playlist, UpdatePlaylist, DeletePlaylist } from './queries'
import { Artwork, StarButton, TrackRow } from './components'
import { useMusic } from './context'

export function MusicDetail({ kind }: { kind: 'album' | 'artist' | 'playlist' }) {
  const { id: raw } = useLocalSearchParams<{ id: string }>(),
    id = Number(raw),
    api = useApi(),
    qc = useQueryClient(),
    router = useRouter(),
    { music } = useMusic()
  const [name, setName] = useState<string | null>(null)
  const album = useQuery({ queryKey: ['music', 'album', id], queryFn: async () => (await api.request(Album, { id })).album, enabled: kind === 'album' })
  const artist = useQuery({ queryKey: ['music', 'artist', id], queryFn: async () => (await api.request(Artist, { id })).artist, enabled: kind === 'artist' })
  const playlist = useQuery({
    queryKey: ['music', 'playlist', id],
    queryFn: async () => (await api.request(Playlist, { id })).playlist,
    enabled: kind === 'playlist',
  })
  const data = kind === 'album' ? album.data : kind === 'artist' ? artist.data : playlist.data
  const tracks = kind === 'artist' ? (artist.data?.topTracks ?? []) : kind === 'album' ? (album.data?.tracks ?? []) : (playlist.data?.tracks ?? [])
  const update = async (input: { name?: string; public?: boolean; tracks?: number[] }) => {
    try {
      await api.request(UpdatePlaylist, { id, input })
      await qc.invalidateQueries({ queryKey: ['music'] })
    } catch (e) {
      toastError(e)
    }
  }
  const result = kind === 'album' ? album : kind === 'artist' ? artist : playlist
  const move = (from: number, to: number) => {
    const ordered = tracks.map((t) => t.id)
    ordered.splice(to, 0, ordered.splice(from, 1)[0])
    void update({ tracks: ordered })
  }
  if (result.isError)
    return (
      <Page title="Music">
        <ErrorText>{result.error.message}</ErrorText>
        <Button onPress={() => void result.refetch()}>Retry</Button>
      </Page>
    )
  if (data === undefined)
    return (
      <Page title="Music">
        <Spinner />
      </Page>
    )
  if (!data)
    return (
      <Page title="Music">
        <Empty title="This isn't here anymore" />
      </Page>
    )
  const tint = kind === 'artist' ? artist.data?.coverTint : kind === 'playlist' ? tracks[0]?.coverTint : album.data?.coverTint
  const hero = (
    <View className="items-center gap-4 pb-6 pt-20" style={{ backgroundColor: tint ? rgba(tint, 0.15) : undefined }}>
      <Artwork src={kind === 'playlist' ? playlist.data?.covers[0] : (album.data?.cover ?? artist.data?.cover)} size={240} gyro />
      <Text className="font-sans px-5 text-center text-3xl font-semibold text-ink">{data.name}</Text>
      <Text className="font-sans text-sm text-ink-2">{album.data?.artist ?? `${tracks.length} songs`}</Text>
      <View className="flex-row gap-3">
        <Button variant="primary" onPress={() => void music.play(tracks)}>
          Play
        </Button>
        <Button onPress={() => void music.play(tracks, 0, true)}>Shuffle</Button>
        {kind !== 'playlist' && (
          <StarButton kind={kind === 'album' ? 'ALBUM' : 'ARTIST'} id={id} starred={album.data?.starred ?? artist.data?.starred ?? false} />
        )}
      </View>
    </View>
  )
  const header = (
    <View className="gap-3 mb-3">
      {kind === 'playlist' && playlist.data?.mine && (
        <>
          <Input
            value={name ?? data.name}
            onChangeText={setName}
            onSubmitEditing={() => {
              if (name?.trim()) void update({ name: name.trim() })
              setName(null)
            }}
            placeholder="Playlist name"
          />
          <Toggle value={playlist.data.public} onChange={(value) => void update({ public: value })} label="Public playlist" />
          <Button
            variant="danger"
            onPress={() =>
              void ask({ title: 'Delete playlist?', body: data.name, confirm: 'Delete', danger: true })
                .then(async (yes) => {
                  if (yes) {
                    await api.request(DeletePlaylist, { id })
                    await qc.invalidateQueries({ queryKey: ['music'] })
                    router.back()
                  }
                })
                .catch(toastError)
            }
          >
            Delete playlist
          </Button>
        </>
      )}
      {artist.data &&
        [...artist.data.albums, ...artist.data.appearsOn].map((a) => (
          <Button key={a.id} onPress={() => router.push({ pathname: '/album/[id]', params: { id: a.id } })}>
            {a.name}
          </Button>
        ))}
    </View>
  )
  return (
    <Page
      title={data.name}
      hero={hero}
      header={header}
      onRefresh={() => (kind === 'album' ? album.refetch() : kind === 'artist' ? artist.refetch() : playlist.refetch())}
      list={{
        data: tracks,
        contentContainerStyle: { paddingHorizontal: 20 },
        keyExtractor: (t, i) => `${t.id}:${i}`,
        renderItem: ({ item, index }) => (
          <TrackRow
            track={item}
            onPress={() => void music.play(tracks, index)}
            onMoveUp={playlist.data?.mine && index > 0 ? () => move(index, index - 1) : undefined}
            onMoveDown={playlist.data?.mine && index < tracks.length - 1 ? () => move(index, index + 1) : undefined}
            onRemove={playlist.data?.mine ? () => void update({ tracks: tracks.filter((_, i) => i !== index).map((t) => t.id) }) : undefined}
          />
        ),
      }}
    />
  )
}
