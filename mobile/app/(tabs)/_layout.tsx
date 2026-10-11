// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { House, Library, type LucideIcon, Music, Search, UserRound } from 'lucide-react-native'
import { haptic } from '../../modules/haptics'
import { useSwitcher } from '../../src/components/Switcher'
import { Tabs } from '../../src/components/Tabs'
import { GlassScope } from '../../src/effects/Glass'
import { graphql } from '../../src/gql'
import { usePendingLink } from '../../src/nav'
import { useInbox } from '../../src/notifications'
import { useFeatures, useMe } from '../../src/queries'
import { useApi } from '../../src/session'

const icon =
  (Icon: LucideIcon) =>
  ({ color, size }: { color: string; size: number }) => <Icon color={color} size={size - 2} />

const AttentionQuery = graphql(`
  query Attention($requests: Boolean!, $downloads: Boolean!) {
    requests @include(if: $requests) {
      id
      state
    }
    downloads @include(if: $downloads) {
      id
      state
      importState
    }
  }
`)

/** Unread notifications, and for those who handle them, requests waiting and downloads that failed. */
function useAttention() {
  const api = useApi()
  const me = useMe()
  const features = useFeatures()
  const { data: inbox } = useInbox()
  const requests = !!features && !!me?.permissions.manageRequests
  const downloads = !!features && !!me?.permissions.downloads
  const { data } = useQuery({
    queryKey: ['attention', requests, downloads],
    queryFn: () => api.request(AttentionQuery, { requests, downloads }),
    enabled: requests || downloads,
    refetchInterval: 30_000,
  })
  const pending = data?.requests?.filter((r) => r.state === 'PENDING').length ?? 0
  const failing = data?.downloads?.filter((d) => d.state === 'FAILED' || d.importState === 'FAILED').length ?? 0
  return (inbox?.unread ?? 0) + pending + failing
}

export default function TabsLayout() {
  const switcher = useSwitcher()
  const attention = useAttention()
  usePendingLink()
  return (
    <GlassScope>
      <Tabs screenListeners={{ tabPress: () => haptic('tab') }}>
        <Tabs.Screen name="(home)" options={{ title: 'Home', tabBarIcon: icon(House) }} />
        <Tabs.Screen name="(search)" options={{ title: 'Search', tabBarIcon: icon(Search) }} />
        <Tabs.Screen name="(library)" options={{ title: 'Library', tabBarIcon: icon(Library) }} />
        <Tabs.Screen name="(music)" options={{ title: 'Music', tabBarIcon: icon(Music) }} />
        <Tabs.Screen
          name="(profile)"
          options={{
            title: 'Profile',
            tabBarIcon: icon(UserRound),
            tabBarBadge: attention,
            tabBarOnLongPress: () => {
              haptic('longPressOpen')
              switcher.open()
            },
          }}
        />
      </Tabs>
    </GlassScope>
  )
}
