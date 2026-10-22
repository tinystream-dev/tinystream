// SPDX-License-Identifier: AGPL-3.0-or-later

import { requireNativeView } from 'expo'
import type { ViewProps } from 'react-native'

export type SquircleViewProps = ViewProps & {
  /** The corners' radius, in dp. */
  radius: number
  /** 0 is a plain rounded rectangle; 0.6 is iOS-like. */
  smoothing: number
  /** A hairline along the curve, in these colours at the top, a third of the way down and the bottom. */
  edge?: string[] | null
  /** A dashed hairline along the curve, in this colour. */
  dashed?: string | null
}

/** A view clipped (children and all) to a squircle of its own size, by the system's outline clipping. */
export const SquircleView = requireNativeView<SquircleViewProps>('TinystreamSquircle')
