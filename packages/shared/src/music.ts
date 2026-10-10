// SPDX-License-Identifier: AGPL-3.0-or-later

export type MusicTrack = {
  id: number
  title: string
  artist: string
  album: string
  albumId: number | null
  albumArtist: string | null
  library: string
  disc: number | null
  number: number | null
  year: number | null
  duration: number
  codec: string
  suffix: string
  lossless: boolean
  bitrate: number | null
  sampleRate: number | null
  bitDepth: number | null
  channels: number | null
  size: number
  file: string
  flac: string
  cover: string | null
  coverTint: string | null
  starred: boolean
  rating: number | null
  playCount: number
  artists: Array<{ id: number; name: string }>
  gains: { trackGain: number | null; trackPeak: number | null; albumGain: number | null; albumPeak: number | null; pending: boolean }
}
export type Repeat = 'OFF' | 'ONE' | 'ALL'
export type Item = { key: string; url: string; size: number; ext: string; gain: number; crossfade: number }
export type EngineEvents = {
  track: (key: string) => void
  ended: () => void
  error: (key: string, message: string) => void
  starved: (waiting: boolean) => void
}
export interface AudioEngine {
  rate: number
  on<K extends keyof EngineEvents>(event: K, fn: EngineEvents[K]): () => void
  load(items: Item[], start: number, rate?: number): Promise<void>
  play(): Promise<void>
  pause(): void
  stop(): void
  upcoming(after: string, items: Item[]): void
  gain(key: string, gain: number): void
  volume(gain: number): void
  position(): { key: string; time: number } | null
}
