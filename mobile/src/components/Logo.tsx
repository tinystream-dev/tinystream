// SPDX-License-Identifier: AGPL-3.0-or-later

import { GLYPH, PLATE } from '@tinystream/shared/logo'
import Svg, { Defs, LinearGradient, Path, Stop } from 'react-native-svg'
import { useTheme } from '../theme/ThemeProvider'

/** The logo in the scheme's colours: Layered gets web's soft gradient, the others a flat plate. */
export function Logo({ size }: { size: number }) {
  const { style, tokens } = useTheme()
  return (
    <Svg width={size} height={size} viewBox="0 0 1024 1024">
      {style === 'layered' && (
        <Defs>
          <LinearGradient id="plate" x1="0" y1="0" x2="0" y2="1">
            <Stop offset="0" stopColor={tokens.ink} />
            <Stop offset="1" stopColor={tokens.ink} stopOpacity={0.86} />
          </LinearGradient>
        </Defs>
      )}
      <Path d={PLATE} fill={style === 'layered' ? 'url(#plate)' : tokens.ink} />
      <Path d={GLYPH} fill={tokens.canvas} />
    </Svg>
  )
}
