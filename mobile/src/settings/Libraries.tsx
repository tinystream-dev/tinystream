// SPDX-License-Identifier: AGPL-3.0-or-later
// The server's libraries (web's Libraries tab): where they are, what they
// hold, and adding or changing one, with a browser of the server's folders.

import { useMutation, useQuery } from '@tanstack/react-query'
import { BookOpen, CornerLeftUp, Folder, Music, Plus, RefreshCw, Trash2 } from 'lucide-react-native'
import { useEffect, useState } from 'react'
import { Pressable, ScrollView, Text, View } from 'react-native'
import { haptic } from '../../modules/haptics'
import { ask, toast, toastError } from '../components/Feedback'
import { Sheet } from '../components/Sheet'
import { ListSkeleton } from '../components/Skeleton'
import { Badge, Button, Divider, ErrorText, Field, Group, IconButton, Input, ListRow, Segmented, Select, Toggle } from '../components/ui'
import { Squircle } from '../effects/Squircle'
import { graphql } from '../gql'
import type { LibraryKind, Provider } from '../gql/graphql'
import { type ConfigLibrary, useSettings } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { useSaved } from './kit'

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

const providers: { value: '' | Provider; label: string }[] = [
  { value: '', label: 'None (folder names)' },
  { value: 'ANILIST', label: 'AniList (anime)' },
  { value: 'TMDB', label: 'TMDB (shows and movies)' },
]

export function Libraries() {
  const api = useApi()
  const { tokens } = useTheme()
  const { data } = useSettings()
  const [editing, setEditing] = useState<ConfigLibrary | 'new' | null>(null)
  const scan = useMutation({
    mutationFn: () => api.request(Scan, { library: null }),
    onSuccess: () => toast({ title: 'Rescanning every library', tone: 'ok' }),
    onError: toastError,
  })
  if (!data) return <ListSkeleton rows={3} height={64} />
  return (
    <>
      <View className="flex-row gap-2">
        {data.libraries.length > 0 && (
          <Button className="flex-1" onPress={() => scan.mutate()} icon={<RefreshCw size={15} color={tokens.ink} />}>
            Rescan all
          </Button>
        )}
        <Button className="flex-1" variant="primary" onPress={() => setEditing('new')} icon={<Plus size={17} color={tokens['on-accent']} />}>
          Add
        </Button>
      </View>
      {data.libraries.length === 0 ? (
        <Text className="font-sans py-6 text-center text-sm text-ink-3">No libraries yet.</Text>
      ) : (
        <Group>
          {data.libraries.map((l, i) => (
            <View key={l.name}>
              {i > 0 && <Divider inset={52} />}
              <ListRow
                icon={
                  l.kind === 'MUSIC' ? (
                    <Music size={20} color={tokens['ink-3']} />
                  ) : l.kind === 'BOOKS' ? (
                    <BookOpen size={20} color={tokens['ink-3']} />
                  ) : (
                    <Folder size={20} color={tokens['ink-3']} />
                  )
                }
                label={
                  <View className="flex-row items-center gap-2">
                    <Text className="font-sans text-[15px] text-ink">{l.name}</Text>
                    {l.managed && <Badge>Managed</Badge>}
                  </View>
                }
                hint={
                  <View>
                    <Text className={`font-sans text-xs ${l.exists ? 'text-ink-3' : 'text-danger'}`} numberOfLines={2}>
                      {l.error ?? (l.exists ? l.resolvedPath : `${l.resolvedPath} can't be found`)}
                    </Text>
                    <Text className="font-sans text-xs text-ink-3">
                      {l.titleCount} {l.kind === 'MUSIC' ? 'albums' : l.kind === 'BOOKS' ? 'books' : 'titles'}
                      {l.skippedCount > 0 && `, ${l.skippedCount} skipped`}
                    </Text>
                  </View>
                }
                onPress={() => setEditing(l)}
              />
            </View>
          ))}
        </Group>
      )}
      <Sheet open={!!editing} onClose={() => setEditing(null)}>
        {editing && <LibraryForm key={editing === 'new' ? 'new' : editing.name} library={editing === 'new' ? null : editing} onDone={() => setEditing(null)} />}
      </Sheet>
    </>
  )
}

