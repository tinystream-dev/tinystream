// SPDX-License-Identifier: AGPL-3.0-or-later
// What the reader shows for quotes (web/src/books/quotes.tsx): the bar over a
// selection, who to send one to, what can be done with a highlight, and the
// book's quotes in order.

import { useQueryClient } from '@tanstack/react-query'
import { type BookDocument, percent } from '@tinystream/shared/books'
import * as Clipboard from 'expo-clipboard'
import { Copy, Highlighter, Send, Share2, Trash2, X } from 'lucide-react-native'
import { useEffect, useState } from 'react'
import { Pressable, ScrollView, Share, Text, View } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { Avatar } from '../components/Avatar'
import { toast, toastError } from '../components/Feedback'
import { Sheet } from '../components/Sheet'
import { Button, Checkbox, IconButton, Input, Panel, Toggle } from '../components/ui'
import { resolve } from '../lib/address'
import { useMe, usePeople, usePublicUrl } from '../queries'
import { type Mark, type Quote, DeleteQuote, HideQuote, ShareQuote, UpdateQuote } from '../quotes'
import { useApi, useConnection } from '../session'
import { useTheme } from '../theme/ThemeProvider'

export const sentTo = (n: number, first?: string) => `Sent to ${n === 1 ? first : `${n} people`}`

export async function copy(text: string) {
  await Clipboard.setStringAsync(text)
  toast({ title: 'Copied', tone: 'ok' })
}

/** Over the selection, at the bottom of the screen: quote it, send it, copy it. */
export function SelectionBar({
  long,
  canQuote,
  busy,
  onQuote,
  onSend,
  onCopy,
  onClose,
}: {
  long: boolean
  canQuote: boolean
  busy: boolean
  onQuote: () => void
  onSend: () => void
  onCopy: () => void
  onClose: () => void
}) {
  const insets = useSafeAreaInsets()
  const { tokens } = useTheme()
  return (
    <View pointerEvents="box-none" style={{ position: 'absolute', left: 12, right: 12, bottom: insets.bottom + 12 }}>
      <Panel radius={18}>
        <View className="flex-row items-center gap-1 p-1.5">
          {canQuote && !long && (
            <>
              <Button size="sm" disabled={busy} onPress={onQuote} icon={<Highlighter size={16} color={tokens.ink} />}>
                Quote
              </Button>
              <Button size="sm" disabled={busy} onPress={onSend} icon={<Send size={16} color={tokens.ink} />}>
                Send…
              </Button>
            </>
          )}
          {canQuote && long && <Text className="font-sans px-2 text-[13px] text-ink-3">Too long to quote</Text>}
          <Button size="sm" onPress={onCopy} icon={<Copy size={16} color={tokens.ink} />}>
            Copy
          </Button>
          <View className="flex-1" />
          <IconButton label="Cancel" size={36} onPress={onClose}>
            <X size={18} color={tokens['ink-2']} />
          </IconButton>
        </View>
      </Panel>
    </View>
  )
}

/** Who to send something to, picked from everyone else here. */
export function SendSheet({ open, onClose, onSend }: { open: boolean; onClose: () => void; onSend: (people: { id: number; username: string }[]) => Promise<unknown> }) {
  const { data: people } = usePeople()
  const [picked, setPicked] = useState<number[]>([])
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    if (!open) setPicked([])
  }, [open])
  const send = async () => {
    setBusy(true)
    try {
      await onSend((people ?? []).filter((p) => picked.includes(p.id)))
      onClose()
    } catch (e) {
      toastError(e)
    } finally {
      setBusy(false)
    }
  }
  return (
    <Sheet open={open} onClose={onClose}>
      <Text className="font-sans mb-3 text-[17px] font-semibold text-ink">Send to</Text>
      {people && people.length === 0 ? (
        <Text className="font-sans py-6 text-center text-sm text-ink-3">There's no one else here to send it to.</Text>
      ) : (
        <ScrollView style={{ maxHeight: 360 }}>
          {people?.map((p) => {
            const on = picked.includes(p.id)
            return (
              <Pressable key={p.id} onPress={() => setPicked(on ? picked.filter((id) => id !== p.id) : [...picked, p.id])} className="flex-row items-center gap-3 rounded-xl px-1 py-2.5">
                <Avatar user={p} size={30} />
                <Text className="font-sans flex-1 text-[15px] text-ink">{p.username}</Text>
                <Checkbox checked={on} />
              </Pressable>
            )
          })}
        </ScrollView>
      )}
      <Button variant="primary" size="lg" className="mt-4" disabled={picked.length === 0 || busy} onPress={() => void send()}>
        Send
      </Button>
    </Sheet>
  )
}

function Note({ quote: q }: { quote: Quote }) {
  const api = useApi()
  const qc = useQueryClient()
  const [text, setText] = useState(q.name)
  useEffect(() => setText(q.name), [q.name])
  const save = () => {
    if (text.trim() === q.name) return
    api
      .request(UpdateQuote, { id: q.id, input: { name: text } })
      .then(() => qc.invalidateQueries({ queryKey: ['clips'] }))
      .catch(toastError)
  }
  return <Input value={text} maxLength={120} placeholder="Add a note" onChangeText={setText} onBlur={save} onSubmitEditing={save} returnKeyType="done" />
}

