// SPDX-License-Identifier: AGPL-3.0-or-later
// The tab bar: a glass pill floating over the page, with web's `Segmented`
// thumb sliding between the tabs (its leading edge springs ahead, the
// trailing one catches up). Slid along, it scrubs through the pages, ticking
// at each tab's middle; flicked sideways, it moves a tab over. The thumb is a
// lens: whatever's under it is drawn focused, and while held, magnified.

import { Canvas, RoundedRect } from '@shopify/react-native-skia'
import type { NavigationHelpers, ParamListBase, TabNavigationState } from 'expo-router/react-navigation'
import { type ReactNode, useCallback, useMemo, useRef } from 'react'
import { Pressable, StyleSheet, Text, View } from 'react-native'
import { Gesture, GestureDetector } from 'react-native-gesture-handler'
import Animated, {
  clamp,
  type SharedValue,
  useAnimatedReaction,
  useAnimatedStyle,
  useDerivedValue,
  useReducedMotion,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { Glass } from '../effects/Glass'
import { lead, trail, useMotion } from '../effects/motion'
import { useTheme } from '../theme/ThemeProvider'
import { withAlpha } from '../theme/materials'
import type { Pager, TabEvents, TabOptions } from './Tabs'

const HEIGHT = 64
const PAD = 6
const GAP = 10
/** How long a finger rests on a tab to long-press it. */
const LONG = 500
/** How fast (in tabs a second) a drag let go of goes on to the next tab. */
const FLICK = 3

/** How much of a page's bottom the bar covers. */
export function useTabBarSpace() {
  return useSafeAreaInsets().bottom + GAP + HEIGHT
}

/** How much bigger the lens shows the tab under it while a finger holds it. */
const MAGNIFY = 0.14

/** Stiff enough to keep up with a finger, soft enough to smooth its jitter (a tab's width is a page's). */
const FOLLOW = { stiffness: 420, damping: 42, mass: 1 }

type Props = {
  state: TabNavigationState<ParamListBase>
  descriptors: Record<string, { options: TabOptions }>
  navigation: NavigationHelpers<ParamListBase, TabEvents>
  pager: Pager
}

export function TabBar({ state, descriptors, navigation, pager }: Props) {
  const insets = useSafeAreaInsets()
  const { tokens, material } = useTheme()
  const reduced = useReducedMotion()
  const count = state.routes.length
  const { pos, to, drag, grab, land, load, glide } = pager

  const latest = useRef({ state, descriptors, navigation })
  latest.current = { state, descriptors, navigation }
  const press = useCallback(
    (i: number) => {
      const { state, navigation } = latest.current
      const route = state.routes[i]
      const event = navigation.emit({ type: 'tabPress', target: route.key, canPreventDefault: true })
      if (i === to.value || event.defaultPrevented) return
      glide(i)
      load(i)
      navigation.navigate(route.name, route.params)
    },
    [to, glide, load],
  )
  const longPress = useCallback((i: number) => {
    const { state, descriptors } = latest.current
    descriptors[state.routes[i].key].options.tabBarOnLongPress?.()
  }, [])

  // The thumb's edges, in tabs from the row's left and right. A finger moves them with the pages.
  const width = useSharedValue(0)
  const left = useSharedValue(state.index)
  const right = useSharedValue(count - 1 - state.index)
  useAnimatedReaction(
    () => ({ at: drag.value ? pos.value : -1, to: to.value }),
    (now, was) => {
      if (now.at >= 0) {
        left.value = now.at
        right.value = count - 1 - now.at
        return
      }
      if (was && was.at < 0 && was.to === now.to) return
      const [l, r] = [now.to, count - 1 - now.to]
      const dir = Math.sign(now.to - (left.value + count - 1 - right.value) / 2)
      if (reduced || !was || !dir) {
        left.value = l
        right.value = r
      } else {
        left.value = dir > 0 ? trail(l) : lead(l)
        right.value = dir > 0 ? lead(r) : trail(r)
      }
    },
    [count, reduced],
  )
  // Drawn rather than laid out: a view resized every frame keeps its corners a frame behind, square.
  // The leading edge's overshoot stops at the row's ends, where the canvas would cut it off flat.
  const x = useDerivedValue(() => Math.max(0, left.value) * width.value)
  const span = useDerivedValue(() => (count - Math.max(0, right.value)) * width.value - x.value)
  const clip = useAnimatedStyle(() => ({ left: x.value, width: span.value }))
  const shift = useAnimatedStyle(() => ({ left: -x.value, width: count * width.value }))

  // The lens's middle, in tabs, and how far it's lifted off the bar.
  const lens = useDerivedValue(() => (left.value + count - right.value) / 2 - 0.5)
  const lift = useSharedValue(0)
  useAnimatedReaction(
    () => drag.value,
    (now) => {
      if (reduced) return
      lift.value = withSpring(now ? 1 : 0, { stiffness: 380, damping: 30, mass: 1 })
    },
    [reduced],
  )
  const pressed = useSharedValue(-1)
  const squeeze = useSharedValue(1)

  const loadAll = useCallback(() => latest.current.state.routes.forEach((_, i) => load(i)), [load])
  const holds = state.routes.map((r) => !!descriptors[r.key].options.tabBarOnLongPress).join()
  const holdable = useMemo(() => holds.split(',').map((h) => h === 'true'), [holds])

  // One drag both scrubs and flicks: the pages follow the finger from its first move, and a fast
  // release goes on to the next tab. A tab with something to long-press takes a rest instead.
  const start = useSharedValue(0)
  const at = useSharedValue(0)
  const held = useSharedValue(false)
  const gesture = useMemo(() => {
    const slide = Gesture.Pan()
      .activeOffsetX([-8, 8])
      .failOffsetY([-16, 16])
      .onStart(() => {
        if (held.value) return
        start.value = grab()
        at.value = start.value
        scheduleOnRN(loadAll)
      })
      .onUpdate((e) => {
        if (!drag.value) return
        const p = clamp(start.value + e.translationX / width.value, 0, count - 1)
        if (Math.floor(p) !== Math.floor(at.value)) scheduleOnRN(haptic, 'tick')
        at.value = p
        pos.value = reduced ? p : withSpring(p, FOLLOW)
      })
      .onEnd((e) => {
        if (!drag.value) return
        const v = e.velocityX / width.value
        const p = at.value
        const fast = Math.abs(v) > FLICK
        const i = fast ? (v > 0 ? Math.floor(p) + 1 : Math.ceil(p) - 1) : Math.round(p)
        if (i < 0 || i > count - 1) scheduleOnRN(haptic, 'edge')
        land(clamp(i, 0, count - 1), v, fast)
      })
      .onFinalize(() => {
        if (drag.value) land(to.value)
      })
    const rest = Gesture.LongPress()
      .minDuration(LONG)
      .maxDistance(10)
      .onBegin(() => {
        held.value = false
      })
      .onStart((e) => {
        const i = Math.round(clamp(e.x / width.value - 0.5, 0, count - 1))
        if (!holdable[i]) return
        held.value = true
        scheduleOnRN(longPress, i)
      })
    return Gesture.Simultaneous(slide, rest)
  }, [count, reduced, holdable, pos, to, drag, grab, land, loadAll, longPress, start, at, held, width])

  return (
    <View pointerEvents="box-none" style={{ position: 'absolute', left: 16, right: 16, bottom: insets.bottom + GAP }}>
      <Glass radius={24} style={{ height: HEIGHT, padding: PAD }}>
        <GestureDetector gesture={gesture}>
          <View className="flex-1 flex-row" onLayout={(e) => (width.value = e.nativeEvent.layout.width / count)}>
            <Canvas style={StyleSheet.absoluteFill} pointerEvents="none">
              <RoundedRect x={x} y={0} width={span} height={HEIGHT - PAD * 2} r={Math.round((24 - PAD) * material.corners.scale)} color={withAlpha(tokens.press, 1)} />
            </Canvas>
            {state.routes.map((route, i) => {
              const { options } = descriptors[route.key]
              const focused = i === state.index
              const label = options.title ?? route.name
              return (
                <Pressable
                  key={route.key}
                  className="flex-1"
                  accessibilityRole="tab"
                  accessibilityLabel={label}
                  accessibilityState={{ selected: focused }}
                  onPress={() => press(i)}
                  onPressIn={() => {
                    pressed.value = i
                    squeeze.value = withTiming(0.96, { duration: 200 })
                  }}
                  onPressOut={() => (squeeze.value = withTiming(1, { duration: 200 }))}
                >
                  <Face
                    index={i}
                    label={label}
                    icon={options.tabBarIcon?.({ focused, color: tokens['ink-2'], size: 24 })}
                    badge={options.tabBarBadge}
                    color={tokens['ink-2']}
                    lens={lens}
                    lift={lift}
                    pressed={pressed}
                    squeeze={squeeze}
                  />
                </Pressable>
              )
            })}
            <Animated.View pointerEvents="none" importantForAccessibility="no-hide-descendants" style={[styles.clip, clip]}>
              <Animated.View className="flex-row" style={[styles.clip, shift]}>
                {state.routes.map((route, i) => {
                  const { options } = descriptors[route.key]
                  return (
                    <View key={route.key} className="flex-1">
                      <Face
                        index={i}
                        label={options.title ?? route.name}
                        icon={options.tabBarIcon?.({ focused: true, color: tokens.ink, size: 24 })}
                        badge={options.tabBarBadge}
                        color={tokens.ink}
                        lens={lens}
                        lift={lift}
                        pressed={pressed}
                        squeeze={squeeze}
                      />
                    </View>
                  )
                })}
              </Animated.View>
            </Animated.View>
          </View>
        </GestureDetector>
      </Glass>
    </View>
  )
}

const styles = StyleSheet.create({ clip: { position: 'absolute', top: 0, bottom: 0, overflow: 'hidden' } })

/** A tab's icon and label, grown as the lens passes over it and shrunk while pressed. */
function Face({
  index,
  label,
  icon,
  badge,
  color,
  lens,
  lift,
  pressed,
  squeeze,
}: {
  index: number
  label: string
  icon: ReactNode
  badge?: number | boolean
  color: string
  lens: SharedValue<number>
  lift: SharedValue<number>
  pressed: SharedValue<number>
  squeeze: SharedValue<number>
}) {
  const motion = useMotion()
  const style = useAnimatedStyle(() => {
    const grow = 1 + MAGNIFY * lift.value * Math.max(0, 1 - Math.abs(lens.value - index))
    return { transform: [{ scale: grow * (pressed.value === index ? squeeze.value : 1) }] }
  })
  return (
    <Animated.View className="flex-1 items-center justify-center gap-0.5" style={style}>
      <View>
        {icon}
        {!!badge && (
          <Animated.View
            key={String(badge)}
            style={motion.pop}
            className={`absolute -right-2 -top-1 items-center justify-center rounded-full bg-ink ${badge === true ? 'h-2 w-2' : 'h-4 min-w-4 px-1'}`}
          >
            {badge !== true && (
              <Text className="font-sans text-[10px] font-semibold text-canvas" style={{ lineHeight: 12 }}>
                {badge > 99 ? '99+' : badge}
              </Text>
            )}
          </Animated.View>
        )}
      </View>
      <Text className="font-sans text-2xs font-medium" style={{ color }} numberOfLines={1}>
        {label}
      </Text>
    </Animated.View>
  )
}
