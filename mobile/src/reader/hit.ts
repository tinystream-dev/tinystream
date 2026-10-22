// SPDX-License-Identifier: AGPL-3.0-or-later
// Where characters are in a paragraph as the phone laid it out. React Native
// says where each line is (onTextLayout) but not each character, so within a
// line they're placed by how wide letters roughly are: close enough to find
// the word under a finger, which is what a selection snaps to anyway.

/** A line of a paragraph, in the paragraph's own box (onTextLayout's). */
export type Line = { x: number; y: number; width: number; height: number; text: string }

const NARROW = new Set("il.,:;|!'`ı")
const SLIM = new Set('fjrtI ()[]{}"-/\\')
const WIDE = new Set('mwMW@%')

/** About how wide a character is, in ems. */
export function advance(c: string): number {
  if (NARROW.has(c)) return 0.25
  if (SLIM.has(c)) return 0.34
  if (WIDE.has(c)) return 0.86
  const code = c.codePointAt(0) ?? 0
  if (code >= 0x2e80) return 1
  if (c >= 'A' && c <= 'Z') return 0.68
  if (c >= '0' && c <= '9') return 0.58
  return 0.54
}

/** Each line's characters, and the first one's place in the paragraph. */
function split(lines: Line[]) {
  let at = 0
  return lines.map((line) => {
    const chars = [...line.text]
    const out = { line, chars, at }
    at += chars.length
    return out
  })
}

/** How far along a line character `k` starts, in points. */
function xOf(line: Line, chars: string[], k: number) {
  let total = 0
  let before = 0
  for (let i = 0; i < chars.length; i++) {
    if (i === k) before = total
    total += advance(chars[i])
  }
  if (k >= chars.length) before = total
  return total > 0 ? line.x + (before / total) * line.width : line.x
}

/** The character nearest a point in the paragraph's box, counted from its start. */
export function offsetAt(lines: Line[], x: number, y: number): number {
  const all = split(lines)
  if (all.length === 0) return 0
  const found = all.find(({ line }) => y < line.y + line.height) ?? all[all.length - 1]
  const { line, chars, at } = found
  let total = 0
  for (const c of chars) total += advance(c)
  const target = line.width > 0 ? ((x - line.x) / line.width) * total : 0
  let sum = 0
  for (let k = 0; k < chars.length; k++) {
    const w = advance(chars[k])
    if (sum + w / 2 > target) return at + k
    sum += w
  }
  return at + Math.max(0, chars.length - (chars[chars.length - 1] === '\n' ? 1 : 0))
}

/** Where character `offset` starts in the paragraph's box: `end` puts an offset between two lines at the end of the first. */
export function pointAt(lines: Line[], offset: number, end = false): { x: number; y: number; height: number } | null {
  const all = split(lines)
  if (all.length === 0) return null
  const found =
    all.find(({ at, chars }) => (end ? offset > at && offset <= at + chars.length : offset >= at && offset < at + chars.length)) ??
    (offset <= 0 ? all[0] : all[all.length - 1])
  const { line, chars, at } = found
  return { x: xOf(line, chars, Math.max(0, Math.min(chars.length, offset - at))), y: line.y, height: line.height }
}
