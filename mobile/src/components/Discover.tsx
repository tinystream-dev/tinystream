// SPDX-License-Identifier: AGPL-3.0-or-later
// Shows that aren't here yet (web's DiscoverCard.tsx): a card for each, and
// the sheet that adds one (for people who manage shows) or asks for it. A
// long press says more about it, as web's hover card does.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { mediaLabels } from '@tinystream/shared/media'
import { Check, Clock, Plus, Send } from 'lucide-react-native'
import { type ReactNode, useState } from 'react'
import { Text, View } from 'react-native'
import { Squircle } from '../effects/Squircle'
import { Tilt } from '../effects/Tilt'
import { graphql } from '../gql'
import type { Monitor } from '../gql/graphql'
import { useGo } from '../nav'
import { type DiscoverResult, useMe, useSettings } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { MonitorPicker } from './downloads'
import { toast } from './Feedback'
import { Img } from './Img'
import { Section } from './Page'
import { Row } from './Row'
import { Sheet } from './Sheet'
import { Button, ErrorText, Field, Input, Select } from './ui'

const AddSeries = graphql(`
  mutation AddSeries($input: NewSeries!) {
    addSeries(input: $input) {
      id
    }
  }
`)

const AiredEpisodes = graphql(`
  query AiredEpisodes($provider: Provider!, $id: String!) {
    airedEpisodes(provider: $provider, id: $id)
  }
`)

const CreateRequest = graphql(`
  mutation CreateRequest($input: NewRequest!) {
    createRequest(input: $input) {
      id
    }
  }
`)

/** What someone can do with a show that isn't here: add it, ask for it, or neither. */
export type Action = 'add' | 'request' | null

export function useAction(): Action {
  const me = useMe()
  return me?.permissions.manageShows ? 'add' : me?.permissions.request ? 'request' : null
}

/** `because` off leaves out the "Like X" caption; the details still say it. */
export function ResultCard({ r, action, onPick, width, because = true }: { r: DiscoverResult; action: Action; onPick: () => void; width: number; because?: boolean }) {
  const go = useGo()
  const { tokens } = useTheme()
  const [broken, setBroken] = useState(false)
  const [details, setDetails] = useState(false)
  if (r.category === 'MOVIES') action = null
  const status = r.titleId
    ? { label: 'In your library', icon: <Check size={11} color={tokens['media-ink']} /> }
    : r.seriesId && r.monitor && r.monitor !== 'NONE'
      ? { label: 'Downloading', icon: <Clock size={11} color={tokens['media-ink']} /> }
      : r.requestState === 'PENDING'
        ? { label: 'Requested', icon: <Clock size={11} color={tokens['media-ink']} /> }
        : null
  const hint = r.titleId
    ? 'In your library'
    : r.category === 'MOVIES'
      ? 'Place a movie folder in your library to watch. Automatic downloads are available for shows.'
      : status
        ? status.label
        : action === 'add'
          ? 'Tap to add'
          : action === 'request'
            ? 'Tap to request'
            : null
  const press = r.titleId ? () => go(`title/${r.titleId}`) : !status && action ? onPick : () => setDetails(true)
  return (
    <View style={{ width }}>
      <Tilt radius={14} style={{ width, aspectRatio: 2 / 3 }} onPress={press} onLongPress={() => setDetails(true)}>
        <View className="flex-1 bg-raised">
          {r.poster && !broken ? (
            <Img src={r.poster} style={{ flex: 1 }} onError={() => setBroken(true)} />
          ) : (
            <View className="flex-1 justify-end bg-panel p-3">
              <Text className="font-sans text-[15px] font-medium text-ink-2" numberOfLines={3}>
                {r.name}
              </Text>
            </View>
          )}
        </View>
        {status && (
          <View className="absolute right-1.5 top-1.5 flex-row items-center gap-1 rounded-md bg-media-shade/65 px-1.5 py-0.5">
            {status.icon}
            <Text className="font-sans text-2xs font-medium text-media-ink">{status.label}</Text>
          </View>
        )}
        {!status && action && (
          <View className="absolute bottom-2 right-2 h-7 w-7 items-center justify-center rounded-full bg-ink">
            {action === 'add' ? <Plus size={16} color={tokens.canvas} /> : <Send size={13} color={tokens.canvas} />}
          </View>
        )}
      </Tilt>
      <Text className="font-sans mt-2 text-[13px] font-medium text-ink" numberOfLines={1}>
        {r.name}
      </Text>
      {r.romaji && (
        <Text className="font-sans text-xs text-ink-2" numberOfLines={1}>
          {r.romaji}
        </Text>
      )}
      <Text className="font-sans text-xs text-ink-3" numberOfLines={1}>
        {mediaLabels[r.category]} · {because && r.because ? `Like ${r.because}` : (r.year ?? 'Upcoming')}
      </Text>
      <Sheet open={details} onClose={() => setDetails(false)}>
        <Heading r={r} lines={12} />
        {r.because && <Text className="font-sans mt-3 text-xs text-ink-2">Because you watched {r.because}</Text>}
        {hint && <Text className="font-sans mt-3 border-t border-line pt-3 text-xs text-ink-3">{hint}</Text>}
        {!status && action && r.category !== 'MOVIES' && (
          <Button
            variant="primary"
            size="lg"
            className="mt-4"
            icon={action === 'add' ? <Plus size={18} color={tokens['on-accent']} /> : <Send size={16} color={tokens['on-accent']} />}
            onPress={() => {
              setDetails(false)
              onPick()
            }}
          >
            {action === 'add' ? 'Add' : 'Request'}
          </Button>
        )}
      </Sheet>
    </View>
  )
}

