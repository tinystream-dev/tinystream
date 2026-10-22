// SPDX-License-Identifier: AGPL-3.0-or-later
// Recent samples as a soft area chart (web's Sparkline.tsx), each new one
// gliding in from the right over the polling interval.

import { Canvas, LinearGradient, Path, Skia, vec } from '@shopify/react-native-skia'
import { useEffect, useState } from 'react'
import { View, type ViewStyle } from 'react-native'
import { Easing, useDerivedValue, useReducedMotion, useSharedValue, withTiming } from 'react-native-reanimated'
import { withAlpha } from '../theme/materials'

/** How many samples a series keeps, and so how many a sparkline spans. */
const SAMPLES = 60

/**
 * Recent samples of a number, kept outside React so they survive re-renders
 * and navigation. `stamp` (e.g. a query's dataUpdatedAt) makes recording
 * idempotent, so it's safe to call while rendering.
 */
const series = new Map<string, { stamp: number; values: number[] }>()

export function record(key: string, value: number, stamp: number) {
  let s = series.get(key)
  if (!s) series.set(key, (s = { stamp: -1, values: [] }))
  if (s.stamp !== stamp) {
    s.stamp = stamp
    s.values = [...s.values, value].slice(-SAMPLES)
  }
  return s.values
}

export function Sparkline({ values, interval, color, max: fixedMax, style }: { values: number[]; interval: number; color: string; max?: number; style?: ViewStyle }) {
  const [size, setSize] = useState({ w: 0, h: 0 })
  const reduced = useReducedMotion()
  const t = useSharedValue(1)
  useEffect(() => {
    t.value = 0
    t.value = reduced ? 1 : withTiming(1, { duration: interval, easing: Easing.linear })
  }, [values, interval, reduced, t])
  const max = Math.max(1, fixedMax ?? Math.max(...values, 1))
  const quiet = values.every((v) => v <= 0)
  const padded = [...Array<number>(Math.max(0, SAMPLES - values.length)).fill(values[0] ?? 0), ...values]
  const { w, h } = size
  const shape = useDerivedValue(() => {
    const step = w / (SAMPLES - 2)
    const left = w + step * (1 - t.value) - step * (padded.length - 1)
    const pts = padded.map((v, i) => [left + i * step, h - (Math.min(v, max) / max) * (h - 2) - 1] as const)
    const line = Skia.PathBuilder.Make()
    if (!pts.length || !w) return { line: line.build(), area: line.build() }
    line.moveTo(pts[0][0], pts[0][1])
    for (let i = 1; i < pts.length; i++) {
      const [x0, y0] = pts[i - 1]
      const [x1, y1] = pts[i]
      const mx = (x0 + x1) / 2
      line.cubicTo(mx, y0, mx, y1, x1, y1)
    }
    const stroke = line.build()
    const area = line.lineTo(pts[pts.length - 1][0], h).lineTo(left, h).close().detach()
    return { line: stroke, area }
  })
  const line = useDerivedValue(() => shape.value.line)
  const area = useDerivedValue(() => shape.value.area)
  return (
    <View style={[{ opacity: quiet ? 0 : 1 }, style]} pointerEvents="none" onLayout={(e) => setSize({ w: e.nativeEvent.layout.width, h: e.nativeEvent.layout.height })}>
      {w > 0 && (
        <Canvas style={{ flex: 1 }}>
          <Path path={area}>
            <LinearGradient start={vec(0, 0)} end={vec(0, h)} colors={[withAlpha(color, 0.28), withAlpha(color, 0)]} />
          </Path>
          <Path path={line} style="stroke" strokeWidth={1.5} color={color} />
        </Canvas>
      )}
    </View>
  )
}

/** The torrent's piece map: each cell lights up as that stretch arrives. */
export function PieceMap({ pieces, color }: { pieces: number[]; color: string }) {
  if (!pieces.length) return null
  return (
    <View className="h-1.5 flex-row overflow-hidden rounded-[3px]" style={{ gap: 1 }}>
      {pieces.map((p, i) => (
        <View key={i} style={{ flex: 1, backgroundColor: color, opacity: p >= 255 ? 0.9 : 0.1 + (p / 255) * 0.55 }} />
      ))}
    </View>
  )
}
