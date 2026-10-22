// SPDX-License-Identifier: AGPL-3.0-or-later
// Keeps one <video> on a watch-together room's clock.
//
// The server holds the room's clock: a position, the server time it was
// true at, a rate, and whether it's moving. We measure how far our clock is
// from the server's (NTP-style, keeping the quickest round trip), so we can
// work out exactly where the room is at any moment without asking. A small
// control loop then steers the video there: small drift is absorbed by
// briefly nudging the playback rate, which nobody notices; big drift is a seek.

import type { Playback } from '../lib/api'

export type Status = 'joining' | 'loading' | 'ready' | 'buffering' | 'blocked'

export type Member = { id: number; name: string; userId: number | null; avatar: number | null; status: Status }

export type Clock = { paused: boolean; running: boolean; position: number; at: number; rate: number }

/** The audio and subtitles everyone gets unless they pick their own. */
export type SharedTracks = {
  mediaId: number
  audio: number | null
  audioLanguage: string | null
  audioTitle: string | null
  audioChannels: number | null
  audioCodec: string | null
  /** null is off. */
  subtitle: string | null
  subtitleLanguage: string | null
  subtitleTitle: string | null
  subtitleForced: boolean
  subtitleCodec: string | null
}

type SubtitleTrack = Playback['media']['subtitles'][number]
/** What a picked subtitle track was, to find the same one in other files. */
export type SubtitleChoice = { language: string | null; title: string | null; forced: boolean; codec: string | null }

export function subtitleChoice(t: SubtitleTrack): SubtitleChoice {
  return { language: t.language, title: t.title, forced: t.forced, codec: t.codec }
}

/** How the room's tracks describe a subtitle track. */
export function roomSubtitle(t: SubtitleTrack | undefined) {
  return { subtitleLanguage: t?.language ?? null, subtitleTitle: t?.title ?? null, subtitleForced: t?.forced ?? false, subtitleCodec: t?.codec ?? null }
}

/** The track most like the one picked before: same language first, then name, then the rest. */
export function sameSubtitle(usable: SubtitleTrack[], want: SubtitleChoice) {
  const candidates = usable.filter((s) => (want.language ? s.language === want.language : !!want.title && s.title === want.title))
  const score = (s: SubtitleTrack) => (s.title === want.title ? 4 : 0) + (s.forced === want.forced ? 2 : 0) + (s.codec === want.codec ? 1 : 0)
  return candidates.reduce<SubtitleTrack | undefined>((best, s) => (!best || score(s) > score(best) ? s : best), undefined)?.id
}

type AudioTrack = Playback['media']['audio'][number]
/** What a picked audio track was, to find the same one in other files. */
export type AudioChoice = { language: string | null; title: string | null; channels: number | null; codec: string | null }

export function audioChoice(t: AudioTrack): AudioChoice {
  return { language: t.language, title: t.title, channels: t.channels, codec: t.codec }
}

/** How the room's tracks describe an audio track. */
export function roomAudio(t: AudioTrack | undefined) {
  return { audioLanguage: t?.language ?? null, audioTitle: t?.title ?? null, audioChannels: t?.channels ?? null, audioCodec: t?.codec ?? null }
}

/** The track most like the one picked before: same language first, then name, then the rest. */
export function sameAudio(audio: AudioTrack[], want: AudioChoice) {
  const candidates = audio.filter((a) => (want.language ? a.language === want.language : !!want.title && a.title === want.title))
  const score = (a: AudioTrack) => (a.title === want.title ? 4 : 0) + (a.channels === want.channels ? 2 : 0) + (a.codec === want.codec ? 1 : 0)
  return candidates.reduce<AudioTrack | undefined>((best, a) => (!best || score(a) > score(best) ? a : best), undefined)
}

export type RoomState = {
  code: string
  mediaId: number
  itemId: number
  host: { id: number; name: string }
  public: boolean
  invited: number[]
  settings: { control: 'everyone' | 'host'; waitForAll: boolean }
  tracks: SharedTracks
  clock: Clock
  members: Member[]
  action: { seq: number; member: number; by: string; kind: string; value: number | null } | null
}

export type SyncSnapshot = {
  room: RoomState | null
  /** Our member id in the room. */
  you: number | null
  connected: boolean
  /** The browser wants a tap before it'll play. */
  blocked: boolean
  ended: boolean
  /** The last thing the server refused, in words. */
  error: { message: string; at: number } | null
}

/** Drift we leave alone, in seconds. */
const DEADBAND = 0.015
/** Beyond this, seek instead of speeding up or slowing down. */
const HARD_DRIFT = 0.25
/**
 * How many loop readings small drift is judged over. Firefox moves
 * `currentTime` a frame at a time, so one reading can be ±20 ms off; a
 * second of them averages that out.
 */
