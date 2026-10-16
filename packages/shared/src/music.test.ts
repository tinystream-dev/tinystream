// SPDX-License-Identifier: AGPL-3.0-or-later

import { afterEach, expect, test } from 'bun:test'
import { type AudioEngine, DEFAULT_SETTINGS, type EngineEvents, type Item, type MusicTrack, type QueueInput, createPlayer, fadeAfter, gainOf } from './music'

const track = (id: number, more: Partial<MusicTrack> = {}): MusicTrack => ({
  id,
  title: `Track ${id}`,
  artist: 'Artist',
  album: 'Album',
  albumId: 1,
  number: id,
  duration: 200,
  codec: 'flac',
  suffix: 'flac',
  size: 1000,
  file: `/file/${id}`,
  flac: `/flac/${id}`,
  sampleRate: 44100,
  cover: null,
  starred: false,
  gains: { trackGain: -6, trackPeak: 0.5, albumGain: -3, albumPeak: 0.9, pending: false },
  ...more,
})

const entries = (...tracks: MusicTrack[]) => tracks.map((t, i) => ({ uid: `u${i}`, track: t }))

test('album gain while an album plays in order', () => {
  const q = entries(track(1), track(2), track(3, { albumId: 9 }))
  expect(gainOf(0, q, DEFAULT_SETTINGS, false)).toBeCloseTo(10 ** (-3 / 20))
  expect(gainOf(2, q, DEFAULT_SETTINGS, false)).toBeCloseTo(10 ** (-6 / 20))
  expect(gainOf(0, q, DEFAULT_SETTINGS, true)).toBeCloseTo(10 ** (-6 / 20))
  expect(gainOf(0, q, { ...DEFAULT_SETTINGS, gain: 'off' }, false)).toBe(1)
})

test('gain never clips', () => {
  const q = entries(track(1, { gains: { trackGain: 6, trackPeak: 0.9, albumGain: null, albumPeak: null, pending: false } }))
  expect(gainOf(0, q, { ...DEFAULT_SETTINGS, gain: 'track' }, false)).toBeCloseTo(1 / 0.9)
})

test('no crossfade from one track of an album into the next', () => {
  const settings = { ...DEFAULT_SETTINGS, crossfade: 5 }
  const q = entries(track(1), track(2), track(7, { albumId: 2 }), track(4))
  expect(fadeAfter(0, q, settings)).toBe(0)
  expect(fadeAfter(1, q, settings)).toBe(5)
  expect(fadeAfter(2, q, settings)).toBe(5)
  expect(fadeAfter(3, q, settings)).toBe(0)
  expect(fadeAfter(1, q, DEFAULT_SETTINGS)).toBe(0)
})

class FakeEngine implements AudioEngine {
  rate = 48000
  loaded: { items: Item[]; start: number }[] = []
  lined: { after: string; items: Item[] }[] = []
  playing = false
  here: { key: string; time: number } | null = null
  listeners = new Map<string, ((...args: never[]) => void)[]>()
  on<K extends keyof EngineEvents>(event: K, fn: EngineEvents[K]) {
    this.listeners.set(event, [...(this.listeners.get(event) ?? []), fn])
    return () => {}
  }
  emit<K extends keyof EngineEvents>(event: K, ...args: Parameters<EngineEvents[K]>) {
    for (const fn of this.listeners.get(event) ?? []) (fn as (...a: Parameters<EngineEvents[K]>) => void)(...args)
  }
  position() {
    return this.here
  }
  async load(items: Item[], start = 0) {
    this.loaded.push({ items, start })
    this.here = { key: items[0].key, time: start }
  }
  upcoming(after: string, items: Item[]) {
    this.lined.push({ after, items })
  }
  gain() {}
  async play() {
    this.playing = true
  }
  pause() {
    this.playing = false
  }
  volume() {}
  stop() {
    this.playing = false
    this.here = null
  }
}

let players: { dispose(): void }[] = []
afterEach(() => {
  players.forEach((p) => p.dispose())
  players = []
})

function setup(queue: MusicTrack[] = []) {
  const engine = new FakeEngine()
  const store = new Map<string, string>()
  const saved: QueueInput[] = []
  const player = createPlayer<MusicTrack>({
    engine: () => engine,
    storage: { get: (k) => store.get(k) ?? null, set: (k, v) => (v == null ? store.delete(k) : store.set(k, v)) },
    queue: async () => ({ tracks: queue, current: 1, position: 42, shuffled: false, repeat: 'OFF' }),
    track: async (id) => track(id),
    measure: async (id) => track(id),
    save: async (input) => saved.push(input),
    played: async () => {},
    nowPlaying: async () => {},
  })
  players.push(player)
  return { engine, player, saved, store }
}

test('plays from the track asked for, with what comes next lined up', async () => {
  const { engine, player } = setup()
  await player.music.play([track(1), track(2, { codec: 'wavpack', suffix: 'wv' }), track(3)], 0)
  const [{ items }] = engine.loaded
  expect(items.map((i) => i.url)).toEqual(['/file/1', '/flac/2', '/file/3'])
  expect(items[1]).toMatchObject({ ext: 'flac', size: 0 })
  expect(player.getState().playing).toBe(true)
  expect(engine.playing).toBe(true)
})

test('follows the engine to the next track', async () => {
  const { engine, player } = setup()
  await player.music.play([track(1), track(2)], 0)
  engine.emit('track', engine.loaded[0].items[1].key)
  expect(player.getState().index).toBe(1)
})

test('falls back to FLAC when the decoder can’t read a file', async () => {
  const { engine, player } = setup()
  await player.music.play([track(1, { codec: 'mp3', suffix: 'mp3' })], 0)
  engine.emit('error', engine.loaded[0].items[0].key, 'bad')
  await Bun.sleep(0)
  expect(engine.loaded[1].items[0]).toMatchObject({ url: '/flac/1', ext: 'flac' })
  engine.emit('error', engine.loaded[1].items[0].key, 'bad')
  expect(player.getState().error).toContain('Track 1')
})

test('lines up again when the queue changes', async () => {
  const { engine, player } = setup()
  await player.music.play([track(1), track(2)], 0)
  player.music.playNext([track(5)])
  const last = engine.lined.at(-1)!
  expect(last.items.map((i) => i.url)).toEqual(['/file/5', '/file/2'])
})

test('picks up the queue where it was left, paused', async () => {
  const { engine, player } = setup([track(1), track(2)])
  await player.restore()
  const s = player.getState()
  expect([s.index, s.resumeAt, s.playing]).toEqual([1, 42, false])
  await player.music.resume()
  expect(engine.loaded[0].start).toBe(42)
  expect(engine.loaded[0].items[0].url).toBe('/file/2')
})

test('remembers settings', () => {
  const { player, store } = setup()
  player.music.settings({ crossfade: 4 })
  expect(JSON.parse(store.get('tinystream.music')!).crossfade).toBe(4)
})
