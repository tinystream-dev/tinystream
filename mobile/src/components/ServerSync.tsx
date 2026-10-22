// SPDX-License-Identifier: AGPL-3.0-or-later
// Keeps the app in step with the active server: its look, and who's signed
// in as they are now (a new picture shows in the switcher too), or that no
// one is anymore.

import { type Tokens, toRecord } from '@tinystream/shared/theme'
import { useQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import { AppearanceQuery, useStatus } from '../queries'
import { forget, update, useServers } from '../servers'
import { useSession } from '../session'
import { useTheme } from '../theme/ThemeProvider'

export function ServerSync() {
  const { server, api } = useSession()
  const { setLook } = useTheme()
  const { data: status } = useStatus()
  const { data: appearance } = useQuery({
    queryKey: ['appearance', status?.viewer?.id ?? null],
    queryFn: async () => (await api.request(AppearanceQuery)).appearance,
    enabled: !!status,
  })

  const viewer = status?.viewer
  const signedIn = useServers().signedIn.has(server.id)
  useEffect(() => {
    if (viewer) update(server.id, { username: viewer.username, avatar: viewer.avatar ?? null })
    // The server answers a token it no longer takes as if no one were signed in.
    else if (status && signedIn) forget(server.id)
  }, [server.id, status, viewer, signedIn])

  useEffect(() => {
    if (!appearance) return
    setLook({
      mode: appearance.mode,
      light: toRecord(appearance.light.palette.tokens) as Tokens,
      dark: toRecord(appearance.dark.palette.tokens) as Tokens,
      style: appearance.style,
      mediaTint: appearance.mediaTint,
    })
  }, [appearance])

  return null
}
