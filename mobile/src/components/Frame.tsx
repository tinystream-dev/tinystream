// SPDX-License-Identifier: AGPL-3.0-or-later

import { useRouter } from 'expo-router'
import { ArrowLeft, ShieldOff } from 'lucide-react-native'
import type { ReactNode } from 'react'
import { KeyboardAvoidingView, Pressable, ScrollView, Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { haptic } from '../../modules/haptics'
import { useMotion } from '../effects/motion'
import { useTheme } from '../theme/ThemeProvider'
import { Logo } from './Logo'

/** The signed-out screens' layout (web's Login `Frame`): the logo and a title over a form. */
export function Frame({ title, subtitle, back, children }: { title?: string; subtitle?: string; back?: boolean; children: ReactNode }) {
  const insets = useSafeAreaInsets()
  const router = useRouter()
  const motion = useMotion()
  const { tokens } = useTheme()
  return (
    <KeyboardAvoidingView behavior="padding" style={{ flex: 1 }}>
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{ flexGrow: 1, justifyContent: 'center', paddingTop: insets.top + 56, paddingBottom: insets.bottom + 24, paddingHorizontal: 20 }}
      >
        <Animated.View style={motion.rise}>
          {title && (
            <View className="mb-7 flex-row items-center gap-3">
              <Logo size={36} />
              <View className="flex-1">
                <Text className="font-sans text-xl font-semibold tracking-tight text-ink">{title}</Text>
                {subtitle && <Text className="font-sans text-sm text-ink-2">{subtitle}</Text>}
              </View>
            </View>
          )}
          {children}
        </Animated.View>
      </ScrollView>
      {back && router.canGoBack() && (
        <Pressable
          accessibilityLabel="Back"
          hitSlop={12}
          onPress={() => {
            haptic('tick')
            router.back()
          }}
          style={{ position: 'absolute', top: insets.top + 12, left: 16, padding: 6 }}
        >
          <ArrowLeft size={22} color={tokens.ink} />
        </Pressable>
      )}
    </KeyboardAvoidingView>
  )
}

/** Shown wherever a password could be typed over plain http. */
export function Unencrypted() {
  const { tokens } = useTheme()
  return (
    <View className="flex-row items-start gap-2.5">
      <ShieldOff size={16} color={tokens.warn} style={{ marginTop: 2 }} />
      <Text className="font-sans flex-1 text-[13px] leading-5 text-ink-2">
        <Text className="font-medium text-warn">Unencrypted connection: </Text>
        your password and media travel in plain text.
      </Text>
    </View>
  )
}
