// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import {
  ArrowDownToLine,
  BookOpen,
  ChevronRight,
  CircleUser,
  CornerLeftUp,
  FileCode,
  FileX,
  Folder,
  FolderPen,
  Heart,
  KeyRound,
  Library,
  type LucideIcon,
  Palette,
  Plus,
  RefreshCw,
  Rss,
  Music,
  Scissors,
  Server as ServerIcon,
  SlidersHorizontal,
  Trash2,
  Users,
  Zap,
} from 'lucide-react'
import { type ReactNode, useEffect, useState } from 'react'
import { ask, toast, toastError } from '../components/feedback'
import { Page, PageTitle } from '../components/Page'
import { People, Profile } from '../components/People'
import { AutomationSettings, DownloadsSettings, ProfilesSettings, RenamesSettings, SourcesSettings } from '../components/DownloadSettings'
import { AppearanceSettings } from '../components/AppearanceSettings'
import { ClipSettings } from '../components/ClipSettings'
import { Credits } from '../components/Credits'
import { AppPasswords, MusicSettings } from '../components/MusicSettings'
import { Card, Row, useSave, useSettings } from '../components/SettingsKit'
import { Squircle } from '../components/Squircle'
import { Button, Dialog as SharedDialog, Field, IconButton, Input, Segmented, Select, Tip, Toggle } from '../components/ui'
import { graphql } from '../gql'
import { type ConfigLibrary, type Provider, type Settings, request } from '../lib/api'

const Scan = graphql(`
  mutation Scan($library: String) {
    scan(library: $library)
  }
`)

const AddLibrary = graphql(`
  mutation AddLibrary($input: LibraryInput!) {
    addLibrary(input: $input) {
      raw
    }
  }
`)

const UpdateLibrary = graphql(`
  mutation UpdateLibrary($name: String!, $input: LibraryInput!) {
    updateLibrary(name: $name, input: $input) {
      raw
    }
  }
`)

const RemoveLibrary = graphql(`
  mutation RemoveLibrary($name: String!) {
    removeLibrary(name: $name) {
      raw
    }
  }
`)

const FoldersQuery = graphql(`
  query Folders($path: String) {
    folders(path: $path) {
      path
      parent
      home
      folders {
        name
        path
      }
    }
  }
`)

const SaveServer = graphql(`
  mutation SaveServer($patch: ConfigPatch!) {
    updateSettings(patch: $patch) {
      raw
    }
  }
`)

const PasskeysQuery = graphql(`
  query Passkeys {
    viewer {
      passkeys {
        id
        name
        createdAt
        lastUsed
      }
    }
  }
`)

const StartRegistration = graphql(`
  mutation StartPasskeyRegistration($name: String) {
    startPasskeyRegistration(name: $name) {
      challenge
      options
    }
  }
`)

const FinishRegistration = graphql(`
  mutation FinishPasskeyRegistration($challenge: String!, $credential: JSON!) {
    finishPasskeyRegistration(challenge: $challenge, credential: $credential) {
      id
    }
  }
`)

const DeletePasskey = graphql(`
  mutation DeletePasskey($id: Int!) {
    deletePasskey(id: $id) {
      id
    }
  }
`)

const ChangePassword = graphql(`
  mutation ChangePassword($current: String!, $new: String!) {
    changePassword(current: $current, new: $new)
  }
`)

const EmailQuery = graphql(`
  query MyEmail {
    viewer {
      email
    }
  }
`)

const SetEmail = graphql(`
  mutation SetEmail($email: String, $password: String!) {
    setEmail(email: $email, password: $password)
  }
`)

const SendTestEmail = graphql(`
  mutation SendTestEmail($to: String!) {
    sendTestEmail(to: $to)
  }
`)

const ReplaceConfig = graphql(`
  mutation ReplaceConfig($text: String!) {
    replaceConfig(text: $text) {
      raw
    }
  }
`)

const SkippedQuery = graphql(`
  query SkippedFiles {
    skippedFiles {
      library
      path
      reason
    }
  }
`)

/** The parts of config.toml the Server tab edits. */
type ServerConfig = Pick<Settings, 'network' | 'log' | 'scan' | 'metadata' | 'transcode' | 'signIn' | 'smtp' | 'tailscale'>
const serverPart = ({ network, log, scan, metadata, transcode, signIn, smtp, tailscale }: Settings): ServerConfig => ({ network, log, scan, metadata, transcode, signIn, smtp, tailscale })
const SMTP_PORTS = { STARTTLS: 587, TLS: 465, NONE: 25 }
import { useEmailOn, useMe, useStatus } from '../lib/hooks'
import { notifyEnabled, setNotify } from '../lib/notify'
import { useTitle } from '../lib/title'
import { createCredential } from '../lib/webauthn'

type Tab = 'libraries' | 'server' | 'clips' | 'music' | 'downloads' | 'sources' | 'profiles' | 'automation' | 'renames' | 'users' | 'account' | 'appearance' | 'file' | 'skipped' | 'credits'

