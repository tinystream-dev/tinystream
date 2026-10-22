// SPDX-License-Identifier: AGPL-3.0-or-later
// Changing a title's or an episode's artwork (web's ArtworkEditor): a picture
// from the phone, uploaded as it is, or back to the provider's.

import { useMutation, useQueryClient } from '@tanstack/react-query'
import * as ImagePicker from 'expo-image-picker'
import { ImageUp, RotateCcw } from 'lucide-react-native'
import { Text, View } from 'react-native'
import { graphql } from '../gql'
import type { TitleArtwork } from '../gql/graphql'
import type { UploadFile } from '../lib/graphql'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { toast, toastError } from './Feedback'
import { Sheet } from './Sheet'
import { Button } from './ui'

const SetTitleArtwork = graphql(`
  mutation SetTitleArtwork($id: Int!, $kind: TitleArtwork!, $image: Upload) {
    setTitleArtwork(id: $id, kind: $kind, image: $image) {
      id
    }
  }
`)

const SetVideoArtwork = graphql(`
  mutation SetVideoArtwork($videoId: Int!, $image: Upload) {
    setVideoArtwork(videoId: $videoId, image: $image) {
      id
    }
  }
`)

export type ArtworkTarget = { target: { titleId: number; kind: TitleArtwork } | { videoId: number }; custom: boolean; label: string }

const MAX = 8 * 1024 * 1024

/** Picks a picture from the phone; null when nothing was picked. */
async function pick(): Promise<UploadFile | null> {
  const r = await ImagePicker.launchImageLibraryAsync({ mediaTypes: ['images'], quality: 1 })
  const a = r.canceled ? null : r.assets[0]
  if (!a) return null
  if (a.fileSize && a.fileSize > MAX) throw new Error('That picture is too big (8 MB at most).')
  const type = a.mimeType ?? 'image/jpeg'
  return { uri: a.uri, name: a.fileName ?? `artwork.${type.split('/')[1] ?? 'jpg'}`, type }
}

export function ArtworkSheet({ open, onClose, items }: { open: boolean; onClose: () => void; items: ArtworkTarget[] }) {
  return (
    <Sheet open={open} onClose={onClose}>
      <Text className="font-sans mb-3 mt-1 text-lg font-semibold text-ink">Artwork</Text>
      <View className="gap-4">
        {items.map((i) => (
          <Editor key={i.label} item={i} onDone={onClose} />
        ))}
      </View>
    </Sheet>
  )
}

function Editor({ item, onDone }: { item: ArtworkTarget; onDone: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const t = item.target
  const save = useMutation({
    mutationFn: async (image: UploadFile | null) =>
      'titleId' in t ? api.request(SetTitleArtwork, { id: t.titleId, kind: t.kind, image }) : api.request(SetVideoArtwork, { videoId: t.videoId, image }),
    onSuccess: (_, image) => {
      for (const key of ['item', 'home', 'library', 'playback']) void qc.invalidateQueries({ queryKey: [key] })
      toast({ title: image ? `Changed the ${item.label}` : `Reset the ${item.label}`, tone: 'ok' })
      onDone()
    },
    onError: toastError,
  })
  const change = async () => {
    try {
      const file = await pick()
      if (file) save.mutate(file)
    } catch (e) {
      toastError(e)
    }
  }
  return (
    <View className="flex-row items-center gap-2">
      <Button className="flex-1" disabled={save.isPending} onPress={() => void change()} icon={<ImageUp size={16} color={tokens.ink} />}>
        {save.isPending ? 'Saving…' : `Change ${item.label}`}
      </Button>
      {item.custom && (
        <Button variant="plain" disabled={save.isPending} onPress={() => save.mutate(null)} icon={<RotateCcw size={15} color={tokens['ink-2']} />}>
          Reset
        </Button>
      )}
    </View>
  )
}
