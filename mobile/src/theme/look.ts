// SPDX-License-Identifier: AGPL-3.0-or-later

import { type ComponentStyle, type SchemeMode, type Tokens, derive } from '@tinystream/shared/theme'

export type Look = {
  mode: SchemeMode
  light: Tokens
  dark: Tokens
  style: ComponentStyle
  mediaTint: boolean
}

/** Grey, tinystream's own scheme (src/theme.rs), for before any server has said otherwise. */
const GREY = derive(
  {
    canvas: '#191919',
    ink: '#ebebea',
    accent: '#ebebea',
    danger: '#eb8a7a',
    ok: '#8fc79a',
    info: '#7dd3fc',
    warn: '#fcd34d',
    highlight: '#f9a8d4',
    social: '#c4b5fd',
  },
  {
    'ink-2': '#9b9b98',
    'ink-3': '#6e6e6b',
    'accent-hover': '#ffffff',
    'info-deep': '#38bdf8',
    'warn-soft': '#fde68a',
    'warn-deep': '#fbbf24',
  },
)!

export const DEFAULT_LOOK: Look = { mode: 'SINGLE', light: GREY, dark: GREY, style: 'LAYERED', mediaTint: false }
