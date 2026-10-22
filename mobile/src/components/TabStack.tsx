// SPDX-License-Identifier: AGPL-3.0-or-later

import { Stack } from 'expo-router'
import { useRoute } from 'expo-router/react-navigation'
import { TabContext, tabOf } from '../nav'
import { useTheme } from '../theme/ThemeProvider'

/** A tab's own stack of screens, which slide in over each other and go back with the system's gesture. */
export function TabStack() {
  const { tokens } = useTheme()
  const tab = tabOf([useRoute().name])
  return (
    <TabContext.Provider value={tab}>
      <Stack screenOptions={{ headerShown: false, contentStyle: { backgroundColor: tokens.canvas }, animation: 'default' }} />
    </TabContext.Provider>
  )
}
