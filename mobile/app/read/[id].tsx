// SPDX-License-Identifier: AGPL-3.0-or-later
// The reader, over the tabs. It waits for where the book was left from the
// server rather than the cache: it may have been read on since, elsewhere.

import { useQuery } from '@tanstack/react-query'
import { useLocalSearchParams } from 'expo-router'
import { Text, View } from 'react-native'
import { bookQuery, useBookContent } from '../../src/books'
import { Spinner } from '../../src/components/ui'
import { Reader } from '../../src/reader/Reader'
import { useApi } from '../../src/session'

export default function Read() {
  const id = Number(useLocalSearchParams<{ id: string }>().id)
  const api = useApi()
  const { data: book, isFetchedAfterMount } = useQuery({ ...bookQuery(api, id), refetchOnMount: 'always' })
  const content = useBookContent(book?.content)

  if (book === null || content.error)
    return (
      <View className="flex-1 items-center justify-center bg-canvas px-8">
        <Text className="font-sans text-center text-[15px] font-medium text-ink">{book === null ? "This book isn't here anymore" : "This book can't be opened"}</Text>
        {content.error && <Text className="font-sans mt-2 text-center text-sm text-ink-2">{(content.error as Error).message}</Text>}
      </View>
    )
  if (!isFetchedAfterMount || !book || !content.data)
    return (
      <View className="flex-1 items-center justify-center bg-canvas">
        <Spinner size="large" />
      </View>
    )
  return <Reader key={book.id} book={book} doc={content.data} />
}
