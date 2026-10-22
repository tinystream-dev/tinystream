// SPDX-License-Identifier: AGPL-3.0-or-later
// web's keyframes (web/src/styles.css) as Reanimated CSS animations, with the
// durations and easings web uses them with. Spread one into an Animated
// view's style; `useMotion` gives none of them when animations are off.

import { useMemo } from 'react'
import type { ViewStyle } from 'react-native'
import { type CSSAnimationKeyframes, type CSSAnimationProperties, Easing, cubicBezier, useReducedMotion, withDelay, withTiming } from 'react-native-reanimated'
import { useTheme } from '../theme/ThemeProvider'
import { type Material, withAlpha } from '../theme/materials'

/** cubic-bezier(.16, 1, .3, 1): things that develop and sharpen in. */
export const settle = cubicBezier(0.16, 1, 0.3, 1)
/** cubic-bezier(.2, .8, .2, 1): things that move into place. */
export const glide = cubicBezier(0.2, 0.8, 0.2, 1)
/** cubic-bezier(.65, 0, .35, 1): wipes and folds. */
export const wipe = cubicBezier(0.65, 0, 0.35, 1)

/** web's `Segmented` thumb moving over: its leading edge springs ahead… */
export const lead = (to: number) => {
  'worklet'
  return withTiming(to, { duration: 340, easing: Easing.bezier(0.3, 1.35, 0.5, 1) })
}
/** …and the trailing one catches up. */
export const trail = (to: number) => {
  'worklet'
  return withDelay(50, withTiming(to, { duration: 420, easing: Easing.bezier(0.65, 0, 0.25, 1) }))
}

type Motion = CSSAnimationProperties<ViewStyle>

const once = (animationName: CSSAnimationKeyframes<ViewStyle>, animationDuration: number, animationTimingFunction: Motion['animationTimingFunction'], rest: Partial<Motion> = {}): Motion => ({
  animationName,
  animationDuration,
  animationTimingFunction,
  ...rest,
})

export const keyframes = {
  fade: { from: { opacity: 0 } },
  fadeOut: { to: { opacity: 0 } },
  pop: (m: Material) => ({ from: { opacity: 0, transform: [{ translateY: m.pop.y }, { scale: m.pop.scale }] } }),
  rise: { from: { opacity: 0, transform: [{ translateY: 8 }, { scale: 0.98 }] } },
  sink: { to: { opacity: 0, transform: [{ translateY: 6 }, { scale: 0.98 }] } },
  sheet: { from: { transform: [{ translateY: '100%' }] } },
  arrive: (ok: string) => ({ '0%': { backgroundColor: ok }, '100%': { backgroundColor: 'transparent' } }),
  pageIn: { from: { opacity: 0, filter: [{ blur: 8 }], transform: [{ scale: 1.008 }] } },
  pageOut: { to: { opacity: 0, filter: [{ blur: 8 }], transform: [{ scale: 0.992 }] } },
  developIn: { from: { opacity: 0, filter: [{ blur: 10 }], transform: [{ translateY: 6 }] } },
  swapIn: (from: number) => ({ from: { opacity: 0, filter: [{ blur: 8 }], transform: [{ translateX: from }] } }),
  unfold: { from: { opacity: 0, transform: [{ scaleY: 0 }] } },
  resolve: { from: { opacity: 0, filter: [{ blur: 8 }], transform: [{ translateY: 2 }] } },
  glint: { from: { transform: [{ translateX: '-100%' }] }, to: { transform: [{ translateX: '100%' }] } },
  ringDone: (info: string) => ({ '0%': { boxShadow: `0px 0px 0px 0px ${info}` }, '100%': { boxShadow: '0px 0px 0px 10px transparent' } }),
  eq: { from: { transform: [{ scaleY: 0.2 }] }, to: { transform: [{ scaleY: 1 }] } },
  pulseDot: { '0%': { opacity: 1 }, '50%': { opacity: 0.35 }, '100%': { opacity: 1 } },
  flash: {
    '0%': { opacity: 0, transform: [{ scale: 0.85 }] },
    '20%': { opacity: 1, transform: [{ scale: 1 }] },
    '100%': { opacity: 0, transform: [{ scale: 1.12 }] },
  },
  shutter: { '0%': { opacity: 0.55 }, '100%': { opacity: 0 } },
  nudgeRight: (m: Material) => ({ from: { opacity: 0, transform: [{ translateX: -6 }, { scale: m.pop.scale }] } }),
  nudgeLeft: (m: Material) => ({ from: { opacity: 0, transform: [{ translateX: 6 }, { scale: m.pop.scale }] } }),
  sweep: { from: { transform: [{ scaleX: 0 }] }, to: { transform: [{ scaleX: 1 }] } },
  hush: {
    '0%': { transform: [{ rotate: '0deg' }] },
    '20%': { transform: [{ rotate: '-16deg' }] },
    '40%': { transform: [{ rotate: '12deg' }] },
    '60%': { transform: [{ rotate: '-7deg' }] },
    '80%': { transform: [{ rotate: '3deg' }] },
    '100%': { transform: [{ rotate: '0deg' }] },
  },
} satisfies Record<string, CSSAnimationKeyframes | ((...args: never[]) => CSSAnimationKeyframes)>

