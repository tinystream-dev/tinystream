// SPDX-License-Identifier: AGPL-3.0-or-later
// Feeds a <video> from tinystream's live fMP4 stream through Media Source
// Extensions.
//
// The server keeps the file's own timestamps, so a stream started at 10:00
// lands at 10:00 in the buffer. Seeking into already-buffered time is free;
// seeking anywhere else simply starts a new stream there. We only read from
// the network while less than BUFFER_AHEAD seconds are buffered, and that
// backpressure travels all the way back to the server's pipeline.

import type { Playback } from '../lib/api'

const BUFFER_AHEAD = 60
const KEEP_BEHIND = 30
/** Largest single appendBuffer call, well under any browser's quota. */
const APPEND_SIZE = 1 << 20

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

export type StreamPlan = {
  mime: string
  video: 'copy' | 'transcode'
  height: number
  audio: number | null
  audioMode: 'copy' | 'aac'
  /** What's happening, in words, for the quality menu. */
  describe: string
}

const TRANSCODED_VIDEO = 'avc1.640029'
const TRANSCODED_AUDIO = 'mp4a.40.2'

const supported = (mime: string) => typeof MediaSource !== 'undefined' && MediaSource.isTypeSupported(mime)

/**
 * Picks the cheapest way to play: as-is if the browser can decode it,
 * otherwise re-encode only what it can't.
 */
export function plan(pb: Playback, audioIndex: number | null, maxHeight: number | 'auto'): StreamPlan {
  const v = pb.media.video
  const a = pb.media.audio.find((t) => t.index === audioIndex) ?? pb.media.audio.find((t) => t.default) ?? null

  let video: 'copy' | 'transcode' = 'transcode'
  let vCodec = TRANSCODED_VIDEO
  if (maxHeight === 'auto' && v?.codecString && supported(`video/mp4; codecs="${v.codecString}"`)) {
    video = 'copy'
    vCodec = v.codecString
  }
  const height = maxHeight === 'auto' ? Math.min(v?.height ?? 1080, 1080) : maxHeight

  let audioMode: 'copy' | 'aac' = 'aac'
  let aCodec: string | null = a ? TRANSCODED_AUDIO : null
  if (a?.codecString && supported(`audio/mp4; codecs="${a.codecString}"`)) {
    audioMode = 'copy'
    aCodec = a.codecString
  }
  let mime = `video/mp4; codecs="${aCodec ? `${vCodec}, ${aCodec}` : vCodec}"`
  if (!supported(mime) && audioMode === 'copy') {
    audioMode = 'aac'
    mime = `video/mp4; codecs="${vCodec}, ${TRANSCODED_AUDIO}"`
  }

  const hw = pb.transcoding.vaapi ? 'VA-API' : 'software'
  const describe =
    video === 'copy'
      ? audioMode === 'copy'
        ? 'Direct play'
        : 'Direct video, converting audio'
      : `Converting to ${height}p (${hw})`
  return { mime, video, height, audio: a?.index ?? null, audioMode, describe }
}

type Listener = (e: { error?: string; loading?: boolean }) => void

export class StreamEngine {
  private ms = new MediaSource()
  private sb: SourceBuffer | null = null
  private objectUrl: string
  private ctrl: AbortController | null = null
  private generation = 0
  private destroyed = false
  private endReached = false
  /** Where the stream in flight started (null when none is running). */
  private activeFrom: number | null = null
  /** Where the stream in flight's last append ended (null before the first). */
  private front: number | null = null
  /** SourceBuffer operations, one at a time (see `exclusive`). */
  private queue = Promise.resolve()
  private onSeeking = () => this.seeked()

  constructor(
    private video: HTMLVideoElement,
    /** Where this video's endpoints live, e.g. `/api/media/12`. */
    private base: string,
    private duration: number | null,
    private streamPlan: StreamPlan,
    private listener: Listener,
  ) {
    this.objectUrl = URL.createObjectURL(this.ms)
  }

  /** Attaches to the video and starts streaming from `start` seconds. */
  async start(start: number) {
    const opened = new Promise<void>((resolve) => this.ms.addEventListener('sourceopen', () => resolve(), { once: true }))
    this.video.src = this.objectUrl
    await opened
    if (this.destroyed) return
    if (this.duration) this.ms.duration = this.duration
    this.sb = this.ms.addSourceBuffer(this.streamPlan.mime)
    this.video.addEventListener('seeking', this.onSeeking)
    this.video.currentTime = start
    void this.load(start)
  }

  destroy() {
    this.destroyed = true
    this.generation++
    this.ctrl?.abort()
    this.video.removeEventListener('seeking', this.onSeeking)
    this.video.removeAttribute('src')
    this.video.load()
    URL.revokeObjectURL(this.objectUrl)
  }

  private isBuffered(t: number) {
    const b = this.video.buffered
    for (let i = 0; i < b.length; i++) {
      if (b.start(i) <= t + 0.1 && b.end(i) > t + 1) return true
    }
    return false
  }

  private bufferedAhead() {
    const t = this.video.currentTime
    const b = this.video.buffered
    for (let i = 0; i < b.length; i++) {
      if (b.start(i) <= t + 0.5 && b.end(i) >= t) return b.end(i) - t
    }
    return 0
  }

  /**
   * The buffered stretch the stream in flight is writing into: where its last
   * append ended up, with everything before it that's still kept. Before the
   * first append, just where it was asked to start.
   */
  private filling(): [number, number] {
    const at = this.front ?? this.activeFrom!
    const b = this.video.buffered
    for (let i = 0; i < b.length; i++) {
      // The first append may begin a little early (at the keyframe before).
      if (b.start(i) <= at + (this.front === null ? 1 : 0.1) && b.end(i) >= at - 0.1) return [b.start(i), b.end(i)]
    }
    return [at, at]
  }

