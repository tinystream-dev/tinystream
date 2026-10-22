// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Check, ExternalLink, RefreshCw, Undo2 } from 'lucide-react-native'
import { memo, useEffect, useState } from 'react'
import { Text, View } from 'react-native'
import { Squircle } from '../effects/Squircle'
import { Tilt } from '../effects/Tilt'
import { graphql } from '../gql'
import { useGo } from '../nav'
import { type Card, titleQuery, useMe } from '../queries'
import { useApi } from '../session'
import { toast, toastError } from './Feedback'
import { Img } from './Img'
import { Menu } from './Menu'

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

/** Initials on a quiet surface, for titles without artwork. */
function Fallback({ title }: { title: string }) {
  return (
    <View className="flex-1 justify-end bg-panel p-3">
      <Text className="font-sans text-[15px] font-medium leading-5 text-ink-2" numberOfLines={3}>
        {title}
      </Text>
    </View>
  )
}

/** A title's poster (web's Poster): `caption` replaces the watched count under it. */
export const Poster = memo(function Poster({ card, width, caption }: { card: Card; width: number; caption?: string }) {
  const go = useGo()
  const api = useApi()
  const qc = useQueryClient()
  const [broken, setBroken] = useState(false)
  const [menu, setMenu] = useState(false)
  useEffect(() => setBroken(false), [card.poster])
  const done = card.videoCount > 0 && card.watchedCount >= card.videoCount
  const unwatched = card.kind === 'SHOW' ? card.videoCount - card.watchedCount : 0
  const fresh = card.freshCount > 0 && !done
  const badge = fresh ? (card.kind === 'SHOW' && card.freshCount > 1 ? `${card.freshCount} new` : 'New') : card.kind === 'SHOW' && card.watchedCount > 0 && unwatched > 0 ? String(unwatched) : null
  return (
    <View style={{ width }}>
      <Tilt
        radius={14}
        style={{ width, aspectRatio: 2 / 3 }}
        onPressIn={() => void qc.prefetchQuery(titleQuery(api, card.id))}
        onPress={() => go(`title/${card.id}`)}
        onLongPress={() => setMenu(true)}
      >
        <View className="flex-1 bg-raised">
          {card.poster && !broken ? <Img src={card.poster} style={{ flex: 1 }} onError={() => setBroken(true)} /> : <Fallback title={card.name} />}
        </View>
        {card.progress !== null && !done && (
          <View className="absolute bottom-2 left-2 right-2 h-1 overflow-hidden rounded-full bg-media-shade/50">
            <View className="h-full bg-media-ink" style={{ width: `${Math.max(4, card.progress * 100)}%` }} />
          </View>
        )}
        {badge && (
          <View className="absolute right-1.5 top-1.5 flex-row items-center gap-1 rounded-md bg-media-shade/60 px-1.5 py-0.5">
            {fresh && <View className="h-1.5 w-1.5 rounded-full bg-warn" />}
            <Text className="font-sans text-2xs font-medium text-media-ink">{badge}</Text>
          </View>
        )}
      </Tilt>
      <Text className="font-sans mt-2 text-[13px] font-medium text-ink" numberOfLines={1}>
        {card.name}
      </Text>
      <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
        {caption ??
          (done
            ? 'Watched'
            : card.kind === 'SHOW'
              ? card.watchedCount > 0
                ? `${card.watchedCount} of ${card.videoCount} watched`
                : `${card.videoCount} episode${card.videoCount === 1 ? '' : 's'}`
              : (card.year ?? 'Movie'))}
      </Text>
      <PosterMenu card={card} done={done} open={menu} onClose={() => setMenu(false)} />
    </View>
  )
})

/** A poster's long-press menu: what the title page does that makes sense from a card. */
function PosterMenu({ card, done, open, onClose }: { card: Card; done: boolean; open: boolean; onClose: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const go = useGo()
  const can = useMe()?.permissions
  const settle = () => {
    void qc.invalidateQueries({ queryKey: ['item', card.id] })
    void qc.invalidateQueries({ queryKey: ['home'] })
    void qc.invalidateQueries({ queryKey: ['library'] })
  }
  const watch = useMutation({
    mutationFn: (watched: boolean) => api.request(SetTitleWatched, { id: card.id, watched }),
    onSuccess: (_, watched) => toast({ title: watched ? `Marked ${card.name} watched` : `Marked ${card.name} not watched`, image: card.poster }),
    onError: toastError,
    onSettled: settle,
  })
  const refresh = useMutation({ mutationFn: () => api.request(RefreshTitle, { id: card.id }), onError: toastError })
  return (
    <Menu
      open={open}
      onClose={onClose}
      header={
        <View className="flex-row items-center gap-3 px-1">
          <Squircle radius={8} className="bg-panel" style={{ width: 40, aspectRatio: 2 / 3 }}>
            {card.poster && <Img src={card.poster} style={{ flex: 1 }} />}
          </Squircle>
          <View className="flex-1">
            <Text className="font-sans text-[15px] font-semibold text-ink" numberOfLines={2}>
              {card.name}
            </Text>
            <Text className="font-sans text-xs text-ink-3">{[card.year, card.library].filter(Boolean).join(' · ')}</Text>
          </View>
        </View>
      }
      items={[
        { label: 'Open', icon: (c) => <ExternalLink size={18} color={c} />, onPress: () => go(`title/${card.id}`) },
        {
          label: done ? 'Mark not watched' : 'Mark watched',
          icon: (c) => (done ? <Undo2 size={18} color={c} /> : <Check size={18} color={c} />),
          onPress: () => watch.mutate(!done),
        },
        can?.editMetadata && { label: 'Refresh details', icon: (c) => <RefreshCw size={18} color={c} />, onPress: () => refresh.mutate() },
      ]}
    />
  )
}
