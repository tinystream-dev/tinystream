// SPDX-License-Identifier: AGPL-3.0-or-later
// What a screen looks like before its content arrives (web's Skeleton.tsx),
// and the moment it does.

import { useEffect, useState } from 'react'
import { View, useWindowDimensions } from 'react-native'
import { haptic } from '../../modules/haptics'
import { Bone } from '../effects/Shimmer'

/**
 * Whether content has just replaced a skeleton (it wasn't ready when this
 * first rendered), which is felt once; refetches aren't.
 */
export function useArrived(ready: boolean) {
  const [cold] = useState(!ready)
  const arrived = cold && ready
  useEffect(() => {
    if (arrived) haptic('reveal')
  }, [arrived])
  return arrived
}

/** The width of a poster in a grid of `columns` across the screen. */
export function useColumnWidth(columns = 3, inset = 20, gap = 12) {
  const { width } = useWindowDimensions()
  return Math.floor((width - inset * 2 - gap * (columns - 1)) / columns)
}

export function PosterGridSkeleton({ count = 12 }: { count?: number }) {
  const w = useColumnWidth()
  return (
    <View className="flex-row flex-wrap" style={{ gap: 12, rowGap: 20 }}>
      {Array.from({ length: count }, (_, i) => (
        <View key={i} style={{ width: w }}>
          <Bone radius={14} style={{ aspectRatio: 2 / 3 }} />
          <Bone radius={5} style={{ marginTop: 8, height: 11, width: '75%' }} />
          <Bone radius={5} style={{ marginTop: 6, height: 9, width: '40%' }} />
        </View>
      ))}
    </View>
  )
}

export function RowSkeleton({ wide = false }: { wide?: boolean }) {
  return (
    <View>
      <Bone radius={6} style={{ height: 16, width: 150, marginBottom: 14 }} />
      <View className="flex-row gap-3 overflow-hidden">
        {Array.from({ length: 4 }, (_, i) =>
          wide ? (
            <Bone key={i} radius={16} style={{ width: 280, aspectRatio: 16 / 9 }} />
          ) : (
            <Bone key={i} radius={14} style={{ width: 124, aspectRatio: 2 / 3 }} />
          ),
        )}
      </View>
    </View>
  )
}

export function ListSkeleton({ rows = 4, height = 88 }: { rows?: number; height?: number }) {
  return (
    <View className="gap-2">
      {Array.from({ length: rows }, (_, i) => (
        <Bone key={i} radius={16} style={{ height }} />
      ))}
    </View>
  )
}
