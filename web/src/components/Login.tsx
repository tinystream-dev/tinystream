// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { ArrowLeft, KeyRound, UserRound } from 'lucide-react'
import { type FormEvent, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { flushSync } from 'react-dom'
import { graphql } from '../gql'
import { request } from '../lib/api'
import { useEmailOn } from '../lib/hooks'
import { getCredential } from '../lib/webauthn'
import { Avatar } from './Avatar'
import { Logo } from './Logo'
import { Button, Field, Input, Panel, Spinner } from './ui'

const LAST_USER = 'tinystream.lastUsername'
const LAST_PROFILE = 'tinystream.lastProfile'

const ProfilesQuery = graphql(`
  query SignInProfiles {
    signInProfiles {
      key
      avatar
      passkey
    }
  }
`)

const Setup = graphql(`
  mutation Setup($username: String!, $password: String!) {
    setup(username: $username, password: $password) {
      user {
        id
      }
    }
  }
`)

const SignIn = graphql(`
  mutation SignIn($username: String, $profile: String, $password: String!) {
    signIn(username: $username, profile: $profile, password: $password) {
      user {
        id
      }
    }
  }
`)

const StartPasskey = graphql(`
  mutation StartPasskeySignIn($username: String, $profile: String) {
    startPasskeySignIn(username: $username, profile: $profile) {
      challenge
      options
    }
  }
`)

const FinishPasskey = graphql(`
  mutation FinishPasskeySignIn($challenge: String!, $credential: JSON!) {
    finishPasskeySignIn(challenge: $challenge, credential: $credential) {
      user {
        id
      }
    }
  }
`)

type Profile = { key: string; avatar?: string | null; passkey: boolean }

/** Closing the browser's passkey prompt, or us closing it for them. */
const dismissed = (e: unknown) => e instanceof Error && (e.name === 'NotAllowedError' || e.name === 'AbortError')

async function signInWithPasskey(who: { username?: string; profile?: string }, signal?: AbortSignal) {
  const { challenge, options } = (await request(StartPasskey, who)).startPasskeySignIn
  const credential = await getCredential(options, signal)
  await request(FinishPasskey, { challenge, credential })
}

/** The picked picture grows into place (and back), when the browser can. */
function morph(update: () => void) {
  if (!document.startViewTransition || matchMedia('(prefers-reduced-motion: reduce)').matches) return update()
  document.startViewTransition(update)
}

export function Login({ setup }: { setup: boolean }) {
  const queryClient = useQueryClient()
  const { data: profiles } = useQuery({
    queryKey: ['sign-in-profiles'],
    queryFn: async () => (await request(ProfilesQuery)).signInProfiles,
    enabled: !setup,
  })
  const [byName, setByName] = useState(false)
  const [chosen, setChosen] = useState<Profile | null>(null)
  const [last, setLast] = useState(() => localStorage.getItem(LAST_PROFILE))
  const strip = useRef<HTMLDivElement>(null)

  if (!setup && !profiles) return null
  const done = () => queryClient.invalidateQueries()

  if (setup || byName || !profiles?.length)
    return (
      <Frame title={setup ? 'Welcome to tinystream' : 'Sign in'} subtitle={setup ? 'Create the admin account.' : undefined}>
        <UsernameForm setup={setup} onDone={done} onBack={profiles?.length ? () => setByName(false) : undefined} />
      </Frame>
    )

  const choose = (p: Profile, el: HTMLElement) => {
    for (const b of strip.current?.children ?? []) (b as HTMLElement).style.viewTransitionName = ''
    el.style.viewTransitionName = 'profile'
    morph(() => flushSync(() => setChosen(p)))
  }
  const back = () => {
    const key = chosen!.key
    setLast(key)
    morph(() => {
      flushSync(() => setChosen(null))
      const el = strip.current?.querySelector<HTMLElement>(`[data-profile="${key}"]`)
      if (el) el.style.viewTransitionName = 'profile'
    })
  }

  if (chosen)
    return (
      <Frame>
        <Unlock
          profile={chosen}
          onBack={back}
          onDone={async () => {
            localStorage.setItem(LAST_PROFILE, chosen.key)
            await done()
          }}
        />
      </Frame>
    )

  return (
    <Frame title="Who’s watching?" wide>
      <Picker profiles={profiles} focus={last} strip={strip} onChoose={choose} />
      <div className="mt-6 flex justify-center">
        <Button variant="plain" onClick={() => setByName(true)}>
          Sign in with a username instead
        </Button>
      </div>
    </Frame>
  )
}

export function Frame({ title, subtitle, wide, children }: { title?: string; subtitle?: string; wide?: boolean; children: React.ReactNode }) {
  return (
    <div className="grid min-h-dvh place-items-center overflow-x-clip px-5">
      <div className={`w-full ${wide ? 'max-w-3xl' : 'max-w-sm'}`}>
        {title && (
          <div className={`mb-7 flex items-center gap-3 ${wide ? 'justify-center' : ''}`}>
            <Logo size={32} />
            <div>
              <h1 className="text-lg font-semibold tracking-tight">{title}</h1>
              {subtitle && <p className="text-sm text-ink-2">{subtitle}</p>}
            </div>
          </div>
        )}
        {children}
      </div>
    </div>
  )
}

function Face({ profile, size }: { profile: Profile; size: number }) {
  return (
    <Avatar
      user={{ id: null, username: profile.key, avatar: profile.avatar }}
      size={size}
      fallback={<UserRound strokeWidth={1.5} style={{ width: size * 0.46, height: size * 0.46 }} />}
    />
  )
}

/**
 * Everyone's picture, in an order that says nothing about who they are. On
 * narrow screens they overlap in a strip you swipe through (styles.css).
 */
function Picker({
  profiles,
  focus,
  strip,
  onChoose,
}: {
  profiles: Profile[]
  focus: string | null
  strip: React.RefObject<HTMLDivElement | null>
  onChoose: (p: Profile, el: HTMLElement) => void
}) {
  useLayoutEffect(() => {
    const s = strip.current
    const el = s?.querySelector<HTMLElement>(`[data-profile="${focus}"]`)
    if (s && el) s.scrollLeft = el.offsetLeft - (s.clientWidth - el.offsetWidth) / 2
  }, [strip, focus])

  return (
    <div
      ref={strip}
      className="profile-strip -mx-5 flex overflow-x-auto py-4 max-sm:snap-x max-sm:snap-mandatory sm:mx-0 sm:flex-wrap sm:justify-center sm:gap-7 sm:overflow-visible"
    >
      {profiles.map((p, i) => (
        <button
          key={p.key}
          data-profile={p.key}
          aria-label={`Profile ${i + 1}${p.passkey ? ', signs in with a passkey' : ''}`}
          onClick={(e) => onChoose(p, e.currentTarget)}
          className="group relative shrink-0 snap-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-4 focus-visible:ring-offset-canvas"
        >
          <span className="flex rounded-full shadow-[0_0_0_4px_var(--color-canvas)] transition-transform duration-300 ease-out sm:group-hover:-translate-y-1 sm:group-hover:scale-105">
            <Face profile={p} size={104} />
          </span>
          {p.passkey && (
            <span className="absolute right-0.5 bottom-0.5 grid size-7 place-items-center rounded-full bg-panel text-ink-2 shadow-[0_0_0_3px_var(--color-canvas)]">
              <KeyRound className="size-3.5" />
            </span>
          )}
        </button>
      ))}
    </div>
  )
}

/** The picked person: their passkey first, if they have one, else (or after closing it) their password. */
function Unlock({ profile, onBack, onDone }: { profile: Profile; onBack: () => void; onDone: () => Promise<unknown> }) {
  const [waiting, setWaiting] = useState(profile.passkey)
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const prompt = useRef<AbortController | null>(null)

  const passkey = async () => {
    prompt.current?.abort()
    const ctl = (prompt.current = new AbortController())
    setWaiting(true)
    setError(null)
    try {
      await signInWithPasskey({ profile: profile.key }, ctl.signal)
      await onDone()
    } catch (err) {
      if (ctl.signal.aborted) return
      if (!dismissed(err)) setError((err as Error).message)
      setWaiting(false)
    }
  }
  const usePassword = () => {
    prompt.current?.abort()
    setWaiting(false)
  }

  useEffect(() => {
    if (profile.passkey) void passkey()
    return () => prompt.current?.abort()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profile])

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await request(SignIn, { profile: profile.key, password })
      await onDone()
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex flex-col">
      <h1 className="sr-only">Sign in</h1>
      <span className="flex self-center rounded-full" style={{ viewTransitionName: 'profile' }}>
        <Face profile={profile} size={128} />
      </span>
      <Button variant="plain" size="sm" className="mt-3 mb-6 self-center" onClick={onBack}>
        <ArrowLeft className="size-3.5" /> Not you?
      </Button>
      <Panel radius={20} className="p-5">
        {waiting ? (
          <div className="flex flex-col items-center gap-4 py-2 text-center">
            <Spinner />
            <p className="text-sm text-ink-2">Waiting for your passkey…</p>
            <Button variant="plain" size="sm" onClick={usePassword}>
              Use your password instead
            </Button>
          </div>
        ) : (
          <form onSubmit={submit} className="space-y-4">
            <Field label="Password">
              <Input
                type="password"
                autoFocus
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </Field>
            {error && <p className="text-sm text-danger">{error}</p>}
            <Button variant="primary" size="lg" className="w-full" disabled={busy || !password}>
              Sign in
            </Button>
            {profile.passkey && (
              <Button type="button" size="lg" className="w-full" disabled={busy} onClick={passkey}>
                <KeyRound className="size-4.5" />
                Use your passkey
              </Button>
            )}
            <Forgot />
          </form>
        )}
      </Panel>
    </div>
  )
}

