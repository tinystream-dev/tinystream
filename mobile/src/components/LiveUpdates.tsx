// SPDX-License-Identifier: AGPL-3.0-or-later
// Keeps what's on screen in step with the server (web's useLiveUpdates):
// scans, metadata, config edits, lists that changed, and new notifications,
// which also show as a toast.

import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { graphql } from '../gql'
import { useFollow } from '../nav'
import { receive, refresh as refreshNotifications } from '../notifications'
import { useMe } from '../queries'
import { useApi } from '../session'
import { toast } from './Feedback'

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
      }
    }
  }
`)

export function LiveUpdates() {
  const api = useApi()
  const qc = useQueryClient()
  const signedIn = !!useMe()
  // The latest, without subscribing again each time the app goes somewhere.
  const latest = useFollow()
  const follow = useRef(latest)
  follow.current = latest

  useEffect(() => {
    if (!signedIn) return
    const invalidate = (...keys: unknown[][]) => keys.forEach((queryKey) => void qc.invalidateQueries({ queryKey }))
    // Anything that arrived while disconnected.
    const onConnected = () => refreshNotifications(qc)
    return api.subscribe(
      EventsSubscription,
      {},
      ({ events: e }) => {
        switch (e.__typename) {
          case 'LibraryChanged':
          case 'ScanFinished':
            invalidate(['home'], ['libraries'], ['library', e.library], ['settings'], ['music'])
            break
          case 'MetadataChanged':
            if (e.status !== 'FETCHING') invalidate(['item', e.titleId], ['home'], ['library'])
            break
          case 'ConfigChanged':
            invalidate(['auth'], ['settings'], ['libraries'])
            break
          case 'ListChanged':
            switch (e.list) {
              case 'DOWNLOADS':
                invalidate(['downloads'], ['history'], ['attention'])
                break
              case 'RENAME_SUGGESTIONS':
                invalidate(['renames'], ['skipped'])
                break
              case 'REQUESTS':
                invalidate(['requests'], ['attention'])
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
          case 'NotificationReceived': {
            const n = e.notification
            receive(qc, n)
            toast({ title: n.title, body: n.body, image: n.image, onPress: n.link ? () => follow.current(n.link) : undefined })
            break
          }
          case 'ClipChanged':
            invalidate(['clips'])
            break
          case 'EpisodesImported':
            invalidate(['calendar'], ['home'])
            break
        }
      },
      onConnected,
    )
  }, [api, qc, signedIn])

  return null
}
