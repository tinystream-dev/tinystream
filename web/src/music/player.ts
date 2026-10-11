// SPDX-License-Identifier: AGPL-3.0-or-later
// The music player: what's queued, what's playing, and keeping the server's
// copy of the queue in step so any app (or another browser) can carry on.

import { useEffect, useState, useSyncExternalStore } from 'react'
import { request } from '../lib/api'
import { MeasureLoudness, NowPlaying, PlayQueueQuery, Played, type MusicTrack, type Repeat, SavePlayQueue, TrackQuery, cover } from './api'
import { Engine, type Item } from './engine'

export type Entry = { uid: string; track: MusicTrack; fallback?: boolean }

/** Another app playing (Feishin, Symfonium…), shown here without a sound. */
export type Remote = {
  client: string
  track: number
  paused: boolean
  /** Seconds in, as of `at`. */
  position: number
  at: number
}

export type GainMode = 'off' | 'track' | 'album' | 'auto'

export type Settings = {
  volume: number
  muted: boolean
  gain: GainMode
  /** Seconds; 0 is off. */
  crossfade: number
}

export type State = {
  queue: Entry[]
  index: number
  playing: boolean
  /** Waiting on the network or the decoder. */
  waiting: boolean
  repeat: Repeat
  shuffled: boolean
  /** The order before shuffling, to go back to. */
  unshuffled: Entry[] | null
  /** Put away by its owner; the queue stays. */
  dismissed: boolean
  /** Out of the way while a video plays. */
  suspended: boolean
  /** Where it was left off, until playing starts again. */
  resumeAt: number
  error: string | null
  settings: Settings
  /** The listening room being followed, if any. */
  room: string | null
  /** Another app this player is mirroring, until playing here takes over. */
  remote: Remote | null
}

const SETTINGS = 'tinystream.music'
const DISMISSED = 'tinystream.music.dismissed'

function loadSettings(): Settings {
  const defaults: Settings = { volume: 1, muted: false, gain: 'auto', crossfade: 0 }
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(SETTINGS) ?? '{}') }
  } catch {
    return defaults
  }
}

/** Codecs the decoder reads; anything else comes from the server as FLAC. */
const DECODES = new Set(['flac', 'mp3', 'aac', 'alac', 'vorbis', 'pcm'])

let uid = 0
const entry = (track: MusicTrack): Entry => ({ uid: `e${++uid}`, track })

let state: State = {
  queue: [],
  index: 0,
  playing: false,
  waiting: false,
  repeat: 'OFF',
  shuffled: false,
  unshuffled: null,
  dismissed: typeof localStorage !== 'undefined' && localStorage.getItem(DISMISSED) === '1',
  suspended: false,
  resumeAt: 0,
  error: null,
  room: null,
  remote: null,
  settings: typeof localStorage !== 'undefined' ? loadSettings() : { volume: 1, muted: false, gain: 'auto', crossfade: 0 },
}

const subscribers = new Set<() => void>()

function set(patch: Partial<State>) {
  state = { ...state, ...patch }
  subscribers.forEach((f) => f())
}

export const getState = () => state

export function usePlayer<T>(select: (s: State) => T): T {
  return useSyncExternalStore(
    (f) => (subscribers.add(f), () => subscribers.delete(f)),
    () => select(state),
    () => select(state),
  )
}

export const current = (s: State = state): Entry | null => s.queue[s.index] ?? null

/** The rate sound goes out at, once it's been set up. */
export const outputRate = (): number | null => (engine ? engine.rate : null)

let engine: Engine | null = null

/** In a listening room, controls go to the room, and the room drives the player. */
export type RoomControl = {
  play: () => void
  pause: () => void
  seek: (time: number) => void
  skip: (index: number) => void
  add: (tracks: MusicTrack[], next: boolean) => void
  remove: (index: number) => void
  move: (from: number, to: number) => void
}

let room: RoomControl | null = null

