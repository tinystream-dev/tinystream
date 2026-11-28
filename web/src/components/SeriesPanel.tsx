// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { CalendarClock, CircleAlert, Ellipsis, RefreshCw, RotateCcw, Search, SlidersHorizontal, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { graphql } from '../gql'
import type { SeedingInput } from '../gql/graphql'
import { type Item, type Monitor, type Seeding, type Series, request, settingsQuery } from '../lib/api'
import { airs, countdown, episodeCode, monitorLabels, relative, useNow } from '../lib/downloads'
import { useMe } from '../lib/hooks'
import { DeleteItems, ReleaseDialog, useDeleteDownloaded, useLookForAgain } from './downloads'
import { ask, toastError } from './feedback'
import { Squircle } from './Squircle'
import { Button, Dialog, Field, IconButton, Input, MenuItem, Panel, Popover, Segmented, Select } from './ui'

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
  return useQuery({
    queryKey: ['series', 'item', itemId, manage],
    queryFn: async (): Promise<Series | SeriesGlimpse | null> =>
      manage
        ? ((await request(TitleSeriesQuery, { id: itemId })).title?.series ?? null)
        : ((await request(TitleScheduleQuery, { id: itemId })).title?.series ?? null),
    enabled,
  })
}

/** The "next episode" line everyone sees under a show's title. */
export function NextEpisode({ series }: { series: Pick<Series, 'next' | 'status' | 'monitor'> }) {
  const now = useNow(1000)
  const n = series.next
  if (!n?.airAt) return null
  const soon = n.airAt * 1000 - now < 86400 * 1000
  return (
    <p className="flex items-start gap-2 text-sm text-ink-2">
      <CalendarClock className="mt-0.5 size-4 shrink-0 text-ink-3" />
      <span>
        <span className="text-ink">{n.name ? `${episodeCode(n.season, n.episode)} “${n.name}”` : `Episode ${n.episode}`}</span> airs{' '}
        {airs(n.airAt, now).replace(/^Today/, 'today').replace(/^Tomorrow/, 'tomorrow')}
      </span>
      <span className={`tabular ${soon ? 'text-warn' : 'text-ink-3'}`}>· {countdown(n.airAt, now)}</span>
    </p>
  )
}