export const Route = createFileRoute('/settings')({
  validateSearch: (s: Record<string, unknown>): { tab?: Tab } => ({ tab: s.tab as Tab | undefined }),
  component: SettingsPage,
})

function SettingsPage() {
  useTitle('Settings')
  const me = useMe()
  const { tab } = Route.useSearch()
  const navigate = useNavigate()
  const admin = !!me?.isAdmin
  const current: Tab = tab ?? 'account'
  const { data: settings } = useSettings()
  const downloads = admin && !!settings?.downloads
  const sections: { title: string; tabs: { id: Tab; label: string; icon: LucideIcon }[] }[] = [
    {
      title: 'You',
      tabs: [
        { id: 'account', label: 'Account', icon: CircleUser },
        { id: 'appearance', label: 'Appearance', icon: Palette },
      ],
    },
    ...(admin
      ? [
          {
            title: 'Library',
            tabs: [
              { id: 'libraries', label: 'Libraries', icon: Library },
              { id: 'music', label: 'Music', icon: Music },
              { id: 'clips', label: 'Clips', icon: Scissors },
              { id: 'skipped', label: 'Skipped files', icon: FileX },
            ],
          },
          ...(downloads
            ? [
                {
                  title: 'Downloads',
                  tabs: [
                    { id: 'downloads', label: 'Torrents', icon: ArrowDownToLine },
                    { id: 'sources', label: 'Sources', icon: Rss },
                    { id: 'profiles', label: 'Quality profiles', icon: SlidersHorizontal },
                    { id: 'automation', label: 'Automation', icon: Zap },
                    { id: 'renames', label: 'Renames', icon: FolderPen },
                  ],
                },
              ]
            : []),
          {
            title: 'Server',
            tabs: [
              { id: 'server', label: 'General', icon: ServerIcon },
              { id: 'users', label: 'Users', icon: Users },
              { id: 'file', label: 'Config file', icon: FileCode },
            ],
          },
        ]
      : []),
    {
      title: 'About',
      tabs: [{ id: 'credits', label: 'Credits', icon: Heart }],
    },
  ] as { title: string; tabs: { id: Tab; label: string; icon: LucideIcon }[] }[]

  return (
    <Page>
      <PageTitle>Settings</PageTitle>
      <div className="flex flex-col gap-8 md:flex-row">
        <nav className="flex shrink-0 gap-0.5 overflow-x-auto md:w-48 md:flex-col md:gap-5">
          {sections.map((section) => (
            <div key={section.title} className="contents md:flex md:flex-col md:gap-0.5">
              <p className="mb-1 hidden px-2.5 text-2xs font-medium tracking-wide text-ink-3 uppercase md:block">{section.title}</p>
              {section.tabs.map((t) => (
                <Squircle
                  key={t.id}
                  as="button"
                  radius={8}
                  onClick={() => navigate({ to: '/settings', search: { tab: t.id }, replace: true })}
                  className={`flex h-8 shrink-0 items-center gap-2.5 px-2.5 text-left text-sm transition-colors ${current === t.id ? 'bg-press text-ink' : 'text-ink-2 hover:bg-hover hover:text-ink'}`}
                >
                  <t.icon className={`size-4 shrink-0 ${current === t.id ? 'text-ink' : 'text-ink-3'}`} />
                  {t.label}
                </Squircle>
              ))}
            </div>
          ))}
        </nav>
        <div className="min-w-0 flex-1">
          {admin && <ConfigErrorBanner />}
          {current === 'libraries' && admin && <Libraries />}
          {current === 'server' && admin && <Server />}
          {current === 'clips' && admin && <ClipSettings />}
          {current === 'music' && admin && <MusicSettings />}
          {current === 'downloads' && downloads && <DownloadsSettings />}
          {current === 'sources' && downloads && <SourcesSettings />}
          {current === 'profiles' && downloads && <ProfilesSettings />}
          {current === 'automation' && downloads && <AutomationSettings />}
          {current === 'renames' && downloads && <RenamesSettings />}
          {current === 'users' && admin && <People />}
          {current === 'account' && <Account />}
          {current === 'appearance' && <AppearanceSettings />}
          {current === 'file' && admin && <RawConfig />}
          {current === 'skipped' && admin && <Skipped />}
          {current === 'credits' && <Credits />}
        </div>
      </div>
    </Page>
  )
}

function ConfigErrorBanner() {
  const { data } = useSettings()
  if (!data?.error) return null
  return (
    <Squircle radius={14} edge className="mb-6 bg-danger/10 p-4">
      <p className="text-sm font-medium text-danger">config.toml has a mistake, so your last edit wasn't applied</p>
      <pre className="mt-2 overflow-x-auto text-xs leading-relaxed whitespace-pre-wrap text-ink-2">{data.error}</pre>
    </Squircle>
  )
}
const providers: { value: '' | Provider; label: string }[] = [
  { value: '', label: 'None (folder names)' },
  { value: 'ANILIST', label: 'AniList (anime)' },
  { value: 'TMDB', label: 'TMDB (shows and movies)' },
]

