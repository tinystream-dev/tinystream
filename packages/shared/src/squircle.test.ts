// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { squirclePath } from './squircle'

test('degenerate boxes', () => {
  expect(squirclePath(0, 10, 4)).toBe('')
  expect(squirclePath(10, 10, 0)).toBe('M0 0H10V10H0Z')
})

test('corners never overlap, however big the radius', () => {
  const start = Number(/^M([\d.]+) 0/.exec(squirclePath(40, 20, 100, 0.6))![1])
  expect(start).toBeGreaterThanOrEqual(30)
})
