// SPDX-License-Identifier: AGPL-3.0-or-later
// A book as both readers show it (the server's /api/books/:id/content), and
// where someone is in it. Positions count characters of text, so the same
// one lands on the same sentence on any screen and at any size.

export const BOLD = 1
export const ITALIC = 2
export const UNDERLINE = 4
export const STRIKE = 8
export const SUP = 16
export const SUB = 32
export const CODE = 64

/** A run of text in one style; `href` links out of the book, `to` to a block in it. */
export type Span = { t: string; f?: number; href?: string; to?: number }

export type Block = {
  k: 'p' | 'h' | 'quote' | 'pre' | 'li' | 'img' | 'hr'
  /** Characters before it. */
  at: number
  s?: Span[]
  /** A heading's level, or how deep a list item or quote is. */
  l?: number
  /** Centred. */
  c?: boolean
  /** An image's path in the book, under the book's `files`. */
  src?: string
  alt?: string
}

export type Chapter = { title: string; block: number; at: number; depth: number }

export type BookDocument = { length: number; chapters: Chapter[]; blocks: Block[] }

/** Characters in block `i`. */
export function lengthOf(doc: BookDocument, i: number): number {
  const next = i + 1 < doc.blocks.length ? doc.blocks[i + 1].at : doc.length
  return next - doc.blocks[i].at
}

/** The position `fraction` of the way through block `i`. */
export function positionOf(doc: BookDocument, i: number, fraction: number): number {
  const b = doc.blocks[i]
  if (!b) return 0
  return Math.min(doc.length, b.at + Math.round(Math.max(0, Math.min(1, fraction)) * lengthOf(doc, i)))
}

/** The block a position is in, and how far through it. */
export function locate(doc: BookDocument, position: number): { index: number; fraction: number } {
  const { blocks } = doc
  if (blocks.length === 0) return { index: 0, fraction: 0 }
  let lo = 0
  let hi = blocks.length - 1
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    if (blocks[mid].at <= position) lo = mid
    else hi = mid - 1
  }
  const length = lengthOf(doc, lo)
  return { index: lo, fraction: length > 0 ? Math.min(1, Math.max(0, (position - blocks[lo].at) / length)) : 0 }
}

/** Whole percent read: 100 only once it's all been read. */
export function percent(position: number, length: number): number {
  if (length <= 0) return 0
  if (position >= length) return 100
  return Math.min(99, Math.floor((position / length) * 100))
}

/** The chapter a position is in, if the book has any. */
export function chapterAt(chapters: Chapter[], position: number): Chapter | null {
  let found: Chapter | null = null
  for (const c of chapters) {
    if (c.at > position) break
    found = c
  }
  return found
}

/** Where an image in the book is: `files` (the book's) and its `src`. */
export function fileUrl(files: string, src: string): string {
  return files + src.split('/').map(encodeURIComponent).join('/')
}

/** The most characters a quote can have (the server's QUOTE_LENGTH). */
export const QUOTE_LENGTH = 3000

/** How many characters `s` has, counted as positions count them: by code point, not UTF-16 unit. */
export function chars(s: string): number {
  let n = 0
  for (const _ of s) n++
  return n
}

/** `s` from character `from` up to `to`, counted as positions count them. */
export function slice(s: string, from: number, to = Infinity): string {
  let n = 0
  let k = 0
  let a = s.length
  let b = s.length
  for (const c of s) {
    if (n === from) a = k
    if (n === to) {
      b = k
      break
    }
    k += c.length
    n++
  }
  return s.slice(a, b)
}

const textOfBlock = (b: Block) => (b.s ?? []).map((s) => s.t).join('')

/** The text from position `start` to `end`, its blocks a blank line apart (as the server cuts quotes). */
export function textOf(doc: BookDocument, start: number, end: number): string {
  const out: string[] = []
  for (let i = locate(doc, start).index; i < doc.blocks.length; i++) {
    const b = doc.blocks[i]
    if (b.at >= end) break
    if (b.k === 'img') continue
    const part = slice(textOfBlock(b), Math.max(0, start - b.at), end - b.at)
    if (part) out.push(part)
  }
  return out.join('\n\n')
}

const space = /\s/

/** The word at a position (or the nearest one in its block, between words), as positions. */
export function wordAt(doc: BookDocument, position: number): { start: number; end: number } | null {
  const { index } = locate(doc, position)
  const b = doc.blocks[index]
  if (!b || b.k === 'img') return null
  const t = [...textOfBlock(b)]
  if (t.length === 0) return null
  let i = Math.max(0, Math.min(t.length - 1, position - b.at))
  if (space.test(t[i])) {
    let j = i
    while (j < t.length && space.test(t[j])) j++
    if (j === t.length) {
      while (i > 0 && space.test(t[i])) i--
      if (space.test(t[i])) return null
    } else i = j
  }
  let a = i
  let z = i + 1
  while (a > 0 && !space.test(t[a - 1])) a--
  while (z < t.length && !space.test(t[z])) z++
  return { start: b.at + a, end: b.at + z }
}

