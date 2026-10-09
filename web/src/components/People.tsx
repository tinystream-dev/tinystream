// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  ArrowDownToLine,
  ChevronRight,
  Clock3,
  Crown,
  Gauge,
  HardDrive,
  Layers,
  Inbox,
  KeyRound,
  Library,
  Link2,
  ListChecks,
  Minus,
  Plus,
  RotateCcw,
  Scissors,
  Sparkles,
  Trash2,
  Users,
  Wand2,
  Zap,
} from 'lucide-react'
import { type ReactNode, useEffect, useState } from 'react'
import { graphql } from '../gql'
import type { PermissionOverridesInput, UserPatch } from '../gql/graphql'
import { type Permissions, type User, request } from '../lib/api'

const UsersQuery = graphql(`
  query Users {
    users {
      ...Viewer
      createdAt
      lastSeen
      overrides {
        allLibraries
        libraries
        request
        autoApprove
        requestLimit
        manageRequests
        manageShows
        downloads
        editMetadata
        watchTogether
        shareLinks
        clip
        clipMaxLength
        clipLimit
        clipStorage
        clipLinks
      }
    }
  }
`)

const DefaultsQuery = graphql(`
  query PermissionDefaults {
    permissionDefaults {
      ...PermissionsFields
    }
  }
`)

const SetDefaults = graphql(`
  mutation SetPermissionDefaults($permissions: PermissionsInput!) {
    setPermissionDefaults(permissions: $permissions) {
      ...PermissionsFields
    }
  }
`)

const UpdateUser = graphql(`
  mutation UpdateUser($id: Int!, $input: UserPatch!) {
    updateUser(id: $id, input: $input) {
      id
    }
  }
`)

const DeleteUser = graphql(`
  mutation DeleteUser($id: Int!) {
    deleteUser(id: $id)
  }
`)

const CreateUser = graphql(`
  mutation CreateUser($input: NewUser!) {
    createUser(input: $input) {
      id
    }
  }
`)

/** Someone's departures from the defaults; a missing key follows them. */
type Overrides = PermissionOverridesInput
type ManagedUser = User & { createdAt: number; lastSeen: number | null; overrides: Overrides }

/** Only what's set: the server says null for everything that follows the defaults. */
const setOnly = (o: Record<string, unknown>) => Object.fromEntries(Object.entries(o).filter(([, v]) => v != null)) as Overrides
import { relative } from '../lib/downloads'
import { useMe, useStatus } from '../lib/hooks'
import { Invites } from './Invites'
import { Avatar, AvatarPicker } from './Avatar'
import { ask, toast, toastError } from './feedback'
import { Card, useSettings } from './SettingsKit'
import { Squircle } from './Squircle'
import { Badge, Button, Dialog, Field, Input, Segmented, Tip, Toggle } from './ui'
type Key = keyof Permissions
/** Libraries are one setting made of two keys; they're overridden together. */
const LIBRARY_KEYS: Key[] = ['allLibraries', 'libraries']

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)

/**
 * Sets some of someone's permissions. Anything set back to what the defaults
 * say stops being an override, so it follows the defaults again.
 */
function override(o: Overrides, defaults: Permissions, changes: Partial<Permissions>): Overrides {
  const next: Overrides = { ...o, ...changes }
  const keys = Object.keys(changes) as Key[]
  const group = keys.some((k) => LIBRARY_KEYS.includes(k)) ? LIBRARY_KEYS : []
  // Libraries go back to the defaults only when both halves match them.
  if (group.length && group.every((k) => same({ ...defaults, ...next }[k], defaults[k]))) group.forEach((k) => delete next[k])
  for (const k of keys) if (!group.includes(k) && same(next[k], defaults[k])) delete next[k]
  return next
}

function without(o: Overrides, keys: Key[]): Overrides {
  const next = { ...o }
  keys.forEach((k) => delete next[k])
  return next
}

/** How many settings someone has their own value for (libraries count once). */
const customCount = (o: Overrides) => new Set(Object.keys(o).map((k) => (k === 'libraries' ? 'allLibraries' : k))).size

