// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  type CSSProperties,
  type ElementType,
  type ComponentPropsWithoutRef,
  useLayoutEffect,
  useRef,
  useState,
  useId,
} from 'react'
import { squirclePath } from '@tinystream/shared/squircle'
import { useCorners } from '../lib/theme'

const svgUrl = (attrs: string, path: string) =>
  `url("data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" ${attrs} preserveAspectRatio="none"><path d="${path}"/></svg>`,
  )}")`

const tiles = new Map<string, { size: number; mask: string }>()

/**
 * A mask that doesn't depend on the box's size: the four corners as fixed
 * tiles, plus two solid bands for the straight edges. Browsers load mask
 * images asynchronously and paint nothing until they're ready, so a mask
 * rebuilt on every resize makes the box blink whenever its size changes.
 */
function cornerTile(radius: number, smoothing: number) {
  const key = `${radius}/${smoothing}`
  let tile = tiles.get(key)
  if (!tile) {
    // A pixel of straight edge past the curve, so the bands can overlap the
    // corners without a hairline seam at fractional scaling.
    const t = Math.ceil(radius * (1 + smoothing)) + 1
    const path = squirclePath(t * 2, t * 2, radius, smoothing)
    const corner = (x: number, y: number) => svgUrl(`viewBox="${x} ${y} ${t} ${t}"`, path)
    const band = 'linear-gradient(#000, #000)'
    const layers = [
      `${corner(0, 0)} left top / ${t}px ${t}px`,
      `${corner(t, 0)} right top / ${t}px ${t}px`,
      `${corner(0, t)} left bottom / ${t}px ${t}px`,
      `${corner(t, t)} right bottom / ${t}px ${t}px`,
      `${band} 0 ${t - 1}px / 100% calc(100% - ${t * 2 - 2}px)`,
      `${band} ${t - 1}px 0 / calc(100% - ${t * 2 - 2}px) 100%`,
    ]
    tile = { size: t, mask: layers.map((l) => `${l} no-repeat`).join(', ') }
    tiles.set(key, tile)
  }
  return tile
}

type Props<T extends ElementType> = {
  as?: T
  radius?: number
  smoothing?: number
  /** Draw a hairline edge that's brighter at the top, like light from above. */
  edge?: boolean
  /** Draw a dashed hairline along the curve, for placeholders; a CSS border would be cut off at the corners. */
  dashed?: boolean
} & ComponentPropsWithoutRef<T>

/**
 * A box with smoothed corners, masked to a squircle. Measures itself only to
 * draw the edge, and for boxes too small for the fixed corner tiles.
 */
export function Squircle<T extends ElementType = 'div'>({
  as,
  radius: baseRadius = 12,
  smoothing: baseSmoothing,
  edge = false,
  dashed = false,
  style,
  children,
  ...rest
}: Props<T>) {
  const Tag = (as ?? 'div') as ElementType
  const ref = useRef<HTMLElement>(null)
  const [size, setSize] = useState<[number, number] | null>(null)
  const id = 'sq' + useId().replace(/[^a-zA-Z0-9_-]/g, '')
  // The component style decides how round and how smooth corners are.
  const corners = useCorners()
  const radius = Math.round(baseRadius * corners.scale)
  const smoothing = baseSmoothing ?? corners.smoothing

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const measure = () => {
      const w = el.offsetWidth
      const h = el.offsetHeight
      setSize((s) => (s && s[0] === w && s[1] === h ? s : [w, h]))
    }
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const path = size ? squirclePath(size[0], size[1], radius, smoothing) : ''
  // A mask rather than clip-path: Firefox misplaces path() clips by a pixel or
  // two when the box sits at a fractional position (e.g. at 1.2× scaling),
  // cutting off one side and letting children leak past the other. A mask is
  // stretched over the box as painted, so it stays aligned.
  const tile = cornerTile(radius, smoothing)
  const mask =
    size && Math.min(size[0], size[1]) < tile.size * 2
      ? `${svgUrl(`viewBox="0 0 ${size[0]} ${size[1]}"`, path)} 0 0 / 100% 100% no-repeat`
      : tile.mask
  const clip: CSSProperties = { mask, WebkitMask: mask }

  return (
    <Tag ref={ref} style={{ ...clip, position: 'relative', ...style }} {...rest}>
      {children}
      {(edge || dashed) && size && (
        <svg
          aria-hidden
          width={size[0]}
          height={size[1]}
          style={{ position: 'absolute', inset: 0, pointerEvents: 'none', zIndex: 1 }}
        >
          <defs>
            <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" style={{ stopColor: 'var(--color-glow)', stopOpacity: 'var(--edge-top)' }} />
              <stop offset="0.35" style={{ stopColor: 'var(--color-glow)', stopOpacity: 'var(--edge-middle)' }} />
              <stop offset="1" style={{ stopColor: 'var(--color-glow)', stopOpacity: 'var(--edge-bottom)' }} />
            </linearGradient>
          </defs>
          {/* Twice the width, half of it clipped away: a crisp 1px inner edge. */}
          {dashed ? (
            <path d={path} fill="none" stroke="var(--color-line-strong)" strokeWidth={2} strokeDasharray="4 4" />
          ) : (
            <path d={path} fill="none" stroke={`url(#${id})`} strokeWidth={2} />
          )}
        </svg>
      )}
    </Tag>
  )
}
