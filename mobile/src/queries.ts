// SPDX-License-Identifier: AGPL-3.0-or-later
// What several screens ask the active server, and the types of the shared
// fragments (packages/shared/fragments.graphql), as web/src/lib/api.ts has them.

import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { graphql } from './gql'
import type {
  CalendarEntryFieldsFragment,
  CardFragment,
  DiscoverResultFieldsFragment,
  DownloadFieldsFragment,
  EngineFieldsFragment,
  NotificationFieldsFragment,
  PermissionsFieldsFragment,
  ReleaseCandidateFieldsFragment,
  SeriesEpisodeFieldsFragment,
  SeriesFieldsFragment,
  SettingsFieldsFragment,
  TitleDetailFragment,
  TranscodingFieldsFragment,
  VideoRowFragment,
  ViewerFragment,
} from './gql/graphql'
import type { Api } from './lib/graphql'
import { useApi } from './session'

export type Permissions = PermissionsFieldsFragment
/** Whoever is signed in. */
export type User = ViewerFragment
/** A title, as a poster. */
export type Card = CardFragment
export type Episode = VideoRowFragment
export type Item = TitleDetailFragment
export type DiscoverResult = DiscoverResultFieldsFragment
export type Notice = NotificationFieldsFragment
export type Download = DownloadFieldsFragment
export type EngineOverview = EngineFieldsFragment
export type Series = SeriesFieldsFragment
export type SeriesEpisode = SeriesEpisodeFieldsFragment
export type ReleaseCandidate = ReleaseCandidateFieldsFragment
export type CalendarEntry = CalendarEntryFieldsFragment
export type Settings = SettingsFieldsFragment
export type Transcoding = TranscodingFieldsFragment
export type ConfigLibrary = Settings['libraries'][number]
export type SourceConfig = Settings['sources'][number]
export type ProfileConfig = Settings['profiles'][number]
export type Seeding = NonNullable<SourceConfig['seeding']>

const StatusQuery = graphql(`
  query Status {
    server {
      version
      setupRequired
      clips
      downloads
      sources
    }
    viewer {
      ...Viewer
    }
  }
`)

/** Who's signed in, and what this server is and can do. */
export function useStatus() {
  const api = useApi()
  return useQuery({ queryKey: ['auth'], queryFn: () => api.request(StatusQuery) })
}

export function useMe(): User | null {
  return useStatus().data?.viewer ?? null
}

/** Whether clipping is on here at all. */
export function useClipsOn(): boolean {
  return !!useStatus().data?.server.clips
}

export type Features = {
  canRequest: boolean
  autoApproved: boolean
  /** Sources that are turned on. */
  sources: number
}

/** What downloads can do on this server for this person; null when it has none. */
export function useFeatures(): Features | null {
  const { data } = useStatus()
  if (!data?.server.downloads || !data.viewer) return null
  const p = data.viewer.permissions
  return { canRequest: p.request, autoApproved: p.autoApprove, sources: data.server.sources }
}

/** Re-renders every `ms`, for countdowns. */
export function useNow(ms = 1000) {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms)
    return () => clearInterval(t)
  }, [ms])
  return now
}

export const AppearanceQuery = graphql(`
  query Appearance {
    appearance {
      mode
      style
      mediaTint
      light {
        id
        palette {
          tokens {
            name
            value
          }
        }
      }
      dark {
        id
        palette {
          tokens {
            name
            value
          }
        }
      }
    }
  }
`)

const LibrariesQuery = graphql(`
  query Libraries {
    libraries {
      name
      kind
      showCount
      movieCount
      albumCount
      trackCount
      bookCount
    }
  }
`)

export type Library = {
  name: string
  kind: 'VIDEO' | 'MUSIC' | 'BOOKS'
  showCount: number
  movieCount: number
  albumCount: number
  trackCount: number
  bookCount: number
}

/** The libraries you can see. */
export function useLibraries() {
  const api = useApi()
  return useQuery({ queryKey: ['libraries'], queryFn: async (): Promise<Library[]> => (await api.request(LibrariesQuery)).libraries })
}

const SettingsQuery = graphql(`
  query Settings {
    settings {
      ...SettingsFields
    }
    server {
      transcoding {
        ...TranscodingFields
      }
    }
  }
`)

export const settingsQuery = (api: Api) => ({
  queryKey: ['settings'],
  queryFn: async (): Promise<Settings & { transcoding: Transcoding }> => {
    const r = await api.request(SettingsQuery)
    return { ...r.settings, transcoding: r.server.transcoding }
  },
})

const TitleQuery = graphql(`
  query Title($id: Int!) {
    title(id: $id) {
      ...TitleDetail
    }
  }
`)

/** A title's page; fetched as soon as a finger touches its poster. */
export const titleQuery = (api: Api, id: number) => ({
  queryKey: ['item', id],
  queryFn: async (): Promise<Item | null> => (await api.request(TitleQuery, { id })).title,
})

/** config.toml and what came of it (admins only), and what transcoding can use. */
export function useSettings() {
  const api = useApi()
  const me = useMe()
  return useQuery({ ...settingsQuery(api), enabled: !!me && (me.isAdmin || me.permissions.manageShows) })
}

const PeopleQuery = graphql(`
  query People {
    users {
      ...Person
    }
  }
`)

/** Everyone else on this server, by name. */
export function usePeople() {
  const api = useApi()
  const me = useMe()
  return useQuery({
    queryKey: ['people'],
    queryFn: async () => (await api.request(PeopleQuery)).users,
    select: (users) => users.filter((u) => u.id !== me?.id).sort((a, b) => a.username.localeCompare(b.username, undefined, { sensitivity: 'base' })),
  })
}
