// SPDX-License-Identifier: AGPL-3.0-or-later
// What someone types to reach a server ("tiny.lan", "10.0.0.2:3000",
// "https://tv.example.com/") as the origins to try, https first.

/** `scheme://host[:port]`, lowercased, without a path or trailing slash. */
export type Origin = string

/** The origins to try, in order; empty when it can't be a server's address. */
export function origins(input: string): Origin[] {
  const m = /^\s*(?:([a-z][a-z0-9+.-]*):\/\/)?([^/?#\s]+)[^\s]*\s*$/i.exec(input)
  if (!m) return []
  const scheme = m[1]?.toLowerCase()
  const host = m[2].replace(/^[^@]*@/, '').toLowerCase()
  if (!/^(\[[0-9a-f:.]+\]|[a-z0-9.-]+)(:\d{1,5})?$/.test(host) || host.startsWith('.') || host.startsWith(':')) return []
  if (scheme === 'https' || scheme === 'http') return [`${scheme}://${host}`]
  if (scheme) return []
  return [`https://${host}`, `http://${host}`]
}

export const host = (origin: Origin) => origin.replace(/^[a-z]+:\/\//, '')

export const encrypted = (origin: Origin) => origin.startsWith('https://')

/** A path the server gave (`/api/images/…`) on that server; full URLs stay as they are. */
export function resolve(origin: Origin, path: string) {
  return /^[a-z][a-z0-9+.-]*:/i.test(path) ? path : `${origin}${path.startsWith('/') ? '' : '/'}${path}`
}
