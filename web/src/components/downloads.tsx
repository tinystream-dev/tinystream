// SPDX-License-Identifier: AGPL-3.0-or-later

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowDownToLine, Check, CircleAlert, ExternalLink, Layers, RefreshCw, Search, Sprout, Trash2, Users } from 'lucide-react'
import { useState } from 'react'
import { graphql } from '../gql'
import { type EpisodeState, type Monitor, type ReleaseCandidate, request } from '../lib/api'
import { bytes, episodeCode, monitorLabels, relative, stateLabels } from '../lib/downloads'
import { ask, toast, toastError } from './feedback'
import { Squircle } from './Squircle'
import { Badge, Button, Dialog, IconButton, Input, MenuItem, Segmented, Spinner, Tip } from './ui'

const stateTones: Record<EpisodeState, 'quiet' | 'ok' | 'live' | 'warn' | 'danger'> = {
  IDLE: 'quiet',
  WANTED: 'warn',
  GRABBED: 'live',
  MISSING: 'danger',
  DONE: 'ok',
  SKIPPED: 'quiet',
}

const ReleasesQuery = graphql(`
  query Releases($seriesId: Int!, $season: Int!, $episodes: [Int!]!, $query: String) {
    series(id: $seriesId) {
      releases(season: $season, episodes: $episodes, query: $query) {
        ...ReleaseCandidateFields
      }
    }
  }
`)

const GrabRelease = graphql(`
  mutation GrabRelease($release: ReleaseInput!, $seriesId: Int, $episodes: [EpisodeNumberInput!]!) {
    grabRelease(release: $release, seriesId: $seriesId, episodes: $episodes) {
      id
    }
  }
`)

const DeleteDownloaded = graphql(`
  mutation DeleteDownloaded($seriesId: Int!, $season: Int) {
    deleteDownloaded(seriesId: $seriesId, season: $season) {
      undone
      problems
    }
  }
`)

const LookForAgain = graphql(`
  mutation LookForAgain($seriesId: Int!, $season: Int, $episode: Int) {
    lookForAgain(seriesId: $seriesId, season: $season, episode: $episode)
  }
`)

export type DeleteTarget = { seriesId: number; show: string; season?: number | null }

const seasonName = (n: number) => (n === 0 ? 'specials' : `season ${n}`)

/** Deletes what tinystream downloaded of a show, or one season of it, once it's been asked. */
export function useDeleteDownloaded() {
  const qc = useQueryClient()
  const remove = useMutation({
    mutationFn: async (t: DeleteTarget) => (await request(DeleteDownloaded, { seriesId: t.seriesId, season: t.season ?? null })).deleteDownloaded,
    onSuccess: (r, t) =>
      r.problems.length > 0
        ? toast({ title: `Couldn't delete all of ${t.show}`, body: r.problems.join('\n'), tone: 'danger', duration: 7000 })
        : toast({ title: t.season == null ? `Deleted ${t.show}` : `Deleted ${seasonName(t.season)} of ${t.show}`, tone: 'ok' }),
    onError: toastError,
    onSettled: () => void qc.invalidateQueries(),
  })
  return async (t: DeleteTarget) => {
    const ok = await ask({
      title: t.season == null ? `Delete ${t.show}?` : `Delete ${seasonName(t.season)} of ${t.show}?`,
      body:
        t.season == null
          ? 'Every episode tinystream downloaded of it is deleted from your downloads, finished or not, and from your library. Files you added yourself stay. Its episodes are skipped and it stops downloading automatically.'
          : "Every episode tinystream downloaded of it is deleted from your downloads, finished or not, and from your library. Files you added yourself stay. Its episodes are skipped, so they won't be downloaded again.",
      confirm: 'Delete',
      danger: true,
    })
    if (ok) remove.mutate(t)
  }
}

