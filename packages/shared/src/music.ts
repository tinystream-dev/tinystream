// SPDX-License-Identifier: AGPL-3.0-or-later
// The music player both apps share: what's queued, what's playing, and keeping
// the server's copy of the queue in step so any app (or another browser) can
// carry on. Sound, storage and the API come from the app.

export type Repeat = 'ALL' | 'OFF' | 'ONE'

/** What the player needs of a track; the apps' own track types have more. */
export type MusicTrack = {
  id: number
  title: string
  artist: string
  album: string
  albumId: number | null
  number: number | null
  duration: number
  codec: string
  suffix: string
  size: number
  file: string
  flac: string
  sampleRate: number | null
  cover: string | null
  starred: boolean
  gains: { trackGain: number | null; trackPeak: number | null; albumGain: number | null; albumPeak: number | null; pending: boolean }
}

/** Something for the decoder to play. */
export type Item = {
  key: string
  url: string
  /** Bytes, or 0 to ask the server. */
  size: number
  ext: string
  /** Linear; ReplayGain with clipping kept off. */
  gain: number
  /** Seconds to overlap with the next track; 0 for none. */
  crossfade: number
}

export type EngineEvents = {
  /** A track began playing (it's audible now, not just decoded). */
  track: (key: string) => void
  /** Everything queued has been played. */
  ended: () => void
  error: (key: string, message: string) => void
  starved: (starved: boolean) => void
}

/** Sound: the web's worklet, or the app's native player. */
export interface AudioEngine {
  readonly rate: number
  on<K extends keyof EngineEvents>(event: K, fn: EngineEvents[K]): () => void
  /** The track you hear now, and how far into it. */
  position(): { key: string; time: number } | null
  /** Plays `items` in order, from `start` seconds into the first, throwing away whatever was lined up. */
  load(items: Item[], start?: number, rate?: number): Promise<void>
  /** What comes after `after` changed. */
  upcoming(after: string, items: Item[]): void
  gain(key: string, gain: number): void
  play(): Promise<void>
  pause(): void
  volume(v: number): void
  stop(): void
}

export type PlayQueue<T extends MusicTrack> = {
  tracks: T[]
  current: number
  position: number
  shuffled: boolean
  repeat: Repeat
}

export type QueueInput = { tracks: number[]; current: number; position: number; shuffled: boolean; repeat: Repeat }

export type MusicHost<T extends MusicTrack> = {
  engine(): AudioEngine
  /** Small things kept on this device. */
  storage: { get(key: string): string | null; set(key: string, value: string | null): void }
  queue(): Promise<PlayQueue<T>>
  track(id: number): Promise<T | null>
  /** Has the server measure a track's loudness; the track with its gains. */
  measure(trackId: number): Promise<T>
  save(input: QueueInput): Promise<unknown>
  played(trackId: number): Promise<unknown>
  nowPlaying(trackId: number | null, position: number, paused: boolean): Promise<unknown>
  /** A track began, or what's known about it changed: for the system's media controls. */
  started?(entry: Entry<T>): void
  /** Playing here started or stopped. */
  playing?(on: boolean): void
  /** Every second while playing, with where in the track it is. */
  progress?(entry: Entry<T>, time: number): void
}

export type Entry<T extends MusicTrack = MusicTrack> = { uid: string; track: T; fallback?: boolean }

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

