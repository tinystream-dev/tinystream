// SPDX-License-Identifier: AGPL-3.0-or-later
// Small things kept on the device, read synchronously so the first frame can
// already use them. Secrets go in expo-secure-store instead.

import Storage from 'expo-sqlite/kv-store'
import { useSyncExternalStore } from 'react'

export function read<T>(key: string): T | null {
  try {
    const value = Storage.getItemSync(key)
    return value == null ? null : (JSON.parse(value) as T)
  } catch {
    return null
  }
}

export function write(key: string, value: unknown) {
  Storage.setItemSync(key, JSON.stringify(value))
}

/** A stored value that components follow as it changes. */
export function stored<T>(key: string, fallback: T) {
  let value = read<T>(key) ?? fallback
  const listeners = new Set<() => void>()
  const subscribe = (l: () => void) => (listeners.add(l), () => void listeners.delete(l))
  return {
    get: () => value,
    set(next: T) {
      value = next
      write(key, next)
      listeners.forEach((l) => l())
    },
    use: () => useSyncExternalStore(subscribe, () => value),
  }
}
