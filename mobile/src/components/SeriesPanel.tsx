// SPDX-License-Identifier: AGPL-3.0-or-later
// A show being downloaded automatically (web's SeriesPanel.tsx): what's next
// for everyone, and for people who manage shows, the controls and settings.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { airs, countdown, episodeCode, relative } from '@tinystream/shared/downloads'
import { CalendarClock, CircleAlert, EllipsisVertical, RefreshCw, RotateCcw, Search, SlidersHorizontal, Trash2 } from 'lucide-react-native'
import { useEffect, useState } from 'react'
import { ScrollView, Text, View } from 'react-native'
import { Squircle } from '../effects/Squircle'
import { graphql } from '../gql'
import type { Monitor, Numbering, SeedingInput } from '../gql/graphql'
import { useGo } from '../nav'
import { type Item, type Seeding, type Series, useMe, useNow, useSettings } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { MonitorPicker, ReleaseSheet, deleteItems, useDeleteDownloaded, useLookForAgain } from './downloads'
import { ask, toastError } from './Feedback'
import { Menu } from './Menu'
import { Sheet } from './Sheet'
import { Button, Chip, ErrorText, Field, IconButton, Input, Segmented, Select } from './ui'

const TitleSeriesQuery = graphql(`
  query TitleSeries($id: Int!) {
    title(id: $id) {
      series {
        ...SeriesFields
      }
    }
  }
`)

/** What people who don't manage shows see of one: what's next. */
const TitleScheduleQuery = graphql(`
  query TitleSchedule($id: Int!) {
    title(id: $id) {
      series {
        id
        monitor
        status
        next {
          ...SeriesEpisodeFields
        }
      }
    }
  }
`)

const ManageTitle = graphql(`
  mutation ManageTitle($titleId: Int!) {
    manageTitle(titleId: $titleId) {
      id
    }
  }
`)

const UpdateSeries = graphql(`
  mutation UpdateSeries($id: Int!, $patch: SeriesPatch!) {
    updateSeries(id: $id, patch: $patch) {
      ...SeriesFields
    }
  }
`)

const RefreshSchedule = graphql(`
  mutation RefreshSeriesSchedule($id: Int!) {
    refreshSeriesSchedule(id: $id) {
      id
    }
  }
`)

const RemoveSeries = graphql(`
  mutation RemoveSeries($id: Int!) {
    removeSeries(id: $id)
  }
`)

const NamingPreviewQuery = graphql(`
  query NamingPreview($id: Int!, $file: String!) {
    series(id: $id) {
      namingPreview(file: $file) {
        samples
        error
      }
    }
  }
`)

export type SeriesGlimpse = Pick<Series, 'id' | 'monitor' | 'status' | 'next'>

/** A title's managed show: all of it for people who manage shows, what's next for everyone else. */
export function useItemSeries(itemId: number, enabled: boolean, manage: boolean) {
  const api = useApi()
  return useQuery({
    queryKey: ['series', 'item', itemId, manage],
    queryFn: async (): Promise<Series | SeriesGlimpse | null> =>
      manage
        ? ((await api.request(TitleSeriesQuery, { id: itemId })).title?.series ?? null)
        : ((await api.request(TitleScheduleQuery, { id: itemId })).title?.series ?? null),
    enabled,
  })
}

/** The "next episode" line everyone sees under a show's title. */
export function NextEpisode({ series }: { series: Pick<Series, 'next'> }) {
  const now = useNow(1000)
  const { tokens } = useTheme()
  const n = series.next
  if (!n?.airAt) return null
  const soon = n.airAt * 1000 - now < 86400 * 1000
  return (
    <View className="flex-row items-start gap-2">
      <CalendarClock size={16} color={tokens['ink-3']} style={{ marginTop: 2 }} />
      <Text className="font-sans flex-1 text-sm leading-5 text-ink-2">
        <Text className="text-ink">{n.name ? `${episodeCode(n.season, n.episode)} “${n.name}”` : `Episode ${n.episode}`}</Text> airs{' '}
        {airs(n.airAt, now).replace(/^Today/, 'today').replace(/^Tomorrow/, 'tomorrow')}
        <Text className={soon ? 'text-warn' : 'text-ink-3'}> · {countdown(n.airAt, now)}</Text>
      </Text>
    </View>
  )
}

