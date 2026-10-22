// SPDX-License-Identifier: AGPL-3.0-or-later
// The look in use: the scheme's tokens as NativeWind variables at the root,
// light or dark as the system is, and the component style. It starts from
// the last look this device saw (web's `bootstrap`) and changes when a
// server sends one (`setLook`).

import { type Tokens, isDark } from '@tinystream/shared/theme'
import { StatusBar } from 'expo-status-bar'
import * as SystemUI from 'expo-system-ui'
import { vars } from 'nativewind'
import { type ReactNode, createContext, useContext, useEffect, useMemo, useState } from 'react'
import { View, useColorScheme } from 'react-native'
import { read, write } from '../storage'
import { DEFAULT_LOOK, type Look } from './look'
import { MATERIALS, type Material, type StyleName } from './materials'
import { type MediaName, mediaColors, variables } from './vars'

export type Theme = {
  look: Look
  /** The scheme on screen now: the look's light or dark one. */
  tokens: Tokens & Record<MediaName, string>
  dark: boolean
  style: StyleName
  material: Material
  setLook: (look: Look) => void
}

const ThemeContext = createContext<Theme | null>(null)

const KEY = 'theme'

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [look, setLookState] = useState<Look>(() => read<Look>(KEY) ?? DEFAULT_LOOK)
  const system = useColorScheme()

  const theme = useMemo<Theme>(() => {
    const scheme = look.mode === 'SYSTEM' && system === 'dark' ? look.dark : look.light
    const style = look.style.toLowerCase() as StyleName
    return {
      look,
      tokens: { ...scheme, ...mediaColors(scheme, look.mediaTint) },
      dark: isDark(scheme),
      style,
      material: MATERIALS[style] ?? MATERIALS.layered,
      setLook: (next) => {
        write(KEY, next)
        setLookState(next)
      },
    }
  }, [look, system])

  const scheme = theme.tokens
  const root = useMemo(() => vars(variables(scheme, look.mediaTint)), [scheme, look.mediaTint])

  useEffect(() => {
    void SystemUI.setBackgroundColorAsync(scheme.canvas)
  }, [scheme.canvas])

  return (
    <ThemeContext.Provider value={theme}>
      <StatusBar style={theme.dark ? 'light' : 'dark'} />
      <View style={[{ flex: 1, backgroundColor: scheme.canvas }, root]}>{children}</View>
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const theme = useContext(ThemeContext)
  if (!theme) throw new Error('useTheme needs a ThemeProvider above it')
  return theme
}
