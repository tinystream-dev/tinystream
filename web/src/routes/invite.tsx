// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import { type FormEvent, useEffect, useState } from 'react'
import { Frame } from '../components/Login'
import { Button, Field, Input, Panel } from '../components/ui'
import { graphql } from '../gql'
import { request } from '../lib/api'
import { useMe } from '../lib/hooks'

export const Route = createFileRoute('/invite')({ component: InvitePage })

const InviteQuery = graphql(`
  query Invite($token: String!) {
    invite(token: $token) {
      expiresAt
    }
  }
`)

const AcceptInvite = graphql(`
  mutation AcceptInvite($token: String!, $username: String!, $password: String!) {
    acceptInvite(token: $token, username: $username, password: $password) {
      user {
        id
      }
    }
  }
`)

/** /invite#token: the token stays in the fragment, so it's never sent along with a request for the page. */
function InvitePage() {
  const me = useMe()
  const qc = useQueryClient()
  const navigate = useNavigate()
  const [token, setToken] = useState<string | null>(null)
  useEffect(() => {
    const read = () => setToken(location.hash.slice(1))
    read()
    window.addEventListener('hashchange', read)
    return () => window.removeEventListener('hashchange', read)
  }, [])
  const { data, isPending } = useQuery({
    queryKey: ['invite', token],
    queryFn: async () => (await request(InviteQuery, { token: token! })).invite,
    enabled: !!token && !me,
    retry: false,
  })
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const home = () => void navigate({ to: '/' })

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await request(AcceptInvite, { token: token!, username, password })
      history.replaceState(history.state, '', location.pathname)
      qc.removeQueries({ queryKey: ['invite'] })
      await qc.invalidateQueries()
      home()
    } catch (err) {
      setError((err as Error).message)
      setBusy(false)
    }
  }

  if (me)
    return (
      <Frame title="You’re signed in" subtitle="Sign out to make a new account with this invite.">
        <Button variant="primary" size="lg" className="w-full" onClick={home}>
          Go home
        </Button>
      </Frame>
    )
  if (token === null || (token && isPending)) return null

  return (
    <Frame title={data ? 'Join tinystream' : 'This invite doesn’t work anymore'} subtitle={data ? 'Pick a username and password.' : 'Ask whoever sent it for a new one.'}>
      {data && (
        <Panel radius={20} className="p-5">
          <form onSubmit={submit} className="space-y-4">
            <Field label="Username">
              <Input autoFocus autoComplete="username" maxLength={64} value={username} onChange={(e) => setUsername(e.target.value)} />
            </Field>
            <Field label="Password">
              <Input type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
            </Field>
            {error && <p className="text-sm text-danger">{error}</p>}
            <Button variant="primary" size="lg" className="w-full" disabled={busy || !username.trim() || !password}>
              Create account
            </Button>
          </form>
        </Panel>
      )}
      <div className="mt-5 flex justify-center">
        <Button variant="plain" onClick={home}>
          <ArrowLeft className="size-3.5" /> Sign in instead
        </Button>
      </div>
    </Frame>
  )
}
