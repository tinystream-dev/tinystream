// SPDX-License-Identifier: AGPL-3.0-or-later
// The music side of the API: what tracks, albums and artists look like to
// the UI, and the queries the player itself needs.

import { graphql } from '../gql'
import type { AlbumCardFragment, ArtistCardFragment, MusicTrackFragment, PlaylistCardFragment, Repeat } from '../gql/graphql'

export type { Repeat }

export type MusicTrack = MusicTrackFragment
export type AlbumCard = AlbumCardFragment
export type ArtistCard = ArtistCardFragment
export type PlaylistCard = PlaylistCardFragment

export const PlayQueueQuery = graphql(`
  query PlayQueue {
    playQueue {
      tracks {
        ...MusicTrack
      }
      current
      position
      shuffled
      repeat
      changedBy
      updatedAt
    }
  }
`)

export const TrackQuery = graphql(`
  query Track($id: Int!) {
    track(id: $id) {
      ...MusicTrack
    }
  }
`)

export const SavePlayQueue = graphql(`
  mutation SavePlayQueue($input: QueueInput!) {
    savePlayQueue(input: $input) {
      updatedAt
    }
  }
`)

export const MeasureLoudness = graphql(`
  mutation MeasureLoudness($trackId: Int!) {
    measureLoudness(trackId: $trackId) {
      ...MusicTrack
    }
  }
`)

export const Played = graphql(`
  mutation Played($trackId: Int!) {
    played(trackId: $trackId)
  }
`)

export const NowPlaying = graphql(`
  mutation NowPlaying($trackId: Int, $position: Float!, $paused: Boolean!) {
    nowPlaying(trackId: $trackId, position: $position, paused: $paused)
  }
`)

export const Star = graphql(`
  mutation Star($kind: MusicKind!, $id: Int!, $starred: Boolean!) {
    star(kind: $kind, id: $id, starred: $starred)
  }
`)

export const LyricsQuery = graphql(`
  query Lyrics($trackId: Int!) {
    lyrics(trackId: $trackId) {
      synced
      source
      lines {
        start
        text
      }
    }
  }
`)

export const SimilarTracks = graphql(`
  query SimilarTracks($trackId: Int!, $exclude: [Int!]!) {
    similarTracks(trackId: $trackId, count: 25, exclude: $exclude) {
      ...MusicTrack
    }
  }
`)

export const AlbumTracks = graphql(`
  query AlbumTracks($id: Int!) {
    album(id: $id) {
      tracks {
        ...MusicTrack
      }
    }
  }
`)

/** Cover art at a size, so lists don't load full-size pictures. */
export function cover(url: string | null | undefined, size: number): string | undefined {
  if (!url) return undefined
  const px = Math.round(size * (typeof devicePixelRatio === 'number' ? Math.min(devicePixelRatio, 2) : 1))
  return `${url}${url.includes('?') ? '&' : '?'}size=${px}`
}

export { duration, hiRes, length, quality } from '@tinystream/shared/music'
