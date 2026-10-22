// SPDX-License-Identifier: AGPL-3.0-or-later
// Pieces that sit over artwork: the ambient glow a title casts, the shade
// that keeps text readable, and the little labels in its corners.

import type { ReactNode } from 'react'
import { StyleSheet, Text, View, type ViewStyle } from 'react-native'
import { rgba } from '../lib/tint'
import { useTheme } from '../theme/ThemeProvider'
import { withAlpha } from '../theme/materials'

/** The title's colour glowing from a corner (web's `.ambient`); nothing while there's no tint. */
export function Ambient({ tint, alpha = 0.38, at = '0% 100%', size = '120% 90%' }: { tint: string | null | undefined; alpha?: number; at?: string; size?: string }) {
  if (!tint) return null
  return (
    <View
      pointerEvents="none"
      style={[StyleSheet.absoluteFill, { experimental_backgroundImage: `radial-gradient(${size} at ${at}, ${rgba(tint, alpha)}, transparent 60%)` } as ViewStyle]}
    />
  )
}

/** A gradient from the canvas (or the media shade) over artwork, toward `to`. */
export function Shade({ to = 'right', stops, media = false }: { to?: 'right' | 'top' | 'bottom'; stops: [number, number, number]; media?: boolean }) {
  const { tokens } = useTheme()
  const c = media ? tokens['media-shade'] : tokens.canvas
  return (
    <View
      pointerEvents="none"
      style={[
        StyleSheet.absoluteFill,
        { experimental_backgroundImage: `linear-gradient(to ${to}, ${withAlpha(c, stops[0])}, ${withAlpha(c, stops[1])}, ${withAlpha(c, stops[2])})` } as ViewStyle,
      ]}
    />
  )
}

/** A small label over artwork, like "New episode". */
export function Tag({ children, dot, color, style }: { children: ReactNode; dot?: string; color?: string; style?: ViewStyle }) {
  return (
    <View className="flex-row items-center gap-1.5 rounded-md bg-media-shade/60 px-2 py-1" style={style}>
      {dot && <View style={{ width: 6, height: 6, borderRadius: 3, backgroundColor: dot }} />}
      <Text className="font-sans text-2xs font-medium text-media-ink" style={color ? { color } : undefined}>
        {children}
      </Text>
    </View>
  )
}

/** The bar along the bottom of a card, `value` (0–1) of the way. */
export function Bar({ value, color }: { value: number; color?: string }) {
  return (
    <View className="absolute bottom-0 left-0 right-0 h-[3px] bg-media-ink/15">
      <View className="h-full bg-media-ink" style={{ width: `${Math.min(1, value) * 100}%`, backgroundColor: color }} />
    </View>
  )
}
