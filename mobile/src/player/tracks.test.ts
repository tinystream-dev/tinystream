// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import type { Playback } from './plan'
import { SKIPPABLE, defaultAudio, defaultSubtitle, skipLabel, trackName } from './tracks'

const sub = (id: string, over: Partial<Playback['media']['subtitles'][number]> = {}) => ({
  id,
  codec: 'ass',
  language: null,
  title: null,
  default: false,
  forced: false,
  supported: true,
  ...over,
})

const pb = (subtitles: Playback['media']['subtitles'], audio: Playback['media']['audio'] = []) =>
  ({ media: { subtitles, audio } }) as unknown as Playback

test('subtitles: the language picked last, then the default, then any but forced', () => {
  const p = pb([sub('s2', { language: 'eng', forced: true }), sub('s3', { language: 'eng' }), sub('s4', { language: 'jpn', default: true })])
  expect(defaultSubtitle(p, false, 'eng')).toBe('s3')
  expect(defaultSubtitle(p, false, 'fre')).toBe('s4')
  expect(defaultSubtitle(pb([sub('s2', { forced: true }), sub('s3')]), false, null)).toBe('s3')
  expect(defaultSubtitle(pb([sub('s2', { forced: true })]), false, null)).toBe('s2')
})

test('no subtitles when they were turned off, or none can play', () => {
  expect(defaultSubtitle(pb([sub('s2')]), true, null)).toBeNull()
  expect(defaultSubtitle(pb([sub('s2', { supported: false })]), false, null)).toBeNull()
})

test('audio: the language picked last, else the default', () => {
  const a = (index: number, language: string, d = false) => ({ index, codec: 'aac', codecString: null, channels: 2, language, title: null, default: d })
  const p = pb([], [a(1, 'jpn', true), a(2, 'eng')])
  expect(defaultAudio(p, 'eng')).toBe(2)
  expect(defaultAudio(p, null)).toBe(1)
  expect(defaultAudio(pb([]), 'eng')).toBeNull()
})

test('tracks are named by language and title', () => {
  expect(trackName({ title: 'Signs', language: 'eng' }, 'Track 1')).toBe('English · Signs')
  expect(trackName({ title: 'English SDH', language: 'eng' }, 'Track 1')).toBe('English SDH')
  expect(trackName({ title: null, language: null }, 'Track 1')).toBe('Track 1')
})

test('chapters worth skipping', () => {
  expect(SKIPPABLE.test('Opening')).toBe(true)
  expect(SKIPPABLE.test('Part A')).toBe(false)
  expect(skipLabel('ED')).toBe('Skip credits')
  expect(skipLabel('Recap')).toBe('Skip recap')
  expect(skipLabel('Next Episode')).toBe('Skip preview')
})