/** The add or request sheet for a picked show, whichever this person gets. */
export function PickSheet({ r, action, onClose }: { r: DiscoverResult | null; action: Action; onClose: () => void }) {
  const open = !!r && r.category !== 'MOVIES' && !!action
  return (
    <Sheet open={open} onClose={onClose}>
      {r && action === 'add' && <AddForm key={`${r.provider}${r.id}`} r={r} onClose={onClose} />}
      {r && action === 'request' && <RequestForm key={`${r.provider}${r.id}`} r={r} onClose={onClose} />}
    </Sheet>
  )
}

/** A shelf of shows from the provider, each one addable or requestable unless `actionable` is off. */
export function DiscoverShelf({ title, results, aside, actionable = true }: { title: ReactNode; results: DiscoverResult[]; aside?: ReactNode; actionable?: boolean }) {
  const allowed = useAction()
  const action = actionable ? allowed : null
  // "Like X" on every card says nothing when it's the same X each time.
  const varied = new Set(results.map((r) => r.because)).size > 1
  const [picked, setPicked] = useState<DiscoverResult | null>(null)
  return (
    <Section title={title} aside={aside}>
      <Row
        data={results}
        width={124}
        keyOf={(r) => `${r.provider}${r.id}`}
        render={(r) => <ResultCard r={r} width={124} because={varied} action={action} onPick={() => setPicked(r)} />}
      />
      <PickSheet r={picked} action={action} onClose={() => setPicked(null)} />
    </Section>
  )
}

function Heading({ r, lines = 4 }: { r: DiscoverResult; lines?: number }) {
  return (
    <View className="flex-row gap-4">
      <Squircle radius={10} className="bg-panel" style={{ width: 76, aspectRatio: 2 / 3 }}>
        {r.poster && <Img src={r.poster} style={{ flex: 1 }} />}
      </Squircle>
      <View className="min-w-0 flex-1">
        <Text className="font-sans text-[17px] font-semibold leading-snug tracking-tight text-ink">{r.name}</Text>
        {r.romaji && <Text className="font-sans text-sm text-ink-2">{r.romaji}</Text>}
        <Text className="font-sans text-sm text-ink-3">
          {mediaLabels[r.category]} · {r.year ?? 'Upcoming'}
        </Text>
        {r.overview && (
          <Text className="font-sans mt-2 text-[13px] leading-5 text-ink-2" numberOfLines={lines}>
            {r.overview}
          </Text>
        )}
      </View>
    </View>
  )
}

