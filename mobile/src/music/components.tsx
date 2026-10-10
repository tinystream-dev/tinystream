// SPDX-License-Identifier: AGPL-3.0-or-later
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useRouter } from 'expo-router'
import { MoreHorizontal, Star as StarIcon } from 'lucide-react-native'
import { useState } from 'react'
import { Pressable, Text, View } from 'react-native'
import { haptic } from '../../modules/haptics'
import { Img } from '../components/Img'
import { Menu } from '../components/Menu'
import { Sheet } from '../components/Sheet'
import { Button, IconButton, Input } from '../components/ui'
import { toastError } from '../components/Feedback'
import { Tilt } from '../effects/Tilt'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { cover, duration, Star, type MusicTrack } from './api'
import { useMusic } from './context'
import { CreatePlaylist, Playlist, Playlists, UpdatePlaylist } from './queries'

export function Artwork({ src, size = 64, gyro = false }: { src?: string | null; size?: number; gyro?: boolean }) {
  return (
    <Tilt gyro={gyro} radius={size > 100 ? 24 : 12} style={{ width: size, height: size }}>
      <Img src={cover(src, size)} style={{ width: size, height: size }} contentFit="cover" />
    </Tilt>
  )
}
export function StarButton({ kind, id, starred }: { kind: 'TRACK' | 'ALBUM' | 'ARTIST'; id: number; starred: boolean }) {
  const api = useApi(),
    qc = useQueryClient(),
    { tokens } = useTheme(),
    player = useMusic()
  const mutation = useMutation({
    mutationFn: () => api.request(Star, { kind, id, starred: !starred }),
    onSuccess: () => {
      const track = player.current()?.track
      if (kind === 'TRACK' && track?.id === id) player.updateTrack({ ...track, starred: !starred })
      return qc.invalidateQueries({ queryKey: ['music'] })
    },
    onError: toastError,
  })
  return (
    <IconButton label={starred ? 'Unstar' : 'Star'} onPress={() => mutation.mutate()}>
      <StarIcon size={20} color={starred ? tokens.accent : tokens['ink-2']} fill={starred ? tokens.accent : 'transparent'} />
    </IconButton>
  )
}
export function TrackRow({
  track,
  onPress,
  onRemove,
  onMoveUp,
  onMoveDown,
}: {
  track: MusicTrack
  onPress: () => void
  onRemove?: () => void
  onMoveUp?: () => void
  onMoveDown?: () => void
}) {
  const [menu, setMenu] = useState(false),
    [adding, setAdding] = useState(false)
  const { tokens } = useTheme(),
    { music } = useMusic(),
    router = useRouter()
  return (
    <>
      <Pressable
        accessibilityLabel={`Play ${track.title}`}
        onPress={onPress}
        onLongPress={() => {
          haptic('longPressOpen')
          setMenu(true)
        }}
        className="flex-row items-center gap-3 py-2"
      >
        <Artwork src={track.cover} size={48} />
        <View className="flex-1">
          <Text className="font-sans text-sm font-medium text-ink" numberOfLines={1}>
            {track.title}
          </Text>
          <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
            {track.artist} · {duration(track.duration)}
          </Text>
        </View>
        <IconButton label="Song menu" onPress={() => setMenu(true)}>
          <MoreHorizontal size={20} color={tokens['ink-2']} />
        </IconButton>
      </Pressable>
      <Menu
        open={menu}
        onClose={() => setMenu(false)}
        items={[
          { label: 'Play next', onPress: () => music.playNext([track]) },
          { label: 'Add to queue', onPress: () => music.add([track]) },
          { label: 'Add to playlist', onPress: () => setAdding(true) },
          track.albumId != null && { label: 'Go to album', onPress: () => router.push({ pathname: '/album/[id]', params: { id: track.albumId! } }) },
          onMoveUp && { label: 'Move up', onPress: onMoveUp },
          onMoveDown && { label: 'Move down', onPress: onMoveDown },
          onRemove && { label: 'Remove', danger: true, onPress: onRemove },
        ]}
        header={
          <View className="flex-row items-center gap-3 px-3">
            <Text className="font-sans flex-1 text-lg text-ink">{track.title}</Text>
            <StarButton kind="TRACK" id={track.id} starred={track.starred} />
          </View>
        }
      />
      <PlaylistPicker tracks={[track]} open={adding} close={() => setAdding(false)} />
    </>
  )
}
export function PlaylistPicker({ tracks, open, close }: { tracks: MusicTrack[]; open: boolean; close: () => void }) {
  const api = useApi(),
    qc = useQueryClient()
  const [name, setName] = useState('')
  const { data } = useQuery({ queryKey: ['music', 'playlists'], queryFn: () => api.request(Playlists), enabled: open })
  const add = async (id?: number) => {
    try {
      if (id != null) {
        const found = await api.request(Playlist, { id })
        await api.request(UpdatePlaylist, { id, input: { tracks: [...(found.playlist?.tracks.map((t) => t.id) ?? []), ...tracks.map((t) => t.id)] } })
      } else await api.request(CreatePlaylist, { name: name.trim(), tracks: tracks.map((t) => t.id) })
      await qc.invalidateQueries({ queryKey: ['music'] })
      setName('')
      close()
    } catch (e) {
      toastError(e)
    }
  }
  return (
    <Sheet open={open} onClose={close}>
      <View className="gap-3">
        <Text className="font-sans text-xl text-ink">Add to playlist</Text>
        <Input placeholder="New playlist name" value={name} onChangeText={setName} />
        <Button disabled={!name.trim()} onPress={() => void add()}>
          Create playlist
        </Button>
        {data?.playlists
          .filter((p) => p.mine)
          .map((p) => (
            <Button key={p.id} onPress={() => void add(p.id)}>
              {p.name}
            </Button>
          ))}
      </View>
    </Sheet>
  )
}