/** Someone who manages shows: downloading this one automatically. */
export function SeriesPanel({ item, series, season }: { item: Item; series: Series | null; season: number | null }) {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const [settings, setSettings] = useState(false)
  const [searching, setSearching] = useState(false)
  const [more, setMore] = useState(false)
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['series'] })
    void qc.invalidateQueries({ queryKey: ['calendar'] })
    void qc.invalidateQueries({ queryKey: ['wanted'] })
  }
  const manage = useMutation({
    mutationFn: async (monitor: Monitor) => {
      const id = series?.id ?? (await api.request(ManageTitle, { titleId: item.id })).manageTitle.id
      return api.request(UpdateSeries, { id, patch: { monitor } })
    },
    onSuccess: refresh,
  })
  const schedule = useMutation({ mutationFn: () => api.request(RefreshSchedule, { id: series!.id }), onSuccess: refresh, onError: toastError })
  const canDelete = !!useMe()?.permissions.downloads
  const deleteDownloaded = useDeleteDownloaded()
  const lookAgain = useLookForAgain()

  if (item.kind !== 'SHOW') return null
  const unmatched = !item.providerId
  const monitor = series?.monitor ?? 'NONE'
  const c = series?.counts

  return (
    <Squircle radius={18} edge className="gap-3 bg-raised p-4">
      <View>
        <Text className="font-sans text-sm font-medium text-ink">Download automatically</Text>
        {unmatched && <Text className="font-sans text-xs text-ink-3">Match this show to AniList or TMDB first.</Text>}
      </View>
      <MonitorPicker value={monitor} onChange={(m) => manage.mutate(m)} />
      {manage.error && <ErrorText>{(manage.error as Error).message}</ErrorText>}
      {series && (
        <>
          {!series.managed && monitor !== 'NONE' && (
            <View className="flex-row items-start gap-2">
              <CircleAlert size={14} color={tokens.warn} style={{ marginTop: 1 }} />
              <Text className="font-sans flex-1 text-xs text-warn">{series.library} isn't managed; can't import.</Text>
            </View>
          )}
          <View className="flex-row flex-wrap gap-x-4 gap-y-1 border-t border-line pt-3">
            {c && c.total > 0 && (
              <Text className="font-sans text-xs text-ink-3">
                <Text className="text-ink-2">{c.have}</Text> of {c.total} in library
              </Text>
            )}
            {c && c.grabbed > 0 && <Text className="font-sans text-xs text-info">{c.grabbed} downloading</Text>}
            {c && c.wanted > 0 && <Text className="font-sans text-xs text-warn">{c.wanted} to find</Text>}
            {c && c.missing > 0 && <Text className="font-sans text-xs text-danger">{c.missing} missing</Text>}
            {c && c.upcoming > 0 && <Text className="font-sans text-xs text-ink-3">{c.upcoming} not aired yet</Text>}
            {c && c.skipped > 0 && <Text className="font-sans text-xs text-ink-3">{c.skipped} skipped</Text>}
            <Text className="font-sans text-xs text-ink-3">{series.effectiveProfile}</Text>
            {series.scheduleAt && <Text className="font-sans text-xs text-ink-3">schedule {relative(series.scheduleAt)}</Text>}
          </View>
          <View className="flex-row items-center gap-2">
            <Button size="sm" onPress={() => setSearching(true)} disabled={season === null} icon={<Search size={14} color={tokens.ink} />}>
              Search
            </Button>
            <Button size="sm" onPress={() => setSettings(true)} icon={<SlidersHorizontal size={14} color={tokens.ink} />}>
              Settings
            </Button>
            <View className="flex-1" />
            <IconButton label="Refresh the schedule" onPress={() => schedule.mutate()} disabled={schedule.isPending}>
              <RefreshCw size={17} color={tokens['ink-2']} />
            </IconButton>
            {(canDelete || (c && c.skipped > 0)) && (
              <IconButton label="More" onPress={() => setMore(true)}>
                <EllipsisVertical size={18} color={tokens['ink-2']} />
              </IconButton>
            )}
          </View>
          {schedule.error && <ErrorText>{(schedule.error as Error).message}</ErrorText>}
          <Menu
            open={more}
            onClose={() => setMore(false)}
            items={[
              !!c &&
                c.skipped > 0 && {
                  label: 'Look for skipped episodes again',
                  icon: (col) => <RotateCcw size={18} color={col} />,
                  onPress: () => lookAgain.mutate({ seriesId: series.id }),
                },
              ...(canDelete ? deleteItems({ seriesId: series.id, show: series.name, season }, deleteDownloaded) : []),
            ]}
          />
          {season !== null && <ReleaseSheet open={searching} seriesId={series.id} season={season} episodes={[]} title={series.name} onClose={() => setSearching(false)} />}
          <Sheet open={settings} onClose={() => setSettings(false)}>
            {settings && <SeriesSettings series={series} onClose={() => setSettings(false)} />}
          </Sheet>
        </>
      )}
    </Squircle>
  )
}