/** Past this many aired episodes, adding with Everything queues a lot of disk. */
const MANY_EPISODES = 100

function AddForm({ r, onClose }: { r: DiscoverResult; onClose: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const go = useGo()
  const { tokens } = useTheme()
  const { data: settings } = useSettings()
  const [library, setLibrary] = useState(r.library)
  const [monitor, setMonitor] = useState<Monitor | null>(null)
  const [profile, setProfile] = useState('')
  const lib = settings?.libraries.find((l) => l.name === library)
  // Settings → Automation decides what's picked to begin with.
  const chosen: Monitor = monitor ?? settings?.automation.defaultMonitor ?? 'NONE'
  const { data: aired } = useQuery({
    queryKey: ['airedEpisodes', r.provider, r.id],
    queryFn: async () => (await api.request(AiredEpisodes, { provider: r.provider, id: r.id })).airedEpisodes,
    staleTime: Infinity,
  })
  const add = useMutation({
    mutationFn: () =>
      api.request(AddSeries, {
        input: { library, provider: r.provider, providerId: r.id, name: r.name, year: r.year, poster: r.poster, overview: r.overview, monitor: chosen, profile: profile || null },
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['discover'] })
      void qc.invalidateQueries({ queryKey: ['calendar'] })
      onClose()
      toast({ title: `Added ${r.name}`, image: r.poster, tone: 'ok', action: chosen === 'NONE' ? undefined : { label: 'See wanted', run: () => go('wanted') } })
    },
  })
  return (
    <View className="gap-4">
      <Heading r={r} />
      <Field label="Library">
        <Select
          title="Library"
          value={library}
          options={(settings?.libraries ?? []).filter((l) => l.metadataProvider === r.provider).map((l) => ({ value: l.name, label: l.name }))}
          onChange={setLibrary}
        />
      </Field>
      {lib && !lib.managed && <Text className="font-sans text-xs leading-4 text-warn">{lib.name} isn't managed; can't download into it.</Text>}
      <Field label="Download">
        <MonitorPicker value={chosen} onChange={setMonitor} />
      </Field>
      {chosen === 'MISSING' && aired !== undefined && aired >= MANY_EPISODES && (
        <Text className="font-sans text-xs leading-4 text-warn">
          {r.name} has {aired} aired episodes, and all of them will be downloaded. Pick New episodes to only get new ones.
        </Text>
      )}
      {settings && settings.profiles.length > 0 && (
        <Field label="Quality">
          <Select
            title="Quality"
            value={profile}
            options={[{ value: '', label: `Library default${lib?.profile ? ` (${lib.profile})` : ''}` }, ...settings.profiles.map((p) => ({ value: p.name, label: p.name }))]}
            onChange={setProfile}
          />
        </Field>
      )}
      {add.error && <ErrorText>{(add.error as Error).message}</ErrorText>}
      <Button variant="primary" size="lg" disabled={add.isPending} onPress={() => add.mutate()} icon={<Plus size={18} color={tokens['on-accent']} />}>
        Add show
      </Button>
    </View>
  )
}

function RequestForm({ r, onClose }: { r: DiscoverResult; onClose: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const [note, setNote] = useState('')
  const send = useMutation({
    mutationFn: () =>
      api.request(CreateRequest, { input: { library: r.library, providerId: r.id, name: r.name, year: r.year, poster: r.poster, overview: r.overview, note: note || null } }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['discover'] })
      void qc.invalidateQueries({ queryKey: ['requests'] })
      onClose()
      toast({ title: `Requested ${r.name}`, image: r.poster, tone: 'ok' })
    },
  })
  return (
    <View className="gap-4">
      <Heading r={r} />
      <Field label="A note for the admin (optional)">
        <Input value={note} onChangeText={setNote} placeholder="Dubbed if there is one, please" />
      </Field>
      {send.error && <ErrorText>{(send.error as Error).message}</ErrorText>}
      <Button variant="primary" size="lg" disabled={send.isPending} onPress={() => send.mutate()} icon={<Send size={16} color={tokens['on-accent']} />}>
        Request
      </Button>
    </View>
  )
}
