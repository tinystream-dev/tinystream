// SPDX-License-Identifier: AGPL-3.0-or-later
// The music player (packages/shared/src/music.ts) with the browser's sound,
// storage and media keys.

import { createPlayer, type Entry as SharedEntry, type State as SharedState } from '@tinystream/shared/music'
import { useEffect, useState, useSyncExternalStore } from 'react'
import { request } from '../lib/api'
import { MeasureLoudness, NowPlaying, PlayQueueQuery, Played, type MusicTrack, SavePlayQueue, TrackQuery, cover } from './api'
import { Engine } from './engine'

export type { GainMode, Remote, RoomControl, Settings } from '@tinystream/shared/music'
export type Entry = SharedEntry<MusicTrack>
export type State = SharedState<MusicTrack>

const browser = typeof window !== 'undefined'

// Media keys and the lock screen only show for a playing media element, so a silent one plays alongside.
let quiet: HTMLAudioElement | null = null

function silence(on: boolean) {
  if (typeof document === 'undefined') return
  if (!quiet) {
    const rate = 8000
    const bytes = new Uint8Array(44 + rate)
    const view = new DataView(bytes.buffer)
    const text = (at: number, s: string) => [...s].forEach((c, i) => view.setUint8(at + i, c.charCodeAt(0)))
    text(0, 'RIFF')
    view.setUint32(4, 36 + rate, true)
    text(8, 'WAVEfmt ')
    view.setUint32(16, 16, true)
    view.setUint16(20, 1, true)
    view.setUint16(22, 1, true)
    view.setUint32(24, rate, true)
    view.setUint32(28, rate, true)
    view.setUint16(32, 1, true)
    view.setUint16(34, 8, true)
    text(36, 'data')
    view.setUint32(40, rate, true)
    bytes.fill(128, 44)
    quiet = new Audio(URL.createObjectURL(new Blob([bytes], { type: 'audio/wav' })))
    quiet.loop = true
  }
  if (on) void quiet.play().catch(() => {})
  else quiet.pause()
}

const player = createPlayer<MusicTrack>({
  engine: () => new Engine(),
  storage: {
    get: (key) => (browser ? localStorage.getItem(key) : null),
    set: (key, value) => (value == null ? localStorage.removeItem(key) : localStorage.setItem(key, value)),
  },
  queue: () => request(PlayQueueQuery).then((r) => r.playQueue),
  track: (id) => request(TrackQuery, { id }).then((r) => r.track),
  measure: (trackId) => request(MeasureLoudness, { trackId }).then((r) => r.measureLoudness),
  save: (input) => request(SavePlayQueue, { input }),
  played: (trackId) => request(Played, { trackId }),
  nowPlaying: (trackId, position, paused) => request(NowPlaying, { trackId, position, paused }),
  playing: silence,
  started(e) {
    if (typeof navigator === 'undefined' || !('mediaSession' in navigator)) return
    const t = e.track
    navigator.mediaSession.metadata = new MediaMetadata({
      title: t.title,
      artist: t.artist,
      album: t.album,
      artwork: t.cover ? [256, 512].map((s) => ({ src: cover(t.cover, s / 2)!, sizes: `${s}x${s}` })) : [],
    })
  },
  progress(e, time) {
    if (!('mediaSession' in navigator) || e.track.duration <= 0) return
    try {
      navigator.mediaSession.setPositionState({ duration: e.track.duration, position: Math.min(time, e.track.duration), playbackRate: 1 })
    } catch {
      // Some browsers are picky about positions at the very end.
    }
  },
})

export const { getState, current, position, restore, refreshFromServer, followRemote, outputRate } = player

export const music = { ...player.music, supported: Engine.supported }

export function usePlayer<T>(select: (s: State) => T): T {
  return useSyncExternalStore(
    player.subscribe,
    () => select(getState()),
    () => select(getState()),
  )
}

/** The current position, updated every frame while something is on screen to show it. */
export function usePosition(): number {
  const [time, setTime] = useState(player.shown)
  const playing = usePlayer((s) => s.playing || (s.remote != null && !s.remote.paused))
  const index = usePlayer((s) => s.index)
  // Paused, the engine may still hold where it was before a seek; resumeAt already knows where it'll pick up.
  const resumeAt = usePlayer((s) => s.resumeAt)
  useEffect(() => {
    let raf = 0
    const tick = () => {
      setTime(player.shown())
      if (playing) raf = requestAnimationFrame(tick)
    }
    tick()
    return () => cancelAnimationFrame(raf)
  }, [playing, index, resumeAt])
  return time
}

if (browser) window.addEventListener('pagehide', () => void player.flush())

if (typeof navigator !== 'undefined' && 'mediaSession' in navigator) {
  const ms = navigator.mediaSession
  ms.setActionHandler('play', () => void music.resume())
  ms.setActionHandler('pause', () => music.pause())
  ms.setActionHandler('nexttrack', () => music.next())
  ms.setActionHandler('previoustrack', () => music.previous())
  ms.setActionHandler('seekto', (d) => d.seekTime != null && music.seek(d.seekTime))
  ms.setActionHandler('seekbackward', (d) => music.seek((position()?.time ?? 0) - (d.seekOffset ?? 10)))
  ms.setActionHandler('seekforward', (d) => music.seek((position()?.time ?? 0) + (d.seekOffset ?? 10)))
}