function Libraries() {
  const { data } = useSettings()
  const [editing, setEditing] = useState<ConfigLibrary | 'new' | null>(null)
  const saved = useSave()
  const scan = useMutation({ mutationFn: (library?: string) => request(Scan, { library: library ?? null }) })
  if (!data) return null
  return (
    <>
      <Card
        title="Libraries"
        aside={
          <div className="flex gap-2">
            {data.libraries.length > 0 && (
              <Button onClick={() => scan.mutate(undefined)}>
                <RefreshCw className="size-3.5" /> Rescan all
              </Button>
            )}
            <Button variant="primary" onClick={() => setEditing('new')}>
              <Plus className="size-4" /> Add
            </Button>
          </div>
        }
      >
        {data.libraries.length === 0 && (
          <p className="py-6 text-center text-sm text-ink-3">No libraries yet.</p>
        )}
        <div className="-mx-2 space-y-0.5">
          {data.libraries.map((l) => (
            <Squircle
              key={l.name}
              as="button"
              radius={12}
              onClick={() => setEditing(l)}
              className="flex w-full items-center gap-4 px-3 py-2.5 text-left transition-colors hover:bg-hover"
            >
              {l.kind === 'MUSIC' ? (
                <Music className="size-4.5 shrink-0 text-ink-3" />
              ) : l.kind === 'BOOKS' ? (
                <BookOpen className="size-4.5 shrink-0 text-ink-3" />
              ) : (
                <Folder className="size-4.5 shrink-0 text-ink-3" />
              )}
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium">{l.name}</p>
                <p className={`truncate text-xs ${l.exists ? 'text-ink-3' : 'text-danger'}`}>
                  {l.error ?? (l.exists ? l.resolvedPath : `${l.resolvedPath} can't be found`)}
                </p>
              </div>
              {l.managed && <span className="shrink-0 rounded-md bg-panel px-1.5 py-0.5 text-2xs text-ink-2">Managed</span>}
              <span className="shrink-0 text-xs text-ink-3 tabular">
                {l.titleCount} {l.kind === 'MUSIC' ? 'albums' : l.kind === 'BOOKS' ? 'books' : 'titles'}{l.skippedCount > 0 && `, ${l.skippedCount} skipped`}
              </span>
              <ChevronRight className="size-4 shrink-0 text-ink-3" />
            </Squircle>
          ))}
        </div>
      </Card>
      {editing && (
        <LibraryDialog
          library={editing === 'new' ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            saved()
            setEditing(null)
          }}
        />
      )}
    </>
  )
}