function summary(p: Permissions, isAdmin: boolean, libraries: string[], downloads: boolean): string {
  if (isAdmin) return 'Can do everything'
  const seen = libraries.filter((l) => p.allLibraries || p.libraries.includes(l))
  const parts = [
    p.allLibraries ? 'All libraries' : seen.length === 0 ? 'No libraries' : seen.length <= 2 ? seen.join(', ') : `${seen.length} libraries`,
  ]
  if (downloads) {
    if (p.request) parts.push(p.autoApprove ? 'requests go straight through' : 'can request')
    if (p.manageRequests) parts.push('approves requests')
    if (p.manageShows) parts.push('manages shows')
    if (p.downloads) parts.push('downloads')
  }
  if (p.editMetadata) parts.push('fixes details')
  if (p.shareLinks && p.watchTogether) parts.push('shares public links')
  else if (!p.watchTogether) parts.push('can’t start watch parties')
  if (!p.clip) parts.push('can’t clip')
  else if (p.clipLinks) parts.push('shares public clips')
  return parts.join(' · ')
}
export function People() {
  const me = useMe()
  const { data: users } = useQuery({
    queryKey: ['users'],
    queryFn: async (): Promise<ManagedUser[]> => (await request(UsersQuery)).users.map((u) => ({ ...u, overrides: setOnly(u.overrides) })),
  })
  const { data: defaults } = useQuery({ queryKey: ['permission-defaults'], queryFn: async () => (await request(DefaultsQuery)).permissionDefaults })
  const { data: settings } = useSettings()
  const [adding, setAdding] = useState(false)
  const [open, setOpen] = useState<number | null>(null)
  const libraries = settings?.libraries.map((l) => l.name) ?? []
  const downloads = !!useStatus().data?.server.downloads
  const person = users?.find((u) => u.id === open)

  return (
    <>
      <Card
        title="Users"
        aside={
          <Button variant="primary" onClick={() => setAdding(true)}>
            <Plus className="size-4" /> Add someone
          </Button>
        }
      >
        <div className="-mx-2 space-y-0.5">
          {users?.map((u) => {
            const custom = u.isAdmin ? 0 : customCount(u.overrides)
            return (
              <Squircle
                key={u.id}
                as="button"
                radius={12}
                onClick={() => setOpen(u.id)}
                className="group flex w-full items-center gap-3 px-2 py-2.5 text-left transition-colors hover:bg-hover"
              >
                <Avatar user={u} size={36} />
                <div className="min-w-0 flex-1">
                  <p className="flex items-center gap-2 truncate text-sm font-medium">
                    {u.username}
                    {u.id === me?.id && <span className="font-normal text-ink-3">you</span>}
                    {u.isAdmin && (
                      <Badge tone="strong">
                        <Crown className="size-3" /> Admin
                      </Badge>
                    )}
                  </p>
                  <p className="mt-0.5 truncate text-xs text-ink-3">{summary(u.permissions, u.isAdmin, libraries, downloads)}</p>
                </div>
                {custom > 0 && (
                  <Badge tone="live" title="Settings that differ from the defaults">
                    {custom} custom
                  </Badge>
                )}
                <span className="hidden w-24 shrink-0 text-right text-xs text-ink-3 sm:block">
                  {u.lastSeen ? relative(u.lastSeen) : 'Never signed in'}
                </span>
                <ChevronRight className="size-4 shrink-0 text-ink-3 transition-transform group-hover:translate-x-0.5" />
              </Squircle>
            )
          })}
        </div>
      </Card>

      <Invites />

      {defaults && users && (
        <Card title="Default permissions">
          <DefaultsEditor defaults={defaults} users={users} libraries={libraries} downloads={downloads} />
        </Card>
      )}

      {adding && (
        <AddPerson
          onClose={() => setAdding(false)}
          onAdded={(id) => {
            setAdding(false)
            setOpen(id)
          }}
        />
      )}
      {person && defaults && (
        <PersonDialog
          key={person.id}
          person={person}
          defaults={defaults}
          libraries={libraries}
          downloads={downloads}
          onClose={() => setOpen(null)}
        />
      )}
    </>
  )
}

