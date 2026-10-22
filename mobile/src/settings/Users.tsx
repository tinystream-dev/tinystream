// SPDX-License-Identifier: AGPL-3.0-or-later
// The people on this server and what they may do (web's People.tsx): a
// list, each person in a sheet of their own, and the defaults everyone
// follows unless they have their own setting.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { relative } from '@tinystream/shared/downloads'
import {
  ArrowDownToLine,
  Clock3,
  Crown,
  Gauge,
  HardDrive,
  Inbox,
  KeyRound,
  Layers,
  Library,
  Link2,
  ListChecks,
  type LucideIcon,
  Plus,
  RotateCcw,
  Scissors,
  Sparkles,
  Trash2,
  Users as UsersIcon,
  Wand2,
  Zap,
} from 'lucide-react-native'
import { type ReactNode, useEffect, useState } from 'react'
import { Pressable, ScrollView, Text, View } from 'react-native'
import { Avatar } from '../components/Avatar'
import { AvatarPicker } from '../components/AvatarPicker'
import { ask, toast, toastError } from '../components/Feedback'
import { Sheet } from '../components/Sheet'
import { ListSkeleton } from '../components/Skeleton'
import { Badge, Button, Chip, Divider, ErrorText, Field, Group, Input, ListRow, Segmented, Stepper, Toggle } from '../components/ui'
import { Squircle } from '../effects/Squircle'
import { graphql } from '../gql'
import type { PermissionOverridesInput, UserPatch } from '../gql/graphql'
import { type Permissions, type User, useMe, useSettings, useStatus } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'

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
type ManagedUser = User & { createdAt: number; lastSeen?: number | null; overrides: Overrides }
type Key = keyof Permissions

/** Only what's set: the server says null for everything that follows the defaults. */
const setOnly = (o: Record<string, unknown>) => Object.fromEntries(Object.entries(o).filter(([, v]) => v != null)) as Overrides
/** Libraries are one setting made of two keys; they're overridden together. */
const LIBRARY_KEYS: Key[] = ['allLibraries', 'libraries']
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)

/** Sets some of someone's permissions; anything set back to the defaults' value follows them again. */
function override(o: Overrides, defaults: Permissions, changes: Partial<Permissions>): Overrides {
  const next: Overrides = { ...o, ...changes }
  const keys = Object.keys(changes) as Key[]
  const group = keys.some((k) => LIBRARY_KEYS.includes(k)) ? LIBRARY_KEYS : []
  if (group.length && group.every((k) => same({ ...defaults, ...next }[k], defaults[k]))) group.forEach((k) => delete next[k as keyof Overrides])
  for (const k of keys) if (!group.includes(k) && same(next[k as keyof Overrides], defaults[k])) delete next[k as keyof Overrides]
  return next
}

function without(o: Overrides, keys: Key[]): Overrides {
  const next = { ...o }
  keys.forEach((k) => delete next[k as keyof Overrides])
  return next
}

/** How many settings someone has their own value for (libraries count once). */
const customCount = (o: Overrides) => new Set(Object.keys(o).map((k) => (k === 'libraries' ? 'allLibraries' : k))).size

