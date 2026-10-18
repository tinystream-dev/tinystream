// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { type BookDocument, chapterAt, chars, createProgressSaver, currentOf, fileUrl, locate, percent, pieces, positionOf, shelve, slice, snapToWords, textOf, wordAt } from './books'

const doc: BookDocument = {
  length: 30,
  chapters: [
    { title: 'One', block: 0, at: 0, depth: 0 },
    { title: 'Two', block: 3, at: 20, depth: 0 },
  ],
  blocks: [
    { k: 'h', at: 0, s: [{ t: 'One' }] },
    { k: 'p', at: 3, s: [{ t: 'Seventeen letters' }] },
    { k: 'hr', at: 20 },
    { k: 'p', at: 20, s: [{ t: 'Ten chars.' }] },
  ],
}

test('positions go to blocks and back', () => {
  expect(positionOf(doc, 1, 0.5)).toBe(12)
  expect(locate(doc, 12)).toEqual({ index: 1, fraction: 9 / 17 })
  expect(locate(doc, 0)).toEqual({ index: 0, fraction: 0 })
  // An empty block starts where the next does: that one holds the position.
  expect(locate(doc, 20).index).toBe(3)
  expect(locate(doc, 30)).toEqual({ index: 3, fraction: 1 })
  expect(positionOf(doc, 3, 2)).toBe(30)
})

test('percent reaches 100 only at the end', () => {
  expect(percent(0, 30)).toBe(0)
  expect(percent(29, 30)).toBe(96)
  expect(percent(299, 300)).toBe(99)
  expect(percent(30, 30)).toBe(100)
  expect(percent(5, 0)).toBe(0)
})

test('finds the chapter', () => {
  expect(chapterAt(doc.chapters, 19)?.title).toBe('One')
  expect(chapterAt(doc.chapters, 25)?.title).toBe('Two')
  expect(chapterAt([], 5)).toBeNull()
})

test('encodes file paths', () => {
  expect(fileUrl('/api/books/1/files/', 'OEBPS/Images/a b#.jpg')).toBe('/api/books/1/files/OEBPS/Images/a%20b%23.jpg')
})

test('saves once scrolling stops, and on flush', async () => {
  const saved: number[] = []
  const saver = createProgressSaver(async (p) => void saved.push(p), 10)
  saver.known(5)
  saver.update(5)
  await new Promise((r) => setTimeout(r, 20))
  expect(saved).toEqual([])
  saver.update(6)
  saver.update(7)
  await new Promise((r) => setTimeout(r, 20))
  expect(saved).toEqual([7])
  saver.update(9)
  saver.flush()
  expect(saved).toEqual([7, 9])
})

const quoted: BookDocument = {
  length: 69,
  chapters: [],
  blocks: [
    { k: 'p', at: 0, s: [{ t: 'Call me Ishmael.' }] },
    { k: 'img', at: 16, src: 'whale.png' },
    { k: 'p', at: 17, s: [{ t: 'Some years ago, ' }, { t: 'never mind', f: 2 }, { t: ' how long.' }] },
    { k: 'p', at: 53, s: [{ t: 'Call me Ishmael.' }] },
  ],
}

test('quotes are cut as the server cuts them', () => {
  expect(textOf(quoted, 8, 31)).toBe('Ishmael.\n\nSome years ago')
  expect(textOf(quoted, 33, 43)).toBe('never mind')
})

test('characters are counted by code point', () => {
  expect(chars('a😀b')).toBe(3)
  expect(slice('a😀bc', 1, 3)).toBe('😀b')
  expect(slice('abc', 5)).toBe('')
})

test('selections snap to whole words', () => {
  expect(wordAt(quoted, 10)).toEqual({ start: 8, end: 16 })
  expect(wordAt(quoted, 21)).toEqual({ start: 22, end: 27 })
  expect(snapToWords(quoted, 10, 19)).toEqual({ start: 8, end: 21 })
  expect(snapToWords(quoted, 19, 10)).toEqual({ start: 8, end: 21 })
})

test('marks cut spans where they start and end', () => {
  const b = quoted.blocks[2]
  const cut = pieces(b, [{ start: 22, end: 38, id: 1 }])
  expect(cut.map((p) => [p.span.t, p.marks.length])).toEqual([
    ['Some ', 0],
    ['years ago, ', 1],
    ['never', 1],
    [' mind', 0],
    [' how long.', 0],
  ])
  expect(cut[2].span.f).toBe(2)
  expect(pieces(b, [{ start: 0, end: 5, id: 1 }])[0].span).toBe(b.s![0])
})

test('a series is one stack, at the next book to read', () => {
  const book = (id: number, series: string | null, seriesIndex: number | null, reading: { position: number; finished: boolean; updatedAt: number } | null = null) => ({ id, series, seriesIndex, reading })
  const read = { position: 10, finished: true, updatedAt: 1 }
  const shelves = shelve([book(1, 'Dune', 2, read), book(2, 'Moby Dick', null), book(3, 'dune', 1, read), book(4, 'Dune', 3), book(5, 'Solo', 1)])
  expect(shelves.map((s) => (s.kind === 'book' ? s.book.id : s.books.map((b) => b.id)))).toEqual([[3, 1, 4], 2, 5])
  const dune = shelves[0]
  expect(dune.kind === 'series' && [dune.current.id, dune.read]).toEqual([4, 2])
  expect(currentOf([book(1, 'A', 1, read), book(2, 'A', 2, { position: 5, finished: false, updatedAt: 9 }), book(3, 'A', 3)]).id).toBe(2)
  expect(currentOf([book(1, 'A', 1), book(2, 'A', 2, read), book(3, 'A', 3, read)]).id).toBe(1)
})