function Forgot() {
  const navigate = useNavigate()
  if (!useEmailOn()) return null
  return (
    <Button type="button" variant="plain" size="sm" className="w-full" onClick={() => void navigate({ to: '/reset' })}>
      Forgot your password?
    </Button>
  )
}

function UsernameForm({ setup, onDone, onBack }: { setup: boolean; onDone: () => Promise<unknown>; onBack?: () => void }) {
  const [username, setUsername] = useState(() => localStorage.getItem(LAST_USER) ?? '')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const done = async () => {
    localStorage.setItem(LAST_USER, username.trim())
    await onDone()
  }

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      if (setup) await request(Setup, { username, password })
      else await request(SignIn, { username, password })
      await done()
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const passkey = async () => {
    if (!username.trim()) {
      setError('Type your username first, then use your passkey.')
      return
    }
    setBusy(true)
    setError(null)
    try {
      await signInWithPasskey({ username })
      await done()
    } catch (err) {
      setError(dismissed(err) ? 'The passkey prompt was closed.' : (err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <Panel radius={20} className="p-5">
        <form onSubmit={submit} className="space-y-4">
          <Field label="Username">
            <Input
              autoFocus={!username}
              autoComplete="username webauthn"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </Field>
          <Field label="Password">
            <Input
              type="password"
              autoFocus={!!username}
              autoComplete={setup ? 'new-password' : 'current-password'}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          {error && <p className="text-sm text-danger">{error}</p>}
          <Button variant="primary" size="lg" className="w-full" disabled={busy || !username || !password}>
            {setup ? 'Create account' : 'Sign in'}
          </Button>
          {!setup && <Forgot />}
        </form>
        {!setup && (
          <>
            <div className="my-4 flex items-center gap-3 text-xs text-ink-3">
              <span className="h-px flex-1 bg-line" />
              or
              <span className="h-px flex-1 bg-line" />
            </div>
            <Button size="lg" className="w-full" disabled={busy} onClick={passkey}>
              <KeyRound className="size-4.5" />
              Sign in with a passkey
            </Button>
          </>
        )}
      </Panel>
      {onBack && (
        <div className="mt-5 flex justify-center">
          <Button variant="plain" onClick={onBack}>
            <ArrowLeft className="size-3.5" /> Back to pictures
          </Button>
        </div>
      )}
    </>
  )
}
