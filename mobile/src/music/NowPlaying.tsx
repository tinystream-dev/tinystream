// SPDX-License-Identifier: AGPL-3.0-or-later
import { useQuery } from '@tanstack/react-query'
import { ChevronDown, GripVertical, Pause, Play, Repeat, Shuffle, SkipBack, SkipForward, Trash2 } from 'lucide-react-native'
import { useEffect, useRef, useState } from 'react'
import { FlatList, Modal, Pressable, Text, View, useWindowDimensions } from 'react-native'
import { Gesture, GestureDetector, GestureHandlerRootView } from 'react-native-gesture-handler'
import Animated, { useAnimatedStyle, useSharedValue, withSpring, withTiming, interpolate, useReducedMotion } from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { useTabBarSpace } from '../components/TabBar'
import { Glass } from '../effects/Glass'
import { IconButton, Segmented, ErrorText } from '../components/ui'
import { SwipeRow } from '../components/SwipeRow'
import { useApi } from '../session'
import { rgba } from '../lib/tint'
import { useTheme } from '../theme/ThemeProvider'
import { Artwork, StarButton, TrackRow } from './components'
import { duration, LyricsQuery, quality } from './api'
import { useMusic } from './context'
import { gainOf } from '@tinystream/shared/music-policy'
import type { Entry } from '@tinystream/shared/music-player'

