// SPDX-License-Identifier: AGPL-3.0-or-later
// Getting around: each tab has its own stack, and the screens they share
// (a title, the calendar, settings…) open in the tab they're opened from.
// Links the server makes (notifications) are web paths; `place` maps them.

import { type Href, useRouter } from 'expo-router'
import { NavigationContext } from 'expo-router/react-navigation'
import { createContext, useCallback, useContext, useEffect, useRef, useSyncExternalStore } from 'react'

/** The tabs' stacks that hold the shared screens. */
export type Tab = '(home)' | '(search)' | '(library)' | '(profile)'
const TABS: Tab[] = ['(home)', '(search)', '(library)', '(profile)']

/** A shared screen's path, e.g. `title/5` or `settings/appearance`. */
export type Place = string

/** Where a web path goes in the app, or null when it's somewhere the app doesn't have. */
export function place(link: string): { tab?: Tab; path: Place; root?: boolean } | null {
  const url = new URL(link, 'http://x')
  const parts = url.pathname.split('/').filter(Boolean).map(decodeURIComponent)
  const q = url.searchParams
  switch (parts[0]) {
    case undefined:
      return { tab: '(home)', path: '' }
    case 'title':
      return parts[1] ? { path: `title/${parts[1]}` } : null
    case 'watch':
      return parts[1] ? { path: `watch/${parts[1]}`, root: true } : null
    case 'library':
      return { tab: '(library)', path: parts[1] ? `library?name=${encodeURIComponent(parts[1])}` : 'library' }
    case 'discover':
      return { tab: '(search)', path: q.get('q') ? `search?q=${encodeURIComponent(q.get('q')!)}` : 'search' }
    case 'settings':
      return { tab: '(profile)', path: q.get('tab') ? `settings/${q.get('tab')}` : 'settings' }
    case 'calendar':
    case 'requests':
    case 'wanted':
    case 'downloads':
    case 'notifications':
    case 'clips':
      return { path: parts[0] }
    default:
      return null
  }
}

/** The tab a screen is in, from the route's segments. */
export function tabOf(segments: readonly string[]): Tab {
  return TABS.find((t) => segments.includes(t)) ?? '(home)'
}

/** The tab whose stack a screen is in, from its `TabStack`; null outside the tabs. */
export const TabContext = createContext<Tab | null>(null)

/**
 * Runs `to` unless it's from a screen in a tab that's already been left: a
 * second tap landing while the screen it opened is still on its way would
 * open it again.
 */
function useLeave() {
  const tab = useContext(TabContext)
  const screen = useContext(NavigationContext)
  return useCallback(
    (to: () => void) => {
      if (tab && screen && !screen.isFocused()) return
      to()
    },
    [tab, screen],
  )
}

/** Opens shared screens in this tab's stack (or `tab`'s). */
export function useGo() {
  const router = useRouter()
  const current = useContext(TabContext) ?? '(home)'
  const leave = useLeave()
  return useCallback(
    (path: Place, tab?: Tab) => {
      const into = tab ?? current
      leave(() => router.push(`/(tabs)/${into}/${path}` as Href))
    },
    [router, current, leave],
  )
}

/** Opens the player on a video. */
export function useWatch() {
  const router = useRouter()
  const leave = useLeave()
  return useCallback((id: number) => leave(() => router.push(`/watch/${id}` as Href)), [router, leave])
}

/** Follows a server link (a notification's) wherever it leads in the app. */
export function useFollow() {
  const router = useRouter()
  const go = useGo()
  const leave = useLeave()
  return useCallback(
    (link: string | null | undefined) => {
      const to = link ? place(link) : null
      if (!to) return false
      if (to.root) leave(() => router.push(`/${to.path}` as Href))
      // A tab's own first screen is gone to; anything else is pushed onto a stack.
      else if (to.tab && /^(|library|search)(\?|$)/.test(to.path)) router.navigate(`/(tabs)/${to.tab}/${to.path}` as Href)
      else go(to.path, to.tab)
      return true
    },
    [router, go, leave],
  )
}

/** A link to follow once the app is on the right server and showing its tabs (from a notification). */
let pending: string | null = null
const pendingListeners = new Set<() => void>()

export function followLater(link: string | null) {
  pending = link
  pendingListeners.forEach((l) => l())
}

/** Follows the pending link, if any, as soon as there is one. */
export function usePendingLink() {
  const follow = useFollow()
  const latest = useRef(follow)
  latest.current = follow
  const link = useSyncExternalStore(
    (l) => (pendingListeners.add(l), () => void pendingListeners.delete(l)),
    () => pending,
  )
  useEffect(() => {
    if (!link) return
    pending = null
    // After the tabs have settled on their first screens; it's followed even if they re-render meanwhile.
    setTimeout(() => latest.current(link), 50)
  }, [link])
}
