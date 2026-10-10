// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Entry, Settings } from './music-player'
export function gainOf(i: number, queue: Entry[], settings: Settings, shuffled: boolean): number {
  const e = queue[i]
  if (!e) return 1
  const g = e.track.gains
  let mode = settings.gain
  if (mode === 'auto') {
    const same = (o?: Entry) => !!o && o.track.albumId != null && o.track.albumId === e.track.albumId && !shuffled
    mode = same(queue[i - 1]) || same(queue[i + 1]) ? 'album' : 'track'
  }
  if (mode === 'off') return 1
  const db = (mode === 'album' ? (g.albumGain ?? g.trackGain) : g.trackGain) ?? 0
  const peak = (mode === 'album' ? (g.albumPeak ?? g.trackPeak) : g.trackPeak) ?? 0
  let gain = 10 ** (db / 20)
  if (peak > 0 && gain * peak > 1) gain = 1 / peak
  return gain
}

export function fadeAfter(i: number, queue: Entry[], settings: Settings): number {
  const a = queue[i]?.track
  const b = queue[i + 1]?.track
  if (!a || !b || !settings.crossfade) return 0
  if (a.albumId != null && a.albumId === b.albumId && (b.number ?? 0) === (a.number ?? 0) + 1) return 0
  return settings.crossfade
}

export function shuffledCopy<T>(list: T[]): T[] {
  const out = list.slice()
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    ;[out[i], out[j]] = [out[j], out[i]]
  }
  return out
}
