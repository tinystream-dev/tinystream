// SPDX-License-Identifier: AGPL-3.0-or-later
// Toasts and questions (web's feedback.tsx): toasts float above the tab bar
// and can be flicked away; a question is a sheet with its two answers.

import { CircleAlert, CircleCheck } from 'lucide-react-native'
import { type ReactNode, useEffect, useState, useSyncExternalStore } from 'react'
import { Pressable, Text, View } from 'react-native'
import { Gesture, GestureDetector } from 'react-native-gesture-handler'
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { Glass } from '../effects/Glass'
import { Squircle } from '../effects/Squircle'
import { useMotion } from '../effects/motion'
import { useTheme } from '../theme/ThemeProvider'
import { Img } from './Img'
import { Sheet } from './Sheet'
import { Button } from './ui'

export type Toast = {
  id: number
  title: string
  body?: string | null
  image?: string | null
  tone?: 'ok' | 'danger' | 'quiet'
  action?: { label: string; run: () => void }
  /** Tapping the toast. */
  onPress?: () => void
  /** Milliseconds on screen. */
  duration?: number
}

let toasts: Toast[] = []
let seq = 0
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((l) => l())
const subscribe = (l: () => void) => (listeners.add(l), () => void listeners.delete(l))

export function toast(t: Omit<Toast, 'id'>) {
  const id = ++seq
  toasts = [...toasts.slice(-2), { ...t, id }]
  if (t.tone === 'ok') haptic('success')
  if (t.tone === 'danger') haptic('error')
  emit()
  return id
}

export function dismiss(id: number) {
  if (!toasts.some((t) => t.id === id)) return
  toasts = toasts.filter((t) => t.id !== id)
  emit()
}

/** Shows an error from a failed action. */
export function toastError(e: unknown) {
  toast({ title: 'That didn’t work', body: (e as Error)?.message ?? String(e), tone: 'danger', duration: 7000 })
}

type Ask = { title: string; body?: ReactNode; confirm?: string; danger?: boolean; resolve: (ok: boolean) => void }
let asking: Ask | null = null

/** A confirmation sheet. Resolves true when confirmed. */
export function ask(o: Omit<Ask, 'resolve'>): Promise<boolean> {
  return new Promise((resolve) => {
    asking?.resolve(false)
    asking = { ...o, resolve }
    emit()
  })
}

function answer(ok: boolean) {
  asking?.resolve(ok)
  asking = null
  emit()
}

/** Where toasts sit: above the tab bar (and the music player, once it's there). */
export function Feedback({ bottom }: { bottom: number }) {
  const list = useSyncExternalStore(subscribe, () => toasts)
  const question = useSyncExternalStore(subscribe, () => asking)
  const [shown, setShown] = useState<Ask | null>(null)
  if (question && question !== shown) setShown(question)
  return (
    <>
      <View pointerEvents="box-none" style={{ position: 'absolute', left: 12, right: 12, bottom, gap: 8 }}>
        {list.map((t) => (
          <ToastView key={t.id} t={t} />
        ))}
      </View>
      <Sheet open={!!question} onClose={() => answer(false)}>
        {shown && (
          <View className="gap-1.5 pt-1">
            <Text className="font-sans text-lg font-semibold text-ink">{shown.title}</Text>
            {typeof shown.body === 'string' ? <Text className="font-sans text-sm leading-5 text-ink-2">{shown.body}</Text> : shown.body}
            <View className="mt-4 gap-2">
              <Button variant={shown.danger ? 'danger-solid' : 'primary'} size="lg" onPress={() => answer(true)}>
                {shown.confirm ?? 'Continue'}
              </Button>
              <Button variant="plain" size="lg" onPress={() => answer(false)}>
                Cancel
              </Button>
            </View>
          </View>
        )}
      </Sheet>
    </>
  )
}

function ToastView({ t }: { t: Toast }) {
  const { tokens } = useTheme()
  const motion = useMotion()
  const [broken, setBroken] = useState(false)
  const x = useSharedValue(0)
  useEffect(() => {
    const id = setTimeout(() => dismiss(t.id), t.duration ?? 5000)
    return () => clearTimeout(id)
  }, [t])
  const flick = Gesture.Pan()
    .activeOffsetX([-10, 10])
    .onUpdate((e) => (x.value = e.translationX))
    .onEnd((e) => {
      if (Math.abs(e.translationX) > 90 || Math.abs(e.velocityX) > 800) {
        x.value = withTiming(Math.sign(e.translationX || e.velocityX) * 500, { duration: 160 }, () => scheduleOnRN(dismiss, t.id))
        scheduleOnRN(haptic, 'tick')
      } else x.value = withTiming(0)
    })
  const slide = useAnimatedStyle(() => ({ transform: [{ translateX: x.value }], opacity: 1 - Math.min(1, Math.abs(x.value) / 300) }))
  const icon =
    t.tone === 'ok' ? <CircleCheck size={18} color={tokens.ok} /> : t.tone === 'danger' ? <CircleAlert size={18} color={tokens.danger} /> : null
  return (
    <GestureDetector gesture={flick}>
      <Animated.View style={[motion.rise, slide]}>
        <Glass radius={16} style={{ paddingVertical: 10, paddingLeft: 12, paddingRight: 8 }}>
          <Pressable
            disabled={!t.onPress}
            onPress={() => {
              haptic('press')
              t.onPress?.()
              dismiss(t.id)
            }}
            className="flex-row items-center gap-3"
          >
            {t.image && !broken ? (
              <Squircle radius={8} style={{ width: 32, aspectRatio: 2 / 3 }} className="bg-panel">
                <Img src={t.image} style={{ width: '100%', height: '100%' }} onError={() => setBroken(true)} />
              </Squircle>
            ) : (
              icon
            )}
            <View className="min-w-0 flex-1">
              <Text className="font-sans text-sm font-medium text-ink" numberOfLines={1}>
                {t.title}
              </Text>
              {t.body ? (
                <Text className="font-sans mt-0.5 text-xs leading-4 text-ink-2" numberOfLines={2}>
                  {t.body}
                </Text>
              ) : null}
            </View>
            {t.action && (
              <Button
                size="sm"
                variant="plain"
                onPress={() => {
                  t.action!.run()
                  dismiss(t.id)
                }}
              >
                <Text className="font-sans text-[13px] font-semibold text-ink">{t.action.label}</Text>
              </Button>
            )}
          </Pressable>
        </Glass>
      </Animated.View>
    </GestureDetector>
  )
}
