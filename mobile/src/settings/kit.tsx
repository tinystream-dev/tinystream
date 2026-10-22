// SPDX-License-Identifier: AGPL-3.0-or-later
// What the settings screens share (web's SettingsKit): refetching what
// config.toml decides after a save, and drafts of a section with a floating
// Save and Discard while they differ from what's saved.

import { useMutation, useQueryClient } from '@tanstack/react-query'
import { type ReactNode, useState } from 'react'
import { Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { toast } from '../components/Feedback'
import { Button } from '../components/ui'
import { Glass } from '../effects/Glass'
import { Squircle } from '../effects/Squircle'
import { useMotion } from '../effects/motion'
import { graphql } from '../gql'
import type { ConfigPatch } from '../gql/graphql'
import { type Settings, useMe, useSettings } from '../queries'
import { useApi } from '../session'

const SaveSection = graphql(`
  mutation SaveSection($patch: ConfigPatch!) {
    updateSettings(patch: $patch) {
      raw
    }
  }
`)

/** Refetches everything that depends on config.toml after a save. */
export function useSaved() {
  const qc = useQueryClient()
  return () => {
    void qc.invalidateQueries({ queryKey: ['settings'] })
    void qc.invalidateQueries({ queryKey: ['libraries'] })
    void qc.invalidateQueries({ queryKey: ['auth'] })
  }
}

export const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v)) as T

/** A draft of part of config.toml (`pick` takes it out of the settings), saved as one patch. */
export function useDraft<T>(pick: (s: Settings) => T, patch: (draft: T) => ConfigPatch) {
  const api = useApi()
  const { data } = useSettings()
  const saved = useSaved()
  const [draft, setDraft] = useState<T | null>(null)
  const [error, setError] = useState<string | null>(null)
  // The draft starts once, from the first settings to arrive.
  if (data && !draft) setDraft(clone(pick(data)))
  const save = useMutation({
    mutationFn: () => api.request(SaveSection, { patch: patch(draft!) }),
    onSuccess: () => {
      setError(null)
      toast({ title: 'Saved to config.toml', tone: 'ok' })
      saved()
    },
    onError: (e) => setError((e as Error).message),
  })
  const set = (fn: (d: T) => void) => {
    const d = clone(draft!)
    fn(d)
    setDraft(d)
  }
  const dirty = !!data && !!draft && JSON.stringify(draft) !== JSON.stringify(pick(data))
  const bar = (
    <SaveBar dirty={dirty} error={error} busy={save.isPending} onSave={() => save.mutate()} onDiscard={() => data && (setDraft(clone(pick(data))), setError(null))} />
  )
  return { data, draft, set, bar, dirty }
}

/** Save and Discard, floating above the tab bar while there's something to save. */
export function SaveBar({ dirty, error, busy, onSave, onDiscard, label = 'Save changes' }: { dirty: boolean; error?: string | null; busy?: boolean; onSave: () => void; onDiscard: () => void; label?: string }) {
  const motion = useMotion()
  if (!dirty && !error) return null
  return (
    <Animated.View style={motion.rise}>
      <Glass radius={18} style={{ padding: 10, gap: 8 }}>
        {error && <Text className="font-sans px-1 text-sm text-danger">{error}</Text>}
        <View className="flex-row gap-2">
          <Button className="flex-1" variant="plain" disabled={!dirty} onPress={onDiscard}>
            Discard
          </Button>
          <Button className="flex-1" variant="primary" disabled={!dirty || busy} onPress={onSave}>
            {label}
          </Button>
        </View>
      </Glass>
    </Animated.View>
  )
}

/** Numbers typed into a field; anything else is dropped. */
export const num = (s: string) => Number(s.replace(/[^\d.]/g, '')) || 0

export const csv = (v: string[]) => v.join(', ')
export const uncsv = (s: string) =>
  s
    .split(',')
    .map((x) => x.trim())
    .filter(Boolean)

/** A field's label over it, in a group's padding. */
export function Labeled({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <View className="gap-1.5 px-4 py-3">
      <Text className="font-sans text-[13px] text-ink-2">{label}</Text>
      {children}
      {hint ? typeof hint === 'string' ? <Text className="font-sans text-xs leading-4 text-ink-3">{hint}</Text> : hint : null}
    </View>
  )
}

/** config.toml has a mistake: the last edit didn't apply. */
export function ConfigError() {
  const me = useMe()
  const { data } = useSettings()
  if (!me?.isAdmin || !data?.error) return null
  return (
    <Squircle radius={14} edge className="gap-2 bg-danger/10 p-4">
      <Text className="font-sans text-sm font-medium text-danger">config.toml has a mistake, so your last edit wasn't applied</Text>
      <Text className="text-xs leading-5 text-ink-2" style={{ fontFamily: 'monospace' }}>
        {data.error}
      </Text>
    </Squircle>
  )
}
