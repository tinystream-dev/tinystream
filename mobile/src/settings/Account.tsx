// SPDX-License-Identifier: AGPL-3.0-or-later
// Your account (web's Account tab): your picture, passwords for music apps,
// passkeys and your password.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import * as Clipboard from 'expo-clipboard'
import { Copy, KeyRound, Trash2 } from 'lucide-react-native'
import { useState } from 'react'
import { Text, View } from 'react-native'
import { AvatarPicker } from '../components/AvatarPicker'
import { ask, toast, toastError } from '../components/Feedback'
import { Button, Divider, ErrorText, Group, IconButton, Input, ListRow } from '../components/ui'
import { Squircle } from '../effects/Squircle'
import { graphql } from '../gql'
import { useMe } from '../queries'
import { useApi, useSession } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { Labeled } from './kit'

const PasskeysQuery = graphql(`
  query Passkeys {
    viewer {
      passkeys {
        id
        name
        createdAt
        lastUsed
      }
    }
  }
`)

const DeletePasskey = graphql(`
  mutation DeletePasskey($id: Int!) {
    deletePasskey(id: $id) {
      id
    }
  }
`)

const ChangePassword = graphql(`
  mutation ChangePassword($current: String!, $new: String!) {
    changePassword(current: $current, new: $new)
  }
`)

const AppPasswordsQuery = graphql(`
  query AppPasswords {
    appPasswords {
      id
      name
      createdAt
      lastUsed
      client
    }
  }
`)

const CreateAppPassword = graphql(`
  mutation CreateAppPassword($name: String!) {
    createAppPassword(name: $name) {
      secret
      password {
        id
        name
      }
    }
  }
`)

const DeleteAppPassword = graphql(`
  mutation DeleteAppPassword($id: Int!) {
    deleteAppPassword(id: $id)
  }
`)

const date = (s: number) => new Date(s * 1000).toLocaleDateString(undefined, { dateStyle: 'medium' })

export function Account() {
  const me = useMe()
  return (
    <>
      {me && (
        <Group title="Profile">
          <View className="p-4">
            <AvatarPicker user={me} />
          </View>
        </Group>
      )}
      <AppPasswords />
      <Passkeys />
      <Password />
    </>
  )
}

function Copyable({ label, value }: { label: string; value: string }) {
  const { tokens } = useTheme()
  return (
    <View className="flex-row items-center gap-3">
      <Text className="font-sans w-20 text-sm text-ink-3">{label}</Text>
      <Text className="flex-1 text-[13px] text-ink" style={{ fontFamily: 'monospace' }} numberOfLines={1}>
        {value}
      </Text>
      <IconButton
        label={`Copy ${label.toLowerCase()}`}
        size={34}
        onPress={() => void Clipboard.setStringAsync(value).then(() => toast({ title: 'Copied', tone: 'ok' }))}
      >
        <Copy size={15} color={tokens['ink-2']} />
      </IconButton>
    </View>
  )
}

