// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Copy, Link2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { graphql } from '../gql'
import { request } from '../lib/api'
import { toast, toastError } from './feedback'
import { Card } from './SettingsKit'
import { Button, Field, Input, Select } from './ui'

const InvitesQuery = graphql(`
  query Invites {
    invites { id label createdAt expiresAt maxUses uses revoked }
  }
`)
const CreateInvite = graphql(`
  mutation CreateInvite($label: String!, $maxUses: Int!, $expiresInHours: Int!) {
    createInvite(label: $label, maxUses: $maxUses, expiresInHours: $expiresInHours) { link }
  }
`)
const RevokeInvite = graphql(`
  mutation RevokeInvite($id: Int!) { revokeInvite(id: $id) }
`)

export function Invites() {
  const qc = useQueryClient()
  const { data, error } = useQuery({
    queryKey: ['invites'],
    queryFn: async () => (await request(InvitesQuery)).invites,
    refetchInterval: 30_000,
  })
  const [clock, setClock] = useState(() => Date.now())
  useEffect(() => {
    const timer = setInterval(() => setClock(Date.now()), 30_000)
    return () => clearInterval(timer)
  }, [])
  const [label, setLabel] = useState('Invite link')
  const [uses, setUses] = useState('5')
  const [duration, setDuration] = useState('7')
  const [unit, setUnit] = useState('days')
  const [link, setLink] = useState('')
  const hours = Number(duration) * (unit === 'days' ? 24 : 1)
  const valid = label.trim().length > 0 && Number.isInteger(Number(uses)) && Number(uses) >= 1 && Number(uses) <= 1000
    && Number.isInteger(hours) && hours >= 1 && hours <= 8760
  const create = useMutation({
    mutationFn: () => request(CreateInvite, { label: label.trim(), maxUses: Number(uses), expiresInHours: hours }),
    onSuccess: (r) => {
      setLink(new URL(r.createInvite.link, window.location.origin).href)
      void qc.invalidateQueries({ queryKey: ['invites'] })
    },
    onError: toastError,
  })
  const revoke = useMutation({
    mutationFn: (id: number) => request(RevokeInvite, { id }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['invites'] }),
    onError: toastError,
  })
  return (
    <Card title="Invite links">
      <p className="mb-4 text-sm text-ink-2">Let people create their own accounts with the default permissions. A link stops working when it expires or reaches its use limit.</p>
      <form className="space-y-4" onSubmit={(e) => { e.preventDefault(); if (valid && !create.isPending) create.mutate() }}>
        <div className="grid gap-4 sm:grid-cols-3">
          <Field label="Invite name"><Input value={label} maxLength={64} onChange={(e) => setLabel(e.target.value)} required /></Field>
          <Field label="Maximum uses"><Input type="number" min={1} max={1000} step={1} value={uses} onChange={(e) => setUses(e.target.value)} required /></Field>
          <Field label="Expires after">
            <div className="flex gap-2">
              <Input type="number" min={1} max={unit === 'days' ? 365 : 8760} step={1} value={duration} onChange={(e) => setDuration(e.target.value)} required />
              <Select value={unit} onChange={setUnit} options={[{ value: 'days', label: 'Days' }, { value: 'hours', label: 'Hours' }]} />
            </div>
          </Field>
        </div>
        <Button type="submit" variant="primary" disabled={!valid || create.isPending}><Link2 className="size-4" /> {create.isPending ? 'Creating…' : 'Create invite link'}</Button>
      </form>
      {link && (
        <div className="mt-5 space-y-2">
          <Field label="New invite link"><Input readOnly value={link} onFocus={(e) => e.target.select()} /></Field>
          <p className="text-xs text-ink-3">Copy this link now. It is only shown here until you leave this page or create another.</p>
          <Button onClick={() => { if (!navigator.clipboard) { toast({ title: 'Select the link above and copy it.' }); return }; void navigator.clipboard.writeText(link).then(() => toast({ title: 'Invite link copied', tone: 'ok' })).catch(toastError) }}><Copy className="size-4" /> Copy invite link</Button>
        </div>
      )}
      {error && <p role="alert" className="mt-4 text-sm text-danger">{error.message}</p>}
      <div className="mt-5 divide-y divide-ink/10">
        {data?.map((i) => {
          const status = i.revoked ? 'Revoked' : i.expiresAt * 1000 <= clock ? 'Expired' : i.uses >= i.maxUses ? 'Use limit reached' : 'Active'
          return (
            <div key={i.id} className="flex items-center justify-between gap-3 py-3">
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{i.label}</p>
                <p className="text-xs text-ink-3">{status} · {i.uses}/{i.maxUses} uses · Expires {new Date(i.expiresAt * 1000).toLocaleString()}</p>
              </div>
              {status === 'Active' && <Button variant="plain" disabled={revoke.isPending} onClick={() => revoke.mutate(i.id)}>Revoke</Button>}
            </div>
          )
        })}
      </div>
    </Card>
  )
}