function LibraryDialog({ library, onClose, onSaved }: { library: ConfigLibrary | null; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState(library?.name ?? '')
  const [path, setPath] = useState(library?.path ?? '')
  const [kind, setKind] = useState<'VIDEO' | 'MUSIC' | 'BOOKS'>(library?.kind ?? 'VIDEO')
  const [provider, setProvider] = useState<'' | Provider>(library?.metadataProvider ?? '')
  const [managed, setManaged] = useState(library?.managed ?? false)
  const [profile, setProfile] = useState(library?.profile ?? '')
  const [downloadPath, setDownloadPath] = useState(library?.downloadPath ?? '')
  const { data: settings } = useSettings()
  const [browsing, setBrowsing] = useState(!library)
  const [error, setError] = useState<string | null>(null)
  const music = kind === 'MUSIC'
  const video = kind === 'VIDEO'
  const body = {
    name,
    path,
    kind,
    metadataProvider: video ? provider || null : null,
    managed: video ? managed : false,
    profile: video ? profile || null : null,
    downloadPath: downloadPath.trim() || null,
  }
  const save = useMutation({
    mutationFn: async () => void (library ? await request(UpdateLibrary, { name: library.name, input: body }) : await request(AddLibrary, { input: body })),
    onSuccess: onSaved,
    onError: (e) => setError((e as Error).message),
  })
  const remove = useMutation({ mutationFn: () => request(RemoveLibrary, { name: library!.name }), onSuccess: onSaved })

  return (
    <Dialog onClose={onClose}>
      <p className="text-[15px] font-medium">{library ? `Edit ${library.name}` : 'Add a library'}</p>
      <div className="mt-5 space-y-4">
        <Segmented
          value={kind}
          onChange={setKind}
          options={[
            { value: 'VIDEO', label: 'Shows and movies' },
            { value: 'MUSIC', label: 'Music' },
            { value: 'BOOKS', label: 'Books' },
          ]}
        />
        <Field label="Name">
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder={music ? 'Music' : video ? 'Anime' : 'Novels'} autoFocus={!!library} />
        </Field>
        <Field label="Folder">
          <div className="flex gap-2">
            <div className="flex-1">
              <Input value={path} onChange={(e) => setPath(e.target.value)} placeholder="~/Videos/Anime" />
            </div>
            <Button onClick={() => setBrowsing((b) => !b)}>Browse</Button>
          </div>
        </Field>
        {browsing && (
          <FolderPicker
            start={path}
            onPick={(p) => {
              setPath(p)
              if (!name) setName(p.split('/').filter(Boolean).pop() ?? '')
            }}
          />
        )}
        {music ? (
          <p className="text-xs leading-relaxed text-ink-3">Albums and artists come from the files' tags, however the folders are laid out.</p>
        ) : !video ? (
          <p className="text-xs leading-relaxed text-ink-3">
            EPUB, FB2 and plain text books, in any folders. What a book doesn't say about itself is looked up on Apple Books and Open Library.
          </p>
        ) : (
          <Field label="Metadata provider">
            <Select value={provider} options={providers} onChange={setProvider} />
          </Field>
        )}
        {settings?.downloads && video && (
          <>
            <Row label="Managed" hint="Off = read-only">
              <Toggle label="Managed" checked={managed} onChange={setManaged} />
            </Row>
            {managed && (
              <div className="grid gap-4 sm:grid-cols-2">
                <Field label="Quality profile">
                  <Select
                    value={profile}
                    options={[
                      { value: '', label: settings.profiles[0]?.name ? `First profile (${settings.profiles[0].name})` : 'Built-in' },
                      ...settings.profiles.map((p) => ({ value: p.name, label: p.name })),
                    ]}
                    onChange={setProfile}
                  />
                </Field>
                <Field label="Download folder">
                  <Input value={downloadPath} onChange={(e) => setDownloadPath(e.target.value)} placeholder="Global download folder" />
                </Field>
              </div>
            )}
          </>
        )}
        {error && <p className="text-sm text-danger">{error}</p>}
      </div>
      <div className="mt-6 flex items-center gap-2">
        {library && (
          <Button
            variant="danger"
            onClick={async () =>
              (await ask({ title: `Remove ${library.name}?`, body: 'Your files stay where they are.', confirm: 'Remove', danger: true })) &&
              remove.mutate()
            }
          >
            <Trash2 className="size-4" /> Remove
          </Button>
        )}
        <div className="flex-1" />
        <Button variant="plain" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" disabled={!name.trim() || !path.trim() || save.isPending} onClick={() => save.mutate()}>
          {library ? 'Save' : 'Add library'}
        </Button>
      </div>
    </Dialog>
  )
}


function FolderPicker({ start, onPick }: { start: string; onPick: (p: string) => void }) {
  const path = start.trim() || '~'
  const [at, setAt] = useState(path)
  useEffect(() => {
    const timer = setTimeout(() => setAt(path), 200)
    return () => clearTimeout(timer)
  }, [path])
  const { data, error, isFetching } = useQuery({
    queryKey: ['browse', at],
    queryFn: async () => (await request(FoldersQuery, { path: at || null })).folders,
    retry: false,
  })
  const loading = path !== at || isFetching
  const pretty = (p: string) => (data?.home && p.startsWith(data.home) ? `~${p.slice(data.home.length)}` : p)
  const pick = (p: string) => {
    onPick(pretty(p))
  }
  return (
    <Squircle radius={12} edge className="bg-canvas">
      <div className="flex items-center gap-1 border-b border-line px-2 py-1.5">
        <IconButton label="Up a folder" disabled={loading || !!error || !data?.parent} onClick={() => data?.parent && pick(data.parent)}>
          <CornerLeftUp className="size-4" />
        </IconButton>
        <span className="truncate text-xs text-ink-2">{loading ? path : data ? pretty(data.path) : path}</span>
      </div>
      <div className="max-h-56 overflow-y-auto p-1" aria-busy={loading}>
        {loading && <p className="p-3 text-xs text-ink-3">Loading folders…</p>}
        {!loading && error && <p className="p-3 text-xs text-danger">{(error as Error).message}</p>}
        {!loading && !error && data?.folders.length === 0 && <p className="p-3 text-xs text-ink-3">No folders in here.</p>}
        {!loading && !error && data?.folders.map((e) => (
          <button
            key={e.path}
            onClick={() => pick(e.path)}
            className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13px] text-ink-2 hover:bg-hover hover:text-ink"
          >
            <Folder className="size-3.5 shrink-0 text-ink-3" />
            <span className="truncate">{e.name}</span>
          </button>
        ))}
      </div>
    </Squircle>
  )
}

