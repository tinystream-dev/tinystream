// SPDX-License-Identifier: AGPL-3.0-or-later
// The tabs navigator. Its pages lie side by side in one strip, set at `pos`
// (in pages from the first), so they can follow a finger: dragged across the
// page, or scrubbed along the tab bar. A tab press glides the strip over,
// with only the page left and the one picked in it.

import { withLayoutContext } from 'expo-router'
import {
  type DefaultNavigatorOptions,
  type ParamListBase,
  type TabActionHelpers,
  type TabNavigationState,
  TabRouter,
  type TabRouterOptions,
  createNavigatorFactory,
  useNavigationBuilder,
} from 'expo-router/react-navigation'
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { View, useWindowDimensions } from 'react-native'
import { Gesture, GestureDetector } from 'react-native-gesture-handler'
import Animated, {
  Easing,
  type SharedValue,
  cancelAnimation,
  clamp,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { TabBar } from './TabBar'

export type TabOptions = {
  title?: string
  tabBarIcon?: (props: { focused: boolean; color: string; size: number }) => ReactNode
  /** What resting a finger on the tab does. Without it, a rest is just the start of a slide. */
  tabBarOnLongPress?: () => void
  /** A count on the tab's icon, or a dot for `true`. */
  tabBarBadge?: number | boolean
}

export type TabEvents = {
  tabPress: { data: undefined; canPreventDefault: true }
}

/** What moves the pages, shared with the tab bar. */
export type Pager = {
  /** Where the strip is, in pages. */
  pos: SharedValue<number>
  /** The page it's at or going to; while it's dragged, the one it was at. */
  to: SharedValue<number>
  /** A finger has the strip. */
  drag: SharedValue<boolean>
  /** Takes the strip from whatever's moving it, for a finger: where it is, in pages. */
  grab: () => number
  /** Lets go of the strip at page `i` (`velocity` in pages a second), and goes there. */
  land: (i: number, velocity?: number, feel?: boolean) => void
  /** Renders page `i`, before it's been gone to: it's being dragged into view. */
  load: (i: number) => void
  /** Glides the strip to page `i` (a tab press) straight away, rather than once the page has rendered. */
  glide: (i: number) => void
}

const glide = Easing.bezier(0.16, 1, 0.3, 1)
const SPRING = { stiffness: 260, damping: 30, mass: 1, overshootClamping: true }

type Props = DefaultNavigatorOptions<ParamListBase, string | undefined, TabNavigationState<ParamListBase>, TabOptions, TabEvents, unknown> &
  TabRouterOptions

function TabsNavigator({ id, initialRouteName, backBehavior, children, layout, screenListeners, screenOptions, screenLayout, UNSTABLE_router }: Props) {
  const { state, descriptors, navigation, NavigationContent } = useNavigationBuilder<
    TabNavigationState<ParamListBase>,
    TabRouterOptions,
    TabActionHelpers<ParamListBase>,
    TabOptions,
    TabEvents
  >(TabRouter, { id, initialRouteName, backBehavior, children, layout, screenListeners, screenOptions, screenLayout, UNSTABLE_router })
  const { width } = useWindowDimensions()
  const reduced = useReducedMotion()
  const count = state.routes.length

  const pos = useSharedValue(state.index)
  const to = useSharedValue(state.index)
  const drag = useSharedValue(false)
  // A tab press further than the next tab moves the page left into the slot beside the one picked.
  const stand = useSharedValue<{ page: number; slot: number } | null>(null)

  const focused = state.routes[state.index].key
  const [loaded, setLoaded] = useState(() => new Set([focused]))
  if (!loaded.has(focused)) setLoaded(new Set(loaded).add(focused))

  const latest = useRef({ state, navigation })
  latest.current = { state, navigation }
  const load = useCallback((i: number) => {
    const key = latest.current.state.routes[i]?.key
    if (key) setLoaded((l) => (l.has(key) ? l : new Set(l).add(key)))
  }, [])
  // Once the first page has settled, the others are rendered too, one at a time: going to one then only glides.
  const first = loaded.size === 1
  useEffect(() => {
    const next = state.routes.findIndex((r) => !loaded.has(r.key))
    if (next < 0) return
    const t = setTimeout(() => load(next), first ? 1500 : 500)
    return () => clearTimeout(t)
  }, [loaded, first, state.routes, load])

  const arrive = useCallback((i: number, feel: boolean) => {
    const { state, navigation } = latest.current
    const route = state.routes[i]
    if (!route || i === state.index) return
    if (feel) haptic('tab')
    navigation.navigate(route.name, route.params)
  }, [])

  const land = useCallback(
    (i: number, velocity = 0, feel = false) => {
      'worklet'
      const was = to.value
      to.value = i
      drag.value = false
      pos.value = reduced ? i : withSpring(i, { ...SPRING, velocity })
      if (i !== was) scheduleOnRN(arrive, i, feel)
    },
    [to, drag, pos, reduced, arrive],
  )

  const glideTo = useCallback(
    (i: number) => {
      const old = to.value
      if (old === i) return
      to.value = i
      if (reduced) {
        stand.value = null
        pos.value = i
        return
      }
      const dir = Math.sign(i - old)
      const slot = stand.value?.page === old ? stand.value.slot : old
      const from = i - dir + (pos.value - slot)
      stand.value = { page: old, slot: i - dir }
      pos.value = from
      pos.value = withTiming(i, { duration: 420, easing: glide }, (done) => {
        if (done) stand.value = null
      })
    },
    [to, pos, stand, reduced],
  )

  // Gone to some other way than by a finger or a tab press: Back, or a link.
  useEffect(() => glideTo(state.index), [state.index, glideTo])

  const grab = useCallback(() => {
    'worklet'
    cancelAnimation(pos)
    stand.value = null
    drag.value = true
    return pos.value
  }, [pos, stand, drag])

  const pager: Pager = { pos, to, drag, grab, land, load, glide: glideTo }

  // Past a tab's first screen, a sideways swipe is the page's own (or going back), not the tabs'.
  const deep = (state.routes[state.index].state?.index ?? 0) > 0
  const start = useSharedValue(0)
  const swipe = useMemo(
    () =>
      Gesture.Pan()
        .enabled(!deep)
        .activeOffsetX([-16, 16])
        .failOffsetY([-12, 12])
        .onStart(() => {
          start.value = grab()
          if (to.value > 0) scheduleOnRN(load, to.value - 1)
          if (to.value < count - 1) scheduleOnRN(load, to.value + 1)
        })
        .onUpdate((e) => {
          const p = clamp(start.value - e.translationX / width, to.value - 1, to.value + 1)
          // Past the first or last page, it gives a little, reluctantly.
          pos.value = p < 0 ? p / 3 : p > count - 1 ? count - 1 + (p - count + 1) / 3 : p
        })
        .onEnd((e) => {
          const v = -e.velocityX / width
          const near = Math.round(pos.value)
          const i = Math.abs(v) > 0.6 ? (v > 0 ? Math.floor(pos.value) + 1 : Math.ceil(pos.value) - 1) : near
          land(clamp(i, Math.max(0, to.value - 1), Math.min(count - 1, to.value + 1)), v, true)
        })
        .onFinalize((_, success) => {
          if (!success && drag.value) land(to.value)
        }),
    [deep, count, width, pos, to, drag, start, grab, land, load],
  )

  return (
    <NavigationContent>
      <GestureDetector gesture={swipe}>
        <View className="flex-1 overflow-hidden">
          {state.routes.map((route, i) =>
            loaded.has(route.key) ? (
              <Page key={route.key} index={i} pos={pos} stand={stand} width={width} focused={i === state.index}>
                {descriptors[route.key].render()}
              </Page>
            ) : null,
          )}
        </View>
      </GestureDetector>
      <TabBar state={state} descriptors={descriptors} navigation={navigation} pager={pager} />
    </NavigationContent>
  )
}

function Page({
  index,
  pos,
  stand,
  width,
  focused,
  children,
}: {
  index: number
  pos: SharedValue<number>
  stand: SharedValue<{ page: number; slot: number } | null>
  width: number
  focused: boolean
  children: ReactNode
}) {
  const place = useAnimatedStyle(() => {
    const s = stand.value
    const slot = s?.page === index ? s.slot : index
    const covered = s != null && s.slot === index && s.page !== index
    return { opacity: covered ? 0 : 1, transform: [{ translateX: (slot - pos.value) * width }] }
  })
  return (
    <Animated.View
      style={[{ position: 'absolute', top: 0, bottom: 0, left: 0, width }, place]}
      importantForAccessibility={focused ? 'auto' : 'no-hide-descendants'}
      pointerEvents={focused ? 'auto' : 'none'}
    >
      {children}
    </Animated.View>
  )
}

const { Navigator } = createNavigatorFactory(TabsNavigator)()

export const Tabs = withLayoutContext<TabOptions, typeof Navigator, TabNavigationState<ParamListBase>, TabEvents>(Navigator)
