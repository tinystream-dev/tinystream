// SPDX-License-Identifier: AGPL-3.0-or-later
// The book as one long page. Where the top of the screen is in it is kept in
// characters (packages/shared/src/books.ts), so another screen, or the app,
// opens on the same sentence.

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { ChevronLeft, Focus, List, Minus, Plus, Type } from 'lucide-react'
import { type CSSProperties, type ReactNode, memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import {
  type Block,
  type BookDocument,
  type Span,
  chapterAt,
  createProgressSaver,
  fileUrl,
  lengthOf,
  locate,
  percent,
  positionOf,
  styleOf,
} from '@tinystream/shared/books'
import { Empty, Page } from '../components/Page'
import { IconButton, Panel, Popover, Spinner } from '../components/ui'
import { request } from '../lib/api'
import { type BookDetail, SaveReading, contentQuery, patchReading } from './api'
import { setFocus, useFocus } from './focus'

const SIZES = [15, 16, 17, 18, 20, 22, 24, 27]
const SIZE_KEY = 'reader.size'

function useTextSize() {
  const [size, setSize] = useState(() => {
    const saved = Number(typeof localStorage === 'undefined' ? NaN : localStorage.getItem(SIZE_KEY))
    return SIZES.includes(saved) ? saved : 18
  })
  const step = (by: number) => {
    const next = SIZES[Math.max(0, Math.min(SIZES.length - 1, SIZES.indexOf(size) + by))]
    setSize(next)
    localStorage.setItem(SIZE_KEY, String(next))
  }
  return { size, smaller: () => step(-1), larger: () => step(1), smallest: size === SIZES[0], largest: size === SIZES[SIZES.length - 1] }
}

function SpanView({ span, onJump }: { span: Span; onJump: (block: number) => void }) {
  const s = styleOf(span.f)
  const className = [s.bold && 'font-semibold', s.italic && 'italic', s.underline && 'underline', s.strike && 'line-through', s.code && 'font-mono text-[0.92em]']
    .filter(Boolean)
    .join(' ')
  let node: ReactNode = className ? <span className={className}>{span.t}</span> : span.t
  if (s.sup) node = <sup>{node}</sup>
  if (s.sub) node = <sub>{node}</sub>
  const link = 'underline decoration-line-strong underline-offset-[0.2em] transition-colors hover:decoration-ink'
  if (span.href)
    return (
      <a href={span.href} target="_blank" rel="noreferrer noopener" className={link}>
        {node}
      </a>
    )
  if (span.to != null)
    return (
      <a
        href="#"
        className={link}
        onClick={(e) => {
          e.preventDefault()
          onJump(span.to!)
        }}
      >
        {node}
      </a>
    )
  return node
}

// Space above a block is padding, never margin: margins between blocks would collapse into whichever
// was laid out, so the heights measured of them wouldn't add up to the page.
const headings = ['', 'text-[1.6em] pt-[1.45em] mb-[0.9em]', 'text-[1.35em] pt-[1.15em] mb-[0.8em]', 'text-[1.15em] pt-[0.8em] mb-[0.7em]']

const BlockView = memo(function BlockView({ block: b, files, onJump }: { block: Block; files: string; onJump: (block: number) => void }) {
  const spans = b.s?.map((s, i) => <SpanView key={i} span={s} onJump={onJump} />)
  const align = b.c ? 'text-center' : ''
  switch (b.k) {
    case 'h':
      return <h2 className={`font-semibold leading-snug tracking-[-0.01em] text-balance whitespace-pre-line ${headings[b.l ?? 1] ?? 'pt-[0.5em] mb-[0.6em]'} ${align}`}>{spans}</h2>
    case 'quote':
      return (
        <blockquote className={`mb-[0.9em] border-l-2 border-line-strong pl-[1em] whitespace-pre-line text-ink-2 ${align}`} style={{ marginLeft: `${((b.l ?? 1) - 1) * 1}em` }}>
          {spans}
        </blockquote>
      )
    case 'pre':
      return <pre className="mb-[0.9em] overflow-x-auto font-mono text-[0.85em] leading-relaxed whitespace-pre-wrap">{spans}</pre>
    case 'li':
      return (
        <p className="relative mb-[0.4em] whitespace-pre-line" style={{ paddingLeft: `${(b.l ?? 1) * 1.25}em` }}>
          <span aria-hidden className="absolute text-ink-3" style={{ left: `${(b.l ?? 1) * 1.25 - 0.9}em` }}>
            •
          </span>
          {spans}
        </p>
      )
    case 'img':
      return (
        <figure className="mb-[1.6em] flex justify-center pt-[0.7em]">
          <img src={fileUrl(files, b.src ?? '')} alt={b.alt ?? ''} decoding="async" className="max-h-[85vh] max-w-full rounded-md" />
        </figure>
      )
    case 'hr':
      return (
        <div className="mb-[2.2em] pt-[1.3em]">
          <hr className="mx-auto w-16 border-line-strong" />
        </div>
      )
    default:
      return <p className={`mb-[0.9em] whitespace-pre-line ${align}`}>{spans}</p>
  }
})

type Window = { from: number; to: number }

/**
 * The blocks near the screen, between space standing in for the rest of the book. Its children are
 * the space above, blocks `from` to `to`, then the space below.
 */
const Text = memo(function Text({
  doc,
  win,
  above,
  below,
  files,
  onJump,
  articleRef,
}: {
  doc: BookDocument
  win: Window
  above: number
  below: number
  files: string
  onJump: (block: number) => void
  articleRef: React.RefObject<HTMLElement | null>
}) {
  return (
    <article ref={articleRef} className="reader-text">
      <div aria-hidden className="[overflow-anchor:none]" style={{ height: above }} />
      {doc.blocks.slice(win.from, win.to).map((b, i) => (
        <BlockView key={win.from + i} block={b} files={files} onJump={onJump} />
      ))}
      <div aria-hidden className="[overflow-anchor:none]" style={{ height: below }} />
    </article>
  )
})

/** Screens of text kept laid out above the reading line, and below it. */
const BEHIND = 1.5
const AHEAD = 3

/**
 * How tall each block is at one text size and column width: as measured once it's been on screen,
 * guessed from its length (and how far off the guesses have run) until then.
 */
function heightsOf(doc: BookDocument, em: number, width: number) {
  const n = doc.blocks.length
  const sizes = new Float64Array(n)
  const known = new Uint8Array(n)
  const perLine = Math.max(12, width / (em * 0.5))
  let measured = 0
  let guessed = 0
  const rough = (i: number) => {
    const b = doc.blocks[i]
    if (b.k === 'img') return innerHeight * 0.5
    if (b.k === 'hr') return em * 3.5
    const scale = b.k === 'h' ? 1.4 : 1
    return Math.ceil((lengthOf(doc, i) * scale) / perLine) * em * 1.75 * scale + em * 0.9
  }
  // A guess is kept once made, so the space it stands for doesn't change under the reader.
  const of = (i: number) => sizes[i] || (sizes[i] = rough(i) * (guessed > 0 ? measured / guessed : 1))
  const learn = (i: number, h: number) => {
    if (!known[i] && doc.blocks[i].k !== 'img') {
      measured += h
      guessed += rough(i)
    }
    sizes[i] = h
    known[i] = 1
  }
  const sum = (a: number, b: number) => {
    let s = 0
    for (let i = a; i < b; i++) s += of(i)
    return s
  }
  /** The window to lay out with block `i` on the reading line. */
  const around = (i: number): Window => {
    let from = i
    for (let h = 0; from > 0 && h < innerHeight * BEHIND; ) h += of(--from)
    let to = i
    for (let h = 0; to < n && h < innerHeight * AHEAD; ) h += of(to++)
    return { from, to }
  }
  /** The block `y` pixels down the book, by the sizes known and guessed. */
  const at = (y: number) => {
    let i = 0
    while (i < n - 1 && y >= of(i)) y -= of(i++)
    return i
  }
  return { em, width, of, learn, sum, around, at }
}

type Heights = ReturnType<typeof heightsOf>

function Contents({ doc, position, onJump }: { doc: BookDocument; position: number; onJump: (block: number) => void }) {
  const here = chapterAt(doc.chapters, position)
  return (
    <Popover
      align="end"
      trigger={({ toggle }) => (
        <IconButton label="Contents" onClick={toggle} disabled={doc.chapters.length === 0}>
          <List className="size-4.5" />
        </IconButton>
      )}
    >
      {(close) => (
        <Panel className="max-h-[70vh] w-80 overflow-y-auto p-1.5">
          {doc.chapters.map((c, i) => (
            <button
              key={i}
              ref={(el) => {
                // Scrolls the list alone: scrollIntoView would scroll the book behind it too.
                const list = el?.parentElement
                if (!el || !list || c !== here) return
                list.scrollTop += el.getBoundingClientRect().top - list.getBoundingClientRect().top - (list.clientHeight - el.offsetHeight) / 2
              }}
              onClick={() => {
                close()
                onJump(c.block)
              }}
              className={`flex w-full items-baseline gap-3 rounded-lg px-2.5 py-1.5 text-left text-sm transition-colors hover:bg-hover ${c === here ? 'bg-press text-ink' : 'text-ink-2'}`}
              style={{ paddingLeft: `${0.625 + c.depth * 0.9}rem` }}
            >
              <span className="min-w-0 flex-1 truncate">{c.title}</span>
              <span className="shrink-0 text-2xs text-ink-3 tabular">{percent(c.at, doc.length)}%</span>
            </button>
          ))}
        </Panel>
      )}
    </Popover>
  )
}

function TextSize({ text }: { text: ReturnType<typeof useTextSize> }) {
  return (
    <Popover
      align="end"
      trigger={({ toggle }) => (
        <IconButton label="Text size" onClick={toggle}>
          <Type className="size-4.5" />
        </IconButton>
      )}
    >
      {() => (
        <Panel className="flex items-center gap-1 p-1.5">
          <IconButton label="Smaller" onClick={text.smaller} disabled={text.smallest}>
            <Minus className="size-4" />
          </IconButton>
          <span className="w-10 text-center text-sm text-ink-2 tabular">{text.size}</span>
          <IconButton label="Larger" onClick={text.larger} disabled={text.largest}>
            <Plus className="size-4" />
          </IconButton>
        </Panel>
      )}
    </Popover>
  )
}

/** Whether the pointer's been still a while (or the screen untouched), in focus mode. */
function useIdlePointer(on: boolean) {
  const [idle, setIdle] = useState(false)
  useEffect(() => {
    if (!on) return setIdle(false)
    let timer = setTimeout(() => setIdle(true), 2000)
    const move = () => {
      setIdle(false)
      clearTimeout(timer)
      timer = setTimeout(() => setIdle(true), 2000)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerdown', move)
    return () => {
      clearTimeout(timer)
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerdown', move)
    }
  }, [on])
  return idle
}

/** Whether the browser keeps what's on screen in place when the page above it changes; the rest are helped by hand. */
const anchors = typeof CSS !== 'undefined' && CSS.supports('overflow-anchor', 'auto')

/** The block on the reading line `y`, as a child of the text (see `Text`). */
function childAt(kids: HTMLCollection, y: number) {
  let lo = 1
  let hi = kids.length - 2
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (kids[mid].getBoundingClientRect().bottom <= y) lo = mid + 1
    else hi = mid
  }
  return lo
}

export function Reader({ book }: { book: BookDetail }) {
  const qc = useQueryClient()
  const { data: doc, error } = useQuery(contentQuery(book.content))
  const focus = useFocus()
  const idle = useIdlePointer(focus)
  const text = useTextSize()
  const article = useRef<HTMLElement>(null)
  const bar = useRef<HTMLDivElement>(null)
  const start = useRef(book.reading && !book.reading.finished ? book.reading.position : 0)
  // Where the reading line is; the state only follows it as far as the bar shows.
  const here = useRef(start.current)
  const [position, setPosition] = useState(start.current)
  const [placed, setPlaced] = useState(false)
  const [hint, setHint] = useState(false)
  const [win, setWin] = useState<Window>({ from: 0, to: 0 })
  const shown = useRef(win)
  const heights = useRef<Heights | null>(null)
  // A position to put on the reading line once laid out, or a block to keep where it was in the page.
  const target = useRef<number | null>(null)
  const anchor = useRef<{ index: number; y: number } | null>(null)

  const saver = useMemo(
    () =>
      createProgressSaver(async (p) => {
        const r = await request(SaveReading, { id: book.id, position: p })
        patchReading(qc, book.id, r.saveReading.reading ?? null)
        void qc.invalidateQueries({ queryKey: ['books', 'home'] })
      }),
    [book.id, qc],
  )
  useEffect(() => saver.known(start.current), [saver])

  // Where the reading line is: just under the bar, or the top of the screen in focus mode.
  const line = useCallback(() => (bar.current && !focus ? bar.current.getBoundingClientRect().bottom : 0) + 12, [focus])

  /** The heights at this text size and column width, kept while those are. */
  const sizes = useCallback((): Heights | null => {
    const width = article.current?.clientWidth ?? 0
    if (!doc || !width) return null
    const h = heights.current
    return h && h.em === text.size && h.width === width ? h : (heights.current = heightsOf(doc, text.size, width))
  }, [doc, text.size])

  /** Remembers how tall the blocks laid out are (pictures once they've loaded). */
  const learn = useCallback(() => {
    const h = sizes()
    const kids = article.current?.children
    if (!h || !kids || !doc) return
    const { from } = shown.current
    let next = kids[kids.length - 1].getBoundingClientRect().top
    for (let k = kids.length - 2; k >= 1; k--) {
      const top = kids[k].getBoundingClientRect().top
      const img = doc.blocks[from + k - 1].k === 'img' && kids[k].querySelector('img')
      if (!img || img.complete) h.learn(from + k - 1, next - top)
      next = top
    }
  }, [doc, sizes])

  const measure = useCallback((): number | null => {
    const kids = article.current?.children
    if (!kids || !doc || kids.length < 3) return null
    const root = document.scrollingElement ?? document.documentElement
    if (shown.current.to === doc.blocks.length && root.scrollTop + innerHeight >= root.scrollHeight - 4 && root.scrollTop > 0) return doc.length
    const y = line()
    const k = childAt(kids, y)
    const r = kids[k].getBoundingClientRect()
    return positionOf(doc, shown.current.from + k - 1, r.height > 0 ? (y - r.top) / r.height : 0)
  }, [doc, line])

  /** Lays out the text around a position, then puts it on the reading line. */
  const goTo = useCallback(
    (p: number) => {
      const h = sizes()
      if (!h || !doc) return
      learn()
      target.current = p
      setWin(h.around(locate(doc, p).index))
    },
    [doc, sizes, learn],
  )

  const jump = useCallback((block: number) => doc && goTo(doc.blocks[block]?.at ?? 0), [doc, goTo])

  useLayoutEffect(() => {
    shown.current = win
    const kids = article.current?.children
    if (!kids || !doc) return
    learn()
    if (target.current !== null) {
      const { index, fraction } = locate(doc, target.current)
      const block = kids[index - win.from + 1]
      if (block) {
        const r = block.getBoundingClientRect()
        window.scrollTo({ top: scrollY + r.top + fraction * r.height - line(), behavior: 'instant' })
      }
      target.current = null
      setPlaced(true)
    } else if (anchor.current && !anchors) {
      const block = kids[anchor.current.index - win.from + 1]
      if (block) window.scrollBy({ top: block.getBoundingClientRect().top + scrollY - anchor.current.y, behavior: 'instant' })
    }
    anchor.current = null
  }, [win, doc, learn, line])

  // Opens where it was left.
  useLayoutEffect(() => {
    if (doc && !placed) goTo(start.current)
  }, [doc, placed, goTo])

  // A new text size moves everything (and focus mode moves the reading line): stay on the same sentence.
  const laid = useRef({ size: text.size, focus })
  useLayoutEffect(() => {
    if (laid.current.size === text.size && laid.current.focus === focus) return
    laid.current = { size: text.size, focus }
    if (placed) goTo(here.current)
  }, [text.size, focus, placed, goTo])

  useEffect(() => {
    let frame = 0
    const onResize = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        const width = article.current?.clientWidth
        if (width && heights.current && width !== heights.current.width) goTo(here.current)
      })
    }
    window.addEventListener('resize', onResize)
    return () => {
      cancelAnimationFrame(frame)
      window.removeEventListener('resize', onResize)
    }
  }, [goTo])

  useEffect(() => {
    if (!placed || !doc) return
    let frame = 0
    const onFrame = () => {
      const kids = article.current?.children
      const h = sizes()
      if (!kids || !h || target.current !== null) return
      const { from, to } = shown.current
      const y = line()
      const top = kids[0].getBoundingClientRect().bottom
      const bottom = kids[kids.length - 1].getBoundingClientRect().top
      if (y < top || y > bottom) {
        // Dragged somewhere not laid out yet: lay it out where the guesses put it.
        learn()
        return setWin(h.around(h.at(y - kids[0].getBoundingClientRect().top)))
      }
      if ((top > y - innerHeight * 0.5 && from > 0) || (bottom < y + innerHeight * 1.5 && to < doc.blocks.length)) {
        learn()
        const k = childAt(kids, y)
        anchor.current = { index: from + k - 1, y: kids[k].getBoundingClientRect().top + scrollY }
        setWin(h.around(from + k - 1))
      }
      const p = measure()
      if (p === null) return
      here.current = p
      saver.update(p)
      setPosition((was) => (percent(was, doc.length) === percent(p, doc.length) && chapterAt(doc.chapters, was) === chapterAt(doc.chapters, p) ? was : p))
    }
    const onScroll = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(onFrame)
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      cancelAnimationFrame(frame)
      window.removeEventListener('scroll', onScroll)
    }
  }, [placed, doc, sizes, learn, line, measure, saver])

  useEffect(() => {
    const leave = () => saver.flush()
    window.addEventListener('pagehide', leave)
    const hidden = () => document.visibilityState === 'hidden' && saver.flush()
    document.addEventListener('visibilitychange', hidden)
    return () => {
      window.removeEventListener('pagehide', leave)
      document.removeEventListener('visibilitychange', hidden)
      saver.flush()
    }
  }, [saver])

  useEffect(() => () => setFocus(false), [])

  useEffect(() => {
    if (!focus) return setHint(false)
    setHint(true)
    const t = setTimeout(() => setHint(false), 2200)
    return () => clearTimeout(t)
  }, [focus])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return
      if (e.key === 'f') {
        e.preventDefault()
        setFocus(!focus)
      } else if (e.key === 'Escape' && focus) {
        e.preventDefault()
        setFocus(false)
      } else if ((e.key === '=' || e.key === '+') && !focus) text.larger()
      else if (e.key === '-' && !focus) text.smaller()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [focus, text])

  if (error)
    return (
      <Page>
        <Empty title="This book can't be opened">{(error as Error).message}</Empty>
      </Page>
    )

  const h = heights.current
  const above = h ? h.sum(0, win.from) : 0
  const below = h && doc ? h.sum(win.to, doc.blocks.length) : 0
  const chapter = doc ? chapterAt(doc.chapters, position) : null
  const pct = doc ? percent(position, doc.length) : 0

  return (
    <div className={`min-h-dvh ${idle ? 'cursor-none' : ''}`}>
      {!focus && (
        <div ref={bar} className="sticky top-12 z-20 material-bar border-b border-line bg-canvas/85 backdrop-blur-xl md:top-0">
          <div className="mx-auto flex h-12 max-w-[60rem] items-center gap-1 px-3">
            <Link to="/book/$id" params={{ id: String(book.id) }} aria-label="Back to the book" className="grid size-8 shrink-0 place-items-center rounded-lg text-ink-2 transition-colors hover:bg-hover hover:text-ink">
              <ChevronLeft className="size-5" />
            </Link>
            <div className="min-w-0 flex-1 px-1.5">
              <p className="truncate text-[13px] font-medium">{book.title}</p>
              <p className="truncate text-xs text-ink-3">{chapter?.title ?? book.authors.join(', ')}</p>
            </div>
            <span className="px-2 text-xs text-ink-3 tabular">{pct}%</span>
            {doc && <Contents doc={doc} position={position} onJump={jump} />}
            <TextSize text={text} />
            <IconButton label="Focus (F)" onClick={() => setFocus(true)}>
              <Focus className="size-4.5" />
            </IconButton>
          </div>
          <div className="h-px bg-line">
            <div className="h-full bg-ink-2 transition-[width] duration-300" style={{ width: `${doc ? (position / Math.max(1, doc.length)) * 100 : 0}%` }} />
          </div>
        </div>
      )}
      {!doc ? (
        <div className="grid h-[60vh] place-items-center">
          <Spinner />
        </div>
      ) : (
        <div
          className={`mx-auto max-w-[38em] px-6 leading-[1.75] text-ink transition-opacity duration-150 ${focus ? 'py-[18vh]' : 'pt-10 pb-[40vh]'} ${placed ? 'opacity-100' : 'opacity-0'}`}
          style={{ fontSize: text.size } as CSSProperties}
        >
          <Text doc={doc} win={win} above={above} below={below} files={book.files} onJump={jump} articleRef={article} />
          <p className="mt-16 text-center text-sm text-ink-3 tabular">{pct >= 100 ? 'The end' : `${pct}% read`}</p>
        </div>
      )}
      {focus && (
        <button
          aria-label="Leave focus mode"
          onClick={() => setFocus(false)}
          className={`fixed top-4 right-4 z-50 grid size-9 place-items-center rounded-full bg-float/80 text-ink-2 backdrop-blur-md transition-opacity duration-500 hover:text-ink ${idle ? 'pointer-events-none opacity-0' : 'opacity-100'}`}
        >
          <Focus className="size-4.5" />
        </button>
      )}
      {hint && (
        <div className="pointer-events-none fixed inset-x-0 top-6 z-50 flex justify-center">
          <p className="animate-[fade-out_2.2s_ease-in_both] rounded-full bg-float/90 px-3.5 py-1.5 text-xs text-ink-2 shadow-lg backdrop-blur-md">Press Esc or F to leave focus mode</p>
        </div>
      )}
    </div>
  )
}
