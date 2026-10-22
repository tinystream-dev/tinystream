// SPDX-License-Identifier: AGPL-3.0-or-later

import { TriangleAlert, X } from 'lucide-react-native'
import { Pressable, Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { haptic } from '../../modules/haptics'
import { Squircle } from '../effects/Squircle'
import { useMotion } from '../effects/motion'
import { useStatus } from '../queries'
import { useSession } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { useMismatch } from '../updates'

/** The app and the server are different versions: a warning, never a wall. */
export function MismatchBanner() {
  const { server } = useSession()
  const { data } = useStatus()
  const mismatch = useMismatch(server.id, data?.server.version)
  const { tokens } = useTheme()
  const motion = useMotion()
  if (!mismatch) return null
  return (
    <Animated.View style={motion.rise}>
      <Squircle radius={14} className="flex-row items-start gap-3 bg-warn/10 p-3.5">
        <TriangleAlert size={18} color={tokens.warn} style={{ marginTop: 1 }} />
        <Text className="font-sans flex-1 text-sm leading-5 text-ink">
          This app is {mismatch.app}, your server is {mismatch.server}. Some things may not work. Update the {mismatch.update}.
        </Text>
        <Pressable
          hitSlop={12}
          accessibilityLabel="Dismiss"
          onPress={() => {
            haptic('tick')
            mismatch.dismiss()
          }}
        >
          <View>
            <X size={18} color={tokens['ink-2']} />
          </View>
        </Pressable>
      </Squircle>
    </Animated.View>
  )
}
