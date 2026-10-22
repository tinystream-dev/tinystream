// SPDX-License-Identifier: AGPL-3.0-or-later
// Signing in to a server just found (`url`), or again to the active one: the
// first-run setup, the profile pictures or a username, as the server is set
// up for, with web's words (web/src/components/Login.tsx).

import { Redirect, useLocalSearchParams, useRouter } from 'expo-router'
import { ArrowLeft, KeyRound, Server, UserRound } from 'lucide-react-native'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Pressable, type TextInput, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { haptic } from '../modules/haptics'
import { Avatar } from '../src/components/Avatar'
import { Frame, Unencrypted } from '../src/components/Frame'
import { Sheet } from '../src/components/Sheet'
import { useSwitcher } from '../src/components/Switcher'
import { Button, ErrorText, Field, Input, Panel, Spinner } from '../src/components/ui'
import { useMotion } from '../src/effects/motion'
import { graphql } from '../src/gql'
import type { SignInStyle } from '../src/gql/graphql'
import { type Origin, encrypted, host } from '../src/lib/address'
import { type Api, connect } from '../src/lib/graphql'
import { save, servers, useServers } from '../src/servers'
import { ConnectionProvider } from '../src/session'
import { read, write } from '../src/storage'
import { useTheme } from '../src/theme/ThemeProvider'

const ServerQuery = graphql(`
  query SignInServer {
    server {
      setupRequired
      signInStyle
    }
  }
`)

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
        ...Person
      }
      token
    }
  }
`)

const SignInMutation = graphql(`
  mutation SignIn($username: String, $profile: String, $password: String!) {
    signIn(username: $username, profile: $profile, password: $password) {
      user {
        ...Person
      }
      token
    }
  }