  /** Follows the stream in flight to the end of what it has just appended. */
  private advanced() {
    if (this.activeFrom === null) return
    const [, end] = this.filling()
    this.front = Math.max(this.front ?? end, end)
  }

  private seeked() {
    const t = this.video.currentTime
    if (this.isBuffered(t)) return
    // The stream in flight is about to reach this point anyway. Anything
    // before where it started it never will, however close.
    if (this.activeFrom !== null) {
      const [from, to] = this.filling()
      if (t >= from && t <= to + 15) return
    }
    void this.load(t)
  }

  private waitIdle() {
    const sb = this.sb!
    if (!sb.updating) return Promise.resolve()
    return new Promise<void>((resolve) => sb.addEventListener('updateend', () => resolve(), { once: true }))
  }

  /**
   * Runs one operation on the SourceBuffer once everything before it is done.
   * A new stream's reset or first append can otherwise land while the old
   * one is still removing, which the browser refuses outright.
   */
  private exclusive(op: (sb: SourceBuffer) => void) {
    const next = this.queue.then(async () => {
      const sb = this.sb
      if (!sb || this.destroyed) return
      await this.waitIdle()
      op(sb)
      await this.waitIdle()
    })
    this.queue = next.catch(() => {})
    return next
  }

  /**
   * Appends a network chunk in small slices, waiting whenever plenty is
   * buffered. Reads can hand back hundreds of megabytes at once (Firefox
   * does), more than a SourceBuffer will ever hold, and dropping any of it
   * would leave the parser mid-box and kill the stream.
   */
  private async append(chunk: Uint8Array, gen: number) {
    let offset = 0
    while (offset < chunk.length) {
      while (this.bufferedAhead() > BUFFER_AHEAD && gen === this.generation) {
        await sleep(500)
        await this.evict()
      }
      if (gen !== this.generation) return
      const slice = chunk.subarray(offset, offset + APPEND_SIZE)
      try {
        await this.exclusive((sb) => gen === this.generation && sb.appendBuffer(slice as BufferSource))
        if (gen !== this.generation) return
        offset += slice.length
        this.advanced()
      } catch (e) {
        if ((e as DOMException).name !== 'QuotaExceededError') throw e
        // Full: make room, and if that isn't enough, let playback catch up.
        await this.evict(true)
        await sleep(500)
      }
    }
  }

  /** Drops what's far behind the playhead (or, under pressure, everything not near it). */
  private async evict(hard = false) {
    const t = this.video.currentTime
    const b = this.video.buffered
    const cuts: [number, number][] = []
    for (let i = 0; i < b.length; i++) {
      const [s, e] = [b.start(i), b.end(i)]
      const cutEnd = Math.min(e, t - (hard ? 5 : KEEP_BEHIND))
      if (cutEnd > s) cuts.push([s, cutEnd])
      if (hard && s > t + BUFFER_AHEAD) cuts.push([s, e])
    }
    // Not being able to make room is no reason to stop.
    for (const [s, e] of cuts) await this.exclusive((sb) => sb.remove(s, e)).catch(() => {})
  }

  private url(start: number) {
    const p = this.streamPlan
    const q = new URLSearchParams({ start: start.toFixed(3), video: p.video, audioMode: p.audioMode })
    if (p.video === 'transcode') q.set('height', String(p.height))
    if (p.audio !== null) q.set('audio', String(p.audio))
    return `${this.base}/stream?${q}`
  }

  /** (Re)starts the stream at `start`, abandoning any stream in flight. */
  private async load(start: number) {
    const gen = ++this.generation
    this.activeFrom = start
    this.front = null
    this.ctrl?.abort()
    const ctrl = new AbortController()
    this.ctrl = ctrl
    this.endReached = false

    try {
      // The previous stream may have stopped mid-fragment; reset the parser.
      await this.exclusive((sb) => gen === this.generation && this.ms.readyState === 'open' && sb.abort())
      if (gen !== this.generation) return
      this.listener({ loading: true })
      const res = await fetch(this.url(start), { signal: ctrl.signal, credentials: 'same-origin' })
      if (!res.ok || !res.body) {
        let message = `the server said ${res.status}`
        try {
          message = (await res.json()).error ?? message
        } catch {}
        throw new Error(message)
      }
      const reader = res.body.getReader()
      // Backpressure lives in append(): while it waits, we don't read.
      while (gen === this.generation) {
        const { done, value } = await reader.read()
        if (done) break
        await this.append(value, gen)
      }
      reader.cancel().catch(() => {})
      if (gen === this.generation) {
        this.activeFrom = null
        this.endReached = true
        await this.exclusive(() => gen === this.generation && this.ms.readyState === 'open' && this.ms.endOfStream())
      }
    } catch (e) {
      if (gen !== this.generation || (e as Error).name === 'AbortError') return
      this.activeFrom = null
      this.listener({ error: (e as Error).message || 'playback failed' })
    }
  }

  /** Whether the stream has run out for good: it reached the file's end and nothing's left to play here. */
  exhausted() {
    return this.endReached && !this.isBuffered(this.video.currentTime)
  }

  /** Called when the video stalls; restarts the stream if ours has ended early. */
  recover() {
    const t = this.video.currentTime
    if (!this.isBuffered(t) && (this.endReached ? !this.duration || t < this.duration - 1 : false)) {
      void this.load(t)
    }
  }
}