function DefaultsEditor({
  defaults,
  users,
  libraries,
  downloads,
}: {
  defaults: Permissions
  users: ManagedUser[]
  libraries: string[]
  downloads: boolean
}) {
  const qc = useQueryClient()
  const save = useMutation({
    mutationFn: (next: Permissions) => request(SetDefaults, { permissions: next }),
    onMutate: (next) => qc.setQueryData(['permission-defaults'], next),
    onError: (e) => (toastError(e), void qc.invalidateQueries({ queryKey: ['permission-defaults'] })),
    onSettled: () => void qc.invalidateQueries({ queryKey: ['users'] }),
  })
  const members = users.filter((u) => !u.isAdmin)
  // How many people have their own setting, and so won't follow a change here.
  const differ = (keys: Key[]) => {
    const n = members.filter((u) => keys.some((k) => k in u.overrides)).length
    return n ? `${n} ${n === 1 ? 'person has' : 'people have'} their own setting` : undefined
  }
  return (
    <PermissionEditor
      value={defaults}
      libraries={libraries}
      downloads={downloads}
      onChange={(changes) => save.mutate({ ...defaults, ...changes })}
      note={differ}
    />
  )
}
type EditorProps = {
  value: Permissions
  libraries: string[]
  downloads: boolean
  onChange: (changes: Partial<Permissions>) => void
  /** When editing a person: the defaults, and which keys they override. */
  defaults?: Permissions
  overrides?: Overrides
  onReset?: (keys: Key[]) => void
  /** A line under a setting, e.g. how many people differ from a default. */
  note?: (keys: Key[]) => string | undefined
  disabled?: boolean
}

