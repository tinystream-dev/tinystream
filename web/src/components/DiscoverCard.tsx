// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate } from '@tanstack/react-router'
import { Check, Clock, Plus, Send } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { graphql } from '../gql'
import { type DiscoverResult, type Monitor, request, settingsQuery } from '../lib/api'

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
import { mediaLabels } from '@tinystream/shared/media'
import { useMe } from '../lib/hooks'
import { MonitorPicker } from './downloads'
import { toast } from './feedback'
import { Section } from './Page'
import { Row } from './Row'
import { Squircle } from './Squircle'
import { Button, Dialog, Field, HoverCard, Input, Select } from './ui'
import { Img } from './Img'
import { useTilt } from '../lib/tilt'

/** What someone can do with a show that isn't here: add it, ask for it, or neither. */
export type Action = 'add' | 'request' | null

export function useAction(): Action {
  const me = useMe()
  return me?.permissions.manageShows ? 'add' : me?.permissions.request ? 'request' : null
}

/** `because` off leaves out the "Like X" caption; the hover card still says it. */
export function ResultCard({
  r,
  action,
  onPick,
  because = true,
}: {
  r: DiscoverResult
  action: Action
  onPick: () => void
  because?: boolean
}) {
  if (r.category === 'MOVIES') action = null
  const [broken, setBroken] = useState(false)
  const tilt = useTilt<HTMLDivElement>()
  const status = r.titleId
    ? { label: 'In your library', icon: <Check className="size-3.5" /> }
    : r.seriesId && r.monitor && r.monitor !== 'NONE'
      ? { label: 'Downloading', icon: <Clock className="size-3.5" /> }
      : r.requestState === 'PENDING'
        ? { label: 'Requested', icon: <Clock className="size-3.5" /> }
        : null
  const body = (
    <>
      <div ref={tilt.ref} className="tilt transition-[translate] duration-200 ease-out group-hover:-translate-y-0.5">
        <Squircle radius={14} edge className="aspect-[2/3] bg-raised">
          {r.poster && !broken ? (
            <Img src={r.poster} loading="lazy" onError={() => setBroken(true)} className="size-full object-cover" />
          ) : (
            <div className="flex size-full items-end bg-panel p-3 text-[15px] font-medium text-ink-2">{r.name}</div>
          )}
          <div className="glare" />
          <div className="absolute inset-x-2 bottom-2 flex justify-center opacity-0 transition-opacity group-hover:opacity-100">
            {!status && action && (
              <span className="flex h-8 items-center gap-1.5 rounded-full bg-ink px-3.5 text-[13px] font-medium text-canvas">
                {action === 'add' ? <Plus className="size-4" /> : <Send className="size-3.5" />}
                {action === 'add' ? 'Add' : 'Request'}
              </span>
            )}
          </div>
          {status && (
            <span className="absolute top-2 right-2 flex items-center gap-1 rounded-md bg-media-shade/65 px-1.5 py-0.5 text-2xs font-medium text-media-ink backdrop-blur-md">
              {status.icon} {status.label}
            </span>
          )}
        </Squircle>
      </div>
      <p className="mt-2 truncate text-[13px] font-medium text-ink">{r.name}</p>
      {r.romaji && <p className="truncate text-xs text-ink-2">{r.romaji}</p>}
      <p className="truncate text-xs text-ink-3 tabular">
        {mediaLabels[r.category]} · {because && r.because ? `Like ${r.because}` : (r.year ?? 'Upcoming')}
      </p>
    </>
  )
  const hint = r.titleId ? 'In your library' : r.category === 'MOVIES' ? 'Place a movie folder in your library to watch. Automatic downloads are available for shows.' : status ? status.label : action === 'add' ? 'Click to add' : action === 'request' ? 'Click to request' : null
  return (
    <HoverCard content={<ShowDetails r={r} hint={hint} />} className="min-w-0 self-start">
      {r.titleId ? (
        <Link to="/title/$id" params={{ id: String(r.titleId) }} className="group block min-w-0 text-left outline-none" {...tilt.handlers}>
          {body}
        </Link>
      ) : (
        <button
          onClick={onPick}
          disabled={!!status || !action}
          className="group block w-full min-w-0 text-left outline-none disabled:cursor-default"
          {...tilt.handlers}
        >
          {body}
        </button>
      )}
    </HoverCard>
  )
}

