// SPDX-License-Identifier: AGPL-3.0-or-later
// Making a clip: pick the range on a filmstrip, the tracks and the quality,
// watch it loop exactly as it'll come out, then send it off to render.
//
// The editor plays its own copy of the video, so in a watch-together room the
// room keeps going (the player shrinks it into a corner meanwhile).

import { useQuery } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { Check, Copy, Download, Link2, Pause, Play, Scissors, Send, TriangleAlert, X } from 'lucide-react'
import { type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Avatar } from '../components/Avatar'
import { toast, toastError } from '../components/feedback'
import { Squircle } from '../components/Squircle'
import { Button, Checkbox, IconButton, Input, Panel, Popover, Progress, Select, Spinner, Toggle } from '../components/ui'
import { graphql } from '../gql'
import { type Clip, type ClipAllowance, type Person, type Playback, request } from '../lib/api'
import { usePeople } from '../lib/hooks'

const AllowanceQuery = graphql(`
  query ClipAllowance {
    clipAllowance {
      ...ClipAllowanceFields
    }
  }
`)

const UpdateClip = graphql(`
  mutation UpdateClip($id: Int!, $input: ClipPatch!) {
    updateClip(id: $id, input: $input) {
      ...ClipFields
    }
  }
`)

const ShareClip = graphql(`
  mutation ShareClip($id: Int!, $users: [Int!]!) {
    shareClip(id: $id, users: $users) {
      ...ClipFields
    }
  }
`)

const CreateClip = graphql(`
  mutation CreateClip($input: NewClip!) {
    createClip(input: $input) {
      ...ClipFields
    }
  }
`)

/** What you may do with clips; cached under ['clip-allowance']. */
export const allowanceQuery = {
  queryKey: ['clip-allowance'],
  queryFn: async () => (await request(AllowanceQuery)).clipAllowance,
}
import { type Preset, burnable, copyLink, fetchClip, length, presets, space, stamp, worstSize } from '../lib/clips'
import { bytes } from '../lib/downloads'
import { language } from '@tinystream/shared/format'
import { onArrival, useMarkRead } from '../lib/notifications'
import { ClipPlayer } from './ClipPlayer'
import { StreamEngine, plan } from './engine'
import { SubtitleRenderer } from './subtitles'

/** Where the preview's first stretch starts when nothing says otherwise. */
const DEFAULT_LENGTH = 30
const MIN_LENGTH = 0.5
const PREVIEW_STEP = 10
const CJK = ['jpn', 'ja', 'chi', 'zho', 'zh', 'kor', 'ko']

type Props = {
  pb: Playback
  /** Where the video's endpoints live (a room has its own copies). */
  mediaBase: string
  /** Clipping from a watch-together room. */
  room?: string
  /** Where playback was when Clip was pressed. */
  at: number
  audio: number | null
  subtitle: string | null
  /** Changing an existing clip instead of making one. */
  editing?: Clip
  onClose: () => void
  /** Whether the preview is playing (the room's audio steps back meanwhile). */
  onPreviewPlaying?: (playing: boolean) => void
}

export function ClipEditor(props: Props) {
  const { data: you } = useQuery(allowanceQuery)
  const [made, setMade] = useState<number | null>(null)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && !e.defaultPrevented && props.onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [props])
  return (
    <div className="absolute inset-0 z-20 flex flex-col bg-media-canvas text-ink animate-[fade_160ms_ease-out]" onClick={(e) => e.stopPropagation()}>
      {!you ? (
        <div className="grid flex-1 place-items-center text-ink-2">
          <Spinner className="size-8" />
        </div>
      ) : made !== null ? (
        <Rendering id={made} onClose={props.onClose} />
      ) : (
        <Editor {...props} you={you} onMade={setMade} />
      )}
    </div>
  )
}

