// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { type Decoder, type Playback, plan, supportFor } from './plan'

const pb = (over: Partial<Playback['media']> = {}, vaapi: string | null = null): Playback => ({
  id: 1,
  still: '',
  label: null,
  name: null,
  position: null,
  finished: null,
  title: { id: 1, kind: 'MOVIE', name: 'A', backdrop: null },
  previous: null,
  next: null,
  media: {
    duration: 100,
    video: { index: 0, codec: 'hevc', codecString: 'hvc1.2.4.L150.B0', width: 3840, height: 2160, fps: 24, bitDepth: 10, hdr: true },
    audio: [
      { index: 1, codec: 'eac3', codecString: 'ec-3', channels: 6, language: 'eng', title: null, default: true },
      { index: 2, codec: 'aac', codecString: 'mp4a.40.2', channels: 2, language: 'jpn', title: null, default: false },
    ],
    subtitles: [],
    fonts: [],
    chapters: [],
    ...over,
  },
  transcoding: { vaapi, vaapiError: null, softwareH264: true },
})

/** A device that decodes these codecs (and records what it was asked). */
const device = (codecs: string[], asked: string[] = []): Decoder => async (_mime, track) => {
  asked.push(track.codecs)
  return codecs.includes(track.codecs)
}

test('direct play when the device decodes everything', async () => {
  const p = pb()
  const supported = await supportFor(p, device(['hvc1.2.4.L150.B0', 'ec-3', 'avc1.640029', 'mp4a.40.2']))
  expect(plan(p, null, 'auto', supported)).toEqual({
    mime: 'video/mp4; codecs="hvc1.2.4.L150.B0, ec-3"',
    video: 'copy',
    height: 1080,
    audio: 1,
    audioMode: 'copy',
    describe: 'Direct play',
  })
})

test('audio the device lacks is converted, the video still copied', async () => {
  const p = pb()
  const supported = await supportFor(p, device(['hvc1.2.4.L150.B0', 'avc1.640029', 'mp4a.40.2']))
  const s = plan(p, null, 'auto', supported)
  expect([s.video, s.audioMode, s.describe]).toEqual(['copy', 'aac', 'Direct video, converting audio'])
  expect(s.mime).toBe('video/mp4; codecs="hvc1.2.4.L150.B0, mp4a.40.2"')
})

test('video the device lacks is converted, no higher than 1080p', async () => {
  const p = pb({}, '/dev/dri/renderD128')
  const supported = await supportFor(p, device(['ec-3', 'avc1.640029', 'mp4a.40.2']))
  const s = plan(p, 2, 'auto', supported)
  expect([s.video, s.height, s.audio, s.audioMode, s.describe]).toEqual(['transcode', 1080, 2, 'copy', 'Converting to 1080p (VA-API)'])
})

test('only Automatic copies the video', async () => {
  const p = pb()
  const supported = await supportFor(p, device(['hvc1.2.4.L150.B0', 'ec-3', 'avc1.640029', 'mp4a.40.2']))
  const s = plan(p, null, 720, supported)
  expect([s.video, s.height, s.audioMode, s.describe]).toEqual(['transcode', 720, 'copy', 'Converting to 720p (software)'])
})

test('a video without audio or codec strings is converted', async () => {
  const p = pb({ audio: [], video: { index: 0, codec: 'mpeg4', codecString: null, width: 640, height: 480, fps: 25, bitDepth: 8, hdr: false } })
  const s = plan(p, null, 'auto', await supportFor(p, device(['avc1.640029', 'mp4a.40.2'])))
  expect(s).toMatchObject({ mime: 'video/mp4; codecs="avc1.640029"', video: 'transcode', height: 480, audio: null, audioMode: 'aac' })
})

test('each codec is asked about once, and a failing check counts as no', async () => {
  const p = pb({
    audio: [
      { index: 1, codec: 'aac', codecString: 'mp4a.40.2', channels: 2, language: null, title: null, default: true },
      { index: 2, codec: 'aac', codecString: 'mp4a.40.2', channels: 2, language: null, title: null, default: false },
    ],
  })
  const asked: string[] = []
  const supported = await supportFor(p, async (mime, track) => {
    asked.push(track.codecs)
    if (mime === 'video/mp4' && track.codecs !== 'avc1.640029') throw new Error('no decoders')
    return true
  })
  expect(asked.sort()).toEqual(['avc1.640029', 'hvc1.2.4.L150.B0', 'mp4a.40.2'])
  expect(plan(p, null, 'auto', supported).video).toBe('transcode')
})
