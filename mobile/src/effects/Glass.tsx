// SPDX-License-Identifier: AGPL-3.0-or-later
// Floating material (web's `.material` and `.material-bar`). In the Glass
// style it's see-through and blurs what's behind it; otherwise it's solid.
// Android can only blur what's inside a BlurArea, so a screen wraps itself in
// a GlassScope, puts its content in a BlurArea and floats its glass beside
// it. Glass inside the BlurArea it would blur stays solid: blurring itself
// would recurse until the renderer's stack runs out. A scope can hold several
// BlurAreas (one per tab); its glass blurs the active one.

import { BlurTargetView, BlurView } from 'expo-blur'
import { useIsFocused } from 'expo-router'
import { type ReactNode, type RefObject, createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import { StyleSheet, View, type ViewProps } from 'react-native'
import { useTheme } from '../theme/ThemeProvider'
import { lift, withAlpha } from '../theme/materials'
import { Squircle, type SquircleProps } from './Squircle'

type Scope = { target: RefObject<View | null>; claim: (view: View | null, from?: View | null) => void }

const Target = createContext<Scope | null>(null)
const Inside = createContext(false)

/** Where glass can blur the scope's active BlurArea. */
export function GlassScope({ children }: { children?: ReactNode }) {
  const [view, setView] = useState<View | null>(null)
  const claim = useCallback<Scope['claim']>((next, from) => setView((v) => (from === undefined || v === from ? next : v)), [])
  // A new object for each target, so the blur views see it change.
  const scope = useMemo<Scope>(() => ({ target: { current: view }, claim }), [view, claim])
  return <Target.Provider value={scope}>{children}</Target.Provider>
}

/** What the glass in the same GlassScope (but not in here) blurs, while it's `active`. */
export function BlurArea({ active = true, children, ...rest }: ViewProps & { active?: boolean; children?: ReactNode }) {
  const scope = useContext(Target)
  const ref = useRef<View>(null)
  const claim = scope?.claim
  useEffect(() => {
    const view = ref.current
    if (!claim || !active || !view) return
    claim(view)
    return () => claim(null, view)
  }, [claim, active])
  return (
    <Inside.Provider value>
      <BlurTargetView ref={ref} {...rest}>
        {children}
      </BlurTargetView>
    </Inside.Provider>
  )
}

/** A screen's BlurArea, active while the screen is in focus. Only it follows focus, not the whole screen. */
export function ScreenBlurArea(props: ViewProps & { children?: ReactNode }) {
  return <BlurArea active={useIsFocused()} {...props} />
}

export type GlassProps = SquircleProps & {
  /** A bar along the screen's edge (tab bar, top bar), tinted with the canvas rather than lifted. */
  bar?: boolean
}

export function Glass({ bar = false, radius = 16, edge = true, style, children, ...rest }: GlassProps) {
  const { style: name, tokens, material } = useTheme()
  const target = useContext(Target)?.target
  const inside = useContext(Inside)
  const blur = material.glass && target?.current != null && !inside
  const base = bar ? tokens.canvas : tokens.float
  // Same shapes as with the blur, just solid enough to read without it.
  const fill = !material.glass ? base : blur ? withAlpha(base, bar ? 0.62 : 0.7) : withAlpha(tokens.float, 0.96)

  const body = (
    <Squircle radius={bar ? 0 : radius} edge={edge && !bar} style={[{ backgroundColor: blur ? undefined : fill }, style]} {...rest}>
      {blur && (
        <>
          <BlurView
            blurTarget={target as RefObject<View | null>}
            blurMethod="dimezisBlurViewSdk31Plus"
            intensity={bar ? 80 : 70}
            style={StyleSheet.absoluteFill}
          />
          <View style={[StyleSheet.absoluteFill, { backgroundColor: fill }]} />
        </>
      )}
      {children}
    </Squircle>
  )
  const shadow = bar ? undefined : lift(name, tokens)
  return shadow ? <View style={{ boxShadow: shadow, borderRadius: Math.round(radius * material.corners.scale) }}>{body}</View> : body
}
