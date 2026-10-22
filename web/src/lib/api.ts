// SPDX-License-Identifier: AGPL-3.0-or-later
// The pieces of tinystream's GraphQL API the UI shares: the types of the
// fragments for what shows up in several places (packages/shared/fragments.graphql).
// Each page keeps its own queries next to it; `bun run codegen` types them all
// from packages/shared/schema.graphql.

import { graphql } from '../gql'
import type {
  CalendarEntryFieldsFragment,
  CardFragment,
  ClipAllowanceFieldsFragment,
  ClipFieldsFragment,
  DiscoverResultFieldsFragment,
  DownloadFieldsFragment,
  EngineFieldsFragment,
  NotificationFieldsFragment,
  PermissionsFieldsFragment,
  PlaybackFragment,
  TranscodingFieldsFragment,
  PersonFragment,
  ReleaseCandidateFieldsFragment,
  SeriesEpisodeFieldsFragment,
  SeriesFieldsFragment,
  SettingsFieldsFragment,
  TitleDetailFragment,
  VideoRowFragment,
  ViewerFragment,
} from '../gql/graphql'

import { request } from './graphql'

export { ApiError, request, subscribe } from './graphql'
export type {
  ClipState,
  EpisodeState,
  Monitor,
  NotificationKind,
  Provider,
  SourceKind,
  TitleKind,
} from '../gql/graphql'

/** Someone on this server, as anyone sees them. */
export type Person = PersonFragment
export type Permissions = PermissionsFieldsFragment
/** Whoever is signed in. */
export type User = ViewerFragment
/** A title, as a poster. */
export type Card = CardFragment
export type Episode = VideoRowFragment
export type Item = TitleDetailFragment
export type Season = Item['seasons'][number]
export type Transcoding = TranscodingFieldsFragment
/** A video, as the player needs it, and what the server can transcode with. */
export type Playback = PlaybackFragment & { transcoding: Transcoding }
export type SubtitleTrack = Playback['media']['subtitles'][number]
export type AudioTrack = Playback['media']['audio'][number]
export type Chapter = Playback['media']['chapters'][number]
export type DiscoverResult = DiscoverResultFieldsFragment
export type Clip = ClipFieldsFragment
export type ClipAllowance = ClipAllowanceFieldsFragment
export type Notice = NotificationFieldsFragment
export type Download = DownloadFieldsFragment
export type EngineOverview = EngineFieldsFragment
export type Series = SeriesFieldsFragment
export type SeriesEpisode = SeriesEpisodeFieldsFragment
export type ReleaseCandidate = ReleaseCandidateFieldsFragment
export type Release = ReleaseCandidate['release']
export type CalendarEntry = CalendarEntryFieldsFragment
export type Settings = SettingsFieldsFragment
export type ConfigLibrary = Settings['libraries'][number]
export type SourceConfig = Settings['sources'][number]
export type ProfileConfig = Settings['profiles'][number]
export type Seeding = NonNullable<SourceConfig['seeding']>
export type DownloadsConfig = Settings['downloads']
export type ClipsConfig = Settings['clips']
export type MusicConfig = Settings['music']

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

/** The libraries you can see; cached under ['libraries']. */
export const librariesQuery = {
  queryKey: ['libraries'],
  queryFn: async (): Promise<Library[]> => (await request(LibrariesQuery)).libraries,
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

/** config.toml and what came of it (admins only), and what transcoding can use; cached under ['settings']. */
export const settingsQuery = {
  queryKey: ['settings'],
  queryFn: async (): Promise<Settings & { transcoding: Transcoding }> => {
    const r = await request(SettingsQuery)
    return { ...r.settings, transcoding: r.server.transcoding }
  },
}
