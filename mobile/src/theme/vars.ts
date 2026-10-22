// SPDX-License-Identifier: AGPL-3.0-or-later
// A scheme's tokens as the variables tailwind.config.ts reads: each colour
// as the scheme has it, and its channels for opacity modifiers.

import { TOKENS, type Tokens, parseColor } from '@tinystream/shared/theme'

/** Over artwork and video: neutral, unless the look asks for the scheme there too. */
export const MEDIA = ['media-ink', 'media-shade', 'media-panel', 'media-canvas'] as const
export type MediaName = (typeof MEDIA)[number]

export function mediaColors(tokens: Tokens, tint: boolean): Record<MediaName, string> {
  return tint
    ? { 'media-ink': tokens.ink, 'media-shade': tokens.canvas, 'media-panel': tokens.panel, 'media-canvas': tokens.canvas }
    : { 'media-ink': '#ffffff', 'media-shade': '#000000', 'media-panel': '#262626', 'media-canvas': '#101010' }
}

export function variables(tokens: Tokens, mediaTint: boolean): Record<string, string> {
  const colors: [string, string][] = [...TOKENS.map((n) => [n, tokens[n]] as [string, string]), ...Object.entries(mediaColors(tokens, mediaTint))]
  const out: Record<string, string> = {}
  for (const [name, value] of colors) {
    const c = parseColor(value)
    out[`--color-${name}`] = value
    out[`--rgb-${name}`] = c ? `${c.r} ${c.g} ${c.b}` : '0 0 0'
  }
  return out
}
