// SPDX-License-Identifier: AGPL-3.0-or-later
// The instance switcher: a sheet of the saved servers, opened from Profile
// or a long press on the Profile tab. Moving to another server fades
// the whole app out and back in, on that server.

import { useRouter } from 'expo-router'
import { Check, LogOut, Plus, Trash2 } from 'lucide-react-native'
import { type ReactNode, createContext, useCallback, useContext, useMemo, useRef, useState } from 'react'
import { Alert, Pressable, Text, View } from 'react-native'
import ReanimatedSwipeable, { type SwipeableMethods } from 'react-native-gesture-handler/ReanimatedSwipeable'
import Animated, { type SharedValue, useAnimatedStyle, useReducedMotion, useSharedValue, withTiming } from 'react-native-reanimated'
import { scheduleOnRN } from 'react-native-worklets'
import { haptic } from '../../modules/haptics'
import { activate, remove, type Server, signOut, tokenOf, useServers } from '../servers'
import { ConnectionProvider } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { Avatar } from './Avatar'
import { Sheet } from './Sheet'
import { Button } from './ui'

type Switcher = {
  open: () => void
  /** Moves the app to another server, with the cross-fade. */
  switchTo: (id: string) => void
}

const SwitcherContext = createContext<Switcher | null>(null)

export function useSwitcher() {
  const switcher = useContext(SwitcherContext)
  if (!switcher) throw new Error('useSwitcher needs a SwitcherProvider above it')
  return switcher
}

const OUT = 140
const IN = 220

export function SwitcherProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false)
  const reduced = useReducedMotion()
  const opacity = useSharedValue(1)
  const faded = useAnimatedStyle(() => ({ opacity: opacity.value }))

  const switchTo = useCallback(
    (id: string) => {
      haptic('success')
      const land = () => {
        activate(id)
        opacity.value = withTiming(1, { duration: IN })
      }
      if (reduced) return activate(id)
      opacity.value = withTiming(0, { duration: OUT }, (done) => done && scheduleOnRN(land))
    },
    [opacity, reduced],
  )

  const value = useMemo<Switcher>(() => ({ open: () => setOpen(true), switchTo }), [switchTo])

  return (
    <SwitcherContext.Provider value={value}>
      <Animated.View style={[{ flex: 1 }, faded]}>{children}</Animated.View>
      <Sheet open={open} onClose={() => setOpen(false)}>
        <ServerList onDone={() => setOpen(false)} switchTo={switchTo} />
      </Sheet>
    </SwitcherContext.Provider>
  )
}

function ServerList({ onDone, switchTo }: { onDone: () => void; switchTo: (id: string) => void }) {
  const router = useRouter()
  const { tokens } = useTheme()
  const { servers, active, signedIn } = useServers()

  return (
    <View>
      <Text className="font-sans mb-3 mt-1 text-lg font-semibold text-ink">Servers</Text>
      <View className="gap-1">
        {servers.map((s) => (
          <Row
            key={s.id}
            server={s}
            active={s.id === active?.id}
            signedIn={signedIn.has(s.id)}
            onPress={() => {
              onDone()
              if (s.id !== active?.id) switchTo(s.id)
            }}
          />
        ))}
      </View>
      <Button
        className="mt-4"
        size="lg"
        icon={<Plus size={18} color={tokens.ink} />}
        onPress={() => {
          onDone()
          router.push('/connect')
        }}
      >
        Add server
      </Button>
    </View>
  )
}

function Row({ server, active, signedIn, onPress }: { server: Server; active: boolean; signedIn: boolean; onPress: () => void }) {
  const { tokens } = useTheme()
  const swipe = useRef<SwipeableMethods>(null)

  const confirmRemove = () =>
    Alert.alert(`Remove ${server.name}?`, 'You’ll have to connect and sign in again to use it.', [
      { text: 'Cancel', style: 'cancel', onPress: () => swipe.current?.close() },
      { text: 'Remove', style: 'destructive', onPress: () => void remove(server.id) },
    ])

  const actions = (progress: SharedValue<number>) => (
    <Actions progress={progress}>
      {signedIn && (
        <Action
          label="Sign out"
          icon={<LogOut size={18} color={tokens.ink} />}
          onPress={() => {
            swipe.current?.close()
            void signOut(server.id)
          }}
        />
      )}
      <Action label="Remove" danger icon={<Trash2 size={18} color={tokens.danger} />} onPress={confirmRemove} />
    </Actions>
  )

  return (
    <ReanimatedSwipeable
      ref={swipe}
      renderRightActions={actions}
      overshootRight={false}
      rightThreshold={40}
      onSwipeableWillOpen={() => haptic('reveal')}
    >
      <Pressable
        onPress={onPress}
        onLongPress={() => {
          haptic('longPressOpen')
          swipe.current?.openRight()
        }}
        className="flex-row items-center gap-3 rounded-2xl bg-float px-2 py-2.5 active:bg-press"
      >
        <ConnectionProvider value={{ origin: server.url, token: tokenOf(server.id) }}>
          <View style={{ opacity: signedIn ? 1 : 0.5 }}>
            <Avatar user={server} size={44} />
          </View>
        </ConnectionProvider>
        <View className="flex-1">
          <Text className="font-sans text-[15px] font-medium text-ink" numberOfLines={1}>
            {server.name}
          </Text>
          <Text className="font-sans text-[13px] text-ink-2" numberOfLines={1}>
            {server.username} · {signedIn ? 'signed in' : 'signed out'}
          </Text>
          <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
            {server.url}
          </Text>
        </View>
        {active && <Check size={20} color={tokens.ink} />}
      </Pressable>
    </ReanimatedSwipeable>
  )
}

/** The row's actions, sliding in from the edge as the row is pulled aside (they sit behind it). */
function Actions({ progress, children }: { progress: SharedValue<number>; children: ReactNode }) {
  const width = useSharedValue(0)
  const slide = useAnimatedStyle(() => ({
    opacity: progress.value,
    transform: [{ translateX: (1 - progress.value) * width.value }],
  }))
  return (
    <Animated.View onLayout={(e) => (width.value = e.nativeEvent.layout.width)} className="flex-row items-stretch pl-2" style={slide}>
      {children}
    </Animated.View>
  )
}

function Action({ label, icon, danger = false, onPress }: { label: string; icon: ReactNode; danger?: boolean; onPress: () => void }) {
  return (
    <Pressable
      onPress={() => {
        haptic('press')
        onPress()
      }}
      className={`ml-1.5 w-20 items-center justify-center gap-1 rounded-2xl ${danger ? 'bg-danger/15 active:bg-danger/25' : 'bg-raised active:bg-press'}`}
    >
      {icon}
      <Text className={`font-sans text-xs font-medium ${danger ? 'text-danger' : 'text-ink'}`}>{label}</Text>
    </Pressable>
  )
}
