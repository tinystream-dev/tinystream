// SPDX-License-Identifier: AGPL-3.0-or-later
// A bottom sheet in the theme's colours, open while `open` is true: what web
// shows as a dialog or a menu, the app shows as one of these. Drag it down
// or tap outside to close it. It's a Modal, so it keeps the context of where
// it's used (a portal wouldn't).

import { type ReactNode, useEffect, useRef, useState } from 'react'
import { KeyboardAvoidingView, Modal, Pressable, StyleSheet, View, useWindowDimensions } from 'react-native'
import { Gesture, GestureDetector, GestureHandlerRootView } from 'react-native-gesture-handler'
import Animated, { Easing, interpolate, useAnimatedStyle, useReducedMotion, useSharedValue, withSpring, withTiming } from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { useTheme } from '../theme/ThemeProvider'
import { lift, withAlpha } from '../theme/materials'

/** How far down a drag has to go (or how fast) to close the sheet. */
const DISMISS = 110
const FLING = 900

type Props = { open: boolean; onClose: () => void; children: ReactNode }

/** Nothing at all while it's closed: a list can have one on every row. */
export function Sheet(props: Props) {
  const [mounted, setMounted] = useState(props.open)
  if (props.open && !mounted) setMounted(true)
  return mounted ? <Body {...props} onClosed={() => setMounted(false)} /> : null
}

function Body({ open, onClose, onClosed, children }: Props & { onClosed: () => void }) {
  const { height } = useWindowDimensions()
  const insets = useSafeAreaInsets()
  const { tokens, style, material } = useTheme()
  const reduced = useReducedMotion()
  const y = useSharedValue(height)
  const past = useSharedValue(false)

  const shown = useRef(open)
  shown.current = open

  const closed = useRef(onClosed)
  closed.current = onClosed

  useEffect(() => {
    if (open) {
      y.value = reduced ? 0 : withTiming(0, { duration: 280, easing: Easing.bezier(0.2, 0.8, 0.2, 1) })
    } else {
      // Unless it was opened again while closing.
      const unmount = () => shown.current || closed.current()
      if (reduced) unmount()
      else y.value = withTiming(height, { duration: 200 }, (done) => done && scheduleOnRN(unmount))
    }
  }, [open, reduced, height, y])

  const drag = Gesture.Pan()
    .activeOffsetY(8)
    .onUpdate((e) => {
      y.value = Math.max(0, e.translationY)
      const over = e.translationY > DISMISS
      if (over !== past.value) {
        past.value = over
        if (over) scheduleOnRN(haptic, 'dismissThreshold')
      }
    })
    .onEnd((e) => {
      past.value = false
      if (e.translationY > DISMISS || e.velocityY > FLING) scheduleOnRN(onClose)
      else y.value = withSpring(0, { damping: 22, stiffness: 260 })
    })

  const sheet = useAnimatedStyle(() => ({ transform: [{ translateY: y.value }] }))
  const backdrop = useAnimatedStyle(() => ({ opacity: interpolate(y.value, [0, height * 0.6], [1, 0], 'clamp') }))
  const radius = Math.round(28 * material.corners.scale)

  return (
    <Modal visible transparent animationType="none" statusBarTranslucent navigationBarTranslucent onRequestClose={onClose}>
      <GestureHandlerRootView style={{ flex: 1 }}>
        <Animated.View style={[StyleSheet.absoluteFill, { backgroundColor: withAlpha(tokens.shade, 0.5) }, backdrop]}>
          <Pressable style={{ flex: 1 }} onPress={onClose} accessibilityLabel="Close" />
        </Animated.View>
        <KeyboardAvoidingView behavior="padding" style={{ flex: 1, justifyContent: 'flex-end' }} pointerEvents="box-none">
          <GestureDetector gesture={drag}>
            <Animated.View
              style={[
                sheet,
                {
                  backgroundColor: tokens.float,
                  borderTopLeftRadius: radius,
                  borderTopRightRadius: radius,
                  borderTopWidth: StyleSheet.hairlineWidth,
                  borderColor: withAlpha(tokens.glow, material.edge[0]),
                  boxShadow: lift(style, tokens),
                },
              ]}
            >
              <View className="items-center pb-2 pt-3">
                <View className="h-1 w-9 rounded-full bg-ink-3" />
              </View>
              <View style={{ paddingHorizontal: 20, paddingBottom: insets.bottom + 20 }}>{children}</View>
            </Animated.View>
          </GestureDetector>
        </KeyboardAvoidingView>
      </GestureHandlerRootView>
    </Modal>
  )
}
