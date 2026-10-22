// SPDX-License-Identifier: AGPL-3.0-or-later
// Cards lean toward where they're pressed, then spring back (web's useTilt,
// for touch). Moving the finger lets the scroll view have it. Hero artwork
// can also lean with the phone, with a glint where the light would catch.

import { useFocusEffect } from 'expo-router'
import { type ReactNode, useCallback, useEffect, useRef, useState } from 'react'
import { type LayoutChangeEvent, type StyleProp, type ViewStyle } from 'react-native'
import { Gesture, GestureDetector } from 'react-native-gesture-handler'
import Animated, {
  Easing,
  type SharedValue,
  SensorType,
  useAnimatedSensor,
  useAnimatedStyle,
  useDerivedValue,
  useFrameCallback,
  useReducedMotion,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { Glare } from './Glare'
import { Squircle } from './Squircle'

const SPRING = { damping: 14, stiffness: 140, mass: 0.6 }
const LEAN = { duration: 120, easing: Easing.out(Easing.quad) }

/** How far the phone has to turn (radians) for the card to lean most of the way. */
const GYRO_RANGE = 0.3
/** Turning faster than this (rad/s), e.g. picking the phone up, counts for less and less. */
const GYRO_FAST = 1.5
/** How long (seconds) the card takes to settle back once the phone is held still. */
const GYRO_SETTLE = 0.9
/** How long (seconds) the card takes to catch up with the phone. */
const GYRO_EASE = 0.18

export type TiltProps = {
  /** The most it leans, in degrees. Wide cards want less: their edges travel further. */
  max?: number
  /** Also lean with the phone (hero artwork only), while the screen is in focus. */
  gyro?: boolean
  onPress?: () => void
  /** A finger touching the card, before it's known whether it'll be a press: to get ready for one. */
  onPressIn?: () => void
  onLongPress?: () => void
  /** The card's squircle corners. */
  radius?: number
  edge?: boolean
  style?: StyleProp<ViewStyle>
  children: ReactNode
}

export function Tilt({ max = 7, gyro = false, onPress, onPressIn, onLongPress, radius = 14, edge = true, style, children }: TiltProps) {
  const reduced = useReducedMotion()
  const [size, setSize] = useState({ width: 0, height: 0 })
  const width = useSharedValue(0)
  const height = useSharedValue(0)
  // Pressed: the lean in degrees; gyro: the phone's lean (-1–1).
  const rx = useSharedValue(0)
  const ry = useSharedValue(0)
  const gx = useSharedValue(0)
  const gy = useSharedValue(0)

  const press = (x: number, y: number) => {
    'worklet'
    if (reduced) return
    const cx = Math.min(1, Math.max(0, x / Math.max(1, width.value)))
    const cy = Math.min(1, Math.max(0, y / Math.max(1, height.value)))
    ry.value = withTiming((cx - 0.5) * 2 * max, LEAN)
    rx.value = withTiming((0.5 - cy) * 2 * max, LEAN)
  }
  const release = () => {
    'worklet'
    rx.value = withSpring(0, SPRING)
    ry.value = withSpring(0, SPRING)
  }

  const tap = Gesture.Tap()
    .maxDistance(12)
    .onBegin((e) => {
      press(e.x, e.y)
      if (onPressIn) scheduleOnRN(onPressIn)
    })
    .onEnd((_, success) => {
      if (success && onPress) scheduleOnRN(onPress)
    })
    .onFinalize(release)
  // A finger resting on a card that has a menu is felt starting to build, before the menu opens.
  const building = useRef<ReturnType<typeof setTimeout> | null>(null)
  const startBuilding = () => {
    building.current = setTimeout(() => haptic('longPressStart'), 180)
  }
  const stopBuilding = () => {
    if (building.current) clearTimeout(building.current)
    building.current = null
  }
  const hold = Gesture.LongPress()
    .enabled(onLongPress != null)
    .maxDistance(12)
    .onBegin((e) => {
      press(e.x, e.y)
      scheduleOnRN(startBuilding)
    })
    .onStart(() => {
      if (!onLongPress) return
      scheduleOnRN(stopBuilding)
      scheduleOnRN(haptic, 'longPressOpen')
      scheduleOnRN(onLongPress)
    })
    .onFinalize(() => {
      scheduleOnRN(stopBuilding)
      release()
    })

  // The sensor only runs while the screen is in focus. Other cards don't follow it: there are lots of them.
  const [focused, setFocused] = useState(false)
  useFocusEffect(
    useCallback(() => {
      if (!gyro) return
      setFocused(true)
      return () => setFocused(false)
    }, [gyro]),
  )
  const leaning = gyro && focused && !reduced
  useEffect(() => {
    if (leaning) return
    gx.value = withSpring(0, SPRING)
    gy.value = withSpring(0, SPRING)
  }, [leaning, gx, gy])

  const tilt = useAnimatedStyle(() => ({
    transform: [
      { perspective: 700 },
      { rotateX: `${Math.min(max, Math.max(-max, rx.value + gx.value * max))}deg` },
      { rotateY: `${Math.min(max, Math.max(-max, ry.value - gy.value * max))}deg` },
    ],
  }))

  // The glint goes where the phone's lean puts the light.
  const spotX = useDerivedValue(() => 0.5 - gy.value * 0.5)
  const spotY = useDerivedValue(() => 0.5 - gx.value * 0.5)
  const spot = useDerivedValue(() => Math.min(1, Math.hypot(gx.value, gy.value)))

  const measure = (e: LayoutChangeEvent) => {
    const { width: w, height: h } = e.nativeEvent.layout
    width.value = w
    height.value = h
    if (gyro) setSize({ width: w, height: h })
  }

  return (
    <>
      {leaning && <Gyro x={gx} y={gy} />}
      <GestureDetector gesture={Gesture.Exclusive(hold, tap)}>
        <Animated.View style={[style, tilt]} onLayout={measure}>
          <Squircle radius={radius} edge={edge} style={{ flex: 1 }}>
            {children}
            {gyro && !reduced && size.width > 0 && <Glare x={spotX} y={spotY} opacity={spot} width={size.width} height={size.height} />}
          </Squircle>
        </Animated.View>
      </GestureDetector>
    </>
  )
}

/**
 * Leans (x, y: -1–1) with the phone turning, from the gyroscope's rate of
 * turn rather than its attitude: however the phone is held, or put down,
 * the card settles back to rest, and only turning it moves the card.
 */
function Gyro({ x, y }: { x: SharedValue<number>; y: SharedValue<number> }) {
  const { sensor } = useAnimatedSensor(SensorType.GYROSCOPE, { interval: 'auto' })
  const turn = useSharedValue({ x: 0, y: 0 })
  useFrameCallback(({ timeSincePreviousFrame }) => {
    const dt = Math.min(0.05, (timeSincePreviousFrame ?? 16) / 1000)
    const rate = sensor.value
    const soft = (r: number) => Math.tanh(r / GYRO_FAST) * GYRO_FAST
    const settle = Math.exp(-dt / GYRO_SETTLE)
    const t = { x: (turn.value.x + soft(rate.x) * dt) * settle, y: (turn.value.y + soft(rate.y) * dt) * settle }
    turn.value = t
    const ease = 1 - Math.exp(-dt / GYRO_EASE)
    x.value += (Math.tanh(t.x / GYRO_RANGE) - x.value) * ease
    y.value += (Math.tanh(t.y / GYRO_RANGE) - y.value) * ease
  })
  return null
}
