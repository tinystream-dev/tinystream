// SPDX-License-Identifier: AGPL-3.0-or-later
// Continuous-curvature ("squircle") corners, drawn as an SVG path.
//
// Plain border-radius joins a straight edge to a circular arc, and the jump in
// curvature is visible as a faint "kink". Here each corner eases into a short
// arc through two cubic Béziers.
// Firefox has no `corner-shape`, so we build the path ourselves and use it
// both for clip-path and for the outline stroke.

type Vec = [number, number]

const rad = (deg: number) => (deg * Math.PI) / 180
const f = (n: number) => Math.round(n * 1000) / 1000

/** Rotates a vector 90° clockwise `turns` times (SVG's y axis points down). */
function rot([x, y]: Vec, turns: number): Vec {
  for (let i = 0; i < turns; i++) [x, y] = [-y, x]
  return [x, y]
}

function corner(radius: number, smoothing: number, maxP: number) {
  let r = Math.min(radius, maxP)
  let s = smoothing
  let p = (1 + s) * r
  if (p > maxP) {
    s = Math.max(0, Math.min(s, maxP / r - 1))
    p = Math.min(p, maxP)
  }
  const arcMeasure = 90 * (1 - s)
  const arc = Math.sin(rad(arcMeasure / 2)) * r * Math.SQRT2
  const alpha = (90 - arcMeasure) / 2
  const p3p4 = r * Math.tan(rad(alpha / 2))
  const beta = 45 * s
  const c = p3p4 * Math.cos(rad(beta))
  const d = c * Math.tan(rad(beta))
  const b = (p - arc - c - d) / 3
  const a = 2 * b
  return { r, p, a, b, c, d, arc }
}

/**
 * Path for a w×h box with smoothed corners. `smoothing` 0 is a plain rounded
 * rectangle; 0.6 is iOS-like.
 */
export function squirclePath(w: number, h: number, radius: number, smoothing = 0.6): string {
  if (w <= 0 || h <= 0) return ''
  const k = corner(radius, smoothing, Math.min(w, h) / 2)
  if (k.r <= 0) return `M0 0H${f(w)}V${f(h)}H0Z`

  // One corner, as relative moves, for the top-right; the others are rotations.
  const seg = (turns: number) => {
    const c1 = rot([k.a, 0], turns)
    const c2 = rot([k.a + k.b, 0], turns)
    const e1 = rot([k.a + k.b + k.c, k.d], turns)
    const arc = rot([k.arc, k.arc], turns)
    const c3 = rot([k.d, k.c], turns)
    const c4 = rot([k.d, k.b + k.c], turns)
    const e2 = rot([k.d, k.a + k.b + k.c], turns)
    return (
      `c${f(c1[0])} ${f(c1[1])} ${f(c2[0])} ${f(c2[1])} ${f(e1[0])} ${f(e1[1])}` +
      `a${f(k.r)} ${f(k.r)} 0 0 1 ${f(arc[0])} ${f(arc[1])}` +
      `c${f(c3[0])} ${f(c3[1])} ${f(c4[0])} ${f(c4[1])} ${f(e2[0])} ${f(e2[1])}`
    )
  }
  return (
    `M${f(w - k.p)} 0` +
    seg(0) +
    `L${f(w)} ${f(h - k.p)}` +
    seg(1) +
    `L${f(k.p)} ${f(h)}` +
    seg(2) +
    `L0 ${f(k.p)}` +
    seg(3) +
    'Z'
  )
}
