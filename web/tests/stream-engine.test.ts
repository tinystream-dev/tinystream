// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterAll, afterEach, expect, test } from 'bun:test'
import { StreamEngine } from '../src/player/engine'

const original = { MediaSource: globalThis.MediaSource, fetch: globalThis.fetch }
const originalCreate = URL.createObjectURL
const originalRevoke = URL.revokeObjectURL
let source: MediaSourceStub
let failSetup = false
let bufferEnd = 0
let ended = false
class MediaSourceStub extends EventTarget {
  readyState = 'open'
  duration = 0
  constructor() { super(); source = this }
  addSourceBuffer() {
    if (failSetup) throw new Error('unsupported codec')
    return { updating: false, abort() {}, appendBuffer() { bufferEnd = 1 } }
  }
  endOfStream() { ended = true }
}
globalThis.MediaSource = MediaSourceStub as never
URL.createObjectURL = () => 'blob:test'
URL.revokeObjectURL = () => {}
const video = () => Object.assign(new EventTarget(), {
  src: '', currentTime: 0, paused: false,
  buffered: { get length() { return bufferEnd ? 1 : 0 }, start: () => 0, end: () => bufferEnd },
  removeAttribute() {}, load() {},
}) as unknown as HTMLVideoElement
const plan = { mime: 'invalid', video: 'copy', height: 720, audio: null, audioMode: 'copy', describe: '' } as const
afterEach(() => { failSetup = false })
const settle = () => new Promise(resolve => setTimeout(resolve, 10))

test('setup failures notify the player', async () => {
  failSetup = true
  const errors: string[] = []
  const engine = new StreamEngine(video(), '/media/1', 1800, plan, event => { if (event.error) errors.push(event.error) })
  const started = engine.start(0)
  source.dispatchEvent(new Event('sourceopen'))
  await started.catch(() => {})
  expect(errors).toEqual(['unsupported codec'])
  engine.destroy()
  failSetup = false
})

test('early clean EOF reports an error instead of finishing the episode', async () => {
  bufferEnd = 0
  ended = false
  globalThis.fetch = (async () => new Response(new Uint8Array([1]))) as never
  const errors: string[] = []
  const engine = new StreamEngine(video(), '/media/1', 1800, plan, event => { if (event.error) errors.push(event.error) })
  const started = engine.start(0)
  source.dispatchEvent(new Event('sourceopen'))
  await started
  await settle()
  expect(ended).toBe(false)
  expect(engine.exhausted()).toBe(false)
  expect(errors[0]).toContain('before')
  engine.destroy()
})

test('an open stream that stops sending data retries and then reports a bounded failure', async () => {
  const timeout = globalThis.setTimeout
  globalThis.setTimeout = ((callback: () => void, delay: number) => timeout(callback, delay >= 1000 ? 1 : delay)) as never
  let requests = 0
  let cancelled = 0
  bufferEnd = 0
  globalThis.fetch = (async (_url: unknown, init: RequestInit) => {
    requests++
    return new Response(new ReadableStream({
      start(controller) { init.signal!.addEventListener('abort', () => { cancelled++; controller.error(new DOMException('cancelled', 'AbortError')) }) },
    }))
  }) as never
  const errors: string[] = []
  const engine = new StreamEngine(video(), '/media/1', 1800, plan, event => { if (event.error) errors.push(event.error) })
  try {
    const started = engine.start(0)
    source.dispatchEvent(new Event('sourceopen'))
    await started
    for (let i = 0; i < 100 && !errors.length; i++) await new Promise(resolve => timeout(resolve, 2))
    expect(requests).toBe(4)
    expect(cancelled).toBe(4)
    expect(errors[0]).toContain('stopped sending data')
    expect(engine.exhausted()).toBe(false)
  } finally {
    engine.destroy()
    globalThis.setTimeout = timeout
  }
})

afterAll(() => { Object.assign(globalThis, original); URL.createObjectURL = originalCreate; URL.revokeObjectURL = originalRevoke })
