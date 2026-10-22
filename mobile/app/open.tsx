// SPDX-License-Identifier: AGPL-3.0-or-later
// Where a notification leads (tinystream://open?server=…&link=…): onto its
// server, then to what it's about.

import { useLocalSearchParams, useRouter } from 'expo-router'
import { useEffect } from 'react'
import { followLater } from '../src/nav'
import { activate, servers } from '../src/servers'

export default function Open() {
  const { server, link } = useLocalSearchParams<{ server?: string; link?: string }>()
  const router = useRouter()
  useEffect(() => {
    const known = !!server && servers().servers.some((s) => s.id === server)
    if (known) followLater(link ?? null)
    if (known && servers().active?.id !== server) activate(server)
    // Back to where the app was, so the link goes on top of it rather than starting the tab over.
    else if (router.canGoBack()) router.back()
    else router.replace('/')
  }, [server, link, router])
  return null
}