`)

type Profile = { key: string; avatar?: string | null; passkey: boolean }
type SignedIn = { user: { id: number; username: string; avatar?: string | null }; token: string }
type Last = { username?: string; profile?: string }

const lastKey = (origin: Origin) => `signIn.last.${origin}`

/** The server's message, or what went wrong getting to it. */
const message = (e: unknown) =>
  e instanceof TypeError ? 'Couldn’t reach the server. Check your connection and try again.' : (e as Error).message

export default function SignIn() {
  const { url } = useLocalSearchParams<{ url?: string }>()
  const { active } = useServers()
  const origin = url ?? active?.url
  if (!origin) return <Redirect href="/connect" />
  return (
    <ConnectionProvider value={{ origin, token: null }}>
      <SignInTo key={origin} origin={origin} again={!url} />
    </ConnectionProvider>
  )
}

function SignInTo({ origin, again }: { origin: Origin; again: boolean }) {
  const router = useRouter()
  const { switchTo, open } = useSwitcher()
  const { tokens } = useTheme()
  const api = useMemo(() => connect(origin, null), [origin])
  const [server, setServer] = useState<{ setupRequired: boolean; signInStyle: SignInStyle } | null>(null)
  const [profiles, setProfiles] = useState<Profile[] | null>(null)
  const [failed, setFailed] = useState<string | null>(null)
  const [byName, setByName] = useState(false)

  const load = useCallback(async () => {
    setFailed(null)
    try {
      const { server } = await api.request(ServerQuery)
      const list = !server.setupRequired && server.signInStyle === 'PROFILES' ? (await api.request(ProfilesQuery)).signInProfiles : []
      setProfiles(list)
      setServer(server)
    } catch (e) {
      setFailed(message(e))
    }
  }, [api])

  useEffect(() => {
    void load()
  }, [load])

  const done = (signed: SignedIn, last: Last) => {
    write(lastKey(origin), { ...read<Last>(lastKey(origin)), ...last })
    const id = save(origin, signed.user, signed.token)
    if (id !== servers().active?.id) return switchTo(id)
    haptic('success')
    router.dismissTo('/')
  }

  const others = again && (
    <View className="mt-5 items-center">
      <Button variant="plain" icon={<Server size={15} color={tokens['ink-2']} />} onPress={open}>
        Use another server
      </Button>
    </View>
  )

  if (failed)
    return (
      <Frame title="Sign in" subtitle={host(origin)} back>
        <Panel className="gap-4 p-5">
          <ErrorText>{failed}</ErrorText>
          <Button size="lg" onPress={load}>
            Try again
          </Button>
        </Panel>
        {others}
      </Frame>
    )

  if (!server || !profiles)
    return (
      <Frame back>
        <Spinner size="large" />
      </Frame>
    )

  const setup = server.setupRequired
  if (setup || byName || !profiles.length)
    return (
      <Frame title={setup ? 'Welcome to tinystream' : 'Sign in'} subtitle={setup ? 'Create the admin account.' : host(origin)} back>
        <UsernameForm api={api} origin={origin} setup={setup} onDone={done} onBack={profiles.length ? () => setByName(false) : undefined} />
        {others}
      </Frame>
    )

  return (
    <Frame title="Who’s watching?" subtitle={host(origin)} back>
      <Picker api={api} origin={origin} profiles={profiles} onDone={done} />
      <View className="mt-6 items-center">
        <Button variant="plain" onPress={() => setByName(true)}>
          Sign in with a username instead
        </Button>
      </View>
      {others}
    </Frame>
  )
}

function Face({ profile, size }: { profile: Profile; size: number }) {
  const { tokens } = useTheme()
  return (
    <Avatar
      user={{ username: profile.key, avatar: profile.avatar }}
      size={size}
      fallback={<UserRound strokeWidth={1.5} size={size * 0.46} color={tokens['media-ink']} />}
    />
  )
}

/** Everyone's picture, in an order that says nothing about who they are; the password in a sheet. */
function Picker({ api, origin, profiles, onDone }: { api: Api; origin: Origin; profiles: Profile[]; onDone: (s: SignedIn, last: Last) => void }) {
  const [chosen, setChosen] = useState<Profile | null>(null)
  const [open, setOpen] = useState(false)
  const motion = useMotion()
  const { tokens } = useTheme()
  const last = read<Last>(lastKey(origin))?.profile

  return (
    <>
      <View className="flex-row flex-wrap justify-center gap-6 py-2">
        {profiles.map((p, i) => (
          <Animated.View key={p.key} style={motion.developIn(i)}>
            <Pressable
              accessibilityLabel={`Profile ${i + 1}${p.passkey ? ', signs in with a passkey' : ''}`}
              onPress={() => {
                haptic('press')
                setChosen(p)
                setOpen(true)
              }}
              style={({ pressed }) => ({ transform: [{ scale: pressed ? 0.94 : 1 }] })}
            >
              <View style={{ borderRadius: 999, borderWidth: p.key === last ? 2 : 0, borderColor: tokens['line-strong'], padding: p.key === last ? 3 : 5 }}>
                <Face profile={p} size={92} />
              </View>
              {p.passkey && (
                <View className="absolute bottom-1 right-1 h-7 w-7 items-center justify-center rounded-full bg-panel" style={{ borderWidth: 3, borderColor: tokens.canvas }}>
                  <KeyRound size={13} color={tokens['ink-2']} />
                </View>
              )}
            </Pressable>
          </Animated.View>
        ))}
      </View>
      <Sheet open={open} onClose={() => setOpen(false)}>
        {chosen && <Unlock api={api} origin={origin} profile={chosen} onBack={() => setOpen(false)} onDone={onDone} />}
      </Sheet>
    </>
  )
}

function Unlock({
  api,
  origin,
  profile,
  onBack,
  onDone,
}: {
  api: Api
  origin: Origin
  profile: Profile
  onBack: () => void
  onDone: (s: SignedIn, last: Last) => void
}) {
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const { tokens } = useTheme()
  const input = useRef<TextInput>(null)

  // autoFocus doesn't take in a sheet that's still coming up.
  useEffect(() => {
    const t = setTimeout(() => input.current?.focus(), 300)
    return () => clearTimeout(t)
  }, [])

  const submit = async () => {
    if (busy || !password) return
    setBusy(true)
    setError(null)
    try {
      const { signIn } = await api.request(SignInMutation, { profile: profile.key, password })
      onDone(signIn, { profile: profile.key })
    } catch (e) {
      haptic('error')
      setError(message(e))
      setBusy(false)
    }
  }

  return (
    <View className="items-center pt-2">
      <Face profile={profile} size={112} />
      <Button variant="plain" size="sm" className="mb-4 mt-2" icon={<ArrowLeft size={14} color={tokens['ink-2']} />} onPress={onBack}>
        Not you?
      </Button>
      <View className="w-full gap-4">
        {!encrypted(origin) && <Unencrypted />}
        <Field label="Password">
          <Input
            ref={input}
            secureTextEntry
            autoComplete="current-password"
            textContentType="password"
            returnKeyType="go"
            value={password}
            onChangeText={setPassword}
            onSubmitEditing={submit}
          />
        </Field>
        {error && <ErrorText>{error}</ErrorText>}
        <Button variant="primary" size="lg" disabled={busy || !password} onPress={submit}>
          Sign in
        </Button>
        {/* #50 adds "Use your passkey" here, for profiles with one. */}
      </View>
    </View>
  )
}

function UsernameForm({
  api,
  origin,
  setup,
  onDone,
  onBack,
}: {
  api: Api
  origin: Origin
  setup: boolean
  onDone: (s: SignedIn, last: Last) => void
  onBack?: () => void
}) {
  const [username, setUsername] = useState(() => read<Last>(lastKey(origin))?.username ?? '')
  const passwordInput = useRef<TextInput>(null)
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const { tokens } = useTheme()

  const submit = async () => {
    if (busy || !username || !password) return
    setBusy(true)
    setError(null)
    try {
      const signed = setup ? (await api.request(Setup, { username, password })).setup : (await api.request(SignInMutation, { username, password })).signIn
      onDone(signed, { username: username.trim() })
    } catch (e) {
      haptic('error')
      setError(message(e))
      setBusy(false)
    }
  }

  return (
    <>
      <Panel className="gap-4 p-5">
        {!encrypted(origin) && <Unencrypted />}
        <Field label="Username">
          <Input
            autoFocus={!username}
            autoCapitalize="none"
            autoCorrect={false}
            autoComplete="username"
            textContentType="username"
            returnKeyType="next"
            submitBehavior="submit"
            value={username}
            onChangeText={setUsername}
            onSubmitEditing={() => passwordInput.current?.focus()}
          />
        </Field>
        <Field label="Password">
          <Input
            ref={passwordInput}
            secureTextEntry
            autoFocus={!!username}
            autoComplete={setup ? 'new-password' : 'current-password'}
            textContentType={setup ? 'newPassword' : 'password'}
            returnKeyType="go"
            value={password}
            onChangeText={setPassword}
            onSubmitEditing={submit}
          />
        </Field>
        {error && <ErrorText>{error}</ErrorText>}
        <Button variant="primary" size="lg" disabled={busy || !username || !password} onPress={submit}>
          {setup ? 'Create account' : 'Sign in'}
        </Button>
        {/* #50 adds "Sign in with a passkey" here. */}
      </Panel>
      {onBack && (
        <View className="mt-5 items-center">
          <Button variant="plain" icon={<ArrowLeft size={14} color={tokens['ink-2']} />} onPress={onBack}>
            Back to pictures
          </Button>
        </View>
      )}
    </>
  )
}