function Editor({ pb, mediaBase, room, at, audio: startAudio, subtitle: startSubtitle, editing, onClose, onPreviewPlaying, you, onMade }: Props & {
  you: ClipAllowance
  onMade: (id: number) => void
}) {
  const duration = pb.media.duration ?? 0
  const fps = pb.media.video?.fps || 24
  const snap = useCallback((t: number) => Math.round(t * fps) / fps, [fps])
  const max = Math.min(you.maxLength || Infinity, duration || Infinity)

  const [range, setRange] = useState<[number, number]>(() => {
    if (editing) return [editing.start, Math.min(editing.end, editing.start + max)]
    const len = Math.min(DEFAULT_LENGTH, max)
    // The moment just watched, usually; from the start when near it.
    const end = Math.min(duration || Infinity, Math.max(at, len))
    return [snap(Math.max(0, end - len)), snap(end)]
  })
  const [start, end] = range
  const [audio, setAudio] = useState<number | null>(editing ? editing.audio : startAudio)
  const [subtitle, setSubtitle] = useState<string | null>(() => {
    const id = editing ? editing.subtitles : startSubtitle
    return pb.media.subtitles.some((s) => s.id === id && burnable(s)) ? id : null
  })
  const options = useMemo(() => presets(pb.media.video), [pb])
  const [preset, setPreset] = useState(() => {
    const i = editing ? options.findIndex((p) => p.height === editing.quality.height && p.halfRate === editing.quality.halfRate) : 0
    return Math.max(0, i)
  })
  const [title, setTitle] = useState(editing?.name ?? '')
  const [recipients, setRecipients] = useState<Person[]>([])
  const [isPublic, setPublic] = useState(editing?.public ?? false)
  const [saving, setSaving] = useState(false)

  const track = pb.media.subtitles.find((s) => s.id === subtitle) ?? null
  const bitmap = !!track && !track.supported
  const video = useRef<HTMLVideoElement>(null)
  const [time, setTime] = useState(start)
  const [playing, setPlaying] = useState(false)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const rangeRef = useRef(range)
  rangeRef.current = range
  const engine = useRef<StreamEngine | null>(null)
  const resumeAt = useRef(start)
  const resumePlaying = useRef(false)

  // (Re)build the stream when the audio changes, keeping our place (and
  // playing on if it was).
  useEffect(() => {
    const v = video.current
    if (!v) return
    const e = new StreamEngine(v, mediaBase, pb.media.duration, plan(pb, audio, 'auto'), (ev) => ev.error && setError(ev.error))
    engine.current = e
    setError(null)
    setLoading(true)
    const from = resumeAt.current
    const play = resumePlaying.current
    void e.start(from).then(() => {
      if (play && engine.current === e) void v.play().catch(() => {})
    })
    return () => {
      // Until the stream has loaded, the video's time doesn't reflect where
      // we meant to be, so carry the intent over instead.
      const loaded = v.readyState > 0
      resumeAt.current = loaded ? v.currentTime : from
      resumePlaying.current = loaded ? !v.paused : play
      engine.current = null
      e.destroy()
    }
  }, [pb, mediaBase, audio])

  const subs = useRef<SubtitleRenderer | null>(null)
  useEffect(() => {
    const v = video.current
    if (!v) return
    const r = new SubtitleRenderer(v, mediaBase, pb.media.fonts.map((f) => `${mediaBase}/fonts/${f.index}`))
    subs.current = r
    return () => r.destroy()
  }, [pb, mediaBase])
  useEffect(() => {
    void subs.current?.show(track?.supported ? track.id : null).catch((e) => console.warn('subtitles', e))
  }, [track])

  // Loops the range while it plays.
  useEffect(() => {
    if (!playing) return
    let raf = 0
    const tick = () => {
      const v = video.current
      const [s, e] = rangeRef.current
      if (v && (v.currentTime >= e || v.currentTime < s - 0.3)) v.currentTime = s
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [playing])
  useEffect(() => void onPreviewPlaying?.(playing), [playing, onPreviewPlaying])
  useEffect(() => () => onPreviewPlaying?.(false), [onPreviewPlaying])

  const seek = useCallback((t: number) => {
    const v = video.current
    if (!v) return
    v.currentTime = t
    setTime(t)
  }, [])
  const togglePlay = useCallback(() => {
    const v = video.current
    if (!v) return
    if (!v.paused) return v.pause()
    const [s, e] = rangeRef.current
    if (v.currentTime < s || v.currentTime >= e - 0.05) v.currentTime = s
    void v.play().catch(() => {})
  }, [])
  const changeRange = useCallback(
    (r: [number, number], moved: 'start' | 'end' | 'both') => {
      setRange(r)
      // Show the edge being moved; moving the whole range starts it over.
      if (video.current?.paused || moved !== 'end') seek(moved === 'end' ? Math.max(r[0], r[1] - 1 / fps) : r[0])
    },
    [seek, fps],
  )
  const setIn = useCallback(() => {
    const t = snap(video.current?.currentTime ?? start)
    const [, e] = rangeRef.current
    changeRange([t, Math.min(Math.max(e, t + MIN_LENGTH), t + max, duration)], 'start')
  }, [snap, start, max, duration, changeRange])
  const setOut = useCallback(() => {
    const t = snap(video.current?.currentTime ?? end)
    const [s] = rangeRef.current
    changeRange([Math.max(Math.min(s, t - MIN_LENGTH), t - max, 0), t], 'end')
  }, [snap, end, max, changeRange])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target
      if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement || e.metaKey || e.ctrlKey || e.altKey) return
      const k = e.key.toLowerCase()
      if (k === ' ' || k === 'k') (e.preventDefault(), togglePlay())
      else if (k === 'i') (e.preventDefault(), setIn())
      else if (k === 'o') (e.preventDefault(), setOut())
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [togglePlay, setIn, setOut])
  const chosen: Preset = options[preset] ?? options[0]
  const cjk = !!track?.supported && CJK.includes(track.language ?? '') && !you.customDefaultFont && (track.codec !== 'ass' || pb.media.fonts.length === 0)
  const save = async () => {
    setSaving(true)
    const recipe = { start, end, audio, subtitles: subtitle, height: chosen.height, halfRate: chosen.halfRate }
    try {
      let clip: Clip
      if (editing) {
        clip = (await request(UpdateClip, { id: editing.id, input: { name: title, public: isPublic, recipe } })).updateClip
        if (recipients.length) clip = (await request(ShareClip, { id: editing.id, users: recipients.map((p) => p.id) })).shareClip
      } else {
        clip = (
          await request(CreateClip, {
            input: { videoId: pb.id, recipe, name: title, public: isPublic, recipients: recipients.map((p) => p.id), room },
          })
        ).createClip
      }
      video.current?.pause()
      onMade(clip.id)
    } catch (e) {
      toastError(e)
      setSaving(false)
    }
  }

  const len = end - start
  const source = pb.title.kind === 'SHOW' ? [pb.title.name, pb.label].filter(Boolean).join(' · ') : pb.title.name
  return (
    <>
      {/* In a room, the room's own video sits in the top corner. */}
      <header className={`flex items-center gap-3 px-5 pt-4 pb-3 ${room ? 'md:pr-[22rem]' : ''}`}>
        <IconButton label="Close (Esc)" onClick={onClose} className="text-media-ink/80">
          <X className="size-5" />
        </IconButton>
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-2 text-[15px] font-medium">
            <Scissors className="size-4 text-highlight" />
            {editing ? 'Edit clip' : 'New clip'}
          </p>
          <p className="truncate text-[13px] text-ink-3">{source}</p>
        </div>
        <Usage you={you} />
      </header>

      <div className="relative min-h-0 flex-1 px-5">
        <div className="relative size-full overflow-hidden rounded-2xl bg-media-shade">
          <video
            ref={video}
            playsInline
            className="size-full object-contain"
            onClick={togglePlay}
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
            onWaiting={() => setLoading(true)}
            onPlaying={() => setLoading(false)}
            onCanPlay={() => setLoading(false)}
            onSeeked={() => setLoading(false)}
            onStalled={() => engine.current?.recover()}
            onEnded={(e) => {
              // A range that runs to the very end loops too.
              const v = e.currentTarget
              v.currentTime = rangeRef.current[0]
              void v.play().catch(() => {})
            }}
            onTimeUpdate={(e) => setTime(e.currentTarget.currentTime)}
            onError={(e) => e.currentTarget.error && setError('the browser couldn’t play this stream')}
          />
          {bitmap && !playing && <BurnedStill pb={pb} mediaBase={mediaBase} room={room} track={subtitle!} at={time} />}
          {loading && !error && (
            <div className="pointer-events-none absolute inset-0 grid place-items-center text-media-ink/70">
              <Spinner className="size-8" />
            </div>
          )}
          {error && (
            <div className="absolute inset-0 grid place-items-center p-6 text-center text-sm text-ink-2">
              The preview can’t play: {error}. The clip itself will still render.
            </div>
          )}
          {bitmap && playing && (
            <p className="pointer-events-none absolute inset-x-0 bottom-3 text-center text-xs text-media-ink/60">
              Image subtitles show when the preview is paused; they’ll be in the clip either way.
            </p>
          )}
        </div>
      </div>

      <div className="space-y-4 px-5 pt-4 pb-5">
        <RangeStrip
          base={mediaBase}
          duration={duration}
          range={range}
          max={max}
          time={time}
          frame={1 / fps}
          snap={snap}
          onRange={changeRange}
          onSeek={seek}
        />

        <div className="flex flex-wrap items-center gap-2">
          <Button variant="quiet" onClick={togglePlay} aria-label={playing ? 'Pause' : 'Play the clip'}>
            {playing ? <Pause className="size-4 fill-current" /> : <Play className="size-4 fill-current" />}
            {playing ? 'Pause' : 'Play'}
          </Button>
          <Button variant="plain" size="sm" onClick={setIn}>
            Start here <kbd className="font-sans text-2xs text-ink-3">I</kbd>
          </Button>
          <Button variant="plain" size="sm" onClick={setOut}>
            End here <kbd className="font-sans text-2xs text-ink-3">O</kbd>
          </Button>
          <p className="ml-1 text-[13px] text-ink-2 tabular">
            {stamp(start)} – {stamp(end)}
            <span className="ml-2 text-ink">{length(len)}</span>
            {Number.isFinite(max) && <span className="text-ink-3"> of {length(max)} max</span>}
          </p>
        </div>

        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-[minmax(0,1.4fr)_repeat(3,minmax(0,1fr))]">
          <Labeled label="Title">
            <Input value={title} maxLength={120} placeholder="Optional" onChange={(e) => setTitle(e.target.value)} />
          </Labeled>
          <Labeled label="Audio">
            <Select
              value={String(audio ?? '')}
              options={[
                ...(pb.media.audio.length === 0 ? [{ value: '', label: 'None' }] : []),
                ...pb.media.audio.map((a) => ({
                  value: String(a.index),
                  label: `${trackLabel(a, `Track ${a.index}`)}${a.channels > 2 ? ` · ${a.channels} ch → stereo` : ''}`,
                })),
              ]}
              onChange={(v) => setAudio(v === '' ? null : Number(v))}
            />
          </Labeled>
          <Labeled label="Subtitles" hint="Burned into the video">
            <Select
              value={subtitle ?? ''}
              options={[
                { value: '', label: 'None' },
                ...pb.media.subtitles
                  .filter(burnable)
                  .map((s) => ({
                    value: s.id,
                    label: `${trackLabel(s, s.id.startsWith('x') ? 'External' : `Track ${s.id.slice(1)}`)}${s.supported ? (s.forced ? ' · forced' : '') : ' · image'}`,
                  })),
              ]}
              onChange={(v) => setSubtitle(v || null)}
            />
          </Labeled>
          <Labeled label="Quality" hint={`Up to ${bytes(worstSize(chosen, len))}`}>
            <Select
              value={String(preset)}
              options={options.map((p, i) => ({ value: String(i), label: `${p.label}${p.halfRate ? ' · half rate' : ''}` }))}
              onChange={(v) => setPreset(Number(v))}
            />
          </Labeled>
        </div>

        {cjk && (
          <p className="flex items-start gap-2 text-xs leading-relaxed text-warn-soft/90">
            <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
            {language(track!.language)} text may come out as empty boxes: this server has no font for it set up for clips. An
            admin can set <code className="text-warn-soft">[clips] default-font</code> to one that has it.
          </p>
        )}

        <div className="flex flex-wrap items-center gap-x-5 gap-y-3 border-t border-line pt-4">
          <SendTo value={recipients} onChange={setRecipients} />
          {you.canLink && (
            <label className="flex items-center gap-2.5 text-sm text-ink-2">
              <Toggle label="Public link" checked={isPublic} onChange={setPublic} />
              <Link2 className="size-3.5" /> Anyone with the link
            </label>
          )}
          <div className="flex-1" />
          <Button variant="plain" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" size="lg" onClick={() => void save()} disabled={saving || len < MIN_LENGTH}>
            {saving ? <Spinner className="size-4" /> : <Scissors className="size-4" />}
            {editing ? 'Save and render' : 'Make clip'}
          </Button>
        </div>
      </div>
    </>
  )
}