function LibraryForm({ library, onDone }: { library: ConfigLibrary | null; onDone: () => void }) {
  const api = useApi()
  const { tokens } = useTheme()
  const saved = useSaved()
  const { data: settings } = useSettings()
  const [name, setName] = useState(library?.name ?? '')
  const [path, setPath] = useState(library?.path ?? '')
  const [kind, setKind] = useState<LibraryKind>(library?.kind ?? 'VIDEO')
  const [provider, setProvider] = useState<'' | Provider>(library?.metadataProvider ?? '')
  const [managed, setManaged] = useState(library?.managed ?? false)
  const [profile, setProfile] = useState(library?.profile ?? '')
  const [downloadPath, setDownloadPath] = useState(library?.downloadPath ?? '')
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
  const done = () => {
    saved()
    onDone()
  }
  const save = useMutation({
    mutationFn: async () => void (library ? await api.request(UpdateLibrary, { name: library.name, input: body }) : await api.request(AddLibrary, { input: body })),
    onSuccess: () => (toast({ title: library ? `Saved ${name}` : `Added ${name}`, tone: 'ok' }), done()),
    onError: (e) => setError((e as Error).message),
  })
  const remove = useMutation({ mutationFn: () => api.request(RemoveLibrary, { name: library!.name }), onSuccess: done, onError: toastError })

  return (
    <ScrollView style={{ maxHeight: 640 }} keyboardShouldPersistTaps="handled">
      <View className="gap-4 pb-2">
        <Text className="font-sans text-lg font-semibold text-ink">{library ? `Edit ${library.name}` : 'Add a library'}</Text>
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
          <Input value={name} onChangeText={setName} placeholder={music ? 'Music' : video ? 'Anime' : 'Novels'} />
        </Field>
        <Field label="Folder">
          <View className="flex-row gap-2">
            <View className="flex-1">
              <Input value={path} onChangeText={setPath} placeholder="~/Videos/Anime" autoCapitalize="none" autoCorrect={false} />
            </View>
            <Button className="self-center" onPress={() => setBrowsing((b) => !b)}>
              Browse
            </Button>
          </View>
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
          <Text className="font-sans text-xs leading-4 text-ink-3">Albums and artists come from the files' tags, however the folders are laid out.</Text>
        ) : !video ? (
          <Text className="font-sans text-xs leading-4 text-ink-3">
            EPUB, FB2 and plain text books, in any folders. What a book doesn't say about itself is looked up on Apple Books and Open Library.
          </Text>
        ) : (
          <Field label="Metadata provider">
            <Select title="Metadata provider" value={provider} options={providers} onChange={setProvider} />
          </Field>
        )}
        {settings?.downloads && video && (
          <>
            <View className="flex-row items-center gap-3">
              <View className="flex-1">
                <Text className="font-sans text-[15px] text-ink">Managed</Text>
                <Text className="font-sans text-xs text-ink-3">Off = read-only</Text>
              </View>
              <Toggle label="Managed" value={managed} onChange={setManaged} />
            </View>
            {managed && (
              <>
                <Field label="Quality profile">
                  <Select
                    title="Quality profile"
                    value={profile}
                    options={[
                      { value: '', label: settings.profiles[0]?.name ? `First profile (${settings.profiles[0].name})` : 'Built-in' },
                      ...settings.profiles.map((p) => ({ value: p.name, label: p.name })),
                    ]}
                    onChange={setProfile}
                  />
                </Field>
                <Field label="Download folder">
                  <Input value={downloadPath} onChangeText={setDownloadPath} placeholder="Global download folder" autoCapitalize="none" />
                </Field>
              </>
            )}
          </>
        )}
        {error && <ErrorText>{error}</ErrorText>}
        <Button variant="primary" size="lg" disabled={!name.trim() || !path.trim() || save.isPending} onPress={() => save.mutate()}>
          {library ? 'Save' : 'Add library'}
        </Button>
        {library && (
          <Button
            variant="danger"
            size="lg"
            icon={<Trash2 size={17} color={tokens.danger} />}
            onPress={async () => (await ask({ title: `Remove ${library.name}?`, body: 'Your files stay where they are.', confirm: 'Remove', danger: true })) && remove.mutate()}
          >
            Remove
          </Button>
        )}
      </View>
    </ScrollView>
  )
}

/** The server's folders from `start`, to pick one by tapping it. */
function FolderPicker({ start, onPick }: { start: string; onPick: (p: string) => void }) {
  const api = useApi()
  const { tokens } = useTheme()
  const path = start.trim() || '~'
  const [at, setAt] = useState(path)
  useEffect(() => {
    const timer = setTimeout(() => setAt(path), 200)
    return () => clearTimeout(timer)
  }, [path])
  const { data, error, isFetching } = useQuery({
    queryKey: ['browse', at],
    queryFn: async () => (await api.request(FoldersQuery, { path: at || null })).folders,
    retry: false,
  })
  const loading = path !== at || isFetching
  const pretty = (p: string) => (data?.home && p.startsWith(data.home) ? `~${p.slice(data.home.length)}` : p)
  const pick = (p: string) => {
    haptic('tick')
    onPick(pretty(p))
  }
  return (
    <Squircle radius={12} edge className="bg-canvas">
      <View className="flex-row items-center gap-1 border-b border-line px-2 py-1">
        <IconButton label="Up a folder" size={34} disabled={loading || !!error || !data?.parent} onPress={() => data?.parent && pick(data.parent)}>
          <CornerLeftUp size={16} color={tokens['ink-2']} />
        </IconButton>
        <Text className="font-sans flex-1 text-xs text-ink-2" numberOfLines={1}>
          {loading ? path : data ? pretty(data.path) : path}
        </Text>
      </View>
      <ScrollView style={{ maxHeight: 220 }} nestedScrollEnabled>
        {loading && <Text className="font-sans p-3 text-xs text-ink-3">Loading folders…</Text>}
        {!loading && error && <Text className="font-sans p-3 text-xs text-danger">{(error as Error).message}</Text>}
        {!loading && !error && data?.folders.length === 0 && <Text className="font-sans p-3 text-xs text-ink-3">No folders in here.</Text>}
        {!loading &&
          !error &&
          data?.folders.map((e) => (
            <Pressable key={e.path} onPress={() => pick(e.path)}>
              {({ pressed }) => (
                <View className={`flex-row items-center gap-2.5 px-3 py-2.5 ${pressed ? 'bg-press' : ''}`}>
                  <Folder size={15} color={tokens['ink-3']} />
                  <Text className="font-sans flex-1 text-[13px] text-ink-2" numberOfLines={1}>
                    {e.name}
                  </Text>
                </View>
              )}
            </Pressable>
          ))}
      </ScrollView>
    </Squircle>
  )
}
