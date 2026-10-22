// SPDX-License-Identifier: AGPL-3.0-or-later

import { cssInterop } from 'nativewind'
import { type ReactNode, useMemo } from 'react'
import type { ViewProps } from 'react-native'
import { SquircleView } from '../../modules/squircle'
import { useTheme } from '../theme/ThemeProvider'
import { withAlpha } from '../theme/materials'

cssInterop(SquircleView, { className: 'style' })

export type SquircleProps = ViewProps & {
  className?: string
  /** At Layered's scale; the component style makes it bigger or smaller. */
  radius?: number
  smoothing?: number
  /** Draw a hairline edge that's brighter at the top, like light from above. */
  edge?: boolean
  /** Draw a dashed hairline along the curve, for placeholders. */
  dashed?: boolean
  children?: ReactNode
}

/**
 * A box with continuous-curvature corners, clipped (children and all) to a
 * squircle, like web's Squircle. The view builds the path at its own size.
 */
export function Squircle({ radius = 12, smoothing, edge = false, dashed = false, children, ...rest }: SquircleProps) {
  const { material, tokens } = useTheme()
  const glow = useMemo(() => material.edge.map((a) => withAlpha(tokens.glow, a)), [material.edge, tokens.glow])
  return (
    <SquircleView
      radius={Math.round(radius * material.corners.scale)}
      smoothing={smoothing ?? material.corners.smoothing}
      edge={edge && !dashed ? glow : null}
      dashed={dashed ? tokens['line-strong'] : null}
      {...rest}
    >
      {children}
    </SquircleView>
  )
}