function trackLabel(t: { title: string | null; language: string | null }, fallback: string) {
  const lang = language(t.language)
  if (t.title && lang && !t.title.toLowerCase().includes(lang.toLowerCase())) return `${lang} · ${t.title}`
  return t.title || lang || fallback
}

function Labeled({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="min-w-0">
      <p className="mb-1.5 flex items-baseline justify-between gap-2 text-xs text-ink-3">
        {label}
        {hint && <span className="truncate text-2xs tabular">{hint}</span>}
      </p>
      {children}
    </div>
  )
}

/** How much of your clip space is used, when there's a limit. */
function Usage({ you }: { you: ClipAllowance }) {
  if (!you.storage) return null
  const used = you.bytes / (you.storage * 1024 * 1024)
  return (
    <div className="hidden w-44 sm:block">
      <p className="mb-1 text-right text-2xs text-ink-3 tabular">
        {bytes(you.bytes)} of {space(you.storage)} used
      </p>
      <Progress value={used} tone={used > 0.9 ? 'live' : 'ink'} />
    </div>
  )
}

/**
 * A frame with image subtitles burned in, from the server (the browser can't
 * draw PGS or VobSub). Follows the playhead once it settles.
 */
function BurnedStill({ pb, mediaBase, room, track, at }: { pb: Playback; mediaBase: string; room?: string; track: string; at: number }) {
  const [src, setSrc] = useState<string | null>(null)
  useEffect(() => {
    const t = setTimeout(() => {
      const q = new URLSearchParams({ at: at.toFixed(2), subtitles: track })
      if (room) q.set('room', room)
      setSrc(`/api/clips/still/${pb.id}?${q}`)
    }, 250)
    return () => clearTimeout(t)
  }, [pb.id, track, at, room, mediaBase])
  if (!src) return null
  return <img src={src} alt="" className="pointer-events-none absolute inset-0 size-full object-contain" />
}
/**
 * The range on a filmstrip of the part of the video around it, zoomed so a
 * frame is easy to hit, with the whole video in a thin bar above for jumping.
 */
