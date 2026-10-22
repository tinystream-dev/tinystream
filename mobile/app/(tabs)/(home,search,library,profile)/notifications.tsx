// SPDX-License-Identifier: AGPL-3.0-or-later
// The inbox (web's components/Notifications.tsx): new, then earlier. Tap one
// to go where it points; swipe it away to delete it.

import { relative } from '@tinystream/shared/downloads'
import { BellOff, CircleCheck, CirclePlay, CircleX, Inbox as InboxIcon, type LucideIcon, Radio, Scissors, Send, Trash2, Users } from 'lucide-react-native'
import { useState } from 'react'
import { Pressable, Text, View } from 'react-native'
import Animated, { LinearTransition } from 'react-native-reanimated'
import { haptic } from '../../../modules/haptics'
import { Avatar } from '../../../src/components/Avatar'
import { ask } from '../../../src/components/Feedback'
import { Img } from '../../../src/components/Img'
import { Page } from '../../../src/components/Page'
import { ListSkeleton, useArrived } from '../../../src/components/Skeleton'
import { SwipeRow } from '../../../src/components/SwipeRow'
import { Button } from '../../../src/components/ui'
import { Squircle } from '../../../src/effects/Squircle'
import { useMotion } from '../../../src/effects/motion'
import type { NotificationKind } from '../../../src/gql/graphql'
import { useFollow } from '../../../src/nav'
import { useInbox, useMarkRead, useRemove } from '../../../src/notifications'
import { type Notice, useNow } from '../../../src/queries'
import type { Tokens } from '@tinystream/shared/theme'
import { useTheme } from '../../../src/theme/ThemeProvider'

const KINDS: Record<NotificationKind, { icon: LucideIcon; tint: keyof Tokens }> = {
  AIRED: { icon: Radio, tint: 'warn-soft' },
  READY: { icon: CirclePlay, tint: 'info' },
  INVITE: { icon: Users, tint: 'social' },
  REQUEST: { icon: InboxIcon, tint: 'ink-2' },
  REQUEST_APPROVED: { icon: CircleCheck, tint: 'ok' },
  REQUEST_DECLINED: { icon: CircleX, tint: 'danger' },
  CLIP: { icon: Send, tint: 'highlight' },
  CLIP_READY: { icon: Scissors, tint: 'highlight' },
  OTHER: { icon: InboxIcon, tint: 'ink-2' },
}

export default function Notifications() {
  const { data, refetch } = useInbox()
  const read = useMarkRead()
  const remove = useRemove()
  const follow = useFollow()
  const now = useNow(30_000)
  const motion = useMotion()
  const { tokens } = useTheme()
  useArrived(!!data)
  const items = data?.items ?? []
  const fresh = items.filter((n) => n.readAt == null)
  const earlier = items.filter((n) => n.readAt != null)
  const open = (n: Notice) => {
    if (n.readAt == null) read.mutate([n.id])
    follow(n.link)
  }
  return (
    <Page
      title="Notifications"
      onRefresh={() => refetch()}
      right={
        data && data.unread > 0 ? (
          <Button size="sm" variant="plain" onPress={() => read.mutate(undefined)}>
            Mark all read
          </Button>
        ) : items.length > 0 ? (
          <Button
            size="sm"
            variant="plain"
            onPress={async () => (await ask({ title: 'Clear every notification?', confirm: 'Clear', danger: true })) && remove.mutate('all')}
          >
            Clear
          </Button>
        ) : undefined
      }
    >
      {!data && <ListSkeleton rows={5} height={64} />}
      {data && items.length === 0 && (
        <Animated.View style={motion.unfold} className="items-center gap-2 py-20">
          <Animated.View style={motion.hush}>
            <BellOff size={22} color={tokens['ink-3']} />
          </Animated.View>
          <Text className="font-sans text-sm text-ink-2">Nothing yet</Text>
        </Animated.View>
      )}
      {[
        { title: 'New', list: fresh },
        { title: 'Earlier', list: earlier },
      ]
        .filter((g) => g.list.length > 0)
        .map((g) => (
          <View key={g.title}>
            {fresh.length > 0 && earlier.length > 0 && <Text className="font-sans mb-1 px-1 text-2xs font-medium uppercase tracking-wider text-ink-3">{g.title}</Text>}
            {g.list.map((n) => (
              <Animated.View key={n.id} layout={LinearTransition.duration(220)}>
                <SwipeRow right={{ label: 'Delete', icon: <Trash2 size={20} color={tokens.canvas} />, color: tokens.danger, text: tokens.canvas, run: () => remove.mutate(n.id) }}>
                  <Row n={n} now={now} onOpen={() => open(n)} />
                </SwipeRow>
              </Animated.View>
            ))}
          </View>
        ))}
    </Page>
  )
}

function Row({ n, now, onOpen }: { n: Notice; now: number; onOpen: () => void }) {
  const { tokens } = useTheme()
  const unread = n.readAt == null
  const expired = n.expiresAt != null && n.expiresAt * 1000 < now
  const k = KINDS[n.kind]
  return (
    <Pressable
      onPress={() => {
        haptic('press')
        onOpen()
      }}
    >
      {({ pressed }) => (
        <Squircle radius={14} className={pressed ? 'bg-press' : 'bg-canvas'} style={{ flexDirection: 'row', gap: 12, paddingHorizontal: 8, paddingVertical: 10 }}>
          <Thumb n={n} />
          <View className="min-w-0 flex-1">
            <Text className={`font-sans text-[14px] leading-5 ${unread ? 'font-medium text-ink' : 'text-ink-2'}`}>{n.title}</Text>
            {n.body && (
              <Text className="font-sans mt-0.5 text-xs leading-4 text-ink-3" numberOfLines={2}>
                {n.body}
              </Text>
            )}
            <View className="mt-1 flex-row items-center gap-1.5">
              <k.icon size={12} color={tokens[k.tint]} />
              <Text className="font-sans text-2xs text-ink-3">
                {relative(n.createdAt, now)}
                {expired && n.kind === 'INVITE' && ' · expired'}
              </Text>
            </View>
          </View>
          {unread && <View className="mt-2 h-2 w-2 rounded-full bg-ink" />}
        </Squircle>
      )}
    </Pressable>
  )
}

/** Artwork when there is some, with whoever caused it tucked in the corner. */
function Thumb({ n, size = 40 }: { n: Notice; size?: number }) {
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
  const k = KINDS[n.kind]
  const art = n.image && !broken
  const square = n.kind === 'CLIP' || n.kind === 'CLIP_READY'
  return (
    <View style={{ width: size }}>
      {art ? (
        <Squircle radius={size / 5} className="bg-panel" style={{ width: size, aspectRatio: square ? 1 : 2 / 3 }}>
          <Img src={n.image!} style={{ flex: 1 }} onError={() => setBroken(true)} />
        </Squircle>
      ) : n.actor ? (
        <Avatar user={n.actor} size={size} />
      ) : (
        <Squircle radius={size / 4} className="bg-panel" style={{ width: size, height: size, alignItems: 'center', justifyContent: 'center' }}>
          <k.icon size={size * 0.45} color={tokens[k.tint]} />
        </Squircle>
      )}
      {art && n.actor && (
        <View className="absolute -bottom-1.5 -right-1.5 rounded-full border-2 border-canvas">
          <Avatar user={n.actor} size={Math.round(size * 0.5)} />
        </View>
      )}
    </View>
  )
}
