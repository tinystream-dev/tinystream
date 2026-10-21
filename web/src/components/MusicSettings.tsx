// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Copy, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { graphql } from '../gql'
import { type MusicConfig, request } from '../lib/api'
import { useMe } from '../lib/hooks'
import { ask, toast, toastError } from './feedback'
import { Card, Row, useSave, useSettings } from './SettingsKit'
import { Squircle } from './Squircle'
import { Button, Field, IconButton, Input, Toggle } from './ui'

const SaveMusicConfig = graphql(`
  mutation SaveMusicConfig($music: MusicConfigInput!) {
    updateSettings(patch: { music: $music }) {
      music {
        onlineLyrics
      }
    }
  }
`)

/** Settings → Music: lyrics from the internet, and levelling volume. */
export function MusicSettings() {
  const { data } = useSettings()
  const refresh = useSave()
  const [draft, setDraft] = useState<MusicConfig | null>(null)
  useEffect(() => {
    if (data && !draft) setDraft(structuredClone(data.music))
  }, [data, draft])
  const save = useMutation({ mutationFn: () => request(SaveMusicConfig, { music: draft! }), onSuccess: refresh, onError: toastError })
  if (!draft || !data) return null
  const changed = JSON.stringify(draft) !== JSON.stringify(data.music)
  return (
    <Card
      title="Music"
      description="How music plays everywhere: in tinystream and in every music app."
      aside={
        <Button variant="primary" disabled={!changed || save.isPending} onClick={() => save.mutate()}>
          Save
        </Button>
      }
    >
      <div className="space-y-5">
        <Row label="Even out volume" hint="Measures songs without ReplayGain in their tags, while nothing else is going on; one about to play is measured right away">
          <Toggle label="Even out volume" checked={draft.analyzeLoudness} onChange={(v) => setDraft({ ...draft, analyzeLoudness: v })} />
        </Row>
        <Row label="Look up lyrics online" hint="When a song has none in its tags or beside it, they're fetched from LRCLIB and kept">
          <Toggle label="Look up lyrics online" checked={draft.onlineLyrics} onChange={(v) => setDraft({ ...draft, onlineLyrics: v })} />
        </Row>
        {draft.onlineLyrics && (
          <Field label="Lyrics server">
            <Input value={draft.lyricsUrl} onChange={(e) => setDraft({ ...draft, lyricsUrl: e.target.value })} placeholder="https://lrclib.net" />
          </Field>
        )}
      </div>
    </Card>
  )
}

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

export function Copyable({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-3 text-sm">
      <span className="w-24 shrink-0 text-ink-3">{label}</span>
      <code className="min-w-0 flex-1 truncate font-mono text-[13px]">{value}</code>
      <IconButton
        label={`Copy ${label.toLowerCase()}`}
        onClick={() => void navigator.clipboard.writeText(value).then(() => toast({ title: 'Copied', tone: 'ok' }))}
      >
        <Copy className="size-3.5" />
      </IconButton>
    </div>
  )
}

/** Account → Music apps: a password per app, for Subsonic apps like Feishin or Symfonium. */
export function AppPasswords() {
  const me = useMe()
  const qc = useQueryClient()
  const { data } = useQuery({ queryKey: ['app-passwords'], queryFn: async () => (await request(AppPasswordsQuery)).appPasswords })
  const [name, setName] = useState('')
  const [made, setMade] = useState<{ name: string; secret: string } | null>(null)
  const create = useMutation({
    mutationFn: () => request(CreateAppPassword, { name }),
    onSuccess: ({ createAppPassword: r }) => {
      setMade({ name: r.password.name, secret: r.secret })
      setName('')
      void qc.invalidateQueries({ queryKey: ['app-passwords'] })
    },
    onError: toastError,
  })
  const remove = useMutation({
    mutationFn: (id: number) => request(DeleteAppPassword, { id }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['app-passwords'] }),
  })
  const date = (s: number) => new Date(s * 1000).toLocaleDateString(undefined, { dateStyle: 'medium' })
  return (
    <Card
      title="Music apps"
      description="Apps that speak Subsonic, like Feishin, Symfonium or Tempo, play your music from here. Give each its own password; you can take it back any time without changing yours."
    >
      {made && (
        <Squircle radius={12} edge className="mb-4 space-y-1.5 bg-panel p-4">
          <p className="mb-2 text-sm font-medium">Sign in to {made.name} with:</p>
          <Copyable label="Server" value={location.origin} />
          <Copyable label="Username" value={me?.username ?? ''} />
          <Copyable label="Password" value={made.secret} />
          <p className="pt-2 text-xs text-ink-3">This is the only time the password is shown.</p>
        </Squircle>
      )}
      <div className="-mx-2 mb-4 space-y-0.5">
        {data?.length === 0 && <p className="px-2 text-sm text-ink-3">No app passwords yet.</p>}
        {data?.map((p) => (
          <div key={p.id} className="flex items-center gap-3 rounded-xl px-2 py-2 hover:bg-hover">
            <div className="min-w-0 flex-1">
              <p className="text-sm">{p.name}</p>
              <p className="text-xs text-ink-3">
                Made {date(p.createdAt)}
                {p.lastUsed ? ` · last used ${date(p.lastUsed)}${p.client ? ` by ${p.client}` : ''}` : ' · never used'}
              </p>
            </div>
            <IconButton
              label="Revoke"
              onClick={async () =>
                (await ask({ title: `Revoke “${p.name}”?`, body: 'Whatever uses it is signed out.', confirm: 'Revoke', danger: true })) && remove.mutate(p.id)
              }
            >
              <Trash2 className="size-4" />
            </IconButton>
          </div>
        ))}
      </div>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault()
          if (name.trim()) create.mutate()
        }}
      >
        <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Which app or device, e.g. Symfonium on my phone" className="flex-1" />
        <Button disabled={!name.trim() || create.isPending}>New password</Button>
      </form>
    </Card>
  )
}
