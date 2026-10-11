// SPDX-License-Identifier: AGPL-3.0-or-later
// The video player: ExoPlayer on tinystream's stream (see Stream.kt), seeking
// with `?start=` where it hasn't buffered, rustyass subtitles over it, plus what
// a full-screen player needs of the window: brightness, volume, system bars
// and orientation.

import { requireNativeModule, requireNativeView } from 'expo'
import type { Ref } from 'react'
import type { NativeSyntheticEvent, ViewProps } from 'react-native'

/** How the stream is made (`plan()` in src/player/plan.ts). */
export type StreamQuery = { video: 'copy' | 'transcode'; height: number; audio: number | null; audioMode: 'copy' | 'aac' }

export type Load = {
  /** Where the video's endpoints live, e.g. `https://host/api/media/12`. */
  url: string
  headers: Record<string, string>
  plan: StreamQuery
  /** Seconds into the file. */
  startAt: number
  duration: number
  paused: boolean
  /** What the system's media controls show. */
  title: string | null
  subtitle: string | null
  /** A picture for them (the episode's still), fetched with `headers`. */
  artwork: string | null
  hasPrevious: boolean
  hasNext: boolean
}

/** A subtitle track: where its ASS file is, and the video's fonts (`/subtitles/{track}` and `/fonts/{index}`). */
export type Subtitles = { url: string; fonts: string[]; headers: Record<string, string> }

export type PlaybackState = 'idle' | 'buffering' | 'ready' | 'ended' | 'error'
export type Status = { state: PlaybackState; playing: boolean; paused: boolean }
/** Where playback is, at least 4 times a second, and the stretch around it that's buffered. */
export type Progress = { position: number; buffered: [number, number] | null; rate: number; playing: boolean }
/** `detail` is what went wrong in the player's own words, for the debug log. */
export type PlaybackError = { message: string; retrying: boolean; detail?: string }

export type VideoViewHandle = {
  load(load: Load): Promise<void>
  play(): Promise<void>
  pause(): Promise<void>
  seek(seconds: number): Promise<void>
  setRate(rate: number): Promise<void>
  /** Silences the video (not the device). */
  setMuted(muted: boolean): Promise<void>
  /** False when the system won't (PiP turned off for the app). */
  enterPip(): Promise<boolean>
  /** Draws `track` over the picture with rustyass, or no subtitles; resolves once they show. */
  selectSubtitles(track: Subtitles | null): Promise<void>
}

type Event<T> = (e: NativeSyntheticEvent<T>) => void

export type VideoViewProps = ViewProps & {
  ref?: Ref<VideoViewHandle>
  /** Crop to fill the view rather than fit the whole picture in. */
  fill?: boolean
  onProgress?: Event<Progress>
  onStatus?: Event<Status>
  onPlaybackError?: Event<PlaybackError>
  onVideoSize?: Event<{ width: number; height: number }>
  onPip?: Event<{ active: boolean }>
  /** Previous or next, from the system's media controls or headphones. */
  onRemote?: Event<{ action: 'previous' | 'next' }>
}

export const VideoView = requireNativeView<VideoViewProps>('TinystreamPlayer')

/** A track as `canDecode` needs it: its RFC 6381 codec string, and its size or channels. */
export type Track = { codecs: string; width?: number | null; height?: number | null; fps?: number | null; channels?: number | null }

const Player = requireNativeModule<{
  canDecode(mime: 'video/mp4' | 'audio/mp4', track: Track): Promise<boolean>
  volume(): { level: number; max: number }
  setVolume(level: number): void
  brightness(): Promise<number>
  setBrightness(level: number | null): Promise<void>
  setImmersive(on: boolean): Promise<void>
  setOrientation(orientation: Orientation): Promise<void>
}>('TinystreamPlayer')

/**
 * Whether the device plays the track as it is: hardware decoders preferred,
 * and video above 1080p only with one.
 */
export const canDecode = (mime: 'video/mp4' | 'audio/mp4', track: Track) => Player.canDecode(mime, track)

/** The media volume, in the system's own steps. */
export const mediaVolume = () => Player.volume()
export const setMediaVolume = (level: number) => Player.setVolume(level)

/** The window's brightness, 0–1 (the system's while the window has none of its own). */
export const brightness = () => Player.brightness()
/** null follows the system again. */
export const setBrightness = (level: number | null) => Player.setBrightness(level)

/** Hides the system bars (a swipe brings them back for a while). */
export const setImmersive = (on: boolean) => Player.setImmersive(on)

/** `landscape` and `portrait` follow the sensor within them; `locked` keeps the current one; `default` is the app's. */
export type Orientation = 'landscape' | 'portrait' | 'locked' | 'default'
export const setOrientation = (orientation: Orientation) => Player.setOrientation(orientation)