function Dialog({ children, onClose, wide }: { children: ReactNode; onClose: () => void; wide?: boolean }) {
  return (
    <SharedDialog onClose={onClose} width={wide ? 'max-w-2xl' : 'max-w-lg'}>
      {children}
    </SharedDialog>
  )
}
function Server() {
  const { data } = useSettings()
  const smtp = !!useStatus().data?.server.smtp
  const [draft, setDraft] = useState<ServerConfig | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)
  const refresh = useSave()
  useEffect(() => {
    if (data && !draft) setDraft(structuredClone(serverPart(data)))
  }, [data, draft])
  const save = useMutation({
    mutationFn: () => request(SaveServer, { patch: draft! }),
    onSuccess: () => {
      setError(null)
      setSaved(true)
      setTimeout(() => setSaved(false), 1600)
      refresh()
    },
    onError: (e) => setError((e as Error).message),
  })
  if (!data || !draft) return null
  const set = (fn: (c: ServerConfig) => void) => {
    const c = structuredClone(draft)
    fn(c)
    setDraft(c)
  }
  const dirty = JSON.stringify(draft) !== JSON.stringify(serverPart(data))
  const t = data.transcoding
  const expose = async (v: ServerConfig['tailscale']['expose']) => {
    if (v === 'PUBLIC' && draft.tailscale.expose !== 'PUBLIC') {
      const ok = await ask({
        title: 'Open tinystream to the internet?',
        body: draft.signIn.publicProfiles
          ? 'Funnel lets anyone reach the sign-in page, not just your tailnet, and it keeps showing everyone’s profile picture.'
          : 'Funnel lets anyone reach the sign-in page, not just your tailnet. While it’s on, the sign-in page asks for a username instead of showing everyone’s profile picture.',
        confirm: 'Open it',
        danger: true,
      })
      if (!ok) return
    }
    set((c) => void (c.tailscale.expose = v))
  }
  const publicProfiles = async (v: boolean) => {
    if (v) {
      const ok = await ask({
        title: 'Show profiles to the internet?',
        body: 'Anyone who finds this address sees every account’s name and picture, and only needs a password to sign in as one.',
        confirm: 'Show them',
        danger: true,
      })
      if (!ok) return
    }
    set((c) => void (c.signIn.publicProfiles = v))
  }
  const onInternet = draft.tailscale.expose === 'PUBLIC' || data.tailscaleStatus.funnel

  return (
    <>
      <Card title="Network">
        <div className="grid gap-4 sm:grid-cols-[1fr_8rem]">
          <Field label="Host">
            <Input value={draft.network.host} onChange={(e) => set((c) => void (c.network.host = e.target.value))} />
          </Field>
          <Field label="Port">
            <Input
              inputMode="numeric"
              value={draft.network.port}
              onChange={(e) => set((c) => void (c.network.port = Number(e.target.value.replace(/\D/g, '')) || 0))}
            />
          </Field>
        </div>
        <div className="mt-4">
          <Field label="Allowed origins (CORS)" hint="Comma-separated, or *">
            <Input
              value={draft.network.cors.join(', ')}
              placeholder="https://example.com"
              onChange={(e) =>
                set((c) => void (c.network.cors = e.target.value.split(',').map((s) => s.trim()).filter(Boolean)))
              }
            />
          </Field>
        </div>
        <div className="mt-4">
          <Field label="Public address" hint="Where links in invites, emails and shares point">
            <Input
              value={draft.network.publicUrl ?? ''}
              placeholder={data.tailscaleStatus.url ?? 'https://tv.example.com'}
              onChange={(e) => set((c) => void (c.network.publicUrl = e.target.value.trim() || null))}
            />
          </Field>
          <p className="mt-1.5 text-xs text-ink-3">
            {data.network.publicUrl
              ? `Links point to ${data.publicUrl}.`
              : data.publicUrl
                ? `Links point to ${data.publicUrl}, found through Tailscale.`
                : 'Links point to whichever address each person opened tinystream at.'}
          </p>
        </div>
        <div className="mt-5 space-y-4 border-t border-line pt-4">
          <Row label="Find this machine’s tailnet address" hint={tailnet(data.tailscaleStatus)}>
            <Toggle label="Detect Tailscale" checked={draft.tailscale.detect} onChange={(v) => set((c) => void (c.tailscale.detect = v))} />
          </Row>
          {draft.tailscale.detect && (
            <>
              <Field label="tailscaled socket" hint="Only if it isn’t in the usual place, like in Docker">
                <Input
                  value={draft.tailscale.socket ?? ''}
                  placeholder="/var/run/tailscale/tailscaled.sock"
                  onChange={(e) => set((c) => void (c.tailscale.socket = e.target.value.trim() || null))}
                />
              </Field>
              <div className="grid gap-4 sm:grid-cols-[1fr_8rem]">
                <Field label="Tailscale Serve">
                  <Select
                    value={draft.tailscale.expose}
                    options={[
                      { value: 'MANUAL', label: 'Leave it as it is' },
                      { value: 'TAILNET', label: 'HTTPS on the tailnet' },
                      { value: 'PUBLIC', label: 'HTTPS, open to the internet (Funnel)' },
                    ]}
                    onChange={(v) => void expose(v)}
                  />
                </Field>
                {draft.tailscale.expose !== 'MANUAL' && (
                  <Field label="HTTPS port">
                    <Input
                      inputMode="numeric"
                      value={draft.tailscale.httpsPort}
                      onChange={(e) => set((c) => void (c.tailscale.httpsPort = Number(e.target.value.replace(/\D/g, '')) || 0))}
                    />
                  </Field>
                )}
              </div>
            </>
          )}
          {data.tailscaleStatus.serveError && <p className="text-xs leading-relaxed text-danger">{data.tailscaleStatus.serveError}</p>}
          {data.tailscaleStatus.warning && <p className="text-xs leading-relaxed text-warn">{data.tailscaleStatus.warning}</p>}
        </div>
      </Card>

      <Card title="Sign-in">
        <div className="max-w-sm">
          <Field label="Sign-in page">
            <Select
              value={draft.signIn.style}
              options={[
                { value: 'PROFILES', label: 'Profile pictures' },
                { value: 'USERNAME', label: 'Username and password' },
              ]}
              onChange={(v) => set((c) => void (c.signIn.style = v))}
            />
          </Field>
        </div>
        {onInternet && draft.signIn.style === 'PROFILES' && (
          <div className="mt-4">
            <Row label="Keep profile pictures while open to the internet" hint={draft.signIn.publicProfiles ? undefined : 'It asks for a username while Funnel has tinystream open to the internet.'}>
              <Toggle label="Keep profile pictures" checked={draft.signIn.publicProfiles} onChange={(v) => void publicProfiles(v)} />
            </Row>
            {draft.signIn.publicProfiles && (
              <p className="mt-1.5 text-xs leading-relaxed text-warn">Anyone who finds this address sees every account’s name and picture, and only needs a password to sign in as one.</p>
            )}
          </div>
        )}
      </Card>

      {smtp && (
        <Card title="Email" description="Sends invite links, and password resets to people who gave an address.">
          <div className="grid gap-4 sm:grid-cols-[1fr_8rem]">
            <Field label="SMTP server">
              <Input
                value={draft.smtp.host ?? ''}
                placeholder="smtp.example.com"
                onChange={(e) => set((c) => void (c.smtp.host = e.target.value.trim() || null))}
              />
            </Field>
            <Field label="Port">
              <Input
                inputMode="numeric"
                value={draft.smtp.port ?? ''}
                placeholder={String(SMTP_PORTS[draft.smtp.security])}
                onChange={(e) => set((c) => void (c.smtp.port = Number(e.target.value.replace(/\D/g, '')) || null))}
              />
            </Field>
          </div>
          <div className="mt-4 grid gap-4 sm:grid-cols-2">
            <Field label="Encryption">
              <Select
                value={draft.smtp.security}
                options={[
                  { value: 'STARTTLS', label: 'STARTTLS' },
                  { value: 'TLS', label: 'TLS' },
                  { value: 'NONE', label: 'None' },
                ]}
                onChange={(v) => set((c) => void (c.smtp.security = v))}
              />
            </Field>
            <Field label="From">
              <Input
                value={draft.smtp.from ?? ''}
                placeholder="tinystream <me@example.com>"
                onChange={(e) => set((c) => void (c.smtp.from = e.target.value.trim() || null))}
              />
            </Field>
            <Field label="Username">
              <Input
                autoComplete="off"
                value={draft.smtp.username ?? ''}
                onChange={(e) => set((c) => void (c.smtp.username = e.target.value.trim() || null))}
              />
            </Field>
            <Field label="Password">
              <Input
                type="password"
                autoComplete="new-password"
                value={draft.smtp.password ?? ''}
                onChange={(e) => set((c) => void (c.smtp.password = e.target.value || null))}
              />
            </Field>
          </div>
          {data.smtp.host && <TestEmail unsaved={JSON.stringify(draft.smtp) !== JSON.stringify(data.smtp)} />}
        </Card>
      )}

      <Card title="Library scanning">
        <Row label="Watch folders for changes">
          <Toggle label="Watch folders" checked={draft.scan.watch} onChange={(v) => set((c) => void (c.scan.watch = v))} />
        </Row>
        <div className="mt-4 max-w-xs">
          <Field label="Rescan interval" hint="e.g. 6h">
            <Input
              value={draft.scan.interval ?? ''}
              placeholder="off"
              onChange={(e) => set((c) => void (c.scan.interval = e.target.value.trim() || null))}
            />
          </Field>
        </div>
      </Card>

      <Card title="Metadata">
        <div className="grid gap-4 sm:grid-cols-[1fr_10rem]">
          <Field label="TMDB API key" hint="themoviedb.org → Settings → API">
            <Input
              type="password"
              value={draft.metadata.tmdbApiKey ?? ''}
              onChange={(e) => set((c) => void (c.metadata.tmdbApiKey = e.target.value.trim() || null))}
            />
          </Field>
          <Field label="Language">
            <Input value={draft.metadata.language} onChange={(e) => set((c) => void (c.metadata.language = e.target.value))} />
          </Field>
        </div>
      </Card>

      <Card
        title="Transcoding"
        description={
          t.vaapi ? `VA-API: ${t.vaapi}` : t.vaapiError ? `VA-API unavailable: ${t.vaapiError}` : undefined
        }
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="Encoder">
            <Select
              value={draft.transcode.hardware}
              options={[
                { value: 'AUTO', label: 'GPU, falling back to CPU' },
                { value: 'VAAPI', label: 'GPU (VA-API) only' },
                { value: 'SOFTWARE', label: 'CPU only' },
              ]}
              onChange={(v) => set((c) => void (c.transcode.hardware = v))}
            />
          </Field>
          <Field label="GPU device">
            <Input
              value={draft.transcode.vaapiDevice}
              onChange={(e) => set((c) => void (c.transcode.vaapiDevice = e.target.value))}
            />
          </Field>
        </div>
      </Card>

      <Card title="Logs" description={data.paths.log}>
        <div className="max-w-xs">
          <Field label="Level">
            <Select
              value={draft.log.level}
              options={['error', 'warn', 'info', 'debug', 'trace'].map((l) => ({ value: l, label: l }))}
              onChange={(v) => set((c) => void (c.log.level = v))}
            />
          </Field>
        </div>
      </Card>

      <div className="sticky bottom-4 flex items-center justify-end gap-3">
        {error && <p className="text-sm text-danger">{error}</p>}
        {saved && <p className="text-sm text-ink-2">Saved to config.toml</p>}
        <Button variant="plain" disabled={!dirty} onClick={() => setDraft(structuredClone(serverPart(data)))}>
          Discard
        </Button>
        <Button variant="primary" disabled={!dirty || save.isPending} onClick={() => save.mutate()}>
          Save changes
        </Button>
      </div>
    </>
  )
}

