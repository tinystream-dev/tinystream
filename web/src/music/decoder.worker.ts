// SPDX-License-Identifier: AGPL-3.0-or-later
// Decodes the queue into the ring with the sequencer (decoder/core), one
// track straight into the next, so there's never a gap between them. Files
// are read with synchronous range requests, which only workers may make, and
// which keep the Rust side simple.

import init, { Item as Native, Sequencer } from '#wasm/tinystream_decoder.js'
import { FLUSH_AT, GENERATION, READ, WRITTEN, views } from './ring'

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

export type ToWorker =
  | { type: 'init'; sab: SharedArrayBuffer; rate: number }
  | { type: 'play'; items: Item[]; start: number }
  | { type: 'upcoming'; after: string; items: Item[] }
  | { type: 'gain'; key: string; gain: number }
  | { type: 'stop' }

export type FromWorker =
  | { type: 'ready' }
  | { type: 'mark'; generation: number; key: string; frame: number; offset: number; duration: number }
  | { type: 'end'; generation: number; frame: number }
  | { type: 'rewind'; generation: number; frame: number }
  | { type: 'error'; generation: number; key: string; message: string }

declare const self: {
  postMessage(m: FromWorker): void
  onmessage: ((e: MessageEvent<ToWorker>) => void) | null
  tsReadRange: (url: string, start: number, end: number) => Uint8Array
  tsSize: (url: string) => number
}

function xhr(method: string, url: string, range?: string): XMLHttpRequest {
  const x = new XMLHttpRequest()
  x.open(method, url, false)
  if (method === 'GET') x.responseType = 'arraybuffer'
  if (range) x.setRequestHeader('Range', range)
  x.send()
  if (x.status >= 400 || x.status === 0) throw new Error(x.status === 0 ? "can't reach the server" : `the server said ${x.status}`)
  return x
}

self.tsReadRange = (url, start, end) => new Uint8Array(xhr('GET', url, `bytes=${start}-${end}`).response as ArrayBuffer)

self.tsSize = (url) => {
  const n = Number(xhr('HEAD', url).getResponseHeader('content-length'))
  if (!n) throw new Error('the server sent nothing')
  return n
}

const CHUNK = 4096
/** Frames past what's been played that may be on their way out already, so they're never redone. */
const PLAYING = 1024

let ring: ReturnType<typeof views> | null = null
let sequencer: Sequencer | null = null
let out = new Float32Array(CHUNK * 2)

const generation = () => Number(ring!.header[GENERATION])
const written = () => Number(Atomics.load(ring!.header, WRITTEN))
const native = (items: Item[]) => items.map((i) => new Native(i.key, i.url, i.size, i.ext, i.gain, i.crossfade))

/** Tells the engine what the sequencer did. */
function report() {
  for (const e of sequencer!.events()) {
    const g = generation()
    if (e.kind === 'mark') self.postMessage({ type: 'mark', generation: g, key: e.key, frame: e.frame, offset: e.offset, duration: e.duration })
    else if (e.kind === 'end') self.postMessage({ type: 'end', generation: g, frame: e.frame })
    else if (e.kind === 'rewind') self.postMessage({ type: 'rewind', generation: g, frame: e.frame })
    else self.postMessage({ type: 'error', generation: g, key: e.key, message: e.message })
    e.free()
  }
}

function write(src: Float32Array, n: number) {
  const { header, data, frames } = ring!
  let w = written()
  let i = 0
  while (i < n) {
    const at = w % frames
    const run = Math.min(n - i, frames - at)
    data.set(src.subarray(i * 2, (i + run) * 2), at * 2)
    i += run
    w += run
  }
  Atomics.store(header, WRITTEN, BigInt(w))
}

/** Decodes while there's room in the ring. */
function pump() {
  if (!ring || !sequencer) return
  const room = () => ring!.frames - (written() - Number(Atomics.load(ring!.header, READ)))
  let guard = 0
  while (room() >= CHUNK && guard++ < 64) {
    const n = sequencer.read(out)
    if (n > 0) write(out, n)
    report()
    if (n < CHUNK) break
  }
}

self.onmessage = async (e: MessageEvent<ToWorker>) => {
  const m = e.data
  switch (m.type) {
    case 'init':
      await init()
      ring = views(m.sab)
      sequencer = new Sequencer(m.rate)
      out = new Float32Array(CHUNK * 2)
      setInterval(pump, 25)
      self.postMessage({ type: 'ready' })
      break
    case 'play': {
      // Throws away what hasn't been played yet and starts over from here.
      const at = written()
      Atomics.store(ring!.header, FLUSH_AT, BigInt(at))
      Atomics.add(ring!.header, GENERATION, 1n)
      sequencer!.play(native(m.items), m.start, at)
      report()
      pump()
      break
    }
    case 'upcoming': {
      const heard = Number(Atomics.load(ring!.header, READ)) + PLAYING
      const back = sequencer!.upcoming(m.after, native(m.items), heard)
      if (back >= 0) Atomics.store(ring!.header, WRITTEN, BigInt(back))
      report()
      pump()
      break
    }
    case 'gain':
      sequencer!.gain(m.key, m.gain)
      break
    case 'stop':
      sequencer!.stop()
      break
  }
}