/** Menu items deleting a season (when there's one) or the whole show. */
export function DeleteItems({ target, onDelete, close }: { target: DeleteTarget; onDelete: (t: DeleteTarget) => void; close: () => void }) {
  return (
    <>
      {target.season != null && (
        <MenuItem onClick={() => (close(), onDelete(target))}>
          <span className="flex items-center gap-2 text-danger">
            <Trash2 className="size-3.5 shrink-0" /> Delete {seasonName(target.season)}
          </span>
        </MenuItem>
      )}
      <MenuItem onClick={() => (close(), onDelete({ ...target, season: null }))}>
        <span className="flex items-center gap-2 text-danger">
          <Trash2 className="size-3.5 shrink-0" /> Delete show
        </span>
      </MenuItem>
    </>
  )
}

/** Puts skipped episodes of a show back, to be downloaded if it's monitored. */
export function useLookForAgain() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (v: { seriesId: number; season?: number; episode?: number }) =>
      request(LookForAgain, { seriesId: v.seriesId, season: v.season ?? null, episode: v.episode ?? null }),
    onError: toastError,
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: ['series'] })
      void qc.invalidateQueries({ queryKey: ['wanted'] })
    },
  })
}

export function StateBadge({ state }: { state: EpisodeState }) {
  return (
    <Badge tone={stateTones[state]}>
      {state === 'WANTED' && <span className="size-1.5 animate-[pulse-dot_1.6s_ease-in-out_infinite] rounded-full bg-current" />}
      {state === 'DONE' && <Check className="size-3" />}
      {stateLabels[state]}
    </Badge>
  )
}

export function MonitorPicker({ value, onChange, size }: { value: Monitor; onChange: (m: Monitor) => void; size?: 'sm' | 'md' }) {
  return (
    <Segmented
      size={size}
      value={value}
      onChange={onChange}
      options={(['NONE', 'FUTURE', 'MISSING'] as const).map((m) => ({
        value: m,
        label: monitorLabels[m].label,
      }))}
    />
  )
}

