// SPDX-License-Identifier: AGPL-3.0-or-later
// Now Playing: the cover big, and beside it the lyrics in time, what's
// queued, or exactly how the music reaches you.

import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { ChevronDown, Trash2, X } from 'lucide-react'
import { type CSSProperties, useEffect, useLayoutEffect, useRef } from 'react'
import { Segmented } from '../components/ui'
import { request } from '../lib/api'
import { LyricsQuery, cover, duration, hiRes, quality } from './api'
import { Artists, Cover, Playing, QualityBadge, StarButton } from './components'
import { Elsewhere, Scrubber, Transport, Volume } from './controls'
import { current, music, outputRate, usePlayer, usePosition } from './player'

export type Tab = 'lyrics' | 'queue' | 'details'

/** How big the cover is drawn, so it can be loaded before it's opened. */
export const ART = 520

const TABS: { value: Tab; label: string }[] = [
  { value: 'lyrics', label: 'Lyrics' },
  { value: 'queue', label: 'Queue' },
  { value: 'details', label: 'Details' },
]

function Lyrics({ trackId }: { trackId: number }) {
  const { data, isPending } = useQuery({
    queryKey: ['music', 'lyrics', trackId],
    queryFn: async () => (await request(LyricsQuery, { trackId })).lyrics,
    staleTime: Infinity,
  })
  const time = usePosition()
  const box = useRef<HTMLDivElement>(null)
  const lines = data?.lines ?? []
  const ms = time * 1000
  let at = -1
  if (data?.synced) for (let i = 0; i < lines.length && (lines[i].start ?? 0) <= ms + 150; i++) at = i
  useLayoutEffect(() => {
    const el = box.current?.querySelector<HTMLElement>(`[data-line="${at}"]`)
    const b = box.current
    if (!el || !b) return
    b.scrollTo({ top: el.offsetTop - b.clientHeight * 0.38, behavior: 'smooth' })
  }, [at])

  if (isPending) return <p className="py-20 text-center text-sm text-ink-3">Finding the lyrics…</p>
  if (!data) return <p className="py-20 text-center text-sm text-ink-3">No lyrics for this one.</p>
  return (
    <div
      ref={box}
      className="h-full overflow-y-auto px-1 py-[30%]"
      style={{ maskImage: 'linear-gradient(transparent, black 18%, black 78%, transparent)' }}
    >
      {lines.map((l, i) =>
        data.synced ? (
          <button
            key={i}
            data-line={i}
            onClick={() => l.start != null && music.seek(l.start / 1000)}
            className={`block w-full origin-left py-2 text-left text-[clamp(1.25rem,2.4vw,1.9rem)] leading-snug font-semibold tracking-tight transition-[opacity,scale,filter,color] duration-500 ease-out ${
              i === at ? 'scale-100 text-ink opacity-100' : i < at ? 'scale-[0.97] text-ink opacity-35' : 'scale-[0.97] text-ink opacity-50 blur-[0.6px] hover:opacity-80'
            }`}
          >
            {l.text || '♪'}
          </button>
        ) : (
          <p key={i} className="py-1 text-lg leading-relaxed text-ink-2">
            {l.text || ' '}
          </p>
        ),
      )}
      {data.source === 'ONLINE' && <p className="pt-10 text-xs text-ink-3">Lyrics from LRCLIB</p>}
    </div>
  )
}