/** What the hover card says about a show. */
function ShowDetails({ r, hint }: { r: DiscoverResult; hint: string | null }) {
  return (
    <>
      <p className="text-[15px] leading-snug font-semibold tracking-tight text-balance">{r.name}</p>
      {r.romaji && <p className="mt-0.5 text-[13px] text-ink-2">{r.romaji}</p>}
      <p className="mt-0.5 text-xs text-ink-3 tabular">{mediaLabels[r.category]} · {r.year ?? 'Upcoming'}</p>
      {r.because && <p className="mt-2.5 text-xs text-ink-2">Because you watched {r.because}</p>}
      {r.overview && <p className="mt-2.5 line-clamp-6 text-[13px] leading-relaxed whitespace-pre-line text-ink-2">{r.overview}</p>}
      {hint && <p className="mt-3 border-t border-line pt-2.5 text-xs text-ink-3">{hint}</p>}
    </>
  )
}

/** The add or request dialog for a picked show, whichever this person gets. */
export function PickDialog({ r, action, onClose }: { r: DiscoverResult; action: Action; onClose: () => void }) {
  if (r.category === 'MOVIES') return null
  if (action === 'add') return <AddDialog r={r} onClose={onClose} />
  if (action === 'request') return <RequestDialog r={r} onClose={onClose} />
  return null
}

/** A shelf of shows from the provider, each one addable or requestable unless `actionable` is off. */
export function DiscoverShelf({
  title,
  results,
  aside,
  actionable = true,
}: {
  title: ReactNode
  results: DiscoverResult[]
  aside?: ReactNode
  actionable?: boolean
}) {
  const allowed = useAction()
  const action = actionable ? allowed : null
  // "Like X" on every card says nothing when it's the same X each time.
  const varied = new Set(results.map((r) => r.because)).size > 1
  const [picked, setPicked] = useState<DiscoverResult | null>(null)
  return (
    <Section title={title} aside={aside}>
      <Row>
        {results.map((r) => (
          <div key={`${r.provider}${r.id}`} className="w-38 shrink-0 snap-start">
            <ResultCard r={r} because={varied} action={action} onPick={() => setPicked(r)} />
          </div>
        ))}
      </Row>
      {picked && <PickDialog r={picked} action={action} onClose={() => setPicked(null)} />}
    </Section>
  )
}

function Heading({ r }: { r: DiscoverResult }) {
  return (
    <div className="flex gap-4">
      <Squircle radius={10} className="aspect-[2/3] w-20 shrink-0 bg-panel">
        {r.poster && <img src={r.poster} alt="" className="size-full object-cover" />}
      </Squircle>
      <div className="min-w-0">
        <p className="text-[17px] leading-snug font-semibold tracking-tight">{r.name}</p>
        {r.romaji && <p className="text-sm text-ink-2">{r.romaji}</p>}
        <p className="text-sm text-ink-3">{mediaLabels[r.category]} · {r.year ?? 'Upcoming'}</p>
        {r.overview && <p className="mt-2 line-clamp-4 text-[13px] leading-relaxed text-ink-2">{r.overview}</p>}
      </div>
    </div>
  )
}

/** Past this many aired episodes, adding with Missing queues a lot of disk. */
const MANY_EPISODES = 100

