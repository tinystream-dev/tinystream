// SPDX-License-Identifier: AGPL-3.0-or-later
// Colour schemes and component styles in the browser: putting the server's
// look on the page (the tokens themselves are in @tinystream/shared/theme).

import { useSyncExternalStore } from 'react'
import { CORNERS, type ComponentStyle, type SchemeMode, TOKENS, type TokenName, type Tokens, isDark } from '@tinystream/shared/theme'

/** What's kept between visits, so the page is painted right before the app loads. */
export type Painted = {
  css: string
  style: string
  media: boolean
  /** theme-color, with the media query it's for. */
  metas: [string, string | null][]
}

const KEY = 'tinystream:theme'

/**
 * Puts a theme on the page. Runs on its own before the app (see `bootstrap`),
 * so it can't use anything from outside.
 */
function paint(t: Painted | null) {
  if (!t) return
  const head = document.head
  let style = document.getElementById('theme')
  if (!style) {
    style = document.createElement('style')
    style.id = 'theme'
    head.appendChild(style)
  }
  style.textContent = t.css
  const root = document.documentElement
  root.dataset.style = t.style
  if (t.media) root.dataset.mediaTint = ''
  else delete root.dataset.mediaTint
  head.querySelectorAll('meta[data-theme]').forEach((m) => m.remove())
  for (const [color, media] of t.metas) {
    const m = document.createElement('meta')
    m.name = 'theme-color'
    m.content = color
    if (media) m.media = media
    m.dataset.theme = ''
    head.appendChild(m)
  }
}

/** For <head>: the last theme this browser saw, before anything else is drawn. */
export const bootstrap = `try{(${paint.toString()})(JSON.parse(localStorage.getItem(${JSON.stringify(KEY)})))}catch(e){}`

export type Look = {
  mode: SchemeMode
  light: Tokens
  dark: Tokens
  style: ComponentStyle
  mediaTint: boolean
}

function block(tokens: Tokens) {
  return `:root{color-scheme:${isDark(tokens) ? 'dark' : 'light'};${TOKENS.map((n) => `--color-${n}:${tokens[n]};`).join('')}}`
}

export function apply(look: Look) {
  const single = look.mode === 'SINGLE'
  const t: Painted = {
    css: single ? block(look.light) : `${block(look.light)}@media (prefers-color-scheme: dark){${block(look.dark)}}`,
    style: look.style.toLowerCase(),
    media: look.mediaTint,
    metas: single
      ? [[look.light.canvas, null]]
      : [
          [look.light.canvas, '(prefers-color-scheme: light)'],
          [look.dark.canvas, '(prefers-color-scheme: dark)'],
        ],
  }
  paint(t)
  try {
    localStorage.setItem(KEY, JSON.stringify(t))
  } catch {
    // Private browsing and the like: it's only a head start.
  }
  listeners.forEach((l) => l())
}

/** A colour token's value as the page has it right now, e.g. for drawing on a canvas. */
export const cssColor = (name: TokenName | `media-${string}`) =>
  getComputedStyle(document.documentElement).getPropertyValue(`--color-${name}`).trim()

const listeners = new Set<() => void>()
/** Calls `l` whenever a theme is put on the page. */
export const onThemeChange = (l: () => void) => (listeners.add(l), () => void listeners.delete(l))

/** The component style in use. */
export const useStyle = () =>
  useSyncExternalStore(
    onThemeChange,
    () => document.documentElement.dataset.style ?? 'layered',
    () => 'layered',
  )

export function useCorners() {
  return CORNERS[useStyle()] ?? CORNERS.layered
}
