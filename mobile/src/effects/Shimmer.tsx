// SPDX-License-Identifier: AGPL-3.0-or-later
// Placeholders shaped like the content that's about to arrive (web's `bone`
// and Skeleton.tsx): a band of light sweeping across, again and again.

import { useEffect, useState } from 'react'
import { StyleSheet, View, type ViewStyle } from 'react-native'
import Animated, { Easing, useAnimatedStyle, useReducedMotion, useSharedValue, withRepeat, withTiming } from 'react-native-reanimated'
import { useTheme } from '../theme/ThemeProvider'
import { Squircle, type SquircleProps } from './Squircle'

/** The sweep across one placeholder, filling whatever it's in. A plain view, not a canvas: a skeleton has dozens. */
export function Shimmer() {
  const { tokens } = useTheme()
  const reduced = useReducedMotion()
  const [width, setWidth] = useState(0)
  const t = useSharedValue(0)

  useEffect(() => {
    if (reduced) return
    t.value = withRepeat(withTiming(1, { duration: 1600, easing: Easing.inOut(Easing.ease) }), -1)
  }, [reduced, t])

  // A gradient 2.5 times as wide as the box, moving from 150% to -50% (background-position).
  const sweep = useAnimatedStyle(() => ({ transform: [{ translateX: -(1.5 - 2 * t.value) * width * 1.5 }] }))

  return (
    <View style={StyleSheet.absoluteFill} onLayout={(e) => setWidth(e.nativeEvent.layout.width)} pointerEvents="none">
      <Animated.View
        style={[
          {
            position: 'absolute',
            top: 0,
            bottom: 0,
            left: 0,
            width: width * 2.5,
            experimental_backgroundImage: `linear-gradient(to right, ${tokens.raised} 0%, ${tokens.raised} 30%, ${tokens.panel} 50%, ${tokens.raised} 70%, ${tokens.raised} 100%)`,
          } as ViewStyle,
          sweep,
        ]}
      />
    </View>
  )
}

export function Bone({ radius = 12, style, ...rest }: SquircleProps) {
  const { tokens } = useTheme()
  return (
    <Squircle radius={radius} style={[{ backgroundColor: tokens.raised }, style]} {...rest}>
      <Shimmer />
    </Squircle>
  )
}
