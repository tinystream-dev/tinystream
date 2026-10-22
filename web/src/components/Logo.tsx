// SPDX-License-Identifier: AGPL-3.0-or-later

import { useEffect, useId, useState } from 'react'
import { CUT, GLYPH, PLATE } from '@tinystream/shared/logo'
import { lensMap } from '../lib/logo'
import { useStyle } from '../lib/theme'

type Props = { size: number; className?: string }

/** The logo in the scheme's colours, finished like the component style. */
export function Logo({ size, className }: Props) {
  const style = useStyle()
  const id = 'logo' + useId().replace(/[^a-zA-Z0-9_-]/g, '')
  if (style === 'glass') return <Glass size={size} id={id} className={className} />
  return (
    <svg aria-hidden width={size} height={size} viewBox="0 0 1024 1024" className={`shrink-0 ${className ?? ''}`}>
      {style === 'layered' && (
        <defs>
          <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" style={{ stopColor: 'var(--color-ink)' }} />
            <stop offset="1" style={{ stopColor: 'color-mix(in srgb, var(--color-ink) 86%, var(--color-shade))' }} />
          </linearGradient>
        </defs>
      )}
      <path d={PLATE} fill={style === 'layered' ? `url(#${id})` : 'var(--color-ink)'} />
      <path d={GLYPH} fill="var(--color-canvas)" />
    </svg>
  )
}

const svgMask = `url("data:image/svg+xml,${encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024"><path d="${CUT}"/></svg>`,
)}") 0 0 / 100% 100% no-repeat`

/** A colour at some opacity. */
const mix = (color: string, percent: number) => `color-mix(in srgb, ${color} ${Math.min(100, percent)}%, transparent)`
const lightDark = (light: string, dark: string) => `light-dark(${light}, ${dark})`

/**
 * Liquid Glass: "Ts" cut out of a clear pane. Where the browser can, its rim
 * bends what's behind it like the edge of a lens; elsewhere what's behind is
 * only frosted. Edges catch a white line of light, brightest where they face
 * up and left and again where they face down and right, with a darker band
 * just inside where the glass thickens. Sizes are in the logo's units (1024
 * across), worked out from pixels so the edges stay crisp when it's small;
 * small and on a light scheme, the pane is tinted so it doesn't vanish.
 */
