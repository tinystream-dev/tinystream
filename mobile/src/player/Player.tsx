// SPDX-License-Identifier: AGPL-3.0-or-later
// Plays a video (web's player/Player.tsx, which is the spec): the same
// stream, tracks, chapters, next episode and screenshots, with a phone's
// gestures. Tap shows or hides the controls; double-tap a side to skip 10 s
// (more taps, more seconds); drag sideways to scrub, up or down on the left
// for brightness and on the right for volume; pinch to fill the screen.

import { clock } from '@tinystream/shared/format'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useRouter } from 'expo-router'
import {
  ArrowLeft,
  Camera,
  Captions,
  Lock,
  Pause,
  PictureInPicture2,
  Play,
  RotateCw,
  Settings2,
  Smartphone,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
} from 'lucide-react-native'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { AppState, StyleSheet, Text, View, useWindowDimensions } from 'react-native'
import { Gesture, GestureDetector } from 'react-native-gesture-handler'
import Animated, { useAnimatedStyle, withTiming } from 'react-native-reanimated'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { haptic } from '../../modules/haptics'
import {
  type PlaybackError,
  type Progress,
  type Status,
  VideoView,
  type VideoViewHandle,
  brightness,
  canDecode,
  mediaVolume,
  setBrightness,
  setMediaVolume,
} from '../../modules/player'
import { Img } from '../components/Img'
import type { SeriesGlimpse } from '../components/SeriesPanel'
import { Shade } from '../components/media'
import { Segmented, Spinner } from '../components/ui'
import { resolve } from '../lib/address'
import { authHeaders } from '../lib/graphql'
import { useMe, useStatus } from '../queries'
import { useApi, useConnection } from '../session'
import { useMusic } from '../music/context'
import { useTheme } from '../theme/ThemeProvider'
import { type Hud, HudView } from './Hud'
import { ChromeButton, Failure, Finale, PlayerSheet, Resting, SheetHeading, SheetItem, type Shot, ShotCard, SkipPill, UpNext } from './Overlays'
import { type Playback, type Quality, plan, supportFor } from './plan'
import { pref } from './prefs'
import { OverviewQuery, PlaybackQuery, SaveProgress, ScheduleQuery, TakeScreenshot } from './queries'
import { Timeline } from './Timeline'
import { SKIPPABLE, defaultAudio, defaultSubtitle, skipLabel, trackName } from './tracks'

export type Rotation = 'auto' | 'locked' | 'portrait'

/** What the rotation button does: landscape following the sensor, then locked as it is, then portrait. */
const ROTATIONS: { [r in Rotation]: { next: Rotation; label: string } } = {
  auto: { next: 'locked', label: 'Rotates with the phone' },
  locked: { next: 'portrait', label: 'Rotation locked' },
  portrait: { next: 'auto', label: 'Portrait' },
}

const QUALITIES: Quality[] = ['auto', 1080, 720, 480]
const RATES = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3]
/** Controls hide after this long while playing. */
const IDLE_MS = 3000
/** Taps closer together than this are a double tap; once skipping, each tap within the second one adds more. */
const DOUBLE_TAP_MS = 300
const STACK_TAP_MS = 600
/** Scrubbing across the whole screen moves this far (but at most the whole video). */
const SCRUB_SPAN = { min: 60, max: 300 }
/** The levels change over this much of the screen's height. */
const LEVEL_SPAN = 0.7
const BRIGHTNESS_DETENT = 0.1

type Drag = { mode: 'scrub'; from: number } | { mode: 'brightness'; from: number; step: number } | { mode: 'volume'; from: number; max: number }