function summary(p: Permissions, isAdmin: boolean, libraries: string[], downloads: boolean): string {
  if (isAdmin) return 'Can do everything'
  const seen = libraries.filter((l) => p.allLibraries || p.libraries.includes(l))
  const parts = [p.allLibraries ? 'All libraries' : seen.length === 0 ? 'No libraries' : seen.length <= 2 ? seen.join(', ') : `${seen.length} libraries`]
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

export function Users() {
  const api = useApi()
  const me = useMe()
  const { tokens } = useTheme()
  const { data: users } = useQuery({
    queryKey: ['users'],
    queryFn: async (): Promise<ManagedUser[]> => (await api.request(UsersQuery)).users.map((u) => ({ ...u, overrides: setOnly(u.overrides) })),
  })
  const { data: defaults } = useQuery({ queryKey: ['permission-defaults'], queryFn: async () => (await api.request(DefaultsQuery)).permissionDefaults })
  const { data: settings } = useSettings()
  const [adding, setAdding] = useState(false)
  const [open, setOpen] = useState<number | null>(null)
  const libraries = settings?.libraries.map((l) => l.name) ?? []
  const downloads = !!useStatus().data?.server.downloads
  const person = users?.find((u) => u.id === open)
  if (!users) return <ListSkeleton rows={3} height={64} />
  return (
    <>
      <Button variant="primary" onPress={() => setAdding(true)} icon={<Plus size={17} color={tokens['on-accent']} />}>
        Add someone
      </Button>
      <Group title="Users">
        {users.map((u, i) => {
          const custom = u.isAdmin ? 0 : customCount(u.overrides)
          return (
            <View key={u.id}>
              {i > 0 && <Divider inset={64} />}
              <ListRow
                icon={<Avatar user={u} size={36} />}
                label={
                  <View className="flex-row flex-wrap items-center gap-2">
                    <Text className="font-sans text-[15px] font-medium text-ink">{u.username}</Text>
                    {u.id === me?.id && <Text className="font-sans text-sm text-ink-3">you</Text>}
                    {u.isAdmin && (
                      <Badge tone="strong" icon={<Crown size={11} color={tokens.canvas} />}>
                        Admin
                      </Badge>
                    )}
                    {custom > 0 && <Badge tone="live">{`${custom} custom`}</Badge>}
                  </View>
                }
                hint={`${summary(u.permissions, u.isAdmin, libraries, downloads)}\n${u.lastSeen ? `Active ${relative(u.lastSeen)}` : 'Never signed in'}`}
                onPress={() => setOpen(u.id)}
              />
            </View>
          )
        })}
      </Group>
      {defaults && (
        <Group title="Default permissions">
          <View className="px-4 pb-2">
            <DefaultsEditor defaults={defaults} users={users} libraries={libraries} downloads={downloads} />
          </View>
        </Group>
      )}
      <Sheet open={adding} onClose={() => setAdding(false)}>
        {adding && <AddPerson onAdded={(id) => (setAdding(false), setOpen(id))} />}
      </Sheet>
      <Sheet open={!!person && !!defaults} onClose={() => setOpen(null)}>
        {person && defaults && (
          <PersonForm key={person.id} person={person} defaults={defaults} libraries={libraries} downloads={downloads} onClose={() => setOpen(null)} />
        )}
      </Sheet>
    </>
  )
}

function DefaultsEditor({ defaults, users, libraries, downloads }: { defaults: Permissions; users: ManagedUser[]; libraries: string[]; downloads: boolean }) {
  const api = useApi()
  const qc = useQueryClient()
  const save = useMutation({
    mutationFn: (next: Permissions) => api.request(SetDefaults, { permissions: next }),
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
  return <PermissionEditor value={defaults} libraries={libraries} downloads={downloads} onChange={(changes) => save.mutate({ ...defaults, ...changes })} note={differ} />
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

const gb = (mb: number) => Math.round((mb / 1024) * 10) / 10

function PermissionEditor(props: EditorProps) {
  const { value: p, libraries, downloads, onChange, disabled } = props
  const row = (keys: Key[]) => ({ keys, ...props })
  const onOff = (v: boolean) => (v ? 'On' : 'Off')
  const limit = (n: number) => (n ? String(n) : 'No limit')
  const toggle = (key: 'watchTogether' | 'shareLinks' | 'request' | 'autoApprove' | 'clip' | 'clipLinks' | 'manageRequests' | 'manageShows' | 'downloads' | 'editMetadata', label: string) => (
    <Toggle label={label} value={p[key]} onChange={(v) => onChange({ [key]: v })} disabled={disabled} />
  )
  return (
    <View style={{ opacity: disabled ? 0.45 : 1 }} pointerEvents={disabled ? 'none' : 'auto'}>
      <Heading>Watching</Heading>
      <Setting {...row(LIBRARY_KEYS)} icon={Library} label="Libraries" was={(d) => (d.allLibraries ? 'All libraries' : d.libraries.length ? d.libraries.join(', ') : 'None')} wide>
        <LibraryPicker value={p} libraries={libraries} onChange={onChange} />
      </Setting>
      <Setting {...row(['watchTogether'])} icon={UsersIcon} label="Watch together" was={(d) => onOff(d.watchTogether)}>
        {toggle('watchTogether', 'Watch together')}
      </Setting>
      <Setting {...row(['shareLinks'])} icon={Link2} label="Public links" hint="For their rooms" was={(d) => onOff(d.shareLinks)} dim={!p.watchTogether}>
        {toggle('shareLinks', 'Public links')}
      </Setting>

      {downloads && (
        <>
          <Heading>Requests</Heading>
          <Setting {...row(['request'])} icon={Inbox} label="Request shows" was={(d) => onOff(d.request)}>
            {toggle('request', 'Request shows')}
          </Setting>
          <Setting {...row(['autoApprove'])} icon={Zap} label="Skip approval" was={(d) => onOff(d.autoApprove)} dim={!p.request}>
            {toggle('autoApprove', 'Skip approval')}
          </Setting>
          <Setting {...row(['requestLimit'])} icon={Gauge} label="Waiting requests" was={(d) => limit(d.requestLimit)} dim={!p.request || p.autoApprove}>
            <Stepper label="Waiting requests" value={p.requestLimit} onChange={(n) => onChange({ requestLimit: n })} />
          </Setting>
        </>
      )}

      <Heading>Clips</Heading>
      <Setting {...row(['clip'])} icon={Scissors} label="Make clips" was={(d) => onOff(d.clip)}>
        {toggle('clip', 'Make clips')}
      </Setting>
      <Setting {...row(['clipMaxLength'])} icon={Clock3} label="Longest clip" hint="In seconds" was={(d) => (d.clipMaxLength ? `${d.clipMaxLength} s` : 'No limit')} dim={!p.clip}>
        <Stepper label="Longest clip, in seconds" value={p.clipMaxLength} step={5} max={3600} onChange={(n) => onChange({ clipMaxLength: n })} />
      </Setting>
      <Setting {...row(['clipStorage'])} icon={HardDrive} label="Space for clips" hint="In GB" was={(d) => (d.clipStorage ? `${gb(d.clipStorage)} GB` : 'No limit')} dim={!p.clip}>
        <Stepper label="Space for clips, in GB" value={Math.round(p.clipStorage / 1024)} max={10000} onChange={(n) => onChange({ clipStorage: n * 1024 })} />
      </Setting>
      <Setting {...row(['clipLimit'])} icon={Layers} label="Rendered clips" was={(d) => limit(d.clipLimit)} dim={!p.clip}>
        <Stepper label="Rendered clips" value={p.clipLimit} max={9999} onChange={(n) => onChange({ clipLimit: n })} />
      </Setting>
      <Setting {...row(['clipLinks'])} icon={Link2} label="Public clip links" was={(d) => onOff(d.clipLinks)} dim={!p.clip}>
        {toggle('clipLinks', 'Public clip links')}
      </Setting>

      <Heading>Management</Heading>
      {downloads && (
        <>
          <Setting {...row(['manageRequests'])} icon={ListChecks} label="Approve requests" was={(d) => onOff(d.manageRequests)}>
            {toggle('manageRequests', 'Approve requests')}
          </Setting>
          <Setting {...row(['manageShows'])} icon={Sparkles} label="Manage shows" hint="Add, monitor and search" was={(d) => onOff(d.manageShows)}>
            {toggle('manageShows', 'Manage shows')}
          </Setting>
          <Setting {...row(['downloads'])} icon={ArrowDownToLine} label="Downloads" hint="Everyone's, and grab releases" was={(d) => onOff(d.downloads)}>
            {toggle('downloads', 'Downloads')}
          </Setting>
        </>
      )}
      <Setting {...row(['editMetadata'])} icon={Wand2} label="Fix details" hint="Rematch and refresh titles" was={(d) => onOff(d.editMetadata)}>
        {toggle('editMetadata', 'Fix details')}
      </Setting>
    </View>
  )
}

function Heading({ children }: { children: string }) {
  return <Text className="font-sans mb-1 mt-5 text-2xs font-medium uppercase tracking-wider text-ink-3">{children}</Text>
}

function Setting({
  keys,
  icon: Icon,
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
}: EditorProps & { keys: Key[]; icon: LucideIcon; label: string; hint?: string; was: (d: Permissions) => string; dim?: boolean; wide?: boolean; children: ReactNode }) {
  const { tokens } = useTheme()
  const custom = !!overrides && keys.some((k) => k in overrides)
  const line = note?.(keys)
  return (
    <View className="border-t border-line py-3" style={{ opacity: dim ? 0.45 : 1 }}>
      {custom && <View className="absolute bottom-3 top-3 w-0.5 rounded-full bg-info/70" style={{ left: -10 }} />}
      <View className="flex-row items-center gap-3">
        <View className="h-7 w-7 items-center justify-center rounded-lg bg-panel">
          <Icon size={14} color={tokens['ink-2']} />
        </View>
        <View className="min-w-0 flex-1">
          <View className="flex-row flex-wrap items-center gap-2">
            <Text className="font-sans text-sm text-ink">{label}</Text>
            {custom && defaults && onReset && (
              <Pressable onPress={() => onReset(keys)} className="flex-row items-center gap-1 rounded-md bg-info-deep/15 px-1.5 py-0.5">
                <RotateCcw size={11} color={tokens.info} />
                <Text className="font-sans text-2xs font-medium text-info">Use default: {was(defaults)}</Text>
              </Pressable>
            )}
          </View>
          {hint && <Text className="font-sans mt-0.5 text-xs text-ink-3">{hint}</Text>}
          {line && <Text className="font-sans mt-0.5 text-xs text-info/80">{line}</Text>}
        </View>
        {!wide && children}
      </View>
      {wide && <View className="mt-3 pl-10">{children}</View>}
    </View>
  )
}

function LibraryPicker({ value, libraries, onChange }: { value: Permissions; libraries: string[]; onChange: (c: Partial<Permissions>) => void }) {
  if (libraries.length === 0) return <Text className="font-sans text-xs text-ink-3">Add a library first.</Text>
  return (
    <View className="flex-row flex-wrap items-center gap-1.5">
      <Chip on={value.allLibraries} onPress={() => onChange({ allLibraries: !value.allLibraries, libraries: value.allLibraries ? libraries : value.libraries })}>
        All
      </Chip>
      {libraries.map((l) => {
        const on = value.allLibraries || value.libraries.includes(l)
        return (
          <Chip
            key={l}
            on={on}
            soft={value.allLibraries}
            onPress={() => {
              // Picking one out of "all" means everything but that one.
              const current = value.allLibraries ? libraries : value.libraries
              onChange({ allLibraries: false, libraries: on ? current.filter((x) => x !== l) : [...current, l] })
            }}
          >
            {l}
          </Chip>
        )
      })}
    </View>
  )
}

function PersonForm({ person, defaults, libraries, downloads, onClose }: { person: ManagedUser; defaults: Permissions; libraries: string[]; downloads: boolean; onClose: () => void }) {
  const api = useApi()
  const me = useMe()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const self = person.id === me?.id
  const update = useMutation({
    mutationFn: (body: UserPatch) => api.request(UpdateUser, { id: person.id, input: body }),
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
            permissions: isAdmin ? u.permissions : { ...defaults, ...(overrides as Partial<Permissions>) },
          }
        }),
      )
    },
    onError: toastError,
    onSettled: () => void qc.invalidateQueries({ queryKey: ['users'] }),
  })
  const remove = useMutation({
    mutationFn: () => api.request(DeleteUser, { id: person.id }),
    onSuccess: () => (onClose(), void qc.invalidateQueries({ queryKey: ['users'] })),
    onError: toastError,
  })
  const setOverrides = (o: Overrides) => update.mutate({ permissions: o })
  const custom = Object.keys(person.overrides).length > 0

  return (
    <ScrollView style={{ maxHeight: 660 }} keyboardShouldPersistTaps="handled">
      <View className="gap-4 pb-2">
        <AvatarPicker user={person} userId={person.id} size={64} />
        <NameField person={person} onSave={(username) => update.mutate({ username })} />
        <Field label="Role">
          <Segmented
            value={person.isAdmin ? 'admin' : 'member'}
            onChange={async (v) => {
              const admin = v === 'admin'
              if (admin && !(await ask({ title: `Make ${person.username} an admin?`, body: 'Admins can do everything, including removing you.', confirm: 'Make admin' })))
                return
              update.mutate({ isAdmin: admin })
            }}
            options={[
              { value: 'member', label: 'Member' },
              { value: 'admin', label: 'Admin' },
            ]}
          />
        </Field>
        <Text className="font-sans text-xs text-ink-3">
          Joined {new Date(person.createdAt * 1000).toLocaleDateString(undefined, { dateStyle: 'medium' })} ·{' '}
          {person.lastSeen ? `active ${relative(person.lastSeen)}` : 'hasn’t signed in yet'}
        </Text>
        {person.isAdmin ? (
          <Squircle radius={12} className="flex-row items-center gap-3 bg-panel px-3 py-2.5">
            <Crown size={16} color={tokens.ink} />
            <Text className="font-sans text-sm text-ink-2">Admins can do everything.</Text>
          </Squircle>
        ) : (
          <View className="flex-row items-center gap-3">
            <Text className="font-sans flex-1 text-xs text-ink-3">{custom ? 'Some settings are custom' : 'Follows the defaults'}</Text>
            {custom && (
              <Button size="sm" variant="plain" onPress={() => setOverrides({})} icon={<RotateCcw size={14} color={tokens['ink-2']} />}>
                Reset all
              </Button>
            )}
          </View>
        )}
        <View className="pl-3">
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
        </View>
        <PasswordReset person={person} self={self} />
        {!self && (
          <Button
            variant="danger"
            size="lg"
            icon={<Trash2 size={17} color={tokens.danger} />}
            onPress={async () =>
              (await ask({ title: `Remove ${person.username}?`, body: 'Their watch history and requests go too.', confirm: 'Remove', danger: true })) && remove.mutate()
            }
          >
            Remove {person.username}
          </Button>
        )}
      </View>
    </ScrollView>
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
      <Input value={name} onChangeText={setName} onBlur={commit} onSubmitEditing={commit} autoCapitalize="none" autoCorrect={false} />
    </Field>
  )
}