function tailnet(t: Settings['tailscaleStatus']): string {
  switch (t.state) {
    case 'OFF':
      return 'Off'
    case 'MISSING':
      return 'Tailscale isn’t running on this machine.'
    case 'UNREACHABLE':
      return 'Can’t reach tailscaled.'
    case 'STOPPED':
      return 'Tailscale isn’t connected.'
    case 'RUNNING':
      return `${t.dnsName ?? t.ips[0] ?? 'Connected'} · ${t.funnel ? 'open to the internet through Funnel' : t.https ? 'HTTPS through Tailscale Serve' : 'plain HTTP'}`
  }
}

function TestEmail({ unsaved }: { unsaved: boolean }) {
  const [to, setTo] = useState('')
  const send = useMutation({
    mutationFn: () => request(SendTestEmail, { to: to.trim() }),
    onSuccess: () => toast({ title: `Sent to ${to.trim()}`, tone: 'ok' }),
    onError: toastError,
  })
  return (
    <div className="mt-4 flex gap-2">
      <div className="flex-1">
        <Input type="email" value={to} onChange={(e) => setTo(e.target.value)} placeholder={unsaved ? 'Save first, then send a test' : 'Where to send a test email'} />
      </div>
      <Button disabled={unsaved || !to.trim() || send.isPending} onClick={() => send.mutate()}>
        Send test
      </Button>
    </div>
  )
}