export type State<T extends MusicTrack = MusicTrack> = {
  queue: Entry<T>[]
  index: number
  playing: boolean
  /** Waiting on the network or the decoder. */
  waiting: boolean
  repeat: Repeat
  shuffled: boolean
  /** The order before shuffling, to go back to. */
  unshuffled: Entry<T>[] | null
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

/** In a listening room, controls go to the room, and the room drives the player. */
export type RoomControl<T extends MusicTrack = MusicTrack> = {
  play: () => void
  pause: () => void
  seek: (time: number) => void
  skip: (index: number) => void
  add: (tracks: T[], next: boolean) => void
  remove: (index: number) => void
  move: (from: number, to: number) => void
}

const SETTINGS = 'tinystream.music'
const DISMISSED = 'tinystream.music.dismissed'

/** Codecs the decoder reads; anything else comes from the server as FLAC. */
export const DECODES = new Set(['flac', 'mp3', 'aac', 'alac', 'vorbis', 'pcm'])

export const DEFAULT_SETTINGS: Settings = { volume: 1, muted: false, gain: 'auto', crossfade: 0 }

/** How loud to play an entry: ReplayGain by the mode chosen, held back from clipping. */
export function gainOf(i: number, queue: Entry[], settings: Settings, shuffled: boolean): number {
  const e = queue[i]
  if (!e) return 1
  const g = e.track.gains
  let mode = settings.gain
  if (mode === 'auto') {
    // Album gain when an album plays in order, so its quiet songs stay quiet.
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

/** One track after the other on the same album never crossfades; that's gapless. */
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

let uid = 0

export function createPlayer<T extends MusicTrack>(host: MusicHost<T>, codecs: Iterable<string> = DECODES) {
  const decodes = new Set(codecs)
  const entry = (track: T): Entry<T> => ({ uid: `e${++uid}`, track })

  function loadSettings(): Settings {
    try {
      return { ...DEFAULT_SETTINGS, ...JSON.parse(host.storage.get(SETTINGS) ?? '{}') }
    } catch {
      return DEFAULT_SETTINGS
    }
  }

  let state: State<T> = {
    queue: [],
    index: 0,
    playing: false,
    waiting: false,
    repeat: 'OFF',
    shuffled: false,
    unshuffled: null,
    dismissed: host.storage.get(DISMISSED) === '1',
    suspended: false,
    resumeAt: 0,
    error: null,
    room: null,
    remote: null,
    settings: loadSettings(),
  }

  const subscribers = new Set<() => void>()

  function set(patch: Partial<State<T>>) {
    state = { ...state, ...patch }
    subscribers.forEach((f) => f())
  }

  const getState = () => state
  const subscribe = (f: () => void) => (subscribers.add(f), () => void subscribers.delete(f))
  const current = (s: State<T> = state): Entry<T> | null => s.queue[s.index] ?? null

  let engine: AudioEngine | null = null
  let room: RoomControl<T> | null = null

  let timers: ReturnType<typeof setInterval>[] = []

  function sound(): AudioEngine {
    if (engine) return engine
    engine = host.engine()
    timers = [setInterval(tick, 1000), setInterval(() => state.playing && void save(0), 30_000)]
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
        host.playing?.(false)
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

  function item(i: number, queue = state.queue): Item {
    const e = queue[i]
    const t = e.track
    const direct = !e.fallback && decodes.has(t.codec)
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
      p = host
        .measure(trackId)
        .then((t) => {
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
    const e = state.queue[i]
    if (!e) return
    const start = keepPosition ? (position()?.time ?? state.resumeAt) : at
    set({ index: i, error: null, waiting: play, resumeAt: start, dismissed: false, remote: null })
    host.storage.set(DISMISSED, null)
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
      host.playing?.(true)
    }
    measureAhead()
    started(current())
    void save()
  }

  function started(e: Entry<T> | null) {
    if (!e) return
    host.started?.(e)
    void host.nowPlaying(e.track.id, position()?.time ?? 0, !state.playing).catch(() => {})
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

  /** Where the current track is, for showing: where it'll pick up while paused. */
  function shown(): number {
    if (state.playing || (state.remote && !state.remote.paused)) return position()?.time ?? 0
    return current() ? state.resumeAt : 0
  }

  let scrobbled: string | null = null

  // Counts a play once enough of it was heard: half of it, or four minutes.
  function tick() {
    const e = current()
    const p = position()
    if (!state.playing || !e || !p) return
    if (scrobbled !== e.uid && p.time >= Math.min(e.track.duration / 2, 240)) {
      scrobbled = e.uid
      void host.played(e.track.id).catch(() => {})
    }
    host.progress?.(e, p.time)
  }

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
        await host.save(input).catch(() => {})
        resolve()
      }, soon)
    })
  }

  let restored = false

  /** Picks up the queue where it was left, here or in another app; paused, as it was. */
  async function restore() {
    if (restored) return
    restored = true
    const q = await host.queue().catch(() => null)
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
  const keep = (tracks: T[]): Entry<T>[] => tracks.map((t, i) => (state.queue[i]?.track.id === t.id ? state.queue[i] : entry(t)))

  /** Another app changed the queue; take it over unless something's playing here. */
  async function refreshFromServer() {
    if (state.playing) return
    const q = await host.queue()
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
      const q = await host.queue().catch(() => null)
      const i = q?.tracks.findIndex((t) => t.id === track) ?? -1
      if (q && i >= 0) {
        queue = keep(q.tracks)
        index = i
      } else if ((index = state.queue.findIndex((e) => e.track.id === track)) < 0) {
        const found = await host.track(track).catch(() => null)
        if (!found) return
        queue = [entry(found)]
        index = 0
      }
    }
    if (ticket !== following || state.room) return
    if (state.playing || engine?.position()) {
      engine?.stop()
      host.playing?.(false)
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
    host.storage.set(DISMISSED, null)
  }

  const music = {
    /** Plays these tracks, from the one at `start`. */
    async play(tracks: T[], start = 0, shuffle = false) {
      if (!tracks.length) return
      let queue = tracks.map(entry)
      let index = Math.max(0, Math.min(start, queue.length - 1))
      let unshuffled: Entry<T>[] | null = null
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
      if (!s.position()) return jump(state.index, true, false, position()?.time ?? state.resumeAt)
      await s.play()
      set({ playing: true, dismissed: false })
      host.playing?.(true)
      started(e)
    },

    pause() {
      if (room) return room.pause()
      if (!state.playing) return
      engine?.pause()
      set({ playing: false, resumeAt: position()?.time ?? state.resumeAt })
      host.playing?.(false)
      void host.nowPlaying(current()?.track.id ?? null, state.resumeAt, true).catch(() => {})
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
    playNext(tracks: T[]) {
      if (room) return room.add(tracks, true)
      if (!state.queue.length) return music.play(tracks)
      const queue = state.queue.slice()
      queue.splice(state.index + 1, 0, ...tracks.map(entry))
      set({ queue })
      lineUp()
      void save()
    },

    add(tracks: T[]) {
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
      host.playing?.(false)
      set({ queue: [], index: 0, playing: false, resumeAt: 0, unshuffled: null, shuffled: false })
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
      host.storage.set(DISMISSED, '1')
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
      host.storage.set(SETTINGS, JSON.stringify(settings))
      if ('volume' in patch || 'muted' in patch) engine?.volume(settings.muted ? 0 : settings.volume)
      if ('gain' in patch) {
        state.queue.forEach((e, i) => engine?.gain(e.uid, gainOf(i, state.queue, state.settings, state.shuffled)))
        measureAhead()
      }
      if ('crossfade' in patch) lineUp()
    },

    /** What's known about a track changed (it was starred, say). */
    update(track: T) {
      set({ queue: state.queue.map((e) => (e.track.id === track.id ? { ...e, track } : e)) })
      const e = current()
      if (e?.track.id === track.id) host.started?.(e)
    },

    dismissError() {
      set({ error: null })
    },

    /** Hands the controls to a room; the personal queue waits on the server meanwhile. */
    async enterRoom(code: string, control: RoomControl<T>) {
      if (state.queue.length && !room) await save(0)
      engine?.stop()
      room = control
      set({ room: code, queue: [], index: 0, playing: false, resumeAt: 0, dismissed: false, suspended: false, remote: null })
    },

    /** Back to your own queue, paused where you left it. */
    async leaveRoom() {
      room = null
      engine?.stop()
      host.playing?.(false)
      set({ room: null, queue: [], index: 0, playing: false, remote: null })
      restored = false
      await restore()
    },

    /**
     * Makes the player match a room: its queue, the track it's on, and where in
     * it. Small differences are left alone; the decoder keeps everyone gapless.
     */
    async follow(tracks: T[], index: number, time: number, playing: boolean) {
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
        host.playing?.(true)
      } else if (!playing && state.playing) {
        engine!.pause()
        set({ playing: false, resumeAt: time })
        host.playing?.(false)
      }
    },
  }

  return {
    music,
    getState,
    subscribe,
    current,
    position,
    shown,
    restore,
    refreshFromServer,
    followRemote,
    /** The rate sound goes out at, once it's been set up. */
    outputRate: (): number | null => (engine ? engine.rate : null),
    /** Saves the queue now, before the app goes away. */
    flush: () => (state.queue.length ? save(0) : Promise.resolve()),
    /** Stops playing and lets go of everything, for signing out. */
    dispose() {
      timers.forEach(clearInterval)
      if (saving) clearTimeout(saving)
      engine?.stop()
      host.playing?.(false)
      subscribers.clear()
    },
  }
}

export type Player<T extends MusicTrack = MusicTrack> = ReturnType<typeof createPlayer<T>>

/** "FLAC 24/96", "MP3 320", "ALAC 16/44.1": what a file is, the short way. */
export function quality(t: { codec: string; lossless: boolean; bitDepth: number | null; sampleRate: number | null; bitrate: number | null }): string {
  const name = t.codec === 'pcm' ? 'PCM' : t.codec === 'wavpack' ? 'WavPack' : t.codec === 'musepack' ? 'Musepack' : t.codec.toUpperCase()
  if (t.lossless && t.sampleRate) {
    const khz = t.sampleRate / 1000
    return `${name} ${t.bitDepth ?? 16}/${Number.isInteger(khz) ? khz : khz.toFixed(1)}`
  }
  return t.bitrate ? `${name} ${t.bitrate}` : name
}

/** Hi-res: more than CD's 16 bits or 48 kHz. */
export const hiRes = (t: { lossless: boolean; bitDepth: number | null; sampleRate: number | null }) =>
  t.lossless && ((t.bitDepth ?? 16) > 16 || (t.sampleRate ?? 44100) > 48000)

export function duration(secs: number): string {
  const s = Math.max(0, Math.round(secs))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const r = String(s % 60).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${r}` : `${m}:${r}`
}

/** "1 hr 12 min", for whole albums. */
export function length(secs: number): string {
  if (secs < 60) return `${Math.round(secs)} sec`
  const m = Math.round(secs / 60)
  return m >= 60 ? `${Math.floor(m / 60)} hr ${m % 60} min` : `${m} min`
}
