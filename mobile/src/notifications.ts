// SPDX-License-Identifier: AGPL-3.0-or-later
// Someone's notifications (web/src/lib/notifications.ts): the inbox, and live
// arrivals, which also show as a toast while the app is open.

import { type QueryClient, useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { graphql } from './gql'
import type { InboxFieldsFragment } from './gql/graphql'
import type { Notice } from './queries'
import { useApi } from './session'

export type Inbox = InboxFieldsFragment

graphql(`
  fragment InboxFields on Inbox {
    items {
      ...NotificationFields
    }
    unread
  }
`)

export const InboxQuery = graphql(`
  query Inbox {
    notifications {
      ...InboxFields
    }
  }
`)

export const MarkRead = graphql(`
  mutation MarkNotificationsRead($ids: [Int!]) {
    markNotificationsRead(ids: $ids) {
      ...InboxFields
    }
  }
`)

const Delete = graphql(`
  mutation DeleteNotifications($id: Int) {
    deleteNotifications(id: $id) {
      ...InboxFields
    }
  }
`)

const KEY = ['notifications']

export function useInbox(enabled = true) {
  const api = useApi()
  return useQuery({ queryKey: KEY, queryFn: async () => (await api.request(InboxQuery)).notifications, enabled, staleTime: 60_000 })
}

/** Marks some (or, without ids, all) as read, straight away on screen. */
export function useMarkRead() {
  const api = useApi()
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (ids?: number[]) => api.request(MarkRead, { ids }),
    onMutate: (ids) => {
      const now = Math.floor(Date.now() / 1000)
      qc.setQueryData<Inbox>(KEY, (inbox) => {
        if (!inbox) return inbox
        const items = inbox.items.map((n) => (n.readAt == null && (!ids || ids.includes(n.id)) ? { ...n, readAt: now } : n))
        return { items, unread: ids ? Math.max(0, inbox.unread - inbox.items.filter((n) => n.readAt == null && ids.includes(n.id)).length) : 0 }
      })
    },
    onSettled: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

export function useRemove() {
  const api = useApi()
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: number | 'all') => api.request(Delete, { id: id === 'all' ? null : id }),
    onMutate: (id) =>
      qc.setQueryData<Inbox>(KEY, (inbox) => {
        if (!inbox || id === 'all') return { items: [], unread: 0 }
        const gone = inbox.items.find((n) => n.id === id)
        return { items: inbox.items.filter((n) => n.id !== id), unread: inbox.unread - (gone && gone.readAt == null ? 1 : 0) }
      }),
    onSettled: () => qc.invalidateQueries({ queryKey: KEY }),
  })
}

type Listener = (n: Notice) => void
const listeners = new Set<Listener>()

/** Hears about each notification the moment it arrives. */
export function onArrival(l: Listener) {
  listeners.add(l)
  return () => void listeners.delete(l)
}

/** A notification from the server: into the inbox, and to whoever's listening. */
export function receive(qc: QueryClient, n: Notice) {
  qc.setQueryData<Inbox>(KEY, (inbox) => {
    if (!inbox || inbox.items.some((x) => x.id === n.id)) return inbox
    // An invitation to the same room again replaces the first one.
    const replaced = (x: Notice) => n.kind === 'INVITE' && x.kind === 'INVITE' && x.link === n.link && x.readAt == null
    const gone = inbox.items.filter(replaced).length
    return { items: [n, ...inbox.items.filter((x) => !replaced(x))], unread: inbox.unread + 1 - gone }
  })
  if (!qc.getQueryData(KEY)) refresh(qc)
  listeners.forEach((l) => l(n))
}

export function refresh(qc: QueryClient) {
  void qc.invalidateQueries({ queryKey: KEY })
}