function sound(): Engine {
  if (engine) return engine
  engine = new Engine()
  engine.on('track', (key) => {
    const i = state.queue.findIndex((e) => e.uid === key)
    if (i >= 0 && i !== state.index) set({ index: i, resumeAt: 0 })
    started(current())
  })
  engine.on('ended', () => {
    if (state.repeat === 'ALL' && state.queue.length) {
      void jump(0, true)
    } else {
      set({ playing: false, resumeAt: 0 })
      engine?.pause()
      void save()
    }
  })
  engine.on('starved', (waiting) => waiting !== state.waiting && set({ waiting }))
  engine.on('error', (key, message) => {
    const i = state.queue.findIndex((e) => e.uid === key)
    const e = state.queue[i]
    if (!e) return
    if (!e.fallback) {
      // The decoder couldn't read it: get it from the server as FLAC and try again.
      const queue = state.queue.slice()
      queue[i] = { ...e, fallback: true }
      set({ queue })
      if (i === state.index) void jump(i, state.playing, true)
      else lineUp()
      return
    }
    console.warn('music:', message)
    set({ error: `Can't play “${e.track.title}”` })
  })
  return engine
}

/** How loud to play an entry: ReplayGain by the mode chosen, held back from clipping. */
function gainOf(i: number, queue = state.queue): number {
  const e = queue[i]
  if (!e) return 1
  const g = e.track.gains
  let mode = state.settings.gain
  if (mode === 'auto') {
    // Album gain when an album plays in order, so its quiet songs stay quiet.
    const same = (o?: Entry) =>
      !!o && o.track.albumId != null && o.track.albumId === e.track.albumId && !state.shuffled
    mode = same(queue[i - 1]) || same(queue[i + 1]) ? 'album' : 'track'
  }
  if (mode === 'off') return 1
  const db = (mode === 'album' ? (g.albumGain ?? g.trackGain) : g.trackGain) ?? 0
  const peak = (mode === 'album' ? (g.albumPeak ?? g.trackPeak) : g.trackPeak) ?? 0
  let gain = 10 ** (db / 20)
  if (peak > 0 && gain * peak > 1) gain = 1 / peak
  return gain
}

/** One track after the other on the same album never crossfades; that's gapless. */
function fadeAfter(i: number, queue = state.queue): number {
  const a = queue[i]?.track
  const b = queue[i + 1]?.track
  if (!a || !b || !state.settings.crossfade) return 0
  if (a.albumId != null && a.albumId === b.albumId && (b.number ?? 0) === (a.number ?? 0) + 1) return 0
  return state.settings.crossfade
}

function item(i: number, queue = state.queue): Item {
  const e = queue[i]
  const t = e.track
  const direct = !e.fallback && DECODES.has(t.codec)
  return {
    key: e.uid,
    url: direct ? t.file : t.flac,
    size: direct ? t.size : 0,
    ext: direct ? t.suffix : 'flac',
    gain: gainOf(i, queue),
    crossfade: fadeAfter(i, queue),
  }
}

/** The order the decoder should go through: what's after `from`, then the start again on repeat. */
function upcoming(from: number): Item[] {
  const q = state.queue
  if (state.repeat === 'ONE') return [item(from)]
  const out: Item[] = []
  for (let i = from + 1; i < q.length && out.length < 3; i++) out.push(item(i))
  return out
}

/** Tells the decoder what follows the current track, after the queue changed. */
function lineUp() {
  const e = current()
  if (!e || !engine) return
  engine.upcoming(e.uid, upcoming(state.index))
  measureAhead()
}

const measuring = new Map<number, Promise<void>>()

/** Has the server measure a track's loudness if it hasn't yet, and uses what it found. */
function measure(trackId: number): Promise<void> {
  let p = measuring.get(trackId)
  if (!p) {
    p = request(MeasureLoudness, { trackId })
      .then(({ measureLoudness: t }) => {
        const queue = state.queue.map((e) => (e.track.id === t.id ? { ...e, track: { ...e.track, gains: t.gains } } : e))
        set({ queue })
        queue.forEach((e, i) => e.track.id === t.id && engine?.gain(e.uid, gainOf(i)))
      })
      .catch(() => {})
      .finally(() => measuring.delete(trackId))
    measuring.set(trackId, p)
  }
  return p
}

