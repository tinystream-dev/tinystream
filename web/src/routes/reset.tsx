// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import { type FormEvent, useEffect, useState } from 'react'
import { Frame } from '../components/Login'
import { Button, Field, Input, Panel } from '../components/ui'
import { graphql } from '../gql'
import { request } from '../lib/api'

export const Route = createFileRoute('/reset')({ component: ResetPage })

const ResetQuery = graphql(`
  query PasswordReset($token: String!) {
    passwordReset(token: $token)
  }
`)

const RequestReset = graphql(`
  mutation RequestPasswordReset($who: String!) {
    requestPasswordReset(who: $who)
  }
`)

const ResetPassword = graphql(`
  mutation ResetPassword($token: String!, $password: String!) {
    resetPassword(token: $token, password: $password) {
      user {
        id
      }
    }
  }
`)

/** /reset asks for a link; /reset#token, where the link goes, picks the new password. */
function ResetPage() {
  const [token, setToken] = useState<string | null>(null)
  useEffect(() => {
    const read = () => setToken(location.hash.slice(1))
    read()
    window.addEventListener('hashchange', read)
    return () => window.removeEventListener('hashchange', read)
  }, [])
  if (token === null) return null
  return token ? <NewPassword token={token} /> : <Ask />
}

function Back() {
  const navigate = useNavigate()
  return (
    <div className="mt-5 flex justify-center">
      <Button variant="plain" onClick={() => void navigate({ to: '/' })}>
        <ArrowLeft className="size-3.5" /> Back to sign in
      </Button>
    </div>
  )
}

function Ask() {
  const [who, setWho] = useState('')
  const [sent, setSent] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await request(RequestReset, { who: who.trim() })
      setSent(true)
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  if (sent)
    return (
      <Frame title="Check your email" subtitle="If that account has an email address, a link is on its way. It works for an hour.">
        <Back />
      </Frame>
    )

  return (
    <Frame title="Forgot your password?" subtitle="We’ll email you a link to pick a new one.">
      <Panel radius={20} className="p-5">
        <form onSubmit={submit} className="space-y-4">
          <Field label="Username or email">
            <Input autoFocus autoComplete="username" value={who} onChange={(e) => setWho(e.target.value)} />
          </Field>
          {error && <p className="text-sm text-danger">{error}</p>}
          <Button variant="primary" size="lg" className="w-full" disabled={busy || !who.trim()}>
            Send link
          </Button>
        </form>
      </Panel>
      <Back />
    </Frame>
  )
}

function NewPassword({ token }: { token: string }) {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const { data: username, isPending } = useQuery({
    queryKey: ['password-reset', token],
    queryFn: async () => (await request(ResetQuery, { token })).passwordReset,
    retry: false,
  })
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await request(ResetPassword, { token, password })
      history.replaceState(history.state, '', location.pathname)
      qc.removeQueries({ queryKey: ['password-reset'] })
      await qc.invalidateQueries()
      void navigate({ to: '/' })
    } catch (err) {
      setError((err as Error).message)
      setBusy(false)
    }
  }

  if (isPending) return null

  return (
    <Frame
      title={username ? 'Pick a new password' : 'This link doesn’t work anymore'}
      subtitle={username ? `For ${username}. You’ll be signed out everywhere else.` : 'Links work once, for an hour.'}
    >
      {username ? (
        <Panel radius={20} className="p-5">
          <form onSubmit={submit} className="space-y-4">
            <input type="text" autoComplete="username" value={username} readOnly hidden />
            <Field label="New password">
              <Input type="password" autoFocus autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
            </Field>
            {error && <p className="text-sm text-danger">{error}</p>}
            <Button variant="primary" size="lg" className="w-full" disabled={busy || !password}>
              Change password
            </Button>
          </form>
        </Panel>
      ) : (
        <Button variant="primary" size="lg" className="w-full" onClick={() => (location.hash = '')}>
          Send a new link
        </Button>
      )}
      <Back />
    </Frame>
  )
}