/** An admin's controls for downloading a show automatically, as a card beside the episodes. */
export function SeriesPanel({ item, series, season }: { item: Item; series: Series | null; season: number | null }) {
  const qc = useQueryClient()
  const [settings, setSettings] = useState(false)
  const [searching, setSearching] = useState(false)
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['series'] })
    void qc.invalidateQueries({ queryKey: ['calendar'] })
    void qc.invalidateQueries({ queryKey: ['wanted'] })
  }
  const manage = useMutation({
    mutationFn: async (monitor: Monitor) => {
      const id = series?.id ?? (await request(ManageTitle, { titleId: item.id })).manageTitle.id
      return request(UpdateSeries, { id, patch: { monitor } })
    },
    onSuccess: refresh,
  })
  const schedule = useMutation({ mutationFn: () => request(RefreshSchedule, { id: series!.id }), onSuccess: refresh, onError: toastError })
  const canDelete = !!useMe()?.permissions.downloads
  const deleteDownloaded = useDeleteDownloaded()
  const lookAgain = useLookForAgain()

  if (item.kind !== 'SHOW') return null
  const unmatched = !item.providerId
  const monitor = series?.monitor ?? 'NONE'
  const c = series?.counts
  const status = !c || c.total === 0
    ? null
    : c.grabbed > 0
      ? { tone: 'bg-info', text: `${c.grabbed} downloading` }
      : c.missing > 0
        ? { tone: 'bg-danger', text: `${c.missing} missing` }
        : c.wanted > 0
          ? { tone: 'bg-warn', text: `${c.wanted} to find` }
          : c.have >= c.total
            ? { tone: 'bg-ok', text: `All ${c.total} episodes are in the library` }
            : { tone: 'bg-ink-3', text: `${c.have} of ${c.total} in the library` }
  const more = c
    ? [
        (c.grabbed > 0 || c.missing > 0 || c.wanted > 0) && c.have < c.total && `${c.have} of ${c.total} in the library`,
        c.grabbed > 0 && c.wanted > 0 && `${c.wanted} to find`,
        c.upcoming > 0 && `${c.upcoming} not aired yet`,
        c.skipped > 0 && `${c.skipped} skipped`,
      ].filter(Boolean)
    : []

  return (
    <Squircle radius={16} edge className="bg-raised p-4">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm font-medium">Downloads</p>
        <div className="w-40">
          <Select
            value={monitor}
            options={(['NONE', 'FUTURE', 'MISSING'] as const).map((m) => ({ value: m, label: monitorLabels[m].label }))}
            onChange={(m) => manage.mutate(m)}
          />
        </div>
      </div>
      {unmatched && <p className="mt-3 text-xs text-ink-3">Match this show to AniList or TMDB first.</p>}
      {manage.error && <p className="mt-3 text-xs text-danger">{(manage.error as Error).message}</p>}

      {series && (
        <>
          {!series.managed && monitor !== 'NONE' && (
            <p className="mt-3 flex items-start gap-2 text-xs text-warn">
              <CircleAlert className="mt-px size-3.5 shrink-0" />
              {series.library} isn't managed; can't import.
            </p>
          )}
          {status && (
            <p className="mt-3 flex items-center gap-2.5 text-[13px] text-ink-2 tabular">
              <span className={`size-1.5 shrink-0 rounded-full ${status.tone}`} />
              {status.text}
            </p>
          )}
          {more.length > 0 && <p className="mt-1 pl-4 text-xs text-ink-3 tabular">{more.join(', ')}</p>}
          <div className="mt-3 flex items-center gap-1 border-t border-line pt-2.5 text-[13px] text-ink-3">
            <span className="min-w-0 flex-1 truncate">{series.effectiveProfile}</span>
            <IconButton label="Search for releases" onClick={() => setSearching(true)} disabled={season === null}>
              <Search className="size-4" />
            </IconButton>
            <IconButton label="Settings" onClick={() => setSettings(true)}>
              <SlidersHorizontal className="size-4" />
            </IconButton>
            <Popover
              portal
              trigger={({ toggle }) => (
                <IconButton label="More" onClick={toggle}>
                  {schedule.isPending ? <RefreshCw className="size-4 animate-spin" /> : <Ellipsis className="size-4" />}
                </IconButton>
              )}
            >
              {(close) => (
                <Panel className="w-64 p-1.5">
                  <MenuItem onClick={() => (close(), schedule.mutate())} hint={series.scheduleAt ? relative(series.scheduleAt) : undefined}>
                    <span className="flex items-center gap-2">
                      <RefreshCw className="size-3.5 shrink-0" /> Refresh the schedule
                    </span>
                  </MenuItem>
                  {c && c.skipped > 0 && (
                    <MenuItem onClick={() => (close(), lookAgain.mutate({ seriesId: series.id }))}>
                      <span className="flex items-center gap-2">
                        <RotateCcw className="size-3.5 shrink-0" /> Look for skipped episodes again
                      </span>
                    </MenuItem>
                  )}
                  {canDelete && <div className="my-1 h-px bg-line" />}
                  {canDelete && (
                    <DeleteItems
                      target={{ seriesId: series.id, show: series.name, season }}
                      onDelete={deleteDownloaded}
                      close={close}
                    />
                  )}
                </Panel>
              )}
            </Popover>
          </div>
          {schedule.error && <p className="mt-2 text-xs text-danger">{(schedule.error as Error).message}</p>}
        </>
      )}
      {searching && series && season !== null && (
        <ReleaseDialog seriesId={series.id} season={season} episodes={[]} title={series.name} onClose={() => setSearching(false)} />
      )}
      {settings && series && <SeriesSettings series={series} onClose={() => setSettings(false)} />}
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
  const qc = useQueryClient()
  const { data: settings } = useQuery(settingsQuery)
  const [profile, setProfile] = useState(series.profile ?? '')
  const [sources, setSources] = useState<string[]>(series.sources)
  const [groups, setGroups] = useState(series.groups.join(', '))
  const [aliases, setAliases] = useState(series.aliases.join(', '))
  const [numbering, setNumbering] = useState(series.numbering)
  const [naming, setNaming] = useState(series.naming ?? series.style.file)
  const [seeding, setSeeding] = useState(presetOf(series.seeding))
  const [preview, setPreview] = useState<{ samples: string[]; error: string | null } | null>(null)

  useEffect(() => {
    const t = setTimeout(async () => {
      try {
        setPreview((await request(NamingPreviewQuery, { id: series.id, file: naming })).series?.namingPreview ?? null)
      } catch {}
    }, 250)
    return () => clearTimeout(t)
  }, [naming, series.id])

  const save = useMutation({
    mutationFn: () =>
      request(UpdateSeries, {
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
    mutationFn: () => request(RemoveSeries, { id: series.id }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['series'] })
      onClose()
    },
  })
  const allSources = settings?.sources ?? []
  const inferred = series.style.agreement !== null

  return (
    <Dialog onClose={onClose} width="max-w-2xl">
      <p className="text-[15px] font-medium">{series.name}</p>

      <div className="mt-6 space-y-5">
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="Quality profile">
            <Select
              value={profile}
              options={[
                { value: '', label: 'Library default' },
                ...(settings?.profiles ?? []).map((p) => ({ value: p.name, label: p.name })),
              ]}
              onChange={setProfile}
            />
          </Field>
          <Field label="Preferred groups">
            <Input value={groups} onChange={(e) => setGroups(e.target.value)} placeholder="SubsPlease, Erai-raws" />
          </Field>
        </div>

        <div>
          <p className="mb-1.5 text-[13px] text-ink-2">Sources</p>
          <div className="flex flex-wrap gap-1.5">
            <Chip on={sources.length === 0} onClick={() => setSources([])}>
              All sources
            </Chip>
            {allSources.map((s) => {
              const on = sources.includes(s.name)
              return (
                <Chip key={s.name} on={on} onClick={() => setSources(on ? sources.filter((x) => x !== s.name) : [...sources, s.name])}>
                  {sources.indexOf(s.name) >= 0 && <span className="opacity-60">{sources.indexOf(s.name) + 1}</span>}
                  {s.name}
                </Chip>
              )
            })}
            {allSources.length === 0 && (
              <Link to="/settings" search={{ tab: 'sources' }} className="text-xs text-ink-3 underline underline-offset-4">
                Add a source first
              </Link>
            )}
          </div>
        </div>

        <Field
          label="Also known as"
          hint={
            series.knownAs.length > 0
              ? `Already searched: ${series.knownAs.slice(0, 4).join(', ')}${series.knownAs.length > 4 ? '…' : ''}`
              : 'Titles releases might use, comma-separated.'
          }
        >
          <Input value={aliases} onChange={(e) => setAliases(e.target.value)} placeholder="Other titles, comma-separated" />
        </Field>

        <div>
          <p className="mb-1.5 text-[13px] text-ink-2">Episode numbers in releases</p>
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
        </div>

        <Field
          label="File names"
          hint={
            inferred && !series.naming
              ? `Learned from ${series.style.samples} files; ${Math.round((series.style.agreement ?? 0) * 100)}% follow it.`
              : '{show} {year} {season:00} {episode:00} {title} {group} {quality} {codec} {original}'
          }
        >
          <Input value={naming} onChange={(e) => setNaming(e.target.value)} className="font-mono text-[13px]" />
        </Field>
        {preview && (
          <Squircle radius={10} className="-mt-2 bg-canvas px-3 py-2">
            {preview.error ? (
              <p className="text-xs text-danger">{preview.error}</p>
            ) : (
              preview.samples.map((s) => (
                <p key={s} className="truncate font-mono text-xs text-ink-2">
                  {s}
                </p>
              ))
            )}
          </Squircle>
        )}

        <div>
          <p className="mb-1.5 text-[13px] text-ink-2">Seeding</p>
          <Segmented size="sm" value={seeding} onChange={setSeeding} options={seedingPresets.map((p) => ({ value: p.value, label: p.label }))} />
        </div>
        {save.error && <p className="text-sm text-danger">{(save.error as Error).message}</p>}
      </div>

      <div className="mt-7 flex items-center gap-2">
        <Button
          variant="danger"
          onClick={async () =>
            (await ask({ title: `Stop managing ${series.name}?`, body: 'Its files stay exactly where they are.', confirm: 'Stop managing', danger: true })) &&
            remove.mutate()
          }
        >
          <Trash2 className="size-4" /> Stop managing
        </Button>
        <div className="flex-1" />
        <Button variant="plain" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="primary" disabled={save.isPending || !!preview?.error} onClick={() => save.mutate()}>
          Save
        </Button>
      </div>
    </Dialog>
  )
}

function Chip({ on, onClick, children }: { on: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <Squircle
      as="button"
      radius={8}
      aria-pressed={on}
      onClick={onClick}
      className={`flex h-7 items-center gap-1.5 px-2.5 text-xs transition-colors ${on ? 'bg-ink text-canvas' : 'bg-panel text-ink-2 hover:text-ink'}`}
    >
      {children}
    </Squircle>
  )
}
