// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Copy, Link2Off, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { graphql } from '../gql'
import { type ClipsConfig, request } from '../lib/api'

const ClipStorageQuery = graphql(`
  query ClipStorage {
    clipStorage {
      usage {
        user {
          ...Person
        }
        bytes
        rendered
        clips
        storage
        limit
      }
      publicClips {
        ...ClipFields
      }
      bytes
      dir
    }
  }
`)

const SaveClipsConfig = graphql(`
  mutation SaveClipsConfig($clips: ClipsConfigInput!) {
    updateSettings(patch: { clips: $clips }) {
      clips {
        enabled
      }
    }
  }
`)

const DropRenders = graphql(`
  mutation DropClipRenders {
    dropClipRenders
  }
`)

const Unpublish = graphql(`
  mutation UnpublishClip($id: Int!) {
    updateClip(id: $id, input: { public: false }) {
      id
    }
  }
`)

const DeleteClip = graphql(`
  mutation AdminDeleteClip($id: Int!) {
    deleteClip(id: $id)
  }
`)
import { clipName, clipSource, copyLink, length, space } from '../lib/clips'
import { bytes } from '../lib/downloads'
import { useOrigin } from '../lib/hooks'
import { Avatar } from './Avatar'
import { ask, toast, toastError } from './feedback'
import { Card, Row, useSave, useSettings } from './SettingsKit'
import { Squircle } from './Squircle'
import { Button, Field, IconButton, Input, Progress, Toggle } from './ui'