const WINDOW = 10
/** How long a nudge takes to close the drift, in seconds. */
const CATCH_UP = 1
/** The most we'll bend the playback rate to catch up (±). */
const MAX_NUDGE = 0.08
/** Starving for this long (ms) while playing counts as buffering. */
const STARVE_MS = 400
/**
 * Seconds buffered ahead before someone who ran dry is ready again. Saying so
 * at the first frame back has a slow connection stop the room every few
 * seconds; this makes it fewer, longer waits.
 */
const REFILL = 5

export function positionAt(c: Clock, t: number) {
  return c.running ? c.position + (Math.max(0, t - c.at) / 1000) * c.rate : c.position
}

/** Seconds buffered past the playhead. */
function bufferedAhead(v: HTMLVideoElement) {
  const b = v.buffered
  for (let i = 0; i < b.length; i++) {
    if (b.start(i) <= v.currentTime + 0.1 && b.end(i) > v.currentTime) return b.end(i) - v.currentTime
  }
  return 0
}

export function canControl(room: RoomState, you: number | null) {
  if (room.settings.control === 'everyone') return true
  const hostHere = room.members.some((m) => m.userId === room.host.id)
  return !hostHere || room.members.some((m) => m.id === you && m.userId === room.host.id)
}

/**
 * The shared tracks for this video. Every browser works this out the same way
 * from the same facts, so moving to a new episode needs no extra round trip.
 */
export function pickShared(pb: Playback, t: SharedTracks): { audio: number | null; subtitle: string | null } {
  const same = t.mediaId === pb.id
  const audio =
    (same && pb.media.audio.find((a) => a.index === t.audio)) ||
    sameAudio(pb.media.audio, { language: t.audioLanguage, title: t.audioTitle, channels: t.audioChannels, codec: t.audioCodec }) ||
    pb.media.audio.find((a) => a.default) ||
    pb.media.audio[0]
  const usable = pb.media.subtitles.filter((s) => s.supported)
  let subtitle: string | null = null
  if (t.subtitle !== null) {
    subtitle =
      (same && usable.find((s) => s.id === t.subtitle)?.id) ||
      sameSubtitle(usable, { language: t.subtitleLanguage, title: t.subtitleTitle, forced: t.subtitleForced, codec: t.subtitleCodec }) ||
      usable.find((s) => s.default)?.id ||
      usable.find((s) => !s.forced)?.id ||
      null
  }
  return { audio: audio?.index ?? null, subtitle }
}

type Outgoing =
  | { type: 'ping'; id: number; c: number }
  | { type: 'play' | 'pause' | 'seek'; position: number }
  | { type: 'rate'; rate: number }
  | { type: 'status'; status: Status }
  | { type: 'tracks'; tracks: SharedTracks }
  | { type: 'media'; id: number; from: number }
  | { type: 'name'; name: string }
  | { type: 'settings'; control?: 'everyone' | 'host'; waitForAll?: boolean; public?: boolean }
  | { type: 'invite'; userId: number }
  | { type: 'end' }

const localNow = () => performance.timeOrigin + performance.now()

export class SyncClient {
  private snap: SyncSnapshot = { room: null, you: null, connected: false, blocked: false, ended: false, error: null }
  private listeners = new Set<() => void>()
  private ws: WebSocket | null = null
  private closed = true
  private retry = 0
  private retryTimer: ReturnType<typeof setTimeout> | undefined
  private pingTimer: ReturnType<typeof setInterval> | undefined

  /** Server time minus ours, in ms, from the quickest recent round trip. */
  private offset = 0
  private samples: { rtt: number; offset: number }[] = []
  private pingSeq = 0

  private video: HTMLVideoElement | null = null
  private mediaId: number | null = null
  private duration = 0
  private loop: ReturnType<typeof setInterval> | undefined
  private starvingSince: number | null = null
  private lastSeek = 0
  /** Whether we ran out while playing and are still refilling. */
  private ranDry = false
  /** Recent drift readings at the room's own rate, for judging small drift. */
  private drifts: number[] = []
  /** When the nudge under way ends (server time, ms). */
  private nudgeUntil: number | null = null
  /** The clock the readings were taken against; a new one starts them over. */
  private steered = ''
  private startTimer: ReturnType<typeof setTimeout> | undefined
  private sentStatus: { status: Status; at: number } | null = null

  constructor(
    private code: string,
    private guestName: () => string | null,
  ) {}
  subscribe = (fn: () => void) => {
    this.listeners.add(fn)
    return () => void this.listeners.delete(fn)
  }

  getSnapshot = () => this.snap

  private set(patch: Partial<SyncSnapshot>) {
    this.snap = { ...this.snap, ...patch }
    this.listeners.forEach((l) => l())
  }

