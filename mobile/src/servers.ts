// SPDX-License-Identifier: AGPL-3.0-or-later
// The servers this phone knows, who's signed in to each, and which one is in
// use. The list is kept in plain storage; tokens only in the secure store.
// Each server has its own query cache, so one's data never shows on another.

import { QueryClient } from '@tanstack/react-query'
import * as SecureStore from 'expo-secure-store'
import { useSyncExternalStore } from 'react'
import { type TurboModule, TurboModuleRegistry } from 'react-native'
import { graphql } from './gql'
import type { Origin } from './lib/address'
import { type Api, connect } from './lib/graphql'
import { read, write } from './storage'

export type Server = {
  id: string
  url: Origin
  name: string
  userId: number
  username: string
  /** The picture's path on the server. */
  avatar: string | null
}

export type Servers = {
  servers: Server[]
  active: Server | null
  /** Ids of the servers there's a token for. */
  signedIn: ReadonlySet<string>
}

const LIST = 'servers'
const ACTIVE = 'servers.active'
const tokenKey = (id: string) => `token.${id}`

const SignOut = graphql(`
  mutation SignOut {
    signOut
  }
`)

// Android keeps the cookies servers set, and sends them on the live
// connection before any token: only tokens should say who's who.
const Networking = TurboModuleRegistry.get<TurboModule & { clearCookies(done: (ok: boolean) => void): void }>('Networking')

const tokens = new Map<string, string>()
let list = read<Server[]>(LIST) ?? []
for (const s of list) {
  const token = SecureStore.getItem(tokenKey(s.id))
  if (token) tokens.set(s.id, token)
}
let activeId = read<string>(ACTIVE)

let snapshot: Servers
const listeners = new Set<() => void>()

function changed() {
  snapshot = {
    servers: list,
    active: list.find((s) => s.id === activeId) ?? null,
    signedIn: new Set(tokens.keys()),
  }
  write(LIST, list)
  write(ACTIVE, activeId)
  listeners.forEach((l) => l())
}
changed()

const subscribe = (l: () => void) => (listeners.add(l), () => void listeners.delete(l))

export const servers = () => snapshot

export function useServers() {
  return useSyncExternalStore(subscribe, servers)
}

export const tokenOf = (id: string) => tokens.get(id) ?? null

const apis = new Map<string, Api>()
const caches = new Map<string, QueryClient>()

/** The server's API, as whoever is signed in to it. */
export function apiOf(server: Server) {
  let api = apis.get(server.id)
  if (!api) {
    api = connect(server.url, tokenOf(server.id), () => forget(server.id))
    apis.set(server.id, api)
  }
  return api
}

export function cacheOf(id: string) {
  let cache = caches.get(id)
  if (!cache) {
    cache = new QueryClient({ defaultOptions: { queries: { staleTime: 30_000, retry: 1 } } })
    caches.set(id, cache)
  }
  return cache
}

/** Drops the connection and everything fetched, as whoever was signed in. */
function reset(id: string) {
  apis.get(id)?.dispose()
  apis.delete(id)
  caches.get(id)?.clear()
}

const switchListeners = new Set<() => void>()

/** Runs before the app moves to another server: playback and rooms stop there. */
export const onSwitch = (l: () => void) => (switchListeners.add(l), () => void switchListeners.delete(l))

export function activate(id: string | null) {
  if (id === activeId) return
  switchListeners.forEach((l) => l())
  activeId = id
  changed()
}

const uuid = () =>
  'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0
    return (c === 'x' ? r : (r & 3) | 8).toString(16)
  })

/** Keeps a sign-in: the same person on the same server replaces their old one. Returns the server's id. */
export function save(url: Origin, user: { id: number; username: string; avatar?: string | null }, token: string) {
  const old = list.find((s) => s.url === url && s.userId === user.id)
  const id = old?.id ?? uuid()
  const name = url.replace(/^[a-z]+:\/\//, '')
  const server: Server = { id, url, name, userId: user.id, username: user.username, avatar: user.avatar ?? null }
  reset(id)
  Networking?.clearCookies(() => {})
  SecureStore.setItem(tokenKey(id), token)
  tokens.set(id, token)
  list = old ? list.map((s) => (s.id === id ? server : s)) : [...list, server]
  changed()
  return id
}

/** Takes in what the server says about whoever's signed in now (a new picture, a new name). */
export function update(id: string, patch: Partial<Pick<Server, 'username' | 'avatar'>>) {
  const old = list.find((s) => s.id === id)
  if (!old || Object.entries(patch).every(([k, v]) => old[k as keyof Server] === v)) return
  list = list.map((s) => (s.id === id ? { ...s, ...patch } : s))
  changed()
}

/** Forgets the token, as when the server stopped taking it. */
export function forget(id: string) {
  if (!tokens.has(id)) return
  reset(id)
  tokens.delete(id)
  void SecureStore.deleteItemAsync(tokenKey(id))
  changed()
}

/** Signs out on the server too, when it can be reached. */
export async function signOut(id: string) {
  const server = list.find((s) => s.id === id)
  if (server && tokens.has(id)) await apiOf(server).request(SignOut).catch((e) => console.warn('sign out:', e))
  forget(id)
}

export async function remove(id: string) {
  await signOut(id)
  caches.delete(id)
  list = list.filter((s) => s.id !== id)
  if (activeId === id) activate(list[0]?.id ?? null)
  changed()
}
