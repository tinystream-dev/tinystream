// SPDX-License-Identifier: AGPL-3.0-or-later
// What the screens about shows being downloaded share (web's
// components/downloads.tsx): episode states, the monitor picker, searching a
// show's sources for a release, and deleting what was downloaded.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { bytes, episodeCode, monitorLabels, relative, stateLabels } from '@tinystream/shared/downloads'
import { ArrowDownToLine, Check, CircleAlert, ExternalLink, Layers, RefreshCw, Search, Sprout, Trash2, Users } from 'lucide-react-native'
import { useState } from 'react'
import { Linking, Pressable, ScrollView, Text, View } from 'react-native'
import Animated from 'react-native-reanimated'
import { Squircle } from '../effects/Squircle'
import { useMotion } from '../effects/motion'
import { graphql } from '../gql'
import type { EpisodeState, Monitor } from '../gql/graphql'
import type { ReleaseCandidate } from '../queries'
import { useApi } from '../session'
import { useTheme } from '../theme/ThemeProvider'
import { ask, toast, toastError } from './Feedback'
import type { MenuItem } from './Menu'
import { Sheet } from './Sheet'
import { Badge, Button, IconButton, Input, Segmented, Spinner, type Tone } from './ui'

const stateTones: Record<EpisodeState, Tone> = {
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
  const api = useApi()
  const qc = useQueryClient()
  const remove = useMutation({
    mutationFn: async (t: DeleteTarget) => (await api.request(DeleteDownloaded, { seriesId: t.seriesId, season: t.season ?? null })).deleteDownloaded,
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
export function deleteItems(target: DeleteTarget, onDelete: (t: DeleteTarget) => void): MenuItem[] {
  const icon = (c: string) => <Trash2 size={18} color={c} />
  return [
    ...(target.season != null ? [{ label: `Delete ${seasonName(target.season)}`, icon, danger: true, onPress: () => onDelete(target) }] : []),
    { label: 'Delete show', icon, danger: true, onPress: () => onDelete({ ...target, season: null }) },
  ]
}

/** Puts skipped episodes of a show back, to be downloaded if it's monitored. */
export function useLookForAgain() {
  const api = useApi()
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (v: { seriesId: number; season?: number; episode?: number }) =>
      api.request(LookForAgain, { seriesId: v.seriesId, season: v.season ?? null, episode: v.episode ?? null }),
    onError: toastError,
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: ['series'] })
      void qc.invalidateQueries({ queryKey: ['wanted'] })
    },
  })
}

export function StateBadge({ state }: { state: EpisodeState }) {
  const { tokens } = useTheme()
  const motion = useMotion()
  return (
    <Badge
      tone={stateTones[state]}
      icon={
        state === 'WANTED' ? (
          <Animated.View style={[{ width: 6, height: 6, borderRadius: 3, backgroundColor: tokens.warn }, motion.pulseDot]} />
        ) : state === 'DONE' ? (
          <Check size={11} color={tokens.ok} />
        ) : undefined
      }
    >
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
      options={(['NONE', 'FUTURE', 'MISSING'] as const).map((m) => ({ value: m, label: monitorLabels[m].label }))}
    />
  )
}

/** Searches a show's sources and lets someone pick the release to download (web's ReleaseDialog). */
export function ReleaseSheet({
  open,
  seriesId,
  season,
  episodes,
  title,
  onClose,
}: {
  open: boolean
  seriesId: number
  season: number
  episodes: number[]
  title: string
  onClose: () => void
}) {
  return (
    <Sheet open={open} onClose={onClose}>
      {open && <Releases seriesId={seriesId} season={season} episodes={episodes} title={title} />}
    </Sheet>
  )
}

