// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Reader } from '../books/Reader'
import { bookQuery, contentQuery } from '../books/api'
import { Empty, Page } from '../components/Page'
import { useTitle } from '../lib/title'

export const Route = createFileRoute('/read/$id')({ component: ReadPage })

function ReadPage() {
  const id = Number(Route.useParams().id)
  // Where it was left may have moved on another screen since this was cached: wait for the server's.
  const { data: book, isFetchedAfterMount } = useQuery({ ...bookQuery(id), refetchOnMount: 'always' })
  useQuery({ ...contentQuery(book?.content ?? ''), enabled: !!book })
  useTitle(book?.title)
  if (!isFetchedAfterMount) return null
  if (!book) return <Page><Empty title="This book isn't here anymore" /></Page>
  return <Reader key={book.id} book={book} />
}