function PermissionEditor(props: EditorProps) {
  const { value: p, libraries, downloads, onChange, disabled } = props
  const row = (keys: Key[]) => ({ keys, ...props })
  const onOff = (v: boolean) => (v ? 'On' : 'Off')
  const limit = (n: number) => (n ? String(n) : 'No limit')
  return (
    <div className={disabled ? 'pointer-events-none opacity-45' : ''} aria-disabled={disabled}>
      <Group title="Watching">
        <Setting
          {...row(LIBRARY_KEYS)}
          icon={<Library />}
          label="Libraries"
          was={(d) => (d.allLibraries ? 'All libraries' : d.libraries.length ? d.libraries.join(', ') : 'None')}
          wide
        >
          <LibraryPicker value={p} libraries={libraries} onChange={onChange} />
        </Setting>
        <Setting
          {...row(['watchTogether'])}
          icon={<Users />}
          label="Watch together"
          was={(d) => onOff(d.watchTogether)}
        >
          <Toggle label="Watch together" checked={p.watchTogether} onChange={(v) => onChange({ watchTogether: v })} />
        </Setting>
        <Setting
          {...row(['shareLinks'])}
          icon={<Link2 />}
          label="Public links"
          hint="For their rooms"
          was={(d) => onOff(d.shareLinks)}
          dim={!p.watchTogether}
        >
          <Toggle label="Public links" checked={p.shareLinks} onChange={(v) => onChange({ shareLinks: v })} />
        </Setting>
      </Group>

      {downloads && (
        <Group title="Requests">
          <Setting {...row(['request'])} icon={<Inbox />} label="Request shows" was={(d) => onOff(d.request)}>
            <Toggle label="Request shows" checked={p.request} onChange={(v) => onChange({ request: v })} />
          </Setting>
          <Setting
            {...row(['autoApprove'])}
            icon={<Zap />}
            label="Skip approval"
            was={(d) => onOff(d.autoApprove)}
            dim={!p.request}
          >
            <Toggle label="Skip approval" checked={p.autoApprove} onChange={(v) => onChange({ autoApprove: v })} />
          </Setting>
          <Setting
            {...row(['requestLimit'])}
            icon={<Gauge />}
            label="Waiting requests"
            was={(d) => limit(d.requestLimit)}
            dim={!p.request || p.autoApprove}
          >
            <Stepper value={p.requestLimit} onChange={(n) => onChange({ requestLimit: n })} />
          </Setting>
        </Group>
      )}

      <Group title="Clips">
        <Setting {...row(['clip'])} icon={<Scissors />} label="Make clips" was={(d) => onOff(d.clip)}>
          <Toggle label="Make clips" checked={p.clip} onChange={(v) => onChange({ clip: v })} />
        </Setting>
        <Setting
          {...row(['clipMaxLength'])}
          icon={<Clock3 />}
          label="Longest clip"
          hint="In seconds"
          was={(d) => (d.clipMaxLength ? `${d.clipMaxLength} s` : 'No limit')}
          dim={!p.clip}
        >
          <Stepper value={p.clipMaxLength} step={5} max={3600} label="Longest clip, in seconds" onChange={(n) => onChange({ clipMaxLength: n })} />
        </Setting>
        <Setting
          {...row(['clipStorage'])}
          icon={<HardDrive />}
          label="Space for clips"
          hint="In GB"
          was={(d) => (d.clipStorage ? `${gb(d.clipStorage)} GB` : 'No limit')}
          dim={!p.clip}
        >
          <Stepper
            value={Math.round(p.clipStorage / 1024)}
            max={10000}
            label="Space for clips, in GB"
            onChange={(n) => onChange({ clipStorage: n * 1024 })}
          />
        </Setting>
        <Setting
          {...row(['clipLimit'])}
          icon={<Layers />}
          label="Rendered clips"
          was={(d) => limit(d.clipLimit)}
          dim={!p.clip}
        >
          <Stepper value={p.clipLimit} max={9999} label="Rendered clips" onChange={(n) => onChange({ clipLimit: n })} />
        </Setting>
        <Setting
          {...row(['clipLinks'])}
          icon={<Link2 />}
          label="Public clip links"
          was={(d) => onOff(d.clipLinks)}
          dim={!p.clip}
        >
          <Toggle label="Public clip links" checked={p.clipLinks} onChange={(v) => onChange({ clipLinks: v })} />
        </Setting>
      </Group>

      <Group title="Management">
        {downloads && (
          <>
            <Setting
              {...row(['manageRequests'])}
              icon={<ListChecks />}
              label="Approve requests"
              was={(d) => onOff(d.manageRequests)}
            >
              <Toggle label="Approve requests" checked={p.manageRequests} onChange={(v) => onChange({ manageRequests: v })} />
            </Setting>
            <Setting
              {...row(['manageShows'])}
              icon={<Sparkles />}
              label="Manage shows"
              hint="Add, monitor and search"
              was={(d) => onOff(d.manageShows)}
            >
              <Toggle label="Manage shows" checked={p.manageShows} onChange={(v) => onChange({ manageShows: v })} />
            </Setting>
            <Setting
              {...row(['downloads'])}
              icon={<ArrowDownToLine />}
              label="Downloads"
              hint="Everyone's, and grab releases"
              was={(d) => onOff(d.downloads)}
            >
              <Toggle label="Downloads" checked={p.downloads} onChange={(v) => onChange({ downloads: v })} />
            </Setting>
          </>
        )}
        <Setting
          {...row(['editMetadata'])}
          icon={<Wand2 />}
          label="Fix details"
          hint="Rematch and refresh titles"
          was={(d) => onOff(d.editMetadata)}
        >
          <Toggle label="Fix details" checked={p.editMetadata} onChange={(v) => onChange({ editMetadata: v })} />
        </Setting>
      </Group>
    </div>
  )
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="mt-5 first:mt-0">
      <p className="mb-1 text-2xs font-medium tracking-wide text-ink-3 uppercase">{title}</p>
      <div className="divide-y divide-line">{children}</div>
    </div>
  )
}

