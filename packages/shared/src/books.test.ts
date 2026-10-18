// SPDX-License-Identifier: AGPL-3.0-or-later

import { expect, test } from 'bun:test'
import { type BookDocument, chapterAt, createProgressSaver, fileUrl, locate, percent, positionOf } from './books'

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
