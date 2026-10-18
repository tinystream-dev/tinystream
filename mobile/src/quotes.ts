// SPDX-License-Identifier: AGPL-3.0-or-later
// Quotes (web/src/books/quotes.tsx): highlights in the reader that are clips
// too, so they're sent and linked with them.

import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import { graphql } from './gql'
import type { QuoteMarkFragment } from './gql/graphql'
import { useClipsOn } from './queries'
import { useApi } from './session'

export type Quote = QuoteMarkFragment

/** A quote where it is in the book as it is now. */
export type Mark = { start: number; end: number; quote: Quote }

const BookQuotesQuery = graphql(`
  query BookQuotes($id: Int!) {
    book(id: $id) {
      id
      quotes {
        ...QuoteMark
      }
    }
  }
`)

export const CreateQuote = graphql(`
  mutation CreateQuote($input: NewQuote!) {
    createQuote(input: $input) {
      ...QuoteMark
    }
  }
`)

export const UpdateQuote = graphql(`
  mutation UpdateQuote($id: Int!, $input: ClipPatch!) {
    updateClip(id: $id, input: $input) {
      id
    }
  }
`)

export const ShareQuote = graphql(`
  mutation ShareQuote($id: Int!, $users: [Int!]!) {
    shareClip(id: $id, users: $users) {
      id
    }
  }
`)

export const DeleteQuote = graphql(`
  mutation DeleteQuote($id: Int!) {
    deleteClip(id: $id)
  }
`)

export const HideQuote = graphql(`
  mutation HideQuote($id: Int!) {
    hideClip(id: $id)
  }
`)

/** A book's quotes, kept under ['clips'] so clip events reach them. */
export function useQuotes(book: number): Mark[] {
  const api = useApi()
  const on = useClipsOn()
  const { data } = useQuery({
    queryKey: ['clips', 'book', book],
    queryFn: async () => ({ clips: (await api.request(BookQuotesQuery, { id: book })).book?.quotes ?? [] }),
    enabled: on,
  })
  return useMemo(
    () => (on ? (data?.clips ?? []) : []).flatMap((quote) => (quote.quote?.range ? [{ start: quote.quote.range.start, end: quote.quote.range.end, quote }] : [])),
    [on, data],
  )
}
