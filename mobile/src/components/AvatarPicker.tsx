// SPDX-License-Identifier: AGPL-3.0-or-later
// A picture you can change (web's AvatarPicker): picked from the phone and
// cropped square by Android's own cropper, then uploaded.

import { useMutation, useQueryClient } from '@tanstack/react-query'
import * as ImagePicker from 'expo-image-picker'
import { Camera, ImageUp, Trash2 } from 'lucide-react-native'
import { Pressable, View } from 'react-native'
import { haptic } from '../../modules/haptics'
import { graphql } from '../gql'
import type { UploadFile } from '../lib/graphql'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { Avatar } from './Avatar'
import { toast, toastError } from './Feedback'
import { Button } from './ui'

const SetAvatar = graphql(`
  mutation SetAvatar($image: Upload!, $userId: Int) {
    setAvatar(image: $image, userId: $userId) {
      id
      avatar
    }
  }
`)

const RemoveAvatar = graphql(`
  mutation RemoveAvatar($userId: Int) {
    removeAvatar(userId: $userId) {
      id
      avatar
    }
  }
`)

/** `userId` is whose picture it is, when it isn't yours. */
export function AvatarPicker({ user, userId, size = 72 }: { user: { username: string; avatar?: string | null }; userId?: number; size?: number }) {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['auth'] })
    void qc.invalidateQueries({ queryKey: ['users'] })
  }
  const save = useMutation({
    mutationFn: (image: UploadFile) => api.request(SetAvatar, { image, userId }),
    onSuccess: () => (refresh(), toast({ title: 'Picture changed', tone: 'ok' })),
    onError: toastError,
  })
  const remove = useMutation({ mutationFn: () => api.request(RemoveAvatar, { userId }), onSuccess: refresh, onError: toastError })
  const pick = async () => {
    const r = await ImagePicker.launchImageLibraryAsync({ mediaTypes: ['images'], allowsEditing: true, aspect: [1, 1], quality: 0.88 })
    const a = r.canceled ? null : r.assets[0]
    if (!a) return
    const type = a.mimeType ?? 'image/jpeg'
    save.mutate({ uri: a.uri, name: a.fileName ?? `avatar.${type.split('/')[1] ?? 'jpg'}`, type })
  }
  return (
    <View className="flex-row items-center gap-4">
      <Pressable
        accessibilityLabel="Change picture"
        onPress={() => {
          haptic('press')
          void pick()
        }}
      >
        <Avatar user={user} size={size} />
        <View className="absolute bottom-0 right-0 h-7 w-7 items-center justify-center rounded-full border-2 border-raised bg-panel">
          <Camera size={14} color={tokens.ink} />
        </View>
      </Pressable>
      <View className="items-start gap-1">
        <Button size="sm" disabled={save.isPending} onPress={() => void pick()} icon={<ImageUp size={14} color={tokens.ink} />}>
          {save.isPending ? 'Saving…' : user.avatar ? 'Change picture' : 'Upload a picture'}
        </Button>
        {user.avatar && (
          <Button size="sm" variant="plain" onPress={() => remove.mutate()} disabled={remove.isPending} icon={<Trash2 size={14} color={tokens['ink-2']} />}>
            Remove
          </Button>
        )}
      </View>
    </View>
  )
}
