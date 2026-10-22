// SPDX-License-Identifier: AGPL-3.0-or-later
// Talking to one tinystream server's GraphQL API, as web/src/lib/graphql.ts
// does, but from afar: queries and mutations over HTTP (with file uploads)
// and subscriptions over a WebSocket, signed in with a bearer token.

import { type Client, createClient } from 'graphql-ws'
import type { TypedDocumentString } from '../gql/graphql'
import type { Origin } from './address'

const ENDPOINT = '/api/graphql'

export class ApiError extends Error {
  constructor(
    /** The server's `extensions.code`: UNAUTHENTICATED, FORBIDDEN, NOT_FOUND, CONFLICT, BAD_REQUEST… */
    public code: string,
    message: string,
  ) {
    super(message)
  }
}

/** A file on the device, as React Native's FormData sends one. */
export type UploadFile = { uri: string; name: string; type: string }

const isFile = (v: unknown): v is UploadFile =>
  !!v && typeof v === 'object' && typeof (v as UploadFile).uri === 'string' && typeof (v as UploadFile).type === 'string'

type Result<T> = { data?: T | null; errors?: { message: string; extensions?: { code?: string } }[] }

/** Files anywhere in `value`, by where they are, e.g. `variables.image`. */
function findFiles(value: unknown, path: string, out: Map<string, UploadFile>): unknown {
  if (isFile(value)) {
    out.set(path, value)
    return null
  }
  if (Array.isArray(value)) return value.map((v, i) => findFiles(v, `${path}.${i}`, out))
  if (value && typeof value === 'object')
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, findFiles(v, `${path}.${k}`, out)]))
  return value
}

/** The body for a request: JSON, or multipart when it carries files. */
function body(query: string, variables: unknown): { body: BodyInit; headers: Record<string, string> } {
  const files = new Map<string, UploadFile>()
  const vars = findFiles(variables ?? {}, 'variables', files)
  if (!files.size) return { body: JSON.stringify({ query, variables }), headers: { 'content-type': 'application/json' } }
  // https://github.com/jaydenseric/graphql-multipart-request-spec
  const form = new FormData()
  form.append('operations', JSON.stringify({ query, variables: vars }))
  form.append('map', JSON.stringify(Object.fromEntries([...files.keys()].map((path, i) => [i, [path]]))))
  ;[...files.values()].forEach((file, i) => form.append(String(i), file as unknown as Blob))
  return { body: form, headers: {} }
}

export type Api = {
  origin: Origin
  /** Runs a query or mutation; any error the server reports is thrown. */
  request<R, V>(document: TypedDocumentString<R, V>, ...[variables]: V extends Record<string, never> ? [] : [V]): Promise<R>
  /** Follows a subscription; returns how to stop. `onConnected` runs on every (re)connection. */
  subscribe<R, V>(document: TypedDocumentString<R, V>, variables: V, onData: (data: R) => void, onConnected?: () => void): () => void
  /** Drops the live connection. */
  dispose(): void
}

/** The header that signs a request to the server in, for media and images too. */
export const authHeaders = (token: string | null): Record<string, string> => (token ? { authorization: `Bearer ${token}` } : {})

/**
 * The API of the server at `origin`, as whoever `token` belongs to (or no
 * one). `onUnauthenticated` runs when the server no longer takes the token.
 */
export function connect(origin: Origin, token: string | null, onUnauthenticated?: () => void): Api {
  let client: Client | null = null

  const fail = (error: ApiError) => {
    if (error.code === 'UNAUTHENTICATED' && token) onUnauthenticated?.()
    return error
  }

  const ws = () =>
    (client ??= createClient({
      url: origin.replace(/^http/, 'ws') + ENDPOINT,
      connectionParams: () => (token ? { token } : {}),
      lazy: true,
      retryAttempts: Infinity,
      shouldRetry: () => true,
    }))

  return {
    origin,
    async request(document, ...[variables]) {
      const init = body(document.toString(), variables)
      const res = await fetch(origin + ENDPOINT, {
        method: 'POST',
        credentials: 'omit',
        body: init.body,
        headers: { ...init.headers, ...authHeaders(token) },
      })
      let result: Result<never>
      try {
        result = await res.json()
      } catch {
        throw fail(new ApiError(res.status === 401 ? 'UNAUTHENTICATED' : 'INTERNAL', res.statusText || 'the server sent something unexpected'))
      }
      const error = result.errors?.[0]
      if (error) throw fail(new ApiError(error.extensions?.code ?? 'INTERNAL', error.message))
      if (!result.data) throw new ApiError('INTERNAL', res.statusText || 'the server sent nothing back')
      return result.data
    },
    subscribe(document, variables, onData, onConnected) {
      const c = ws()
      const off = onConnected ? c.on('connected', onConnected) : () => {}
      const stop = c.subscribe<never>(
        { query: document.toString(), variables: variables as Record<string, unknown> },
        {
          next: (r) => r.data && onData(r.data),
          error: (e) => console.warn('live updates:', e),
          complete: () => {},
        },
      )
      return () => {
        off()
        stop()
      }
    },
    dispose() {
      void client?.dispose()
      client = null
    },
  }
}