export function Player({ mediaId, startAt, rotation, onRotation }: { mediaId: number; startAt?: number; rotation: Rotation; onRotation: (r: Rotation) => void }) {
  const { music } = useMusic()
  useEffect(() => {
    music.suspend()
    return () => music.unsuspend()
  }, [music])
  const api = useApi()
  const connection = useConnection()
  const origin = connection?.origin
  const token = connection?.token ?? null
  const router = useRouter()
  const qc = useQueryClient()
  const insets = useSafeAreaInsets()
  const { width, height } = useWindowDimensions()
  const { tokens } = useTheme()
  const me = useMe()
  const server = useStatus().data?.server
  const canClip = !!server?.clips && !!me?.permissions.clip
  const ink = tokens['media-ink']
  const mediaBase = `/api/media/${mediaId}`

  const video = useRef<VideoViewHandle>(null)

  const { data: pb, error: infoError, refetch } = useQuery({
    queryKey: ['playback', mediaId],
    queryFn: async (): Promise<Playback> => {
      const r = await api.request(PlaybackQuery, { id: mediaId })
      if (!r.video) throw new Error('This video isn’t here anymore.')
      return { ...r.video, transcoding: r.server.transcoding }
    },
    staleTime: Infinity,
    gcTime: 0,
  })
  const { data: supported } = useQuery({
    queryKey: ['decoders', mediaId],
    queryFn: () => supportFor(pb!, canDecode),
    enabled: !!pb,
    staleTime: Infinity,
    gcTime: 0,
  })

  const [status, setStatus] = useState<Status>({ state: 'idle', playing: false, paused: false })
  const [time, setTime] = useState(0)
  const [buffered, setBuffered] = useState<[number, number] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [retrying, setRetrying] = useState(false)
  /** Whether the video has shown a frame yet. */
  const [started, setStarted] = useState(false)
  const [idle, setIdle] = useState(false)
  // undefined until the initial tracks are picked, so the stream is built once.
  const [audioIndex, setAudioIndex] = useState<number | null | undefined>(undefined)
  const [subtitle, setSubtitle] = useState<string | null>(null)
  const [quality, setQuality] = useState<Quality>('auto')
  const [rate, setRate] = useState(() => Number(pref.get('rate') ?? 1))
  const [muted, setMuted] = useState(false)
  const [fill, setFill] = useState(false)
  const [pip, setPip] = useState(false)
  const [sheet, setSheet] = useState<'tracks' | 'settings' | null>(null)
  const [hud, setHud] = useState<Hud | null>(null)
  const [shot, setShot] = useState<Shot | null>(null)
  const [scrub, setScrub] = useState<number | null>(null)
  const [countdown, setCountdown] = useState<number | null>(null)
  const [attempt, setAttempt] = useState(0)

  const playing = status.playing
  const ended = status.state === 'ended'
  const waiting = !error && (status.state === 'buffering' || status.state === 'idle' || retrying)
  const duration = pb?.media.duration ?? 0

  // What callbacks from native and timers need now, not when they were made.
  const now = useRef({ time: 0, playing: false, paused: true, loaded: false })
  now.current.time = time
  now.current.playing = playing
  now.current.paused = status.paused

  const hudTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  /** Briefly shows what a gesture just did. Repeated seeks the same way add up. */
  const flash = useCallback((h: Omit<Hud, 'key'>, ms = 700) => {
    clearTimeout(hudTimer.current)
    setHud((prev) => {
      const add = h.kind === 'seek' && prev?.kind === 'seek' && Math.sign(prev.amount ?? 0) === Math.sign(h.amount ?? 0)
      return { ...h, amount: add ? (prev!.amount ?? 0) + (h.amount ?? 0) : h.amount, key: (prev?.key ?? 0) + 1 }
    })
    hudTimer.current = setTimeout(() => setHud(null), ms)
  }, [])

  // Pick initial tracks once the file's info arrives.
  useEffect(() => {
    if (!pb) return
    setAudioIndex(defaultAudio(pb, pref.get('audioLanguage')))
    setSubtitle(defaultSubtitle(pb, pref.get('subtitles') === 'off', pref.get('subtitleLanguage')))
  }, [pb])

  const streamPlan = useMemo(
    () => (pb && supported && audioIndex !== undefined ? plan(pb, audioIndex, quality, supported) : null),
    [pb, supported, audioIndex, quality],
  )

  // (Re)build the stream whenever the plan changes, keeping our place.
  const resumeAt = useRef<number | null>(startAt ?? null)
  const resumePaused = useRef(false)
  useEffect(() => {
    const v = video.current
    if (!pb || !streamPlan || !v || !origin) return
    const start = resumeAt.current ?? (!pb.finished && pb.position && pb.position > 5 ? pb.position : 0)
    const paused = resumeAt.current !== null && resumePaused.current
    setError(null)
    setRetrying(false)
    now.current.loaded = false
    const show = pb.title.kind === 'SHOW'
    void v.load({
      url: resolve(origin, `/api/media/${pb.id}`),
      headers: authHeaders(token),
      plan: { video: streamPlan.video, height: streamPlan.height, audio: streamPlan.audio, audioMode: streamPlan.audioMode },
      startAt: start,
      duration: pb.media.duration ?? 0,
      paused,
      title: show ? [pb.label, pb.name].filter(Boolean).join(' · ') || pb.title.name : pb.title.name,
      subtitle: show ? pb.title.name : null,
      artwork: pb.still ? resolve(origin, pb.still) : null,
      hasPrevious: !!pb.previous,
      hasNext: !!pb.next,
    })
    return () => {
      // Until the stream has loaded, where playback is doesn't say where we meant to be, so carry the intent over instead.
      const loaded = now.current.loaded
      resumeAt.current = loaded ? now.current.time : start
      resumePaused.current = loaded ? now.current.paused : paused
    }
  }, [pb, streamPlan, origin, token, attempt])

  useEffect(() => {
    if (!pb || !origin) return
    const base = resolve(origin, `/api/media/${pb.id}`)
    const track = subtitle ? { url: `${base}/subtitles/${subtitle}`, fonts: pb.media.fonts.map((f) => `${base}/fonts/${f.index}`), headers: authHeaders(token) } : null
    video.current?.selectSubtitles(track).catch((e) => console.warn('subtitles:', e))
  }, [subtitle, pb, origin, token])

  useEffect(() => void video.current?.setRate(rate), [rate, streamPlan])
  useEffect(() => void video.current?.setMuted(muted), [muted, streamPlan])

  // Save progress regularly, when pausing, when the app goes away, and when leaving.
  const save = useCallback(() => {
    const t = now.current.time
    if (!duration || t < 1) return
    void api
      .request(SaveProgress, { videoId: mediaId, position: t, duration })
      .then(() => {
        // So pages behind the player show where we are now.
        void qc.invalidateQueries({ queryKey: ['home'] })
        void qc.invalidateQueries({ queryKey: ['item'] })
        void qc.invalidateQueries({ queryKey: ['library'] })
      })
      .catch(() => {})
  }, [api, mediaId, duration, qc])
  useEffect(() => {
    const id = setInterval(() => now.current.playing && save(), 10_000)
    const sub = AppState.addEventListener('change', (state) => state !== 'active' && save())
    return () => {
      clearInterval(id)
      sub.remove()
      save()
    }
  }, [save])

  // The controls hide when left alone while playing.
  const idleTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const poke = useCallback(() => {
    setIdle(false)
    clearTimeout(idleTimer.current)
    idleTimer.current = setTimeout(() => setIdle(true), IDLE_MS)
  }, [])
  useEffect(() => {
    poke()
    return () => {
      clearTimeout(idleTimer.current)
      clearTimeout(hudTimer.current)
    }
  }, [poke])

  const onStatus = (s: Status) => {
    if (s.playing && !status.playing) {
      setCountdown(null)
      poke()
    }
    if (!s.playing && status.playing && s.paused) save()
    if (s.state === 'ready' || s.playing) {
      now.current.loaded = true
      setStarted(true)
      setRetrying(false)
    }
    setStatus(s)
  }
  const onProgress = (p: Progress) => {
    setTime(p.position)
    setBuffered(p.buffered ?? null)
  }
  const [problem, setProblem] = useState<string | null>(null)
  const onError = (e: PlaybackError) => {
    console.warn('player:', e.retrying ? 'retrying after' : 'gave up:', e.detail ?? e.message)
    setRetrying(e.retrying)
    setProblem(e.retrying ? e.message : null)
    if (!e.retrying) setError(e.message)
  }

  /** Plays or pauses (toggles without `play`); returns whether it's now playing. */
  const setPlayback = useCallback((play?: boolean) => {
    const want = play ?? now.current.paused
    if (want) void video.current?.play()
    else void video.current?.pause()
    return want
  }, [])

  /** Where double taps are taking playback, while they keep coming. */
  const target = useRef<{ at: number; until: number } | null>(null)
  const seek = useCallback(
    (t: number) => {
      const to = Math.max(0, Math.min(duration || Infinity, t))
      void video.current?.seek(to)
      setTime(to)
      now.current.time = to
      poke()
    },
    [duration, poke],
  )
  const seekBy = useCallback(
    (delta: number) => {
      const base = target.current && target.current.until > Date.now() ? target.current.at : now.current.time
      const to = Math.max(0, Math.min(duration || Infinity, base + delta))
      target.current = { at: to, until: Date.now() + 1000 }
      seek(to)
      flash({ kind: 'seek', amount: delta })
    },
    [seek, flash, duration],
  )

  const changeRate = useCallback(
    (r: number) => {
      const next = Math.max(0.25, Math.min(3, Math.round(r * 100) / 100))
      setRate(next)
      pref.set('rate', String(next))
      flash({ kind: 'speed', text: `${next}×` })
    },
    [flash],
  )
  const chooseAudio = (index: number) => {
    resumeAt.current = now.current.time
    resumePaused.current = now.current.paused
    setAudioIndex(index)
    const lang = pb?.media.audio.find((a) => a.index === index)?.language
    if (lang) pref.set('audioLanguage', lang)
  }
  const chooseSubtitle = (id: string | null) => {
    setSubtitle(id)
    pref.set('subtitles', id ? 'on' : 'off')
    const lang = id && pb?.media.subtitles.find((s) => s.id === id)?.language
    if (lang) pref.set('subtitleLanguage', lang)
  }
  const chooseQuality = (q: Quality) => {
    resumeAt.current = now.current.time
    resumePaused.current = now.current.paused
    setQuality(q)
  }

  const goTo = useCallback(
    (id: number) => {
      save()
      router.setParams({ id: String(id), t: undefined })
    },
    [router, save],
  )
  const back = useCallback(() => {
    if (router.canGoBack()) router.back()
    else router.replace('/')
  }, [router])

  const rotate = () => {
    const next = ROTATIONS[rotation].next
    onRotation(next)
    flash({ kind: 'text', text: ROTATIONS[next].label }, 1200)
  }
  const toggleFill = useCallback(
    (on: boolean) => {
      setFill(on)
      haptic('tick')
      flash({ kind: 'text', text: on ? 'Filling the screen' : 'Fitting the screen' })
    },
    [flash],
  )
  const enterPip = async () => {
    if (!(await video.current?.enterPip())) flash({ kind: 'text', text: 'Picture-in-picture is turned off for tinystream' }, 2000)
  }

  /** Screenshots what's on screen (with the subtitles showing), and plays on. */
  const shooting = useRef(false)
  const takeScreenshot = async () => {
    if (!pb || shooting.current) return
    shooting.current = true
    const key = Date.now()
    setShot({ key })
    haptic('press')
    try {
      const clip = (await api.request(TakeScreenshot, { input: { videoId: pb.id, at: now.current.time, subtitles: subtitle } })).takeScreenshot
      setShot((s) => (s?.key === key ? { key, clip } : s))
      haptic('success')
      void qc.invalidateQueries({ queryKey: ['clips'] })
    } catch (e) {
      setShot((s) => (s?.key === key ? { key, error: (e as Error)?.message ?? String(e) } : s))
      haptic('error')
    } finally {
      shooting.current = false
    }
  }
  useEffect(() => {
    if (!shot?.clip && !shot?.error) return
    const t = setTimeout(() => setShot(null), shot.error ? 6000 : 4000)
    return () => clearTimeout(t)
  }, [shot])

  const chapters = pb?.media.chapters ?? []
  const chapter = chapters.find((c) => time >= c.start && time < c.end)
  const skippable = chapter?.title && SKIPPABLE.test(chapter.title) && chapter.end - time > 3 ? chapter : null
  const nearEnd = !!pb?.next && duration > 0 && (time > duration - 25 || (!!chapter?.title && /^(ed|ending|credits)/i.test(chapter.title)))

  // Autoplay the next episode a few seconds after this one ends.
  useEffect(() => {
    if (!ended || !pb?.next) return
    setCountdown(5)
    const id = setInterval(() => setCountdown((c) => (c === null ? null : c - 1)), 1000)
    return () => clearInterval(id)
  }, [ended, pb])
  useEffect(() => {
    if (countdown !== null && countdown <= 0 && pb?.next) goTo(pb.next.id)
  }, [countdown, pb, goTo])

  // What's next once the last episode ends.
  const { data: seriesInfo } = useQuery({
    queryKey: ['series', 'item', pb?.title.id, 'glimpse'],
    queryFn: async (): Promise<SeriesGlimpse | null> => (await api.request(ScheduleQuery, { id: pb!.title.id })).title?.series ?? null,
    enabled: !!pb && pb.title.kind === 'SHOW' && !pb.next && ended,
  })

  // Scrubbing, on the bar or across the screen: ticks every 10 s and at chapters, a thud at either end.
  const scrubbed = useRef<number | null>(null)
  const frame = useRef<number | null>(null)
  const pending = useRef<number | null>(null)
  const onScrub = useCallback(
    (at: number | null) => {
      const prev = scrubbed.current
      scrubbed.current = at
      if (at !== null && prev !== null && at !== prev) {
        const [lo, hi] = prev < at ? [prev, at] : [at, prev]
        const crossed = Math.floor(hi / 10) !== Math.floor(lo / 10) || chapters.some((c) => c.start > lo && c.start <= hi)
        if ((at <= 0 || at >= duration) && prev > 0 && prev < duration) haptic('edge')
        else if (crossed) haptic('tick')
      }
      if (at !== null) poke()
      // At most once a frame: the finger moves more often than that.
      pending.current = at
      if (at === null) {
        if (frame.current !== null) cancelAnimationFrame(frame.current)
        frame.current = null
        setScrub(null)
      } else if (frame.current === null) {
        frame.current = requestAnimationFrame(() => {
          frame.current = null
          setScrub(pending.current)
        })
      }
    },
    [chapters, duration, poke],
  )
  const onBarSeek = (at: number) => {
    seek(at)
    target.current = null
  }

  // Touch: tap shows or hides the controls; double-tap a side to skip.
  const lastTap = useRef<{ t: number; side: number; skipping: boolean } | null>(null)
  const tapTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(tapTimer.current), [])
  const onTap = (x: number) => {
    const t = Date.now()
    const prev = lastTap.current
    const side = x < width / 3 ? -1 : x > (width * 2) / 3 ? 1 : 0
    if (prev && side !== 0 && t - prev.t < (prev.skipping ? STACK_TAP_MS : DOUBLE_TAP_MS)) {
      clearTimeout(tapTimer.current)
      lastTap.current = { t, side, skipping: true }
      haptic('tick')
      seekBy(side * 10)
      return
    }
    lastTap.current = { t, side, skipping: false }
    clearTimeout(tapTimer.current)
    tapTimer.current = setTimeout(() => {
      if (!idle && playing) {
        clearTimeout(idleTimer.current)
        setIdle(true)
      } else poke()
    }, 260)
  }

  // Dragging: sideways scrubs; up and down sets brightness (left half) or volume (right half).
  const drag = useRef<Drag | null>(null)
  const level = useRef(0.5)
  useEffect(() => {
    void brightness().then((b) => (level.current = b))
  }, [])
  const span = Math.max(SCRUB_SPAN.min, Math.min(SCRUB_SPAN.max, duration * 0.5, duration || Infinity))
  const onDragStart = (x: number, dx: number, dy: number) => {
    if (Math.abs(dx) > Math.abs(dy)) {
      if (!duration) return
      drag.current = { mode: 'scrub', from: now.current.time }
    } else if (x < width / 2) {
      drag.current = { mode: 'brightness', from: level.current, step: Math.round(level.current / BRIGHTNESS_DETENT) }
    } else {
      const v = mediaVolume()
      drag.current = { mode: 'volume', from: v.level, max: v.max }
    }
  }
  const onDragMove = (dx: number, dy: number) => {
    const d = drag.current
    if (!d) return
    if (d.mode === 'scrub') {
      onScrub(Math.max(0, Math.min(duration, d.from + (dx / width) * span)))
    } else if (d.mode === 'brightness') {
      const b = Math.max(0.01, Math.min(1, d.from - dy / (height * LEVEL_SPAN)))
      const step = Math.round(b / BRIGHTNESS_DETENT)
      if (step !== d.step) {
        d.step = step
        haptic(b <= 0.01 || b >= 1 ? 'edge' : 'tick')
      }
      level.current = b
      void setBrightness(b)
      flash({ kind: 'brightness', amount: b }, 900)
    } else {
      const v = Math.round(Math.max(0, Math.min(d.max, d.from - (dy / (height * LEVEL_SPAN)) * d.max)))
      if (v !== mediaVolume().level) {
        setMediaVolume(v)
        haptic(v === 0 || v === d.max ? 'edge' : 'tick')
      }
      flash({ kind: 'volume', amount: v / d.max }, 900)
    }
  }
  const onDragEnd = () => {
    const d = drag.current
    drag.current = null
    if (d?.mode === 'scrub' && scrubbed.current !== null) {
      const at = scrubbed.current
      onScrub(null)
      seek(at)
    }
  }

  // Pinching out fills the screen, in fits it again.
  const pinched = useRef(false)
  const onPinch = (scale: number) => {
    if (pinched.current) return
    if (scale > 1.12 && !fill) (pinched.current = true), toggleFill(true)
    else if (scale < 0.88 && fill) (pinched.current = true), toggleFill(false)
  }

  const gestures = Gesture.Race(
    Gesture.Pinch()
      .runOnJS(true)
      .onUpdate((e) => onPinch(e.scale))
      .onFinalize(() => (pinched.current = false)),
    Gesture.Pan()
      .runOnJS(true)
      .maxPointers(1)
      .minDistance(14)
      .onStart((e) => onDragStart(e.x - e.translationX, e.translationX, e.translationY))
      .onUpdate((e) => onDragMove(e.translationX, e.translationY))
      .onFinalize(onDragEnd),
    Gesture.Tap()
      .runOnJS(true)
      .maxDuration(250)
      .onEnd((e, ok) => ok && onTap(e.x)),
  )

  // Paused and left alone for a while: the frame recedes and what's playing steps forward.
  const resting = !playing && idle && started && !ended && !waiting && !error && !!pb && !pip && !sheet
  const [rested, setRested] = useState(false)
  useEffect(() => {
    setRested(false)
    if (!resting) return
    const t = setTimeout(() => setRested(true), 2400)
    return () => clearTimeout(t)
  }, [resting])
  const { data: details } = useQuery({
    queryKey: ['overview', pb?.title.id, mediaId],
    queryFn: () => api.request(OverviewQuery, { id: pb!.title.id, videoId: mediaId }),
    enabled: resting && !!pb,
  })
  const overview = details?.video?.overview ?? details?.title?.overview ?? null

  const failure = error ?? (infoError ? (infoError as Error).message : null)
  const chrome = ((!playing && !rested) || !idle || !!failure || scrub !== null || !!sheet) && !pip
  const chromeStyle = useAnimatedStyle(() => ({ opacity: withTiming(chrome ? 1 : 0, { duration: 220 }) }), [chrome])
  const title = pb?.title.name ?? ''
  const line = pb?.title.kind === 'SHOW' ? [pb.label, pb.name].filter(Boolean).join(' · ') : null
  const pad = { left: Math.max(insets.left, 16), right: Math.max(insets.right, 16), top: Math.max(insets.top, 12), bottom: Math.max(insets.bottom, 10) }

  return (
    <View style={{ flex: 1, backgroundColor: tokens['media-shade'] }}>
      <GestureDetector gesture={gestures}>
        <View style={StyleSheet.absoluteFill} collapsable={false}>
          <VideoView
            ref={video}
            style={StyleSheet.absoluteFill}
            fill={fill}
            onStatus={(e) => onStatus(e.nativeEvent)}
            onProgress={(e) => onProgress(e.nativeEvent)}
            onPlaybackError={(e) => onError(e.nativeEvent)}
            onPip={(e) => setPip(e.nativeEvent.active)}
            onRemote={(e) => {
              const to = e.nativeEvent.action === 'next' ? pb?.next : pb?.previous
              if (to) goTo(to.id)
            }}
          />
          {/* The still holds the frame until the video has one of its own. */}
          {pb && !started && <Img src={pb.still} contentFit="contain" style={StyleSheet.absoluteFill} pointerEvents="none" />}
          {pb && rested && <Resting pb={pb} line={line} overview={overview} time={time} duration={duration} />}
        </View>
      </GestureDetector>

      {waiting && !pip && (
        <View pointerEvents="none" style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center' }]}>
          <Spinner size="large" />
          {retrying && <Text className="font-sans mt-3 text-sm text-media-ink/70">Reconnecting…</Text>}
          {retrying && problem && (
            <Text className="font-sans mt-1 px-10 text-center text-xs text-media-ink/45" numberOfLines={3}>
              {problem}
            </Text>
          )}
        </View>
      )}

      {hud && !pip && <HudView hud={hud} />}

      <Animated.View pointerEvents={chrome ? 'box-none' : 'none'} style={[StyleSheet.absoluteFill, chromeStyle]}>
        <View pointerEvents="none" style={{ position: 'absolute', left: 0, right: 0, top: 0, height: 140 }}>
          <Shade to="bottom" stops={[0.7, 0.3, 0]} media />
        </View>
        <View pointerEvents="none" style={{ position: 'absolute', left: 0, right: 0, bottom: 0, height: 180 }}>
          <Shade to="top" stops={[0.85, 0.4, 0]} media />
        </View>

        <View className="flex-row items-start gap-2" style={{ position: 'absolute', left: pad.left - 8, right: pad.right - 8, top: pad.top }}>
          <ChromeButton label="Back" onPress={back}>
            <ArrowLeft size={22} color={ink} />
          </ChromeButton>
          <View className="min-w-0 flex-1 pt-1.5">
            <Text className="font-sans text-[15px] font-medium text-media-ink" numberOfLines={1}>
              {title}
            </Text>
            {line ? (
              <Text className="font-sans text-[13px] text-media-ink/60" numberOfLines={1}>
                {line}
              </Text>
            ) : null}
          </View>
          <ChromeButton label={`Rotation: ${ROTATIONS[rotation].label}`} onPress={rotate} active={rotation !== 'auto'}>
            {rotation === 'auto' ? <RotateCw size={20} color={ink} /> : rotation === 'locked' ? <Lock size={20} color={ink} /> : <Smartphone size={20} color={ink} />}
          </ChromeButton>
          <ChromeButton label="Picture in picture" onPress={() => void enterPip()}>
            <PictureInPicture2 size={20} color={ink} />
          </ChromeButton>
        </View>

        {!failure && (
          <View pointerEvents="box-none" style={[StyleSheet.absoluteFill, { alignItems: 'center', justifyContent: 'center', flexDirection: 'row', gap: 28 }]}>
            {pb?.previous ? (
              <ChromeButton label={`Previous: ${pb.previous.label ?? ''}`} onPress={() => goTo(pb.previous!.id)} size={52}>
                <SkipBack size={24} color={ink} fill={ink} />
              </ChromeButton>
            ) : (
              <View style={{ width: 52 }} />
            )}
            {!waiting ? (
              <ChromeButton label={status.paused ? 'Play' : 'Pause'} onPress={() => setPlayback()} size={72}>
                {status.paused ? <Play size={36} color={ink} fill={ink} /> : <Pause size={36} color={ink} fill={ink} />}
              </ChromeButton>
            ) : (
              <View style={{ width: 72 }} />
            )}
            {pb?.next ? (
              <ChromeButton label={`Next: ${pb.next.label ?? ''}`} onPress={() => goTo(pb.next!.id)} size={52}>
                <SkipForward size={24} color={ink} fill={ink} />
              </ChromeButton>
            ) : (
              <View style={{ width: 52 }} />
            )}
          </View>
        )}

        <View style={{ position: 'absolute', left: pad.left, right: pad.right, bottom: pad.bottom }}>
          <Timeline base={mediaBase} time={time} duration={duration} buffered={buffered} chapters={chapters} scrubbing={scrub} onScrub={onScrub} onSeek={onBarSeek} />
          <View className="mt-1 flex-row items-center" style={{ marginHorizontal: -8 }}>
            <ChromeButton label={muted ? 'Unmute' : 'Mute'} onPress={() => setMuted((m) => !m)}>
              {muted ? <VolumeX size={20} color={ink} /> : <Volume2 size={20} color={ink} />}
            </ChromeButton>
            <Text className="font-sans ml-1 text-[13px] text-media-ink/80" style={{ fontVariant: ['tabular-nums'] }}>
              {clock(time)} <Text className="text-media-ink/40">/ {clock(duration)}</Text>
            </Text>
            {chapter?.title ? (
              <Text className="font-sans ml-3 shrink text-[13px] text-media-ink/50" numberOfLines={1}>
                {chapter.title}
              </Text>
            ) : null}
            {rate !== 1 && (
              <Text className="font-sans ml-3 overflow-hidden rounded-md bg-media-ink/10 px-1.5 py-0.5 text-2xs text-media-ink/80" style={{ fontVariant: ['tabular-nums'] }}>
                {rate}×
              </Text>
            )}
            <View className="flex-1" />
            {pb && (
              <ChromeButton label="Audio & subtitles" onPress={() => setSheet('tracks')} active={sheet === 'tracks'}>
                <Captions size={20} color={ink} />
              </ChromeButton>
            )}
            {streamPlan && (
              <ChromeButton label="Quality & speed" onPress={() => setSheet('settings')} active={sheet === 'settings'}>
                <Settings2 size={20} color={ink} />
              </ChromeButton>
            )}
            {canClip && pb && (
              <ChromeButton label="Screenshot" onPress={() => void takeScreenshot()}>
                <Camera size={20} color={ink} />
              </ChromeButton>
            )}
          </View>
        </View>
      </Animated.View>

      {!pip && (
        <View pointerEvents="box-none" style={{ position: 'absolute', right: pad.right, bottom: pad.bottom + 92, alignItems: 'flex-end', gap: 12 }}>
          {skippable && !nearEnd && <SkipPill onPress={() => seek(skippable.end)}>{skipLabel(skippable.title!)}</SkipPill>}
          {pb?.next && (nearEnd || ended) && <UpNext next={pb.next} countdown={countdown} onPlay={() => goTo(pb.next!.id)} />}
        </View>
      )}

      {/* The last episode there is, for now. */}
      {ended && pb && pb.title.kind === 'SHOW' && !pb.next && !pip && <Finale pb={pb} series={seriesInfo ?? null} onBack={back} />}

      {shot && !pip && (
        <ShotCard
          shot={shot}
          style={{ right: pad.right, bottom: pad.bottom + 92 }}
          onPress={() => {
            setShot(null)
            router.push('/(tabs)/(home)/clips')
          }}
        />
      )}

      {failure && !pip && (
        <Failure
          message={failure}
          onRetry={() => {
            if (infoError) void refetch()
            else {
              resumeAt.current = now.current.time
              setAttempt((a) => a + 1)
            }
          }}
          onConvert={() => chooseQuality(720)}
        />
      )}

      <PlayerSheet open={sheet === 'tracks'} onClose={() => setSheet(null)}>
        {pb && (
          <>
            <SheetHeading>Audio</SheetHeading>
            {pb.media.audio.map((a) => (
              <SheetItem
                key={a.index}
                label={trackName(a, `Track ${a.index}`)}
                hint={a.channels > 2 ? `${a.channels} ch` : undefined}
                active={a.index === streamPlan?.audio}
                onPress={() => {
                  chooseAudio(a.index)
                  setSheet(null)
                }}
              />
            ))}
            <SheetHeading>Subtitles</SheetHeading>
            <SheetItem
              label="Off"
              active={subtitle === null}
              onPress={() => {
                chooseSubtitle(null)
                setSheet(null)
              }}
            />
            {pb.media.subtitles.map((s) => (
              <SheetItem
                key={s.id}
                label={trackName(s, s.id.startsWith('x') ? 'External' : `Track ${s.id.slice(1)}`)}
                hint={!s.supported ? 'image' : s.forced ? 'forced' : undefined}
                active={s.id === subtitle}
                disabled={!s.supported}
                onPress={() => {
                  chooseSubtitle(s.id)
                  setSheet(null)
                }}
              />
            ))}
          </>
        )}
      </PlayerSheet>

      <PlayerSheet open={sheet === 'settings'} onClose={() => setSheet(null)}>
        <SheetHeading>Speed</SheetHeading>
        <View className="flex-row flex-wrap gap-1 px-1 pb-2">
          {RATES.map((r) => (
            <SpeedChip key={r} rate={r} active={rate === r} onPress={() => changeRate(r)} />
          ))}
        </View>
        <SheetHeading>Picture</SheetHeading>
        <View className="px-1 pb-2">
          <Segmented
            size="sm"
            value={fill ? 'fill' : 'fit'}
            onChange={(v) => toggleFill(v === 'fill')}
            options={[
              { value: 'fit', label: 'Fit' },
              { value: 'fill', label: 'Fill' },
            ]}
          />
        </View>
        <SheetHeading>Quality</SheetHeading>
        {streamPlan && <Text className="font-sans px-2.5 pb-2 text-xs leading-5 text-ink-3">{streamPlan.describe}</Text>}
        {QUALITIES.map((q) => (
          <SheetItem
            key={q}
            label={q === 'auto' ? 'Automatic' : `${q}p`}
            hint={q === 'auto' ? 'best for this device' : undefined}
            active={quality === q}
            onPress={() => {
              chooseQuality(q)
              setSheet(null)
            }}
          />
        ))}
      </PlayerSheet>
    </View>
  )
}

function SpeedChip({ rate, active, onPress }: { rate: number; active: boolean; onPress: () => void }) {
  return (
    <Text
      onPress={() => {
        haptic('press')
        onPress()
      }}
      className={`font-sans overflow-hidden rounded-lg px-3 py-1.5 text-xs ${active ? 'bg-ink text-canvas' : 'bg-raised text-ink-2'}`}
      style={{ fontVariant: ['tabular-nums'] }}
    >
      {rate}×
    </Text>
  )
}