function Setting({
  keys,
  icon,
  label,
  hint,
  was,
  dim,
  wide,
  children,
  defaults,
  overrides,
  onReset,
  note,
}: EditorProps & {
  keys: Key[]
  icon: ReactNode
  label: string
  hint?: string
  /** Describes the default, for the reset button. */
  was: (d: Permissions) => string
  dim?: boolean
  wide?: boolean
  children: ReactNode
}) {
  const custom = !!overrides && keys.some((k) => k in overrides)
  const line = note?.(keys)
  const reset = custom && defaults && onReset && (
    <Tip label={`Go back to the default: ${was(defaults)}`} className="flex">
      <button
        onClick={() => onReset(keys)}
        className="group/reset inline-flex h-5 shrink-0 items-center gap-1 rounded-md bg-info-deep/12 px-1.5 text-2xs font-medium text-info transition-colors hover:bg-info-deep/20"
      >
        <span className="group-hover/reset:hidden">Custom</span>
        <RotateCcw className="hidden size-3 group-hover/reset:block" />
        <span className="hidden group-hover/reset:inline">Use default</span>
      </button>
    </Tip>
  )
  return (
    <div className={`relative py-3 transition-opacity ${dim ? 'opacity-45' : ''}`}>
      {custom && <span className="absolute top-3 bottom-3 -left-3 w-0.5 rounded-full bg-info/70" />}
      <div className="flex items-center gap-3">
        <span className="grid size-7 shrink-0 place-items-center rounded-lg bg-panel text-ink-2 [&>svg]:size-3.5">{icon}</span>
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-2 text-sm">
            {label}
            {reset}
          </p>
          {hint && <p className="mt-0.5 text-xs text-ink-3">{hint}</p>}
          {line && <p className="mt-0.5 text-xs text-info/80">{line}</p>}
        </div>
        {!wide && children}
      </div>
      {wide && <div className="mt-3 pl-10">{children}</div>}
    </div>
  )
}

function LibraryPicker({ value, libraries, onChange }: { value: Permissions; libraries: string[]; onChange: (c: Partial<Permissions>) => void }) {
  if (libraries.length === 0) return <p className="text-xs text-ink-3">Add a library first.</p>
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <Chip on={value.allLibraries} onClick={() => onChange({ allLibraries: !value.allLibraries, libraries: value.allLibraries ? libraries : value.libraries })}>
        All
      </Chip>
      <span className="mx-1 h-4 w-px bg-line-strong" />
      {libraries.map((l) => {
        const on = value.allLibraries || value.libraries.includes(l)
        return (
          <Chip
            key={l}
            on={on}
            soft={value.allLibraries}
            onClick={() => {
              // Picking one out of "all" means everything but that one.
              const current = value.allLibraries ? libraries : value.libraries
              onChange({ allLibraries: false, libraries: on ? current.filter((x) => x !== l) : [...current, l] })
            }}
          >
            {l}
          </Chip>
        )
      })}
    </div>
  )
}

function Chip({ on, soft, onClick, children }: { on: boolean; soft?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <Squircle
      as="button"
      radius={8}
      aria-pressed={on}
      onClick={onClick}
      className={`h-7 px-2.5 text-xs transition-colors ${on ? (soft ? 'bg-ink/75 text-canvas' : 'bg-ink text-canvas') : 'bg-panel text-ink-2 hover:text-ink'}`}
    >
      {children}
    </Squircle>
  )
}

const gb = (mb: number) => Math.round((mb / 1024) * 10) / 10

