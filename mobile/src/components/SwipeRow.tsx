// SPDX-License-Identifier: AGPL-3.0-or-later
// A row with an action on each side, revealed by pulling it aside: pulled
// far enough (felt as it passes the point), let go, and the action happens.

import { type ReactNode, useRef } from 'react'
import { Text, View } from 'react-native'
import ReanimatedSwipeable, { type SwipeableMethods } from 'react-native-gesture-handler/ReanimatedSwipeable'
import Animated, { type SharedValue, interpolate, useAnimatedReaction, useAnimatedStyle } from 'react-native-reanimated'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'

export type SwipeAction = { label: string; icon: ReactNode; color: string; text?: string; run: () => void }

const WIDTH = 96

export function SwipeRow({ left, right, children }: { left?: SwipeAction; right?: SwipeAction; children: ReactNode }) {
  const ref = useRef<SwipeableMethods>(null)
  if (!left && !right) return <>{children}</>
  return (
    <ReanimatedSwipeable
      ref={ref}
      friction={1.4}
      leftThreshold={WIDTH * 0.9}
      rightThreshold={WIDTH * 0.9}
      overshootFriction={6}
      renderLeftActions={left ? (p) => <Side action={left} progress={p} side="left" /> : undefined}
      renderRightActions={right ? (p) => <Side action={right} progress={p} side="right" /> : undefined}
      onSwipeableWillOpen={(direction) => {
        // "left" is where the row went, so it's the action on the right.
        const action = direction === 'left' ? right : left
        ref.current?.close()
        action?.run()
      }}
    >
      {children}
    </ReanimatedSwipeable>
  )
}

function Side({ action, progress, side }: { action: SwipeAction; progress: SharedValue<number>; side: 'left' | 'right' }) {
  useAnimatedReaction(
    () => progress.value >= 0.9,
    (past, was) => {
      if (was !== null && past !== was && past) scheduleOnRN(haptic, 'dismissThreshold')
    },
  )
  const style = useAnimatedStyle(() => ({
    opacity: interpolate(progress.value, [0, 0.4, 0.9], [0, 0.6, 1], 'clamp'),
    transform: [{ scale: interpolate(progress.value, [0, 0.9, 1.2], [0.7, 1, 1.08], 'clamp') }],
  }))
  return (
    <View style={{ width: WIDTH, justifyContent: 'center', alignItems: side === 'left' ? 'flex-start' : 'flex-end', paddingHorizontal: 8 }}>
      <Animated.View style={[{ alignItems: 'center', gap: 4, width: WIDTH - 16, paddingVertical: 10, borderRadius: 16, backgroundColor: action.color }, style]}>
        {action.icon}
        <Text className="font-sans text-xs font-medium" style={{ color: action.text ?? '#fff' }}>
          {action.label}
        </Text>
      </Animated.View>
    </View>
  )
}
