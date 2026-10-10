// SPDX-License-Identifier: AGPL-3.0-or-later
import { requireNativeModule } from 'expo'
import type { Item } from '@tinystream/shared/music'
export type NativeItem = Item & { headers: Record<string, string> }
export type Metadata = {
  queue: { key: string; title: string; artist: string; album: string; duration: number }[]
  index: number
  key: string
  title: string
  artist: string
  album: string
  artwork: string | null
  headers: Record<string, string>
  duration: number
  starred: boolean
  shuffled: boolean
  repeat: number
}
export const Audio = requireNativeModule<{
  load(items: NativeItem[], start: number): Promise<void>
  play(): Promise<void>
  pause(): void
  stop(): void
  upcoming(after: string, items: NativeItem[]): void
  gain(key: string, gain: number): void
  volume(value: number): void
  metadata(value: Metadata): void
  rate(): number
  addListener(event: string, listener: (event: any) => void): { remove(): void }
}>('TinystreamAudio')