function PasswordReset({ person, self }: { person: ManagedUser; self: boolean }) {
  const api = useApi()
  const { tokens } = useTheme()
  const [open, setOpen] = useState(false)
  const [password, setPassword] = useState('')
  const save = useMutation({
    mutationFn: () => api.request(UpdateUser, { id: person.id, input: { password } }),
    onSuccess: () => {
      setOpen(false)
      setPassword('')
      toast({ title: 'Password changed', body: self ? undefined : `${person.username} has been signed out everywhere.`, tone: 'ok' })
    },
    onError: toastError,
  })
  if (!open)
    return (
      <View className="flex-row">
        <Button size="sm" variant="plain" onPress={() => setOpen(true)} icon={<KeyRound size={14} color={tokens['ink-2']} />}>
          Set a new password
        </Button>
      </View>
    )
  return (
    <View className="gap-2">
      <Field label={`New password for ${person.username}`}>
        <Input value={password} onChangeText={setPassword} autoCapitalize="none" autoCorrect={false} onSubmitEditing={() => password && save.mutate()} />
      </Field>
      <View className="flex-row gap-2">
        <Button className="flex-1" variant="plain" onPress={() => (setOpen(false), setPassword(''))}>
          Cancel
        </Button>
        <Button className="flex-1" variant="primary" disabled={!password || save.isPending} onPress={() => save.mutate()}>
          Save
        </Button>
      </View>
    </View>
  )
}

