// SPDX-License-Identifier: AGPL-3.0-or-later
// Quotes: highlights in the reader that are clips too (they're listed, sent and
// linked with them). What's selected is turned into positions, widened to
// whole words, and the server cuts the text itself.

import { type QueryClient, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { Copy, Highlighter, Link2, Send, Trash2, X } from 'lucide-react'
import { type ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { type BookDocument, QUOTE_LENGTH, chars, locate, percent, snapToWords, textOf } from '@tinystream/shared/books'
import { Avatar } from '../components/Avatar'
import { toast, toastError } from '../components/feedback'
import { Button, IconButton, Input, Panel, Popover, Toggle } from '../components/ui'
import { graphql } from '../gql'
import type { QuoteMarkFragment } from '../gql/graphql'
import { type Person, request } from '../lib/api'
import { copyLink } from '../lib/clips'
import { relative } from '../lib/downloads'
import { useCanClip, useClipsOn, useOrigin } from '../lib/hooks'
import { SendTo, allowanceQuery } from '../player/ClipEditor'

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

const CreateQuote = graphql(`
  mutation CreateQuote($input: NewQuote!) {
    createQuote(input: $input) {
      ...QuoteMark
    }
  }
`)

const UpdateQuote = graphql(`
  mutation UpdateQuote($id: Int!, $input: ClipPatch!) {
    updateClip(id: $id, input: $input) {
      id
    }
  }
`)

const ShareQuote = graphql(`
  mutation ShareQuote($id: Int!, $users: [Int!]!) {
    shareClip(id: $id, users: $users) {
      id
    }
  }
`)

const DeleteQuote = graphql(`
  mutation DeleteQuote($id: Int!) {
    deleteClip(id: $id)
  }
`)

const HideQuote = graphql(`
  mutation HideQuote($id: Int!) {
    hideClip(id: $id)
  }
`)

/** A book's quotes, kept under ['clips'] (as `{ clips }`) so clip events reach them. */
export function useQuotes(book: number): Mark[] {
  const on = useClipsOn()
  const { data } = useQuery({
    queryKey: ['clips', 'book', book],
    queryFn: async () => ({ clips: (await request(BookQuotesQuery, { id: book })).book?.quotes ?? [] }),
    enabled: on,
  })
  return useMemo(
    () => (on ? (data?.clips ?? []) : []).flatMap((quote) => (quote.quote?.range ? [{ start: quote.quote.range.start, end: quote.quote.range.end, quote }] : [])),
    [on, data],
  )
}

const refresh = (qc: QueryClient) => void qc.invalidateQueries({ queryKey: ['clips'] })

/** The marks over each block, by block. */
export function marksByBlock(doc: BookDocument, marks: Mark[]): Map<number, Mark[]> {
  const out = new Map<number, Mark[]>()
  for (const m of marks) {
    const from = locate(doc, m.start).index
    for (let i = from; i < doc.blocks.length && doc.blocks[i].at < m.end; i++) {
      const list = out.get(i)
      if (list) list.push(m)
      else out.set(i, [m])
    }
  }
  return out
}

/** Where a point in the text is, in characters; the blocks are the elements marked `data-b`. */
function positionIn(doc: BookDocument, block: HTMLElement, node: Node, offset: number): number {
  const b = doc.blocks[Number(block.dataset.b)]
  const before = document.createRange()
  before.setStart(block, 0)
  before.setEnd(node, offset)
  let n = 0
  const walk = document.createTreeWalker(block, NodeFilter.SHOW_TEXT)
  for (let t = walk.nextNode() as Text | null; t; t = walk.nextNode() as Text | null) {
    if (t.parentElement?.closest('[aria-hidden]')) continue
    if (t === node) {
      n += chars(t.data.slice(0, offset))
      break
    }
    if (!before.intersectsNode(t)) break
    n += chars(t.data)
  }
  return b.at + n
}

export type Picked = { start: number; end: number; rect: DOMRect }

/** What's selected in the text, as whole words, while something is. */
export function useSelectionIn(article: React.RefObject<HTMLElement | null>, doc: BookDocument | undefined): [Picked | null, () => void] {
  const [picked, setPicked] = useState<Picked | null>(null)
  useEffect(() => {
    if (!doc) return
    let frame = 0
    const read = () => {
      const sel = getSelection()
      const root = article.current
      if (!sel || sel.isCollapsed || sel.rangeCount === 0 || !root) return setPicked(null)
      const range = sel.getRangeAt(0)
      if (!root.contains(range.commonAncestorContainer)) return setPicked(null)
      const blocks = [...root.querySelectorAll<HTMLElement>('[data-b]')].filter((b) => range.intersectsNode(b))
      if (blocks.length === 0) return setPicked(null)
      const [first, last] = [blocks[0], blocks[blocks.length - 1]]
      const firstAt = doc.blocks[Number(first.dataset.b)].at
      const lastIndex = Number(last.dataset.b)
      const start = first.contains(range.startContainer) ? positionIn(doc, first, range.startContainer, range.startOffset) : firstAt
      const end = last.contains(range.endContainer)
        ? positionIn(doc, last, range.endContainer, range.endOffset)
        : (doc.blocks[lastIndex + 1]?.at ?? doc.length)
      if (end <= start) return setPicked(null)
      setPicked({ ...snapToWords(doc, start, end), rect: range.getBoundingClientRect() })
    }
    const onChange = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(read)
    }
    document.addEventListener('selectionchange', onChange)
    return () => {
      cancelAnimationFrame(frame)
      document.removeEventListener('selectionchange', onChange)
    }
  }, [article, doc])
  return [picked, () => getSelection()?.removeAllRanges()]
}

/**
 * Floats over the page at `rect` (above it, or under it when there's no room), and goes away on a
 * press outside it or Escape.
 */
function Floating({ rect, onClose, children }: { rect: DOMRect; onClose?: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)
  const [below, setBelow] = useState(false)
  useLayoutEffect(() => {
    const h = ref.current?.offsetHeight ?? 0
    setBelow(rect.top - h - 10 < 64)
  }, [rect])
  useEffect(() => {
    if (!onClose) return
    const onDown = (e: PointerEvent) => !ref.current?.contains(e.target as Node) && onClose()
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.stopPropagation()
      onClose()
    }
    document.addEventListener('pointerdown', onDown)
    document.addEventListener('keydown', onKey, true)
    return () => {
      document.removeEventListener('pointerdown', onDown)
      document.removeEventListener('keydown', onKey, true)
    }
  }, [onClose])
  // Pinned to the page where it was asked for, so it scrolls with the text.
  const at = useMemo(
    () => ({ x: Math.max(12, Math.min(innerWidth - 12, rect.left + rect.width / 2)) + scrollX, top: rect.top - 10 + scrollY, bottom: rect.bottom + 10 + scrollY }),
    [rect],
  )
  return createPortal(
    <div
      ref={ref}
      className={`absolute z-50 -translate-x-1/2 animate-[pop_120ms_ease-out] ${below ? '' : '-translate-y-full'}`}
      style={{ left: at.x, top: below ? at.bottom : at.top }}
    >
      {children}
    </div>,
    document.body,
  )
}