function measureAhead() {
  if (state.settings.gain === 'off') return
  for (const e of state.queue.slice(state.index + 1, state.index + 3)) if (e.track.gains.pending) void measure(e.track.id)
}

/** Starts the entry at `i`, `at` seconds in. */
async function jump(i: number, play: boolean, keepPosition = false, at = 0) {
  const e = state.queue[i]
  if (!e) return
  const start = keepPosition ? (position()?.time ?? state.resumeAt) : at
  set({ index: i, error: null, waiting: play, resumeAt: start, dismissed: false, remote: null })
  localStorage.removeItem(DISMISSED)
  const s = sound()
  if (e.track.gains.pending && state.settings.gain !== 'off') {
    // Measured just now, so the first second is already at the right level; it takes a moment.
    await Promise.race([measure(e.track.id), new Promise((r) => setTimeout(r, 4000))])
  }
  const items = [item(state.index), ...upcoming(state.index)]
  // Only a fresh start may change the rate sound goes out at; carrying on keeps it.
  await s.load(items, start, keepPosition ? undefined : (e.track.sampleRate ?? undefined))
  s.volume(state.settings.muted ? 0 : state.settings.volume)
  if (play) {
    await s.play()
    set({ playing: true })
    silence(true)
  }
  measureAhead()
  started(current())
  void save()
}

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

let scrobbled: string | null = null

function started(e: Entry | null) {
  if (!e || typeof navigator === 'undefined') return
  const t = e.track
  if ('mediaSession' in navigator) {
    navigator.mediaSession.metadata = new MediaMetadata({
      title: t.title,
      artist: t.artist,
      album: t.album,
      artwork: t.cover ? [256, 512].map((s) => ({ src: cover(t.cover, s / 2)!, sizes: `${s}x${s}` })) : [],
    })
  }
  void request(NowPlaying, { trackId: t.id, position: position()?.time ?? 0, paused: !state.playing }).catch(() => {})
}

/** Where in the current track we are, in seconds. */
export function position(): { key: string; time: number } | null {
  const p = engine?.position()
  const e = current()
  if (p && e && p.key === e.uid) return p
  if (!e) return null
  const r = state.remote
  const time = r && !r.paused ? Math.min(r.position + (Date.now() - r.at) / 1000, e.track.duration) : state.resumeAt
  return { key: e.uid, time }
}

/** The current position, updated every frame while something is on screen to show it. */
export function usePosition(): number {
  const [time, setTime] = useState(() => position()?.time ?? 0)
  const playing = usePlayer((s) => s.playing || (s.remote != null && !s.remote.paused))
  const index = usePlayer((s) => s.index)
  // Paused, the engine may still hold where it was before a seek; resumeAt already knows where it'll pick up.
  const resumeAt = usePlayer((s) => s.resumeAt)
  useEffect(() => {
    let raf = 0
    const tick = () => {
      setTime(playing ? (position()?.time ?? 0) : current() ? resumeAt : 0)
      if (playing) raf = requestAnimationFrame(tick)
    }
    tick()
    return () => cancelAnimationFrame(raf)
  }, [playing, index, resumeAt])
  return time
}

// Counts a play once enough of it was heard: half of it, or four minutes.
const browser = typeof window !== 'undefined'
if (browser) setInterval(() => {
  const e = current()
  const p = position()
  if (!state.playing || !e || !p) return
  if (scrobbled !== e.uid && p.time >= Math.min(e.track.duration / 2, 240)) {
    scrobbled = e.uid
    void request(Played, { trackId: e.track.id }).catch(() => {})
  }
  if ('mediaSession' in navigator && e.track.duration > 0) {
    try {
      navigator.mediaSession.setPositionState({ duration: e.track.duration, position: Math.min(p.time, e.track.duration), playbackRate: 1 })
    } catch {
      // Some browsers are picky about positions at the very end.
    }
  }
}, 1000)

