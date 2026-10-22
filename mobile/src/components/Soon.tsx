// SPDX-License-Identifier: AGPL-3.0-or-later

import type { LucideIcon } from 'lucide-react-native'
import { Text } from 'react-native'
import Animated from 'react-native-reanimated'
import { useMotion } from '../effects/motion'
import { useTheme } from '../theme/ThemeProvider'

/** Where something the app doesn't have yet will be. */
export function Soon({ icon: Icon, title, body }: { icon: LucideIcon; title: string; body: string }) {
  const { tokens } = useTheme()
  const motion = useMotion()
  return (
    <Animated.View style={motion.rise} className="items-center gap-3 px-8 py-24">
      <Icon size={28} color={tokens['ink-3']} />
      <Text className="font-sans text-center text-[17px] font-semibold text-ink">{title}</Text>
      <Text className="font-sans text-center text-sm leading-5 text-ink-2">{body}</Text>
    </Animated.View>
  )
}
