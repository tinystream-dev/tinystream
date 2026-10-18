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
