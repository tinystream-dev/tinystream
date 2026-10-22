// SPDX-License-Identifier: AGPL-3.0-or-later
// Profile: who's signed in where, and everything web keeps in its sidebar
// and account menu, gated as web gates it (Shell.tsx).

import { useQuery } from '@tanstack/react-query'
import { speed } from '@tinystream/shared/downloads'
import { ArrowDownToLine, Bell, CalendarDays, Inbox, ListTodo, LogOut, Scissors, Server, Settings } from 'lucide-react-native'
import { Text, View } from 'react-native'
import { Avatar } from '../../../src/components/Avatar'
import { ask } from '../../../src/components/Feedback'
import { Page } from '../../../src/components/Page'
import { useSwitcher } from '../../../src/components/Switcher'
import { Badge, Divider, Group, ListRow } from '../../../src/components/ui'
import { Ticker } from '../../../src/effects/Ticker'
import { graphql } from '../../../src/gql'
import { useGo } from '../../../src/nav'
import { useInbox } from '../../../src/notifications'
import { useClipsOn, useFeatures, useMe } from '../../../src/queries'
import { signOut } from '../../../src/servers'
import { useApi, useSession } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

const TransfersQuery = graphql(`
  query Transfers {
    downloadEngine {
      downloadRate
      killSwitch
    }
    downloads {
      id
      state
      importState
    }
  }
`)

const RequestCountQuery = graphql(`
  query RequestCount {
    requests {
      id
      state
    }
  }
`)

export default function Profile() {
  const api = useApi()
  const { server } = useSession()
  const { open } = useSwitcher()
  const go = useGo()
  const me = useMe()
  const features = useFeatures()
  const clipsOn = useClipsOn()
  const { tokens } = useTheme()
  const { data: inbox } = useInbox()
  const can = me?.permissions
  const downloads = !!features && !!can?.downloads
  const transfers = useQuery({ queryKey: ['downloads', 'transfers'], queryFn: () => api.request(TransfersQuery), enabled: downloads, refetchInterval: 3000 })
  const requests = useQuery({
    queryKey: ['requests', 'count'],
    queryFn: async () => (await api.request(RequestCountQuery)).requests,
    enabled: !!features && !!can?.manageRequests,
  })
  const pending = requests.data?.filter((r) => r.state === 'PENDING').length ?? 0
  const failing = transfers.data?.downloads.filter((d) => d.state === 'FAILED' || d.importState === 'FAILED').length ?? 0
  const engine = transfers.data?.downloadEngine
  const icon = (Icon: typeof Bell) => <Icon size={20} color={tokens['ink-2']} />

  return (
    <Page title="Profile" onRefresh={() => Promise.all([transfers.refetch(), requests.refetch()])}>
      <View className="flex-row items-center gap-4">
        <Avatar user={me ?? server} size={64} />
        <View className="min-w-0 flex-1">
          <Text className="font-sans text-xl font-semibold text-ink" numberOfLines={1}>
            {me?.username ?? server.username}
          </Text>
          <Text className="font-sans text-sm text-ink-2" numberOfLines={1}>
            {server.name}
          </Text>
          {me?.isAdmin && <Text className="font-sans text-xs text-ink-3">Admin</Text>}
        </View>
      </View>

      {features && (
        <Group title="Shows">
          <ListRow icon={icon(CalendarDays)} label="Calendar" onPress={() => go('calendar')} />
          {(can?.request || can?.manageRequests) && (
            <>
              <Divider inset={52} />
              <ListRow
                icon={icon(Inbox)}
                label="Requests"
                onPress={() => go('requests')}
                right={pending > 0 ? <Badge tone="warn">{`${pending} waiting`}</Badge> : undefined}
                chevron
              />
            </>
          )}
          {can?.manageShows && (
            <>
              <Divider inset={52} />
              <ListRow icon={icon(ListTodo)} label="Wanted" onPress={() => go('wanted')} />
            </>
          )}
          {can?.downloads && (
            <>
              <Divider inset={52} />
              <ListRow
                icon={icon(ArrowDownToLine)}
                label="Downloads"
                onPress={() => go('downloads')}
                chevron
                right={
                  engine?.killSwitch ? (
                    <Text className="font-sans text-sm text-danger">VPN down</Text>
                  ) : (
                    <View className="flex-row items-center gap-2">
                      {failing > 0 && <Badge tone="danger">{`${failing} failed`}</Badge>}
                      {engine && engine.downloadRate > 0 && <Ticker value={speed(engine.downloadRate)} className="font-sans text-sm text-info" />}
                    </View>
                  )
                }
              />
            </>
          )}
        </Group>
      )}

      <Group>
        {clipsOn && (
          <>
            <ListRow icon={icon(Scissors)} label="Clips" onPress={() => go('clips')} />
            <Divider inset={52} />
          </>
        )}
        <ListRow
          icon={icon(Bell)}
          label="Notifications"
          onPress={() => go('notifications')}
          chevron
          right={inbox && inbox.unread > 0 ? <Badge tone="strong">{String(inbox.unread)}</Badge> : undefined}
        />
        <Divider inset={52} />
        <ListRow icon={icon(Settings)} label="Settings" onPress={() => go('settings')} />
      </Group>

      <Group>
        <ListRow icon={icon(Server)} label="Switch server" onPress={open} />
        <Divider inset={52} />
        <ListRow
          icon={<LogOut size={20} color={tokens.danger} />}
          label={`Sign out ${me?.username ?? server.username}`}
          danger
          chevron={false}
          onPress={async () => {
            if (await ask({ title: 'Sign out?', body: `You'll have to sign in to ${server.name} again to use it.`, confirm: 'Sign out', danger: true })) void signOut(server.id)
          }}
        />
      </Group>
    </Page>
  )
}