/** A number, where 0 (shown as ∞) is no limit. */
function Stepper({
  value,
  onChange,
  step: by = 1,
  max = 999,
  label = 'Waiting requests',
}: {
  value: number
  onChange: (n: number) => void
  step?: number
  max?: number
  label?: string
}) {
  const [text, setText] = useState(String(value || ''))
  useEffect(() => setText(String(value || '')), [value])
  const commit = () => {
    const n = Math.max(0, Math.min(max, Number.parseInt(text, 10) || 0))
    if (n !== value) onChange(n)
    else setText(String(value || ''))
  }
  const step = (d: number) => onChange(Math.max(0, Math.min(max, value + d * by)))
  return (
    <Squircle radius={9} edge className="flex h-8 shrink-0 items-center bg-raised">
      <button aria-label="Fewer" onClick={() => step(-1)} disabled={value === 0} className="grid h-full w-7 place-items-center text-ink-3 hover:text-ink disabled:opacity-30">
        <Minus className="size-3.5" />
      </button>
      <input
        value={text}
        inputMode="numeric"
        placeholder="∞"
        aria-label={label}
        onChange={(e) => setText(e.target.value.replace(/\D/g, ''))}
        onBlur={commit}
        onKeyDown={(e) => e.key === 'Enter' && commit()}
        className="w-10 bg-transparent text-center text-sm tabular outline-none placeholder:text-ink-3"
      />
      <button aria-label="More" onClick={() => step(1)} className="grid h-full w-7 place-items-center text-ink-3 hover:text-ink">
        <Plus className="size-3.5" />
      </button>
    </Squircle>
  )
}
function PersonDialog({
  person,
  defaults,
  libraries,
  downloads,
  onClose,
}: {
  person: ManagedUser
  defaults: Permissions
  libraries: string[]
  downloads: boolean
  onClose: () => void
}) {
  const me = useMe()
  const qc = useQueryClient()
  const self = person.id === me?.id
  const update = useMutation({
    mutationFn: (body: UserPatch) => request(UpdateUser, { id: person.id, input: body }),
    // Show the change at once; the server's answer replaces it.
    onMutate: (body: UserPatch) => {
      qc.setQueryData<ManagedUser[]>(['users'], (list) =>
        list?.map((u) => {
          if (u.id !== person.id) return u
          const overrides = body.permissions ?? u.overrides
          const isAdmin = body.isAdmin ?? u.isAdmin
          return {
            ...u,
            username: body.username ?? u.username,
            isAdmin,
            overrides,
            // Overrides never hold nulls here (see setOnly).
            permissions: isAdmin ? u.permissions : { ...defaults, ...(overrides as Partial<Permissions>) },
          }
        }),
      )
    },
    onError: toastError,
    onSettled: () => void qc.invalidateQueries({ queryKey: ['users'] }),
  })
  const remove = useMutation({
    mutationFn: () => request(DeleteUser, { id: person.id }),
    onSuccess: () => (onClose(), void qc.invalidateQueries({ queryKey: ['users'] })),
    onError: toastError,
  })
  const setOverrides = (o: Overrides) => update.mutate({ permissions: o })
  const custom = Object.keys(person.overrides).length > 0

  return (
    <Dialog onClose={onClose} width="max-w-xl">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <AvatarPicker user={person} userId={person.id} size={64} />
      </div>
      <div className="mt-5 grid gap-4 sm:grid-cols-[1fr_auto] sm:items-end">
        <NameField person={person} onSave={(username) => update.mutate({ username })} />
        <div>
          <p className="mb-1.5 text-[13px] text-ink-2">Role</p>
          <Segmented
            value={person.isAdmin ? 'admin' : 'member'}
            onChange={async (v) => {
              const admin = v === 'admin'
              if (
                admin &&
                !(await ask({
                  title: `Make ${person.username} an admin?`,
                  body: 'Admins can do everything, including removing you.',
                  confirm: 'Make admin',
                }))
              )
                return
              update.mutate({ isAdmin: admin })
            }}
            options={[
              { value: 'member', label: 'Member' },
              { value: 'admin', label: <><Crown className="size-3.5" /> Admin</> },
            ]}
          />
        </div>
      </div>
      <p className="mt-2 text-xs text-ink-3">
        Joined {new Date(person.createdAt * 1000).toLocaleDateString(undefined, { dateStyle: 'medium' })}
        {' · '}
        {person.lastSeen ? `active ${relative(person.lastSeen)}` : 'hasn’t signed in yet'}
      </p>

      <div className="my-5 h-px bg-line" />

      {person.isAdmin ? (
        <Squircle radius={12} className="mb-4 flex items-center gap-3 bg-panel px-3 py-2.5 text-sm text-ink-2">
          <Crown className="size-4 shrink-0 text-ink" />
          Admins can do everything.
        </Squircle>
      ) : (
        <div className="mb-3 flex items-center gap-3">
          <p className="flex-1 text-xs text-ink-3">
            {custom ? 'Some settings are custom' : 'Follows the defaults'}
          </p>
          {custom && (
            <Button size="sm" variant="plain" onClick={() => setOverrides({})}>
              <RotateCcw className="size-3.5" /> Reset all
            </Button>
          )}
        </div>
      )}
      <div className="pl-3">
        <PermissionEditor
          value={person.permissions}
          defaults={defaults}
          overrides={person.isAdmin ? undefined : person.overrides}
          libraries={libraries}
          downloads={downloads}
          disabled={person.isAdmin}
          onChange={(changes) => setOverrides(override(person.overrides, defaults, changes))}
          onReset={(keys) => setOverrides(without(person.overrides, keys))}
        />
      </div>

      <div className="my-5 h-px bg-line" />
      <PasswordReset person={person} self={self} />
      <div className="mt-5 flex items-center justify-between gap-2">
        {!self ? (
          <Button
            variant="danger"
            onClick={async () =>
              (await ask({ title: `Remove ${person.username}?`, body: 'Their watch history and requests go too.', confirm: 'Remove', danger: true })) &&
              remove.mutate()
            }
          >
            <Trash2 className="size-4" /> Remove {person.username}
          </Button>
        ) : (
          <span />
        )}
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      </div>
    </Dialog>
  )
}

