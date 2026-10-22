// SPDX-License-Identifier: AGPL-3.0-or-later
// Haptics by what happened, not by how it feels: each event is tuned once, in
// HapticsModule.kt, from the vibrator's primitives where it has them.

import { requireNativeModule } from 'expo'

export type HapticEvent =
  | 'tick'
  | 'edge'
  | 'press'
  | 'longPressStart'
  | 'longPressOpen'
  | 'toggleOn'
  | 'toggleOff'
  | 'pullProgress'
  | 'pullTrigger'
  | 'dismissThreshold'
  | 'tab'
  | 'reveal'
  | 'texture'
  | 'success'
  | 'error'

export type HapticIntensity = 'off' | 'subtle' | 'full'

const Haptics = requireNativeModule<{
  play(event: HapticEvent, amount: number | null): void
  intensity(): HapticIntensity
  setIntensity(level: HapticIntensity): void
}>('TinystreamHaptics')

/** Plays an event. `amount` (0–1) is how far along something is, e.g. a pull to refresh. */
export function haptic(event: HapticEvent, amount?: number) {
  Haptics.play(event, amount ?? null)
}

/** Off, Subtle (softer, and no texture while dragging) or Full; kept per device. */
export const hapticIntensity = () => Haptics.intensity()
export const setHapticIntensity = (level: HapticIntensity) => Haptics.setIntensity(level)
