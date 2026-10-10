// SPDX-License-Identifier: AGPL-3.0-or-later

import { QueryClientProvider, useQueryClient } from '@tanstack/react-query'
import { HeadContent, Outlet, Scripts, createRootRoute, useRouterState } from '@tanstack/react-router'
import { type ReactNode, useEffect } from 'react'
import { Feedback } from '../components/feedback'
import { Login } from '../components/Login'
import { Shell } from '../components/Shell'
import { graphql } from '../gql'
import { type Clip, type Item, subscribe } from '../lib/api'
import type { ClipList } from '../lib/clips'
import { useStatus } from '../lib/hooks'
import { receive, refresh as refreshNotifications } from '../lib/notifications'
import { preload, startFetching, stopFetching } from '../lib/refreshing'
import { useAppearance } from '../lib/appearance'
import { useFavicon } from '../lib/logo'
import { bootstrap } from '../lib/theme'
import { followRemote, music, refreshFromServer, restore } from '../music/player'
import { queryClient } from '../router'
import appCss from '../styles.css?url'

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1, viewport-fit=cover' },
      { title: 'tinystream' },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: 'icon', href: '/favicon.svg', type: 'image/svg+xml' },
      { rel: 'preconnect', href: 'https://image.tmdb.org' },
      { rel: 'preconnect', href: 'https://s4.anilist.co' },
    ],
  }),
  shellComponent: Document,
  component: App,
})

function Document({ children }: { children: ReactNode }) {
  return (
    // The theme is put on <html> before the app loads, so it won't match what's rendered here.
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: bootstrap }} />
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Gate />
    </QueryClientProvider>
  )
}

const EventsSubscription = graphql(`
  subscription Events {
    events {
      __typename
      ... on ConfigChanged {
        error
      }
      ... on ScanFinished {
        library
      }
      ... on LibraryChanged {
        library
      }
      ... on MetadataChanged {
        titleId
        status
      }
      ... on ListChanged {
        list
      }
      ... on SeriesChanged {
        seriesId
      }
      ... on EpisodesImported {
        library
      }
      ... on NotificationReceived {
        notification {
          ...NotificationFields
        }
      }
      ... on ClipChanged {
        clipId
        state
        progress
      }
      ... on QueueChanged {
        by
      }
      ... on PlaybackChanged {
        client
        trackId
        position
        paused
      }
    }
  }
`)

