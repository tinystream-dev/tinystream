// SPDX-License-Identifier: AGPL-3.0-or-later
// The same colour names as web's Tailwind theme (web/src/styles.css), so
// `bg-canvas` or `text-ink-2` mean the same in both apps. Their values are
// the scheme in use, set as variables at the root by ThemeProvider.

import { TOKENS } from '@tinystream/shared/theme'
import type { Config } from 'tailwindcss'
import { MEDIA } from './src/theme/vars'

type Opacity = { opacityValue?: string }

/**
 * The token as the scheme has it, or at an opacity (`bg-ink/10`) from its
 * channels. Tailwind takes functions here, though its types don't say so.
 */
const color = (name: string) =>
  (({ opacityValue }: Opacity) =>
    opacityValue === undefined || opacityValue === '1' ? `var(--color-${name})` : `rgb(var(--rgb-${name}) / ${opacityValue})`) as unknown as string

export default {
  content: ['./app/**/*.{ts,tsx}', './src/**/*.{ts,tsx}'],
  presets: [require('nativewind/preset')],
  corePlugins: {
    backgroundOpacity: false,
    textOpacity: false,
    borderOpacity: false,
    divideOpacity: false,
    placeholderOpacity: false,
    ringOpacity: false,
  },
  theme: {
    extend: {
      colors: Object.fromEntries([...TOKENS, ...MEDIA].map((name) => [name, color(name)])),
      fontFamily: { sans: ['Geist'] },
      fontSize: { '2xs': ['11px', '16px'] },
    },
  },
} satisfies Config
