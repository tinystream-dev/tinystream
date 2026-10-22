// SPDX-License-Identifier: AGPL-3.0-or-later
// The tab's icon, drawn from the logo's shapes in the scheme's colours, and
// the glass logo's lens.

import { CUT, logoSvg } from '@tinystream/shared/logo'
import { useEffect, useSyncExternalStore } from 'react'
import { cssColor, onThemeChange } from './theme'

const lenses = new Map<number, string>()

/**
 * How glass in the logo's shape bends what's behind it, as a displacement map
 * (red and green: how far right and down to look): toward the inside along a
 * thin rounded rim, not at all where it's flat. Null where it can't be used:
 * only Chromium can run an SVG filter on what's behind an element.
 */
export function lensMap(size: number) {
  const brands = (navigator as { userAgentData?: { brands: { brand: string }[] } }).userAgentData?.brands
  if (!brands?.some((b) => b.brand === 'Chromium')) return null
  let url = lenses.get(size)
  if (url) return url
  const n = Math.max(64, size * 2)
  const k = n / 1024
  const shape = new OffscreenCanvas(n, n).getContext('2d', { willReadFrequently: true })!
  shape.scale(k, k)
  shape.fill(new Path2D(CUT))
  const soft = new OffscreenCanvas(n, n).getContext('2d', { willReadFrequently: true })!
  soft.filter = `blur(${Math.max(1, 14 * k)}px)`
  soft.drawImage(shape.canvas, 0, 0)
  const inside = shape.getImageData(0, 0, n, n).data
  const depth = soft.getImageData(0, 0, n, n).data
  const at = (x: number, y: number) => depth[(Math.min(n - 1, Math.max(0, y)) * n + Math.min(n - 1, Math.max(0, x))) * 4 + 3] / 255
  const map = new ImageData(n, n)
  for (let y = 0; y < n; y++)
    for (let x = 0; x < n; x++) {
      const i = (y * n + x) * 4
      let dx = 0
      let dy = 0
      if (inside[i + 3]) {
        // Which way is inward, from how the blurred shape rises.
        const gx = at(x + 2, y - 1) + at(x + 2, y) + at(x + 2, y + 1) - at(x - 2, y - 1) - at(x - 2, y) - at(x - 2, y + 1)
        const gy = at(x - 1, y + 2) + at(x, y + 2) + at(x + 1, y + 2) - at(x - 1, y - 2) - at(x, y - 2) - at(x + 1, y - 2)
        const length = Math.hypot(gx, gy)
        // 1 at the very edge, 0 where the glass is flat; the rim bends light most at its edge.
        const t = Math.max(0, Math.min(1, (0.97 - at(x, y)) / 0.47))
        if (length > 1e-4) {
          dx = (gx / length) * t * t * (3 - 2 * t)
          dy = (gy / length) * t * t * (3 - 2 * t)
        }
      }
      map.data[i] = 128 + dx * 127
      map.data[i + 1] = 128 + dy * 127
      map.data[i + 2] = 128
      map.data[i + 3] = 255
    }
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = n
  canvas.getContext('2d')!.putImageData(map, 0, 0)
  url = canvas.toDataURL()
  lenses.set(size, url)
  return url
}

let progress: number | null = null
const listeners = new Set<() => void>()

/** Download progress to draw on the tab's icon, or null for none. */
export function setIconProgress(p: number | null) {
  if (p === progress) return
  progress = p
  listeners.forEach((l) => l())
}

function subscribe(l: () => void) {
  // With a light and a dark scheme, the colours change with the system's.
  const media = matchMedia('(prefers-color-scheme: dark)')
  const off = onThemeChange(l)
  listeners.add(l)
  media.addEventListener('change', l)
  return () => {
    off()
    listeners.delete(l)
    media.removeEventListener('change', l)
  }
}

const snapshot = () => `${cssColor('ink')}/${cssColor('canvas')}/${progress}`

/** Keeps the tab's icon in the scheme's colours, with download progress on it. */
export function useFavicon() {
  const key = useSyncExternalStore(subscribe, snapshot, () => null)
  useEffect(() => {
    const link = document.querySelector<HTMLLinkElement>('link[rel="icon"]')
    if (!link || !key) return
    const svg = `data:image/svg+xml,${encodeURIComponent(logoSvg(cssColor('ink'), cssColor('canvas')))}`
    const pct = progress
    if (pct === null) {
      link.type = 'image/svg+xml'
      link.href = svg
      return
    }
    let stale = false
    const icon = new Image()
    icon.onload = () => {
      if (stale) return
      const canvas = document.createElement('canvas')
      canvas.width = canvas.height = 64
      const ctx = canvas.getContext('2d')!
      ctx.drawImage(icon, 0, 0, 64, 64)
      // A badge in the corner with the ring inside it.
      ctx.fillStyle = cssColor('canvas')
      ctx.beginPath()
      ctx.arc(46, 46, 18, 0, Math.PI * 2)
      ctx.fill()
      ctx.lineWidth = 6
      ctx.lineCap = 'round'
      ctx.strokeStyle = cssColor('glow')
      ctx.globalAlpha = 0.2
      ctx.beginPath()
      ctx.arc(46, 46, 11, 0, Math.PI * 2)
      ctx.stroke()
      ctx.globalAlpha = 1
      ctx.strokeStyle = cssColor('info')
      ctx.beginPath()
      ctx.arc(46, 46, 11, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * Math.max(0.02, pct))
      ctx.stroke()
      link.type = 'image/png'
      link.href = canvas.toDataURL('image/png')
    }
    icon.src = svg
    return () => {
      stale = true
    }
  }, [key])
}
