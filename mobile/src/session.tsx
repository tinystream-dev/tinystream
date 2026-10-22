// SPDX-License-Identifier: AGPL-3.0-or-later
// The server screens talk to: the active one, or (while signing in) one
// that isn't saved yet.

import { type ReactNode, createContext, useContext } from 'react'
import type { Origin } from './lib/address'
import type { Api } from './lib/graphql'
import type { Server } from './servers'

export type Connection = { origin: Origin; token: string | null }

const ConnectionContext = createContext<Connection | null>(null)
const SessionContext = createContext<{ server: Server; api: Api } | null>(null)

/** Where server paths (pictures, artwork) resolve, and who fetches them. */
export function ConnectionProvider({ value, children }: { value: Connection; children: ReactNode }) {
  return <ConnectionContext.Provider value={value}>{children}</ConnectionContext.Provider>
}

export function SessionProvider({ server, api, token, children }: { server: Server; api: Api; token: string | null; children: ReactNode }) {
  return (
    <SessionContext.Provider value={{ server, api }}>
      <ConnectionProvider value={{ origin: server.url, token }}>{children}</ConnectionProvider>
    </SessionContext.Provider>
  )
}

export function useConnection() {
  return useContext(ConnectionContext)
}

/** The active server and its API; only for screens shown signed in. */
export function useSession() {
  const session = useContext(SessionContext)
  if (!session) throw new Error('useSession needs an active server')
  return session
}

export const useApi = () => useSession().api
