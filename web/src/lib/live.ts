// SPDX-License-Identifier: AGPL-3.0-or-later

import type { QueryClient } from '@tanstack/react-query'

export function refreshLibrary(client: QueryClient, library: string) {
  return Promise.all([
    ['home'], ['libraries'], ['library', library], ['item'], ['series'], ['overview'],
    ['settings'], ['skipped'], ['music'], ['books', 'library', library], ['books', 'home'], ['books', 'book'],
  ].map(queryKey => client.invalidateQueries({ queryKey })))
}