let saving: ReturnType<typeof setTimeout> | null = null

/** Writes the queue to the server soon, so other apps see it. */
function save(soon = 1500): Promise<void> {
  if (saving) clearTimeout(saving)
  return new Promise((resolve) => {
    saving = setTimeout(async () => {
      saving = null
      if ((state.queue.length === 0 && !restored) || state.room || state.remote) return resolve()
      const input = {
        tracks: state.queue.map((e) => e.track.id),
        current: state.index,
        position: position()?.time ?? 0,
        shuffled: state.shuffled,
        repeat: state.repeat,
      }
      await request(SavePlayQueue, { input }).catch(() => {})
      resolve()
    }, soon)
  })
}

if (browser) setInterval(() => state.playing && void save(0), 30_000)
if (browser) window.addEventListener('pagehide', () => state.queue.length && void save(0))

let restored = false

/** Picks up the queue where it was left, here or in another app; paused, as it was. */
export async function restore() {
  if (restored) return
  restored = true
  const { playQueue: q } = await request(PlayQueueQuery).catch(() => ({ playQueue: null }))
  if (!q || state.queue.length || !q.tracks.length) return
  set({
    queue: q.tracks.map(entry),
    index: q.current,
    resumeAt: q.position,
    repeat: q.repeat,
    shuffled: q.shuffled,
  })
}

/** Entries for these tracks, keeping the ones already in place so nothing's thrown away. */
const keep = (tracks: MusicTrack[]): Entry[] => tracks.map((t, i) => (state.queue[i]?.track.id === t.id ? state.queue[i] : entry(t)))

/** Another app changed the queue; take it over unless something's playing here. */
export async function refreshFromServer() {
  if (state.playing) return
  const { playQueue: q } = await request(PlayQueueQuery)
  // While mirroring, stay on what the other app last said it's playing.
  const r = state.remote
  const index = r ? q.tracks.findIndex((t) => t.id === r.track) : q.current
  if (index < 0) return
  set({ queue: keep(q.tracks), index, resumeAt: r ? state.resumeAt : q.position, repeat: q.repeat, shuffled: q.shuffled })
}

/** The last each app said, so its regular check-ins aren't taken for a fresh start. */
const heard = new Map<string, { track: number; paused: boolean }>()
let following = 0

/**
 * Another app started, paused, moved or stopped: show the same track at the
 * same spot without playing it. Playing here stops for a fresh start elsewhere,
 * and pressing play here takes over.
 */
export async function followRemote(client: string, track: number | null, time: number, paused: boolean) {
  const before = heard.get(client)
  if (track == null) heard.delete(client)
  else heard.set(client, { track, paused })
  if (state.room) return
  if (track == null) {
    if (state.remote?.client === client) set({ resumeAt: position()?.time ?? state.resumeAt, remote: null })
    return
  }
  const fresh = !before || before.track !== track || (before.paused && !paused)
  if (state.playing && (paused || !fresh)) return
  const ticket = ++following
  let queue = state.queue
  let index = current()?.track.id === track ? state.index : -1
  if (index < 0) {
    const { playQueue: q } = await request(PlayQueueQuery).catch(() => ({ playQueue: null }))
    const i = q?.tracks.findIndex((t) => t.id === track) ?? -1
    if (q && i >= 0) {
      queue = keep(q.tracks)
      index = i
    } else if ((index = state.queue.findIndex((e) => e.track.id === track)) < 0) {
      const found = await request(TrackQuery, { id: track }).catch(() => ({ track: null }))
      if (!found.track) return
      queue = [entry(found.track)]
      index = 0
    }
  }
  if (ticket !== following || state.room) return
  if (state.playing || engine?.position()) {
    engine?.stop()
    silence(false)
  }
  set({
    queue,
    index,
    playing: false,
    waiting: false,
    error: null,
    dismissed: false,
    resumeAt: time,
    remote: { client, track, paused, position: time, at: Date.now() },
  })
  localStorage.removeItem(DISMISSED)
}