/** What can be done with the text selected: quote it, send it as a quote, or copy it. */
export function SelectionBar({ book, doc, picked, onDone }: { book: number; doc: BookDocument; picked: Picked; onDone: () => void }) {
  const qc = useQueryClient()
  const canClip = useCanClip()
  const [sending, setSending] = useState(false)
  const [to, setTo] = useState<Person[]>([])
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    setSending(false)
    setTo([])
  }, [picked.start, picked.end])
  const long = picked.end - picked.start > QUOTE_LENGTH
  const quote = async (recipients: number[] = []) => {
    setBusy(true)
    try {
      await request(CreateQuote, { input: { bookId: book, start: picked.start, end: picked.end, recipients } })
      refresh(qc)
      toast({ title: recipients.length ? `Sent to ${recipients.length === 1 ? to[0].username : `${recipients.length} people`}` : 'Quoted', tone: 'ok' })
      onDone()
    } catch (e) {
      toastError(e)
    } finally {
      setBusy(false)
    }
  }
  const copy = async () => {
    await navigator.clipboard.writeText(textOf(doc, picked.start, picked.end))
    toast({ title: 'Copied', tone: 'ok' })
    onDone()
  }
  return (
    <Floating rect={picked.rect}>
      <Panel radius={12} className="p-1">
        {sending ? (
          <div className="flex items-center gap-1.5 p-0.5">
            <SendTo value={to} onChange={setTo} />
            <Button variant="primary" size="sm" disabled={to.length === 0 || busy} onClick={() => void quote(to.map((p) => p.id))}>
              Send
            </Button>
          </div>
        ) : (
          <div className="flex items-center gap-0.5">
            {canClip && !long && (
              <>
                <Button size="sm" disabled={busy} onMouseDown={(e) => e.preventDefault()} onClick={() => void quote()}>
                  <Highlighter className="size-3.5" /> Quote
                </Button>
                <Button size="sm" onMouseDown={(e) => e.preventDefault()} onClick={() => setSending(true)}>
                  <Send className="size-3.5" /> Send…
                </Button>
              </>
            )}
            {canClip && long && <span className="px-2.5 text-xs text-ink-3">Too long to quote</span>}
            <Button size="sm" onMouseDown={(e) => e.preventDefault()} onClick={() => void copy()}>
              <Copy className="size-3.5" /> Copy
            </Button>
          </div>
        )}
      </Panel>
    </Floating>
  )
}