/** Settings → Clips: whether clipping is on, and what it takes up. */
export function ClipSettings() {
  const { data } = useSettings()
  const { data: admin } = useQuery({ queryKey: ['clips-admin'], queryFn: async () => (await request(ClipStorageQuery)).clipStorage })
  const origin = useOrigin()
  const qc = useQueryClient()
  const refresh = useSave()
  const [draft, setDraft] = useState<ClipsConfig | null>(null)
  useEffect(() => {
    if (data && !draft) setDraft(structuredClone(data.clips))
  }, [data, draft])
  const save = useMutation({
    mutationFn: () => request(SaveClipsConfig, { clips: draft! }),
    onSuccess: () => {
      toast({ title: 'Saved', tone: 'ok' })
      refresh()
      void qc.invalidateQueries({ queryKey: ['auth'] })
    },
    onError: toastError,
  })
  const reload = () => {
    void qc.invalidateQueries({ queryKey: ['clips-admin'] })
    void qc.invalidateQueries({ queryKey: ['clips'] })
  }
  if (!data || !draft) return null
  const set = (change: Partial<ClipsConfig>) => setDraft({ ...draft, ...change })
  const dirty = JSON.stringify(draft) !== JSON.stringify(data.clips)
  const text = (v: string) => v.trim() || null

  return (
    <>
      <Card
        title="General"
        aside={
          <Button variant="primary" disabled={!dirty || save.isPending} onClick={() => save.mutate()}>
            Save
          </Button>
        }
      >
        <div className="space-y-4">
          <Row label="Enable clipping">
            <Toggle label="Enable clipping" checked={draft.enabled} onChange={(v) => set({ enabled: v })} />
          </Row>
          <Row label="Public links">
            <Toggle label="Public links" checked={draft.publicLinks} onChange={(v) => set({ publicLinks: v })} />
          </Row>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Concurrent renders">
              <Input
                inputMode="numeric"
                value={draft.concurrency}
                onChange={(e) => set({ concurrency: Math.max(1, Number(e.target.value.replace(/\D/g, '')) || 1) })}
              />
            </Field>
            <Field label="Total storage limit (GB)">
              <Input
                inputMode="numeric"
                placeholder="No limit"
                value={draft.maxStorage ? Math.round(draft.maxStorage / 1024) : ''}
                onChange={(e) => set({ maxStorage: (Number(e.target.value.replace(/\D/g, '')) || 0) * 1024 })}
              />
            </Field>
            <Field label="Clip folder">
              <Input value={draft.path ?? ''} placeholder={admin?.dir ?? 'clips, in the data folder'} onChange={(e) => set({ path: text(e.target.value) })} />
            </Field>
            <Field label="Subtitle fonts folder">
              <Input value={draft.fontsDir ?? ''} placeholder="None" onChange={(e) => set({ fontsDir: text(e.target.value) })} />
            </Field>
            <Field label="Fallback subtitle font" hint="Built-in covers Latin, Greek and Cyrillic">
              <Input value={draft.defaultFont ?? ''} placeholder="Built-in Noto Sans" onChange={(e) => set({ defaultFont: text(e.target.value) })} />
            </Field>
          </div>
        </div>
      </Card>

      {admin && (
        <Card title="Storage" description={`${bytes(admin.bytes)}${admin.dir ? ` in ${admin.dir}` : ''}`}>
          <div className="-mx-2 space-y-0.5">
            {admin.usage.map((u) => {
              const share = u.storage ? u.bytes / (u.storage * 1024 * 1024) : null
              return (
                <div key={u.user.id} className="flex items-center gap-3 px-2 py-2">
                  <Avatar user={u.user} size={28} />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm">{u.user.username}</p>
                    <p className="text-xs text-ink-3 tabular">
                      {u.clips} {u.clips === 1 ? 'clip' : 'clips'}, {u.rendered} rendered
                      {u.limit ? ` of ${u.limit}` : ''}
                    </p>
                  </div>
                  <div className="w-40 shrink-0">
                    <p className="mb-1 text-right text-xs text-ink-2 tabular">
                      {bytes(u.bytes)}
                      {u.storage ? <span className="text-ink-3"> of {space(u.storage)}</span> : null}
                    </p>
                    {share !== null && <Progress value={share} tone={share > 0.9 ? 'live' : 'ink'} />}
                  </div>
                </div>
              )
            })}
          </div>
          <div className="mt-4 flex justify-end">
            <Button
              variant="danger"
              onClick={async () => {
                const ok = await ask({
                  title: 'Drop every rendered clip?',
                  body: 'Clips re-render when opened.',
                  confirm: 'Drop renders',
                  danger: true,
                })
                if (!ok) return
                try {
                  const dropped = (await request(DropRenders)).dropClipRenders
                  toast({ title: `Dropped ${dropped} ${dropped === 1 ? 'render' : 'renders'}`, tone: 'ok' })
                  reload()
                } catch (e) {
                  toastError(e)
                }
              }}
            >
              <Trash2 className="size-4" /> Drop every render
            </Button>
          </div>
        </Card>
      )}

      {admin && (
        <Card title="Public links">
          {admin.publicClips.length === 0 ? (
            <p className="text-sm text-ink-3">None right now.</p>
          ) : (
            <div className="-mx-2 space-y-0.5">
              {admin.publicClips.map((c) => (
                <div key={c.id} className="flex items-center gap-3 px-2 py-2">
                  <Squircle radius={8} className="aspect-video w-20 shrink-0 bg-panel">
                    {c.poster && <img src={c.poster} alt="" className="size-full object-cover" />}
                  </Squircle>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm">{clipName(c)}</p>
                    <p className="truncate text-xs text-ink-3">
                      {c.owner.username} · {clipSource(c)} · {length(c.end - c.start)}
                      {!c.linkLive && ' · link is off for them'}
                    </p>
                  </div>
                  {c.link && c.linkLive && (
                    <IconButton label="Copy link" onClick={() => void copyLink(c.link!, origin).then((url) => toast({ title: 'Link copied', body: url, tone: 'ok' }))}>
                      <Copy className="size-4" />
                    </IconButton>
                  )}
                  <IconButton
                    label="Turn the link off"
                    onClick={() => request(Unpublish, { id: c.id }).then(reload).catch(toastError)}
                  >
                    <Link2Off className="size-4" />
                  </IconButton>
                  <IconButton
                    label="Delete the clip"
                    className="hover:text-danger"
                    onClick={async () => {
                      const ok = await ask({ title: `Delete ${c.owner.username}'s clip?`, confirm: 'Delete', danger: true })
                      if (ok) request(DeleteClip, { id: c.id }).then(reload).catch(toastError)
                    }}
                  >
                    <Trash2 className="size-4" />
                  </IconButton>
                </div>
              ))}
            </div>
          )}
        </Card>
      )}
    </>
  )
}
