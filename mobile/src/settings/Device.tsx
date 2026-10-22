// SPDX-License-Identifier: AGPL-3.0-or-later
// Settings that belong to this phone rather than to an account on a server.

import * as Clipboard from 'expo-clipboard'
import Constants from 'expo-constants'
import { useRouter } from 'expo-router'
import { Bell, Bug, ClipboardCopy, Info, Settings2 } from 'lucide-react-native'
import { useState } from 'react'
import { Text, View } from 'react-native'
import { type HapticIntensity, haptic, hapticIntensity, setHapticIntensity } from '../../modules/haptics'
import { openNotificationSettings } from '../../modules/notify'
import { backgroundChecks, setBackgroundChecks } from '../background'
import { toast, toastError } from '../components/Feedback'
import { Divider, Group, ListRow, Segmented, Toggle } from '../components/ui'
import { logText } from '../log'
import { useStatus } from '../queries'
import { useTheme } from '../theme/ThemeProvider'
import { checkForUpdates } from '../updates'

export function Device() {
  const router = useRouter()
  const { tokens } = useTheme()
  const { data: status } = useStatus()
  const [haptics, setHaptics] = useState<HapticIntensity>(hapticIntensity)
  const updates = checkForUpdates.use()
  const background = backgroundChecks.use()
  const icon = (Icon: typeof Bell) => <Icon size={20} color={tokens['ink-2']} />
  return (
    <>
      <Group title="Haptics" description="How much the phone answers your touch.">
        <View className="p-3">
          <Segmented<HapticIntensity>
            value={haptics}
            options={[
              { value: 'off', label: 'Off' },
              { value: 'subtle', label: 'Subtle' },
              { value: 'full', label: 'Full' },
            ]}
            onChange={(level) => {
              setHapticIntensity(level)
              setHaptics(level)
              haptic('success')
            }}
          />
        </View>
      </Group>

      <Group title="Notifications" description="The app asks your servers for new notifications every 15 minutes or so, while it's closed too. Android may wait longer to save battery.">
        <ListRow
          icon={icon(Bell)}
          label="Check in the background"
          right={<Toggle label="Check in the background" value={background} onChange={(on) => void setBackgroundChecks(on).catch(toastError)} />}
        />
        <Divider inset={52} />
        <ListRow icon={icon(Settings2)} label="Android notification settings" onPress={openNotificationSettings} />
      </Group>

      <Group title="Updates" description="Once a day, asks GitHub whether there's a newer release. Nothing else is sent.">
        <ListRow label="Check for updates" right={<Toggle label="Check for updates" value={updates} onChange={(on) => checkForUpdates.set(on)} />} />
      </Group>

      <Group title="Troubleshooting" description="The log has no passwords or tokens in it, nor the addresses' query strings.">
        <ListRow
          icon={icon(ClipboardCopy)}
          label="Copy debug log"
          chevron={false}
          onPress={async () => {
            await Clipboard.setStringAsync(logText())
            toast({ title: 'Debug log copied', tone: 'ok' })
          }}
        />
        <Divider inset={52} />
        <ListRow icon={icon(Bug)} label="Debug screen" onPress={() => router.push('/debug')} />
      </Group>

      <Group title="About">
        <ListRow icon={icon(Info)} label="This app" value={Constants.expoConfig?.version ?? '—'} />
        <Divider inset={52} />
        <ListRow label="The server" value={status?.server.version ?? '—'} icon={<Text />} />
      </Group>
    </>
  )
}
