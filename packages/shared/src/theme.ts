// SPDX-License-Identifier: AGPL-3.0-or-later
// Colour schemes and component styles. The server (src/theme.rs) is the real
// thing: it derives every scheme and makes its code. The copy of the
// derivation here only previews a scheme while it's being edited, and must
// match it exactly.

import type { ComponentStyle, SchemeMode } from './schema'

export type { ComponentStyle, SchemeMode }

export const TOKENS = [
  'canvas',
  'raised',
  'panel',
  'float',
  'ink',
  'ink-2',
  'ink-3',
  'line',
  'line-strong',
  'hover',
  'press',
  'accent',
  'accent-hover',
  'on-accent',
  'danger',
  'ok',
  'info',
  'info-deep',
  'warn',
  'warn-soft',
  'warn-deep',
  'highlight',
  'social',
  'focus',
  'selection',
  'scrollbar',
  'glow',
  'shade',
] as const
export type TokenName = (typeof TOKENS)[number]

export const SEEDS = ['canvas', 'ink', 'accent', 'danger', 'ok', 'info', 'warn', 'highlight', 'social'] as const
export type SeedName = (typeof SEEDS)[number]

export type Tokens = Record<TokenName, string>
export type NamedColor = { name: string; value: string }

/** sRGB, alpha in thousandths. */
type Color = { r: number; g: number; b: number; a: number }

const WHITE: Color = { r: 255, g: 255, b: 255, a: 1000 }
const BLACK: Color = { r: 0, g: 0, b: 0, a: 1000 }

/** `#rrggbb` or `rgb(r g b / a)`, like the server takes them. */
export function parseColor(s: string): Color | null {
  s = s.trim()
  const hex = /^#([0-9a-f]{6})$/i.exec(s)
  if (hex) {
    const v = parseInt(hex[1], 16)
    return { r: v >> 16, g: (v >> 8) & 255, b: v & 255, a: 1000 }
  }
  const m = /^rgb\(\s*(\d{1,3})\s+(\d{1,3})\s+(\d{1,3})\s*\/\s*(\d+)(?:\.(\d{1,3}))?\s*\)$/.exec(s)
  if (!m) return null
  const [r, g, b] = [m[1], m[2], m[3]].map(Number)
  const a = Number(m[4]) * 1000 + Number((m[5] ?? '').padEnd(3, '0'))
  if (r > 255 || g > 255 || b > 255 || a > 1000) return null
  return { r, g, b, a }
}

export function formatColor({ r, g, b, a }: Color): string {
  if (a === 1000) return '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')
  const alpha = String(a).padStart(3, '0').replace(/0+$/, '')
  return `rgb(${r} ${g} ${b} / ${alpha ? `0.${alpha}` : '0'})`
}

const luma = (c: Color) => 2126 * c.r + 7152 * c.g + 722 * c.b
const mix = (c: Color, to: Color, t: number): Color => {
  const ch = (a: number, b: number) => Math.floor((a * (1000 - t) + b * t + 500) / 1000)
  return { r: ch(c.r, to.r), g: ch(c.g, to.g), b: ch(c.b, to.b), a: c.a }
}
const alpha = (c: Color, a: number): Color => ({ ...c, a })

/** Every token from seeds and overrides: `ts1`'s derivation, step for step. */
export function derive(seeds: Record<SeedName, string>, overrides: Partial<Record<TokenName, string>>): Tokens | null {
  const s = SEEDS.map((n) => parseColor(seeds[n]))
  if (s.some((c) => !c || c.a !== 1000)) return null
  const [canvas, ink, accent, danger, ok, info, warn, highlight, social] = s as Color[]
  const dark = luma(canvas) < luma(ink)
  const glow = dark ? WHITE : BLACK
  const [raised, panel, float] = (dark ? [30, 57, 91] : [350, 650, 1000]).map((t) => mix(canvas, WHITE, t))
  const onAccent = Math.abs(luma(accent) - luma(canvas)) >= Math.abs(luma(accent) - luma(ink)) ? canvas : ink
  const t = [
    canvas,
    raised,
    panel,
    float,
    ink,
    mix(ink, canvas, 381),
    mix(ink, canvas, 600),
    alpha(glow, 75),
    alpha(glow, 130),
    alpha(glow, 55),
    alpha(glow, 90),
    accent,
    mix(accent, glow, 250),
    onAccent,
    danger,
    ok,
    info,
    mix(info, canvas, 200),
    warn,
    mix(warn, ink, 400),
    mix(warn, canvas, 200),
    highlight,
    social,
    alpha(glow, 550),
    alpha(glow, 180),
    alpha(glow, 120),
    glow,
    BLACK,
  ]
  const out = {} as Tokens
  TOKENS.forEach((name, i) => {
    const pinned = overrides[name] && parseColor(overrides[name])
    out[name] = formatColor(pinned || t[i])
  })
  return out
}

export const toRecord = (list: readonly NamedColor[]) => Object.fromEntries(list.map((t) => [t.name, t.value]))

export type Warning = { foreground: string; background: string; ratio: number; minimum: number }

/** Like the server's: text needs 4.5:1, quieter text and accents 3:1. */
const PAIRS: [TokenName, TokenName, number][] = [
  ['ink', 'canvas', 4.5],
  ['ink', 'float', 4.5],
  ['ink-2', 'canvas', 4.5],
  ['ink-3', 'canvas', 3],
  ['on-accent', 'accent', 4.5],
  ['danger', 'canvas', 3],
  ['ok', 'canvas', 3],
  ['info', 'canvas', 3],
  ['warn', 'canvas', 3],
  ['highlight', 'canvas', 3],
  ['social', 'canvas', 3],
]

const over = (top: Color, bottom: Color): Color => {
  const ch = (t: number, b: number) => Math.floor((t * top.a + b * (1000 - top.a) + 500) / 1000)
  return { r: ch(top.r, bottom.r), g: ch(top.g, bottom.g), b: ch(top.b, bottom.b), a: 1000 }
}
const luminance = (c: Color) => {
  const lin = (v: number) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)
  return 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

export function contrastWarnings(tokens: Tokens): Warning[] {
  const get = (n: TokenName) => parseColor(tokens[n]) ?? BLACK
  return PAIRS.flatMap(([fg, bg, minimum]) => {
    const back = over(get(bg), get('canvas'))
    const [a, b] = [luminance(over(get(fg), back)), luminance(back)]
    const ratio = Math.round(((Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)) * 100) / 100
    return ratio < minimum ? [{ foreground: fg, background: bg, ratio, minimum }] : []
  })
}

export const isDark = (tokens: Tokens) => {
  const [canvas, ink] = [parseColor(tokens.canvas), parseColor(tokens.ink)]
  return !!canvas && !!ink && luma(canvas) < luma(ink)
}

/** Squircle corners for each style: how big, relative to Layered, and how smooth. */
export const CORNERS: Record<string, { scale: number; smoothing: number }> = {
  layered: { scale: 1, smoothing: 0.6 },
  flat: { scale: 0.8, smoothing: 0 },
  glass: { scale: 1.25, smoothing: 0.8 },
}
