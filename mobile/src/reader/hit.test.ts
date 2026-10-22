// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { type Line, offsetAt, pointAt } from './hit'

// Two lines of 10 points per character (letters all as wide, to keep the sums easy).
const lines: Line[] = [
  { x: 0, y: 0, width: 50, height: 20, text: 'aaaaa' },
  { x: 0, y: 20, width: 30, height: 20, text: 'bbb' },
]

test('finds the character under a point', () => {
  expect(offsetAt(lines, 12, 5)).toBe(1)
  expect(offsetAt(lines, 16, 5)).toBe(2)
  expect(offsetAt(lines, 0, 25)).toBe(5)
  expect(offsetAt(lines, 22, 25)).toBe(7)
  // Past a line's end, or under the last one.
  expect(offsetAt(lines, 200, 25)).toBe(8)
  expect(offsetAt(lines, 2, 400)).toBe(5)
})

test('places a character', () => {
  expect(pointAt(lines, 2)).toEqual({ x: 20, y: 0, height: 20 })
  expect(pointAt(lines, 5)).toEqual({ x: 0, y: 20, height: 20 })
  expect(pointAt(lines, 5, true)).toEqual({ x: 50, y: 0, height: 20 })
  expect(pointAt(lines, 8, true)).toEqual({ x: 30, y: 20, height: 20 })
  expect(pointAt([], 0)).toBeNull()
})
