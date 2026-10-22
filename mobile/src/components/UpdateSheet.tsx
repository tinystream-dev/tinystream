// SPDX-License-Identifier: AGPL-3.0-or-later
// A newer release on GitHub. The APK downloads in the browser: the app
// never installs anything itself.

import { Download } from 'lucide-react-native'
import { Linking, Text, View } from 'react-native'
import { useTheme } from '../theme/ThemeProvider'
import { useAppVersion, useUpdate } from '../updates'
import { Sheet } from './Sheet'
import { Button } from './ui'

export function UpdateSheet() {
  const { release, open, dismiss } = useUpdate()
  const app = useAppVersion()
  const { tokens } = useTheme()
  return (
    <Sheet open={open} onClose={dismiss}>
      {release && (
        <View className="gap-1 pt-1">
          <Text className="font-sans text-lg font-semibold text-ink">tinystream {release.version} is out</Text>
          <Text className="font-sans text-sm text-ink-2">You have {app}. The app and your server work best on the same version.</Text>
          <View className="mt-4 gap-2">
            <Button variant="primary" size="lg" icon={<Download size={18} color={tokens['on-accent']} />} onPress={() => void Linking.openURL(release.download)}>
              Download
            </Button>
            <Button size="lg" onPress={() => void Linking.openURL(release.notes)}>
              Release notes
            </Button>
            <Button variant="plain" size="lg" onPress={dismiss}>
              Not now
            </Button>
          </View>
        </View>
      )}
    </Sheet>
  )
}
