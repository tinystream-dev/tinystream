// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, mock, test } from 'bun:test'

mock.module('expo-router', () => ({ useRouter: () => null }))
mock.module('expo-router/react-navigation', () => ({ NavigationContext: null }))
const { place, tabOf } = await import('./nav')

test('web links land on the app’s screens', () => {
  expect(place('/title/12')).toEqual({ path: 'title/12' })
  expect(place('/watch/7')).toEqual({ path: 'watch/7', root: true })
  expect(place('/library/My%20Anime')).toEqual({ tab: '(library)', path: 'library?name=My%20Anime' })
  expect(place('/settings?tab=users')).toEqual({ tab: '(profile)', path: 'settings/users' })
  expect(place('/requests')).toEqual({ path: 'requests' })
  expect(place('/together/abc')).toBeNull()
})

test('screens know their tab', () => {
  expect(tabOf(['(tabs)', '(search)', 'title', '[id]'])).toBe('(search)')
  expect(tabOf(['watch'])).toBe('(home)')
})