function AddPerson({ onAdded }: { onAdded: (id: number) => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [isAdmin, setIsAdmin] = useState(false)
  const create = useMutation({
    mutationFn: async () => (await api.request(CreateUser, { input: { username, password, isAdmin } })).createUser,
    onSuccess: async ({ id }) => {
      await qc.invalidateQueries({ queryKey: ['users'] })
      onAdded(id)
    },
  })
  return (
    <View className="gap-4">
      <Text className="font-sans text-lg font-semibold text-ink">Add someone</Text>
      <Field label="Username">
        <Input value={username} onChangeText={setUsername} autoCapitalize="none" autoCorrect={false} />
      </Field>
      <Field label="Password">
        <Input value={password} onChangeText={setPassword} autoCapitalize="none" autoCorrect={false} />
      </Field>
      <Field label="Role">
        <Segmented
          value={isAdmin ? 'admin' : 'member'}
          onChange={(v) => setIsAdmin(v === 'admin')}
          options={[
            { value: 'member', label: 'Member' },
            { value: 'admin', label: 'Admin' },
          ]}
        />
      </Field>
      {create.error && <ErrorText>{(create.error as Error).message}</ErrorText>}
      <Button variant="primary" size="lg" disabled={!username || !password || create.isPending} onPress={() => create.mutate()}>
        Add
      </Button>
    </View>
  )
}
