// SPDX-License-Identifier: AGPL-3.0-or-later

import { Link } from '@tanstack/react-router'
import { type CSSProperties, type MouseEvent, useEffect, useState } from 'react'
import type { Card } from '../lib/api'
import { useFetching } from '../lib/refreshing'
import { useTilt } from '../lib/tilt'
import { Squircle } from './Squircle'
import { Img } from './Img'

/** Initials on a quiet surface, for titles without artwork. */
function Fallback({ title }: { title: string }) {
  return (
    <div className="flex size-full items-end bg-panel p-3">
      <span className="line-clamp-3 text-[15px] leading-snug font-medium text-ink-2">{title}</span>
    </div>
  )
}

/**
 * Names the clicked artwork for the view transition, so it grows into the
 * page's matching artwork: a poster into the title page's (`poster`), an
 * episode still into the player (`still`). Only the one clicked gets the
 * name: two elements with the same name would cancel the transition.
 */
export function morphFrom(e: MouseEvent<HTMLElement>, name = 'poster') {
  const art = e.currentTarget.querySelector<HTMLElement>('[data-morph]')
  if (!art) return
  art.style.viewTransitionName = name
  holdScroll()
}

/**
 * The morph's destination is measured once, when the transition starts, but
 * the page underneath keeps scrolling: a flick still coasting would drag the
 * title page away and the poster would snap to it at the end. So stop any
 * scroll in flight and swallow new input until the transition is over.
 */
function holdScroll() {
  window.scrollTo({ top: window.scrollY, left: window.scrollX, behavior: 'instant' })
  const block = (e: Event) => e.preventDefault()
  const opts = { capture: true, passive: false }
  window.addEventListener('wheel', block, opts)
  window.addEventListener('touchmove', block, opts)
  let released = false
  const release = () => {
    if (released) return
    released = true
    window.removeEventListener('wheel', block, opts)
    window.removeEventListener('touchmove', block, opts)
  }
  // The router starts the transition a moment after the click; wait for it,
  // and let go once it settles. The timeout covers browsers without
  // `activeViewTransition` and a transition that never begins.
  const fallback = setTimeout(release, 800)
  let frames = 0
  const watch = () => {
    const vt = (document as { activeViewTransition?: ViewTransition | null }).activeViewTransition
    if (vt) {
      clearTimeout(fallback)
      vt.finished.finally(release)
    } else if (++frames < 30) requestAnimationFrame(watch)
  }
  requestAnimationFrame(watch)
}

/** `caption` replaces the watched count under the title. */
export function Poster({ card, caption }: { card: Card; caption?: string }) {
  const [broken, setBroken] = useState(false)
  useEffect(() => setBroken(false), [card.poster])
  const done = card.videoCount > 0 && card.watchedCount >= card.videoCount
  const unwatched = card.kind === 'SHOW' ? card.videoCount - card.watchedCount : 0
  const fresh = card.freshCount > 0 && !done
  const fetching = useFetching(card.id)
  const tilt = useTilt<HTMLDivElement>()
  return (
    <Link
      to="/title/$id"
      params={{ id: String(card.id) }}
      viewTransition
      onClick={morphFrom}
      className="group block min-w-0 outline-none"
      aria-label={card.name}
      data-fetching={fetching || undefined}
      {...tilt.handlers}
    >
      <div className="rounded-[15px] outline-offset-3 transition-transform duration-200 ease-out group-hover:-translate-y-0.5 group-focus-visible:-translate-y-0.5 group-focus-visible:outline-2 group-focus-visible:outline-glow/70">
        <div ref={tilt.ref} className="tilt">
        <Squircle radius={14} edge className="aspect-[2/3] bg-raised" data-morph>
          <div className="refreshable size-full" style={{ '--blur': '12px', '--scale': 1.08 } as CSSProperties}>
            {card.poster && !broken ? (
              <Img
                src={card.poster}
                loading="lazy"
                decoding="async"
                onError={() => setBroken(true)}
                className="size-full object-cover"
              />
            ) : (
              <Fallback title={card.name} />
            )}
          </div>
          <div className="sheen" />
          <div className="glare" />
          <div className="absolute inset-0 bg-media-ink/0 transition-colors group-hover:bg-media-ink/[0.04]" />
          {card.progress !== null && !done && (
            <div className="absolute inset-x-2.5 bottom-2.5 h-1 overflow-hidden rounded-full bg-media-shade/50">
              <div className="h-full bg-media-ink" style={{ width: `${Math.max(4, card.progress * 100)}%` }} />
            </div>
          )}
          {fresh ? (
            <span className="absolute top-2 right-2 flex items-center gap-1 rounded-md bg-media-shade/60 px-1.5 py-0.5 text-2xs font-medium text-media-ink backdrop-blur-md tabular">
              <span className="size-1.5 rounded-full bg-warn" />
              {card.kind === 'SHOW' && card.freshCount > 1 ? `${card.freshCount} new` : 'New'}
            </span>
          ) : (
            card.kind === 'SHOW' &&
            card.watchedCount > 0 &&
            unwatched > 0 && (
              <span className="absolute top-2 right-2 rounded-md bg-media-shade/60 px-1.5 py-0.5 text-2xs font-medium text-media-ink backdrop-blur-md tabular">
                {unwatched}
              </span>
            )
          )}
        </Squircle>
        </div>
      </div>
      <p className="refreshable mt-2 truncate text-[13px] font-medium text-ink" style={{ '--i': 1 } as CSSProperties}>{card.name}</p>
      <p className="truncate text-xs text-ink-3 tabular">
        {caption ??
          (done
            ? 'Watched'
            : card.kind === 'SHOW'
              ? card.watchedCount > 0
                ? (card.year ?? '')
                : `${card.videoCount} episode${card.videoCount === 1 ? '' : 's'}`
              : (card.year ?? 'Movie'))}
      </p>
    </Link>
  )
}