/** The animations as web uses them. */
export function motions(material: Material, tokens: { ok: string; info: string }) {
  return {
    pop: once(keyframes.pop(material), 160, 'ease-out'),
    fade: once(keyframes.fade, 200, 'ease-out'),
    fadeOut: once(keyframes.fadeOut, 320, 'ease', { animationFillMode: 'both' }),
    rise: once(keyframes.rise, 220, glide),
    sink: once(keyframes.sink, 160, 'ease-in', { animationFillMode: 'forwards' }),
    sheet: once(keyframes.sheet, 220, glide),
    arrive: once(keyframes.arrive(withAlpha(tokens.ok, 0.16)), 2400, 'ease-out'),
    pageIn: once(keyframes.pageIn, 420, settle, { animationFillMode: 'both' }),
    pageOut: once(keyframes.pageOut, 220, 'ease-in', { animationFillMode: 'both' }),
    /** For the n-th piece of content replacing a skeleton, top to bottom. */
    developIn: (n = 0) => once(keyframes.developIn, 900, settle, { animationFillMode: 'both', animationDelay: Math.min(n, 5) * 70 }),
    swapIn: (from: number) => once(keyframes.swapIn(from), 560, settle, { animationFillMode: 'both' }),
    unfold: once(keyframes.unfold, 420, settle, { animationFillMode: 'both' }),
    /** For the n-th letter of a title resolving. */
    resolve: (n = 0) => once(keyframes.resolve, 800, settle, { animationFillMode: 'both', animationDelay: n * 30 }),
    glint: once(keyframes.glint, 1400, cubicBezier(0.45, 0, 0.2, 1), { animationFillMode: 'both' }),
    ringDone: once(keyframes.ringDone(withAlpha(tokens.info, 0.6)), 1400, 'ease-out', { animationFillMode: 'both' }),
    eq: (delay = 0) => once(keyframes.eq, 900, 'ease-in-out', { animationIterationCount: 'infinite', animationDirection: 'alternate', animationDelay: delay }),
    pulseDot: once(keyframes.pulseDot, 1600, 'ease-in-out', { animationIterationCount: 'infinite' }),
    flash: once(keyframes.flash, 600, 'ease-out', { animationFillMode: 'forwards' }),
    shutter: once(keyframes.shutter, 420, 'ease-out', { animationFillMode: 'forwards' }),
    nudgeRight: once(keyframes.nudgeRight(material), 160, 'ease-out'),
    nudgeLeft: once(keyframes.nudgeLeft(material), 160, 'ease-out'),
    /** Fills from the left over `ms` (web's countdown to the next episode); give it transformOrigin 'left'. */
    sweep: (ms: number) => once(keyframes.sweep, ms, 'linear', { animationFillMode: 'forwards' }),
    hush: once(keyframes.hush, 700, 'ease-out', { animationFillMode: 'both', animationDelay: 120 }),
  }
}

export type Motions = ReturnType<typeof motions>

const NONE: Motion = { animationName: 'none' }

/** The animations, or none at all when "Remove animations" is on. */
export function useMotion(): Motions {
  const { material, tokens } = useTheme()
  const reduced = useReducedMotion()
  return useMemo(() => {
    const all = motions(material, tokens)
    if (!reduced) return all
    return Object.fromEntries(Object.entries(all).map(([k, v]) => [k, typeof v === 'function' ? () => NONE : NONE])) as unknown as Motions
  }, [material, tokens, reduced])
}