/** Searches a show's sources and lets an admin pick the release to download. */
export function ReleaseDialog({
  seriesId,
  season,
  episodes,
  title,
  onClose,
}: {
  seriesId: number
  season: number
  episodes: number[]
  title: string
  onClose: () => void
}) {
  const qc = useQueryClient()
  const [showRejected, setShowRejected] = useState(false)
  // Empty means let the server pick the words.
  const [draft, setDraft] = useState('')
  const [query, setQuery] = useState('')
  const queryKey = ['releases', seriesId, season, episodes, query]
  const { data, isFetching, error } = useQuery({
    queryKey,
    queryFn: async () => (await request(ReleasesQuery, { seriesId, season, episodes, query: query || null })).series?.releases ?? [],
    staleTime: 0,
    gcTime: 0,
  })
  const [grabbed, setGrabbed] = useState<Set<string>>(new Set())
  const [manual, setManual] = useState<ReleaseCandidate | null>(null)
  const [manualSeason, setManualSeason] = useState(String(season))
  const [manualEpisode, setManualEpisode] = useState(episodes.length === 1 ? String(episodes[0]) : '')
  const grab = useMutation({
    mutationFn: (c: ReleaseCandidate) =>
      request(GrabRelease, { release: c.release, seriesId, episodes: c.episodes }),
    onSuccess: (_, c) => {
      setGrabbed((g) => new Set(g).add(c.release.link))
      setManual(null)
      void qc.invalidateQueries({ queryKey: ['downloads'] })
      void qc.invalidateQueries({ queryKey: ['series'] })
    },
  })
  const download = async (c: ReleaseCandidate) => {
    const ok =
      c.verdict.warnings.length === 0 ||
      (await ask({
        title: `Download ${bytes(c.release.size)}?`,
        body: `${c.verdict.warnings.join(', ')}, which is a lot more than usual.`,
        confirm: 'Download anyway',
      }))
    if (ok) grab.mutate(c)
  }
  const matched = data?.filter((c) => c.episodes.length > 0) ?? []
  const accepted = matched.filter((c) => c.verdict.accepted)
  const rejected = data?.filter((c) => !c.verdict.accepted) ?? []
  const what = episodes.length === 1 ? episodeCode(season, episodes[0]) : `season ${season}`
  // Drop what's shown so a fresh search looks like one.
  const refresh = () => {
    setManual(null)
    void qc.resetQueries({ queryKey, exact: true })
  }
  const submit = () => {
    setManual(null)
    const q = draft.trim()
    if (q === query) refresh()
    else setQuery(q)
  }

  return (
    <Dialog onClose={onClose} width="max-w-3xl">
      <div className="flex items-start gap-4">
        <div className="min-w-0 flex-1">
          <p className="text-[15px] font-medium">
            {title} <span className="text-ink-3">{what}</span>
          </p>
          <p className="mt-0.5 text-sm text-ink-3">
            {isFetching
              ? 'Asking every source…'
              : data
                ? `${accepted.length} good match${accepted.length === 1 ? '' : 'es'} out of ${data.length} result${data.length === 1 ? '' : 's'}`
                : ''}
          </p>
        </div>
        <IconButton label="Search again" onClick={refresh} disabled={isFetching}>
          <RefreshCw className={`size-4 ${isFetching ? 'animate-spin' : ''}`} />
        </IconButton>
      </div>

      <form
        className="mt-3 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault()
          submit()
        }}
      >
        <div className="flex-1">
          <Input value={draft} onChange={(e) => setDraft(e.target.value)} placeholder={`${title}, or your own words`} />
        </div>
        <Button type="submit" className="h-9" disabled={isFetching}>
          <Search className="size-3.5" /> Search
        </Button>
      </form>

      {manual && (
        <form
          className="mt-3 space-y-3 rounded-xl bg-panel p-3"
          onSubmit={(e) => {
            e.preventDefault()
            const s = Number(manualSeason)
            const ep = Number(manualEpisode)
            if (!manualSeason.trim() || !manualEpisode.trim() || !Number.isInteger(s) || !Number.isInteger(ep) || s < 0 || s > 999 || ep < 1 || ep > 9999) return
            grab.mutate({ ...manual, episodes: [{ season: s, episode: ep }] })
          }}
        >
          <p className="text-sm text-ink-2">Couldn't resolve the episode for {manual.release.title}. Choose it below, or search for another release.</p>
          <p className="text-xs text-ink-3">For a single-episode release only.</p>
          <div className="flex flex-wrap items-end gap-2">
            <label className="w-24 text-xs text-ink-3">
              Season
              <Input type="number" min={0} max={999} required value={manualSeason} onChange={(e) => setManualSeason(e.target.value)} />
            </label>
            <label className="w-24 text-xs text-ink-3">
              Episode
              <Input type="number" min={1} max={9999} required value={manualEpisode} onChange={(e) => setManualEpisode(e.target.value)} />
            </label>
            <Button type="submit" disabled={grab.isPending}>Download this episode</Button>
            <Button type="button" variant="plain" disabled={grab.isPending} onClick={() => setManual(null)}>Cancel</Button>
          </div>
        </form>
      )}

      <div className="mt-4 max-h-[62vh] space-y-1 overflow-y-auto">
        {isFetching && !data && (
          <div className="grid place-items-center py-16 text-ink-3">
            <Spinner />
          </div>
        )}
        {error && (
          <p className="flex items-center gap-2 py-8 text-sm text-danger">
            <CircleAlert className="size-4" /> {(error as Error).message}
          </p>
        )}
        {data && data.length === 0 && <p className="py-10 text-center text-sm text-ink-3">No results</p>}
        {accepted.map((c, i) => (
          <ReleaseRow key={c.release.link} c={c} best={i === 0 && !c.verdict.nonstandard} done={grabbed.has(c.release.link)} busy={grab.isPending} onGrab={() => void download(c)} />
        ))}
        {rejected.length > 0 && (
          <button className="mt-3 w-full py-2 text-left text-xs text-ink-3 hover:text-ink-2" onClick={() => setShowRejected((s) => !s)}>
            {showRejected ? 'Hide' : 'Show'} {rejected.length} that don't fit
          </button>
        )}
        {showRejected &&
          rejected.map((c) => (
            <ReleaseRow key={c.release.link} c={c} done={grabbed.has(c.release.link)} busy={grab.isPending} onGrab={() => {
              if (c.episodes.length > 0) void download(c)
              else {
                setManual(c)
                setManualSeason(String(season))
                setManualEpisode(episodes.length === 1 ? String(episodes[0]) : '')
              }
            }} />
          ))}
        {grab.error && <p className="pt-2 text-sm text-danger">{(grab.error as Error).message}</p>}
      </div>
    </Dialog>
  )
}