function NameField({ person, onSave }: { person: ManagedUser; onSave: (name: string) => void }) {
  const [name, setName] = useState(person.username)
  useEffect(() => setName(person.username), [person.username])
  const commit = () => {
    const n = name.trim()
    if (n && n !== person.username) onSave(n)
    else setName(person.username)
  }
  return (
    <Field label="Username">
      <Input value={name} onChange={(e) => setName(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === 'Enter' && commit()} />
    </Field>
  )
}

function PasswordReset({ person, self }: { person: ManagedUser; self: boolean }) {
  const [open, setOpen] = useState(false)
  const [password, setPassword] = useState('')
  const save = useMutation({
    mutationFn: () => request(UpdateUser, { id: person.id, input: { password } }),
    onSuccess: () => {
      setOpen(false)
      setPassword('')
      toast({ title: 'Password changed', body: self ? undefined : `${person.username} has been signed out everywhere.`, tone: 'ok' })
    },
    onError: toastError,
  })
  if (!open)
    return (
      <Button size="sm" variant="plain" onClick={() => setOpen(true)}>
        <KeyRound className="size-3.5" /> Set a new password
      </Button>
    )
  return (
    <div className="flex items-end gap-2">
      <div className="flex-1">
        <Field label={`New password for ${person.username}`}>
          <Input autoFocus type="text" value={password} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && password && save.mutate()} />
        </Field>
      </div>
      <Button variant="plain" onClick={() => (setOpen(false), setPassword(''))}>
        Cancel
      </Button>
      <Button variant="primary" disabled={!password || save.isPending} onClick={() => save.mutate()}>
        Save
      </Button>
    </div>
  )
}

function AddPerson({ onClose, onAdded }: { onClose: () => void; onAdded: (id: number) => void }) {
  const qc = useQueryClient()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [isAdmin, setIsAdmin] = useState(false)
  const create = useMutation({
    mutationFn: async () => (await request(CreateUser, { input: { username, password, isAdmin } })).createUser,
    onSuccess: async ({ id }) => {
      await qc.invalidateQueries({ queryKey: ['users'] })
      onAdded(id)
    },
  })
  return (
    <Dialog onClose={onClose}>
      <p className="text-[15px] font-medium">Add someone</p>
      <form
        className="mt-5 space-y-4"
        onSubmit={(e) => {
          e.preventDefault()
          if (username && password) create.mutate()
        }}
      >
        <Field label="Username">
          <Input autoFocus value={username} onChange={(e) => setUsername(e.target.value)} />
        </Field>
        <Field label="Password">
          <Input type="text" value={password} onChange={(e) => setPassword(e.target.value)} />
        </Field>
        <div>
          <p className="mb-1.5 text-[13px] text-ink-2">Role</p>
          <Segmented
            value={isAdmin ? 'admin' : 'member'}
            onChange={(v) => setIsAdmin(v === 'admin')}
            options={[
              { value: 'member', label: 'Member' },
              { value: 'admin', label: <><Crown className="size-3.5" /> Admin</> },
            ]}
          />
        </div>
        {create.error && <p className="text-sm text-danger">{(create.error as Error).message}</p>}
        <div className="flex justify-end gap-2 pt-2">
          <Button type="button" variant="plain" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" disabled={!username || !password || create.isPending}>
            Add
          </Button>
        </div>
      </form>
    </Dialog>
  )
}
/** Your own picture, in Settings → Account. */
export function Profile() {
  const me = useMe()
  if (!me) return null
  return (
    <Card title="Profile">
      <AvatarPicker user={me} />
    </Card>
  )
}
