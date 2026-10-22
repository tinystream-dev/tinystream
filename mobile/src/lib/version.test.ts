// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { minor, mismatched, newer } from './version'

test('only major and minor have to match', () => {
  expect(minor('0.28.3')).toBe('0.28')
  expect(mismatched('0.28.3', '0.28.0')).toBe(false)
  expect(mismatched('0.29.0', '0.28.3')).toBe(true)
  expect(mismatched('1.0.0', '0.28.3')).toBe(true)
})

test('tags compare as versions', () => {
  expect(newer('v0.29.0', '0.28.3')).toBe(true)
  expect(newer('v0.28.10', '0.28.9')).toBe(true)
  expect(newer('v0.28.3', '0.28.3')).toBe(false)
  expect(newer('v0.27.9', '0.28.3')).toBe(false)
  expect(newer('nightly', '0.28.3')).toBe(false)
})