function ReleaseRow({ c, best, done, busy, onGrab }: { c: ReleaseCandidate; best?: boolean; done: boolean; busy: boolean; onGrab: () => void }) {
  const a = c.attributes
  const r = c.release
  const ok = c.verdict.accepted
  return (
    <Squircle radius={12} className={`flex items-center gap-3 px-3 py-2.5 transition-colors hover:bg-hover ${ok ? '' : 'opacity-60'}`}>
      <div className="min-w-0 flex-1">
        <Tip label={r.title} className="block truncate text-[13px] font-medium">
          {r.title}
        </Tip>
        <div className="mt-1 flex flex-wrap items-center gap-1">
          {best && <Badge tone="strong">Best match</Badge>}
          {a.resolution && <Badge>{a.resolution}p</Badge>}
          {a.group && <Badge>{a.group}</Badge>}
          {a.codec && <Badge>{a.codec.toUpperCase()}</Badge>}
          {a.source && <Badge>{a.source === 'bluray' ? 'Blu-ray' : a.source.toUpperCase()}</Badge>}
          {a.dualAudio && <Badge>Dual audio</Badge>}
          {a.version > 1 && <Badge>v{a.version}</Badge>}
          {c.batch && (
            <Badge tone="live">
              <Layers className="size-3" /> {c.episodes.length} episodes
            </Badge>
          )}
          {c.episodes.length === 1 && <Badge>{episodeCode(c.episodes[0].season, c.episodes[0].episode)}</Badge>}
          {c.verdict.warnings.map((why) => (
            <Badge key={why} tone="warn">
              {why}
            </Badge>
          ))}
          {c.verdict.nonstandard && (
            <Badge tone="warn" title="Best to avoid: nonstandard episode numbering">
              <span className="max-w-36 truncate sm:max-w-none">Best to avoid: nonstandard episode numbering</span>
            </Badge>
          )}
          {c.verdict.rejections.map((why) => (
            <Badge key={why} tone="danger" title={why}>
              <span className="max-w-36 truncate sm:max-w-none">{why}</span>
            </Badge>
          ))}
        </div>
      </div>
      <div className="hidden shrink-0 flex-col items-end gap-0.5 text-xs text-ink-3 tabular sm:flex">
        <span>{bytes(r.size)}</span>
        <span className="flex items-center gap-2">
          {r.seeders != null && (
            <Tip label="Seeding" className="flex items-center gap-0.5">
              <Sprout className="size-3" />
              {r.seeders}
            </Tip>
          )}
          {r.leechers != null && (
            <Tip label="Downloading" className="flex items-center gap-0.5">
              <Users className="size-3" />
              {r.leechers}
            </Tip>
          )}
        </span>
      </div>
      <div className="hidden w-24 shrink-0 text-right text-xs text-ink-3 md:block">
        <p className="truncate">{r.source}</p>
        {r.published && <p>{relative(r.published)}</p>}
      </div>
      {r.page && (
        <Tip label="Open on the source">
          <a href={r.page} target="_blank" rel="noreferrer" aria-label="Open on the source" className="block text-ink-3 hover:text-ink">
            <ExternalLink className="size-4" />
          </a>
        </Tip>
      )}
      <Button variant={best ? 'primary' : 'quiet'} size="sm" disabled={done || busy} onClick={onGrab}>
        {done ? <Check className="size-3.5" /> : <ArrowDownToLine className="size-3.5" />}
        {done ? 'Added' : c.episodes.length === 0 ? 'Choose episode' : 'Download'}
      </Button>
    </Squircle>
  )
}