  connect() {
    if (!this.closed) return
    this.closed = false
    this.open()
  }

  close() {
    this.closed = true
    clearTimeout(this.retryTimer)
    clearInterval(this.pingTimer)
    this.ws?.close()
    this.ws = null
    this.detach()
  }

  private open() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const q = new URLSearchParams()
    const name = this.guestName()
    if (name) q.set('name', name)
    const ws = new WebSocket(`${proto}//${location.host}/api/together/${this.code}/ws?${q}`)
    this.ws = ws
    ws.onopen = () => {
      this.retry = 0
      this.samples = []
      this.set({ connected: true })
      // A quick burst to lock onto the server's clock, then a steady trickle.
      for (let i = 0; i < 6; i++) setTimeout(() => this.ping(), i * 120)
      clearInterval(this.pingTimer)
      this.pingTimer = setInterval(() => this.ping(), 4000)
    }
    ws.onmessage = (m) => this.receive(JSON.parse(m.data as string))
    ws.onclose = () => {
      clearInterval(this.pingTimer)
      if (this.ws === ws) this.ws = null
      this.set({ connected: false })
      if (this.closed || this.snap.ended) return
      // Back off, but not by much: people are waiting.
      const wait = Math.min(5000, 400 * 2 ** this.retry++)
      this.retryTimer = setTimeout(() => this.open(), wait)
    }
  }

  private send(m: Outgoing) {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(m))
  }

  private ping() {
    this.send({ type: 'ping', id: ++this.pingSeq, c: localNow() })
  }

  private receive(m: { type: string; [k: string]: unknown }) {
    switch (m.type) {
      case 'pong': {
        const now = localNow()
        const c = m.c as number
        const rtt = now - c
        // The server read its clock about halfway through the round trip.
        this.samples = [...this.samples.slice(-11), { rtt, offset: (m.s as number) - (c + rtt / 2) }]
        this.offset = this.samples.reduce((best, s) => (s.rtt < best.rtt ? s : best)).offset
        break
      }
      case 'welcome':
        this.sentStatus = null
        this.set({ you: m.you as number })
        break
      case 'state':
        this.set({ room: m as unknown as RoomState })
        this.tick()
        break
      case 'ended':
        this.set({ ended: true })
        this.close()
        break
      case 'error':
        this.set({ error: { message: m.message as string, at: Date.now() } })
        break
    }
  }

  /** The server's clock, as best we know it. */
  now() {
    return localNow() + this.offset
  }

  /** Where the room is right now, in seconds. */
  target() {
    const room = this.snap.room
    if (!room) return 0
    const t = positionAt(room.clock, this.now())
    return this.duration ? Math.min(t, this.duration) : t
  }
  /**
   * Applies a change to our copy of the room right away, so the video
   * responds instantly; the server's answer follows a moment later.
   */
  private predict(clock: Partial<Clock>) {
    const room = this.snap.room
    if (!room) return
    this.set({ room: { ...room, clock: { ...room.clock, at: this.now(), ...clock } } })
    this.tick()
  }

  play(position: number) {
    this.send({ type: 'play', position })
    // The server starts everyone a moment from now; so do we.
    this.predict({ paused: false, running: true, position, at: this.now() + 150 })
  }

  pause(position: number) {
    this.send({ type: 'pause', position })
    this.predict({ paused: true, running: false, position })
  }

  seek(position: number) {
    this.send({ type: 'seek', position })
    this.predict({ running: false, position })
  }

  setRate(rate: number) {
    const room = this.snap.room
    this.send({ type: 'rate', rate })
    if (room) this.predict({ position: positionAt(room.clock, this.now()), rate })
  }

  setTracks(tracks: SharedTracks) {
    this.send({ type: 'tracks', tracks })
  }

  changeMedia(id: number, from: number) {
    this.send({ type: 'media', id, from })
  }

  rename(name: string) {
    this.send({ type: 'name', name })
  }

  settings(s: { control?: 'everyone' | 'host'; waitForAll?: boolean; public?: boolean }) {
    this.send({ type: 'settings', ...s })
  }

  invite(userId: number) {
    this.send({ type: 'invite', userId })
  }

  end() {
    this.send({ type: 'end' })
  }

  /** After a tap, when the browser refused to play on its own. */
  unblock() {
    this.set({ blocked: false })
    this.sentStatus = null
    if (this.video && this.snap.room?.clock.running) void this.video.play().catch(() => {})
    this.tick()
  }
  /** Starts steering `video`, which is playing `mediaId`. */
  attach(video: HTMLVideoElement, mediaId: number, duration: number) {
    this.detach()
    this.video = video
    this.mediaId = mediaId
    this.duration = duration
    this.starvingSince = null
    this.sentStatus = null
    this.ranDry = false
    this.restartSteering()
    this.loop = setInterval(() => this.tick(), 100)
    this.tick()
  }

  detach() {
    clearInterval(this.loop)
    clearTimeout(this.startTimer)
    this.startTimer = undefined
    this.video = null
    this.mediaId = null
  }

  private playOwn(v: HTMLVideoElement) {
    if (this.snap.blocked) return
    v.play().catch((e: DOMException) => {
      if (e.name === 'NotAllowedError') this.set({ blocked: true })
    })
  }

  private tick() {
    const v = this.video
    const room = this.snap.room
    if (!v || !room || room.mediaId !== this.mediaId) return
    // Nothing to steer until the stream has something to show.
    if (v.readyState === 0) return this.report('buffering')
    if (this.snap.blocked) return this.report('blocked')

    const c = room.clock
    const now = this.now()
    const end = this.duration || Infinity
    const target = Math.min(positionAt(c, now), end)
    const starving = v.readyState < 3 || v.seeking
    this.starvingSince = starving ? (this.starvingSince ?? now) : null
    const clock = `${c.running} ${c.position} ${c.at} ${c.rate}`
    if (clock !== this.steered) {
      this.steered = clock
      this.restartSteering()
    }

    if (!c.running || now < c.at) {
      // Hold still where the room is, ready to go.
      if (!v.paused) v.pause()
      if (Math.abs(v.currentTime - target) > 0.04 && !v.seeking) v.currentTime = target
      if (v.playbackRate !== c.rate) v.playbackRate = c.rate
      // Starting shortly: start on the exact moment rather than catching up.
      if (c.running && !this.startTimer) {
        this.startTimer = setTimeout(() => {
          this.startTimer = undefined
          this.tick()
        }, c.at - now)
      }
      const ahead = bufferedAhead(v)
      if (ahead >= REFILL || v.currentTime + ahead >= end - 0.5) this.ranDry = false
      return this.report(starving || this.ranDry ? 'buffering' : 'ready')
    }
    clearTimeout(this.startTimer)
    this.startTimer = undefined

    // The credits are over for everyone.
    if (v.ended || target >= end - 0.05) return this.report('ready')

    const drift = v.currentTime - target
    if (Math.abs(drift) > HARD_DRIFT) {
      // One seek at a time: let the last one land before judging again.
      if (!v.seeking && now - this.lastSeek > 600) {
        this.lastSeek = now
        v.currentTime = target
      }
      this.restartSteering()
      if (v.playbackRate !== c.rate) v.playbackRate = c.rate
    } else if (starving) {
      // Where a seek or a stall leaves currentTime says nothing about drift.
      this.restartSteering()
    } else this.steer(v, c, drift, now)
    if (v.paused) this.playOwn(v)
    const buffering = this.starvingSince !== null && now - this.starvingSince > STARVE_MS
    if (buffering) this.ranDry = true
    this.report(buffering ? 'buffering' : 'ready')
  }

  private restartSteering() {
    this.drifts = []
    this.nudgeUntil = null
  }

  /**
   * Absorbs small drift. It's judged over a second of readings, then closed
   * with one nudge held for just long enough. Bending the rate on every
   * reading chases noise, and Firefox stutters and slips its audio out of
   * step with the picture when the rate keeps changing.
   */
  private steer(v: HTMLVideoElement, c: Clock, drift: number, now: number) {
    if (this.nudgeUntil !== null) {
      if (now < this.nudgeUntil) return
      this.restartSteering()
    }
    if (v.playbackRate !== c.rate) v.playbackRate = c.rate
    this.drifts.push(drift)
    if (this.drifts.length < WINDOW) return
    const average = this.drifts.reduce((a, b) => a + b) / this.drifts.length
    this.drifts.shift()
    if (Math.abs(average) <= DEADBAND) return
    // Ahead: a touch slower. Behind: a touch faster.
    const nudge = Math.max(-MAX_NUDGE, Math.min(MAX_NUDGE, -average / CATCH_UP))
    const r = Math.round(c.rate * (1 + nudge) * 200) / 200
    if (r === c.rate) return
    v.playbackRate = r
    this.drifts = []
    this.nudgeUntil = now + (-average / (r - c.rate)) * 1000
  }

  /** Tells the room how we're doing, when the room thinks otherwise. */
  private report(status: Status) {
    const room = this.snap.room
    const me = room?.members.find((m) => m.id === this.snap.you)
    if (!me) return
    // Someone still arriving isn't waited for, whatever they say.
    const agrees = me.status === status || (status === 'buffering' && (me.status === 'joining' || me.status === 'loading'))
    if (agrees) return
    const sent = this.sentStatus
    if (sent && sent.status === status && Date.now() - sent.at < 500) return
    this.sentStatus = { status, at: Date.now() }
    this.send({ type: 'status', status })
  }
}
