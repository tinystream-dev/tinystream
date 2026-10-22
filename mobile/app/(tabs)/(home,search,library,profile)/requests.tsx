// SPDX-License-Identifier: AGPL-3.0-or-later
// Requests (web/src/routes/requests.tsx). Whoever approves them swipes a
// request right to approve it, or left to decline it.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { relative } from '@tinystream/shared/downloads'
import { Check, Plus, X } from 'lucide-react-native'
import { Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { Avatar } from '../../../src/components/Avatar'
import { toast, toastError } from '../../../src/components/Feedback'
import { Img } from '../../../src/components/Img'
import { Page, Section } from '../../../src/components/Page'
import { ListSkeleton, useArrived } from '../../../src/components/Skeleton'
import { SwipeRow } from '../../../src/components/SwipeRow'
import { Badge, Button, Empty, ErrorText, IconButton, Progress } from '../../../src/components/ui'
import { Squircle } from '../../../src/effects/Squircle'
import { useMotion } from '../../../src/effects/motion'
import { graphql } from '../../../src/gql'
import type { RequestsQuery } from '../../../src/gql/graphql'
import { useGo } from '../../../src/nav'
import { useMe } from '../../../src/queries'
import { useApi } from '../../../src/session'
import { useTheme } from '../../../src/theme/ThemeProvider'

const RequestsQueryDoc = graphql(`
  query Requests {
    requests {
      id
      user {
        ...Person
      }
      name
      year
      poster
      library
      state
      title {
        id
      }
      note
      createdAt
      have
      aired
    }
  }
`)

const Approve = graphql(`
  mutation ApproveRequest($id: Int!) {
    approveRequest(id: $id) {
      id
    }
  }
`)

const Decline = graphql(`
  mutation DeclineRequest($id: Int!) {
    declineRequest(id: $id) {
      id
    }
  }
`)

const Delete = graphql(`
  mutation DeleteRequest($id: Int!) {
    deleteRequest(id: $id)
  }
`)

type MediaRequest = RequestsQuery['requests'][number]

export default function Requests() {
  const api = useApi()
  const me = useMe()
  const go = useGo()
  const motion = useMotion()
  const { tokens } = useTheme()
  const { data, refetch } = useQuery({ queryKey: ['requests'], queryFn: async () => (await api.request(RequestsQueryDoc)).requests, refetchInterval: 15_000 })
  const arrived = useArrived(!!data)
  const pending = data?.filter((r) => r.state === 'PENDING') ?? []
  const decided = data?.filter((r) => r.state !== 'PENDING') ?? []
  return (
    <Page
      title="Requests"
      onRefresh={() => refetch()}
      right={
        <IconButton label={me?.permissions.manageShows ? 'Add a show' : 'Request a show'} onPress={() => go('search', '(search)')}>
          <Plus size={22} color={tokens.ink} />
        </IconButton>
      }
    >
      {!data && <ListSkeleton rows={3} />}
      {data?.length === 0 && <Empty title="No requests yet" />}
      {pending.length > 0 && (
        <Animated.View style={arrived ? motion.developIn(0) : undefined}>
          <Section title={me?.permissions.manageRequests ? 'Waiting for you' : 'Waiting for approval'}>
            {me?.permissions.manageRequests && <Text className="font-sans -mt-1 mb-3 text-xs text-ink-3">Swipe right to approve, left to decline.</Text>}
            <View className="gap-2">
              {pending.map((r) => (
                <RequestRow key={r.id} r={r} />
              ))}
            </View>
          </Section>
        </Animated.View>
      )}
      {decided.length > 0 && (
        <Animated.View style={arrived ? motion.developIn(1) : undefined}>
          <Section title="Earlier">
            <View className="gap-2">
              {decided.map((r) => (
                <RequestRow key={r.id} r={r} />
              ))}
            </View>
          </Section>
        </Animated.View>
      )}
    </Page>
  )
}

function RequestRow({ r }: { r: MediaRequest }) {
  const api = useApi()
  const me = useMe()
  const go = useGo()
  const { tokens } = useTheme()
  const manage = !!me?.permissions.manageRequests
  const qc = useQueryClient()
  const done = () => {
    void qc.invalidateQueries({ queryKey: ['requests'] })
    void qc.invalidateQueries({ queryKey: ['calendar'] })
    void qc.invalidateQueries({ queryKey: ['attention'] })
  }
  const approve = useMutation({
    mutationFn: () => api.request(Approve, { id: r.id }),
    onSuccess: () => (done(), toast({ title: `Approved ${r.name}`, image: r.poster, tone: 'ok' })),
    onError: toastError,
  })
  const decline = useMutation({
    mutationFn: () => api.request(Decline, { id: r.id }),
    onSuccess: () => (done(), toast({ title: `Declined ${r.name}`, image: r.poster })),
    onError: toastError,
  })
  const cancel = useMutation({ mutationFn: () => api.request(Delete, { id: r.id }), onSuccess: done, onError: toastError })
  const progress = r.aired ? (r.have ?? 0) / r.aired : null
  const error = (approve.error ?? decline.error ?? cancel.error) as Error | null
  const deciding = r.state === 'PENDING' && manage
  return (
    <SwipeRow
      left={deciding ? { label: 'Approve', icon: <Check size={20} color={tokens.canvas} />, color: tokens.ok, text: tokens.canvas, run: () => approve.mutate() } : undefined}
      right={deciding ? { label: 'Decline', icon: <X size={20} color={tokens.canvas} />, color: tokens.danger, text: tokens.canvas, run: () => decline.mutate() } : undefined}
    >
      <Squircle radius={16} edge className="flex-row gap-3 bg-raised p-3">
        <Squircle radius={10} className="bg-panel" style={{ width: 52, aspectRatio: 2 / 3 }}>
          {r.poster && <Img src={r.poster} style={{ flex: 1 }} />}
        </Squircle>
        <View className="min-w-0 flex-1 gap-1">
          <View className="flex-row flex-wrap items-center gap-2">
            <Text
              className="font-sans flex-shrink text-sm font-medium text-ink"
              numberOfLines={1}
              onPress={r.title ? () => go(`title/${r.title!.id}`) : undefined}
            >
              {r.name}
            </Text>
            {r.year && <Text className="font-sans text-xs text-ink-3">{r.year}</Text>}
            <Badge tone={r.state === 'APPROVED' ? 'ok' : r.state === 'DECLINED' ? 'danger' : 'warn'}>
              {r.state === 'APPROVED' ? 'Approved' : r.state === 'DECLINED' ? 'Declined' : 'Pending'}
            </Badge>
          </View>
          <View className="flex-row items-center gap-1.5">
            {r.user && r.user.id !== me?.id && <Avatar user={r.user} size={16} />}
            <Text className="font-sans flex-1 text-xs text-ink-3" numberOfLines={1}>
              {r.user?.id !== me?.id ? `${r.user?.username ?? 'Someone'} asked ` : 'You asked '}
              {relative(r.createdAt)}
              {r.library && ` · for ${r.library}`}
            </Text>
          </View>
          {r.note && <Text className="font-sans text-[13px] text-ink-2">“{r.note}”</Text>}
          {r.state === 'APPROVED' && progress !== null && (
            <View className="mt-1 flex-row items-center gap-3">
              <View className="flex-1">
                <Progress value={progress} tone={progress >= 1 ? 'ok' : 'live'} />
              </View>
              <Text className="font-sans text-xs text-ink-3">
                {r.have} of {r.aired} episodes
              </Text>
            </View>
          )}
          {error && <ErrorText>{error.message}</ErrorText>}
          {r.state === 'PENDING' && !manage && r.user?.id === me?.id && (
            <View className="flex-row">
              <Button size="sm" variant="plain" onPress={() => cancel.mutate()}>
                Cancel request
              </Button>
            </View>
          )}
        </View>
      </Squircle>
    </SwipeRow>
  )
}
