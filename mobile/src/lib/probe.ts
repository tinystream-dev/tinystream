// SPDX-License-Identifier: AGPL-3.0-or-later
// Finding the server someone typed in: each origin in turn, until one
// answers as a tinystream server would.

import type { SignInStyle } from '../gql/graphql'
import { type Origin, origins } from './address'

export type Found = { origin: Origin; version: string; setupRequired: boolean; signInStyle: SignInStyle }

export type ProbeFailure = 'address' | 'unreachable' | 'tls' | 'not-tinystream'

export class ProbeError extends Error {
  constructor(
    public reason: ProbeFailure,
    message: string,
  ) {
    super(message)
  }
}

const QUERY = '{ server { version setupRequired signInStyle } }'
const TIMEOUT = 8000

type Answer = { ok: true; body: string } | { ok: false; error: string; timedOut: boolean }

/**
 * A POST with XMLHttpRequest rather than fetch: on Android, its error
 * keeps the reason (a certificate the phone doesn't trust, say), which
 * fetch drops for "Network request failed".
 */
function post(url: string, body: string): Promise<Answer> {
  return new Promise((done) => {
    const xhr = new XMLHttpRequest()
    xhr.open('POST', url)
    xhr.setRequestHeader('content-type', 'application/json')
    xhr.withCredentials = false
    xhr.timeout = TIMEOUT
    xhr.onload = () => done({ ok: true, body: xhr.responseText })
    xhr.onerror = () => done({ ok: false, error: String(xhr.responseText || ''), timedOut: false })
    xhr.ontimeout = () => done({ ok: false, error: '', timedOut: true })
    xhr.send(body)
  })
}

const tlsError = (error: string) => /ssl|tls|certificate|cert path|trust anchor|handshake/i.test(error)

function server(body: string): Omit<Found, 'origin'> | null {
  try {
    const s = JSON.parse(body)?.data?.server
    return s && typeof s.version === 'string' ? s : null
  } catch {
    return null
  }
}

export async function probe(input: string): Promise<Found> {
  const tries = origins(input)
  if (!tries.length) throw new ProbeError('address', 'That doesn’t look like an address. Try something like tinystream.lan:3000.')
  let failure: ProbeError | null = null
  for (const origin of tries) {
    const answer = await post(`${origin}/api/graphql`, JSON.stringify({ query: QUERY }))
    if (answer.ok) {
      const found = server(answer.body)
      if (found) return { origin, ...found }
      failure ??= new ProbeError('not-tinystream', `Something answered at ${origin}, but it isn’t a tinystream server.`)
      continue
    }
    console.info(`probe ${origin}:`, answer.timedOut ? 'timed out' : answer.error)
    // Plain http on an https port fails its handshake too, so a TLS error only counts when it's an https origin's.
    if (origin.startsWith('https://') && tlsError(answer.error))
      failure ??= new ProbeError('tls', 'The server’s certificate isn’t trusted by this phone. If it’s from your own CA, install the CA in Android’s settings.')
  }
  throw failure ?? new ProbeError('unreachable', 'Couldn’t reach a server there. Check the address, and that this phone is on the same network.')
}
