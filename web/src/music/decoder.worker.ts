// SPDX-License-Identifier: AGPL-3.0-or-later
// Decodes the queue into the ring, one track straight into the next, so
// there's never a gap between them. Files are read with synchronous range
// requests, which only workers may make, and which keep the Rust side simple.

import init, { Sequencer } from '#decoder/tinystream_decoder.js'
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
  | { type: 'rewind'; generation: number; frame: number }
  | { type: 'end'; generation: number; frame: number }
  | { type: 'error'; generation: number; key: string; message: string }

declare const self: {
  postMessage(m: FromWorker): void
  onmessage: ((e: MessageEvent<ToWorker>) => void) | null
  tsSizeOf: (url: string) => number
  tsReadRange: (url: string, start: number, end: number) => Uint8Array
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

function sizeOf(url: string): number {
  const n = Number(xhr('HEAD', url).getResponseHeader('content-length'))
  if (!n) throw new Error('the server sent nothing')
  return n
}

self.tsSizeOf = sizeOf
const CHUNK = 4096
let ring: ReturnType<typeof views> | null = null
let sequencer: Sequencer | null = null
let finished = true
const samples = new Float32Array(CHUNK * 2)
const written = () => Number(Atomics.load(ring!.header, WRITTEN))

function events() {
  for (const event of JSON.parse(sequencer!.events())) {
    self.postMessage({ ...event, generation: Number(ring!.header[GENERATION]) })
    if (event.type === 'end') finished = true
  }
}
function pump() {
  if (!ring || !sequencer || finished) return
  const { header, data, frames } = ring
  let guard = 0
  while (frames - (written() - Number(Atomics.load(header, READ))) >= CHUNK && guard++ < 64 && !finished) {
    const n = sequencer.read(samples)
    let w = written()
    for (let i = 0; i < n;) {
      const at = w % frames
      const run = Math.min(n - i, frames - at)
      data.set(samples.subarray(i * 2, (i + run) * 2), at * 2)
      i += run
      w += run
    }
    Atomics.store(header, WRITTEN, BigInt(w))
    events()
    if (n === 0) finished = true
  }
}
self.onmessage = async (e: MessageEvent<ToWorker>) => {
  const m = e.data
  switch (m.type) {
    case 'init':
      await init()
      ring = views(m.sab)
      sequencer = new Sequencer(m.rate)
      setInterval(pump, 25)
      self.postMessage({ type: 'ready' })
      break
    case 'play': {
      const at = written()
      Atomics.store(ring!.header, FLUSH_AT, BigInt(at))
      Atomics.add(ring!.header, GENERATION, 1n)
      finished = false
      sequencer!.play(JSON.stringify(m.items), m.start, at)
      events()
      pump()
      break
    }
    case 'upcoming': {
      const rewind = sequencer!.upcoming(m.after, JSON.stringify(m.items), Number(Atomics.load(ring!.header, READ)))
      if (rewind >= 0) Atomics.store(ring!.header, WRITTEN, BigInt(rewind))
      finished = false
      events()
      pump()
      break
    }
    case 'gain':
      sequencer!.gain(m.key, m.gain)
      break
    case 'stop':
      sequencer!.stop()
      finished = true
      break
  }
}
