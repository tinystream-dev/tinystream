// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterAll, afterEach, expect, mock, test } from 'bun:test'

mock.module('../src/music/worklet.ts?worker&url', () => ({ default: 'worklet.js' }))
const { Engine } = await import('../src/music/engine')
const original = { AudioContext: globalThis.AudioContext, AudioWorkletNode: globalThis.AudioWorkletNode, Worker: globalThis.Worker }
let attempts = 0
let closed = 0
let terminated = 0
let failModule = false
let workerFailure = false

class Context {
  sampleRate = 48000
  destination = {}
  audioWorklet = { addModule: async () => { attempts++; if (failModule) throw new Error('module failed') } }
  async close() { closed++ }
}
class WorkerStub {
  onmessage: ((event: unknown) => void) | null = null
  onerror: ((event: unknown) => void) | null = null
  onmessageerror: ((event: unknown) => void) | null = null
  postMessage() {
    queueMicrotask(() => workerFailure
      ? this.onerror?.({ message: 'decoder failed', preventDefault() {} })
      : this.onmessage?.({ data: { type: 'ready' } }))
  }
  terminate() { terminated++ }
}
globalThis.AudioContext = Context as never
globalThis.AudioWorkletNode = class { port = { postMessage() {} }; connect() {}; disconnect() {} } as never
globalThis.Worker = WorkerStub as never

afterEach(() => { failModule = false; workerFailure = false; attempts = 0; closed = 0; terminated = 0 })

test('a failed worklet setup releases resources and can retry', async () => {
  const engine = new Engine()
  failModule = true
  await expect(engine.start()).rejects.toThrow('module failed')
  expect(closed).toBe(1)
  failModule = false
  await engine.start()
  expect(attempts).toBe(2)
})

test('worker startup errors reject and allow a fresh worker', async () => {
  const engine = new Engine()
  workerFailure = true
  await expect(Promise.race([engine.start(), new Promise((_, reject) => setTimeout(() => reject(new Error('startup stayed pending')), 100))])).rejects.toThrow('decoder failed')
  expect(terminated).toBe(1)
  expect(closed).toBe(1)
  workerFailure = false
  await engine.start()
})

afterAll(() => Object.assign(globalThis, original))