/** Keeps the UI in sync with the server: scans, metadata, config edits. */
function useLiveUpdates(enabled: boolean) {
  const qc = useQueryClient()
  useEffect(() => {
    if (!enabled) return
    const invalidate = (...keys: unknown[][]) => keys.forEach((queryKey) => void qc.invalidateQueries({ queryKey }))
    // Anything that arrived while disconnected.
    const onConnected = () => refreshNotifications(qc)
    return subscribe(
      EventsSubscription,
      {},
      ({ events: e }) => {
        switch (e.__typename) {
          case 'LibraryChanged':
          case 'ScanFinished':
            invalidate(['home'], ['libraries'], ['library', e.library], ['settings'], ['music'])
            break
          case 'QueueChanged':
            // Another app moved the queue on; follow it unless we're the ones playing.
            void refreshFromServer().catch(() => {})
            break
          case 'PlaybackChanged':
            void followRemote(e.client, e.trackId ?? null, e.position, e.paused)
            break
          case 'MetadataChanged': {
            const id = e.titleId
            if (e.status === 'FETCHING') startFetching(id)
            else if (e.status === 'FAILED') stopFetching(id, true)
            else {
              // Stay blurred until the new details are on screen and their artwork is ready.
              void Promise.all([
                qc.invalidateQueries({ queryKey: ['item', id] }),
                qc.invalidateQueries({ queryKey: ['home'] }),
                qc.invalidateQueries({ queryKey: ['library'] }),
              ])
                .then(() => {
                  const item = qc.getQueryData<Item | null>(['item', id])
                  const stills = item?.seasons.flatMap((s) => s.episodes.map((x) => x.still)).slice(0, 12) ?? []
                  return preload([item?.poster, item?.backdrop, ...stills])
                })
                .finally(() => stopFetching(id))
            }
            break
          }
          case 'ConfigChanged':
            invalidate(['auth'], ['settings'], ['libraries'])
            break
          case 'ListChanged':
            switch (e.list) {
              case 'DOWNLOADS':
                invalidate(['downloads'], ['history'])
                break
              case 'RENAME_SUGGESTIONS':
                invalidate(['renames'], ['skipped'])
                break
              case 'REQUESTS':
                invalidate(['requests'])
                break
              case 'USERS':
                // Permissions decide what's on screen, so everything that depends on them goes.
                invalidate(['auth'], ['users'], ['people'], ['permission-defaults'], ['libraries'], ['requests'])
                break
              case 'NOTIFICATIONS':
                refreshNotifications(qc)
                break
              case 'PLAYLISTS':
                invalidate(['music'])
                break
              case 'APPEARANCE':
                invalidate(['appearance'], ['schemes'], ['appearance-settings'], ['server-appearance'])
                break
            }
            break
          case 'SeriesChanged':
            invalidate(['series'], ['wanted'], ['calendar'])
            break
          case 'NotificationReceived':
            receive(qc, e.notification)
            break
          case 'ClipChanged': {
            // Progress ticks in several times a second: patch it in place rather than refetch.
            if (e.state === 'RENDERING' && e.progress != null) {
              // Only caches that have it get written to: writing marks a cache fresh, and one left
              // stale while nothing showed it must still refetch when it's back on screen.
              const has = (c: Clip | null | undefined) => c?.id === e.clipId && c.state === 'RENDERING'
              const tick = (c: Clip) => (has(c) ? { ...c, progress: e.progress } : c)
              let found = false
              qc.setQueryData<Clip | null>(['clip', e.clipId], (c) => (has(c) ? ((found = true), tick(c!)) : undefined))
              qc.setQueriesData<ClipList>({ queryKey: ['clips'] }, (l) =>
                l?.clips.some(has) ? ((found = true), { ...l, clips: l.clips.map(tick) }) : undefined,
              )
              if (found) break
            }
            invalidate(['clips'], ['clip', e.clipId])
            break
          }
          case 'EpisodesImported':
            invalidate(['calendar'], ['home'])
            break
        }
      },
      onConnected,
    )
  }, [enabled, qc])
}

function Gate() {
  const { data, isPending } = useStatus()
  // Where we are, not where we're going: switching layouts before the new
  // page commits would remount the old one mid-navigation (under the view
  // transition's snapshot, and without whatever was named for it to morph).
  const path = useRouterState({ select: (s) => (s.resolvedLocation ?? s.location).pathname })
  const fullscreen = path.startsWith('/watch/') || path.startsWith('/together/') || path.startsWith('/listen/')
  const video = path.startsWith('/watch/') || path.startsWith('/together/')
  useLiveUpdates(!!data?.viewer)
  useAppearance(data?.viewer?.id ?? null, !isPending)
  useFavicon()
  useEffect(() => {
    if (data?.viewer) void restore()
  }, [data?.viewer])
  // Music steps aside while a video plays, and comes back paused after.
  useEffect(() => {
    if (video) music.suspend()
    else music.unsuspend()
  }, [video])

  if (isPending) return null
  // Rooms decide for themselves who gets in; public ones need no account.
  if (path.startsWith('/together/') || path.startsWith('/listen/'))
    return (
      <>
        <Outlet />
        <Feedback />
      </>
    )
  if (!data?.viewer) return <Login setup={!!data?.server.setupRequired} />
  if (fullscreen)
    return (
      <>
        <Outlet />
        <Feedback />
      </>
    )
  return (
    <Shell user={data.viewer}>
      <Outlet />
    </Shell>
  )
}
