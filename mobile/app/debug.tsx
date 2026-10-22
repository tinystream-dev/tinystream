// SPDX-License-Identifier: AGPL-3.0-or-later
// The debug screen: every effect and haptic the screens are built from, to
// try them out on a device, and versions to pretend to be, to see the
// mismatch banner and the update sheet.

import { Canvas, LinearGradient, Rect, vec } from '@shopify/react-native-skia'
import type { ComponentStyle } from '@tinystream/shared/theme'
import Constants from 'expo-constants'
import { type ReactNode, useState } from 'react'
import { Pressable, ScrollView, StyleSheet, Switch, Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { nativeVersion } from '../modules/about'
import { type HapticEvent, type HapticIntensity, haptic, hapticIntensity, setHapticIntensity } from '../modules/haptics'
import { BlurArea, Glass, GlassScope } from '../src/effects/Glass'
import { Bone } from '../src/effects/Shimmer'
import { Squircle } from '../src/effects/Squircle'
import { Ticker } from '../src/effects/Ticker'
import { Tilt } from '../src/effects/Tilt'
import { type Motions, useMotion } from '../src/effects/motion'
import { Field, Input } from '../src/components/ui'
import { useTheme } from '../src/theme/ThemeProvider'
import { checkForUpdates, fakeVersions } from '../src/updates'

const EVENTS: HapticEvent[] = [
  'tick',
  'edge',
  'press',
  'longPressStart',
  'longPressOpen',
  'toggleOn',
  'toggleOff',
  'pullTrigger',
  'dismissThreshold',
  'tab',
  'reveal',
  'texture',
  'success',
  'error',
]

const MOTIONS = ['pop', 'fade', 'rise', 'sheet', 'pageIn', 'developIn', 'unfold', 'resolve', 'flash', 'hush'] as const

export default function Debug() {
  const insets = useSafeAreaInsets()
  const { tokens, look, setLook } = useTheme()
  const [count, setCount] = useState(7)

  return (
    <GlassScope>
      <BlurArea style={{ flex: 1 }}>
        <ScrollView contentContainerStyle={{ paddingTop: insets.top + 24, paddingBottom: insets.bottom + 120, paddingHorizontal: 20, gap: 28 }}>
          <View>
            <Text className="font-sans text-3xl font-semibold text-ink">Debug</Text>
            <Text className="font-sans mt-1 text-sm text-ink-2">
              app {Constants.expoConfig?.version} · rust {nativeVersion()}
            </Text>
          </View>

          <Section title="Versions">
            <Versions />
          </Section>

          <Section title="Style">
            <Segmented<ComponentStyle>
              value={look.style}
              options={['LAYERED', 'FLAT', 'GLASS']}
              onChange={(style) => setLook({ ...look, style })}
            />
          </Section>

          <Section title="Tilt and glare">
            <View className="flex-row gap-4">
              <Tilt style={{ width: 150, aspectRatio: 2 / 3 }} onPress={() => haptic('press')} onLongPress={() => {}}>
                <Artwork from={tokens.info} to={tokens.highlight} />
              </Tilt>
              <View className="flex-1 justify-center">
                <Text className="font-sans text-sm text-ink-2">Press it: the card leans toward your finger.</Text>
              </View>
            </View>
            <Tilt gyro max={5} radius={22} style={{ height: 180, marginTop: 16 }}>
              <Artwork from={tokens.social} to={tokens.ok} />
              <View className="absolute bottom-4 left-4">
                <Text className="font-sans text-xl font-semibold text-media-ink">Hero artwork</Text>
                <Text className="font-sans text-sm text-media-ink/70">Leans with the phone, too.</Text>
              </View>
            </Tilt>
          </Section>

          <Section title="Shimmer">
            <View className="flex-row gap-4">
              <Bone radius={14} style={{ width: 96, aspectRatio: 2 / 3 }} />
              <View className="flex-1 gap-2 pt-1">
                <Bone radius={6} style={{ height: 14, width: '75%' }} />
                <Bone radius={6} style={{ height: 10, width: '40%' }} />
                <Bone radius={6} style={{ height: 10, width: '55%' }} />
              </View>
            </View>
          </Section>

          <Section title="Ticker">
            <View className="flex-row items-center gap-4">
              <Ticker value={`${count} new`} className="font-sans text-2xl font-semibold text-ink" />
              <Button label="+1" onPress={() => setCount((c) => c + 1)} />
              <Button label="+9" onPress={() => setCount((c) => c + 9)} />
            </View>
          </Section>

          <Section title="Motion">
            <MotionDemo />
          </Section>

          <Section title="Haptics">
            <Intensity />
            <View className="mt-3 flex-row flex-wrap gap-2">
              {EVENTS.map((e) => (
                <Button key={e} label={e} onPress={() => haptic(e)} silent />
              ))}
              <PullDemo />
            </View>
          </Section>
        </ScrollView>
      </BlurArea>

      <View pointerEvents="box-none" style={[StyleSheet.absoluteFill, { justifyContent: 'flex-end', padding: 16, paddingBottom: insets.bottom + 16 }]}>
        <Glass style={{ padding: 16 }}>
          <Text className="font-sans font-medium text-ink">A glass panel</Text>
          <Text className="font-sans text-sm text-ink-2">Blurs what scrolls under it in the Glass style.</Text>
        </Glass>
      </View>
    </GlassScope>
  )
}

/** Pretend versions apply as they're typed; the update sheet looks again on the next start. */
function Versions() {
  const fake = fakeVersions.use()
  const check = checkForUpdates.use()
  const { tokens } = useTheme()
  const field = (key: keyof typeof fake, label: string, placeholder: string) => (
    <Field label={label}>
      <Input
        value={fake[key] ?? ''}
        placeholder={placeholder}
        autoCapitalize="none"
        autoCorrect={false}
        onChangeText={(v) => fakeVersions.set({ ...fakeVersions.get(), [key]: v.trim() || undefined })}
      />
    </Field>
  )
  return (
    <View className="gap-3">
      {field('app', 'This app', Constants.expoConfig?.version ?? '')}
      {field('server', 'The server', 'as it says')}
      {field('latest', 'Latest release', 'as GitHub says')}
      <View className="flex-row items-center justify-between">
        <Text className="font-sans text-sm text-ink">Check for updates</Text>
        <Switch
          value={check}
          trackColor={{ true: tokens.accent, false: tokens.press }}
          thumbColor={tokens.canvas}
          onValueChange={(on) => {
            haptic(on ? 'toggleOn' : 'toggleOff')
            checkForUpdates.set(on)
          }}
        />
      </View>
    </View>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <View>
      <Text className="font-sans mb-3 text-xs font-medium uppercase tracking-wider text-ink-3">{title}</Text>
      {children}
    </View>
  )
}

function Artwork({ from, to }: { from: string; to: string }) {
  return (
    <Canvas style={StyleSheet.absoluteFill}>
      <Rect x={0} y={0} width={400} height={400}>
        <LinearGradient start={vec(0, 0)} end={vec(220, 320)} colors={[from, to]} />
      </Rect>
    </Canvas>
  )
}

function Button({ label, onPress, silent = false }: { label: string; onPress: () => void; silent?: boolean }) {
  return (
    <Pressable
      onPress={() => {
        if (!silent) haptic('press')
        onPress()
      }}
    >
      {({ pressed }) => (
        <Squircle radius={10} edge className={pressed ? 'bg-press' : 'bg-raised'} style={{ paddingHorizontal: 12, paddingVertical: 8 }}>
          <Text className="font-sans text-sm font-medium text-ink">{label}</Text>
        </Squircle>
      )}
    </Pressable>
  )
}

function Segmented<T extends string>({ value, options, onChange }: { value: T; options: T[]; onChange: (v: T) => void }) {
  return (
    <Squircle radius={12} className="flex-row bg-raised p-1">
      {options.map((o) => (
        <Pressable
          key={o}
          className="flex-1"
          onPress={() => {
            haptic('tick')
            onChange(o)
          }}
        >
          <Squircle radius={9} className={o === value ? 'bg-float' : ''} edge={o === value} style={{ paddingVertical: 8, alignItems: 'center' }}>
            <Text className={`font-sans text-sm font-medium ${o === value ? 'text-ink' : 'text-ink-2'}`}>{o[0] + o.slice(1).toLowerCase()}</Text>
          </Squircle>
        </Pressable>
      ))}
    </Squircle>
  )
}

function Intensity() {
  const [level, setLevel] = useState<HapticIntensity>(hapticIntensity)
  return (
    <Segmented<HapticIntensity>
      value={level}
      options={['off', 'subtle', 'full']}
      onChange={(l) => {
        setHapticIntensity(l)
        setLevel(l)
      }}
    />
  )
}

/** pullProgress, as a pull to refresh would play it: building up, then triggering. */
function PullDemo() {
  return (
    <Button
      label="pull…"
      silent
      onPress={() => {
        for (let i = 1; i <= 8; i++) setTimeout(() => haptic('pullProgress', i / 8), i * 60)
        setTimeout(() => haptic('pullTrigger'), 9 * 60)
      }}
    />
  )
}

function MotionDemo() {
  const motion = useMotion()
  const [shown, setShown] = useState<{ name: (typeof MOTIONS)[number]; key: number }>({ name: 'pop', key: 0 })
  const preset = motion[shown.name] as Motions[keyof Motions]
  const style = typeof preset === 'function' ? (preset as () => object)() : preset
  return (
    <View>
      <View className="flex-row flex-wrap gap-2">
        {MOTIONS.map((name) => (
          <Button key={name} label={name} onPress={() => setShown((s) => ({ name, key: s.key + 1 }))} />
        ))}
      </View>
      <View className="mt-4 h-24 items-center justify-center">
        <Animated.View key={shown.key} style={style}>
          <Squircle radius={16} edge className="bg-float" style={{ width: 160, height: 72, alignItems: 'center', justifyContent: 'center' }}>
            <Text className="font-sans font-medium text-ink">{shown.name}</Text>
          </Squircle>
        </Animated.View>
      </View>
    </View>
  )
}