export function MusicOverlay() {
  const player = useMusic(),
    s = player.usePlayer((s) => s),
    current = player.current(),
    [open, setOpen] = useState(false)
  const { tokens } = useTheme(),
    bottom = useTabBarSpace(),
    { music } = player
  const visible = !!current && !s.dismissed && !s.suspended
  useEffect(() => {
    if (!visible) setOpen(false)
  }, [visible])
  const slide = Gesture.Pan().onEnd((e) => {
    if (e.translationY < -35) scheduleOnRN(setOpen, true)
    else if (e.translationY > 35 && !s.playing) {
      scheduleOnRN(haptic, 'dismissThreshold')
      scheduleOnRN(music.dismiss)
    } else if (Math.abs(e.translationX) > 60) {
      scheduleOnRN(haptic, 'tick')
      scheduleOnRN(e.translationX < 0 ? music.next : music.previous)
    }
  })
  if (!current || !visible) return null
  const t = current.track
  return (
    <>
      <GestureDetector gesture={slide}>
        <View
          style={{
            position: 'absolute',
            left: 16,
            right: 16,
            bottom: bottom + 10,
          }}
        >
          <Glass
            radius={24}
            style={{
              backgroundColor: t.coverTint ? rgba(t.coverTint, 0.18) : undefined,
              padding: 10,
            }}
          >
            <Pressable onPress={() => setOpen(true)} className="flex-row items-center gap-3" accessibilityLabel="Open Now Playing">
              <Artwork src={t.cover} size={48} />
              <View className="flex-1">
                <Text className="font-sans text-sm font-medium text-ink" numberOfLines={1}>
                  {t.title}
                </Text>
                <Text className="font-sans text-xs text-ink-2" numberOfLines={1}>
                  {t.artist}
                </Text>
              </View>
              <IconButton label={s.playing ? 'Pause' : 'Play'} onPress={() => void music.toggle()}>
                {s.playing ? <Pause size={24} color={tokens.ink} /> : <Play size={24} color={tokens.ink} />}
              </IconButton>
              <IconButton label="Next" onPress={music.next}>
                <SkipForward size={22} color={tokens.ink} />
              </IconButton>
            </Pressable>
          </Glass>
        </View>
      </GestureDetector>
      <Modal visible={open} transparent animationType="none" onRequestClose={() => setOpen(false)} statusBarTranslucent navigationBarTranslucent>
        <GestureHandlerRootView style={{ flex: 1 }}>
          <Full close={() => setOpen(false)} />
        </GestureHandlerRootView>
      </Modal>
    </>
  )
}
function Full({ close }: { close: () => void }) {
  const player = useMusic(),
    s = player.usePlayer((s) => s),
    t = player.current()!.track,
    { tokens } = useTheme(),
    insets = useSafeAreaInsets(),
    { width, height } = useWindowDimensions(),
    bottom = useTabBarSpace(),
    art = Math.min(width - 80, height * 0.3)
  const [tab, setTab] = useState<'lyrics' | 'queue' | 'info'>('lyrics'),
    time = player.usePosition(),
    { music } = player
  const expansion = useSharedValue(0),
    offset = useSharedValue(0),
    reduced = useReducedMotion()
  useEffect(() => {
    expansion.value = reduced ? 1 : withSpring(1, { damping: 28, stiffness: 220 })
  }, [expansion, reduced])
  const style = useAnimatedStyle(() => ({
    position: 'absolute',
    left: interpolate(expansion.value, [0, 1], [16, 0]),
    right: interpolate(expansion.value, [0, 1], [16, 0]),
    top: interpolate(expansion.value, [0, 1], [height - bottom - 78, 0]),
    height: interpolate(expansion.value, [0, 1], [68, height]),
    borderRadius: interpolate(expansion.value, [0, 1], [24, 0]),
    overflow: 'hidden',
    transform: [{ translateY: offset.value }],
  }))
  const content = useAnimatedStyle(() => ({
    opacity: interpolate(expansion.value, [0, 0.4, 1], [0, 0, 1], 'clamp'),
  }))
  const artwork = useAnimatedStyle(() => ({
    position: 'absolute',
    left: interpolate(expansion.value, [0, 1], [10, (width - art) / 2]),
    top: interpolate(expansion.value, [0, 1], [10, insets.top + 44]),
    transformOrigin: 'top left',
    transform: [{ scale: interpolate(expansion.value, [0, 1], [48 / art, 1]) }],
    zIndex: 2,
  }))
  const collapse = () => {
    if (reduced) return close()
    offset.value = 0
    expansion.value = withTiming(0, { duration: 220 }, (finished) => {
      if (finished) scheduleOnRN(close)
    })
  }
  const swipe = Gesture.Pan()
    .activeOffsetY(20)
    .onUpdate((e) => {
      offset.value = Math.max(0, e.translationY)
    })
    .onEnd((e) => {
      if (e.translationY > 100) {
        scheduleOnRN(haptic, 'dismissThreshold')
        scheduleOnRN(collapse)
      } else offset.value = withSpring(0)
    })
  return (
    <Animated.View
      style={[
        {
          flex: 1,
          backgroundColor: tokens.canvas,
          paddingTop: insets.top,
          paddingBottom: insets.bottom,
        },
        style,
      ]}
    >
      {t.coverTint && (
        <View
          pointerEvents="none"
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            right: 0,
            height: height * 0.65,
            backgroundColor: rgba(t.coverTint, 0.13),
          }}
        />
      )}
      <Animated.View style={[{ flex: 1 }, content]}>
        <GestureDetector gesture={swipe}>
          <View className="flex-row items-center justify-between px-5">
            <Text className="font-sans text-sm text-ink-2">Now playing</Text>
            <IconButton label="Collapse" onPress={collapse}>
              <ChevronDown size={24} color={tokens.ink} />
            </IconButton>
          </View>
        </GestureDetector>
        <View className="items-center gap-3 px-6">
          <View style={{ width: art, height: art }} />
          <Text className="font-sans mt-2 text-center text-xl font-semibold text-ink" numberOfLines={2}>
            {t.title}
          </Text>
          <Text className="font-sans text-sm text-ink-2">
            {t.artist} · {t.album}
          </Text>
          <Scrubber duration={t.duration} time={time} seek={music.seek} />
          <View className="flex-row items-center justify-between self-stretch">
            <IconButton label="Shuffle" onPress={() => music.shuffle()}>
              <Shuffle size={22} color={s.shuffled ? tokens.accent : tokens['ink-2']} />
            </IconButton>
            <IconButton label="Previous" onPress={music.previous}>
              <SkipBack size={28} color={tokens.ink} />
            </IconButton>
            <IconButton label={s.playing ? 'Pause' : 'Play'} size={56} tone="raised" onPress={() => void music.toggle()}>
              {s.playing ? <Pause size={30} color={tokens.ink} /> : <Play size={30} color={tokens.ink} />}
            </IconButton>
            <IconButton label="Next" onPress={music.next}>
              <SkipForward size={28} color={tokens.ink} />
            </IconButton>
            <IconButton label={`Repeat ${s.repeat}`} onPress={() => music.repeat(s.repeat === 'OFF' ? 'ALL' : s.repeat === 'ALL' ? 'ONE' : 'OFF')}>
              <View>
                <Repeat size={22} color={s.repeat === 'OFF' ? tokens['ink-2'] : tokens.accent} />
                {s.repeat === 'ONE' && <Text className="font-sans absolute -right-1 -top-1 text-xs text-accent">1</Text>}
              </View>
            </IconButton>
          </View>
          {!!s.error && <ErrorText>{s.error}</ErrorText>}
          <Segmented
            value={tab}
            onChange={setTab}
            options={[
              { value: 'lyrics', label: 'Lyrics' },
              { value: 'queue', label: 'Queue' },
              { value: 'info', label: 'Info' },
            ]}
          />
        </View>
        <View className="flex-1 px-6 pt-3">
          {tab === 'lyrics' ? (
            <Lyrics id={t.id} time={time} seek={music.seek} />
          ) : tab === 'queue' ? (
            <FlatList
              data={s.queue}
              keyExtractor={(e) => e.uid}
              renderItem={({ item, index }) => <QueueRow entry={item} index={index} count={s.queue.length} />}
            />
          ) : (
            <View className="gap-3">
              <Text className="font-sans text-sm text-ink">
                {quality(t)} · {t.channels ?? 2} channels
              </Text>
              <Text className="font-sans text-sm text-ink-2">
                Output: {player.outputRate()} Hz · ReplayGain: {s.settings.gain} (
                {(20 * Math.log10(gainOf(s.index, s.queue, s.settings, s.shuffled))).toFixed(1)} dB)
              </Text>
              <Text className="font-sans text-sm text-ink-2">
                Source: {player.source()} · {t.bitrate ?? '—'} kbps
              </Text>
              <StarButton kind="TRACK" id={t.id} starred={t.starred} />
            </View>
          )}
        </View>
      </Animated.View>
      <Animated.View style={artwork}>
        <Artwork src={t.cover} size={art} gyro />
      </Animated.View>
    </Animated.View>
  )
}
function Scrubber({ time, duration: total, seek }: { time: number; duration: number; seek: (t: number) => void }) {
  const { tokens } = useTheme(),
    [width, setWidth] = useState(1),
    [draft, setDraft] = useState<number | null>(null),
    step = useSharedValue(-1)
  const update = (x: number) => setDraft(Math.max(0, Math.min(total, (x / width) * total)))
  const gesture = Gesture.Pan()
    .minDistance(0)
    .onBegin((e) => {
      scheduleOnRN(update, e.x)
    })
    .onUpdate((e) => {
      const next = Math.floor(Math.max(0, Math.min(total, (e.x / width) * total)) / 10)
      if (next !== step.value) {
        step.value = next
        scheduleOnRN(haptic, 'tick')
      }
      scheduleOnRN(update, e.x)
    })
    .onEnd((e) => {
      scheduleOnRN(seek, Math.max(0, Math.min(total, (e.x / width) * total)))
      scheduleOnRN(setDraft, null)
    })
  const at = draft ?? time
  return (
    <View className="self-stretch">
      <GestureDetector gesture={gesture}>
        <View
          accessibilityRole="adjustable"
          accessibilityLabel="Playback position"
          accessibilityValue={{
            min: 0,
            max: total,
            now: at,
            text: duration(at),
          }}
          accessibilityActions={[{ name: 'increment' }, { name: 'decrement' }]}
          onAccessibilityAction={(e) => seek(at + (e.nativeEvent.actionName === 'increment' ? 10 : -10))}
          style={{ height: 32, justifyContent: 'center' }}
          onLayout={(e) => setWidth(e.nativeEvent.layout.width)}
        >
          <View style={{ height: 5, borderRadius: 3, backgroundColor: tokens.line }}>
            <View
              style={{
                height: 5,
                width: `${Math.max(0, Math.min(100, (at / Math.max(1, total)) * 100))}%`,
                borderRadius: 3,
                backgroundColor: tokens.ink,
              }}
            />
          </View>
        </View>
      </GestureDetector>
      <View className="flex-row justify-between">
        <Text className="font-sans text-xs text-ink-3">{duration(at)}</Text>
        <Text className="font-sans text-xs text-ink-3">{duration(total)}</Text>
      </View>
    </View>
  )
}
function Lyrics({ id, time, seek }: { id: number; time: number; seek: (time: number) => void }) {
  const api = useApi(),
    ref = useRef<FlatList>(null)
  const { data } = useQuery({
    queryKey: ['music', 'lyrics', id],
    queryFn: async () => (await api.request(LyricsQuery, { trackId: id })).lyrics,
  })
  const index = data?.synced ? data.lines.findLastIndex((l) => l.start != null && l.start <= time * 1000 + 150) : -1
  useEffect(() => {
    if (index != null && index >= 0) ref.current?.scrollToIndex({ index, viewPosition: 0.3, animated: true })
  }, [index])
  return (
    <FlatList
      ref={ref}
      data={data?.lines ?? []}
      keyExtractor={(_, i) => String(i)}
      getItemLayout={(_, i) => ({ index: i, length: 64, offset: 64 * i })}
      renderItem={({ item, index: i }) => (
        <Pressable
          style={{ height: 64, justifyContent: 'center' }}
          onPress={() => {
            if (data?.synced && item.start != null) {
              haptic('tick')
              seek(item.start / 1000)
            }
          }}
        >
          <Text className={`font-sans text-lg font-medium ${i === index ? 'text-ink' : 'text-ink-3'}`} numberOfLines={2}>
            {item.text}
          </Text>
        </Pressable>
      )}
      ListEmptyComponent={<Text className="font-sans text-sm text-ink-3">No lyrics available</Text>}
    />
  )
}
function QueueRow({ entry, index, count }: { entry: Entry; index: number; count: number }) {
  const { music } = useMusic(),
    { tokens } = useTheme(),
    drag = useSharedValue(0),
    slot = useSharedValue(index)
  const gesture = Gesture.Pan()
    .activateAfterLongPress(250)
    .onUpdate((e) => {
      drag.value = e.translationY
      const next = Math.max(0, Math.min(count - 1, index + Math.round(e.translationY / 64)))
      if (next !== slot.value) {
        slot.value = next
        scheduleOnRN(haptic, 'tick')
      }
    })
    .onEnd(() => {
      scheduleOnRN(music.move, index, slot.value)
      drag.value = withSpring(0)
    })
  const style = useAnimatedStyle(() => ({
    transform: [{ translateY: drag.value }],
    zIndex: drag.value ? 2 : 0,
  }))
  return (
    <SwipeRow
      right={{
        label: 'Remove',
        icon: <Trash2 size={20} color={tokens.canvas} />,
        color: tokens.danger,
        run: () => music.remove(entry.uid),
      }}
    >
      <Animated.View
        style={[
          {
            flexDirection: 'row',
            height: 64,
            alignItems: 'center',
            backgroundColor: tokens.canvas,
          },
          style,
        ]}
      >
        <GestureDetector gesture={gesture}>
          <View style={{ paddingRight: 12, height: 64, justifyContent: 'center' }} accessibilityLabel="Drag to reorder">
            <GripVertical size={20} color={tokens['ink-3']} />
          </View>
        </GestureDetector>
        <View className="flex-1">
          <TrackRow track={entry.track} onPress={() => music.jumpTo(index)} />
        </View>
      </Animated.View>
    </SwipeRow>
  )
}
