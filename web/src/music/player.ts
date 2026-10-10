// SPDX-License-Identifier: AGPL-3.0-or-later
import { createMusicPlayer } from '@tinystream/shared/music-player'
import { request } from '../lib/api'
import { MeasureLoudness, NowPlaying, PlayQueueQuery, Played, SavePlayQueue, TrackQuery, cover } from './api'
import { Engine } from './engine'
export type { Entry, GainMode, Settings } from '@tinystream/shared/music-player'
const player = createMusicPlayer({
  timers: typeof window !== 'undefined',
  animate: (fn) => requestAnimationFrame(fn),
  cancelAnimation: (id) => cancelAnimationFrame(id),
  storage: typeof localStorage !== 'undefined' ? localStorage : { getItem: () => null, setItem() {}, removeItem() {} },
  engine: () => new Engine(),
  supported: Engine.supported,
  playing: silence,
  queue: () => request(PlayQueueQuery),
  track: (id) => request(TrackQuery, { id }),
  measure: (trackId) => request(MeasureLoudness, { trackId }),
  save: (input) => request(SavePlayQueue, { input }),
  played: (trackId) => request(Played, { trackId }),
  nowPlaying: (trackId, position, paused) => request(NowPlaying, { trackId, position, paused }),
  metadata(t) {
    if (typeof navigator !== 'undefined' && 'mediaSession' in navigator)
      navigator.mediaSession.metadata = new MediaMetadata({
        title: t.title,
        artist: t.artist,
        album: t.album,
        artwork: t.cover ? [256, 512].map((s) => ({ src: cover(t.cover, s / 2)!, sizes: `${s}x${s}` })) : [],
      })
  },
})
export const { music, getState, usePlayer, current, position, usePosition, outputRate, restore, refreshFromServer, followRemote } = player
export type Remote = NonNullable<ReturnType<typeof getState>['remote']>
export type State = ReturnType<typeof getState>
export type RoomControl = Parameters<typeof music.enterRoom>[1]
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

if (typeof window !== 'undefined') window.addEventListener('pagehide', () => void player.flush())
if (typeof navigator !== 'undefined' && 'mediaSession' in navigator)
  setInterval(() => {
    const e = current(),
      p = position()
    if (!e || !p || e.track.duration <= 0) return
    try {
      navigator.mediaSession.setPositionState({ duration: e.track.duration, position: Math.min(p.time, e.track.duration), playbackRate: 1 })
    } catch {}
  }, 1000)
