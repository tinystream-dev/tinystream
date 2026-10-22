// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { bytes, countdown, duration, episodeCode, relative, speed } from './downloads'

test('sizes and speeds', () => {
  expect(bytes(null)).toBe('—')
  expect(bytes(512)).toBe('512 B')
  expect(bytes(1_500_000)).toBe('1.5 MB')
  expect(bytes(250_000_000)).toBe('250 MB')
  expect(speed(0)).toBe('0 KB/s')
  expect(speed(2_000_000)).toBe('2.0 MB/s')
})

test('durations', () => {
  expect(duration(45)).toBe('45 s')
  expect(duration(3 * 3600 + 12 * 60)).toBe('3 h 12 min')
  expect(duration(2 * 86400 + 4 * 3600)).toBe('2 d 4 h')
})

test('times from now', () => {
  const now = 1_000_000_000_000
  expect(relative(now / 1000 + 10, now)).toBe('now')
  expect(relative(now / 1000 - 300, now)).toBe('5 min ago')
  expect(countdown(now / 1000 + 90, now)).toBe('in 1:30')
  expect(countdown(now / 1000 - 1, now)).toBe('airing now')
})

test('episode codes', () => {
  expect(episodeCode(1, 2)).toBe('S01E02')
  expect(episodeCode(0, 3)).toBe('Special 3')
})
