// SPDX-License-Identifier: AGPL-3.0-or-later
// A screen in a tab's stack: a large title (or a hero) that scrolls away
// under a fade of the canvas, which then shows the title small, between
// floating glass buttons: back, when there's somewhere to go back to, and
// the screen's own; pull to refresh; and back to the top when the tab is
// tapped again. Content is a scroll view, or a list for long ones.

import { useScrollToTop } from 'expo-router/react-navigation'
import { useNavigation, useRouter } from 'expo-router'
import { ArrowLeft } from 'lucide-react-native'
import { type ReactNode, createContext, useCallback, useContext, useRef, useState } from 'react'
import { type FlatListProps, type LayoutChangeEvent, Pressable, Text, View, type ViewStyle } from 'react-native'
import { FlatList as GestureFlatList, Gesture, GestureDetector, ScrollView as GestureScrollView } from 'react-native-gesture-handler'
import Animated, {
  type SharedValue,
  interpolate,
  useAnimatedScrollHandler,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { Glass, ScreenBlurArea } from '../effects/Glass'
import { withAlpha } from '../theme/materials'
import { useTheme } from '../theme/ThemeProvider'
import { useMusicSpace } from '../music/context'
import { useTabBarSpace } from './TabBar'
import { Spinner } from './ui'

export const BAR = 56
const FADE = 32

const AnimatedScroll = Animated.createAnimatedComponent(GestureScrollView)
const AnimatedList = Animated.createAnimatedComponent(GestureFlatList) as unknown as typeof GestureFlatList

/** How far a pull has to go to refresh, and how far it can go at all. */
const PULL = 72
const PULL_MAX = 120
const STEPS = 6

const ScrollContext = createContext<SharedValue<number> | null>(null)

/** How far the page is scrolled, for parallax. */
export function usePageScroll() {
  return useContext(ScrollContext)
}

type Common = {
  title: string
  /** Shown big at the top until it scrolls away. Off for heroes and tab roots that have their own. */
  large?: boolean
  /** Edge-to-edge artwork at the top instead of the large title. */
  hero?: ReactNode
  /** Buttons at the bar's right. */
  right?: ReactNode
  /** Under the large title, above the content. */
  header?: ReactNode
  onRefresh?: () => Promise<unknown>
  /** Content padding at the sides. */
  inset?: number
  /** Kept under the bar while the page scrolls, `pinnedHeight` tall. */
  pinned?: ReactNode
  pinnedHeight?: number
  /** Floats above the tab bar, like a form's Save button. */
  footer?: ReactNode
  /** For scrolling the page from outside, e.g. to an item in its list. */
  scrollRef?: React.RefObject<unknown>
}

type ScrollPage = Common & { children?: ReactNode; list?: undefined }
type ListPage<T> = Common & { list: Omit<FlatListProps<T>, 'onScroll' | 'ListHeaderComponent'>; children?: undefined }

export function Page<T>(props: ScrollPage | ListPage<T>) {
  const { title, large = true, hero, right, header, onRefresh, inset = 20, pinned, pinnedHeight = 0, footer } = props
  const insets = useSafeAreaInsets()
  const navigation = useNavigation()
  const bottom = useTabBarSpace() + useMusicSpace()
  const y = useSharedValue(0)
  const [collapse, setCollapse] = useState(hero ? 240 : 48)
  const own = useRef<GestureScrollView & GestureFlatList<T>>(null)
  const ref = (props.scrollRef ?? own) as typeof own
  useScrollToTop(ref as never)
  const top = insets.top + BAR + (pinned ? pinnedHeight : 0)
  // Only within this stack: a tab's first screen has nothing to go back to, whatever the tabs did before.
  const back = (navigation.getState()?.index ?? 0) > 0

  const onScroll = useAnimatedScrollHandler((e) => {
    y.value = e.contentOffset.y
  })

  const pull = usePull(y, ref, onRefresh)

  const measured = (e: LayoutChangeEvent) => setCollapse(Math.max(24, e.nativeEvent.layout.height - (hero ? top : 8)))

  const head = (
    <View>
      {hero ? (
        <View onLayout={measured}>{hero}</View>
      ) : large ? (
        <View onLayout={measured} style={{ paddingHorizontal: inset, paddingTop: 4, paddingBottom: 12 }}>
          <Text className="font-sans text-3xl font-semibold tracking-tight text-ink" numberOfLines={2}>
            {title}
          </Text>
        </View>
      ) : null}
      {header && <View style={{ paddingHorizontal: inset }}>{header}</View>}
    </View>
  )

  const content = props.list ? (
    <AnimatedList
      ref={ref}
      {...props.list}
      onScroll={onScroll}
      scrollEventThrottle={16}
      ListHeaderComponent={head}
      contentContainerStyle={[{ paddingTop: hero ? 0 : top, paddingBottom: bottom + 32 }, props.list.contentContainerStyle]}
    />
  ) : (
    <AnimatedScroll
      ref={ref}
      onScroll={onScroll}
      scrollEventThrottle={16}
      keyboardShouldPersistTaps="handled"
      contentContainerStyle={{ paddingTop: hero ? 0 : top, paddingBottom: bottom + (footer ? 96 : 32) }}
    >
      {head}
      <View style={{ paddingHorizontal: inset, paddingTop: hero ? 20 : 0, gap: 24 }}>{props.children}</View>
    </AnimatedScroll>
  )

  return (
    <ScrollContext.Provider value={y}>
      <ScreenBlurArea style={{ flex: 1 }}>
        <GestureDetector gesture={pull.gesture}>{content}</GestureDetector>
      </ScreenBlurArea>
      {footer && (
        <View pointerEvents="box-none" style={{ position: 'absolute', left: 16, right: 16, bottom: bottom + 10 }}>
          {footer}
        </View>
      )}
      {onRefresh && <PullIndicator pulled={pull.pulled} refreshing={pull.refreshing} top={top} />}
      <Bar title={title} y={y} collapse={hero || large ? collapse : -100} top={insets.top} back={back} right={right} solidAt={hero ? collapse - 40 : pinned ? -100 : 4} pinned={pinned} pinnedHeight={pinnedHeight} />
    </ScrollContext.Provider>
  )
}

function Bar({
  title,
  y,
  collapse,
  top,
  back,
  right,
  solidAt,
  pinned,
  pinnedHeight = 0,
}: {
  title: string
  y: SharedValue<number>
  collapse: number
  top: number
  back: boolean
  right?: ReactNode
  solidAt: number
  pinned?: ReactNode
  pinnedHeight?: number
}) {
  const router = useRouter()
  const { tokens } = useTheme()
  const fade = useAnimatedStyle(() => ({ opacity: interpolate(y.value, [solidAt, solidAt + 24], [0, 1], 'clamp') }))
  const height = top + BAR + (pinned ? pinnedHeight : 0)
  const name = useAnimatedStyle(() => ({
    opacity: interpolate(y.value, [collapse - 12, collapse + 12], [0, 1], 'clamp'),
    transform: [{ translateY: interpolate(y.value, [collapse - 12, collapse + 12], [6, 0], 'clamp') }],
  }))
  return (
    <View pointerEvents="box-none" style={{ position: 'absolute', top: 0, left: 0, right: 0, height }}>
      <Animated.View pointerEvents="none" style={[{ position: 'absolute', top: 0, left: 0, right: 0 }, fade]}>
        <TopFade height={height} />
      </Animated.View>
      <View pointerEvents="box-none" style={{ marginTop: top, height: BAR, flexDirection: 'row', alignItems: 'center', paddingHorizontal: 12, gap: 4 }}>
        {back && (
          <Pressable
            accessibilityLabel="Back"
            hitSlop={8}
            onPress={() => {
              haptic('tick')
              router.back()
            }}
          >
            <Glass radius={20} style={{ width: 40, height: 40, alignItems: 'center', justifyContent: 'center' }}>
              <ArrowLeft size={20} color={tokens.ink} />
            </Glass>
          </Pressable>
        )}
        <Animated.View style={[{ flex: 1, paddingHorizontal: back ? 4 : 8 }, name]} pointerEvents="none">
          <Text className="font-sans text-[17px] font-semibold tracking-tight text-ink" numberOfLines={1}>
            {title}
          </Text>
        </Animated.View>
        {right && (
          <Glass radius={20} style={{ height: 40, flexDirection: 'row', alignItems: 'center' }}>
            {right}
          </Glass>
        )}
      </View>
      {pinned && <View style={{ height: pinnedHeight }}>{pinned}</View>}
    </View>
  )
}

/** The canvas under what floats at the top of a screen, `height` tall, then fading out so content scrolls away under it softly. */
export function TopFade({ height }: { height: number }) {
  const { tokens } = useTheme()
  return (
    <View pointerEvents="none">
      <View style={{ height, backgroundColor: tokens.canvas }} />
      <View style={{ height: FADE, experimental_backgroundImage: `linear-gradient(to bottom, ${tokens.canvas}, ${withAlpha(tokens.canvas, 0)})` } as ViewStyle} />
    </View>
  )
}

/**
 * Pulling down at the top: ticks as it builds, a thud where letting go
 * refreshes, then a spinner until the refresh is done.
 */
function usePull(y: SharedValue<number>, scroll: React.RefObject<unknown>, onRefresh?: () => Promise<unknown>) {
  const pulled = useSharedValue(0)
  const from = useSharedValue<number | null>(null)
  const step = useSharedValue(0)
  const busy = useSharedValue(false)
  const [refreshing, setRefreshing] = useState(false)

  const refresh = useCallback(() => {
    if (!onRefresh) return
    setRefreshing(true)
    onRefresh()
      .catch(() => {})
      .finally(() => {
        setRefreshing(false)
        busy.value = false
        pulled.value = withTiming(0, { duration: 220 })
      })
  }, [onRefresh, busy, pulled])

  const gesture = Gesture.Pan()
    .enabled(!!onRefresh)
    .activeOffsetY(6)
    .failOffsetX([-20, 20])
    .simultaneousWithExternalGesture(scroll as never)
    .onBegin(() => {
      from.value = null
    })
    .onUpdate((e) => {
      if (busy.value) return
      if (from.value === null) {
        if (y.value > 1 || e.translationY <= 0) return
        from.value = e.translationY
      }
      const d = Math.max(0, e.translationY - from.value)
      const p = Math.min(PULL_MAX, d * 0.55)
      pulled.value = p
      const s = Math.min(STEPS, Math.floor((p / PULL) * STEPS))
      if (s !== step.value) {
        if (s > step.value) scheduleOnRN(haptic, s >= STEPS ? 'pullTrigger' : 'pullProgress', s / STEPS)
        step.value = s
      }
    })
    .onEnd(() => {
      if (busy.value) return
      if (pulled.value >= PULL) {
        busy.value = true
        pulled.value = withSpring(PULL * 0.75, { damping: 18 })
        scheduleOnRN(refresh)
      } else pulled.value = withTiming(0, { duration: 200 })
      step.value = 0
      from.value = null
    })

  return { gesture, pulled, refreshing }
}

function PullIndicator({ pulled, refreshing, top }: { pulled: SharedValue<number>; refreshing: boolean; top: number }) {
  const { tokens } = useTheme()
  const style = useAnimatedStyle(() => ({
    opacity: interpolate(pulled.value, [8, PULL * 0.6], [0, 1], 'clamp'),
    transform: [{ translateY: pulled.value - 44 }, { rotate: `${(pulled.value / PULL) * 270}deg` }, { scale: interpolate(pulled.value, [0, PULL], [0.6, 1], 'clamp') }],
  }))
  return (
    <View pointerEvents="none" style={{ position: 'absolute', top: top + 8, left: 0, right: 0, alignItems: 'center' }}>
      <Animated.View style={style}>
        <Glass radius={18} style={{ width: 36, height: 36, alignItems: 'center', justifyContent: 'center' }}>
          {refreshing ? <Spinner /> : <View style={{ width: 14, height: 14, borderRadius: 7, borderWidth: 2, borderColor: tokens['ink-2'], borderTopColor: 'transparent' }} />}
        </Glass>
      </Animated.View>
    </View>
  )
}

/** A section with a heading and, at its right, a link like "See all" (web's Section). */
export function Section({ title, aside, children, inset = 0 }: { title: ReactNode; aside?: ReactNode; children: ReactNode; inset?: number }) {
  return (
    <View>
      <View className="mb-3 flex-row items-baseline gap-3" style={{ paddingHorizontal: inset }}>
        {typeof title === 'string' ? <Text className="font-sans flex-1 text-[17px] font-semibold tracking-tight text-ink">{title}</Text> : <View className="flex-1">{title}</View>}
        {aside}
      </View>
      {children}
    </View>
  )
}

/** The small link at a section's right. */
export function SectionLink({ label, onPress }: { label: string; onPress: () => void }) {
  return (
    <Pressable
      hitSlop={10}
      onPress={() => {
        haptic('tick')
        onPress()
      }}
    >
      <Text className="font-sans text-[13px] text-ink-3">{label}</Text>
    </Pressable>
  )
}
