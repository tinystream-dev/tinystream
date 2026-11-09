// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { QueryClient, QueryObserver } from '@tanstack/react-query'
import { refreshLibrary } from '../src/lib/live'

test('a library change refetches the mounted title even when its cached data is fresh', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } })
  client.setQueryData(['item', 1], ['episode 1'])
  const observer = new QueryObserver(client, { queryKey: ['item', 1], queryFn: async () => ['episode 1', 'episode 2'] })
  const stop = observer.subscribe(() => {})
  await refreshLibrary(client, 'Shows')
  expect(observer.getCurrentResult().data).toEqual(['episode 1', 'episode 2'])
  stop()
  client.clear()
})

test('reconciliation after a connection gap refetches active data', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } })
  client.setQueryData(['library', 'Shows'], ['old'])
  const observer = new QueryObserver(client, { queryKey: ['library', 'Shows'], queryFn: async () => ['new'] })
  const stop = observer.subscribe(() => {})
  await client.invalidateQueries()
  expect(observer.getCurrentResult().data).toEqual(['new'])
  stop()
  client.clear()
})
