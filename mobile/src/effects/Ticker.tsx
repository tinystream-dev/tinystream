// SPDX-License-Identifier: AGPL-3.0-or-later

import { useEffect, useState } from 'react'
import { type StyleProp, Text, type TextStyle, View } from 'react-native'
import Animated, { Easing, useAnimatedStyle, useReducedMotion, useSharedValue, withTiming } from 'react-native-reanimated'

const ROLL = { duration: 500, easing: Easing.bezier(0.2, 0.8, 0.2, 1) }

const DIGITS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]

/**
 * Text whose digits roll to their new value (web's Ticker). Each digit is
 * keyed by its distance from the end, so "9" → "10" rolls the last place and
 * a new place appears in front.
 */
export function Ticker({ value, className = '', style }: { value: string; className?: string; style?: StyleProp<TextStyle> }) {
  const [line, setLine] = useState(0)
  const chars = [...value]
  const text = `${className} tabular-nums`
  return (
    <View accessible accessibilityLabel={value} className="flex-row">
      {/* Measures a line of this text, which is how far each digit rolls. */}
      <Text className={`${text} absolute opacity-0`} style={style} onLayout={(e) => setLine(e.nativeEvent.layout.height)}>
        0
      </Text>
      {chars.map((c, i) => {
        const key = chars.length - i
        return /\d/.test(c) ? (
          <Digit key={key} d={Number(c)} line={line} className={text} style={style} />
        ) : (
          <Text key={`${key}${c}`} className={text} style={style}>
            {c}
          </Text>
        )
      })}
    </View>
  )
}

function Digit({ d, line, className, style }: { d: number; line: number; className: string; style?: StyleProp<TextStyle> }) {
  const reduced = useReducedMotion()
  const at = useSharedValue(d)
  useEffect(() => {
    at.value = reduced ? d : withTiming(d, ROLL)
  }, [d, reduced, at])
  const roll = useAnimatedStyle(() => ({ transform: [{ translateY: -at.value * line }] }))
  return (
    <View style={{ height: line || undefined, overflow: 'hidden' }}>
      <Text className={`${className} opacity-0`} style={style}>
        0
      </Text>
      <Animated.View style={[{ position: 'absolute', left: 0, right: 0, top: 0, alignItems: 'center' }, roll]}>
        {DIGITS.map((n) => (
          <Text key={n} className={className} style={style}>
            {n}
          </Text>
        ))}
      </Animated.View>
    </View>
  )
}
