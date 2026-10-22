// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { DEFAULT_LOOK } from './look'
import { variables } from './vars'

test('every token becomes a colour and its channels', () => {
  const v = variables(DEFAULT_LOOK.light, false)
  expect(v['--color-canvas']).toBe('#191919')
  expect(v['--rgb-canvas']).toBe('25 25 25')
  expect(v['--color-hover']).toBe('rgb(255 255 255 / 0.055)')
  expect(v['--rgb-hover']).toBe('255 255 255')
  expect(v['--color-media-ink']).toBe('#ffffff')
})

test('media colours follow the scheme when tinted', () => {
  const v = variables(DEFAULT_LOOK.light, true)
  expect(v['--color-media-ink']).toBe('#ebebea')
  expect(v['--color-media-shade']).toBe('#191919')
})