function RangeStrip({
  base,
  duration,
  range,
  max,
  time,
  frame,
  snap,
  onRange,
  onSeek,
}: {
  base: string
  duration: number
  range: [number, number]
  max: number
  time: number
  /** One frame, in seconds. */
  frame: number
  snap: (t: number) => number
  onRange: (r: [number, number], moved: 'start' | 'end' | 'both') => void
  onSeek: (t: number) => void
}) {
  const [start, end] = range
  const len = end - start
  const strip = useRef<HTMLDivElement>(null)
  const [drag, setDrag] = useState<{ what: 'start' | 'end' | 'both'; offset: number } | null>(null)
  const around = useCallback(
    (r: [number, number]): [number, number] => {
      const span = Math.min(duration, Math.max(60, (r[1] - r[0]) * 4))
      const from = Math.max(0, Math.min(duration - span, (r[0] + r[1]) / 2 - span / 2))
      return [from, from + span]
    },
    [duration],
  )
  const [view, setView] = useState(() => around(range))
  // Keep the view still while dragging; follow the range once it wanders off.
  useEffect(() => {
    if (!drag && (start < view[0] || end > view[1])) setView(around([start, end]))
  }, [start, end, view, drag, around])
  const span = view[1] - view[0] || 1
  const x = (t: number) => `${((t - view[0]) / span) * 100}%`
  const timeAt = (clientX: number) => {
    const r = strip.current!.getBoundingClientRect()
    return view[0] + Math.max(0, Math.min(1, (clientX - r.left) / r.width)) * span
  }

  const move = (what: 'start' | 'end' | 'both', t: number, offset = 0) => {
    if (what === 'start') {
      const s = snap(Math.max(0, end - max, Math.min(t, end - MIN_LENGTH)))
      onRange([s, end], 'start')
    } else if (what === 'end') {
      const e = snap(Math.min(duration, start + max, Math.max(t, start + MIN_LENGTH)))
      onRange([start, e], 'end')
    } else {
      const s = snap(Math.max(0, Math.min(duration - len, t - offset)))
      onRange([s, s + len], 'both')
    }
  }
  const grab = (what: 'start' | 'end' | 'both') => (e: ReactPointerEvent) => {
    e.stopPropagation()
    e.currentTarget.setPointerCapture(e.pointerId)
    setDrag({ what, offset: timeAt(e.clientX) - start })
  }
  const nudge = (what: 'start' | 'end') => (e: ReactKeyboardEvent) => {
    const d = e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowRight' ? 1 : 0
    if (!d) return
    e.preventDefault()
    e.stopPropagation()
    move(what, (what === 'start' ? start : end) + d * (e.shiftKey ? 1 : frame))
  }

  const tiles = 8
  return (
    <div className="select-none">
      {/* The whole video: where the view and the range are. */}
      <div
        className="relative mb-2 h-2 cursor-pointer rounded-full bg-media-ink/8"
        onPointerDown={(e) => {
          const r = e.currentTarget.getBoundingClientRect()
          const t = ((e.clientX - r.left) / r.width) * duration
          const s = snap(Math.max(0, Math.min(duration - len, t - len / 2)))
          onRange([s, s + len], 'both')
        }}
      >
        <div
          className="absolute inset-y-0 rounded-full bg-media-ink/15"
          style={{ left: `${(view[0] / duration) * 100}%`, width: `${(span / duration) * 100}%` }}
        />
        <div
          className="absolute inset-y-0 min-w-1 rounded-full bg-highlight"
          style={{ left: `${(start / duration) * 100}%`, width: `${(len / duration) * 100}%` }}
        />
      </div>

      <div
        ref={strip}
        className="relative h-16 cursor-crosshair touch-none"
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId)
          onSeek(timeAt(e.clientX))
        }}
        onPointerMove={(e) => {
          if (drag) move(drag.what, timeAt(e.clientX), drag.offset)
          else if (e.buttons === 1) onSeek(timeAt(e.clientX))
        }}
        onPointerUp={() => setDrag(null)}
        onPointerCancel={() => setDrag(null)}
      >
        <Squircle radius={10} style={{ position: 'absolute', inset: 0 }} className="flex overflow-hidden bg-raised">
          {Array.from({ length: tiles }, (_, i) => {
            const t = Math.floor((view[0] + ((i + 0.5) * span) / tiles) / PREVIEW_STEP) * PREVIEW_STEP
            return (
              <img
                key={i}
                src={`${base}/preview/${t}`}
                alt=""
                draggable={false}
                onError={(e) => (e.currentTarget.style.visibility = 'hidden')}
                className="h-full min-w-0 flex-1 object-cover opacity-60"
              />
            )
          })}
        </Squircle>
        {/* Outside the range goes dark. */}
        <div className="pointer-events-none absolute inset-y-0 left-0 rounded-l-[10px] bg-media-shade/55" style={{ width: x(start) }} />
        <div className="pointer-events-none absolute inset-y-0 right-0 rounded-r-[10px] bg-media-shade/55" style={{ left: x(end) }} />
        <div
          className="absolute inset-y-0 cursor-grab rounded-md shadow-[inset_0_0_0_2px_var(--color-highlight)] active:cursor-grabbing"
          style={{ left: x(start), width: `${(len / span) * 100}%` }}
          onPointerDown={grab('both')}
        />
        <Handle at={x(start)} side="start" label="Start of the clip" onPointerDown={grab('start')} onKeyDown={nudge('start')} />
        <Handle at={x(end)} side="end" label="End of the clip" onPointerDown={grab('end')} onKeyDown={nudge('end')} />
        {time >= view[0] && time <= view[1] && (
          <div className="pointer-events-none absolute -inset-y-1 w-0.5 -translate-x-1/2 rounded-full bg-media-ink shadow-[0_0_6px_color-mix(in_srgb,var(--color-media-shade)_60%,transparent)]" style={{ left: x(time) }} />
        )}
      </div>
      <div className="mt-1 flex justify-between text-2xs text-ink-3 tabular">
        <span>{stamp(view[0])}</span>
        <span>{stamp(view[1])}</span>
      </div>
    </div>
  )
}

