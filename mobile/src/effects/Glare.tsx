// SPDX-License-Identifier: AGPL-3.0-or-later

import { Canvas, Circle, RadialGradient } from '@shopify/react-native-skia'
import { StyleSheet } from 'react-native'
import { type SharedValue, useDerivedValue } from 'react-native-reanimated'
import { useTheme } from '../theme/ThemeProvider'
import { withAlpha } from '../theme/materials'

/**
 * The glint where light would catch a tilted card (web's `.glare`): a soft
 * spot at (x, y), each 0–1 across the card, sized off its short side.
 */
export function Glare({
  x,
  y,
  opacity,
  width,
  height,
}: {
  x: SharedValue<number>
  y: SharedValue<number>
  opacity: SharedValue<number>
  width: number
  height: number
}) {
  const { tokens } = useTheme()
  const r = Math.min(width, height) * 0.65
  const c = useDerivedValue(() => ({ x: x.value * width, y: y.value * height }))
  const cx = useDerivedValue(() => c.value.x)
  const cy = useDerivedValue(() => c.value.y)
  return (
    <Canvas style={StyleSheet.absoluteFill} pointerEvents="none">
      <Circle cx={cx} cy={cy} r={r} opacity={opacity} blendMode="softLight">
        <RadialGradient c={c} r={r} colors={[withAlpha(tokens['media-ink'], 0.55), withAlpha(tokens['media-ink'], 0)]} positions={[0, 0.7]} />
      </Circle>
    </Canvas>
  )
}
