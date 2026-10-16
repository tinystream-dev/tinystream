// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, useCanGoBack, useNavigate, useRouter } from '@tanstack/react-router'
import {
  ArrowLeft,
  Camera,
  Captions,
  PictureInPicture2,
  RotateCcw,
  RotateCw,
  Maximize,
  Minimize,
  Pause,
  Play,
  Scissors,
  Settings2,
  SkipBack,
  SkipForward,
  Users,
  Volume1,
  Volume2,
  VolumeX,
} from 'lucide-react'
import { type ReactNode, type RefObject, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { clock, language } from '@tinystream/shared/format'
import { Shortcuts } from '../components/Shell'
import { Squircle } from '../components/Squircle'
import { MenuItem, Panel, Popover, Segmented, Spinner, useTip } from '../components/ui'
import { toastError } from '../components/feedback'
import { graphql } from '../gql'
import { type Chapter, type Clip, type Playback, request } from '../lib/api'
import { fetchClip } from '../lib/clips'
import type { SeriesGlimpse } from '../components/SeriesPanel'
import { airs, episodeCode } from '../lib/downloads'
import { useCanClip, useMe } from '../lib/hooks'
import { ClipEditor } from './ClipEditor'
import { StreamEngine, plan } from './engine'
import { type RoomContext, RoomButton, RoomNotices, TapToJoin, controlledBy, useRoomSnapshot } from './Room'
import { SubtitleRenderer } from './subtitles'
import { canControl, pickShared, roomSubtitle, sameSubtitle, subtitleChoice, type SubtitleChoice } from './sync'

type Quality = 'auto' | 1080 | 720 | 480

const SKIPPABLE = /^(op|opening|intro|ed|ending|credits|preview|next episode|recap)\b/i

function skipLabel(title: string) {
  const t = title.toLowerCase()
  if (/^(op|opening|intro)/.test(t)) return 'Skip opening'
  if (/^(ed|ending|credits)/.test(t)) return 'Skip credits'
  if (/^recap/.test(t)) return 'Skip recap'
  return 'Skip preview'
}

export const pref = {
  get: (k: string) => (typeof localStorage === 'undefined' ? null : localStorage.getItem(`tinystream.${k}`)),
  set: (k: string, v: string) => localStorage.setItem(`tinystream.${k}`, v),
}

function readChoice(k: string): SubtitleChoice | 'off' | null {
  const v = pref.get(k)
  if (!v) return null
  if (v === 'off') return v
  try {
    return JSON.parse(v) as SubtitleChoice
  } catch {
    return null
  }
}

/** The subtitles picked last for this show, or anywhere when it's new. */
function defaultSubtitle(pb: Playback): string | null {
  const usable = pb.media.subtitles.filter((s) => s.supported)
  const choice = readChoice(`subtitles.${pb.title.id}`) ?? readChoice('subtitleTrack')
  if (!usable.length || choice === 'off' || (!choice && pref.get('subtitles') === 'off')) return null
  const lang = pref.get('subtitleLanguage')
  return (
    (choice && sameSubtitle(usable, choice)) ||
    (lang && usable.find((s) => s.language === lang && !s.forced)?.id) ||
    usable.find((s) => s.default)?.id ||
    usable.find((s) => !s.forced)?.id ||
    usable[0].id
  )
}

function trackName(t: { title: string | null; language: string | null; codec: string }, fallback: string) {
  const lang = language(t.language)
  if (t.title && lang && !t.title.toLowerCase().includes(lang.toLowerCase())) return `${lang} · ${t.title}`
  return t.title || lang || fallback
}

/**
 * Plays a video on its own, or, given a room, in step with everyone in it.
 * In a room, the room decides where playback is and what the audio and
 * subtitles are (though anyone can pick their own tracks).
 */
const PlaybackQuery = graphql(`
  query Playback($id: Int!) {
    video(id: $id) {
      ...Playback
    }
    server {
      transcoding {
        ...TranscodingFields
      }
    }
  }
`)

const RoomPlaybackQuery = graphql(`
  query RoomPlayback($code: String!, $id: Int!) {
    room(code: $code) {
      video(id: $id) {
        ...Playback
      }
    }
    server {
      transcoding {
        ...TranscodingFields
      }
    }
  }
`)

const SaveProgress = graphql(`
  mutation SaveProgress($videoId: Int!, $position: Float!, $duration: Float!) {
    saveProgress(videoId: $videoId, position: $position, duration: $duration) {
      id
    }
  }
`)

const TakeScreenshot = graphql(`
  mutation TakeScreenshot($input: NewScreenshot!) {
    takeScreenshot(input: $input) {
      ...ClipFields
    }
  }
`)

const StartRoom = graphql(`
  mutation StartRoom($input: NewRoom!) {
    startRoom(input: $input) {
      code
    }
  }
`)

const ScheduleQuery = graphql(`
  query PlayerSchedule($id: Int!) {
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

const OverviewQuery = graphql(`
  query PlayerOverview($id: Int!, $videoId: Int!) {
    title(id: $id) {
      overview
    }
    video(id: $videoId) {
      overview
    }
  }
`)

/** What the player needs about a video, alone or through a room. */
async function playback(mediaId: number, room: string | null): Promise<Playback> {
  const r = room
    ? await request(RoomPlaybackQuery, { code: room, id: mediaId }).then((r) => ({ video: r.room.video, server: r.server }))
    : await request(PlaybackQuery, { id: mediaId })
  if (!r.video) throw new Error('This video isn’t here anymore.')
  return { ...r.video, transcoding: r.server.transcoding }
}

export function Player({ mediaId, room, editClip, startAt }: { mediaId: number; room?: RoomContext; editClip?: number; startAt?: number }) {
  const navigate = useNavigate()
  const router = useRouter()
  const canGoBack = useCanGoBack()
  const qc = useQueryClient()
  const me = useMe()
  // Rooms have their own copies of the media endpoints, so guests can play.
  const apiBase = room ? `/together/${room.info.code}` : ''
  const mediaBase = `/api${apiBase}/media/${mediaId}`
  const { data: pb, error: infoError } = useQuery({
    queryKey: ['playback', room?.info.code ?? null, mediaId],
    queryFn: () => playback(mediaId, room?.info.code ?? null),
    staleTime: Infinity,
    gcTime: 0,
  })
  const snap = useRoomSnapshot(room)
  const rs = snap?.room ?? null
  /** Whether we may play, pause, seek and change episodes. */
  const controls = !room || (!!rs && canControl(rs, snap!.you))
  const controller = controlledBy(rs, snap?.you ?? null)
  const signedIn = !room || room.info.signedIn

  const stage = useRef<HTMLDivElement>(null)
  const videoRef = useRef<HTMLVideoElement>(null)
  const engine = useRef<StreamEngine | null>(null)
  const subs = useRef<SubtitleRenderer | null>(null)

  const [playing, setPlaying] = useState(false)
  const [time, setTime] = useState(0)
  const [buffered, setBuffered] = useState<[number, number][]>([])
  const [waiting, setWaiting] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [volume, setVolume] = useState(() => Number(pref.get('volume') ?? 1))
  const [muted, setMuted] = useState(false)
  const [fullscreen, setFullscreen] = useState(false)
  const [idle, setIdle] = useState(false)
  // undefined until the initial tracks are picked, so the stream is built once.
  const [audioIndex, setAudioIndex] = useState<number | null | undefined>(undefined)
  const [subtitle, setSubtitle] = useState<string | null>(null)
  // In a room: whether we picked our own tracks rather than the room's, and
  // whether picking changes them for everyone or just us.
  const [ownAudio, setOwnAudio] = useState(false)
  const [ownSubtitle, setOwnSubtitle] = useState(false)
  const [trackScope, setTrackScope] = useState<'everyone' | 'me'>('everyone')
  const scope = room && controls ? trackScope : 'me'
  const [quality, setQuality] = useState<Quality>('auto')
  const [ended, setEnded] = useState(false)
  const [countdown, setCountdown] = useState<number | null>(null)
  const [rate, setRate] = useState(() => Number(pref.get('rate') ?? 1))
  const [hud, setHud] = useState<Hud | null>(null)
  /** Whether the video has shown a frame yet. */
  const [started, setStarted] = useState(false)
  const [help, setHelp] = useState(false)
  /** Making a clip: where playback was when it started. */
  const [clipping, setClipping] = useState<{ at: number; editing?: Clip } | null>(null)
  /** The screenshot just taken: being taken, saved, or why it wasn't. */
  const [shot, setShot] = useState<Shot | null>(null)
  /** The room's sound steps back while a clip's preview plays over it. */
  const [ducked, setDucked] = useState(false)
  const canClip = useCanClip() && signedIn
  const hudTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  /** Briefly shows what a key or gesture just did. Repeated seeks add up. */
  const flash = useCallback((h: Omit<Hud, 'key'>) => {
    clearTimeout(hudTimer.current)
    setHud((prev) => {
      const add = h.kind === 'seek' && prev?.kind === 'seek' && Math.sign(prev.amount ?? 0) === Math.sign(h.amount ?? 0)
      return { ...h, amount: add ? (prev!.amount ?? 0) + (h.amount ?? 0) : h.amount, key: (prev?.key ?? 0) + 1 }
    })
    hudTimer.current = setTimeout(() => setHud(null), 700)
  }, [])
  useEffect(() => () => clearTimeout(hudTimer.current), [])

  const duration = pb?.media.duration ?? 0
  const streamPlan = useMemo(
    () => (pb && audioIndex !== undefined ? plan(pb, audioIndex, quality) : null),
    [pb, audioIndex, quality],
  )

  // Pick initial tracks once the file's info arrives. (Rooms pick below.)
  useEffect(() => {
    if (!pb || room) return
    const lang = pref.get('audioLanguage')
    const a = (lang && pb.media.audio.find((t) => t.language === lang)) || pb.media.audio.find((t) => t.default)
    setAudioIndex(a?.index ?? null)
    setSubtitle(defaultSubtitle(pb))
  }, [pb, room])

  // In a room, follow its tracks unless we've picked our own.
  const shared = room && pb && rs ? pickShared(pb, rs.tracks) : null
  const hasShared = !!shared
  const sharedAudio = shared?.audio
  const sharedSubtitle = shared?.subtitle
  useEffect(() => {
    if (!hasShared) return
    if (!ownAudio) setAudioIndex(sharedAudio ?? null)
    if (!ownSubtitle) setSubtitle(sharedSubtitle ?? null)
  }, [hasShared, sharedAudio, sharedSubtitle, ownAudio, ownSubtitle])

  /** Changes the room's tracks for everyone. */
  const shareTracks = (change: { audio?: number | null; subtitle?: string | null }) => {
    if (!room || !pb || !rs || !shared) return
    const audio = change.audio !== undefined ? change.audio : shared.audio
    const sub = change.subtitle !== undefined ? change.subtitle : shared.subtitle
    const track = pb.media.subtitles.find((s) => s.id === sub)
    room.client.setTracks({
      // Kept while off, so turning them back on finds the same track.
      ...rs.tracks,
      ...(track && roomSubtitle(track)),
      mediaId: pb.id,
      audio,
      audioLanguage: pb.media.audio.find((a) => a.index === audio)?.language ?? null,
      subtitle: sub,
    })
  }
  const chooseAudio = (index: number) => {
    resumeAt.current = videoRef.current?.currentTime ?? null
    setAudioIndex(index)
    const lang = pb?.media.audio.find((a) => a.index === index)?.language
    if (lang) pref.set('audioLanguage', lang)
    if (room) {
      setOwnAudio(scope === 'me')
      if (scope === 'everyone') shareTracks({ audio: index })
    }
  }
  const chooseSubtitle = (id: string | null) => {
    setSubtitle(id)
    pref.set('subtitles', id ? 'on' : 'off')
    const track = id ? pb?.media.subtitles.find((s) => s.id === id) : null
    if (track?.language) pref.set('subtitleLanguage', track.language)
    if (pb) {
      const choice = track ? JSON.stringify(subtitleChoice(track)) : 'off'
      pref.set(`subtitles.${pb.title.id}`, choice)
      if (track) pref.set('subtitleTrack', choice)
    }
    if (room) {
      setOwnSubtitle(scope === 'me')
      if (scope === 'everyone') shareTracks({ subtitle: id })
    }
  }

  // (Re)build the stream whenever the plan changes, keeping our place.
  const resumeAt = useRef<number | null>(startAt ?? null)
  const resumePaused = useRef(false)
  useEffect(() => {
    const video = videoRef.current
    if (!pb || !streamPlan || !video) return
    // In a room, start wherever the room is; it takes over from there.
    const start = room ? room.client.target() : (resumeAt.current ?? (!pb.finished && pb.position && pb.position > 5 ? pb.position : 0))
    const wasPaused = !!room || (resumeAt.current !== null && resumePaused.current)
    setError(null)
    setWaiting(true)
    const e = new StreamEngine(video, `/api${apiBase}/media/${pb.id}`, pb.media.duration, streamPlan, (ev) => {
      if (ev.error) setError(ev.error)
    })
    engine.current = e
    void e.start(start).then(() => {
      if (!wasPaused) video.play().catch(() => setPlaying(false))
    })
    return () => {
      // Until the stream has loaded, the video's own time and paused state
      // don't reflect where we meant to be, so carry the intent over instead.
      const loaded = video.readyState > 0
      resumeAt.current = loaded ? video.currentTime : start
      resumePaused.current = loaded ? video.paused : wasPaused
      e.destroy()
    }
  }, [pb, streamPlan, room, apiBase])

  // Hand the video to the room to steer.
  useEffect(() => {
    const video = videoRef.current
    if (!room || !pb || !video) return
    room.client.attach(video, pb.id, pb.media.duration ?? 0)
    return () => room.client.detach()
  }, [room, pb])

  useEffect(() => {
    const video = videoRef.current
    if (!pb || !video) return
    const base = `/api${apiBase}/media/${pb.id}`
    const fonts = pb.media.fonts.map((f) => `${base}/fonts/${f.index}`)
    const r = new SubtitleRenderer(video, base, fonts)
    subs.current = r
    return () => r.destroy()
  }, [pb, apiBase])

  useEffect(() => {
    void subs.current?.show(subtitle).catch((e) => console.warn('subtitles', e))
  }, [subtitle, pb])

  // Save progress regularly, when pausing, and when leaving.
  const save = useCallback(() => {
    const v = videoRef.current
    // Guests have nowhere to keep their place.
    if (!v || !duration || v.currentTime < 1 || !signedIn) return
    void request(SaveProgress, { videoId: mediaId, position: v.currentTime, duration })
      .then(() => {
        // So pages behind the player show where we are now.
        void qc.invalidateQueries({ queryKey: ['home'] })
        void qc.invalidateQueries({ queryKey: ['item', pb?.title.id] })
        void qc.invalidateQueries({ queryKey: ['library'] })
      })
      .catch(() => {})
  }, [mediaId, duration, qc, pb?.title.id, signedIn])
  useEffect(() => {
    const id = setInterval(() => !videoRef.current?.paused && save(), 10_000)
    window.addEventListener('pagehide', save)
    return () => {
      clearInterval(id)
      window.removeEventListener('pagehide', save)
      save()
    }
  }, [save])

  // Hide controls and cursor when idle during playback.
  const idleTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const poke = useCallback(() => {
    setIdle(false)
    clearTimeout(idleTimer.current)
    idleTimer.current = setTimeout(() => setIdle(true), 2600)
  }, [])
  useEffect(() => () => clearTimeout(idleTimer.current), [])

  /** Plays or pauses (toggles without `play`); returns whether it's now playing. */
  const setPlayback = useCallback(
    (play?: boolean): boolean | undefined => {
      const v = videoRef.current
      if (!v) return
      if (room) {
        const s = room.client.getSnapshot()
        if (!s.room || !canControl(s.room, s.you)) return
        const want = play ?? s.room.clock.paused
        if (want) room.client.play(v.currentTime)
        else room.client.pause(v.currentTime)
        return want
      }
      const want = play ?? v.paused
      if (want) void v.play()
      else v.pause()
      return want
    },
    [room],
  )
  const togglePlay = useCallback(() => setPlayback(), [setPlayback])
  const seek = useCallback(
    (t: number) => {
      const v = videoRef.current
      if (!v) return
      const to = Math.max(0, Math.min(duration || Infinity, t))
      if (room) {
        const s = room.client.getSnapshot()
        if (!s.room || !canControl(s.room, s.you)) return
        room.client.seek(to)
      }
      v.currentTime = to
      setTime(to)
      poke()
    },
    [duration, poke, room],
  )
  const seekBy = useCallback(
    (delta: number) => {
      const v = videoRef.current
      if (!v) return
      seek(v.currentTime + delta)
      flash({ kind: 'seek', amount: delta })
    },
    [seek, flash],
  )
  const togglePip = useCallback(() => {
    const v = videoRef.current
    if (!v || !document.pictureInPictureEnabled) return
    if (document.pictureInPictureElement) void document.exitPictureInPicture()
    else void v.requestPictureInPicture().catch(() => {})
  }, [])
  const changeRate = useCallback(
    (r: number) => {
      const next = Math.max(0.25, Math.min(3, Math.round(r * 100) / 100))
      if (room) {
        const s = room.client.getSnapshot()
        if (!s.room || !canControl(s.room, s.you)) return
        room.client.setRate(next)
      } else {
        setRate(next)
        pref.set('rate', String(next))
      }
      flash({ kind: 'speed', text: `${next}×` })
    },
    [flash, room],
  )
  const toggleFullscreen = useCallback(() => {
    if (document.fullscreenElement) void document.exitFullscreen()
    else void stage.current?.requestFullscreen()
  }, [])

  const goTo = useCallback(
    (id: number) => {
      save()
      resumeAt.current = null
      // Everyone moves together; the room puts the new episode on.
      if (room) room.client.changeMedia(id, mediaId)
      else void navigate({ to: '/watch/$id', params: { id: String(id) }, replace: true })
    },
    [navigate, save, room, mediaId],
  )
  // Guests have nowhere to go back to.
  const canLeave = signedIn
  const back = useCallback(() => {
    if (!canLeave) return
    save()
    if (document.fullscreenElement) void document.exitFullscreen()
    // Return to wherever playback was started from; episode changes replace
    // history, so this skips over them.
    if (canGoBack) router.history.back()
    else if (room) void navigate({ to: '/' })
    else void navigate({ to: '/title/$id', params: { id: String(pb?.title.id ?? '') } })
  }, [navigate, router, canGoBack, pb, save, room, canLeave])

  /** Opens the clip editor on what's playing (a room plays on meanwhile). */
  const startClip = useCallback(() => {
    const v = videoRef.current
    if (!pb || !v) return
    if (!room) setPlayback(false)
    setClipping({ at: v.currentTime })
  }, [pb, room, setPlayback])
  const closeClip = useCallback(() => {
    setClipping(null)
    setDucked(false)
  }, [])
  /** Screenshots what's on screen (with the subtitles showing), and plays on. */
  const shooting = useRef(false)
  const takeScreenshot = useCallback(async () => {
    const v = videoRef.current
    if (!pb || !v || shooting.current) return
    shooting.current = true
    const key = Date.now()
    setShot({ key })
    try {
      const clip = (await request(TakeScreenshot, { input: { videoId: pb.id, at: v.currentTime, subtitles: subtitle, room: room?.info.code } })).takeScreenshot
      setShot((s) => (s?.key === key ? { key, clip } : s))
      void qc.invalidateQueries({ queryKey: ['clips'] })
    } catch (e) {
      setShot((s) => (s?.key === key ? { key, error: (e as Error)?.message ?? String(e) } : s))
    } finally {
      shooting.current = false
    }
  }, [pb, subtitle, room, qc])
  useEffect(() => {
    if (!shot?.clip && !shot?.error) return
    const t = setTimeout(() => setShot(null), shot.error ? 6000 : 4000)
    return () => clearTimeout(t)
  }, [shot])

  // Opened to change a clip: straight into the editor with it.
  useEffect(() => {
    if (!editClip || !pb) return
    fetchClip(editClip)
      .then((c) => {
        if (!c) throw new Error('That clip isn’t here anymore.')
        setClipping({ at: c.start, editing: c })
      })
      .catch(toastError)
  }, [editClip, pb])

  /** Moves this video into a new room, right where it is. */
  const [starting, setStarting] = useState(false)
  const startTogether = async () => {
    const v = videoRef.current
    if (!pb || !v || starting) return
    setStarting(true)
    try {
      const { code } = (
        await request(StartRoom, {
          input: {
            videoId: mediaId,
            position: v.currentTime,
            paused: v.paused,
            tracks: {
              audio: audioIndex ?? null,
              audioLanguage: pb.media.audio.find((a) => a.index === audioIndex)?.language ?? null,
              subtitle,
              ...roomSubtitle(pb.media.subtitles.find((s) => s.id === subtitle)),
            },
          },
        })
      ).startRoom
      save()
      void navigate({ to: '/together/$code', params: { code }, replace: true })
    } catch (e) {
      toastError(e)
      setStarting(false)
    }
  }

  const chapter: Chapter | undefined = pb?.media.chapters.find((c) => time >= c.start && time < c.end)
  const skippable = chapter?.title && SKIPPABLE.test(chapter.title) && chapter.end - time > 3 ? chapter : null
  const nearEnd =
    !!pb?.next && duration > 0 && (time > duration - 25 || (!!chapter?.title && /^(ed|ending|credits)/i.test(chapter.title)))

  // Autoplay the next episode a few seconds after this one ends.
  useEffect(() => {
    if (!ended || !pb?.next) return
    setCountdown(5)
    const id = setInterval(() => setCountdown((c) => (c === null ? null : c - 1)), 1000)
    return () => clearInterval(id)
  }, [ended, pb])
  useEffect(() => {
    if (countdown !== null && countdown <= 0 && pb?.next && controls) goTo(pb.next.id)
  }, [countdown, pb, goTo, controls])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (help || clipping || e.target instanceof HTMLInputElement || e.metaKey || e.ctrlKey || e.altKey) return
      const v = videoRef.current
      if (!v) return
      const k = e.key.toLowerCase()
      const handled = () => {
        e.preventDefault()
        poke()
      }
      if (k === ' ' || k === 'k') (handled(), flash({ kind: togglePlay() ? 'play' : 'pause' }))
      else if (k === 'arrowleft') (handled(), seekBy(-5))
      else if (k === 'arrowright') (handled(), seekBy(5))
      else if (k === 'j') (handled(), seekBy(-10))
      else if (k === 'l') (handled(), seekBy(10))
      else if (k === 'arrowup') {
        handled()
        const x = Math.min(1, v.volume + 0.05)
        setVolume(x)
        setMuted(false)
        flash({ kind: 'volume', amount: x })
      } else if (k === 'arrowdown') {
        handled()
        const x = Math.max(0, v.volume - 0.05)
        setVolume(x)
        flash({ kind: 'volume', amount: x })
      } else if (k === 'm') (handled(), setMuted((x) => !x), flash({ kind: 'volume', amount: v.muted ? v.volume : 0 }))
      else if (e.key === '?') (handled(), setHelp((h) => !h))
      else if (e.key === '>') (handled(), changeRate(v.playbackRate + 0.25))
      else if (e.key === '<') (handled(), changeRate(v.playbackRate - 0.25))
      else if (k === 'p') (handled(), togglePip())
      else if (k === 'x' && canClip) (handled(), startClip())
      else if (e.key === 'S' && canClip) (handled(), void takeScreenshot())
      else if (k === 'f') (handled(), toggleFullscreen())
      else if (k === 'n' && pb?.next) (handled(), goTo(pb.next.id))
      else if (k === 's' && skippable) (handled(), seek(skippable.end))
      else if (k === 'c' && pb) {
        handled()
        const usable = pb.media.subtitles.filter((s) => s.supported)
        const i = usable.findIndex((s) => s.id === subtitle)
        chooseSubtitle(i + 1 < usable.length ? usable[i + 1].id : null)
      } else if (k === 'escape' && !document.fullscreenElement) back()
      else if (/^[0-9]$/.test(k) && duration) (handled(), seek((duration * Number(k)) / 10))
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // chooseSubtitle is rebuilt every render; what it reads is listed here.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [help, clipping, canClip, takeScreenshot, togglePlay, seek, seekBy, flash, changeRate, togglePip, toggleFullscreen, goTo, back, pb, skippable, subtitle, duration, poke, scope, shared?.audio, shared?.subtitle])

  // In a room, the room sets the pace (and bends it a little to keep us in step).
  const shownRate = room ? (rs?.clock.rate ?? 1) : rate
  useEffect(() => {
    if (videoRef.current && !room) videoRef.current.playbackRate = rate
  }, [rate, streamPlan, room])

  // The OS's media controls: keyboard media keys, lock screen, headphones.
  useEffect(() => {
    if (!pb || !('mediaSession' in navigator)) return
    const ms = navigator.mediaSession
    ms.metadata = new MediaMetadata({
      title: pb.title.kind === 'SHOW' ? [pb.label, pb.name].filter(Boolean).join(' · ') || pb.title.name : pb.title.name,
      artist: pb.title.kind === 'SHOW' ? pb.title.name : '',
      artwork: [
        ...(pb.title.kind === 'SHOW' ? [{ src: pb.still, sizes: '640x360' }] : []),
        ...(pb.title.backdrop ? [{ src: pb.title.backdrop, sizes: '1280x720' }] : []),
      ],
    })
    const set = (a: MediaSessionAction, h: MediaSessionActionHandler | null) => {
      try {
        ms.setActionHandler(a, h)
      } catch {}
    }
    set('play', () => void setPlayback(true))
    set('pause', () => void setPlayback(false))
    set('seekbackward', (d) => seekBy(-(d.seekOffset ?? 10)))
    set('seekforward', (d) => seekBy(d.seekOffset ?? 10))
    set('seekto', (d) => d.seekTime != null && seek(d.seekTime))
    set('nexttrack', pb.next ? () => goTo(pb.next!.id) : null)
    set('previoustrack', pb.previous ? () => goTo(pb.previous!.id) : null)
    return () => {
      for (const a of ['play', 'pause', 'seekbackward', 'seekforward', 'seekto', 'nexttrack', 'previoustrack'] as const) set(a, null)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pb, seek, seekBy, goTo, setPlayback])
  useEffect(() => {
    if (!duration || !('mediaSession' in navigator) || !navigator.mediaSession.setPositionState) return
    try {
      navigator.mediaSession.setPositionState({ duration, position: Math.min(time, duration), playbackRate: shownRate })
    } catch {}
    // Once every few seconds is plenty; the OS extrapolates in between.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [Math.floor(time / 5), duration, shownRate])

  // What's next once the last episode ends.
  const { data: seriesInfo } = useQuery({
    queryKey: ['series', 'item', pb?.title.id, 'glimpse'],
    queryFn: async (): Promise<SeriesGlimpse | null> => (await request(ScheduleQuery, { id: pb!.title.id })).title?.series ?? null,
    enabled: !!pb && pb.title.kind === 'SHOW' && !pb.next && ended && signedIn,
  })

  // Touch: tap shows or hides the controls; double-tap a side to skip.
  const lastTap = useRef<{ t: number; x: number } | null>(null)
  const tapTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const onTouchTap = (x: number, width: number) => {
    const now = Date.now()
    const prev = lastTap.current
    const side = x < width / 3 ? -1 : x > (width * 2) / 3 ? 1 : 0
    if (prev && now - prev.t < 300 && side !== 0) {
      clearTimeout(tapTimer.current)
      lastTap.current = { t: now, x }
      seekBy(side * 10)
      return
    }
    lastTap.current = { t: now, x }
    clearTimeout(tapTimer.current)
    tapTimer.current = setTimeout(() => {
      if (!idle && playing) {
        clearTimeout(idleTimer.current)
        setIdle(true)
      } else poke()
    }, 260)
  }
  const pointerType = useRef('mouse')

  useEffect(() => {
    const v = videoRef.current
    if (!v) return
    v.volume = volume
    v.muted = muted || ducked
    pref.set('volume', String(volume))
  }, [volume, muted, ducked])

  useEffect(() => {
    const onFs = () => setFullscreen(!!document.fullscreenElement)
    document.addEventListener('fullscreenchange', onFs)
    return () => document.removeEventListener('fullscreenchange', onFs)
  }, [])

  // Paused and left alone for a while: the frame recedes and what's playing steps forward.
  const resting = !playing && idle && started && !ended && !waiting && !error && !!pb && !snap?.blocked
  const [rested, setRested] = useState(false)
  useEffect(() => {
    setRested(false)
    if (!resting) return
    const t = setTimeout(() => setRested(true), 2400)
    return () => clearTimeout(t)
  }, [resting])
  const { data: details } = useQuery({
    queryKey: ['overview', pb?.title.id, mediaId],
    queryFn: () => request(OverviewQuery, { id: pb!.title.id, videoId: mediaId }),
    enabled: resting && !room && !!pb,
  })
  const overview = details?.video?.overview ?? details?.title?.overview ?? null

  const showChrome = ((!playing && !rested) || !idle || !!error) && !clipping
  const title = pb?.title.name ?? ''
  const subtitleLine = pb?.title.kind === 'SHOW' ? [pb.label, pb.name].filter(Boolean).join(' · ') : null

  return (
    <div
      ref={stage}
      onPointerMove={(e) => e.pointerType === 'mouse' && poke()}
      onPointerDown={(e) => {
        pointerType.current = e.pointerType
        if (e.pointerType === 'mouse') poke()
      }}
      className={`fixed inset-0 bg-media-shade select-none ${showChrome || clipping ? '' : 'cursor-none'}`}
    >
      <Ambilight video={videoRef} />
      <video
        ref={videoRef}
        className={
          clipping && room
            ? 'absolute top-5 right-5 z-30 aspect-video w-64 rounded-xl bg-media-shade object-cover shadow-[0_0_0_1px_color-mix(in_srgb,var(--color-media-ink)_12%,transparent),0_12px_32px_color-mix(in_srgb,var(--color-media-shade)_60%,transparent)] transition-all duration-300 md:w-80'
            : 'recede absolute inset-0 size-full'
        }
        data-receded={rested || undefined}
        playsInline
        onClick={(e) => {
          if (pointerType.current === 'touch') onTouchTap(e.nativeEvent.offsetX, e.currentTarget.clientWidth)
          else {
            const on = togglePlay()
            if (on !== undefined) flash({ kind: on ? 'play' : 'pause' })
          }
        }}
        onDoubleClick={() => pointerType.current !== 'touch' && toggleFullscreen()}
        onPlay={() => (setPlaying(true), setEnded(false), setCountdown(null))}
        onPause={() => (setPlaying(false), save())}
        onWaiting={() => setWaiting(true)}
        onPlaying={() => (setWaiting(false), setStarted(true))}
        onCanPlay={() => (setWaiting(false), setStarted(true))}
        onStalled={() => engine.current?.recover()}
        onEnded={() => (setEnded(true), save())}
        onRateChange={(e) => !room && setRate(e.currentTarget.playbackRate)}
        onTimeUpdate={(e) => {
          const v = e.currentTarget
          setTime(v.currentTime)
          const b = v.buffered
          const r: [number, number][] = []
          for (let i = 0; i < b.length; i++) r.push([b.start(i), b.end(i)])
          setBuffered(r)
        }}
        onError={() => setError('the browser could not play this stream')}
      />

      {/* The still holds the frame until the video has one of its own; the one clicked grows into it. */}
      <img
        src={pb?.still}
        alt=""
        onError={(e) => (e.currentTarget.style.display = 'none')}
        style={{ viewTransitionName: 'still' }}
        className={`pointer-events-none absolute inset-0 size-full object-contain transition-opacity duration-700 ${started ? 'opacity-0' : ''}`}
      />

      {pb && <Resting pb={pb} line={subtitleLine} overview={overview} time={time} duration={duration} shown={rested} />}

      {(waiting || !pb) && !error && (
        <div className="pointer-events-none absolute inset-0 grid place-items-center text-media-ink/80">
          <Spinner className="size-9" />
        </div>
      )}

      {hud && <HudView hud={hud} />}
      {shot && <ShotCard shot={shot} onClose={() => setShot(null)} />}
      {room && <RoomNotices client={room.client} />}
      {room && snap?.blocked && <TapToJoin client={room.client} title={[pb?.title.name, pb?.label].filter(Boolean).join(' · ')} />}

      {(error || infoError) && (
        <div className="absolute inset-0 grid place-items-center p-6">
          <Panel className="max-w-md p-5">
            <p className="text-[15px] font-medium">This video can't play right now</p>
            <p className="mt-1.5 text-sm leading-relaxed text-ink-2">{error ?? (infoError as Error).message}</p>
            <div className="mt-4 flex gap-2">
              <button className="text-sm text-ink underline-offset-4 hover:underline" onClick={() => location.reload()}>
                Try again
              </button>
              <span className="text-ink-3">or</span>
              <button className="text-sm text-ink underline-offset-4 hover:underline" onClick={() => setQuality(720)}>
                convert it to 720p
              </button>
            </div>
          </Panel>
        </div>
      )}

      <div
        className={`absolute inset-x-0 top-0 flex items-start gap-3 bg-linear-to-b from-media-shade/70 to-transparent px-5 pt-4 pb-16 transition-opacity duration-300 ${showChrome ? 'opacity-100' : 'pointer-events-none opacity-0'}`}
      >
        {canLeave && (
          <ChromeButton label="Back" onClick={back}>
            <ArrowLeft className="size-5" />
          </ChromeButton>
        )}
        <div className="min-w-0 flex-1 pt-1">
          <p className="truncate text-[15px] font-medium text-media-ink">{title}</p>
          {subtitleLine && <p className="truncate text-[13px] text-media-ink/60">{subtitleLine}</p>}
        </div>
        {room && <RoomButton ctx={room} />}
      </div>

      <div className="absolute right-6 bottom-28 flex flex-col items-end gap-3">
        {skippable && !nearEnd && controls && (
          <SkipPill onClick={() => seek(skippable.end)}>{skipLabel(skippable.title!)}</SkipPill>
        )}
        {pb?.next && (nearEnd || ended) && (
          <UpNext
            next={pb.next}
            still={pb.next.still}
            countdown={controls ? countdown : null}
            onPlay={controls ? () => goTo(pb.next!.id) : undefined}
          />
        )}
      </div>

      {/* The last episode there is, for now. */}
      {ended && pb && pb.title.kind === 'SHOW' && !pb.next && (
        <Finale pb={pb} series={seriesInfo ?? null} onBack={canLeave ? back : undefined} />
      )}

      <div
        className={`absolute inset-x-0 bottom-0 bg-linear-to-t from-media-shade/80 via-media-shade/40 to-transparent px-5 pt-20 pb-4 transition-opacity duration-300 ${showChrome ? 'opacity-100' : 'pointer-events-none opacity-0'}`}
      >
        <Timeline
          base={mediaBase}
          time={time}
          duration={duration}
          buffered={buffered}
          chapters={pb?.media.chapters ?? []}
          onSeek={seek}
        />
        <div className="mt-2 flex items-center gap-1">
          <ChromeButton label={playing ? 'Pause (k)' : 'Play (k)'} onClick={togglePlay} disabled={!controls}>
            {playing ? <Pause className="size-5 fill-current" /> : <Play className="size-5 fill-current" />}
          </ChromeButton>
          {pb?.previous && controls && (
            <ChromeButton label={`Previous: ${pb.previous.label ?? ''}`} onClick={() => goTo(pb.previous!.id)}>
              <SkipBack className="size-4.5 fill-current" />
            </ChromeButton>
          )}
          {pb?.next && controls && (
            <ChromeButton label={`Next: ${pb.next.label ?? ''} (n)`} onClick={() => goTo(pb.next!.id)}>
              <SkipForward className="size-4.5 fill-current" />
            </ChromeButton>
          )}
          <VolumeControl volume={volume} muted={muted} setVolume={setVolume} setMuted={setMuted} />
          <span className="ml-2 text-[13px] text-media-ink/80 tabular">
            {clock(time)} <span className="text-media-ink/40">/ {clock(duration)}</span>
          </span>
          {chapter?.title && <span className="ml-3 hidden truncate text-[13px] text-media-ink/50 sm:inline">{chapter.title}</span>}
          {shownRate !== 1 && <span className="ml-3 rounded-md bg-media-ink/10 px-1.5 py-0.5 text-2xs text-media-ink/80 tabular">{shownRate}×</span>}
          {controller && <span className="ml-3 hidden truncate text-[13px] text-media-ink/50 sm:inline">{controller} is in control</span>}
          <div className="flex-1" />

          {pb && (
            <Popover
              side="top"
              trigger={({ toggle, open }) => (
                <ChromeButton label="Audio & subtitles (c)" onClick={toggle} active={open}>
                  <Captions className="size-5" />
                </ChromeButton>
              )}
            >
              {(close) => (
                <Panel className="w-[34rem] max-w-[calc(100vw-2.5rem)] p-1.5">
                  {room && (
                    <div className="flex items-center gap-3 px-1.5 pt-1 pb-2">
                      {controls ? (
                        <Segmented
                          size="sm"
                          value={trackScope}
                          onChange={setTrackScope}
                          options={[
                            { value: 'everyone', label: 'For everyone' },
                            { value: 'me', label: 'Just for me' },
                          ]}
                        />
                      ) : (
                        <p className="text-xs text-ink-3">Just for you</p>
                      )}
                    </div>
                  )}
                  <div className="flex gap-1">
                  <div className="min-w-0 flex-1">
                    <TrackHeading label="Audio" own={!!room && ownAudio} onFollow={() => setOwnAudio(false)} />
                    {pb.media.audio.map((a) => (
                      <MenuItem
                        key={a.index}
                        active={a.index === audioIndex}
                        hint={a.channels > 2 ? `${a.channels} ch` : undefined}
                        onClick={() => {
                          chooseAudio(a.index)
                          close()
                        }}
                      >
                        {trackName(a, `Track ${a.index}`)}
                      </MenuItem>
                    ))}
                  </div>
                  <div className="min-w-0 flex-1">
                    <TrackHeading label="Subtitles" own={!!room && ownSubtitle} onFollow={() => setOwnSubtitle(false)} />
                    <MenuItem
                      active={subtitle === null}
                      onClick={() => {
                        chooseSubtitle(null)
                        close()
                      }}
                    >
                      Off
                    </MenuItem>
                    {pb.media.subtitles.map((s) => (
                      <MenuItem
                        key={s.id}
                        active={s.id === subtitle}
                        hint={!s.supported ? 'image' : s.forced ? 'forced' : undefined}
                        onClick={() => {
                          if (!s.supported) return
                          chooseSubtitle(s.id)
                          close()
                        }}
                      >
                        {trackName(s, s.id.startsWith('x') ? 'External' : `Track ${s.id.slice(1)}`)}
                      </MenuItem>
                    ))}
                  </div>
                  </div>
                </Panel>
              )}
            </Popover>
          )}

          {pb && streamPlan && (
            <Popover
              side="top"
              trigger={({ toggle, open }) => (
                <ChromeButton label="Quality" onClick={toggle} active={open}>
                  <Settings2 className="size-5" />
                </ChromeButton>
              )}
            >
              {(close) => (
                <Panel className="w-64 p-1.5">
                  <p className="px-2.5 pt-1.5 pb-1 text-xs text-ink-3">Speed</p>
                  <div className="mb-1.5 flex gap-0.5 px-1">
                    {[0.75, 1, 1.25, 1.5, 2].map((r) => (
                      <button
                        key={r}
                        onClick={() => changeRate(r)}
                        disabled={!controls}
                        className={`h-7 flex-1 rounded-lg text-xs tabular transition-colors disabled:opacity-40 ${shownRate === r ? 'bg-ink text-canvas' : 'text-ink-2 hover:bg-hover'}`}
                      >
                        {r}×
                      </button>
                    ))}
                  </div>
                  <div className="my-1 h-px bg-line" />
                  <p className="px-2.5 pt-1.5 pb-2 text-xs leading-relaxed text-ink-3">{streamPlan.describe}</p>
                  {(['auto', 1080, 720, 480] as Quality[]).map((q) => (
                    <MenuItem
                      key={q}
                      active={quality === q}
                      hint={q === 'auto' ? 'best for this browser' : undefined}
                      onClick={() => {
                        resumeAt.current = videoRef.current?.currentTime ?? null
                        setQuality(q)
                        close()
                      }}
                    >
                      {q === 'auto' ? 'Automatic' : `${q}p`}
                    </MenuItem>
                  ))}
                </Panel>
              )}
            </Popover>
          )}

          {canClip && pb && (
            <ChromeButton label="Screenshot (shift+s)" onClick={() => void takeScreenshot()}>
              <Camera className="size-5" />
            </ChromeButton>
          )}
          {canClip && pb && (
            <ChromeButton label="Clip (x)" onClick={startClip}>
              <Scissors className="size-5" />
            </ChromeButton>
          )}
          {!room && me?.permissions.watchTogether && pb && (
            <ChromeButton label="Watch together" onClick={() => void startTogether()} disabled={starting}>
              <Users className="size-5" />
            </ChromeButton>
          )}
          {typeof document !== 'undefined' && document.pictureInPictureEnabled && (
            <ChromeButton label="Picture in picture (p)" onClick={togglePip}>
              <PictureInPicture2 className="size-5" />
            </ChromeButton>
          )}
          <ChromeButton label={fullscreen ? 'Exit full screen (f)' : 'Full screen (f)'} onClick={toggleFullscreen}>
            {fullscreen ? <Minimize className="size-5" /> : <Maximize className="size-5" />}
          </ChromeButton>
        </div>
      </div>
      {help && <Shortcuts onClose={() => setHelp(false)} />}
      {clipping && pb && (
        <ClipEditor
          pb={pb}
          mediaBase={mediaBase}
          room={room?.info.code}
          at={clipping.at}
          audio={audioIndex ?? null}
          subtitle={subtitle}
          editing={clipping.editing}
          onClose={closeClip}
          onPreviewPlaying={room ? setDucked : undefined}
        />
      )}
    </div>
  )
}

type Shot = { key: number; clip?: Clip; error?: string }

/**
 * A screenshot being taken: the picture flashes like a shutter, then what was
 * saved slides into the corner (toasts can't reach a full-screen player).
 */
function ShotCard({ shot, onClose }: { shot: Shot; onClose: () => void }) {
  const { clip, error } = shot
  return (
    <>
      <div key={shot.key} className="pointer-events-none absolute inset-0 z-30 animate-[shutter_420ms_ease-out_forwards] bg-media-ink" />
      <div className="absolute right-5 bottom-28 z-40 w-56 animate-[rise_220ms_cubic-bezier(.2,.8,.2,1)] md:w-64">
        <Squircle radius={14} edge className="overflow-hidden bg-float shadow-[0_12px_32px_color-mix(in_srgb,var(--color-media-shade)_50%,transparent)]">
          {error ? (
            <div className="p-3">
              <p className="text-sm font-medium">Couldn’t take a screenshot</p>
              <p className="mt-0.5 line-clamp-3 text-xs text-ink-2">{error}</p>
            </div>
          ) : (
            <Link to="/clips" search={{ clip: clip?.id }} disabled={!clip} onClick={onClose} className="block outline-none">
              <div className="grid aspect-video place-items-center bg-media-shade">
                {clip?.poster ? <img src={clip.poster} alt="" className="size-full object-cover" /> : <Spinner className="size-5" />}
              </div>
              <p className="flex items-center gap-2 px-3 py-2 text-[13px] font-medium">
                <Camera className="size-3.5 text-highlight" />
                {clip ? 'Saved to Clips' : 'Taking a screenshot…'}
              </p>
            </Link>
          )}
        </Squircle>
      </div>
    </>
  )
}

/** A track list's heading; in a room, says when you've gone your own way. */
function TrackHeading({ label, own, onFollow }: { label: string; own: boolean; onFollow: () => void }) {
  return (
    <p className="flex items-center justify-between gap-2 px-2.5 pt-1.5 pb-1 text-xs text-ink-3">
      {label}
      {own && (
        <button onClick={onFollow} className="text-ink-2 underline-offset-2 hover:text-ink hover:underline">
          Your own · follow the room
        </button>
      )}
    </p>
  )
}

export type Hud = { kind: 'seek' | 'volume' | 'play' | 'pause' | 'speed'; amount?: number; text?: string; key: number }

/** A quiet confirmation of what just happened, fading as it appears. */
export function HudView({ hud }: { hud: Hud }) {
  if (hud.kind === 'seek') {
    const back = (hud.amount ?? 0) < 0
    return (
      <div className={`pointer-events-none absolute inset-y-0 flex items-center ${back ? 'left-[12%]' : 'right-[12%]'}`}>
        <div key={hud.key} className="flex flex-col items-center gap-1 text-media-ink animate-[pop_160ms_ease-out]">
          <span className="grid size-16 place-items-center rounded-full bg-media-shade/45 backdrop-blur-md">
            {back ? <RotateCcw className="size-7" /> : <RotateCw className="size-7" />}
          </span>
          <span className="text-sm font-medium tabular [text-shadow:0_1px_4px_color-mix(in_srgb,var(--color-media-shade)_60%,transparent)]">
            {back ? '−' : '+'}
            {Math.abs(hud.amount ?? 0)} s
          </span>
        </div>
      </div>
    )
  }
  if (hud.kind === 'volume') {
    const v = hud.amount ?? 0
    const Icon = v === 0 ? VolumeX : v < 0.5 ? Volume1 : Volume2
    return (
      <div className="pointer-events-none absolute inset-x-0 top-[12%] flex justify-center">
        <div className="flex items-center gap-3 rounded-full bg-media-shade/55 px-4 py-2.5 text-media-ink backdrop-blur-md">
          <Icon className="size-4.5" />
          <div className="h-1 w-36 overflow-hidden rounded-full bg-media-ink/20">
            <div className="h-full bg-media-ink transition-[width] duration-150" style={{ width: `${v * 100}%` }} />
          </div>
          <span className="w-8 text-right text-xs tabular">{Math.round(v * 100)}</span>
        </div>
      </div>
    )
  }
  if (hud.kind === 'speed') {
    return (
      <div className="pointer-events-none absolute inset-x-0 top-[12%] flex justify-center">
        <span className="rounded-full bg-media-shade/55 px-4 py-2 text-sm font-medium text-media-ink tabular backdrop-blur-md">{hud.text}</span>
      </div>
    )
  }
  return (
    <div className="pointer-events-none absolute inset-0 grid place-items-center">
      <span key={hud.key} className="grid size-20 animate-[flash_600ms_ease-out_forwards] place-items-center rounded-full bg-media-shade/45 text-media-ink backdrop-blur-md">
        {hud.kind === 'play' ? <Play className="size-8 fill-current" /> : <Pause className="size-8 fill-current" />}
      </span>
    </div>
  )
}

/**
 * Light from the picture spilling into the black around it: a few pixels of
 * the frame, blown up and blurred. Each sample is blended over the last, so
 * a cut glows over rather than flashing.
 */
function Ambilight({ video }: { video: RefObject<HTMLVideoElement | null> }) {
  const canvas = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    const v = video.current
    const ctx = canvas.current?.getContext('2d')
    if (!v || !ctx || matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const { width, height } = ctx.canvas
    const id = setInterval(() => {
      if (v.readyState < 2 || document.hidden) return
      ctx.globalAlpha = v.paused ? 1 : 0.3
      try {
        ctx.drawImage(v, 0, 0, width, height)
      } catch {
        clearInterval(id)
      }
    }, 150)
    return () => clearInterval(id)
  }, [video])
  return (
    <canvas
      ref={canvas}
      width={32}
      height={18}
      aria-hidden
      className="pointer-events-none absolute inset-0 size-full scale-110 opacity-55 blur-[60px] saturate-150"
    />
  )
}

/** Paused for a while: what you're watching, big, over the receded frame. */
function Resting({
  pb,
  line,
  overview,
  time,
  duration,
  shown,
}: {
  pb: Playback
  line: string | null
  overview: string | null
  time: number
  duration: number
  shown: boolean
}) {
  return (
    <div
      aria-hidden={!shown}
      className={`pointer-events-none absolute inset-0 flex items-center bg-linear-to-r from-media-shade/65 via-media-shade/25 to-transparent px-[7vw] transition-opacity duration-700 ${shown ? 'opacity-100' : 'opacity-0'}`}
    >
      <div
        className={`max-w-xl transition-[translate,filter] duration-[1200ms] ease-[cubic-bezier(.16,1,.3,1)] ${shown ? '' : 'translate-y-4 blur-md'}`}
      >
        <p className="text-xs font-medium tracking-[0.2em] text-media-ink/50 uppercase">Paused</p>
        <p className="mt-3 text-[40px] leading-[1.05] font-semibold tracking-[-0.025em] text-balance text-media-ink md:text-[52px]">
          {pb.title.name}
        </p>
        {line && <p className="mt-2.5 text-lg text-media-ink/75">{line}</p>}
        {overview && <p className="mt-4 line-clamp-4 text-[15px] leading-relaxed text-media-ink/60">{overview}</p>}
        {duration > 0 && (
          <p className="mt-6 text-sm text-media-ink/45 tabular">
            {clock(time)} of {clock(duration)}
          </p>
        )}
      </div>
    </div>
  )
}

/** Shown after the newest episode: you're caught up, and when the next one lands. */
function Finale({ pb, series, onBack }: { pb: Playback; series: SeriesGlimpse | null; onBack?: () => void }) {
  const n = series?.next
  return (
    <div className="absolute inset-0 grid place-items-center bg-media-shade/60 p-6 backdrop-blur-sm animate-[fade_300ms_ease-out]">
      <div className="max-w-md text-center animate-[rise_400ms_cubic-bezier(.2,.8,.2,1)]">
        <p className="text-xs font-medium tracking-wide text-media-ink/60 uppercase">{n?.airAt ? 'You’re all caught up' : 'That’s the last one'}</p>
        <p className="mt-2 text-[30px] leading-tight font-semibold tracking-tight text-media-ink">{pb.title.name}</p>
        <p className="mt-2 text-[15px] text-media-ink/70">
          {n?.airAt
            ? `${episodeCode(n.season, n.episode)}${n.name ? ` “${n.name}”` : ''} airs ${airs(n.airAt).replace(/^(Today|Tomorrow)/, (w) => w.toLowerCase())}.`
            : series?.status === 'finished'
              ? 'You’ve finished the whole show.'
              : `You’ve watched every episode there is${pb.label ? `, up to ${pb.label}` : ''}.`}
        </p>
        {onBack && (
          <button
            onClick={onBack}
            className="mt-6 inline-flex h-11 items-center gap-2 rounded-[14px] bg-media-ink px-5 text-[15px] font-medium text-media-shade hover:bg-media-ink/90"
          >
            <ArrowLeft className="size-4.5" /> Back to the show
          </button>
        )}
      </div>
    </div>
  )
}

export function ChromeButton({
  label,
  onClick,
  children,
  active,
  disabled,
}: {
  label: string
  onClick: () => void
  children: ReactNode
  active?: boolean
  disabled?: boolean
}) {
  const { props, tip } = useTip(label)
  return (
    <Squircle
      as="button"
      radius={10}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={`grid size-10 place-items-center text-media-ink/85 transition-colors hover:bg-media-ink/10 hover:text-media-ink disabled:pointer-events-none disabled:opacity-40 ${active ? 'bg-media-ink/10 text-media-ink' : ''}`}
      {...props}
    >
      {children}
      {tip}
    </Squircle>
  )
}

function SkipPill({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <div className="lift animate-[pop_160ms_ease-out]">
      <Squircle
        as="button"
        radius={14}
        edge
        onClick={onClick}
        className="flex h-11 items-center gap-2 bg-media-ink/12 px-4 text-sm font-medium text-media-ink backdrop-blur-xl transition-colors hover:bg-media-ink/20"
      >
        {children}
        <span className="text-2xs text-media-ink/50">S</span>
      </Squircle>
    </div>
  )
}

function UpNext({
  next,
  still,
  countdown,
  onPlay,
}: {
  next: NonNullable<Playback['next']>
  still: string
  countdown: number | null
  /** Missing when someone else decides. */
  onPlay?: () => void
}) {
  return (
    <div className="lift animate-[pop_180ms_ease-out]">
      <Squircle radius={16} edge className="w-80 bg-media-panel/85 p-2.5 backdrop-blur-xl">
        <div className="flex gap-3">
          <Squircle radius={9} className="aspect-video w-28 shrink-0 bg-panel">
            <img src={still} alt="" className="size-full object-cover" />
          </Squircle>
          <div className="min-w-0 flex-1 py-0.5">
            <p className="text-xs text-ink-3">Up next · {next.label}</p>
            <p className="mt-0.5 line-clamp-2 text-sm leading-snug font-medium">{next.name ?? next.label}</p>
          </div>
        </div>
        {onPlay && (
        <button
          onClick={onPlay}
          className="relative mt-2.5 flex h-9 w-full items-center justify-center gap-2 overflow-hidden rounded-[10px] bg-accent/85 text-sm font-medium text-on-accent hover:bg-accent-hover"
        >
          {/* The countdown, as a fill sweeping across. */}
          {countdown !== null && (
            <span className="absolute inset-0 origin-left animate-[sweep_5s_linear_forwards] bg-accent-hover" />
          )}
          <span className="relative flex items-center gap-2">
            <Play className="size-4 fill-current" />
            {countdown !== null ? 'Playing next' : 'Play now'}
          </span>
        </button>
        )}
      </Squircle>
    </div>
  )
}

export function VolumeControl({
  volume,
  muted,
  setVolume,
  setMuted,
}: {
  volume: number
  muted: boolean
  setVolume: (v: number) => void
  setMuted: (fn: (m: boolean) => boolean) => void
}) {
  const Icon = muted || volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2
  return (
    <div className="group flex items-center pointer-coarse:[&>input]:hidden">
      <ChromeButton label="Mute (m)" onClick={() => setMuted((m) => !m)}>
        <Icon className="size-5" />
      </ChromeButton>
      <input
        type="range"
        aria-label="Volume"
        min={0}
        max={1}
        step={0.01}
        value={muted ? 0 : volume}
        onChange={(e) => {
          setVolume(Number(e.target.value))
          setMuted(() => false)
        }}
        className="h-1 w-0 cursor-pointer appearance-none rounded-full bg-media-ink/25 opacity-0 accent-media-ink transition-all duration-200 group-hover:w-20 group-hover:opacity-100 focus:w-20 focus:opacity-100"
      />
    </div>
  )
}

export function Timeline({
  base,
  time,
  duration,
  buffered,
  chapters,
  onSeek,
}: {
  /** Where this video's endpoints live; without them, no frames on hover. */
  base?: string
  time: number
  duration: number
  buffered: [number, number][]
  chapters: Chapter[]
  onSeek: (t: number) => void
}) {
  const bar = useRef<HTMLDivElement>(null)
  const [hover, setHover] = useState<number | null>(null)
  const [drag, setDrag] = useState<number | null>(null)
  const at = (clientX: number) => {
    const r = bar.current!.getBoundingClientRect()
    return Math.max(0, Math.min(1, (clientX - r.left) / r.width)) * duration
  }
  const pct = (t: number) => (duration ? `${(t / duration) * 100}%` : '0%')
  const shown = drag ?? time

  // Chapters become segments with small gaps; without chapters, one segment.
  const segments = chapters.length > 1 ? chapters.map((c) => [c.start, c.end] as const) : [[0, duration] as const]
  const hoverChapter = hover !== null ? chapters.find((c) => hover >= c.start && hover < c.end) : undefined

  return (
    <div
      ref={bar}
      className="group relative flex h-5 cursor-pointer items-center"
      onPointerMove={(e) => {
        setHover(at(e.clientX))
        if (drag !== null) setDrag(at(e.clientX))
      }}
      onPointerLeave={() => setHover(null)}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId)
        setDrag(at(e.clientX))
      }}
      onPointerUp={(e) => {
        if (drag !== null) onSeek(at(e.clientX))
        setDrag(null)
      }}
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={Math.round(time)}
    >
      {segments.map(([s, e], i) => (
        <div
          key={i}
          className="absolute h-1 overflow-hidden rounded-full bg-media-ink/20 transition-[height] duration-150 group-hover:h-1.5"
          style={{ left: pct(s), width: `calc(${pct(e - s)} - ${i < segments.length - 1 ? 3 : 0}px)` }}
        >
          {buffered.map(([bs, be], j) => {
            const from = Math.max(bs, s)
            const to = Math.min(be, e)
            if (to <= from) return null
            return (
              <div
                key={j}
                className="absolute inset-y-0 bg-media-ink/25"
                style={{ left: `${((from - s) / (e - s)) * 100}%`, width: `${((to - from) / (e - s)) * 100}%` }}
              />
            )
          })}
          <div
            className="absolute inset-y-0 left-0 bg-media-ink"
            style={{ width: `${Math.max(0, Math.min(1, (shown - s) / (e - s))) * 100}%` }}
          />
        </div>
      ))}
      <div
        className="absolute size-3.5 -translate-x-1/2 rounded-full bg-media-ink opacity-0 shadow-[0_0_0_4px_color-mix(in_srgb,var(--color-media-ink)_15%,transparent)] transition-opacity group-hover:opacity-100"
        style={{ left: pct(shown) }}
      />
      {hover !== null && (
        <div
          className="pointer-events-none absolute bottom-6 -translate-x-1/2 text-center text-xs whitespace-nowrap text-media-ink tabular"
          style={{ left: `clamp(5.5rem, ${pct(hover)}, calc(100% - 5.5rem))` }}
        >
          {base && <Preview base={base} at={hover} />}
          <span className="mt-1.5 inline-block rounded-lg bg-media-panel/90 px-2 py-1 backdrop-blur-md">
            {clock(hover)}
            {hoverChapter?.title && <span className="ml-1.5 text-media-ink/55">{hoverChapter.title}</span>}
          </span>
        </div>
      )}
    </div>
  )
}

const PREVIEW_STEP = 10

/**
 * A frame from around the hovered time. Frames come in 10 s steps so each is
 * made once; while the next one loads, the last one stays up.
 */
function Preview({ base, at }: { base: string; at: number }) {
  const step = Math.floor(at / PREVIEW_STEP) * PREVIEW_STEP
  const [wanted, setWanted] = useState(step)
  const [shown, setShown] = useState<number | null>(null)
  const [failed, setFailed] = useState(false)
  // Wait for the pointer to settle a little before asking the server.
  useEffect(() => {
    const t = setTimeout(() => setWanted(step), 60)
    return () => clearTimeout(t)
  }, [step])
  useEffect(() => {
    let alive = true
    const img = new Image()
    img.onload = () => alive && setShown(wanted)
    img.onerror = () => alive && setFailed(true)
    img.src = `${base}/preview/${wanted}`
    return () => {
      alive = false
    }
  }, [base, wanted])
  if (failed) return null
  return (
    <Squircle radius={10} edge className="block aspect-video w-44 bg-media-panel/90 shadow-lg">
      {shown !== null && <img src={`${base}/preview/${shown}`} alt="" className="size-full object-cover" />}
    </Squircle>
  )
}
