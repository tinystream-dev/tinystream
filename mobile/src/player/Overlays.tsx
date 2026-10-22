// SPDX-License-Identifier: AGPL-3.0-or-later
// What floats over the picture (web's Player.tsx has them all): the chrome's
// buttons, skipping a chapter, what's up next, the end of a show, the paused
// title, a screenshot being taken, why a video can't play, and the player's
// own sheet for its menus.

import { clock } from '@tinystream/shared/format'
import { airs, episodeCode } from '@tinystream/shared/downloads'
import { ArrowLeft, Camera, Play } from 'lucide-react-native'
import { type ReactNode, useEffect } from 'react'
import { BackHandler, Pressable, ScrollView, StyleSheet, Text, View, useWindowDimensions } from 'react-native'
import Animated from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { haptic } from '../../modules/haptics'
import { Img } from '../components/Img'
import type { SeriesGlimpse } from '../components/SeriesPanel'
import { Shade } from '../components/media'
import { Spinner } from '../components/ui'
import { Squircle } from '../effects/Squircle'
import { useMotion } from '../effects/motion'
import type { ClipFieldsFragment } from '../gql/graphql'
import { useTheme } from '../theme/ThemeProvider'
import type { Playback } from './plan'

export function ChromeButton({
  label,
  onPress,
  children,
  active,
  disabled,
  size = 44,
}: {
  label: string
  onPress: () => void
  children: ReactNode
  active?: boolean
  disabled?: boolean
  size?: number
}) {
  return (
    <Pressable
      accessibilityLabel={label}
      accessibilityRole="button"
      hitSlop={4}
      disabled={disabled}
      onPress={() => {
        haptic('press')
        onPress()
      }}
      style={{ opacity: disabled ? 0.4 : 1 }}
    >
      {({ pressed }) => (
        <Squircle
          radius={size / 4.4}
          className={pressed || active ? 'bg-media-ink/10' : ''}
          style={{ width: size, height: size, alignItems: 'center', justifyContent: 'center' }}
        >
          {children}
        </Squircle>
      )}
    </Pressable>
  )
}

export function SkipPill({ children, onPress }: { children: ReactNode; onPress: () => void }) {
  const motion = useMotion()
  return (
    <Animated.View style={[motion.pop, { boxShadow: '0px 18px 48px -10px rgba(0, 0, 0, 0.45)', borderRadius: 14 }]}>
      <Pressable
        onPress={() => {
          haptic('press')
          onPress()
        }}
      >
        {({ pressed }) => (
          <Squircle radius={14} edge className={pressed ? 'bg-media-ink/20' : 'bg-media-panel/80'} style={{ height: 44, paddingHorizontal: 16, justifyContent: 'center' }}>
            <Text className="font-sans text-sm font-medium text-media-ink">{children}</Text>
          </Squircle>
        )}
      </Pressable>
    </Animated.View>
  )
}

export function UpNext({
  next,
  countdown,
  onPlay,
}: {
  next: NonNullable<Playback['next']>
  countdown: number | null
  /** Missing when someone else decides. */
  onPlay?: () => void
}) {
  const motion = useMotion()
  const { tokens } = useTheme()
  return (
    <Animated.View style={[motion.pop, { boxShadow: '0px 18px 48px -10px rgba(0, 0, 0, 0.45)', borderRadius: 16 }]}>
      <Squircle radius={16} edge className="bg-media-panel/90" style={{ width: 300, padding: 10 }}>
        <View className="flex-row gap-3">
          <Squircle radius={9} className="bg-panel" style={{ width: 104, aspectRatio: 16 / 9 }}>
            <Img src={next.still} contentFit="cover" style={StyleSheet.absoluteFill} />
          </Squircle>
          <View className="min-w-0 flex-1 py-0.5">
            <Text className="font-sans text-xs text-media-ink/55" numberOfLines={1}>
              Up next · {next.label}
            </Text>
            <Text className="font-sans mt-0.5 text-sm font-medium leading-5 text-media-ink" numberOfLines={2}>
              {next.name ?? next.label}
            </Text>
          </View>
        </View>
        {onPlay && (
          <Pressable
            onPress={() => {
              haptic('press')
              onPlay()
            }}
          >
            <Squircle radius={10} className="bg-accent/85" style={{ marginTop: 10, height: 38, alignItems: 'center', justifyContent: 'center' }}>
              {/* The countdown, as a fill sweeping across. */}
              {countdown !== null && (
                <Animated.View style={[StyleSheet.absoluteFill, { backgroundColor: tokens['accent-hover'], transformOrigin: 'left' }, motion.sweep(5000)]} />
              )}
              <View className="flex-row items-center gap-2">
                <Play size={16} color={tokens['on-accent']} fill={tokens['on-accent']} />
                <Text className="font-sans text-sm font-medium text-on-accent">{countdown !== null ? 'Playing next' : 'Play now'}</Text>
              </View>
            </Squircle>
          </Pressable>
        )}
      </Squircle>
    </Animated.View>
  )
}