const seedingPresets: { value: string; label: string; rules: SeedingInput | null }[] = [
  { value: 'default', label: 'Default', rules: null },
  { value: 'none', label: "Don't seed", rules: { ratio: 0, then: 'REMOVE' } },
  { value: 'ratio', label: 'Ratio 1.0', rules: { ratio: 1, then: 'REMOVE' } },
  { value: 'week', label: '7 days', rules: { time: '7days', then: 'REMOVE' } },
  { value: 'forever', label: 'Forever', rules: { then: 'PAUSE' } },
]

function presetOf(s: Seeding | null) {
  if (!s) return 'default'
  if (s.ratio === 0) return 'none'
  if (s.ratio === 1 && !s.time && !s.idle) return 'ratio'
  if (!s.ratio && s.time && !s.idle) return 'week'
  if (!s.ratio && !s.time && !s.idle) return 'forever'
  return 'default'
}

const list = (s: string) =>
  s
    .split(',')
    .map((x) => x.trim())
    .filter(Boolean)

function SeriesSettings({ series, onClose }: { series: Series; onClose: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const go = useGo()
  const { tokens } = useTheme()
  const { data: settings } = useSettings()
  const [profile, setProfile] = useState(series.profile ?? '')
  const [sources, setSources] = useState<string[]>(series.sources)
  const [groups, setGroups] = useState(series.groups.join(', '))
  const [aliases, setAliases] = useState(series.aliases.join(', '))
  const [numbering, setNumbering] = useState<Numbering>(series.numbering)
  const [naming, setNaming] = useState(series.naming ?? series.style.file)
  const [seeding, setSeeding] = useState(presetOf(series.seeding ?? null))
  const [preview, setPreview] = useState<{ samples: string[]; error?: string | null } | null>(null)

  useEffect(() => {
    const t = setTimeout(async () => {
      try {
        setPreview((await api.request(NamingPreviewQuery, { id: series.id, file: naming })).series?.namingPreview ?? null)
      } catch {}
    }, 250)
    return () => clearTimeout(t)
  }, [api, naming, series.id])

  const save = useMutation({
    mutationFn: () =>
      api.request(UpdateSeries, {
        id: series.id,
        patch: {
          profile: profile || null,
          sources,
          groups: list(groups),
          aliases: list(aliases),
          numbering,
          naming: naming.trim() === series.style.file && !series.naming ? null : naming.trim() || null,
          seeding: seedingPresets.find((p) => p.value === seeding)?.rules ?? null,
        },
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['series'] })
      onClose()
    },
  })
  const remove = useMutation({
    mutationFn: () => api.request(RemoveSeries, { id: series.id }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['series'] })
      onClose()
    },
  })
  const allSources = settings?.sources ?? []
  const inferred = series.style.agreement != null

  return (
    <ScrollView style={{ maxHeight: 620 }} keyboardShouldPersistTaps="handled">
      <View className="gap-4 pb-2">
        <Text className="font-sans text-lg font-semibold text-ink">{series.name}</Text>
        <Field label="Quality profile">
          <Select
            title="Quality profile"
            value={profile}
            options={[{ value: '', label: 'Library default' }, ...(settings?.profiles ?? []).map((p) => ({ value: p.name, label: p.name }))]}
            onChange={setProfile}
          />
        </Field>
        <Field label="Preferred groups">
          <Input value={groups} onChangeText={setGroups} placeholder="SubsPlease, Erai-raws" autoCapitalize="none" />
        </Field>
        <Field label="Sources">
          <View className="flex-row flex-wrap gap-1.5">
            <Chip on={sources.length === 0} onPress={() => setSources([])}>
              All sources
            </Chip>
            {allSources.map((s) => {
              const on = sources.includes(s.name)
              return (
                <Chip key={s.name} on={on} onPress={() => setSources(on ? sources.filter((x) => x !== s.name) : [...sources, s.name])}>
                  {on ? `${sources.indexOf(s.name) + 1}  ${s.name}` : s.name}
                </Chip>
              )
            })}
            {allSources.length === 0 && (
              <Button size="sm" variant="plain" onPress={() => (onClose(), go('settings/sources', '(profile)'))}>
                Add a source first
              </Button>
            )}
          </View>
        </Field>
        <Field
          label="Also known as"
          hint={
            series.knownAs.length > 0
              ? `Already searched: ${series.knownAs.slice(0, 4).join(', ')}${series.knownAs.length > 4 ? '…' : ''}`
              : 'Titles releases might use, comma-separated.'
          }
        >
          <Input value={aliases} onChangeText={setAliases} placeholder="Other titles, comma-separated" />
        </Field>
        <Field label="Episode numbers in releases">
          <Segmented
            size="sm"
            value={numbering}
            onChange={setNumbering}
            options={[
              { value: 'AUTO', label: 'Work it out' },
              { value: 'SEASONAL', label: 'Per season' },
              { value: 'ABSOLUTE', label: 'From the first episode' },
            ]}
          />
        </Field>
        <Field
          label="File names"
          hint={
            inferred && !series.naming
              ? `Learned from ${series.style.samples} files; ${Math.round((series.style.agreement ?? 0) * 100)}% follow it.`
              : '{show} {year} {season:00} {episode:00} {title} {group} {quality} {codec} {original}'
          }
        >
          <Input value={naming} onChangeText={setNaming} autoCapitalize="none" autoCorrect={false} style={{ fontFamily: 'monospace', fontSize: 13 }} />
        </Field>
        {preview && (
          <Squircle radius={10} className="bg-canvas px-3 py-2">
            {preview.error ? (
              <Text className="font-sans text-xs text-danger">{preview.error}</Text>
            ) : (
              preview.samples.map((s) => (
                <Text key={s} className="text-xs text-ink-2" style={{ fontFamily: 'monospace' }} numberOfLines={1}>
                  {s}
                </Text>
              ))
            )}
          </Squircle>
        )}
        <Field label="Seeding">
          <Segmented size="sm" value={seeding} onChange={setSeeding} options={seedingPresets.map((p) => ({ value: p.value, label: p.label }))} />
        </Field>
        {save.error && <ErrorText>{(save.error as Error).message}</ErrorText>}
        <Button variant="primary" size="lg" disabled={save.isPending || !!preview?.error} onPress={() => save.mutate()}>
          Save
        </Button>
        <Button
          variant="danger"
          size="lg"
          icon={<Trash2 size={17} color={tokens.danger} />}
          onPress={async () =>
            (await ask({ title: `Stop managing ${series.name}?`, body: 'Its files stay exactly where they are.', confirm: 'Stop managing', danger: true })) && remove.mutate()
          }
        >
          Stop managing
        </Button>
      </View>
    </ScrollView>
  )
}
