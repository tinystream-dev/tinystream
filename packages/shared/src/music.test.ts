// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, test } from 'bun:test'
import { gainOf, fadeAfter } from './music-policy'
import { createMusicPlayer, type Entry, type PlayerHost, type Settings } from './music-player'
import type { AudioEngine, EngineEvents, Item, MusicTrack } from './music'
const settings: Settings = { volume: 1, muted: false, gain: 'auto', crossfade: 5 }
const track = (id: number, album = 1, number = id): MusicTrack =>
  ({
    id,
    title: `Song ${id}`,
    albumId: album,
    number,
    album: 'Album',
    artist: 'Artist',
    gains: { trackGain: 6, albumGain: -6, trackPeak: 0.75, albumPeak: 0.8, pending: false },
    codec: 'mp3',
    file: `/file/${id}`,
    flac: `/flac/${id}`,
    suffix: 'mp3',
    size: 100,
    duration: 100,
    sampleRate: 48000,
  }) as MusicTrack
const entry = (id: number, album = 1, number = id): Entry => ({ uid: String(id), track: track(id, album, number) })
describe('shared music policy', () => {
  test('auto uses album gain in ordered albums and track gain when shuffled', () => {
    const q = [entry(1), entry(2)]
    expect(gainOf(0, q, settings, false)).toBeCloseTo(10 ** (-6 / 20))
    expect(gainOf(0, q, settings, true)).toBeCloseTo(1 / 0.75)
  })
  test('never fades consecutive album tracks, fades across albums', () => {
    expect(fadeAfter(0, [entry(1), entry(2)], settings)).toBe(0)
    expect(fadeAfter(0, [entry(1), entry(2, 2)], settings)).toBe(5)
  })
  test('album gain falls back to track gain with clipping guard', () => {
    const q = [entry(1)]
    q[0].track.gains.albumGain = null
    q[0].track.gains.albumPeak = null
    expect(gainOf(0, q, { ...settings, gain: 'album' }, false)).toBeCloseTo(1 / 0.75)
    expect(gainOf(0, q, { ...settings, gain: 'off' }, false)).toBe(1)
  })
})
class Engine implements AudioEngine {
  rate = 48000
  loaded: Item[] = []
  after: Item[] = []
  here: { key: string; time: number } | null = null
  stopped = false
  callbacks: Partial<EngineEvents> = {}
  on<K extends keyof EngineEvents>(e: K, f: EngineEvents[K]) {
    this.callbacks[e] = f
    return () => {
      delete this.callbacks[e]
    }
  }
  async load(items: Item[], start: number) {
    this.loaded = items
    this.here = { key: items[0].key, time: start }
  }
  async play() {}
  pause() {}
  stop() {
    this.here = null
    this.stopped = true
  }
  upcoming(_key: string, items: Item[]) {
    this.after = items
  }
  gain() {}
  volume() {}
  position() {
    return this.here
  }
}
function setup(overrides: Partial<PlayerHost> = {}) {
  const engine = new Engine(),
    reports: number[] = []
  const host: PlayerHost = {
    timers: false,
    storage: { getItem: () => null, setItem() {}, removeItem() {} },
    engine: () => engine,
    supported: () => true,
    queue: async () => ({ playQueue: null }),
    track: async () => ({ track: null }),
    measure: async (id) => ({ measureLoudness: track(id) }),
    save: async () => {},
    played: async (id) => {
      reports.push(id)
    },
    nowPlaying: async () => {},
    ...overrides,
  }
  return { player: createMusicPlayer(host), engine, reports }
}
test('queue changes preserve the current entry through shuffle, reorder and remove', async () => {
  const { player: p, engine: e } = setup()
  await p.music.play([track(1), track(2), track(3)])
  const uid = p.current()!.uid
  p.music.move(0, 2)
  expect(p.current()!.uid).toBe(uid)
  expect(p.getState().index).toBe(2)
  p.music.shuffle()
  expect(p.current()!.uid).toBe(uid)
  p.music.shuffle(false)
  expect(p.current()!.uid).toBe(uid)
  p.music.playNext([track(4)])
  expect(e.after[0].url).toBe('/file/4')
  p.dispose()
})
test('decoder failure retries as FLAC and replenishes on audible track change', async () => {
  const { player: p, engine: e } = setup()
  await p.music.play([track(1), track(2), track(3), track(4), track(5)])
  e.callbacks.error!(p.current()!.uid, 'bad file')
  await Promise.resolve()
  await Promise.resolve()
  expect(e.loaded[0].url).toBe('/flac/1')
  expect(e.loaded[0].ext).toBe('flac')
  const next = p.getState().queue[1]
  e.callbacks.track!(next.uid)
  expect(p.getState().index).toBe(1)
  expect(e.after.map((i) => i.url)).toEqual(['/file/3', '/file/4', '/file/5'])
  p.dispose()
})
test('restores server queue paused and uses that position when resuming', async () => {
  const { player: p, engine: e } = setup({
    queue: async () => ({ playQueue: { tracks: [track(1), track(2)], current: 1, position: 12, shuffled: false, repeat: 'OFF' } }),
  })
  await p.restore()
  expect(p.getState().playing).toBe(false)
  expect(p.position()?.time).toBe(12)
  await p.music.resume()
  expect(e.here?.time).toBe(12)
  p.dispose()
})
test('server switching cancels loudness-delayed playback', async () => {
  let resolve!: (value: { measureLoudness: MusicTrack }) => void
  const { player: p, engine: e } = setup({
    measure: () =>
      new Promise((r) => {
        resolve = r
      }),
  })
  const t = track(1)
  t.gains.pending = true
  const playing = p.music.play([t])
  p.dispose()
  resolve({ measureLoudness: t })
  await playing
  expect(e.loaded).toHaveLength(0)
})

test('Android enables Opus without changing web fallback', async () => {
  const opus = { ...track(1), codec: 'opus', suffix: 'opus' }
  const web = setup(),
    android = setup({ extraCodecs: ['opus'] })
  await web.player.music.play([opus])
  await android.player.music.play([opus])
  expect(web.engine.loaded[0].url).toBe('/flac/1')
  expect(android.engine.loaded[0].url).toBe('/file/1')
  web.player.dispose()
  android.player.dispose()
})

test('resuming a finished queue reloads instead of playing an exhausted decoder', async () => {
  const { player, engine } = setup()
  await player.music.play([track(1)])
  engine.here!.time = 100
  engine.callbacks.ended!()
  await player.music.resume()
  expect(engine.here!.time).toBe(0)
  expect(player.getState().playing).toBe(true)
  player.dispose()
})

test('a paused client follows another client’s server queue edits', async () => {
  let server: Parameters<NonNullable<PlayerHost['save']>>[0] = { tracks: [], current: 0, position: 0, shuffled: false, repeat: 'OFF' }
  const host: Partial<PlayerHost> = {
    save: async (input) => {
      server = input
    },
    queue: async () => ({ playQueue: { ...server, tracks: server.tracks.map((id) => track(id)) } }),
  }
  const phone = setup(host).player,
    web = setup(host).player
  await phone.music.play([track(1), track(2), track(3)])
  await phone.flush()
  await web.restore()
  expect(web.getState().queue.map((e) => e.track.id)).toEqual([1, 2, 3])
  phone.music.pause()
  phone.music.move(2, 1)
  await phone.flush()
  await web.refreshFromServer()
  expect(web.getState().queue.map((e) => e.track.id)).toEqual([1, 3, 2])
  expect(web.getState().playing).toBe(false)
  phone.dispose()
  web.dispose()
})