/** Shown after the newest episode: you're caught up, and when the next one lands. */
export function Finale({ pb, series, onBack }: { pb: Playback; series: SeriesGlimpse | null; onBack: () => void }) {
  const motion = useMotion()
  const { tokens } = useTheme()
  const n = series?.next
  return (
    <Animated.View className="items-center justify-center bg-media-shade/60 p-6" style={[StyleSheet.absoluteFill, motion.fade]}>
      <Animated.View style={[{ maxWidth: 440, alignItems: 'center' }, motion.rise]}>
        <Text className="font-sans text-xs font-medium uppercase tracking-wide text-media-ink/60">{n?.airAt ? 'You’re all caught up' : 'That’s the last one'}</Text>
        <Text className="font-sans mt-2 text-center text-[28px] font-semibold leading-tight tracking-tight text-media-ink">{pb.title.name}</Text>
        <Text className="font-sans mt-2 text-center text-[15px] leading-6 text-media-ink/70">
          {n?.airAt
            ? `${episodeCode(n.season, n.episode)}${n.name ? ` “${n.name}”` : ''} airs ${airs(n.airAt).replace(/^(Today|Tomorrow)/, (w) => w.toLowerCase())}.`
            : series?.status === 'finished'
              ? 'You’ve finished the whole show.'
              : `You’ve watched every episode there is${pb.label ? `, up to ${pb.label}` : ''}.`}
        </Text>
        <Pressable
          onPress={() => {
            haptic('press')
            onBack()
          }}
        >
          <Squircle radius={14} className="bg-media-ink" style={{ marginTop: 24, height: 44, paddingHorizontal: 20, flexDirection: 'row', alignItems: 'center', gap: 8 }}>
            <ArrowLeft size={18} color={tokens['media-shade']} />
            <Text className="font-sans text-[15px] font-medium text-media-shade">Back to the show</Text>
          </Squircle>
        </Pressable>
      </Animated.View>
    </Animated.View>
  )
}

/** Paused for a while: what you're watching, big, over the receded frame. */
export function Resting({
  pb,
  line,
  overview,
  time,
  duration,
}: {
  pb: Playback
  line: string | null
  overview: string | null
  time: number
  duration: number
}) {
  const motion = useMotion()
  const insets = useSafeAreaInsets()
  return (
    <Animated.View pointerEvents="none" style={[StyleSheet.absoluteFill, { justifyContent: 'center', paddingLeft: Math.max(insets.left, 0) + 40, paddingRight: 40 }, motion.fade]}>
      <Shade to="right" stops={[0.7, 0.3, 0]} media />
      <Animated.View style={[{ maxWidth: 520 }, motion.developIn()]}>
        <Text className="font-sans text-xs font-medium uppercase tracking-[2px] text-media-ink/50">Paused</Text>
        <Text className="font-sans mt-3 text-[34px] font-semibold leading-[38px] tracking-tight text-media-ink" numberOfLines={3}>
          {pb.title.name}
        </Text>
        {line && <Text className="font-sans mt-2.5 text-lg text-media-ink/75">{line}</Text>}
        {overview && (
          <Text className="font-sans mt-4 text-[15px] leading-6 text-media-ink/60" numberOfLines={4}>
            {overview}
          </Text>
        )}
        {duration > 0 && (
          <Text className="font-sans mt-6 text-sm text-media-ink/45" style={{ fontVariant: ['tabular-nums'] }}>
            {clock(time)} of {clock(duration)}
          </Text>
        )}
      </Animated.View>
    </Animated.View>
  )
}

export type Shot = { key: number; clip?: ClipFieldsFragment; error?: string }

/** A screenshot being taken: the picture flashes like a shutter, then what was saved slides into the corner. */
export function ShotCard({ shot, onPress, style }: { shot: Shot; onPress: () => void; style?: object }) {
  const motion = useMotion()
  const { tokens } = useTheme()
  const { clip, error } = shot
  return (
    <>
      <Animated.View key={shot.key} pointerEvents="none" className="bg-media-ink" style={[StyleSheet.absoluteFill, motion.shutter]} />
      <Animated.View style={[{ position: 'absolute', width: 220 }, style, motion.rise]}>
        <Squircle radius={14} edge className="bg-float" style={{ overflow: 'hidden' }}>
          {error ? (
            <View className="p-3">
              <Text className="font-sans text-sm font-medium text-ink">Couldn’t take a screenshot</Text>
              <Text className="font-sans mt-0.5 text-xs text-ink-2" numberOfLines={3}>
                {error}
              </Text>
            </View>
          ) : (
            <Pressable disabled={!clip} onPress={onPress}>
              <View className="items-center justify-center bg-media-shade" style={{ aspectRatio: 16 / 9 }}>
                {clip?.poster ? <Img src={clip.poster} contentFit="cover" style={StyleSheet.absoluteFill} /> : <Spinner />}
              </View>
              <View className="flex-row items-center gap-2 px-3 py-2">
                <Camera size={14} color={tokens.highlight} />
                <Text className="font-sans text-[13px] font-medium text-ink">{clip ? 'Saved to Clips' : 'Taking a screenshot…'}</Text>
              </View>
            </Pressable>
          )}
        </Squircle>
      </Animated.View>
    </>
  )
}