export function AddDialog({ r, onClose }: { r: DiscoverResult; onClose: () => void }) {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const { data: settings } = useQuery(settingsQuery)
  const [library, setLibrary] = useState(r.library)
  const [monitor, setMonitor] = useState<Monitor | null>(null)
  const [profile, setProfile] = useState('')
  const lib = settings?.libraries.find((l) => l.name === library)
  // Settings → Automation decides what's picked to begin with.
  const chosen: Monitor = monitor ?? settings?.automation.defaultMonitor ?? 'NONE'
  const { data: aired } = useQuery({
    queryKey: ['airedEpisodes', r.provider, r.id],
    queryFn: async () => (await request(AiredEpisodes, { provider: r.provider, id: r.id })).airedEpisodes,
    staleTime: Infinity,
  })
  const add = useMutation({
    mutationFn: () =>
      request(AddSeries, {
        input: {
          library,
          provider: r.provider,
          providerId: r.id,
          name: r.name,
          year: r.year,
          poster: r.poster,
          overview: r.overview,
          monitor: chosen,
          profile: profile || null,
        },
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['discover'] })
      void qc.invalidateQueries({ queryKey: ['calendar'] })
      onClose()
      toast({
        title: `Added ${r.name}`,
        image: r.poster,
        tone: 'ok',
        action: chosen === 'NONE' ? undefined : { label: 'See wanted', run: () => void navigate({ to: '/wanted' }) },
      })
    },
  })
  return (
    <Dialog onClose={onClose}>
      <Heading r={r} />
      <div className="mt-6 space-y-4">
        <Field label="Library">
          <Select
            value={library}
            options={(settings?.libraries ?? []).filter((l) => l.metadataProvider === r.provider).map((l) => ({ value: l.name, label: l.name }))}
            onChange={setLibrary}
          />
        </Field>
        {lib && !lib.managed && (
          <p className="text-xs leading-relaxed text-warn">
            {lib.name} isn't managed; can't download into it.
          </p>
        )}
        <div>
          <p className="mb-1.5 text-[13px] text-ink-2">Download</p>
          <MonitorPicker value={chosen} onChange={setMonitor} />
        </div>
        {chosen === 'MISSING' && aired !== undefined && aired >= MANY_EPISODES && (
          <p className="text-xs leading-relaxed text-warn">
            {r.name} has {aired} aired episodes, and all of them will be downloaded. Pick Future to only get new ones.
          </p>
        )}
        {settings && settings.profiles.length > 0 && (
          <Field label="Quality">
            <Select
              value={profile}
              options={[
                { value: '', label: `Library default${lib?.profile ? ` (${lib.profile})` : ''}` },
                ...settings.profiles.map((p) => ({ value: p.name, label: p.name })),
              ]}
              onChange={setProfile}
            />
          </Field>
        )}
        {add.error && <p className="text-sm text-danger">{(add.error as Error).message}</p>}
      </div>
      <div className="mt-6 flex justify-end gap-2">
        <Button variant="plain" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" disabled={add.isPending} onClick={() => add.mutate()}>
          <Plus className="size-4" /> Add show
        </Button>
      </div>
    </Dialog>
  )
}

export function RequestDialog({ r, onClose }: { r: DiscoverResult; onClose: () => void }) {
  const qc = useQueryClient()
  const [note, setNote] = useState('')
  const send = useMutation({
    mutationFn: () =>
      request(CreateRequest, {
        input: {
          library: r.library,
          providerId: r.id,
          name: r.name,
          year: r.year,
          poster: r.poster,
          overview: r.overview,
          note: note || null,
        },
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['discover'] })
      void qc.invalidateQueries({ queryKey: ['requests'] })
      onClose()
      toast({ title: `Requested ${r.name}`, image: r.poster, tone: 'ok' })
    },
  })
  return (
    <Dialog onClose={onClose}>
      <Heading r={r} />
      <div className="mt-6">
        <Field label="A note for the admin (optional)">
          <Input value={note} onChange={(e) => setNote(e.target.value)} placeholder="Dubbed if there is one, please" />
        </Field>
        {send.error && <p className="mt-3 text-sm text-danger">{(send.error as Error).message}</p>}
      </div>
      <div className="mt-6 flex items-center justify-end gap-2">
        <Button variant="plain" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" disabled={send.isPending} onClick={() => send.mutate()}>
          <Send className="size-3.5" /> Request
        </Button>
      </div>
    </Dialog>
  )
}