function Account() {
  const qc = useQueryClient()
  const { data: keys } = useQuery({ queryKey: ['passkeys'], queryFn: async () => (await request(PasskeysQuery)).viewer?.passkeys ?? [] })
  const [name, setName] = useState('')
  const [keyError, setKeyError] = useState<string | null>(null)
  const addKey = useMutation({
    mutationFn: async () => {
      const { challenge, options } = (await request(StartRegistration, { name: name || null })).startPasskeyRegistration
      const credential = await createCredential(options)
      await request(FinishRegistration, { challenge, credential })
    },
    onSuccess: () => {
      setName('')
      setKeyError(null)
      void qc.invalidateQueries({ queryKey: ['passkeys'] })
    },
    onError: (e) => setKeyError((e as Error).name === 'NotAllowedError' ? 'The passkey prompt was closed.' : (e as Error).message),
  })
  const removeKey = useMutation({
    mutationFn: (id: number) => request(DeletePasskey, { id }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['passkeys'] }),
  })

  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const change = useMutation({
    mutationFn: () => request(ChangePassword, { current, new: next }),
    onSuccess: () => {
      setCurrent('')
      setNext('')
    },
  })
  const date = (s: number) => new Date(s * 1000).toLocaleDateString(undefined, { dateStyle: 'medium' })

  return (
    <>
      <Profile />
      <Notifications />
      <AppPasswords />
      <Card title="Passkeys">
        <div className="-mx-2 mb-4 space-y-0.5">
          {keys?.length === 0 && <p className="px-2 text-sm text-ink-3">No passkeys yet.</p>}
          {keys?.map((k) => (
            <div key={k.id} className="flex items-center gap-3 rounded-xl px-2 py-2 hover:bg-hover">
              <KeyRound className="size-4 text-ink-3" />
              <div className="min-w-0 flex-1">
                <p className="text-sm">{k.name}</p>
                <p className="text-xs text-ink-3">
                  Added {date(k.createdAt)}
                  {k.lastUsed && `, last used ${date(k.lastUsed)}`}
                </p>
              </div>
              <IconButton label={`Remove ${k.name}`} onClick={() => removeKey.mutate(k.id)}>
                <Trash2 className="size-4" />
              </IconButton>
            </div>
          ))}
        </div>
        <div className="flex gap-2">
          <div className="flex-1">
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Name, e.g. YubiKey" />
          </div>
          <Button variant="primary" onClick={() => addKey.mutate()} disabled={addKey.isPending}>
            Add passkey
          </Button>
        </div>
        {keyError && <p className="mt-2 text-sm text-danger">{keyError}</p>}
      </Card>

      <Card title="Password">
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="Current password">
            <Input type="password" autoComplete="current-password" value={current} onChange={(e) => setCurrent(e.target.value)} />
          </Field>
          <Field label="New password">
            <Input type="password" autoComplete="new-password" value={next} onChange={(e) => setNext(e.target.value)} />
          </Field>
        </div>
        <div className="mt-4 flex items-center justify-end gap-3">
          {change.isSuccess && <p className="text-sm text-ink-2">Password changed</p>}
          {change.error && <p className="text-sm text-danger">{(change.error as Error).message}</p>}
          <Button variant="primary" disabled={!current || !next || change.isPending} onClick={() => change.mutate()}>
            Change password
          </Button>
        </div>
      </Card>

      <Email />
    </>
  )
}