/** Why the video can't play, with what might help. */
export function Failure({ message, onRetry, onConvert }: { message: string; onRetry: () => void; onConvert: () => void }) {
  const motion = useMotion()
  return (
    <View className="items-center justify-center p-6" style={StyleSheet.absoluteFill}>
      <Animated.View style={[{ maxWidth: 420, width: '100%' }, motion.pop]}>
        <Squircle radius={20} edge className="bg-float" style={{ padding: 20 }}>
          <Text className="font-sans text-[15px] font-medium text-ink">This video can't play right now</Text>
          <Text className="font-sans mt-1.5 text-sm leading-5 text-ink-2">{message}</Text>
          <View className="mt-4 flex-row items-center gap-2">
            <Pressable onPress={onRetry} hitSlop={8}>
              <Text className="font-sans text-sm text-ink underline">Try again</Text>
            </Pressable>
            <Text className="font-sans text-sm text-ink-3">or</Text>
            <Pressable onPress={onConvert} hitSlop={8}>
              <Text className="font-sans text-sm text-ink underline">convert it to 720p</Text>
            </Pressable>
          </View>
        </Squircle>
      </Animated.View>
    </View>
  )
}

/**
 * The player's menus: a panel over the picture, from the side in landscape
 * and from the bottom in portrait. It's part of the player rather than a
 * Modal, which would bring the system bars back.
 */
export function PlayerSheet({ open, onClose, children }: { open: boolean; onClose: () => void; children: ReactNode }) {
  const { width, height } = useWindowDimensions()
  const insets = useSafeAreaInsets()
  const motion = useMotion()
  const landscape = width > height

  useEffect(() => {
    if (!open) return
    const sub = BackHandler.addEventListener('hardwareBackPress', () => (onClose(), true))
    return () => sub.remove()
  }, [open, onClose])

  if (!open) return null
  return (
    <View style={StyleSheet.absoluteFill}>
      <Pressable style={StyleSheet.absoluteFill} onPress={onClose} accessibilityLabel="Close" />
      <Animated.View
        style={[
          landscape
            ? { position: 'absolute', top: 12, bottom: 12, right: Math.max(insets.right, 12), width: Math.min(400, width * 0.5) }
            : { position: 'absolute', left: 8, right: 8, bottom: Math.max(insets.bottom, 8), maxHeight: height * 0.65 },
          { boxShadow: '0px 18px 48px -10px rgba(0, 0, 0, 0.5)', borderRadius: 22 },
          motion.rise,
        ]}
      >
        <Squircle radius={22} edge className="bg-float" style={landscape ? { flex: 1 } : { maxHeight: height * 0.65 }}>
          <ScrollView contentContainerStyle={{ padding: 8 }} showsVerticalScrollIndicator={false}>
            {children}
          </ScrollView>
        </Squircle>
      </Animated.View>
    </View>
  )
}

/** A heading in the player's sheet. */
export function SheetHeading({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <View className="flex-row items-center justify-between px-2.5 pb-1 pt-2">
      <Text className="font-sans text-xs text-ink-3">{children}</Text>
      {aside}
    </View>
  )
}

/** A choice in the player's sheet, checked when it's the one in use. */
export function SheetItem({ label, hint, active, disabled, onPress }: { label: string; hint?: string; active?: boolean; disabled?: boolean; onPress: () => void }) {
  const { tokens } = useTheme()
  return (
    <Pressable
      disabled={disabled}
      onPress={() => {
        haptic('press')
        onPress()
      }}
      style={{ opacity: disabled ? 0.4 : 1 }}
    >
      {({ pressed }) => (
        <Squircle radius={12} className={pressed ? 'bg-press' : active ? 'bg-raised' : ''} style={{ flexDirection: 'row', alignItems: 'center', paddingHorizontal: 12, paddingVertical: 11, gap: 10 }}>
          <View style={{ width: 6, height: 6, borderRadius: 3, backgroundColor: active ? tokens.accent : 'transparent' }} />
          <Text className="font-sans flex-1 text-[15px] text-ink" numberOfLines={1}>
            {label}
          </Text>
          {hint && <Text className="font-sans text-xs text-ink-3">{hint}</Text>}
        </Squircle>
      )}
    </Pressable>
  )
}