function Handle({
  at,
  side,
  label,
  onPointerDown,
  onKeyDown,
}: {
  at: string
  side: 'start' | 'end'
  label: string
  onPointerDown: (e: ReactPointerEvent) => void
  onKeyDown: (e: ReactKeyboardEvent) => void
}) {
  return (
    <button
      aria-label={`${label}: arrow keys move it a frame, with Shift a second`}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      className={`absolute -inset-y-1 z-10 w-3.5 cursor-ew-resize rounded-[5px] bg-highlight outline-none focus-visible:ring-2 focus-visible:ring-media-ink ${side === 'start' ? '-translate-x-full' : ''}`}
      style={{ left: at }}
    >
      <span className="absolute inset-y-4 left-1/2 w-0.5 -translate-x-1/2 rounded-full bg-media-shade/40" />
    </button>
  )
}
/** Picks people on this server to send the clip to. */
export function SendTo({ value, onChange }: { value: Person[]; onChange: (p: Person[]) => void }) {
  const { data: people } = usePeople()
  if (people && people.length === 0) return null
  const picked = new Set(value.map((p) => p.id))
  return (
    <div className="flex min-w-0 items-center gap-2">
      <Popover
        side="top"
        align="start"
        trigger={({ toggle, open }) => (
          <Button variant="quiet" onClick={toggle} className={open ? 'bg-float' : ''}>
            <Send className="size-3.5" /> Send to
          </Button>
        )}
      >
        {() => (
          <Panel className="max-h-72 w-64 overflow-y-auto p-1.5">
            {people?.map((p) => (
              <label key={p.id} className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5 text-[13px] hover:bg-hover">
                <Checkbox
                  checked={picked.has(p.id)}
                  onChange={(e) => onChange(e.target.checked ? [...value, p] : value.filter((x) => x.id !== p.id))}
                />
                <Avatar user={p} size={22} />
                <span className="min-w-0 flex-1 truncate">{p.username}</span>
              </label>
            ))}
          </Panel>
        )}
      </Popover>
      <div className="flex min-w-0 -space-x-1.5">
        {value.slice(0, 6).map((p) => (
          <span key={p.id} className="rounded-full ring-2 ring-media-canvas" title={p.username}>
            <Avatar user={p} size={24} />
          </span>
        ))}
      </div>
      {value.length > 0 && (
        <span className="truncate text-xs text-ink-3">{value.length === 1 ? value[0].username : `${value.length} people`}</span>
      )}
    </div>
  )
}
/** Follows the render once the clip is made; there's no need to stay. */
function Rendering({ id, onClose }: { id: number; onClose: () => void }) {
  const navigate = useNavigate()
  const { mutate: markRead } = useMarkRead()
  const { data: clip } = useQuery({ queryKey: ['clip', id], queryFn: () => fetchClip(id) })
  // Watching it finish here is news enough; no need for a notification too.
  useEffect(
    () => onArrival((n) => n.kind === 'CLIP_READY' && n.link === `/clips?clip=${id}` && markRead([n.id])),
    [id, markRead],
  )
  const [copied, setCopied] = useState(false)
  if (!clip) return null
  const done = clip.state === 'READY'
  return (
    <div className="grid flex-1 place-items-center p-6">
      <div className="w-full max-w-md text-center">
        <Squircle radius={18} className="relative mx-auto mb-6 aspect-video w-full overflow-hidden bg-raised">
          {done && clip.poster ? (
            <ClipPlayer src={clip.file} poster={clip.poster} muted />
          ) : (
            <div className="grid size-full place-items-center text-highlight">
              {clip.state === 'FAILED' ? <TriangleAlert className="size-8 text-danger" /> : <Scissors className="size-8 animate-pulse" />}
            </div>
          )}
        </Squircle>
        <p className="text-lg font-semibold tracking-tight">
          {done ? 'Your clip is ready' : clip.state === 'FAILED' ? 'The clip didn’t render' : clip.state === 'QUEUED' ? 'Waiting its turn' : 'Rendering your clip'}
        </p>
        {clip.state === 'FAILED' ? (
          <p className="mt-1.5 text-sm text-ink-2">{clip.error}</p>
        ) : !done ? (
          <>
            <Progress value={clip.state === 'RENDERING' ? (clip.progress ?? 0) : null} className="mx-auto mt-4 w-64" />
            <p className="mt-3 text-[13px] text-ink-3">
              Keep watching if you like; you’ll get a notification when it’s done.
            </p>
          </>
        ) : (
          <p className="mt-1.5 text-sm text-ink-2 tabular">
            {clip.width}×{clip.height} · {Math.round(clip.fps ?? 0)} fps · {bytes(clip.bytes)}
          </p>
        )}
        <div className="mt-6 flex flex-wrap justify-center gap-2">
          {done && (
            <a href={`${clip.file}?download=1`} download>
              <Button variant="primary" size="lg" tabIndex={-1}>
                <Download className="size-4" /> Download
              </Button>
            </a>
          )}
          {done && clip.link && clip.linkLive && (
            <Button
              variant="quiet"
              size="lg"
              onClick={() =>
                void copyLink(clip.link!).then(() => {
                  setCopied(true)
                  toast({ title: 'Link copied', tone: 'ok' })
                })
              }
            >
              {copied ? <Check className="size-4" /> : <Copy className="size-4" />} Copy link
            </Button>
          )}
          <Button variant="quiet" size="lg" onClick={() => void navigate({ to: '/clips', search: { clip: id } })}>
            Your clips
          </Button>
          <Button variant="plain" size="lg" onClick={onClose}>
            {done || clip.state === 'FAILED' ? 'Back to the video' : 'Keep watching'}
          </Button>
        </div>
      </div>
    </div>
  )
}