function Queue() {
  const queue = usePlayer((s) => s.queue)
  const index = usePlayer((s) => s.index)
  const paused = usePlayer((s) => !s.playing)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>('[data-current]')?.scrollIntoView({ block: 'center' })
  }, [])
  return (
    <div ref={ref} className="h-full overflow-y-auto pr-1">
      <div className="mb-2 flex items-center justify-between px-2">
        <p className="text-xs text-ink-3 tabular">
          {queue.length} track{queue.length === 1 ? '' : 's'} · {duration(queue.slice(index).reduce((n, e) => n + e.track.duration, 0))} left
        </p>
        <button onClick={() => music.clear()} className="flex items-center gap-1.5 rounded-lg px-2 py-1 text-xs text-ink-3 hover:bg-hover hover:text-ink">
          <Trash2 className="size-3.5" /> Clear
        </button>
      </div>
      {queue.map((e, i) => (
        <div
          key={e.uid}
          data-current={i === index || undefined}
          onClick={() => i !== index && music.jumpTo(i)}
          className={`group flex items-center gap-3 rounded-xl px-2 py-1.5 transition-colors hover:bg-hover ${i < index ? 'opacity-45' : ''} ${i === index ? 'bg-press' : ''}`}
        >
          <span className="relative">
            <Cover src={e.track.cover} size={40} className="size-10" />
            {i === index && (
              <span className="absolute inset-0 grid place-items-center rounded-md bg-media-shade/55 text-media-ink">
                <Playing paused={paused} />
              </span>
            )}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-sm">{e.track.title}</span>
            <span className="block truncate text-xs text-ink-3">{e.track.artist}</span>
          </span>
          <span className="text-xs text-ink-3 tabular">{duration(e.track.duration)}</span>
          {i !== index && (
            <button
              aria-label="Remove from queue"
              onClick={(ev) => {
                ev.stopPropagation()
                music.remove(e.uid)
              }}
              className="grid size-7 place-items-center rounded-full text-ink-3 opacity-0 transition-opacity group-hover:opacity-100 hover:text-ink"
            >
              <X className="size-3.5" />
            </button>
          )}
        </div>
      ))}
    </div>
  )
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline gap-4 border-b border-line py-2.5 text-sm last:border-0">
      <span className="w-36 shrink-0 text-ink-3">{label}</span>
      <span className="min-w-0 flex-1 text-ink">{children}</span>
    </div>
  )
}

const DECODED_HERE = new Set(['flac', 'mp3', 'aac', 'alac', 'vorbis', 'pcm'])

function Details() {
  const entry = usePlayer(current)
  const gainMode = usePlayer((s) => s.settings.gain)
  const crossfade = usePlayer((s) => s.settings.crossfade)
  if (!entry) return null
  const t = entry.track
  const direct = !entry.fallback && DECODED_HERE.has(t.codec)
  const rate = outputRate()
  const g = t.gains
  const gain = gainMode === 'off' ? null : gainMode === 'album' ? (g.albumGain ?? g.trackGain) : g.trackGain
  return (
    <div className="h-full overflow-y-auto pr-1">
      <Row label="File">
        {quality(t)}
        {hiRes(t) && ' · Hi-res'}
        {t.channels && t.channels !== 2 ? ` · ${t.channels} channels` : ''}
      </Row>
      <Row label="Bitrate">{t.bitrate ? `${t.bitrate} kbit/s` : '—'}</Row>
      <Row label="Size">{(t.size / 1_048_576).toFixed(1)} MB</Row>
      <Row label="Getting here">{direct ? 'The original file, untouched' : `Converted to FLAC on the server, losslessly (${t.codec} can't be decoded in the browser)`}</Row>
      <Row label="Decoded">In your browser, gaplessly</Row>
      <Row label="Output">
        {rate && t.sampleRate && rate !== t.sampleRate ? `Resampled from ${t.sampleRate / 1000} kHz to your device's ${rate / 1000} kHz` : `${(rate ?? 48000) / 1000} kHz, as recorded`}
      </Row>
      <Row label="Volume levelling">
        <span className="flex flex-wrap items-center gap-2">
          <Segmented
            size="sm"
            value={gainMode}
            onChange={(v) => music.settings({ gain: v })}
            options={[
              { value: 'auto', label: 'Auto', title: "Album levels when an album plays in order, each song's otherwise" },
              { value: 'track', label: 'Track' },
              { value: 'album', label: 'Album' },
              { value: 'off', label: 'Off' },
            ]}
          />
          {gain != null && <span className="text-xs text-ink-3 tabular">{gain > 0 ? '+' : ''}{gain.toFixed(1)} dB{g.pending ? ' (measuring)' : ''}</span>}
        </span>
      </Row>
      <Row label="Crossfade">
        <span className="flex items-center gap-3">
          <input
            type="range"
            min={0}
            max={12}
            step={1}
            value={crossfade}
            onChange={(e) => music.settings({ crossfade: Number(e.target.value) })}
            className="w-40 accent-[var(--color-ink)]"
          />
          <span className="text-xs text-ink-3 tabular">{crossfade ? `${crossfade} s, never inside an album` : 'Off'}</span>
        </span>
      </Row>
    </div>
  )
}

