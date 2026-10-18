// SPDX-License-Identifier: AGPL-3.0-or-later
// Books: what the library, book page and reader ask the server, and keeping
// what they show in step when someone reads on.

import type { QueryClient } from '@tanstack/react-query'
import type { BookDocument } from '@tinystream/shared/books'
import { graphql } from '../gql'
import type { BookCardFragment, BookDetailFragment } from '../gql/graphql'
import { request } from '../lib/api'

export type BookCard = BookCardFragment
export type BookDetail = BookDetailFragment
export type Reading = NonNullable<BookCard['reading']>

export const BooksQuery = graphql(`
  query Books($library: String!) {
    books(library: $library) {
      ...BookCard
    }
  }
`)

export const BookQuery = graphql(`
  query Book($id: Int!) {
    book(id: $id) {
      ...BookDetail
    }
  }
`)

export const ReadingNowQuery = graphql(`
  query ReadingNow {
    readingNow {
      ...BookCard
    }
    newBooks(limit: 18) {
      library
      books {
        ...BookCard
      }
    }
  }
`)

export const SaveReading = graphql(`
  mutation SaveReading($id: Int!, $position: Int!) {
    saveReading(id: $id, position: $position) {
      id
      reading {
        position
        progress
        finished
        updatedAt
      }
    }
  }
`)

export const SetBookFinished = graphql(`
  mutation SetBookFinished($id: Int!, $finished: Boolean!) {
    setBookFinished(id: $id, finished: $finished) {
      id
      reading {
        position
        progress
        finished
        updatedAt
      }
    }
  }
`)

export const RefreshBook = graphql(`
  mutation RefreshBook($id: Int!) {
    refreshBook(id: $id) {
      id
    }
  }
`)

export const bookQuery = (id: number) => ({
  queryKey: ['books', 'book', id],
  queryFn: async () => (await request(BookQuery, { id })).book,
})

/** The text, fetched once per version of the book. */
export const contentQuery = (url: string) => ({
  queryKey: ['books', 'content', url],
  queryFn: async (): Promise<BookDocument> => {
    const res = await fetch(url, { credentials: 'include' })
    if (!res.ok) throw new Error(res.status === 404 ? "This book can't be read" : `The server said ${res.status}`)
    return res.json()
  },
  staleTime: Infinity,
  gcTime: 5 * 60_000,
})

/** Puts where someone is in a book into everything showing it, without fetching it all again. */
export function patchReading(qc: QueryClient, id: number, reading: Reading | null) {
  qc.setQueryData<BookDetail | null>(['books', 'book', id], (b) => (b ? { ...b, reading } : b))
  qc.setQueriesData<BookCard[]>({ queryKey: ['books', 'library'] }, (list) => list?.map((b) => (b.id === id ? { ...b, reading } : b)))
}

/** A cover at `size` CSS pixels wide. */
export function bookCover(url: string | null | undefined, size: number): string | undefined {
  if (!url) return undefined
  const px = Math.round(size * (typeof devicePixelRatio === 'number' ? Math.min(devicePixelRatio, 2) : 1))
  return `${url}${url.includes('?') ? '&' : '?'}size=${px}`
}

export function byline(authors: string[]): string {
  if (authors.length <= 2) return authors.join(' & ')
  return `${authors[0]} and ${authors.length - 1} others`
}

/** "Book 3", or "3.5" for the books between books. */
export function volume(index: number | null | undefined): string | null {
  if (index == null) return null
  return `Book ${Number.isInteger(index) ? index : index.toFixed(1)}`
}
