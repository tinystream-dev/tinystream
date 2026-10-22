// SPDX-License-Identifier: AGPL-3.0-or-later
// What each component style changes: material and motion only, never layout.
// The values are web's (web/src/styles.css, `:root[data-style=…]`).

import { CORNERS, type Tokens, parseColor } from '@tinystream/shared/theme'

export type StyleName = 'layered' | 'flat' | 'glass'

export type Material = {
  /** Squircle corners: how big, relative to Layered, and how smooth. */
  corners: { scale: number; smoothing: number }
  /** The hairline edge's opacity at the top, a third of the way down, and at the bottom. */
  edge: [number, number, number]
  /** Where things that pop in start from. */
  pop: { y: number; scale: number }
  /** What floats is see-through and blurs what's behind it. */
  glass: boolean
}

export const MATERIALS: Record<StyleName, Material> = {
  layered: { corners: CORNERS.layered, edge: [0.14, 0.07, 0.045], pop: { y: -3, scale: 0.985 }, glass: false },
  flat: { corners: CORNERS.flat, edge: [0.1, 0.1, 0.1], pop: { y: 0, scale: 1 }, glass: false },
  glass: { corners: CORNERS.glass, edge: [0.26, 0.1, 0.16], pop: { y: -6, scale: 0.95 }, glass: true },
}

/** A token at an opacity, e.g. for shadows: `color-mix(in srgb, <token> <amount>, transparent)`. */
export function withAlpha(color: string, amount: number) {
  const c = parseColor(color)
  if (!c) return color
  return `rgba(${c.r}, ${c.g}, ${c.b}, ${Math.round((c.a / 1000) * amount * 1000) / 1000})`
}

/** The soft, deep shadow under things that float above the page (web's `lift`), as a `boxShadow`. */
export function lift(style: StyleName, tokens: Tokens) {
  const shade = (amount: number) => withAlpha(tokens.shade, amount)
  if (style === 'flat') return undefined
  if (style === 'glass') return `0px 18px 48px -10px ${shade(0.45)}`
  return `0px 1px 1px ${shade(0.35)}, 0px 12px 32px ${shade(0.45)}`
}