function QuoteActions({ quote: q, onSend, onClose }: { quote: Quote; onSend: (q: Quote) => void; onClose: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const me = useMe()
  const connection = useConnection()
  const publicUrl = usePublicUrl()
  const { tokens } = useTheme()
  const act = async (f: () => Promise<unknown>) => {
    try {
      await f()
      await qc.invalidateQueries({ queryKey: ['clips'] })
    } catch (e) {
      toastError(e)
    }
  }
  const canLink = !!me?.permissions.clipLinks || q.public
  return (
    <View className="gap-3 py-3">
      {!q.mine && (
        <View className="flex-row items-center gap-2">
          <Avatar user={q.owner} size={20} />
          <Text className="font-sans text-[13px] text-ink-3">{q.owner.username} sent you this</Text>
        </View>
      )}
      {q.canManage ? <Note quote={q} /> : !!q.name && <Text className="font-sans text-[15px] text-ink">{q.name}</Text>}
      {q.canManage && canLink && (
        <View className="flex-row items-center gap-3">
          <Text className="font-sans flex-1 text-[15px] text-ink">Anyone with the link</Text>
          {q.public && q.link && q.linkLive && connection && (
            <IconButton label="Share the link" size={36} onPress={() => void Share.share({ message: resolve(publicUrl ?? connection.origin, q.link!) })}>
              <Share2 size={18} color={tokens.ink} />
            </IconButton>
          )}
          <Toggle label="Anyone with the link" value={q.public} onChange={(v) => void act(() => api.request(UpdateQuote, { id: q.id, input: { public: v } }))} />
        </View>
      )}
      <View className="flex-row items-center gap-1.5">
        {q.canManage && (
          <Button size="sm" onPress={() => onSend(q)} icon={<Send size={16} color={tokens.ink} />}>
            Send…
          </Button>
        )}
        <Button size="sm" onPress={() => void copy(q.quote?.text ?? '')} icon={<Copy size={16} color={tokens.ink} />}>
          Copy
        </Button>
        <View className="flex-1" />
        {q.canManage ? (
          <IconButton
            label="Delete"
            size={36}
            onPress={() =>
              void act(async () => {
                await api.request(DeleteQuote, { id: q.id })
                onClose()
              })
            }
          >
            <Trash2 size={18} color={tokens.danger} />
          </IconButton>
        ) : (
          <IconButton
            label="Remove from your list"
            size={36}
            onPress={() =>
              void act(async () => {
                await api.request(HideQuote, { id: q.id })
                onClose()
              })
            }
          >
            <X size={18} color={tokens['ink-2']} />
          </IconButton>
        )}
      </View>
    </View>
  )
}

/** The quotes under a highlight that was tapped (more than one where they overlap). */
export function QuoteSheet({ quotes, onClose }: { quotes: Quote[]; onClose: () => void }) {
  const api = useApi()
  const qc = useQueryClient()
  const [sending, setSending] = useState<Quote | null>(null)
  return (
    <>
      <Sheet open={quotes.length > 0 && !sending} onClose={onClose}>
        <ScrollView style={{ maxHeight: 520 }} keyboardShouldPersistTaps="handled">
          {quotes.map((q) => (
            <QuoteActions key={q.id} quote={q} onSend={setSending} onClose={onClose} />
          ))}
        </ScrollView>
      </Sheet>
      <SendSheet
        open={!!sending}
        onClose={() => setSending(null)}
        onSend={async (people) => {
          await api.request(ShareQuote, { id: sending!.id, users: people.map((p) => p.id) })
          await qc.invalidateQueries({ queryKey: ['clips'] })
          toast({ title: sentTo(people.length, people[0]?.username), tone: 'ok' })
        }}
      />
    </>
  )
}

/** The book's quotes in the order they're in it. */
export function QuoteList({ open, doc, marks, onJump, onClose }: { open: boolean; doc: BookDocument; marks: Mark[]; onJump: (position: number) => void; onClose: () => void }) {
  return (
    <Sheet open={open} onClose={onClose}>
      <Text className="font-sans mb-2 text-[17px] font-semibold text-ink">Quotes</Text>
      {marks.length === 0 ? (
        <Text className="font-sans py-6 text-center text-sm text-ink-3">Press and hold on some text to quote it.</Text>
      ) : (
        <ScrollView style={{ maxHeight: 520 }} contentContainerStyle={{ paddingBottom: 12 }}>
          {marks.map(({ start, quote: q }) => (
            <Pressable
              key={q.id}
              onPress={() => {
                onClose()
                onJump(start)
              }}
              className="flex-row gap-3 rounded-xl py-2.5"
            >
              {!q.mine && <Avatar user={q.owner} size={22} />}
              <View className="flex-1">
                <Text className="font-sans text-[15px] leading-snug text-ink" numberOfLines={3}>
                  {q.quote?.text.replace(/\s+/g, ' ')}
                </Text>
                <Text className="font-sans mt-1 text-xs text-ink-3" numberOfLines={1}>
                  {[q.name, q.source.label, `${percent(start, doc.length)}%`].filter(Boolean).join(' · ')}
                </Text>
              </View>
            </Pressable>
          ))}
        </ScrollView>
      )}
    </Sheet>
  )
}
