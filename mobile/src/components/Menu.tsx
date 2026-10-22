// SPDX-License-Identifier: AGPL-3.0-or-later
// What web shows as a popover menu: a sheet of actions, opened by a long
// press or an overflow button.

import type { ReactNode } from 'react'
import { Pressable, Text, View } from 'react-native'
import { haptic } from '../../modules/haptics'
import { Squircle } from '../effects/Squircle'
import { useTheme } from '../theme/ThemeProvider'
import { Sheet } from './Sheet'

export type MenuItem = {
  label: string
  icon?: (color: string) => ReactNode
  onPress: () => void
  danger?: boolean
  disabled?: boolean
}

export function Menu({
  open,
  onClose,
  header,
  items,
  note,
}: {
  open: boolean
  onClose: () => void
  header?: ReactNode
  items: (MenuItem | false | null | undefined)[]
  /** Said when there's nothing to do. */
  note?: string
}) {
  const { tokens } = useTheme()
  const shown = items.filter((i): i is MenuItem => !!i)
  return (
    <Sheet open={open} onClose={onClose}>
      {header && <View className="mb-2">{header}</View>}
      {shown.length === 0 && note && <Text className="font-sans px-3 py-3 text-sm text-ink-3">{note}</Text>}
      {shown.map((item) => {
        const color = item.danger ? tokens.danger : tokens.ink
        return (
          <Pressable
            key={item.label}
            disabled={item.disabled}
            onPress={() => {
              haptic('press')
              onClose()
              item.onPress()
            }}
            style={{ opacity: item.disabled ? 0.4 : 1 }}
          >
            {({ pressed }) => (
              <Squircle radius={12} className={pressed ? 'bg-press' : ''} style={{ flexDirection: 'row', alignItems: 'center', gap: 14, paddingHorizontal: 12, paddingVertical: 14 }}>
                {item.icon && <View style={{ width: 22, alignItems: 'center' }}>{item.icon(item.danger ? tokens.danger : tokens['ink-2'])}</View>}
                <Text className="font-sans flex-1 text-[15px]" style={{ color }}>
                  {item.label}
                </Text>
              </Squircle>
            )}
          </Pressable>
        )
      })}
    </Sheet>
  )
}
