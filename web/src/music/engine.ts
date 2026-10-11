// SPDX-License-Identifier: AGPL-3.0-or-later
// The music player's sound: a worker decodes (Rust, see web/decoder) into a
// shared ring, and an audio worklet plays it. Where we are is worked out
// from how many frames the worklet has played and where each track began.

import type { FromWorker, Item, ToWorker } from './decoder.worker'
import { makeRing } from './ring'
import type { FromWorklet, ToWorklet } from './worklet'
import workletUrl from './worklet.ts?worker&url'

export type { Item }

type Mark = { key: string; frame: number; offset: number; duration: number }

export type EngineEvents = {
  /** A track began playing (it's audible now, not just decoded). */
  track: (key: string) => void
  /** Everything queued has been played. */
  ended: () => void
  error: (key: string, message: string) => void
  starved: (starved: boolean) => void
}

export class Engine {
  private ctx: AudioContext | null = null
  private node: AudioWorkletNode | null = null
  private worker: Worker | null = null
  private starting: Promise<void> | null = null
  private generation = 0
  private marks: Mark[] = []
  private end: number | null = null
  private read = 0
  private readAt = 0
  private current: string | null = null
  private playing = false
  private listeners = new Map<keyof EngineEvents, Set<(...args: never[]) => void>>()

  on<K extends keyof EngineEvents>(event: K, fn: EngineEvents[K]): () => void {
    const set = this.listeners.get(event) ?? new Set()
    this.listeners.set(event, set)
    set.add(fn)
    return () => void set.delete(fn)
  }

  private emit<K extends keyof EngineEvents>(event: K, ...args: Parameters<EngineEvents[K]>) {
    for (const fn of this.listeners.get(event) ?? []) (fn as (...a: Parameters<EngineEvents[K]>) => void)(...args)
  }

  /** Whether this browser can run the player at all: it needs shared memory. */
  static supported(): boolean {
    return typeof SharedArrayBuffer !== 'undefined' && typeof AudioWorkletNode !== 'undefined' && crossOriginIsolated
  }

  get rate(): number {
    return this.ctx?.sampleRate ?? 48000
  }

  /**
   * Sets up sound at `rate`, the music's own: then an album plays through
   * untouched, and the browser converts to the speakers' rate just once, all
   * the way through. Has to follow a click or a key the first time.
   */
  start(rate?: number): Promise<void> {
    const wanted = rate ? Math.max(8000, Math.min(192000, rate)) : undefined
    if (this.starting && wanted && this.ctx && this.ctx.sampleRate !== wanted) {
      this.ctx.close().catch(() => {})
      this.worker?.terminate()
      this.ctx = null
      this.node = null
      this.worker = null
      this.starting = null
    }
    this.starting ??= (async () => {
      let ctx: AudioContext
      try {
        ctx = new AudioContext({ latencyHint: 'playback', sampleRate: wanted })
      } catch {
        ctx = new AudioContext({ latencyHint: 'playback' })
      }
      await ctx.audioWorklet.addModule(workletUrl)
      const sab = makeRing(ctx.sampleRate)
      const node = new AudioWorkletNode(ctx, 'tinystream-music', { numberOfInputs: 0, outputChannelCount: [2] })
      node.connect(ctx.destination)
      node.port.onmessage = (e: MessageEvent<FromWorklet>) => this.played(e.data)
      node.port.postMessage({ sab } satisfies ToWorklet)
      const worker = new Worker(new URL('./decoder.worker.ts', import.meta.url), { type: 'module' })
      const ready = new Promise<void>((resolve) => {
        worker.onmessage = (e: MessageEvent<FromWorker>) => {
          if (e.data.type === 'ready') resolve()
          else this.heard(e.data)
        }
      })
      worker.postMessage({ type: 'init', sab, rate: ctx.sampleRate } satisfies ToWorker)
      await ready
      this.ctx = ctx
      this.node = node
      this.worker = worker
    })()
    return this.starting
  }

  private send(m: ToWorker) {
    this.worker?.postMessage(m)
  }

  private heard(m: FromWorker) {
    if (m.type === 'ready') return
    if (m.generation !== this.generation) return
    if (m.type === 'mark') {
      this.marks.push({ key: m.key, frame: m.frame, offset: m.offset, duration: m.duration })
      this.marks.sort((a, b) => a.frame - b.frame)
    } else if (m.type === 'end') {
      this.end = m.frame
    } else if (m.type === 'error') {
      this.emit('error', m.key, m.message)
    }
  }

  private played({ read, starved }: FromWorklet) {
    this.read = read
    this.readAt = performance.now()
    this.emit('starved', starved)
    const mark = this.markAt(read)
    if (mark && mark.key !== this.current) {
      this.current = mark.key
      this.emit('track', mark.key)
    }
    // Marks long behind are of no more use.
    const live = this.marks.findIndex((m) => m === mark)
    if (live > 4) this.marks.splice(0, live - 1)
    if (this.end !== null && read >= this.end && this.playing) {
      this.end = null
      this.emit('ended')
    }
  }

  private markAt(frame: number): Mark | null {
    let found: Mark | null = null
    for (const m of this.marks) {
      if (m.frame <= frame) found = m
      else break
    }
    return found
  }

  /** The track you hear now, and how far into it, counting the time sound takes to reach the speakers. */
  position(): { key: string; time: number } | null {
    const ctx = this.ctx
    if (!ctx) return null
    const elapsed = this.playing ? ((performance.now() - this.readAt) / 1000) * ctx.sampleRate : 0
    const latency = ((ctx.outputLatency || 0) + (ctx.baseLatency || 0)) * ctx.sampleRate
    const frame = Math.max(0, this.read + Math.min(elapsed, 2048) - (this.playing ? latency : 0))
    const mark = this.markAt(frame)
    if (!mark) return null
    return { key: mark.key, time: Math.min(mark.offset + (frame - mark.frame) / ctx.sampleRate, mark.duration || Infinity) }
  }

  /** Plays `items` in order, from `start` seconds into the first, throwing away whatever was lined up. */
  async load(items: Item[], start = 0, rate?: number) {
    const was = this.playing
    await this.start(rate)
    if (was && this.ctx?.state === 'suspended') await this.ctx.resume()
    this.generation++
    this.marks = []
    this.end = null
    this.current = null
    this.send({ type: 'play', items, start })
  }

  /** What comes after `after` changed. */
  upcoming(after: string, items: Item[]) {
    this.end = null
    this.send({ type: 'upcoming', after, items })
  }

  gain(key: string, gain: number) {
    this.send({ type: 'gain', key, gain })
  }

  async play() {
    await this.start()
    await this.ctx!.resume()
    this.playing = true
    this.node!.port.postMessage({ playing: true } satisfies ToWorklet)
  }

  pause() {
    this.playing = false
    this.node?.port.postMessage({ playing: false } satisfies ToWorklet)
  }

  volume(v: number) {
    this.node?.port.postMessage({ volume: Math.max(0, Math.min(1, v)) ** 2 } satisfies ToWorklet)
  }

  stop() {
    this.pause()
    this.marks = []
    this.current = null
    this.send({ type: 'stop' })
  }
}
