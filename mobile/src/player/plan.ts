// SPDX-License-Identifier: AGPL-3.0-or-later
// How to play a video: web's plan() (web/src/player/engine.ts), with the
// device's decoders (modules/player's canDecode) answering in place of
// MediaSource.isTypeSupported.

import type { PlaybackFragment, TranscodingFieldsFragment } from '../gql/graphql'

export type Playback = PlaybackFragment & { transcoding: TranscodingFieldsFragment }

export type Quality = 'auto' | 1080 | 720 | 480

export type StreamPlan = {
  mime: string
  video: 'copy' | 'transcode'
  height: number
  audio: number | null
  audioMode: 'copy' | 'aac'
  /** What's happening, in words, for the quality menu. */
  describe: string
}

export const TRANSCODED_VIDEO = 'avc1.640029'
export const TRANSCODED_AUDIO = 'mp4a.40.2'

/** Whether a `video/mp4; codecs="…"` type plays here, as MediaSource.isTypeSupported says on the web. */
export type Supported = (mime: string) => boolean

/** What a codec check needs to know about a track. */
export type Decoder = (mime: 'video/mp4' | 'audio/mp4', track: { codecs: string; width?: number; height?: number; fps?: number; channels?: number }) => Promise<boolean>

/**
 * Asks the device, once, about every track of this video `plan` could copy,
 * and answers its questions from that: a type plays when each of its codecs does.
 */
export async function supportFor(pb: Playback, canDecode: Decoder): Promise<Supported> {
  const v = pb.media.video
  const known = new Map<string, boolean>()
  const checks: Promise<void>[] = []
  const asked = new Set<string>()
  const check = (codecs: string, ask: () => Promise<boolean>) => {
    if (asked.has(codecs)) return
    asked.add(codecs)
    checks.push(ask().then((ok) => void known.set(codecs, ok), () => void known.set(codecs, false)))
  }
  if (v?.codecString) check(v.codecString, () => canDecode('video/mp4', { codecs: v.codecString!, width: v.width, height: v.height, fps: v.fps }))
  check(TRANSCODED_VIDEO, () => canDecode('video/mp4', { codecs: TRANSCODED_VIDEO, width: 1920, height: 1080 }))
  for (const a of pb.media.audio) {
    if (a.codecString) check(a.codecString, () => canDecode('audio/mp4', { codecs: a.codecString!, channels: a.channels }))
  }
  check(TRANSCODED_AUDIO, () => canDecode('audio/mp4', { codecs: TRANSCODED_AUDIO, channels: 2 }))
  await Promise.all(checks)
  return (mime) => {
    const codecs = mime.match(/codecs="([^"]*)"/)?.[1].split(',').map((c) => c.trim()) ?? []
    return codecs.length > 0 && codecs.every((c) => known.get(c) === true)
  }
}

/**
 * Picks the cheapest way to play: as-is if the device can decode it,
 * otherwise re-encode only what it can't.
 */
export function plan(pb: Playback, audioIndex: number | null, maxHeight: Quality, supported: Supported): StreamPlan {
  const v = pb.media.video
  const a = pb.media.audio.find((t) => t.index === audioIndex) ?? pb.media.audio.find((t) => t.default) ?? null

  let video: 'copy' | 'transcode' = 'transcode'
  let vCodec = TRANSCODED_VIDEO
  if (maxHeight === 'auto' && v?.codecString && supported(`video/mp4; codecs="${v.codecString}"`)) {
    video = 'copy'
    vCodec = v.codecString
  }
  const height = maxHeight === 'auto' ? Math.min(v?.height ?? 1080, 1080) : maxHeight

  let audioMode: 'copy' | 'aac' = 'aac'
  let aCodec: string | null = a ? TRANSCODED_AUDIO : null
  if (a?.codecString && supported(`audio/mp4; codecs="${a.codecString}"`)) {
    audioMode = 'copy'
    aCodec = a.codecString
  }
  let mime = `video/mp4; codecs="${aCodec ? `${vCodec}, ${aCodec}` : vCodec}"`
  if (!supported(mime) && audioMode === 'copy') {
    audioMode = 'aac'
    mime = `video/mp4; codecs="${vCodec}, ${TRANSCODED_AUDIO}"`
  }

  const hw = pb.transcoding.vaapi ? 'VA-API' : 'software'
  const describe =
    video === 'copy'
      ? audioMode === 'copy'
        ? 'Direct play'
        : 'Direct video, converting audio'
      : `Converting to ${height}p (${hw})`
  return { mime, video, height, audio: a?.index ?? null, audioMode, describe }
}