/** A password per music app (Subsonic), each one taken back on its own. */
export function AppPasswords() {
  const api = useApi()
  const me = useMe()
  const { server } = useSession()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const { data } = useQuery({ queryKey: ['app-passwords'], queryFn: async () => (await api.request(AppPasswordsQuery)).appPasswords })
  const [name, setName] = useState('')
  const [made, setMade] = useState<{ name: string; secret: string } | null>(null)
  const create = useMutation({
    mutationFn: () => api.request(CreateAppPassword, { name }),
    onSuccess: ({ createAppPassword: r }) => {
      setMade({ name: r.password.name, secret: r.secret })
      setName('')
      void qc.invalidateQueries({ queryKey: ['app-passwords'] })
    },
    onError: toastError,
  })
  const remove = useMutation({ mutationFn: (id: number) => api.request(DeleteAppPassword, { id }), onSuccess: () => void qc.invalidateQueries({ queryKey: ['app-passwords'] }) })
  return (
    <Group
      title="Music apps"
      description="Apps that speak Subsonic, like Feishin, Symfonium or Tempo, play your music from here. Give each its own password; you can take it back any time without changing yours."
    >
      {made && (
        <Squircle radius={12} edge className="m-3 gap-1 bg-panel p-3">
          <Text className="font-sans mb-1 text-sm font-medium text-ink">Sign in to {made.name} with:</Text>
          <Copyable label="Server" value={server.url} />
          <Copyable label="Username" value={me?.username ?? ''} />
          <Copyable label="Password" value={made.secret} />
          <Text className="font-sans pt-1 text-xs text-ink-3">This is the only time the password is shown.</Text>
        </Squircle>
      )}
      {data?.length === 0 && <Text className="font-sans px-4 pt-3 text-sm text-ink-3">No app passwords yet.</Text>}
      {data?.map((p) => (
        <View key={p.id}>
          <ListRow
            label={p.name}
            hint={`Made ${date(p.createdAt)}${p.lastUsed ? ` · last used ${date(p.lastUsed)}${p.client ? ` by ${p.client}` : ''}` : ' · never used'}`}
            right={
              <IconButton
                label="Revoke"
                onPress={async () =>
                  (await ask({ title: `Revoke “${p.name}”?`, body: 'Whatever uses it is signed out.', confirm: 'Revoke', danger: true })) && remove.mutate(p.id)
                }
              >
                <Trash2 size={17} color={tokens['ink-3']} />
              </IconButton>
            }
          />
          <Divider />
        </View>
      ))}
      <View className="flex-row gap-2 p-3">
        <View className="flex-1">
          <Input value={name} onChangeText={setName} placeholder="Which app, e.g. Symfonium on my phone" />
        </View>
        <Button className="self-center" disabled={!name.trim() || create.isPending} onPress={() => create.mutate()}>
          New
        </Button>
      </View>
    </Group>
  )
}

function Passkeys() {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const { data: keys } = useQuery({ queryKey: ['passkeys'], queryFn: async () => (await api.request(PasskeysQuery)).viewer?.passkeys ?? [] })
  const remove = useMutation({ mutationFn: (id: number) => api.request(DeletePasskey, { id }), onSuccess: () => void qc.invalidateQueries({ queryKey: ['passkeys'] }), onError: toastError })
  return (
    <Group title="Passkeys" description="Adding a passkey from the app comes with passkey sign-in; add them in tinystream in a browser for now.">
      {keys?.length === 0 && <Text className="font-sans p-4 text-sm text-ink-3">No passkeys yet.</Text>}
      {keys?.map((k, i) => (
        <View key={k.id}>
          {i > 0 && <Divider inset={52} />}
          <ListRow
            icon={<KeyRound size={18} color={tokens['ink-3']} />}
            label={k.name}
            hint={`Added ${date(k.createdAt)}${k.lastUsed ? `, last used ${date(k.lastUsed)}` : ''}`}
            right={
              <IconButton label={`Remove ${k.name}`} onPress={async () => (await ask({ title: `Remove ${k.name}?`, confirm: 'Remove', danger: true })) && remove.mutate(k.id)}>
                <Trash2 size={17} color={tokens['ink-3']} />
              </IconButton>
            }
          />
        </View>
      ))}
    </Group>
  )
}

function Password() {
  const api = useApi()
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const change = useMutation({
    mutationFn: () => api.request(ChangePassword, { current, new: next }),
    onSuccess: () => {
      setCurrent('')
      setNext('')
      toast({ title: 'Password changed', tone: 'ok' })
    },
  })
  return (
    <Group title="Password">
      <Labeled label="Current password">
        <Input secureTextEntry autoComplete="current-password" value={current} onChangeText={setCurrent} />
      </Labeled>
      <Labeled label="New password">
        <Input secureTextEntry autoComplete="new-password" value={next} onChangeText={setNext} />
      </Labeled>
      <View className="gap-2 p-3">
        {change.error && <ErrorText>{(change.error as Error).message}</ErrorText>}
        <Button variant="primary" disabled={!current || !next || change.isPending} onPress={() => change.mutate()}>
          Change password
        </Button>
      </View>
    </Group>
  )
}
