// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQueryClient } from '@tanstack/react-query'
import { ImageUp, RotateCcw } from 'lucide-react'
import { useRef } from 'react'
import { graphql } from '../gql'
import { request } from '../lib/api'
import type { TitleArtwork } from '../gql/graphql'
import { toastError } from './feedback'
import { IconButton } from './ui'

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

export type Target = { titleId: number; kind: TitleArtwork } | { videoId: number }

/**
 * Changes or resets a picture: `pick` opens the file chooser and `input` is
 * the hidden field it uses, which has to stay rendered until a file is chosen
 * (so not inside a menu that closes on click).
 */
export function useArtwork(target: Target) {
  const input = useRef<HTMLInputElement>(null)
  const qc = useQueryClient()
  const save = useMutation({
    mutationFn: async (image: Blob | null) => 'titleId' in target
      ? request(SetTitleArtwork, { id: target.titleId, kind: target.kind, image })
      : request(SetVideoArtwork, { videoId: target.videoId, image }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['item'] })
      void qc.invalidateQueries({ queryKey: ['home'] })
      void qc.invalidateQueries({ queryKey: ['library'] })
      void qc.invalidateQueries({ queryKey: ['playback'] })
    },
    onError: toastError,
  })
  return {
    pending: save.isPending,
    pick: () => input.current?.click(),
    reset: () => save.mutate(null),
    input: (
      <input ref={input} type="file" accept="image/png,image/jpeg,image/webp,image/gif" hidden onChange={(e) => {
        const file = e.target.files?.[0]
        e.target.value = ''
        if (!file) return
        if (file.size > 8 * 1024 * 1024) {
          toastError(new Error('That picture is too big (8 MB at most).'))
          return
        }
        save.mutate(file)
      }} />
    ),
  }
}

export function ArtworkEditor({ target, custom, label }: { target: Target; custom: boolean; label: string }) {
  const art = useArtwork(target)
  return (
    <div className="flex items-center gap-1">
      <IconButton label={`Change ${label}`} disabled={art.pending} onClick={art.pick}>
        <ImageUp className="size-4.5" />
      </IconButton>
      {custom && (
        <IconButton label={`Reset ${label}`} disabled={art.pending} onClick={art.reset}>
          <RotateCcw className="size-4" />
        </IconButton>
      )}
      {art.input}
    </div>
  )
}