function shuffledCopy<T>(list: T[]): T[] {
  const out = list.slice()
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    ;[out[i], out[j]] = [out[j], out[i]]
  }
  return out
}

export const music = {
  /** Plays these tracks, from the one at `start`. */
  async play(tracks: MusicTrack[], start = 0, shuffle = false) {
    if (!tracks.length) return
    let queue = tracks.map(entry)
    let index = Math.max(0, Math.min(start, queue.length - 1))
    let unshuffled: Entry[] | null = null
    if (shuffle) {
      unshuffled = queue
      queue = shuffledCopy(queue)
      index = 0
    }
    set({ queue, index, shuffled: shuffle, unshuffled, suspended: false })
    await jump(index, true)
  },

  async toggle() {
    if (room) return state.playing ? room.pause() : room.play()
    if (state.playing) return music.pause()
    return music.resume()
  },

  async resume() {
    if (room) return room.play()
    const e = current()
    if (!e) return
    const s = sound()
    if (state.suspended) set({ suspended: false })
    if (!engine || !engine.position()) return jump(state.index, true, false, position()?.time ?? state.resumeAt)
    await s.play()
    set({ playing: true, dismissed: false })
    silence(true)
    started(e)
  },

  pause() {
    if (room) return room.pause()
    if (!state.playing) return
    engine?.pause()
    set({ playing: false, resumeAt: position()?.time ?? state.resumeAt })
    silence(false)
    void request(NowPlaying, { trackId: current()?.track.id, position: state.resumeAt, paused: true }).catch(() => {})
    void save(0)
  },

  seek(time: number) {
    if (room) return room.seek(time)
    const e = current()
    if (!e) return
    const t = Math.max(0, Math.min(time, e.track.duration - 0.25))
    if (!engine) return set({ resumeAt: t, remote: null })
    void jump(state.index, state.playing, false, t)
  },

  next() {
    if (room) return state.index + 1 < state.queue.length && room.skip(state.index + 1)
    if (state.index + 1 < state.queue.length) void jump(state.index + 1, true)
    else if (state.repeat === 'ALL' && state.queue.length) void jump(0, true)
  },

  /** Back to the start of this one, or to the one before when it's only just begun. */
  previous() {
    if (room) return (position()?.time ?? 0) > 3 || state.index === 0 ? room.seek(0) : room.skip(state.index - 1)
    const p = position()?.time ?? 0
    if (p > 3 || state.index === 0) music.seek(0)
    else void jump(state.index - 1, true)
  },

  jumpTo(index: number) {
    if (room) return room.skip(index)
    void jump(index, true)
  },

  /** Plays these right after the current track. */
  playNext(tracks: MusicTrack[]) {
    if (room) return room.add(tracks, true)
    if (!state.queue.length) return music.play(tracks)
    const queue = state.queue.slice()
    queue.splice(state.index + 1, 0, ...tracks.map(entry))
    set({ queue })
    lineUp()
    void save()
  },

  add(tracks: MusicTrack[]) {
    if (room) return room.add(tracks, false)
    if (!state.queue.length) return music.play(tracks)
    set({ queue: [...state.queue, ...tracks.map(entry)] })
    lineUp()
    void save()
  },

  remove(uid: string) {
    if (room) return room.remove(state.queue.findIndex((e) => e.uid === uid))
    const i = state.queue.findIndex((e) => e.uid === uid)
    if (i < 0) return
    if (i === state.index) {
      const queue = state.queue.filter((e) => e.uid !== uid)
      if (!queue.length) return music.clear()
      set({ queue, index: Math.min(i, queue.length - 1) })
      void jump(state.index, state.playing)
      return
    }
    set({ queue: state.queue.filter((e) => e.uid !== uid), index: i < state.index ? state.index - 1 : state.index })
    lineUp()
    void save()
  },

  move(from: number, to: number) {
    if (room) return room.move(from, to)
    const queue = state.queue.slice()
    const [e] = queue.splice(from, 1)
    queue.splice(to, 0, e)
    const playing = current()
    set({ queue, index: queue.findIndex((x) => x === playing) })
    lineUp()
    void save()
  },

  clear() {
    engine?.stop()
    silence(false)
    set({ queue: [], index: 0, playing: false, resumeAt: 0, unshuffled: null, shuffled: false })
    void request(NowPlaying, { trackId: null, position: 0, paused: true }).catch(() => {})
    void save(0)
  },

  shuffle(on = !state.shuffled) {
    const e = current()
    if (!e) return
    if (on) {
      const rest = shuffledCopy(state.queue.filter((x) => x !== e))
      set({ queue: [e, ...rest], index: 0, shuffled: true, unshuffled: state.queue })
    } else {
      const back = state.unshuffled?.filter((x) => state.queue.includes(x)) ?? state.queue
      const extra = state.queue.filter((x) => !back.includes(x))
      const queue = [...back, ...extra]
      set({ queue, index: queue.indexOf(e), shuffled: false, unshuffled: null })
    }
    lineUp()
    void save()
  },

  repeat(mode: Repeat) {
    set({ repeat: mode })
    lineUp()
    void save()
  },

  /** Puts the player away, pausing it first; the queue stays for later. A room plays on, so it stays. */
  dismiss() {
    if (room) return
    music.pause()
    set({ dismissed: true })
    localStorage.setItem(DISMISSED, '1')
  },

  /** Steps aside for a video: pauses, and keeps out of sight until it's over. */
  suspend() {
    if (state.playing) music.pause()
    set({ suspended: true })
  },

  unsuspend() {
    set({ suspended: false })
  },

  settings(patch: Partial<Settings>) {
    const settings = { ...state.settings, ...patch }
    set({ settings })
    localStorage.setItem(SETTINGS, JSON.stringify(settings))
    if ('volume' in patch || 'muted' in patch) engine?.volume(settings.muted ? 0 : settings.volume)
    if ('gain' in patch) {
      state.queue.forEach((e, i) => engine?.gain(e.uid, gainOf(i)))
      measureAhead()
    }
    if ('crossfade' in patch) lineUp()
  },

  dismissError() {
    set({ error: null })
  },

  /** Hands the controls to a room; the personal queue waits on the server meanwhile. */
  async enterRoom(code: string, control: RoomControl) {
    if (state.queue.length && !room) await save(0)
    engine?.stop()
    room = control
    set({ room: code, queue: [], index: 0, playing: false, resumeAt: 0, dismissed: false, suspended: false, remote: null })
  },

  /** Back to your own queue, paused where you left it. */
  async leaveRoom() {
    room = null
    engine?.stop()
    silence(false)
    set({ room: null, queue: [], index: 0, playing: false, remote: null })
    restored = false
    await restore()
  },

  /**
   * Makes the player match a room: its queue, the track it's on, and where in
   * it. Small differences are left alone; the decoder keeps everyone gapless.
   */
  async follow(tracks: MusicTrack[], index: number, time: number, playing: boolean) {
    const same = tracks.length === state.queue.length && tracks.every((t, i) => state.queue[i]?.track.id === t.id)
    if (!same) {
      // Keep entries that are still there, so the decoder's work isn't thrown away.
      set({ queue: keep(tracks) })
      if (engine && state.index === index) lineUp()
    }
    const here = position()
    const local = here ? state.queue.slice(0, state.index).reduce((n, e) => n + e.track.duration, 0) + here.time : 0
    const target = state.queue.slice(0, index).reduce((n, e) => n + e.track.duration, 0) + time
    const off = !engine || state.index !== index || (playing ? Math.abs(local - target) > 0.35 : Math.abs(local - target) > 1)
    if (off) {
      await jump(index, playing, false, time)
      return
    }
    if (playing && !state.playing) {
      await engine!.play()
      set({ playing: true })
      silence(true)
    } else if (!playing && state.playing) {
      engine!.pause()
      set({ playing: false, resumeAt: time })
      silence(false)
    }
  },

  supported: Engine.supported,
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
