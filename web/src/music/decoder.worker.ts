// SPDX-License-Identifier: AGPL-3.0-or-later
// Decodes the queue into the ring, one track straight into the next, so
// there's never a gap between them. Files are read with synchronous range
// requests, which only workers may make, and which keep the Rust side simple.

import init, { Track } from '#wasm/tinystream_decoder.js'
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
  | { type: 'error'; generation: number; key: string; message: string }

declare const self: {
  postMessage(m: FromWorker): void
  onmessage: ((e: MessageEvent<ToWorker>) => void) | null
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

const CHUNK = 4096

let ring: ReturnType<typeof views> | null = null
let rate = 48000
let list: Item[] = []
let index = 0
let current: Track | null = null
/** Frame where each started item began, for undoing what's not been heard yet. */
let started: { key: string; frame: number }[] = []
let fade: { next: Track; item: Item; total: number; done: number } | null = null
/** Seconds into each item where decoding began. */
const offsets = new Map<string, number>()
let finished = false
let a = new Float32Array(CHUNK * 2)
let b = new Float32Array(CHUNK * 2)

const post = (m: FromWorker) => self.postMessage(m)
const generation = () => Number(ring!.header[GENERATION])
const written = () => Number(Atomics.load(ring!.header, WRITTEN))

function open(item: Item, start: number): Track {
  const t = new Track(item.url, item.size || sizeOf(item.url), item.ext, rate)
  t.gain = item.gain
  if (start > 0) t.seek(start)
  return t
}

function free(t: Track | null) {
  try {
    t?.free()
  } catch {
    // Already gone.
  }
}

/** Starts the item at `index`, or the next one that opens; false when there's nothing left. */
function begin(start: number, frame: number): boolean {
  while (index < list.length) {
    const item = list[index]
    try {
      current = open(item, start)
      started.push({ key: item.key, frame })
      offsets.set(item.key, start)
      post({ type: 'mark', generation: generation(), key: item.key, frame, offset: start, duration: current.duration })
      return true
    } catch (e) {
      post({ type: 'error', generation: generation(), key: item.key, message: String((e as Error).message ?? e) })
      index++
      start = 0
    }
  }
  current = null
  return false
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
  if (!ring || finished || !current) return
  const free_ = () => ring!.frames - (written() - Number(Atomics.load(ring!.header, READ)))
  let guard = 0
  while (current && free_() >= CHUNK && guard++ < 64) {
    const item = list[index]
    const next = list[index + 1]
    let n: number
    try {
      if (fade) {
        n = current.read(a)
        const m = fade.next.read(b)
        for (let i = 0; i < Math.max(n, m); i++) {
          const x = Math.min(1, (fade.done + i) / fade.total)
          const out = Math.cos((x * Math.PI) / 2)
          const into = Math.sin((x * Math.PI) / 2)
          const ai = i < n ? 1 : 0
          const bi = i < m ? 1 : 0
          a[i * 2] = a[i * 2] * out * ai + b[i * 2] * into * bi
          a[i * 2 + 1] = a[i * 2 + 1] * out * ai + b[i * 2 + 1] * into * bi
        }
        fade.done += Math.max(n, m)
        write(a, Math.max(n, m))
        if (n < CHUNK) {
          free(current)
          current = fade.next
          index++
          fade = null
        }
        continue
      }
      n = current.read(a)
    } catch (e) {
      post({ type: 'error', generation: generation(), key: item.key, message: String((e as Error).message ?? e) })
      n = 0
    }
    if (n > 0) write(a, n)
    if (n < CHUNK) {
      free(current)
      current = null
      index++
      if (!begin(0, written())) {
        finished = true
        post({ type: 'end', generation: generation(), frame: written() })
        return
      }
      continue
    }
    // Close enough to the end to start mixing in the next one.
    if (next && item.crossfade > 0) {
      const total = Math.round(item.crossfade * rate)
      if (shouldFade(item, total)) {
        try {
          const t = open(next, 0)
          started.push({ key: next.key, frame: written() })
          offsets.set(next.key, 0)
          post({ type: 'mark', generation: generation(), key: next.key, frame: written(), offset: 0, duration: t.duration })
          fade = { next: t, item: next, total, done: 0 }
        } catch (e) {
          post({ type: 'error', generation: generation(), key: next.key, message: String((e as Error).message ?? e) })
          list.splice(index + 1, 1)
        }
      }
    }
  }
}

/** Whether the item's last `total` frames have begun; its length is the decoder's best guess. */
function shouldFade(item: Item, total: number): boolean {
  const mark = started.findLast((s) => s.key === item.key)
  if (!current || !mark || current.duration <= 0) return false
  return current.duration * rate - (written() - mark.frame + offsets.get(item.key)! * rate) <= total
}

/** Throws away what hasn't been played yet and starts over from `items[0]` at `start`. */
function play(items: Item[], start: number) {
  const { header } = ring!
  free(current)
  if (fade) free(fade.next)
  fade = null
  current = null
  list = items
  index = 0
  started = []
  finished = false
  const at = written()
  Atomics.store(header, FLUSH_AT, BigInt(at))
  Atomics.add(header, GENERATION, 1n)
  if (!begin(start, at)) {
    finished = true
    post({ type: 'end', generation: generation(), frame: at })
  }
  pump()
}

/** Changes what follows `after`; whatever of the old order is only buffered, not heard, is redone. */
function upcoming(after: string, items: Item[]) {
  const at = list.findIndex((i) => i.key === after)
  if (at < 0) return
  const old = list
  list = [...list.slice(0, at + 1), ...items]
  const firstChanged = list.findIndex((item, i) => old[i]?.key !== item.key)
  if (firstChanged < 0 || firstChanged > index + (fade ? 1 : 0)) {
    if (finished && index < list.length) {
      finished = false
      if (begin(0, written())) pump()
    }
    return
  }
  // The next track has started decoding already; rewind to where it began, if that's not been played.
  const mark = started.find((s) => s.key === old[firstChanged]?.key && s.frame >= 0)
  const read = Number(Atomics.load(ring!.header, READ))
  if (!mark || mark.frame <= read) return
  if (fade) {
    free(fade.next)
    fade = null
  }
  if (firstChanged === index) {
    free(current)
    current = null
  }
  Atomics.store(ring!.header, WRITTEN, BigInt(mark.frame))
  started = started.filter((s) => s.frame < mark.frame)
  index = firstChanged
  finished = false
  if (!current) {
    if (!begin(0, mark.frame)) {
      finished = true
      post({ type: 'end', generation: generation(), frame: mark.frame })
      return
    }
  }
  pump()
}

self.onmessage = async (e: MessageEvent<ToWorker>) => {
  const m = e.data
  switch (m.type) {
    case 'init':
      await init()
      ring = views(m.sab)
      rate = m.rate
      a = new Float32Array(CHUNK * 2)
      b = new Float32Array(CHUNK * 2)
      setInterval(pump, 25)
      post({ type: 'ready' })
      break
    case 'play':
      play(m.items, m.start)
      break
    case 'upcoming':
      upcoming(m.after, m.items)
      break
    case 'gain':
      for (const i of list) if (i.key === m.key) i.gain = m.gain
      if (current && list[index]?.key === m.key) current.gain = m.gain
      break
    case 'stop':
      free(current)
      current = null
      list = []
      finished = true
      break
  }
}