export function NowPlaying({ tab, onTab, onClose }: { tab: Tab; onTab: (t: Tab) => void; onClose: () => void }) {
  const entry = usePlayer(current)
  const tint = entry?.track.coverTint ?? null
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return
      if (e.key === 'Escape') onClose()
      else if (e.key === ' ') {
        e.preventDefault()
        void music.toggle()
      } else if (e.key === 'ArrowRight' && e.shiftKey) music.next()
      else if (e.key === 'ArrowLeft' && e.shiftKey) music.previous()
    }
    window.addEventListener('keydown', key)
    const overflow = document.documentElement.style.overflow
    document.documentElement.style.overflow = 'hidden'
    return () => {
      window.removeEventListener('keydown', key)
      document.documentElement.style.overflow = overflow
    }
  }, [onClose])
  if (!entry) return null
  const t = entry.track
  return (
    <div className="fixed inset-0 z-[46] overflow-hidden bg-canvas animate-[fade_200ms_ease-out]" style={{ '--tint': tint ?? '128 128 128' } as CSSProperties}>
      <div aria-hidden className="absolute inset-0 scale-125 opacity-45 blur-[90px] saturate-150">
        {t.cover && <img src={cover(t.cover, 64)} alt="" className="size-full object-cover" />}
      </div>
      <div aria-hidden className="absolute inset-0 bg-[radial-gradient(90%_80%_at_20%_30%,rgb(var(--tint)/0.25),transparent_70%)]" />
      <div aria-hidden className="absolute inset-0 bg-canvas/55" />

      <div className="relative mx-auto flex h-full max-w-[1400px] flex-col px-5 pt-[max(1rem,env(safe-area-inset-top))] pb-[max(1.25rem,env(safe-area-inset-bottom))] md:px-12">
        <div className="flex h-12 items-center">
          <button onClick={onClose} aria-label="Close" className="grid size-10 place-items-center rounded-full text-ink-2 hover:bg-hover hover:text-ink">
            <ChevronDown className="size-5" />
          </button>
          <div className="flex-1" />
          <Segmented value={tab} onChange={onTab} options={TABS} size="sm" />
        </div>

        <div className="grid min-h-0 flex-1 gap-8 pt-4 max-md:grid-rows-[minmax(0,1fr)] md:grid-cols-[minmax(0,26rem)_minmax(0,1fr)] md:gap-16 md:pt-8 lg:grid-cols-[minmax(0,32rem)_minmax(0,1fr)]">
          <div className="flex min-h-0 flex-col justify-center max-md:hidden">
            <Cover src={t.cover} size={ART} className="aspect-square w-full max-w-[min(32rem,52vh)] self-center shadow-[0_40px_80px_-30px_var(--color-shade)] [view-transition-name:now-playing]" />
            <div className="mx-auto mt-7 w-full max-w-[min(32rem,52vh)]">
              <div className="flex items-start gap-3">
                <div className="min-w-0 flex-1">
                  <p key={t.id} className="animate-[fade_400ms_ease-out] truncate text-2xl font-semibold tracking-tight">{t.title}</p>
                  <p className="truncate text-[15px] text-ink-2">
                    <Artists track={t} />
                    {t.albumId != null && (
                      <>
                        {' · '}
                        <Link to="/album/$id" params={{ id: String(t.albumId) }} onClick={onClose} className="hover:text-ink hover:underline">
                          {t.album}
                        </Link>
                      </>
                    )}
                  </p>
                </div>
                <QualityBadge track={t} />
                <StarButton kind="TRACK" id={t.id} starred={t.starred} />
              </div>
              <Elsewhere className="mt-4" />
              <div className="mt-4">
                <Scrubber tint={tint} />
              </div>
              <div className="mt-2">
                <Transport size="lg" />
              </div>
              <div className="mx-auto mt-4 max-w-52">
                <Volume />
              </div>
            </div>
          </div>

          <div className="flex min-h-0 flex-col">
            {/* Small screens: the cover and controls live around the tabs. */}
            <div className="mb-5 flex shrink-0 items-center gap-4 md:hidden">
              <Cover src={t.cover} size={64} className="size-16 shrink-0" />
              <div className="min-w-0 flex-1">
                <p className="truncate font-semibold">{t.title}</p>
                <p className="truncate text-sm text-ink-2">{t.artist}</p>
              </div>
              <StarButton kind="TRACK" id={t.id} starred={t.starred} />
            </div>
            <div className="min-h-0 flex-1">
              {tab === 'lyrics' && <Lyrics trackId={t.id} />}
              {tab === 'queue' && <Queue />}
              {tab === 'details' && <Details />}
            </div>
            <div className="shrink-0 pt-3 md:hidden">
              <Elsewhere className="mb-2" />
              <Scrubber tint={tint} />
              <Transport />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
