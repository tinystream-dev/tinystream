// SPDX-License-Identifier: AGPL-3.0-or-later
import { useEffect, useState, useSyncExternalStore } from 'react'
import type { AudioEngine, MusicTrack, Repeat } from './music'
import { gainOf, fadeAfter, shuffledCopy } from './music-policy'

export type Entry = { uid: string; track: MusicTrack; fallback?: boolean }
export type GainMode = 'off' | 'track' | 'album' | 'auto'
export type Settings = { volume: number; muted: boolean; gain: GainMode; crossfade: number }
export type QueueSnapshot = { tracks: MusicTrack[]; current: number; position: number; shuffled: boolean; repeat: Repeat }
export type PlayerHost = {
  extraCodecs?: readonly string[]
  storage: { getItem(key: string): string | null; setItem(key: string, value: string): void; removeItem(key: string): void }
  engine(): AudioEngine
  animate?(fn: () => void): number
  cancelAnimation?(id: number): void
  timers?: boolean
  supported(): boolean
  playing?(on: boolean): void
  metadata?(track: MusicTrack): void
  queue(): Promise<{ playQueue: QueueSnapshot | null }>
  track(id: number): Promise<{ track: MusicTrack | null }>
  measure(id: number): Promise<{ measureLoudness: MusicTrack }>
  save(input: { tracks: number[]; current: number; position: number; shuffled: boolean; repeat: Repeat }): Promise<unknown>
  played(id: number): Promise<unknown>
  nowPlaying(id: number | null | undefined, position: number, paused: boolean): Promise<unknown>
}
let playerId = 0
export function createMusicPlayer(host: PlayerHost) {
  const identity = ++playerId
  /** Another app playing (Feishin, Symfonium…), shown here without a sound. */
  type Remote = {
    client: string
    track: number
    paused: boolean
    /** Seconds in, as of `at`. */
    position: number
    at: number
  }

  type State = {
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
      return { ...defaults, ...JSON.parse(host.storage.getItem(SETTINGS) ?? '{}') }
    } catch {
      return defaults
    }
  }

  /** Codecs the decoder reads; anything else comes from the server as FLAC. */
  const DECODES = new Set(['flac', 'mp3', 'aac', 'alac', 'vorbis', 'pcm', ...(host.extraCodecs ?? [])])

  let uid = 0
  const entry = (track: MusicTrack): Entry => ({ uid: `p${identity}e${++uid}`, track })

  let state: State = {
    queue: [],
    index: 0,
    playing: false,
    waiting: false,
    repeat: 'OFF',
    shuffled: false,
    unshuffled: null,
    dismissed: host.storage.getItem(DISMISSED) === '1',
    suspended: false,
    resumeAt: 0,
    error: null,
    room: null,
    remote: null,
    settings: loadSettings(),
  }

  const subscribers = new Set<() => void>()

  function set(patch: Partial<State>) {
    state = { ...state, ...patch }
    subscribers.forEach((f) => f())
    if (engine && state.queue[state.index]) host.metadata?.(state.queue[state.index].track)
  }

  const getState = () => state

  function usePlayer<T>(select: (s: State) => T): T {
    return useSyncExternalStore(
      (f) => (subscribers.add(f), () => subscribers.delete(f)),
      () => select(state),
      () => select(state),
    )
  }

  const current = (s: State = state): Entry | null => s.queue[s.index] ?? null

  /** The rate sound goes out at, once it's been set up. */
  const outputRate = (): number | null => (engine ? engine.rate : null)

  let engine: AudioEngine | null = null
  let disposed = false
  let loading = 0
  let exhausted = false

  /** In a listening room, controls go to the room, and the room drives the player. */
  type RoomControl = {
    play: () => void
    pause: () => void
    seek: (time: number) => void
    skip: (index: number) => void
    add: (tracks: MusicTrack[], next: boolean) => void
    remove: (index: number) => void
    move: (from: number, to: number) => void
  }

  let room: RoomControl | null = null

  function sound(): AudioEngine {
    if (engine) return engine
    engine = host.engine()
    engine.on('track', (key) => {
      const i = state.queue.findIndex((e) => e.uid === key)
      if (i >= 0 && i !== state.index) set({ index: i, resumeAt: 0 })
      lineUp()
      scrobbled = null
      started(current())
    })
    engine.on('ended', () => {
      exhausted = true
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

  function item(i: number, queue = state.queue): import('./music').Item {
    const e = queue[i]
    const t = e.track
    const direct = !e.fallback && DECODES.has(t.codec)
    return {
      key: e.uid,
      url: direct ? t.file : t.flac,
      size: direct ? t.size : 0,
      ext: direct ? t.suffix : 'flac',
      gain: gainOf(i, queue, state.settings, state.shuffled),
      crossfade: fadeAfter(i, queue, state.settings),
    }
  }

  /** The order the decoder should go through: what's after `from`, then the start again on repeat. */
  function upcoming(from: number): import('./music').Item[] {
    const q = state.queue
    if (state.repeat === 'ONE') return [item(from)]
    const out: import('./music').Item[] = []
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
      p = host
        .measure(trackId)
        .then(({ measureLoudness: t }) => {
          if (disposed) return
          const queue = state.queue.map((e) => (e.track.id === t.id ? { ...e, track: { ...e.track, gains: t.gains } } : e))
          set({ queue })
          queue.forEach((e, i) => e.track.id === t.id && engine?.gain(e.uid, gainOf(i, state.queue, state.settings, state.shuffled)))
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
    exhausted = false
    const ticket = ++loading
    const e = state.queue[i]
    if (!e || disposed) return
    const start = keepPosition ? (position()?.time ?? state.resumeAt) : at
    set({ index: i, error: null, waiting: play, resumeAt: start, dismissed: false, remote: null })
    host.storage.removeItem(DISMISSED)
    const s = sound()
    if (e.track.gains.pending && state.settings.gain !== 'off') {
      // Measured just now, so the first second is already at the right level; it takes a moment.
      await Promise.race([measure(e.track.id), new Promise((r) => setTimeout(r, 4000))])
    }
    if (disposed || ticket !== loading) return
    const items = [item(state.index), ...upcoming(state.index)]
    // Only a fresh start may change the rate sound goes out at; carrying on keeps it.
    if (!play) s.pause()
    await s.load(items, start, keepPosition ? undefined : (e.track.sampleRate ?? undefined))
    if (disposed || ticket !== loading) return
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

  const silence = (on: boolean) => host.playing?.(on)

  let scrobbled: string | null = null

  function started(e: Entry | null) {
    if (!e) return
    const t = e.track
    host.metadata?.(t)
    void host.nowPlaying(t.id, position()?.time ?? 0, !state.playing).catch(() => {})
  }

  /** Where in the current track we are, in seconds. */
  function position(): { key: string; time: number } | null {
    const p = engine?.position()
    const e = current()
    if (p && e && p.key === e.uid) return p
    if (!e) return null
    const r = state.remote
    const time = r && !r.paused ? Math.min(r.position + (Date.now() - r.at) / 1000, e.track.duration) : state.resumeAt
    return { key: e.uid, time }
  }

  /** The current position, updated every frame while something is on screen to show it. */
  function usePosition(): number {
    const [time, setTime] = useState(() => position()?.time ?? 0)
    const playing = usePlayer((s) => s.playing || (s.remote != null && !s.remote.paused))
    const index = usePlayer((s) => s.index)
    // Paused, the engine may still hold where it was before a seek; resumeAt already knows where it'll pick up.
    const resumeAt = usePlayer((s) => s.resumeAt)
    useEffect(() => {
      let raf = 0
      const tick = () => {
        setTime(playing ? (position()?.time ?? 0) : current() ? resumeAt : 0)
        if (playing) raf = host.animate?.(tick) ?? 0
      }
      tick()
      return () => host.cancelAnimation?.(raf)
    }, [playing, index, resumeAt])
    return time
  }

  // Counts a play once enough of it was heard: half of it, or four minutes.
  const timers: ReturnType<typeof setInterval>[] = []
  const browser = host.timers !== false
  function tick() {
    const e = current(),
      p = position()
    if (disposed || !state.playing || !e || !p) return
    if (scrobbled !== e.uid && p.time >= Math.min(e.track.duration / 2, 240)) {
      scrobbled = e.uid
      void host.played(e.track.id).catch(() => {})
    }
  }
  if (browser) timers.push(setInterval(tick, 1000))

  let saving: ReturnType<typeof setTimeout> | null = null

  /** Writes the queue to the server soon, so other apps see it. */
  function save(soon = 1500): Promise<void> {
    if (saving) clearTimeout(saving)
    const write = async () => {
      saving = null
      if (disposed || (state.queue.length === 0 && !restored) || state.room || state.remote) return
      const input = {
        tracks: state.queue.map((e) => e.track.id),
        current: state.index,
        position: position()?.time ?? 0,
        shuffled: state.shuffled,
        repeat: state.repeat,
      }
      await host.save(input).catch(() => {})
    }
    if (soon <= 0) return write()
    return new Promise((resolve) => {
      saving = setTimeout(() => void write().then(resolve), soon)
    })
  }

  if (browser) timers.push(setInterval(() => state.playing && void save(0), 30_000))

  let restored = false

  /** Picks up the queue where it was left, here or in another app; paused, as it was. */
  async function restore() {
    if (restored) return
    restored = true
    const { playQueue: q } = await host.queue().catch(() => ({ playQueue: null }))
    if (disposed || !q || state.queue.length || !q.tracks.length) return
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
  async function refreshFromServer() {
    if (state.playing) return
    const { playQueue: q } = await host.queue()
    if (!q || disposed) return
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
  async function followRemote(client: string, track: number | null, time: number, paused: boolean) {
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
      const { playQueue: q } = await host.queue().catch(() => ({ playQueue: null }))
      const i = q?.tracks.findIndex((t) => t.id === track) ?? -1
      if (q && i >= 0) {
        queue = keep(q.tracks)
        index = i
      } else if ((index = state.queue.findIndex((e) => e.track.id === track)) < 0) {
        const found = await host.track(track).catch(() => ({ track: null }))
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
    host.storage.removeItem(DISMISSED)
  }

  const music = {
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
      if (exhausted) {
        exhausted = false
        return jump(state.index, true, false, 0)
      }
      if (!s.position()) return jump(state.index, true, false, position()?.time ?? state.resumeAt)
      await s.play()
      set({ playing: true, dismissed: false })
      silence(true)
      started(e)
    },

    pause(alreadyPaused = false) {
      if (room) return room.pause()
      if (!state.playing) return
      if (!alreadyPaused) engine?.pause()
      set({ playing: false, resumeAt: position()?.time ?? state.resumeAt })
      silence(false)
      void host.nowPlaying(current()?.track.id, state.resumeAt, true).catch(() => {})
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
      set({ waiting: false, queue: [], index: 0, playing: false, resumeAt: 0, unshuffled: null, shuffled: false })
      void host.nowPlaying(null, 0, true).catch(() => {})
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
      host.storage.setItem(DISMISSED, '1')
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
      host.storage.setItem(SETTINGS, JSON.stringify(settings))
      if ('volume' in patch || 'muted' in patch) engine?.volume(settings.muted ? 0 : settings.volume)
      if ('gain' in patch) {
        state.queue.forEach((e, i) => engine?.gain(e.uid, gainOf(i, state.queue, state.settings, state.shuffled)))
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

    supported: host.supported,
  }

  return {
    music,
    subscribe: (fn: () => void) => {
      subscribers.add(fn)
      return () => {
        subscribers.delete(fn)
      }
    },
    getState,
    usePlayer,
    current,
    position,
    usePosition,
    outputRate,
    source: () => (current()?.fallback || !DECODES.has(current()?.track.codec ?? '') ? 'flac' : 'file'),
    restore,
    refreshFromServer,
    followRemote,
    tick,
    updateTrack(track: MusicTrack) {
      set({ queue: state.queue.map((e) => (e.track.id === track.id ? { ...e, track } : e)) })
      if (current()?.track.id === track.id) host.metadata?.(track)
    },
    flush: () => save(0),
    dispose() {
      disposed = true
      loading++
      following++
      timers.forEach(clearInterval)
      if (saving) clearTimeout(saving)
      engine?.stop()
      silence(false)
      subscribers.clear()
    },
  }
}
export type MusicPlayer = ReturnType<typeof createMusicPlayer>
