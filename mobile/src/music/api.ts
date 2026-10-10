// SPDX-License-Identifier: AGPL-3.0-or-later

import { graphql } from '../gql'
import type { AlbumCardFragment, ArtistCardFragment, MusicTrackFragment, PlaylistCardFragment, Repeat } from '../gql/graphql'

export type { Repeat }

graphql(`
  fragment MusicTrack on Track {
    id
    title
    artist
    artists {
      id
      name
    }
    album
    albumId
    albumArtist
    library
    disc
    number
    year
    duration
    codec
    suffix
    lossless
    bitrate
    sampleRate
    bitDepth
    channels
    size
    file
    flac
    cover
    coverTint
    gains {
      trackGain
      trackPeak
      albumGain
      albumPeak
      pending
    }
    starred
    rating
    playCount
  }
`)

graphql(`
  fragment AlbumCard on Album {
    id
    name
    artist
    artists {
      id
      name
    }
    year
    cover
    trackCount
    duration
    compilation
    starred
    playCount
    addedAt
  }
`)

graphql(`
  fragment ArtistCard on Artist {
    id
    name
    albumCount
    trackCount
    cover
    starred
  }
`)

graphql(`
  fragment PlaylistCard on Playlist {
    id
    name
    comment
    public
    mine
    trackCount
    duration
    covers
    owner {
      id
      username
    }
  }
`)

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

export function cover(url: string | null | undefined, size: number): string | undefined {
  if (!url) return undefined
  const px = Math.round(size * 2)
  return `${url}${url.includes('?') ? '&' : '?'}size=${px}`
}

export function quality(t: Pick<MusicTrack, 'codec' | 'lossless' | 'bitDepth' | 'sampleRate' | 'bitrate'>): string {
  const name = t.codec === 'pcm' ? 'PCM' : t.codec === 'wavpack' ? 'WavPack' : t.codec === 'musepack' ? 'Musepack' : t.codec.toUpperCase()
  if (t.lossless && t.sampleRate) {
    const khz = t.sampleRate / 1000
    return `${name} ${t.bitDepth ?? 16}/${Number.isInteger(khz) ? khz : khz.toFixed(1)}`
  }
  return t.bitrate ? `${name} ${t.bitrate}` : name
}

export const hiRes = (t: Pick<MusicTrack, 'lossless' | 'bitDepth' | 'sampleRate'>) => t.lossless && ((t.bitDepth ?? 16) > 16 || (t.sampleRate ?? 44100) > 48000)

export function duration(secs: number): string {
  const s = Math.max(0, Math.round(secs))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const r = String(s % 60).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${r}` : `${m}:${r}`
}

export function length(secs: number): string {
  if (secs < 60) return `${Math.round(secs)} sec`
  const m = Math.round(secs / 60)
  return m >= 60 ? `${Math.floor(m / 60)} hr ${m % 60} min` : `${m} min`
}
