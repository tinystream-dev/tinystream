// SPDX-License-Identifier: AGPL-3.0-or-later
// A quiet confirmation of what a gesture just did (web's HudView): seeks add
// up while they keep coming, levels show while they're dragged.

import { Pause, Play, RotateCcw, RotateCw, Sun, SunDim, Volume1, Volume2, VolumeX } from 'lucide-react-native'
import { Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { useMotion } from '../effects/motion'
import { useTheme } from '../theme/ThemeProvider'

export type Hud = {
  kind: 'seek' | 'play' | 'pause' | 'speed' | 'text' | 'volume' | 'brightness'
  /** Seconds sought (adding up), or a level from 0 to 1. */
  amount?: number
  text?: string
  key: number
}

const shadowed = { textShadowColor: 'rgba(0, 0, 0, 0.6)', textShadowRadius: 4, textShadowOffset: { width: 0, height: 1 } }

export function HudView({ hud }: { hud: Hud }) {
  const { tokens } = useTheme()
  const motion = useMotion()
  const ink = tokens['media-ink']

  if (hud.kind === 'seek') {
    const back = (hud.amount ?? 0) < 0
    return (
      <View pointerEvents="none" style={{ position: 'absolute', top: 0, bottom: 0, justifyContent: 'center', [back ? 'left' : 'right']: '12%' }}>
        <Animated.View key={hud.key} style={[{ alignItems: 'center', gap: 4 }, back ? motion.nudgeLeft : motion.nudgeRight]}>
          <View className="items-center justify-center rounded-full bg-media-shade/45" style={{ width: 64, height: 64 }}>
            {back ? <RotateCcw size={28} color={ink} /> : <RotateCw size={28} color={ink} />}
          </View>
          <Text className="font-sans text-sm font-medium text-media-ink" style={[{ fontVariant: ['tabular-nums'] }, shadowed]}>
            {back ? '−' : '+'}
            {Math.abs(hud.amount ?? 0)} s
          </Text>
        </Animated.View>
      </View>
    )
  }

  if (hud.kind === 'volume' || hud.kind === 'brightness') {
    const v = hud.amount ?? 0
    const Icon = hud.kind === 'brightness' ? (v < 0.5 ? SunDim : Sun) : v === 0 ? VolumeX : v < 0.5 ? Volume1 : Volume2
    return (
      <View pointerEvents="none" style={{ position: 'absolute', left: 0, right: 0, top: '12%', alignItems: 'center' }}>
        <View className="flex-row items-center gap-3 rounded-full bg-media-shade/55 px-4 py-2.5">
          <Icon size={18} color={ink} />
          <View className="h-1 w-36 overflow-hidden rounded-full bg-media-ink/20">
            <View className="h-full bg-media-ink" style={{ width: `${v * 100}%` }} />
          </View>
          <Text className="font-sans w-8 text-right text-xs text-media-ink" style={{ fontVariant: ['tabular-nums'] }}>
            {Math.round(v * 100)}
          </Text>
        </View>
      </View>
    )
  }

  if (hud.kind === 'speed' || hud.kind === 'text') {
    return (
      <View pointerEvents="none" style={{ position: 'absolute', left: 0, right: 0, top: '12%', alignItems: 'center' }}>
        <Animated.View key={hud.key} style={motion.pop}>
          <Text className="font-sans overflow-hidden rounded-full bg-media-shade/55 px-4 py-2 text-sm font-medium text-media-ink" style={{ fontVariant: ['tabular-nums'] }}>
            {hud.text}
          </Text>
        </Animated.View>
      </View>
    )
  }

  return (
    <View pointerEvents="none" style={{ position: 'absolute', top: 0, bottom: 0, left: 0, right: 0, alignItems: 'center', justifyContent: 'center' }}>
      <Animated.View key={hud.key} className="items-center justify-center rounded-full bg-media-shade/45" style={[{ width: 80, height: 80 }, motion.flash]}>
        {hud.kind === 'play' ? <Play size={32} color={ink} fill={ink} /> : <Pause size={32} color={ink} fill={ink} />}
      </Animated.View>
    </View>
  )
}