function Releases({ seriesId, season, episodes, title }: { seriesId: number; season: number; episodes: number[]; title: string }) {
  const api = useApi()
  const qc = useQueryClient()
  const { tokens } = useTheme()
  const [showRejected, setShowRejected] = useState(false)
  // Empty means let the server pick the words.
  const [draft, setDraft] = useState('')
  const [query, setQuery] = useState('')
  const queryKey = ['releases', seriesId, season, episodes, query]
  const { data, isFetching, error } = useQuery({
    queryKey,
    queryFn: async () => (await api.request(ReleasesQuery, { seriesId, season, episodes, query: query || null })).series?.releases ?? [],
    staleTime: 0,
    gcTime: 0,
  })
  const [grabbed, setGrabbed] = useState<Set<string>>(new Set())
  const [manual, setManual] = useState<ReleaseCandidate | null>(null)
  const [manualSeason, setManualSeason] = useState(String(season))
  const [manualEpisode, setManualEpisode] = useState(episodes.length === 1 ? String(episodes[0]) : '')
  const grab = useMutation({
    mutationFn: (c: ReleaseCandidate) => api.request(GrabRelease, { release: c.release, seriesId, episodes: c.episodes }),
    onSuccess: (_, c) => {
      toast({ title: 'Downloading', body: c.release.title, tone: 'ok' })
      setGrabbed((g) => new Set(g).add(c.release.link))
      setManual(null)
      void qc.invalidateQueries({ queryKey: ['downloads'] })
      void qc.invalidateQueries({ queryKey: ['series'] })
    },
  })
  const download = async (c: ReleaseCandidate) => {
    const ok =
      c.verdict.warnings.length === 0 ||
      (await ask({ title: `Download ${bytes(c.release.size)}?`, body: `${c.verdict.warnings.join(', ')}, which is a lot more than usual.`, confirm: 'Download anyway' }))
    if (ok) grab.mutate(c)
  }
  const matched = data?.filter((c) => c.episodes.length > 0) ?? []
  const accepted = matched.filter((c) => c.verdict.accepted)
  const rejected = data?.filter((c) => !c.verdict.accepted) ?? []
  const what = episodes.length === 1 ? episodeCode(season, episodes[0]) : `season ${season}`
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
  const submitManual = () => {
    const s = Number(manualSeason)
    const ep = Number(manualEpisode)
    if (!manual || !Number.isInteger(s) || !Number.isInteger(ep) || s < 0 || s > 999 || ep < 1 || ep > 9999) return
    grab.mutate({ ...manual, episodes: [{ season: s, episode: ep }] })
  }

  return (
    <View style={{ maxHeight: 640 }}>
      <View className="flex-row items-start gap-3">
        <View className="min-w-0 flex-1">
          <Text className="font-sans text-[17px] font-semibold text-ink" numberOfLines={2}>
            {title} <Text className="text-ink-3">{what}</Text>
          </Text>
          <Text className="font-sans mt-0.5 text-sm text-ink-3">
            {isFetching ? 'Asking every source…' : data ? `${accepted.length} good match${accepted.length === 1 ? '' : 'es'} out of ${data.length} results` : ''}
          </Text>
        </View>
        <IconButton label="Search again" onPress={refresh} disabled={isFetching}>
          <RefreshCw size={18} color={tokens['ink-2']} />
        </IconButton>
      </View>
      <View className="mt-3 flex-row gap-2">
        <View className="flex-1">
          <Input value={draft} onChangeText={setDraft} onSubmitEditing={submit} returnKeyType="search" placeholder={`${title}, or your own words`} />
        </View>
        <Button onPress={submit} disabled={isFetching} icon={<Search size={16} color={tokens.ink} />} className="self-center" />
      </View>
      {manual && (
        <Squircle radius={12} className="mt-3 gap-2 bg-panel p-3">
          <Text className="font-sans text-sm text-ink-2">Couldn't resolve the episode for {manual.release.title}. Choose it below, or search for another release.</Text>
          <Text className="font-sans text-xs text-ink-3">For a single-episode release only.</Text>
          <View className="flex-row items-end gap-2">
            <View className="w-20">
              <Input keyboardType="number-pad" value={manualSeason} onChangeText={setManualSeason} placeholder="Season" />
            </View>
            <View className="w-20">
              <Input keyboardType="number-pad" value={manualEpisode} onChangeText={setManualEpisode} placeholder="Episode" />
            </View>
            <Button variant="primary" size="sm" disabled={grab.isPending} onPress={submitManual} className="self-center">
              Download
            </Button>
          </View>
        </Squircle>
      )}
      <ScrollView className="mt-3" style={{ flexGrow: 0 }} nestedScrollEnabled>
        {isFetching && !data && (
          <View className="items-center py-14">
            <Spinner />
          </View>
        )}
        {error && (
          <View className="flex-row items-center gap-2 py-6">
            <CircleAlert size={16} color={tokens.danger} />
            <Text className="font-sans flex-1 text-sm text-danger">{(error as Error).message}</Text>
          </View>
        )}
        {data && data.length === 0 && <Text className="font-sans py-10 text-center text-sm text-ink-3">No results</Text>}
        {accepted.map((c, i) => (
          <ReleaseRow key={c.release.link} c={c} best={i === 0 && !c.verdict.nonstandard} done={grabbed.has(c.release.link)} busy={grab.isPending} onGrab={() => void download(c)} />
        ))}
        {rejected.length > 0 && (
          <Pressable onPress={() => setShowRejected((s) => !s)} className="py-3">
            <Text className="font-sans text-xs text-ink-3">
              {showRejected ? 'Hide' : 'Show'} {rejected.length} that don't fit
            </Text>
          </Pressable>
        )}
        {showRejected &&
          rejected.map((c) => (
            <ReleaseRow
              key={c.release.link}
              c={c}
              done={grabbed.has(c.release.link)}
              busy={grab.isPending}
              onGrab={() => {
                if (c.episodes.length > 0) void download(c)
                else {
                  setManual(c)
                  setManualSeason(String(season))
                  setManualEpisode(episodes.length === 1 ? String(episodes[0]) : '')
                }
              }}
            />
          ))}
        {grab.error && <Text className="font-sans pt-2 text-sm text-danger">{(grab.error as Error).message}</Text>}
      </ScrollView>
    </View>
  )
}