/** A range widened to whole words at both ends. */
export function snapToWords(doc: BookDocument, start: number, end: number): { start: number; end: number } {
  const [from, to] = start <= end ? [start, end] : [end, start]
  const a = wordAt(doc, from)
  const z = wordAt(doc, Math.max(from, to - 1))
  return { start: Math.min(a?.start ?? from, z?.start ?? from), end: Math.max(a?.end ?? to, z?.end ?? to) }
}

/** A run of text in one style, and the marks over it. */
export type Piece<M> = { span: Span; marks: M[] }

/** A block's spans, cut where `marks` (ranges of positions) start and end. */
export function pieces<M extends { start: number; end: number }>(b: Block, marks: M[]): Piece<M>[] {
  const spans = b.s ?? []
  let at = b.at
  const end = at + spans.reduce((n, s) => n + chars(s.t), 0)
  const over = marks.filter((m) => m.start < end && m.end > at)
  if (over.length === 0) return spans.map((span) => ({ span, marks: [] }))
  const out: Piece<M>[] = []
  for (const span of spans) {
    const length = chars(span.t)
    const cuts = new Set([0, length])
    for (const m of over) {
      if (m.start > at && m.start < at + length) cuts.add(m.start - at)
      if (m.end > at && m.end < at + length) cuts.add(m.end - at)
    }
    const sorted = [...cuts].sort((x, y) => x - y)
    for (let k = 0; k + 1 < sorted.length; k++) {
      const [a, z] = [at + sorted[k], at + sorted[k + 1]]
      const piece = sorted.length === 2 ? span : { ...span, t: slice(span.t, sorted[k], sorted[k + 1]) }
      out.push({ span: piece, marks: over.filter((m) => m.start < z && m.end > a) })
    }
    at += length
  }
  return out
}

/** What a span's style flags make it, in CSS-ish terms both apps map to their own. */
export function styleOf(f = 0) {
  return {
    bold: (f & BOLD) !== 0,
    italic: (f & ITALIC) !== 0,
    underline: (f & UNDERLINE) !== 0,
    strike: (f & STRIKE) !== 0,
    sup: (f & SUP) !== 0,
    sub: (f & SUB) !== 0,
    code: (f & CODE) !== 0,
  }
}

/**
 * Keeps the server told where someone is without telling it on every
 * scroll: positions are sent `delay` ms after they stop changing, and
 * `flush` sends what's waiting straight away (when the reader closes).
 */
export function createProgressSaver(save: (position: number) => Promise<unknown>, delay = 1500) {
  let waiting: number | null = null
  let sent: number | null = null
  let timer: ReturnType<typeof setTimeout> | null = null

  const send = () => {
    timer = null
    if (waiting === null || waiting === sent) return
    const position = waiting
    sent = position
    waiting = null
    void save(position).catch(() => {
      // Tried again with whatever comes next.
      if (sent === position) sent = null
    })
  }

  return {
    update(position: number) {
      waiting = position
      if (timer) clearTimeout(timer)
      timer = setTimeout(send, delay)
    },
    flush() {
      if (timer) clearTimeout(timer)
      send()
    },
    /** Where the server already has them, so opening a book doesn't save it back straight away. */
    known(position: number) {
      sent = position
    },
  }
}

/** What a library's tile needs of a book to put it on a shelf. */
type Shelved = { series?: string | null; seriesIndex?: number | null; reading?: { position: number; finished: boolean; updatedAt: number } | null }

/** A book on its own, or a series' books stacked as one. */
export type Shelf<B> = { kind: 'book'; book: B } | { kind: 'series'; series: string; books: B[]; current: B; read: number }

/** The book a series is at: the one being read (the latest), or else the next one not read yet. */
export function currentOf<B extends Shelved>(books: B[]): B {
  const reading = books.filter((b) => b.reading && !b.reading.finished && b.reading.position > 0)
  if (reading.length) return reading.reduce((a, b) => (b.reading!.updatedAt > a.reading!.updatedAt ? b : a))
  const last = books.map((b) => !!b.reading?.finished).lastIndexOf(true)
  return books.slice(last + 1).find((b) => !b.reading?.finished) ?? books.find((b) => !b.reading?.finished) ?? books[0]
}

/** A series' books in order, the ones without a number last. */
export function inOrder<B extends Shelved>(books: B[]): B[] {
  return [...books].sort((a, b) => (a.seriesIndex ?? Infinity) - (b.seriesIndex ?? Infinity))
}

/** Books as they're shelved: a series with two or more of them is one stack, where the first of them is. */
export function shelve<B extends Shelved>(books: B[]): Shelf<B>[] {
  const series = new Map<string, B[]>()
  for (const b of books) {
    if (!b.series) continue
    const key = b.series.toLowerCase()
    series.set(key, [...(series.get(key) ?? []), b])
  }
  const out: Shelf<B>[] = []
  const placed = new Set<string>()
  for (const b of books) {
    const key = b.series?.toLowerCase()
    const all = key ? series.get(key)! : []
    if (all.length < 2) out.push({ kind: 'book', book: b })
    else if (!placed.has(key!)) {
      placed.add(key!)
      const ordered = inOrder(all)
      out.push({ kind: 'series', series: b.series!, books: ordered, current: currentOf(ordered), read: all.filter((v) => v.reading?.finished).length })
    }
  }
  return out
}
