// SPDX-License-Identifier: AGPL-3.0-or-later
// The music player, out of the way: a pill at the bottom that opens into a
// card with everything you'd reach for, and into Now Playing from there.

import { Link } from '@tanstack/react-router'
import { Captions, ListMusic, Maximize2, SkipForward, X } from 'lucide-react'
import { type CSSProperties, type PointerEvent, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react'
import { createPortal, flushSync } from 'react-dom'
import { cover } from './api'
import { Artists, Cover, QualityBadge, StarButton } from './components'
import { Elsewhere, PlayButton, Scrubber, Transport, Volume } from './controls'
import { ART, NowPlaying, type Tab } from './NowPlaying'
import { current, music, usePlayer, usePosition } from './player'

type View = { open: 'pill' | 'card' | 'full'; tab: Tab }

let view: View = { open: 'pill', tab: 'lyrics' }
const watchers = new Set<() => void>()

function setView(v: Partial<View>, morph = false) {
  const apply = () => {
    view = { ...view, ...v }
    watchers.forEach((f) => f())
  }
  // The cover grows from where it is into where it's going.
  const doc = document as Document & { startViewTransition?: (f: () => void) => { finished: Promise<void> } }
  if (morph && doc.startViewTransition && !matchMedia('(prefers-reduced-motion: reduce)').matches) {
    // Not a navigation: the page under Now Playing holds still.
    const root = document.documentElement
    root.dataset.morph = v.open === 'full' ? 'open' : 'close'
    doc.startViewTransition(() => flushSync(apply)).finished.finally(() => delete root.dataset.morph)
  } else apply()
}

export const useView = () =>
  useSyncExternalStore(
    (f) => (watchers.add(f), () => watchers.delete(f)),
    () => view,
    () => view,
  )

// The big cover has to be there when it's captured, or it slides in as nothing.
function preload(src: string | undefined) {
  if (!src) return Promise.resolve()
  const img = new Image()
  img.src = src
  return img.decode().catch(() => {})
}

/** Opens Now Playing on a tab, from anywhere. */
export function openNowPlaying(tab: Tab = view.tab) {
  const art = preload(cover(current()?.track.cover, ART))
  void Promise.race([art, new Promise((r) => setTimeout(r, 300))]).then(() => setView({ open: 'full', tab }, true))
}

export function closeNowPlaying() {
  setView({ open: 'pill' }, true)
}

function Progress({ tint }: { tint: string | null }) {
  const time = usePosition()
  const length = usePlayer((s) => current(s)?.track.duration ?? 0)
  return (
    <div className="pointer-events-none absolute inset-x-5 bottom-0 h-[2px] overflow-hidden rounded-full bg-ink/10">
      <div className="h-full rounded-full bg-ink/70" style={{ width: `${length ? Math.min(100, (time / length) * 100) : 0}%`, background: tint ? `rgb(${tint})` : undefined }} />
    </div>
  )
}

export function MusicPill() {
  const entry = usePlayer(current)
  const playing = usePlayer((s) => s.playing)
  const hidden = usePlayer((s) => s.dismissed || s.suspended || s.queue.length === 0)
  const next = usePlayer((s) => s.queue[s.index + 1]?.track ?? null)
  const error = usePlayer((s) => s.error)
  const room = usePlayer((s) => s.room != null)
  const remote = usePlayer((s) => s.remote)
  const { open, tab } = useView()
  const track = entry?.track ?? null
  const tint = track?.coverTint ?? null
  const inner = useRef<HTMLDivElement>(null)
  const root = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState<{ w: number; h: number } | null>(null)
  const [drag, setDrag] = useState<{ y: number; start: number } | null>(null)
  const [leaving, setLeaving] = useState(false)
  // Keeps showing the last track while the pill goes away.
  const last = useRef(track)
  if (track) last.current = track
  const shown = track ?? last.current
  const card = open === 'card'

  useLayoutEffect(() => {
    const el = inner.current
    if (!el) return
    const measure = () => setSize({ w: el.offsetWidth, h: el.offsetHeight })
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [!shown])

  useEffect(() => {
    if (!card) return
    void preload(cover(current()?.track.cover, ART))
    const away = (e: globalThis.PointerEvent) => !root.current?.contains(e.target as Node) && setView({ open: 'pill' })
    const key = (e: KeyboardEvent) => e.key === 'Escape' && setView({ open: 'pill' })
    document.addEventListener('pointerdown', away)
    window.addEventListener('keydown', key)
    return () => {
      document.removeEventListener('pointerdown', away)
      window.removeEventListener('keydown', key)
    }
  }, [card])

  useEffect(() => {
    if (hidden && open !== 'pill') setView({ open: 'pill' })
  }, [hidden, open])

  if (typeof document === 'undefined' || !shown) return null
  const visible = !hidden && !leaving && open !== 'full'

  // Pulled down while paused, it goes away; while playing it springs back.
  const onDown = (e: PointerEvent) => {
    if (card || (e.target as HTMLElement).closest('button,a')) return
    setDrag({ y: 0, start: e.clientY })
    e.currentTarget.setPointerCapture(e.pointerId)
  }
  const onMove = (e: PointerEvent) => {
    if (!drag) return
    const dy = e.clientY - drag.start
    setDrag({ ...drag, y: dy > 0 ? (playing ? dy * 0.25 : dy) : dy * 0.15 })
  }
  const onUp = () => {
    if (!drag) return
    const moved = Math.abs(drag.y) > 4
    if (drag.y > 36 && !playing) {
      setLeaving(true)
      setTimeout(() => {
        music.dismiss()
        setLeaving(false)
      }, 260)
    } else if (!moved) setView({ open: 'card' })
    setDrag(null)
  }

  const radius = card ? 26 : (size?.h ?? 52) / 2
  return createPortal(
    <>
      <div
        ref={root}
        className="pointer-events-none fixed bottom-[calc(4.25rem+env(safe-area-inset-bottom)+0.625rem)] left-1/2 z-[44] -translate-x-1/2 [view-transition-name:music] md:bottom-5 md:left-[calc(50%+7.5rem)]"
      >
        <div
          className={`lift origin-bottom transition-[opacity,transform,translate,scale,filter] duration-[360ms] ease-[cubic-bezier(.2,.8,.2,1)] ${
            visible ? 'pointer-events-auto opacity-100' : 'pointer-events-none translate-y-6 scale-90 opacity-0 blur-[3px]'
          }`}
          style={{
            '--lift-radius': `${radius}px`,
            transform: drag ? `translateY(${drag.y}px) scale(${1 - Math.min(0.06, Math.abs(drag.y) / 900)})` : undefined,
            transition: drag ? 'none' : undefined,
            opacity: drag && drag.y > 0 && !playing ? Math.max(0.3, 1 - drag.y / 160) : undefined,
          } as CSSProperties}
        >
          <div
            className="material relative flex items-end justify-center overflow-hidden bg-float inset-ring inset-ring-glow/8 transition-[width,height,border-radius] duration-[480ms] ease-[cubic-bezier(.3,1.2,.4,1)]"
            style={size ? { width: size.w, height: size.h, borderRadius: radius } : { borderRadius: 26 }}
          >
            {tint && (
              <div
                aria-hidden
                className="pointer-events-none absolute inset-0 transition-opacity duration-700"
                style={{ background: `radial-gradient(130% 160% at ${card ? '50% 0%' : '0% 50%'}, rgb(${tint} / 0.3), transparent 62%)` }}
              />
            )}
            <div ref={inner} className="relative w-max shrink-0">
              {card ? (
                <div key="card" className="w-[min(22rem,calc(100vw-1.5rem))] animate-[fade_260ms_ease-out] p-4">
                  <button
                    onClick={() => openNowPlaying('lyrics')}
                    className="block w-full outline-none"
                    aria-label="Open Now Playing"
                  >
                    <Cover src={shown.cover} size={320} className="aspect-square w-full shadow-[0_18px_40px_-18px_var(--color-shade)] [view-transition-name:now-playing]" />
                  </button>
                  <div className="mt-4 flex items-start gap-2">
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[17px] leading-snug font-semibold tracking-tight">{shown.title}</p>
                      <p className="truncate text-[13px] text-ink-2">
                        <Artists track={shown} />
                        {shown.albumId != null && (
                          <>
                            {' · '}
                            <Link to="/album/$id" params={{ id: String(shown.albumId) }} onClick={() => setView({ open: 'pill' })} className="hover:text-ink hover:underline">
                              {shown.album}
                            </Link>
                          </>
                        )}
                      </p>
                    </div>
                    <StarButton kind="TRACK" id={shown.id} starred={shown.starred} className="-mt-1 -mr-1" />
                  </div>
                  <Elsewhere className="mt-3" />
                  <div className="mt-3">
                    <Scrubber tint={tint} />
                  </div>
                  <div className="mt-1.5">
                    <Transport />
                  </div>
                  <div className="mt-3 flex items-center gap-1">
                    <div className="w-28">
                      <Volume />
                    </div>
                    <div className="flex-1" />
                    <QualityBadge track={shown} />
                    <button aria-label="Lyrics" onClick={() => openNowPlaying('lyrics')} className="grid size-8 place-items-center rounded-full text-ink-3 hover:bg-hover hover:text-ink">
                      <Captions className="size-4" />
                    </button>
                    <button aria-label="Queue" onClick={() => openNowPlaying('queue')} className="grid size-8 place-items-center rounded-full text-ink-3 hover:bg-hover hover:text-ink">
                      <ListMusic className="size-4" />
                    </button>
                    <button aria-label="Open Now Playing" onClick={() => openNowPlaying()} className="grid size-8 place-items-center rounded-full text-ink-3 hover:bg-hover hover:text-ink">
                      <Maximize2 className="size-4" />
                    </button>
                  </div>
                  {next && (
                    <p className="mt-3 truncate border-t border-line pt-3 text-xs text-ink-3">
                      Next: <span className="text-ink-2">{next.title}</span> · {next.artist}
                    </p>
                  )}
                </div>
              ) : (
                <div
                  key="pill"
                  onPointerDown={onDown}
                  onPointerMove={onMove}
                  onPointerUp={onUp}
                  onPointerCancel={() => setDrag(null)}
                  className="group flex h-[52px] max-w-[calc(100vw-1.5rem)] animate-[fade_260ms_ease-out] cursor-pointer touch-none items-center gap-3 pr-2 pl-[7px] select-none"
                >
                  <Cover src={shown.cover} size={38} round className={`size-[38px] shrink-0 ${open === 'pill' ? '[view-transition-name:now-playing]' : ''}`} />
                  <div className="w-[min(13rem,calc(100vw-13.5rem))] min-w-0">
                    <p key={shown.id} className="animate-[fade_300ms_ease-out] truncate text-[13px] leading-tight font-medium">
                      {error ?? shown.title}
                    </p>
                    <p className="truncate text-xs leading-tight text-ink-3">{remote ? `${remote.paused ? 'Paused' : 'Playing'} on ${remote.client}` : shown.artist}</p>
                  </div>
                  <PlayButton size="sm" />
                  <button
                    aria-label="Next"
                    disabled={!next}
                    onClick={() => music.next()}
                    className="grid size-9 place-items-center rounded-full text-ink-2 transition-[color,scale] hover:text-ink active:scale-90 disabled:opacity-30"
                  >
                    <SkipForward className="size-4.5 fill-current" />
                  </button>
                  {!room && (
                    <button
                      aria-label="Close the player"
                      onClick={() => {
                        music.pause()
                        setLeaving(true)
                        setTimeout(() => {
                          music.dismiss()
                          setLeaving(false)
                        }, 260)
                      }}
                      className="-ml-2 grid size-9 place-items-center rounded-full text-ink-3 transition-[color,scale,background-color] hover:bg-hover hover:text-ink active:scale-90"
                    >
                      <X className="size-4.5" />
                    </button>
                  )}
                  <Progress tint={tint} />
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
      {open === 'full' && !hidden && <NowPlaying tab={tab} onTab={(t) => setView({ tab: t })} onClose={closeNowPlaying} />}
    </>,
    document.body,
  )
}
