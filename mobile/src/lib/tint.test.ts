// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { rgba, tintPending, titleTint } from './tint'

test('a title tints with its backdrop, else its poster', () => {
  expect(titleTint({ backdrop: '/b', backdropTint: '1 2 3', posterTint: '4 5 6' })).toBe('1 2 3')
  expect(titleTint({ poster: '/p', posterTint: '4 5 6' })).toBe('4 5 6')
  expect(titleTint({ backdrop: '/b', backdropTint: '' })).toBeNull()
})

test('a tint is pending while its artwork has none', () => {
  expect(tintPending({ backdrop: '/b' })).toBe(true)
  expect(tintPending({ poster: '/p', posterTint: '1 2 3' })).toBe(false)
  expect(tintPending({})).toBe(false)
})

test('tints become colours React Native reads', () => {
  expect(rgba('12 34 56', 0.5)).toBe('rgba(12, 34, 56, 0.5)')
})
