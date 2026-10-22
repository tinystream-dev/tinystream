// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { TOKENS, contrastWarnings, derive, formatColor, isDark, parseColor } from './theme'

const grey = derive(
  { canvas: '#191919', ink: '#ebebea', accent: '#ebebea', danger: '#eb8a7a', ok: '#8fc79a', info: '#7dd3fc', warn: '#fcd34d', highlight: '#f9a8d4', social: '#c4b5fd' },
  { 'ink-2': '#9b9b98', 'ink-3': '#6e6e6b', 'accent-hover': '#ffffff', 'info-deep': '#38bdf8', 'warn-soft': '#fde68a', 'warn-deep': '#fbbf24' },
)

// The server's grey_is_the_old_palette (src/theme.rs): the derivations must match.
test('Grey derives like the server does', () => {
  expect(grey).not.toBeNull()
  expect(TOKENS.map((n) => `${n}: ${grey![n]}`)).toEqual([
    'canvas: #191919',
    'raised: #202020',
    'panel: #262626',
    'float: #2e2e2e',
    'ink: #ebebea',
    'ink-2: #9b9b98',
    'ink-3: #6e6e6b',
    'line: rgb(255 255 255 / 0.075)',
    'line-strong: rgb(255 255 255 / 0.13)',
    'hover: rgb(255 255 255 / 0.055)',
    'press: rgb(255 255 255 / 0.09)',
    'accent: #ebebea',
    'accent-hover: #ffffff',
    'on-accent: #191919',
    'danger: #eb8a7a',
    'ok: #8fc79a',
    'info: #7dd3fc',
    'info-deep: #38bdf8',
    'warn: #fcd34d',
    'warn-soft: #fde68a',
    'warn-deep: #fbbf24',
    'highlight: #f9a8d4',
    'social: #c4b5fd',
    'focus: rgb(255 255 255 / 0.55)',
    'selection: rgb(255 255 255 / 0.18)',
    'scrollbar: rgb(255 255 255 / 0.12)',
    'glow: #ffffff',
    'shade: #000000',
  ])
  expect(isDark(grey!)).toBe(true)
  expect(contrastWarnings(grey!)).toEqual([])
})

test('colours round-trip', () => {
  for (const c of ['#0a0b0c', 'rgb(255 255 255 / 0.055)', 'rgb(0 0 0 / 0)']) expect(formatColor(parseColor(c)!)).toBe(c)
  expect(parseColor('rgb(256 0 0 / 1)')).toBeNull()
})