function Note({ quote: q }: { quote: Quote }) {
  const qc = useQueryClient()
  const [text, setText] = useState(q.name)
  useEffect(() => setText(q.name), [q.name])
  const save = () => {
    if (text.trim() === q.name) return
    request(UpdateQuote, { id: q.id, input: { name: text } })
      .then(() => refresh(qc))
      .catch(toastError)
  }
  return (
    <Input
      value={text}
      maxLength={120}
      placeholder="Add a note"
      onChange={(e) => setText(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
    />
  )
}

function QuoteActions({ quote: q, onClose }: { quote: Quote; onClose: () => void }) {
  const qc = useQueryClient()
  const { data: you } = useQuery(allowanceQuery)
  const origin = useOrigin()
  const [to, setTo] = useState<Person[]>([])
  const act = async (f: () => Promise<unknown>) => {
    try {
      await f()
      refresh(qc)
    } catch (e) {
      toastError(e)
    }
  }
  const text = q.quote?.text ?? ''
  return (
    <div className="w-80 space-y-3 p-3">
      {!q.mine && (
        <p className="flex items-center gap-2 text-xs text-ink-3">
          <Avatar user={q.owner} size={18} /> {q.owner.username} sent you this · {relative(q.sharedAt ?? q.createdAt)}
        </p>
      )}
      {q.canManage ? <Note quote={q} /> : q.name && <p className="text-sm">{q.name}</p>}
      {q.canManage && (
        <div className="flex items-center gap-2">
          <SendTo value={to} onChange={setTo} />
          {to.length > 0 && (
            <Button
              variant="primary"
              size="sm"
              onClick={() =>
                void act(async () => {
                  await request(ShareQuote, { id: q.id, users: to.map((p) => p.id) })
                  toast({ title: `Sent to ${to.length === 1 ? to[0].username : `${to.length} people`}`, tone: 'ok' })
                  setTo([])
                })
              }
            >
              Send
            </Button>
          )}
        </div>
      )}
      {q.canManage && (you?.canLink || q.public) && (
        <div className="flex items-center gap-2.5">
          <Toggle label="Anyone with the link" checked={q.public} onChange={(v) => void act(() => request(UpdateQuote, { id: q.id, input: { public: v } }))} />
          <span className="min-w-0 flex-1 text-[13px]">Anyone with the link</span>
          {q.public && q.link && q.linkLive && (
            <IconButton label="Copy link" onClick={() => void copyLink(q.link!, origin).then((url) => toast({ title: 'Link copied', body: url, tone: 'ok' }))}>
              <Link2 className="size-4" />
            </IconButton>
          )}
        </div>
      )}
      <div className="flex items-center gap-1 border-t border-line pt-2.5">
        <Button size="sm" onClick={() => void navigator.clipboard.writeText(text).then(() => toast({ title: 'Copied', tone: 'ok' }))}>
          <Copy className="size-3.5" /> Copy
        </Button>
        <Link to="/clips" search={{ tab: q.mine ? 'mine' : 'received', clip: q.id }} className="rounded-lg px-2.5 py-1 text-[13px] text-ink-2 hover:bg-hover hover:text-ink">
          In Clips
        </Link>
        <span className="flex-1" />
        {q.canManage ? (
          <IconButton
            label="Delete"
            className="hover:text-danger"
            onClick={() =>
              void act(async () => {
                await request(DeleteQuote, { id: q.id })
                onClose()
              })
            }
          >
            <Trash2 className="size-4" />
          </IconButton>
        ) : (
          <IconButton
            label="Remove from your list"
            onClick={() =>
              void act(async () => {
                await request(HideQuote, { id: q.id })
                onClose()
              })
            }
          >
            <X className="size-4" />
          </IconButton>
        )}
      </div>
    </div>
  )
}

/** The quotes under a highlight that was pressed (more than one where they overlap). */
export function QuoteCard({ quotes, rect, onClose }: { quotes: Quote[]; rect: DOMRect; onClose: () => void }) {
  if (quotes.length === 0) return null
  return (
    <Floating rect={rect} onClose={onClose}>
      <Panel className="max-h-[70vh] divide-y divide-line overflow-y-auto">
        {quotes.map((q) => (
          <QuoteActions key={q.id} quote={q} onClose={onClose} />
        ))}
      </Panel>
    </Floating>
  )
}

/** The book's quotes in the order they're in it, for the reader's bar. */
export function QuoteList({ doc, marks, onJump }: { doc: BookDocument; marks: Mark[]; onJump: (position: number) => void }) {
  return (
    <Popover
      align="end"
      trigger={({ toggle }) => (
        <IconButton label="Quotes" onClick={toggle}>
          <Highlighter className="size-4.5" />
        </IconButton>
      )}
    >
      {(close) => (
        <Panel className="max-h-[70vh] w-80 overflow-y-auto p-1.5">
          {marks.length === 0 ? (
            <p className="px-2.5 py-2 text-[13px] text-ink-3">Select some text to quote it.</p>
          ) : (
            marks.map(({ start, quote: q }) => (
              <button
                key={q.id}
                onClick={() => {
                  close()
                  onJump(start)
                }}
                className="flex w-full items-start gap-2.5 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-hover"
              >
                {!q.mine && <Avatar user={q.owner} size={20} />}
                <span className="min-w-0 flex-1">
                  <span className="line-clamp-3 text-[13px] leading-snug text-ink">{q.quote?.text}</span>
                  <span className="mt-1 block truncate text-2xs text-ink-3 tabular">
                    {[q.name, q.source.label, `${percent(start, doc.length)}%`].filter(Boolean).join(' · ')}
                  </span>
                </span>
              </button>
            ))
          )}
        </Panel>
      )}
    </Popover>
  )
}
