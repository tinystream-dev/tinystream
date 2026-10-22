// SPDX-License-Identifier: AGPL-3.0-or-later

import { Maximize, Minimize, Pause, PictureInPicture2, Play } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { clock } from '@tinystream/shared/format'
import { Spinner } from '../components/ui'
import { music } from '../music/player'
import { ChromeButton, type Hud, HudView, Timeline, VolumeControl, pref } from './Player'

/**
 * A rendered clip, played with the player's own controls, cut down to what a
 * short file needs: no tracks, quality, chapters or episodes.
 */
export function ClipPlayer({ src, poster, muted: startMuted = false }: { src: string; poster?: string; muted?: boolean }) {
  const stage = useRef<HTMLDivElement>(null)
  const videoRef = useRef<HTMLVideoElement>(null)
  const suspended = useRef(false)
  useEffect(() => () => void (suspended.current && music.unsuspend()), [])
  const [playing, setPlaying] = useState(false)
  const [time, setTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [buffered, setBuffered] = useState<[number, number][]>([])
  const [waiting, setWaiting] = useState(true)
  const [error, setError] = useState(false)
  const [volume, setVolume] = useState(() => Number(pref.get('volume') ?? 1))
  const [muted, setMuted] = useState(startMuted)
  const [fullscreen, setFullscreen] = useState(false)
  const [idle, setIdle] = useState(false)
  const [hud, setHud] = useState<Hud | null>(null)

  const hudTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const flash = useCallback((h: Omit<Hud, 'key'>) => {
    clearTimeout(hudTimer.current)
    setHud((prev) => {
      const add = h.kind === 'seek' && prev?.kind === 'seek' && Math.sign(prev.amount ?? 0) === Math.sign(h.amount ?? 0)
      return { ...h, amount: add ? (prev!.amount ?? 0) + (h.amount ?? 0) : h.amount, key: (prev?.key ?? 0) + 1 }
    })
    hudTimer.current = setTimeout(() => setHud(null), 700)
  }, [])
  useEffect(() => () => clearTimeout(hudTimer.current), [])

  const idleTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const poke = useCallback(() => {
    setIdle(false)
    clearTimeout(idleTimer.current)
    idleTimer.current = setTimeout(() => setIdle(true), 2000)
  }, [])
  useEffect(() => () => clearTimeout(idleTimer.current), [])

  const togglePlay = useCallback(() => {
    const v = videoRef.current
    if (!v) return
    const want = v.paused
    if (want) void v.play()
    else v.pause()
    return want
  }, [])
  const seek = useCallback(
    (t: number) => {
      const v = videoRef.current
      if (!v) return
      const to = Math.max(0, Math.min(duration || Infinity, t))
      v.currentTime = to
      setTime(to)
      poke()
    },
    [duration, poke],
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
  const toggleFullscreen = useCallback(() => {
    if (document.fullscreenElement) void document.exitFullscreen()
    else void stage.current?.requestFullscreen()
  }, [])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target
      if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement || e.metaKey || e.ctrlKey || e.altKey) return
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
      else if (k === 'm') (handled(), setMuted((x) => !x), flash({ kind: 'volume', amount: v.muted ? v.volume : 0 }))
      else if (k === 'p') (handled(), togglePip())
      else if (k === 'f') (handled(), toggleFullscreen())
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [togglePlay, seekBy, flash, togglePip, toggleFullscreen, poke])

  useEffect(() => {
    const v = videoRef.current
    if (!v) return
    v.volume = volume
    v.muted = muted
    if (!startMuted) pref.set('volume', String(volume))
  }, [volume, muted, startMuted])

  useEffect(() => {
    const onFs = () => setFullscreen(document.fullscreenElement === stage.current)
    document.addEventListener('fullscreenchange', onFs)
    return () => document.removeEventListener('fullscreenchange', onFs)
  }, [])

  const showChrome = !playing || !idle || error
  return (
    <div
      ref={stage}
      onPointerMove={(e) => e.pointerType === 'mouse' && poke()}
      onPointerDown={poke}
      className={`relative size-full bg-media-shade select-none ${showChrome ? '' : 'cursor-none'}`}
    >
      <video
        ref={videoRef}
        src={src}
        poster={poster}
        autoPlay
        playsInline
        muted={startMuted}
        className="absolute inset-0 size-full object-contain"
        onClick={() => flash({ kind: togglePlay() ? 'play' : 'pause' })}
        onDoubleClick={toggleFullscreen}
        onPlay={() => {
          setPlaying(true)
          // Music steps aside while a clip plays, and comes back paused after.
          music.suspend()
          suspended.current = true
        }}
        onPause={() => setPlaying(false)}
        onWaiting={() => setWaiting(true)}
        onPlaying={() => setWaiting(false)}
        onCanPlay={() => setWaiting(false)}
        onLoadedMetadata={(e) => setDuration(e.currentTarget.duration)}
        onTimeUpdate={(e) => {
          const v = e.currentTarget
          setTime(v.currentTime)
          const b = v.buffered
          const r: [number, number][] = []
          for (let i = 0; i < b.length; i++) r.push([b.start(i), b.end(i)])
          setBuffered(r)
        }}
        onError={() => setError(true)}
      />

      {waiting && !error && (
        <div className="pointer-events-none absolute inset-0 grid place-items-center text-media-ink/80">
          <Spinner className="size-8" />
        </div>
      )}
      {error && (
        <div className="pointer-events-none absolute inset-0 grid place-items-center p-6 text-center text-sm text-media-ink/70">
          The browser could not play this clip
        </div>
      )}

      {hud && <HudView hud={hud} />}

      <div
        className={`absolute inset-x-0 bottom-0 bg-linear-to-t from-media-shade/80 via-media-shade/40 to-transparent px-3 pt-14 pb-2 transition-opacity duration-300 ${showChrome ? 'opacity-100' : 'pointer-events-none opacity-0'}`}
      >
        <Timeline time={time} duration={duration} buffered={buffered} chapters={[]} onSeek={seek} />
        <div className="mt-1 flex items-center gap-1">
          <ChromeButton label={playing ? 'Pause (k)' : 'Play (k)'} onClick={() => void togglePlay()}>
            {playing ? <Pause className="size-5 fill-current" /> : <Play className="size-5 fill-current" />}
          </ChromeButton>
          <VolumeControl volume={volume} muted={muted} setVolume={setVolume} setMuted={setMuted} />
          <span className="ml-2 text-[13px] text-media-ink/80 tabular">
            {clock(time)} <span className="text-media-ink/40">/ {clock(duration)}</span>
          </span>
          <div className="flex-1" />
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
    </div>
  )
}
