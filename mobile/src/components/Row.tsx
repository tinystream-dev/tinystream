// SPDX-License-Identifier: AGPL-3.0-or-later

import type { ReactElement } from 'react'
import { FlatList } from 'react-native-gesture-handler'

/**
 * A horizontal shelf (web's Row) that snaps to its cards. It's the gesture
 * handler's list, so a sideways drag on it scrolls it rather than the tabs.
 */
export function Row<T>({
  data,
  width,
  render,
  keyOf,
  gap = 12,
  inset = 20,
}: {
  data: readonly T[]
  /** Each card's width, which the snapping steps by. */
  width: number
  render: (item: T, index: number) => ReactElement
  keyOf: (item: T) => string | number
  gap?: number
  inset?: number
}) {
  return (
    <FlatList
      horizontal
      data={data}
      keyExtractor={(item) => String(keyOf(item))}
      renderItem={({ item, index }) => render(item, index)}
      showsHorizontalScrollIndicator={false}
      snapToInterval={width + gap}
      decelerationRate="fast"
      contentContainerStyle={{ paddingHorizontal: inset, gap, paddingVertical: 4 }}
      style={{ marginHorizontal: -inset }}
      initialNumToRender={4}
      windowSize={5}
    />
  )
}
