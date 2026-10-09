// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { graphql } from '../gql'
import { request } from '../lib/api'
import { useMe } from '../lib/hooks'
import { Button, Field, Input, Panel, Spinner } from '../components/ui'

export const Route = createFileRoute('/invite')({ component: InvitePage })
const InviteQuery = graphql(`
  query Invite($token: String!) { invite(token: $token) { expiresAt remainingUses } }
`)
const AcceptInvite = graphql(`
  mutation AcceptInvite($token: String!, $username: String!, $password: String!) {
    acceptInvite(token: $token, username: $username, password: $password) { user { id } }
  }
`)

export function InvitePage() {
  const me = useMe()
  const qc = useQueryClient()
  const navigate = useNavigate()
  const [token, setToken] = useState<string | null>(null)
  useEffect(() => {
    const read = () => setToken(window.location.hash.slice(1))
    read()
    window.addEventListener('hashchange', read)
    return () => window.removeEventListener('hashchange', read)
  }, [])
  const { data, isPending, error } = useQuery({
    queryKey: ['invite', token],
    queryFn: async () => (await request(InviteQuery, { token: token! })).invite,
    enabled: !!token && !me,
    retry: false,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
  })
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [confirmation, setConfirmation] = useState('')
  const accept = useMutation({
    mutationFn: () => request(AcceptInvite, { token: token!, username: username.trim(), password }),
    onSuccess: async () => {
      window.history.replaceState(window.history.state, '', window.location.pathname)
      qc.removeQueries({ queryKey: ['invite'] })
      await qc.invalidateQueries()
      await navigate({ to: '/' })
    },
    onError: () => { void qc.invalidateQueries({ queryKey: ['invite', token] }) },
  })
  const valid = username.trim().length > 0 && password.length >= 4 && password === confirmation
  return (
    <div className="grid min-h-dvh place-items-center px-5">
      <div className="w-full max-w-sm">
        <h1 className="mb-5 text-lg font-semibold">Join tinystream</h1>
        <Panel className="p-5">
          {me ? <p className="text-sm text-ink-2">You’re already signed in. Sign out to create an account with this invite.</p>
            : token === null || (!!token && isPending) ? <Spinner />
            : error ? <p role="alert" className="text-sm text-danger">{error.message}</p>
            : !token || !data ? <p role="alert" className="text-sm text-ink-2">This invite is invalid, expired, revoked, or has reached its use limit. Ask an admin for a new link.</p>
            : (
              <form className="space-y-4" onSubmit={(e) => { e.preventDefault(); if (valid && !accept.isPending) accept.mutate() }}>
                <p className="text-sm text-ink-2">Choose your username and password. This invite has {data.remainingUses} {data.remainingUses === 1 ? 'use' : 'uses'} left and expires {new Date(data.expiresAt * 1000).toLocaleString()}.</p>
                <Field label="Username"><Input autoFocus autoComplete="username" maxLength={64} value={username} onChange={(e) => setUsername(e.target.value)} required /></Field>
                <Field label="Password"><Input type="password" autoComplete="new-password" minLength={4} value={password} onChange={(e) => setPassword(e.target.value)} required /></Field>
                <Field label="Confirm password"><Input type="password" autoComplete="new-password" value={confirmation} onChange={(e) => setConfirmation(e.target.value)} required /></Field>
                {confirmation && password !== confirmation && <p className="text-xs text-danger">Passwords don’t match.</p>}
                {accept.error && <p role="alert" className="text-sm text-danger">{accept.error.message}</p>}
                <Button type="submit" variant="primary" disabled={!valid || accept.isPending}>{accept.isPending ? 'Creating account…' : 'Create account'}</Button>
              </form>
            )}
        </Panel>
        <Button className="mt-4" variant="plain" onClick={() => void navigate({ to: '/' })}>{me ? 'Go to home' : 'Back to sign in'}</Button>
      </div>
    </div>
  )
}