function Glass({ size, id, className }: Props & { id: string }) {
  const [lens, setLens] = useState<string | null>(null)
  useEffect(() => setLens(lensMap(size)), [size])
  const px = 1024 / size
  const hair = Math.max(7, 0.75 * px)
  const soft = Math.max(2, 0.25 * px)
  const small = size <= 40
  const backdrop = lens
    ? `url(#${id}-lens) blur(${Math.max(0.4, size / 160)}px) saturate(1.6) brightness(1.04)`
    : `blur(${Math.max(1, size / 48)}px) saturate(1.8) brightness(1.06)`
  const white = (light: number, dark: number) => lightDark(`rgb(255 255 255 / ${light})`, `rgb(255 255 255 / ${dark})`)
  const stop = (offset: number, light: number, dark: number) => (
    <stop offset={offset} style={{ stopColor: lightDark(small ? mix('var(--color-ink)', light) : mix('#fff', light), mix('#fff', dark)) }} />
  )
  const lit = (dx: number, dy: number, opacity: number, result: string) => (
    <>
      <feOffset in="SourceAlpha" dx={dx} dy={dy} result={`${result}-moved`} />
      <feComposite in="SourceAlpha" in2={`${result}-moved`} operator="out" />
      <feGaussianBlur stdDeviation={soft} result={`${result}-shape`} />
      <feFlood style={{ floodColor: white(small ? opacity * 0.6 : opacity, opacity) }} />
      <feComposite in2={`${result}-shape`} operator="in" result={result} />
    </>
  )
  return (
    <span className={`relative inline-block shrink-0 ${className ?? ''}`} style={{ width: size, height: size }}>
      {/* A mask would stop Chromium running the lens, so there it's a clip. */}
      <span
        className="absolute inset-0"
        style={{
          backdropFilter: backdrop,
          WebkitBackdropFilter: backdrop,
          ...(lens ? { clipPath: `url(#${id}-clip)` } : { mask: svgMask, WebkitMask: svgMask }),
        }}
      />
      <svg aria-hidden width={size} height={size} viewBox="0 0 1024 1024" className="absolute inset-0 overflow-visible">
        <defs>
          <path id={`${id}-shape`} d={CUT} />
          <clipPath id={`${id}-clip`} clipPathUnits="objectBoundingBox">
            <path transform={`scale(${1 / 1024})`} d={CUT} />
          </clipPath>
          {lens && (
            <filter
              id={`${id}-lens`}
              x="0"
              y="0"
              width={size}
              height={size}
              filterUnits="userSpaceOnUse"
              primitiveUnits="userSpaceOnUse"
              colorInterpolationFilters="sRGB"
            >
              <feImage href={lens} x="0" y="0" width={size} height={size} preserveAspectRatio="none" result="map" />
              <feDisplacementMap in="SourceGraphic" in2="map" scale={size * 0.07} xChannelSelector="R" yChannelSelector="G" />
            </filter>
          )}
          <filter id={`${id}-shadow`} x="-40%" y="-40%" width="180%" height="190%" colorInterpolationFilters="sRGB">
            <feGaussianBlur in="SourceAlpha" stdDeviation={Math.max(18, 2.2 * px)} />
            <feOffset dy={Math.max(14, 1.6 * px)} result="wide" />
            <feFlood style={{ floodColor: lightDark(mix('var(--color-shade)', 16), mix('var(--color-shade)', 45)) }} />
            <feComposite in2="wide" operator="in" result="far" />
            <feGaussianBlur in="SourceAlpha" stdDeviation={Math.max(3, 0.45 * px)} result="tight" />
            <feFlood style={{ floodColor: lightDark(mix('var(--color-shade)', small ? 45 : 30), mix('var(--color-shade)', 50)) }} />
            <feComposite in2="tight" operator="in" result="near" />
            <feMerge>
              <feMergeNode in="far" />
              <feMergeNode in="near" />
            </feMerge>
            {/* Only around it: what's under the glass shows through. */}
            <feComposite in2="SourceAlpha" operator="out" />
          </filter>
          <filter id={`${id}-light`} x="0" y="0" width="100%" height="100%" colorInterpolationFilters="sRGB">
            <feGaussianBlur in="SourceAlpha" stdDeviation={Math.max(10, 1.4 * px)} result="blurred" />
            <feComposite in="SourceAlpha" in2="blurred" operator="out" result="thick" />
            <feFlood style={{ floodColor: lightDark(mix('var(--color-ink)', small ? 75 : 42), mix('#fff', small ? 22 : 16)) }} />
            <feComposite in2="thick" operator="in" result="band" />
            <feMorphology in="SourceAlpha" operator="erode" radius={hair} result="inner" />
            <feComposite in="SourceAlpha" in2="inner" operator="out" />
            <feGaussianBlur stdDeviation={soft * 0.6} result="line" />
            <feFlood style={{ floodColor: white(small ? 0.35 : 0.95, 0.32) }} />
            <feComposite in2="line" operator="in" result="rim" />
            {lit(hair * 0.7, hair * 1.3, 1, 'top')}
            {lit(-hair * 0.7, -hair * 1.3, 0.75, 'bottom')}
            <feMerge>
              <feMergeNode in="band" />
              <feMergeNode in="rim" />
              <feMergeNode in="top" />
              <feMergeNode in="bottom" />
            </feMerge>
            <feComposite in2="SourceAlpha" operator="in" />
          </filter>
          <linearGradient id={`${id}-body`} x1="0" y1="0" x2="0.3" y2="1">
            {stop(0, small ? 48 : 55, 14)}
            {stop(0.6, small ? 30 : 28, 6)}
            {stop(1, small ? 38 : 42, 10)}
          </linearGradient>
        </defs>
        <use href={`#${id}-shape`} filter={`url(#${id}-shadow)`} />
        <use href={`#${id}-shape`} fill={`url(#${id}-body)`} />
        <use href={`#${id}-shape`} filter={`url(#${id}-light)`} />
      </svg>
    </span>
  )
}

