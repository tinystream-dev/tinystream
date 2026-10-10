// SPDX-License-Identifier: AGPL-3.0-or-later
import type { AudioEngine, EngineEvents, Item } from '@tinystream/shared/music'
import { Audio, type Metadata } from '../../modules/audio'
import { resolve } from '../lib/address'
import type { Connection } from '../session'
import { authHeaders } from '../lib/graphql'

export class Engine implements AudioEngine {
  private listeners: { remove(): void }[] = []
  private here: { key: string; time: number } | null = null
  constructor(private connection: Connection) {
    this.listeners.push(
      Audio.addListener('position', (p) => {
        this.here = p.key ? { key: p.key, time: p.time } : null
      }),
    )
  }
  get rate() {
    return Audio.rate()
  }
  on<K extends keyof EngineEvents>(event: K, fn: EngineEvents[K]): () => void {
    const sub = Audio.addListener(event, (e) => {
      if (event === 'track') (fn as EngineEvents['track'])(e.key)
      if (event === 'ended') (fn as EngineEvents['ended'])()
      if (event === 'error') (fn as EngineEvents['error'])(e.key, e.message)
      if (event === 'starved') (fn as EngineEvents['starved'])(e.waiting)
    })
    this.listeners.push(sub)
    return () => sub.remove()
  }
  private items(items: Item[]) {
    return items.map((i) => ({ ...i, url: resolve(this.connection.origin, i.url), headers: authHeaders(this.connection.token) }))
  }
  async load(items: Item[], start: number) {
    this.here = items[0] ? { key: items[0].key, time: start } : null
    await Audio.load(this.items(items), start)
  }
  play() {
    return Audio.play()
  }
  pause() {
    Audio.pause()
  }
  stop() {
    Audio.stop()
    this.here = null
  }
  upcoming(after: string, items: Item[]) {
    Audio.upcoming(after, this.items(items))
  }
  gain(key: string, gain: number) {
    Audio.gain(key, gain)
  }
  volume(gain: number) {
    Audio.volume(Math.max(0, Math.min(1, gain)) ** 2)
  }
  position() {
    return this.here
  }
  metadata(data: Omit<Metadata, 'headers' | 'artwork'> & { artwork: string | null }) {
    Audio.metadata({ ...data, artwork: data.artwork ? resolve(this.connection.origin, data.artwork) : null, headers: authHeaders(this.connection.token) })
  }
  dispose() {
    this.stop()
    this.listeners.forEach((s) => s.remove())
  }
}
