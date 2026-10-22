// SPDX-License-Identifier: AGPL-3.0-or-later
// Where the server is: the first thing the app asks, and "Add server".

import { useRouter } from 'expo-router'
import { useState } from 'react'
import { Text, View } from 'react-native'
import { haptic } from '../modules/haptics'
import { Frame, Unencrypted } from '../src/components/Frame'
import { Button, ErrorText, Field, Input, Panel, Spinner } from '../src/components/ui'
import { origins } from '../src/lib/address'
import { probe } from '../src/lib/probe'
import { useServers } from '../src/servers'

export default function Connect() {
  const router = useRouter()
  const { servers } = useServers()
  const [address, setAddress] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const tries = origins(address)
  const plain = tries.length === 1 && tries[0].startsWith('http://')

  const submit = async () => {
    if (busy || !address.trim()) return
    setBusy(true)
    setError(null)
    try {
      const found = await probe(address)
      router.push({ pathname: '/sign-in', params: { url: found.origin } })
    } catch (e) {
      haptic('error')
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Frame title={servers.length ? 'Add a server' : 'Welcome to tinystream'} subtitle="Where do you open tinystream?" back>
      <Panel className="gap-4 p-5">
        <Field label="Server address" hint="Like tinystream.lan:3000 or https://tv.example.com">
          <Input
            value={address}
            onChangeText={setAddress}
            onSubmitEditing={submit}
            autoFocus
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="url"
            returnKeyType="go"
            textContentType="URL"
            placeholder="tinystream.lan:3000"
          />
        </Field>
        {plain && <Unencrypted />}
        {error && <ErrorText>{error}</ErrorText>}
        <Button variant="primary" size="lg" disabled={busy || !address.trim()} onPress={submit}>
          {busy ? (
            <View className="flex-row items-center gap-2.5">
              <Spinner />
              <Text className="font-sans text-[15px] font-medium text-on-accent">Connecting…</Text>
            </View>
          ) : (
            'Connect'
          )}
        </Button>
      </Panel>
    </Frame>
  )
}