function ReleaseRow({ c, best, done, busy, onGrab }: { c: ReleaseCandidate; best?: boolean; done: boolean; busy: boolean; onGrab: () => void }) {
  const { tokens } = useTheme()
  const a = c.attributes
  const r = c.release
  return (
    <View className="gap-1.5 border-b border-line py-3" style={{ opacity: c.verdict.accepted ? 1 : 0.6 }}>
      <Text className="font-sans text-[13px] font-medium text-ink" numberOfLines={2}>
        {r.title}
      </Text>
      <View className="flex-row flex-wrap gap-1">
        {best && <Badge tone="strong">Best match</Badge>}
        {a.resolution && <Badge>{`${a.resolution}p`}</Badge>}
        {a.group && <Badge>{a.group}</Badge>}
        {a.codec && <Badge>{a.codec.toUpperCase()}</Badge>}
        {a.source && <Badge>{a.source === 'bluray' ? 'Blu-ray' : a.source.toUpperCase()}</Badge>}
        {a.dualAudio && <Badge>Dual audio</Badge>}
        {a.version > 1 && <Badge>{`v${a.version}`}</Badge>}
        {c.batch && (
          <Badge tone="live" icon={<Layers size={11} color={tokens.info} />}>
            {`${c.episodes.length} episodes`}
          </Badge>
        )}
        {c.episodes.length === 1 && <Badge>{episodeCode(c.episodes[0].season, c.episodes[0].episode)}</Badge>}
        {c.verdict.warnings.map((why) => (
          <Badge key={why} tone="warn">
            {why}
          </Badge>
        ))}
        {c.verdict.nonstandard && <Badge tone="warn">Best to avoid: nonstandard episode numbering</Badge>}
        {c.verdict.rejections.map((why) => (
          <Badge key={why} tone="danger">
            {why}
          </Badge>
        ))}
      </View>
      <View className="flex-row items-center gap-3">
        <Text className="font-sans flex-1 text-xs text-ink-3" numberOfLines={1}>
          {[bytes(r.size), r.source, r.published ? relative(r.published) : null].filter(Boolean).join(' · ')}
        </Text>
        {r.seeders != null && (
          <View className="flex-row items-center gap-1">
            <Sprout size={12} color={tokens['ink-3']} />
            <Text className="font-sans text-xs text-ink-3">{r.seeders}</Text>
          </View>
        )}
        {r.leechers != null && (
          <View className="flex-row items-center gap-1">
            <Users size={12} color={tokens['ink-3']} />
            <Text className="font-sans text-xs text-ink-3">{r.leechers}</Text>
          </View>
        )}
        {r.page && (
          <IconButton label="Open on the source" size={32} onPress={() => void Linking.openURL(r.page!)}>
            <ExternalLink size={15} color={tokens['ink-3']} />
          </IconButton>
        )}
        <Button
          variant={best ? 'primary' : 'quiet'}
          size="sm"
          disabled={done || busy}
          onPress={onGrab}
          icon={done ? <Check size={14} color={best ? tokens['on-accent'] : tokens.ink} /> : <ArrowDownToLine size={14} color={best ? tokens['on-accent'] : tokens.ink} />}
        >
          {done ? 'Added' : c.episodes.length === 0 ? 'Choose episode' : 'Download'}
        </Button>
      </View>
    </View>
  )
}
