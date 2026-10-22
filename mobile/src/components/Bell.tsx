// SPDX-License-Identifier: AGPL-3.0-or-later

import { Bell as BellIcon } from 'lucide-react-native'
import { Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { useMotion } from '../effects/motion'
import { useGo } from '../nav'
import { useInbox } from '../notifications'
import { useTheme } from '../theme/ThemeProvider'
import { IconButton } from './ui'

/** The notifications bell, with how many are unread. */
export function Bell() {
  const { data } = useInbox()
  const go = useGo()
  const { tokens } = useTheme()
  const motion = useMotion()
  const unread = data?.unread ?? 0
  return (
    <IconButton label={unread ? `Notifications, ${unread} unread` : 'Notifications'} onPress={() => go('notifications')}>
      <View>
        <BellIcon size={21} color={tokens.ink} />
        {unread > 0 && (
          <Animated.View key={unread} style={motion.pop} className="absolute -right-1.5 -top-1 h-4 min-w-4 items-center justify-center rounded-full bg-ink px-1">
            <Text className="font-sans text-[10px] font-semibold text-canvas" style={{ lineHeight: 12 }}>
              {unread > 9 ? '9+' : unread}
            </Text>
          </Animated.View>
        )}
      </View>
    </IconButton>
  )
}