function Email() {
  const qc = useQueryClient()
  const on = useEmailOn()
  const { data: saved } = useQuery({ queryKey: ['email'], queryFn: async () => (await request(EmailQuery)).viewer?.email ?? null, enabled: on })
  const [draft, setDraft] = useState<string | null>(null)
  const [password, setPassword] = useState('')
  const save = useMutation({
    mutationFn: async (email: string | null) => (await request(SetEmail, { email, password })).setEmail,
    onSuccess: (email) => {
      qc.setQueryData(['email'], email ?? null)
      setDraft(null)
      setPassword('')
    },
  })
  if (!on || saved === undefined) return null
  const value = draft ?? saved ?? ''

  return (
    <Card title="Email" description="Only used to send you a link if you forget your password.">
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Address">
          <Input type="email" autoComplete="email" value={value} placeholder="None" onChange={(e) => setDraft(e.target.value)} />
        </Field>
        <Field label="Your password">
          <Input type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} />
        </Field>
      </div>
      <div className="mt-4 flex items-center justify-end gap-3">
        {save.isSuccess && <p className="text-sm text-ink-2">Saved</p>}
        {save.error && <p className="text-sm text-danger">{(save.error as Error).message}</p>}
        {saved && (
          <Button variant="plain" disabled={!password || save.isPending} onClick={() => save.mutate(null)}>
            Remove
          </Button>
        )}
        <Button
          variant="primary"
          disabled={!value.trim() || value.trim() === saved || !password || save.isPending}
          onClick={() => save.mutate(value.trim())}
        >
          Save
        </Button>
      </div>
    </Card>
  )
}

function Notifications() {
  const [on, setOn] = useState(() => typeof window !== 'undefined' && notifyEnabled())
  const denied = typeof Notification !== 'undefined' && Notification.permission === 'denied'
  if (typeof Notification === 'undefined') return null
  return (
    <Card title="Notifications">
      <Row
        label="System notifications"
        hint={denied ? 'Blocked in browser settings' : undefined}
      >
        <Toggle label="System notifications" checked={on} onChange={async (v) => setOn(await setNotify(v))} />
      </Row>
    </Card>
  )
}
function RawConfig() {
  const { data } = useSettings()
  const [text, setText] = useState<string | null>(null)
  const refresh = useSave()
  const save = useMutation({
    mutationFn: () => request(ReplaceConfig, { text: text ?? '' }),
    onSuccess: () => {
      setText(null)
      refresh()
    },
  })
  if (!data) return null
  const value = text ?? data.raw
  return (
    <Card title="config.toml" description={data.paths.config}>
      <Squircle radius={12} edge className="bg-canvas">
        <textarea
          value={value}
          onChange={(e) => setText(e.target.value)}
          spellCheck={false}
          className="block min-h-[26rem] w-full resize-y bg-transparent p-4 font-mono text-[13px] leading-relaxed text-ink outline-none"
        />
      </Squircle>
      <div className="mt-4 flex items-center justify-end gap-3">
        {save.error && <p className="min-w-0 flex-1 text-sm whitespace-pre-wrap text-danger">{(save.error as Error).message}</p>}
        <Button variant="plain" disabled={text === null} onClick={() => setText(null)}>
          Discard
        </Button>
        <Button variant="primary" disabled={text === null || save.isPending} onClick={() => save.mutate()}>
          Save file
        </Button>
      </div>
    </Card>
  )
}
function Skipped() {
  const { data } = useQuery({
    queryKey: ['skipped'],
    queryFn: async () => (await request(SkippedQuery)).skippedFiles,
  })
  const { data: settings } = useSettings()
  if (!data) return null
  const roots = new Map(settings?.libraries.map((l) => [l.name, l.resolvedPath ?? '']))
  const groups = new Map<string, typeof data>()
  for (const s of data) groups.set(s.reason, [...(groups.get(s.reason) ?? []), s])
  return (
    <Card title="Skipped files">
      {data.length === 0 && <p className="text-sm text-ink-3">Nothing skipped.</p>}
      <div className="space-y-6">
        {[...groups].map(([reason, items]) => (
          <div key={reason}>
            <p className="text-sm font-medium">
              {reason} <span className="font-normal text-ink-3 tabular">{items.length}</span>
            </p>
            <ul className="mt-2 space-y-1">
              {items.map((s) => (
                <li key={s.path} className="text-xs text-ink-3">
                  <Tip label={s.path} className="block truncate">
                    <span className="text-ink-2">{s.library}</span>
                    {s.path.slice((roots.get(s.library) ?? '').length)}
                  </Tip>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </Card>
  )
}
