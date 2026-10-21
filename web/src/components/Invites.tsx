// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { graphql } from '../gql'
import { request } from '../lib/api'
import { relative } from '../lib/downloads'
import { ask, toastError } from './feedback'
import { Copyable } from './MusicSettings'
import { Card } from './SettingsKit'
import { Squircle } from './Squircle'
import { Button, Dialog, Field, IconButton, Input, Select } from './ui'

const InvitesQuery = graphql(`
  query Invites {
    invites {
      id
      label
      expiresAt
      maxUses
      uses
      revoked
    }
  }
`)

const CreateInvite = graphql(`
  mutation CreateInvite($label: String!, $maxUses: Int!, $expiresInHours: Int!) {
    createInvite(label: $label, maxUses: $maxUses, expiresInHours: $expiresInHours) {
      link
    }
  }
`)

const RevokeInvite = graphql(`
  mutation RevokeInvite($id: Int!) {
    revokeInvite(id: $id)
  }
`)

const LIFETIMES = [
  { value: '24', label: '1 day' },
  { value: '168', label: '1 week' },
  { value: '720', label: '30 days' },
  { value: '8760', label: '1 year' },
]

/** Settings → Users: links people make their own account with. */
export function Invites() {
  const qc = useQueryClient()
  const { data } = useQuery({ queryKey: ['invites'], queryFn: async () => (await request(InvitesQuery)).invites })
  const [adding, setAdding] = useState(false)
  const revoke = useMutation({
    mutationFn: (id: number) => request(RevokeInvite, { id }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['invites'] }),
    onError: toastError,
  })
  // Used up, expired and revoked links are of no use to anyone; the people they let in are in Users.
  const live = data?.filter((i) => !i.revoked && i.uses < i.maxUses && i.expiresAt * 1000 > Date.now())
  return (
    <>
      <Card
        title="Invite links"
        description="Send someone a link and they make their own account, with the default permissions."
        aside={
          <Button onClick={() => setAdding(true)}>
            <Plus className="size-4" /> New link
          </Button>
        }
      >
        <div className="-mx-2 space-y-0.5">
          {live?.length === 0 && <p className="px-2 text-sm text-ink-3">No invite links.</p>}
          {live?.map((i) => (
            <div key={i.id} className="flex items-center gap-3 rounded-xl px-2 py-2 hover:bg-hover">
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm">{i.label}</p>
                <p className="text-xs text-ink-3">
                  {i.uses} of {i.maxUses} used · expires {relative(i.expiresAt)}
                </p>
              </div>
              <IconButton
                label="Revoke"
                onClick={async () =>
                  (await ask({ title: `Revoke “${i.label}”?`, body: 'The link stops working. Accounts made with it stay.', confirm: 'Revoke', danger: true })) &&
                  revoke.mutate(i.id)
                }
              >
                <Trash2 className="size-4" />
              </IconButton>
            </div>
          ))}
        </div>
      </Card>

      {adding && <NewInvite onClose={() => setAdding(false)} />}
    </>
  )
}

function NewInvite({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient()
  const [label, setLabel] = useState('')
  const [uses, setUses] = useState('5')
  const [lifetime, setLifetime] = useState('168')
  const [link, setLink] = useState<string | null>(null)
  const maxUses = Number(uses)
  const valid = !!label.trim() && Number.isInteger(maxUses) && maxUses >= 1 && maxUses <= 1000
  const create = useMutation({
    mutationFn: async () => (await request(CreateInvite, { label: label.trim(), maxUses, expiresInHours: Number(lifetime) })).createInvite,
    onSuccess: (r) => {
      setLink(new URL(r.link, location.origin).href)
      void qc.invalidateQueries({ queryKey: ['invites'] })
    },
  })
  return (
    <Dialog onClose={onClose}>
      <p className="text-[15px] font-medium">{link ? `Invite link for ${label.trim()}` : 'New invite link'}</p>
      {link ? (
        <>
          <Squircle radius={12} edge className="mt-5 space-y-1.5 bg-panel p-4">
            <Copyable label="Link" value={link} />
            <p className="pt-2 text-xs text-ink-3">This is the only time the link is shown.</p>
          </Squircle>
          <div className="flex justify-end pt-6">
            <Button variant="primary" onClick={onClose}>
              Done
            </Button>
          </div>
        </>
      ) : (
        <form
          className="mt-5 space-y-4"
          onSubmit={(e) => {
            e.preventDefault()
            if (valid) create.mutate()
          }}
        >
          <Field label="Name">
            <Input autoFocus maxLength={64} value={label} onChange={(e) => setLabel(e.target.value)} placeholder="Who it’s for, e.g. Family" />
          </Field>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Accounts">
              <Input type="number" min={1} max={1000} step={1} value={uses} onChange={(e) => setUses(e.target.value)} />
            </Field>
            <Field label="Expires after">
              <Select value={lifetime} onChange={setLifetime} options={LIFETIMES} />
            </Field>
          </div>
          {create.error && <p className="text-sm text-danger">{(create.error as Error).message}</p>}
          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="plain" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" variant="primary" disabled={!valid || create.isPending}>
              Create
            </Button>
          </div>
        </form>
      )}
    </Dialog>
  )
}
