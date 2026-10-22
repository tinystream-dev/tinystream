// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, createFileRoute, useNavigate } from '@tanstack/react-router'
import { BookOpen, Camera, Check, Copy, Download, Link2, Pencil, Play, Quote, RotateCw, Scissors, Trash2, TriangleAlert, X } from 'lucide-react'
import { type ReactNode, useEffect, useRef, useState } from 'react'
import { BookCover } from '../books/components'
import { Avatar } from '../components/Avatar'
import { ask, toast, toastError } from '../components/feedback'
import { Empty, Page, PageTitle } from '../components/Page'
import { Bone, useArrived } from '../components/Skeleton'
import { Squircle } from '../components/Squircle'
import { Badge, Button, Dialog, IconButton, Input, Progress, Segmented, Spinner, Swap, Toggle } from '../components/ui'
import { graphql } from '../gql'
import type { ClipKind, ClipScope } from '../gql/graphql'
import { type Clip, type Person, request } from '../lib/api'
import type { ClipList } from '../lib/clips'
import { allowanceQuery } from '../player/ClipEditor'
import { ClipPlayer } from '../player/ClipPlayer'

const ClipsQuery = graphql(`
  query Clips($scope: ClipScope!, $kind: ClipKind, $book: Int) {
    clips(scope: $scope, kind: $kind, book: $book) {
      ...ClipFields
    }
    clipAllowance {
      ...ClipAllowanceFields
    }
  }
`)

const RenderClip = graphql(`
  mutation RenderClip($id: Int!) {
    renderClip(id: $id) {
      ...ClipFields
    }
  }
`)

const UpdateClip = graphql(`
  mutation ClipsUpdateClip($id: Int!, $input: ClipPatch!) {
    updateClip(id: $id, input: $input) {
      ...ClipFields
    }
  }
`)

const ShareClip = graphql(`
  mutation ClipsShareClip($id: Int!, $users: [Int!]!) {
    shareClip(id: $id, users: $users) {
      ...ClipFields
    }
  }
`)

const UnshareClip = graphql(`
  mutation UnshareClip($id: Int!, $userId: Int!) {
    unshareClip(id: $id, userId: $userId) {
      ...ClipFields
    }
  }
`)

const DeleteClip = graphql(`
  mutation DeleteClip($id: Int!) {
    deleteClip(id: $id)
  }
`)

const HideClip = graphql(`
  mutation HideClip($id: Int!) {
    hideClip(id: $id)
  }
`)

const SCOPES: Record<Scope, ClipScope> = { mine: 'MINE', received: 'RECEIVED', sent: 'SENT' }
import { clipName, clipSource, copyLink, fetchClip, length, space, stamp, tint } from '../lib/clips'
import { bytes, relative } from '../lib/downloads'
import { useCanClip } from '../lib/hooks'
import { useTitle } from '../lib/title'
import { SendTo } from '../player/ClipEditor'

type Scope = 'mine' | 'received' | 'sent'
const ORDER: Scope[] = ['mine', 'received', 'sent']

type Kind = 'all' | 'clips' | 'screenshots' | 'quotes'
const KINDS: Record<Kind, ClipKind | null> = { all: null, clips: 'CLIP', screenshots: 'SCREENSHOT', quotes: 'QUOTE' }

type Search = { tab?: Scope; kind?: Kind; book?: number; clip?: number }

export const Route = createFileRoute('/clips')({
  validateSearch: (s: Record<string, unknown>): Search => ({
    tab: s.tab as Scope | undefined,
    kind: s.kind as Kind | undefined,
    book: s.book ? Number(s.book) : undefined,
    clip: s.clip ? Number(s.clip) : undefined,
  }),
  component: ClipsPage,
})

const EMPTY: Record<Scope, { title: string }> = {
  mine: { title: 'No clips or screenshots yet' },
  received: { title: 'Nothing here yet' },
  sent: { title: 'You haven’t sent any clips' },
}

function ClipsPage() {
  useTitle('Clips')
  const { tab = 'mine', kind = 'all', book, clip: open } = Route.useSearch()
  const navigate = useNavigate()
  const { data, isPlaceholderData } = useQuery({
    queryKey: ['clips', tab, kind, book],
    queryFn: async (): Promise<ClipList & { scope: Scope; kind: Kind }> => {
      const r = await request(ClipsQuery, { scope: SCOPES[tab], kind: KINDS[kind], book })
      return { clips: r.clips, you: r.clipAllowance, scope: tab, kind }
    },
    // The last tab stays up until the next one is ready, so switching slides between them.
    placeholderData: (p) => p,
  })
  const arrived = useArrived(!!data)
  const go = (search: Search) => void navigate({ to: '/clips', search: { tab, kind, book, ...search }, replace: true })
  const quotes = book !== undefined || kind === 'quotes'

  return (
    <Page arrive={arrived}>
      <PageTitle aside={data && <Space you={data.you} />}>Clips</PageTitle>
      <div className="mb-6 flex flex-wrap items-center gap-x-4 gap-y-3">
        <Segmented
          value={tab}
          onChange={(t) => go({ tab: t, clip: undefined })}
          options={[
            { value: 'mine', label: 'Yours' },
            { value: 'received', label: 'Shared with you' },
            { value: 'sent', label: 'Shared by you' },
          ]}
        />
        {book === undefined ? (
          <Segmented
            size="sm"
            value={kind}
            onChange={(k) => go({ kind: k, clip: undefined })}
            options={[
              { value: 'all', label: 'All' },
              { value: 'clips', label: 'Clips' },
              { value: 'screenshots', label: 'Screenshots' },
              { value: 'quotes', label: 'Quotes' },
            ]}
          />
        ) : (
          <Squircle radius={10} className="flex h-8 items-center gap-2 bg-panel pr-1 pl-2.5 text-[13px]">
            <Quote className="size-3.5 text-ink-3" />
            Quotes{data?.clips[0] ? ` from ${data.clips[0].source.name}` : ''}
            <button aria-label="Show everything" onClick={() => go({ book: undefined, kind: 'quotes' })} className="grid size-6 place-items-center rounded-md text-ink-3 hover:bg-hover hover:text-ink">
              <X className="size-3" />
            </button>
          </Squircle>
        )}
      </div>
      {!data ? (
        <Grid>
          {Array.from({ length: 8 }, (_, i) => (
            <div key={i}>
              <Bone className="aspect-video w-full rounded-[14px]" />
              <Bone className="mt-2.5 h-4 w-2/3 rounded" />
            </div>
          ))}
        </Grid>
      ) : (
        <Swap value={data.scope} order={ORDER} fetching={isPlaceholderData}>
          {data.clips.length === 0 ? (
            data.kind === 'quotes' ? (
              <Empty title="No quotes here">Select some text in a book to quote it.</Empty>
            ) : (
              <Empty title={data.kind === 'all' ? EMPTY[data.scope].title : 'Nothing here yet'} />
            )
          ) : quotes ? (
            <QuotesByBook clips={data.clips} scope={data.scope} onOpen={(id) => go({ clip: id })} />
          ) : (
            <Grid>
              {data.clips.map((c) => (
                <ClipCard key={c.id} clip={c} scope={data.scope} onOpen={() => go({ clip: c.id })} />
              ))}
            </Grid>
          )}
        </Swap>
      )}
      {open !== undefined && <ClipDialog id={open} onClose={() => go({ clip: undefined })} />}
    </Page>
  )
}

function Grid({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-x-5 gap-y-7">{children}</div>
}

/** Quotes under the books they're from, in the order they're in each. */
function QuotesByBook({ clips, scope, onOpen }: { clips: Clip[]; scope: Scope; onOpen: (id: number) => void }) {
  const books = new Map<string, Clip[]>()
  for (const c of clips) {
    const key = c.source.book ? `b${c.source.book.id}` : `n${c.source.name}`
    books.set(key, [...(books.get(key) ?? []), c])
  }
  return (
    <div className="space-y-12">
      {[...books.values()].map((list) => {
        const first = list[0]
        const b = first.source.book
        return (
          <section key={first.id}>
            <div className="mb-4 flex items-center gap-3.5">
              {b ? (
                <Link to="/book/$id" params={{ id: String(b.id) }} className="w-11 shrink-0">
                  <BookCover book={{ cover: b.cover, title: b.title, authors: [] }} size={44} />
                </Link>
              ) : (
                <Squircle radius={8} className="grid h-16 w-11 shrink-0 place-items-center bg-raised">
                  <BookOpen className="size-4 text-ink-3" />
                </Squircle>
              )}
              <div className="min-w-0 flex-1">
                <p className="truncate text-[15px] font-semibold">{first.source.name}</p>
                <p className="truncate text-[13px] text-ink-3">
                  {[first.quote?.author, `${list.length} ${list.length === 1 ? 'quote' : 'quotes'}`].filter(Boolean).join(' · ')}
                </p>
              </div>
              {b && (
                <Link to="/read/$id" params={{ id: String(b.id) }}>
                  <Button size="sm" tabIndex={-1}>
                    Read
                  </Button>
                </Link>
              )}
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              {[...list]
                .sort((x, y) => x.start - y.start)
                .map((c) => (
                  <button key={c.id} onClick={() => onOpen(c.id)} className="group min-w-0 text-left outline-none">
                    <Squircle radius={14} edge className="h-full bg-panel px-4 py-3.5 transition-colors group-hover:bg-hover">
                      <p className="line-clamp-6 text-[15px] leading-relaxed whitespace-pre-line text-ink">{c.quote?.text}</p>
                      {c.name && <p className="mt-2 text-[13px] text-ink-2">{c.name}</p>}
                      <p className="mt-2 flex items-center gap-1.5 truncate text-xs text-ink-3">
                        {!c.mine && <Avatar user={c.owner} size={16} />}
                        {[!c.mine && c.owner.username, c.source.label, relative(scope === 'received' ? (c.sharedAt ?? c.createdAt) : c.createdAt)].filter(Boolean).join(' · ')}
                        {c.public && c.linkLive && <Link2 className="size-3" />}
                      </p>
                    </Squircle>
                  </button>
                ))}
            </div>
          </section>
        )
      })}
    </div>
  )
}

/** Your clip space: how much is used of what you have. */
function Space({ you }: { you: ClipList['you'] }) {
  if (!you.storage && !you.limit) return you.bytes ? <p className="pb-1 text-[13px] text-ink-3 tabular">{bytes(you.bytes)} rendered</p> : null
  const share = Math.max(you.storage ? you.bytes / (you.storage * 1024 * 1024) : 0, you.limit ? you.rendered / you.limit : 0)
  return (
    <div className="w-52 pb-1">
      <p className="mb-1.5 text-right text-xs text-ink-3 tabular">
        {you.storage ? `${bytes(you.bytes)} of ${space(you.storage)}` : bytes(you.bytes)}
        {you.limit ? ` · ${you.rendered} of ${you.limit} clips` : ''}
      </p>
      <Progress value={share} tone={share > 0.9 ? 'live' : 'ink'} />
    </div>
  )
}
function ClipCard({ clip: c, scope, onOpen }: { clip: Clip; scope: Scope; onOpen: () => void }) {
  return (
    <button onClick={onOpen} className="group min-w-0 text-left outline-none">
      <Squircle radius={14} edge className="relative aspect-video w-full overflow-hidden bg-raised">
        <Cover clip={c} />
        <span className="absolute right-2 bottom-2 flex items-center gap-1 rounded-md bg-media-shade/65 px-1.5 py-0.5 text-2xs font-medium text-media-ink tabular backdrop-blur-md">
          {c.quote ? (
            <>
              <Quote className="size-3" /> Quote
            </>
          ) : c.screenshot ? (
            <>
              <Camera className="size-3" /> {stamp(c.start)}
            </>
          ) : (
            length(c.end - c.start)
          )}
        </span>
        {c.public && c.linkLive && (
          <span className="absolute top-2 right-2 grid size-6 place-items-center rounded-full bg-media-shade/60 text-media-ink backdrop-blur-md">
            <Link2 className="size-3" />
          </span>
        )}
      </Squircle>
      <div className="mt-2.5 flex items-start gap-2.5">
        {scope === 'received' && <Avatar user={c.owner} size={26} />}
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium group-hover:underline group-hover:underline-offset-2">{clipName(c)}</p>
          <p className="truncate text-xs text-ink-3">
            {scope === 'received' ? `${c.owner.username} · ${relative(c.sharedAt ?? c.createdAt)}` : c.name ? clipSource(c) : relative(c.createdAt)}
          </p>
        </div>
        {scope === 'sent' && (
          <div className="flex shrink-0 -space-x-1.5 pt-0.5">
            {c.recipients.slice(0, 3).map(({ user: p }) => (
              <span key={p.id} className="rounded-full ring-2 ring-canvas">
                <Avatar user={p} size={20} />
              </span>
            ))}
          </div>
        )}
      </div>
    </button>
  )
}

/** The clip's picture, or where its render is at. */
function Cover({ clip: c }: { clip: Clip }) {
  if (c.quote)
    return (
      <div className="flex size-full items-center px-5 py-4" style={{ background: tint(c.source.book?.coverTint, 0.16) }}>
        <p className="line-clamp-4 text-[15px] leading-snug text-ink transition-transform duration-500 group-hover:scale-[1.02]">{c.quote.text}</p>
      </div>
    )
  if (c.state === 'READY' && c.poster) return <img src={c.poster} alt="" className="size-full object-cover transition-transform duration-500 group-hover:scale-[1.03]" />
  const Icon = c.screenshot ? Camera : Scissors
  return (
    <div className="grid size-full place-items-center p-4 text-center">
      {c.state === 'RENDERING' || c.state === 'QUEUED' ? (
        <div className="w-2/3">
          <Icon className="mx-auto mb-3 size-5 animate-pulse text-highlight" />
          <Progress value={c.state === 'RENDERING' ? (c.progress ?? 0) : null} />
        </div>
      ) : c.state === 'FAILED' ? (
        <p className="flex items-center gap-1.5 text-xs text-danger">
          <TriangleAlert className="size-3.5" /> Didn’t render
        </p>
      ) : (
        <p className="text-xs text-ink-3">Renders when opened</p>
      )}
    </div>
  )
}
function ClipDialog({ id, onClose }: { id: number; onClose: () => void }) {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const canClip = useCanClip()
  const { data: c, error } = useQuery({ queryKey: ['clip', id], queryFn: () => fetchClip(id), retry: false })
  const refresh = (next?: Clip) => {
    if (next) qc.setQueryData(['clip', id], next)
    void qc.invalidateQueries({ queryKey: ['clips'] })
  }

  // A render that was dropped comes back when someone opens it.
  const asked = useRef(false)
  useEffect(() => {
    if (!c || asked.current || c.state !== 'EVICTED') return
    asked.current = true
    request(RenderClip, { id }).then((r) => refresh(r.renderClip)).catch(toastError)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [c, id])

  const [copied, setCopied] = useState(false)
  const [sendTo, setSendTo] = useState<Person[]>([])
  const act = async (f: () => Promise<Clip | undefined | void>) => {
    try {
      refresh((await f()) ?? undefined)
    } catch (e) {
      toastError(e)
    }
  }

  if (error || c === null)
    return (
      <Dialog onClose={onClose}>
        <p className="text-[15px] font-medium">This clip isn’t here anymore</p>
      </Dialog>
    )
  if (!c)
    return (
      <Dialog onClose={onClose} width="max-w-3xl">
        <div className="grid aspect-video place-items-center">
          <Spinner className="size-7" />
        </div>
      </Dialog>
    )

  const ready = c.state === 'READY'
  const editable = !c.screenshot && c.canManage && canClip && !!c.source.video && c.source.status !== 'GONE'
  return (
    <Dialog onClose={onClose} width="max-w-3xl">
      {c.quote ? (
        <div className="-mx-2 -mt-2">
          <Squircle radius={14} className="max-h-[50vh] overflow-y-auto px-6 py-5" style={{ background: tint(c.source.book?.coverTint, 0.14) ?? 'var(--color-panel)' }}>
            <p className="text-[17px] leading-relaxed whitespace-pre-line text-ink">{c.quote.text}</p>
          </Squircle>
        </div>
      ) : (
        <div className="-mx-2 -mt-2">
          <Squircle radius={14} className="relative aspect-video w-full overflow-hidden bg-media-shade">
            {ready && c.screenshot ? (
              <a href={c.file} target="_blank" rel="noreferrer" className="block size-full">
                <img key={c.renderedAt} src={c.file} alt={clipName(c)} className="size-full object-contain" />
              </a>
            ) : ready ? (
              <ClipPlayer key={c.renderedAt} src={c.file} poster={c.poster ?? undefined} />
            ) : (
              <div className="grid size-full place-items-center p-6 text-center">
                {c.state === 'FAILED' ? (
                  <div className="max-w-sm">
                    <TriangleAlert className="mx-auto mb-3 size-6 text-danger" />
                    <p className="text-sm text-ink-2">{c.error ?? 'It didn’t render.'}</p>
                    {c.canManage && c.source.status === 'OK' && (
                      <Button className="mt-4" onClick={() => void act(async () => (await request(RenderClip, { id })).renderClip)}>
                        <RotateCw className="size-3.5" /> Try again
                      </Button>
                    )}
                  </div>
                ) : (
                  <div className="w-60">
                    {c.screenshot ? (
                      <Camera className="mx-auto mb-4 size-6 animate-pulse text-highlight" />
                    ) : (
                      <Scissors className="mx-auto mb-4 size-6 animate-pulse text-highlight" />
                    )}
                    <Progress value={c.state === 'RENDERING' ? (c.progress ?? 0) : null} />
                    <p className="mt-3 text-[13px] text-ink-3">
                      {c.state === 'QUEUED' ? 'Waiting its turn to render' : c.state === 'RENDERING' ? `Rendering · ${Math.round((c.progress ?? 0) * 100)}%` : 'Getting it ready'}
                    </p>
                  </div>
                )}
              </div>
            )}
          </Squircle>
        </div>
      )}

      <div className="mt-5 flex items-start gap-3">
        <div className="min-w-0 flex-1">
          {c.canManage ? <Title clip={c} onSaved={refresh} /> : <p className="text-lg font-semibold tracking-tight">{clipName(c)}</p>}
          <p className="mt-1 text-[13px] text-ink-2">
            {c.source.title ? (
              <Link to="/title/$id" params={{ id: String(c.source.title.id) }} className="hover:text-ink hover:underline">
                {clipSource(c)}
              </Link>
            ) : c.source.book ? (
              <Link to="/book/$id" params={{ id: String(c.source.book.id) }} className="hover:text-ink hover:underline">
                {clipSource(c)}
              </Link>
            ) : (
              clipSource(c)
            )}
            {c.quote ? (
              c.quote.author && <span className="text-ink-3"> · {c.quote.author}</span>
            ) : (
              <span className="text-ink-3 tabular">
                {' '}
                · {c.screenshot ? stamp(c.start) : `${stamp(c.start)}–${stamp(c.end)}`}
              </span>
            )}
          </p>
          <p className="mt-1 flex flex-wrap items-center gap-x-2 text-xs text-ink-3 tabular">
            {!c.mine && (
              <span className="flex items-center gap-1.5">
                <Avatar user={c.owner} size={16} /> {c.owner.username} ·
              </span>
            )}
            {c.quote
              ? 'Quote'
              : c.screenshot
                ? c.width
                  ? `${c.width}×${c.height} PNG`
                  : 'PNG'
                : c.width
                  ? `${c.width}×${c.height} · ${Math.round(c.fps ?? 0)} fps`
                  : `${c.quality.height}p`}
            {ready && c.bytes ? ` · ${bytes(c.bytes)}` : ''} · {relative(c.createdAt)}
          </p>
          {c.source.status === 'CHANGED' && c.canManage && !c.screenshot && !c.quote && (
            <p className="mt-2 flex items-start gap-1.5 text-xs text-warn-soft/90">
              <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
              Source video was replaced; check the range.
            </p>
          )}
        </div>
        <div className="flex shrink-0 gap-1.5">
          {c.quote && (
            <Button variant="primary" onClick={() => void navigator.clipboard.writeText(c.quote!.text).then(() => toast({ title: 'Copied', tone: 'ok' }))}>
              <Copy className="size-4" /> Copy
            </Button>
          )}
          {c.source.book && (
            <IconButton label="Read from here" onClick={() => void navigate({ to: '/read/$id', params: { id: String(c.source.book?.id) }, search: { quote: c.id } })}>
              <BookOpen className="size-4" />
            </IconButton>
          )}
          {ready && !c.quote && (
            <a href={`${c.file}?download=1`} download>
              <Button variant="primary" tabIndex={-1}>
                <Download className="size-4" /> Download
              </Button>
            </a>
          )}
          {c.source.video && (
            <IconButton label="Watch from here" onClick={() => void navigate({ to: '/watch/$id', params: { id: String(c.source.video?.id) }, search: { t: c.start } })}>
              <Play className="size-4" />
            </IconButton>
          )}
          {editable && (
            <IconButton label="Change the range, tracks or quality" onClick={() => void navigate({ to: '/watch/$id', params: { id: String(c.source.video?.id) }, search: { clip: c.id } })}>
              <Pencil className="size-4" />
            </IconButton>
          )}
          {c.canManage ? (
            <IconButton
              label="Delete"
              className="hover:text-danger"
              onClick={async () => {
                const ok = await ask({
                  title: c.quote ? 'Delete this quote?' : c.screenshot ? 'Delete this screenshot?' : 'Delete this clip?',
                  body: c.recipients.length ? 'Also removes it for everyone it was sent to.' : undefined,
                  confirm: 'Delete',
                  danger: true,
                })
                if (!ok) return
                await act(async () => void (await request(DeleteClip, { id })))
                onClose()
              }}
            >
              <Trash2 className="size-4" />
            </IconButton>
          ) : (
            <IconButton
              label="Remove from your list"
              onClick={async () => {
                await act(async () => void (await request(HideClip, { id })))
                onClose()
              }}
            >
              <X className="size-4" />
            </IconButton>
          )}
        </div>
      </div>

      {c.canManage && (
        <div className="mt-5 space-y-4 border-t border-line pt-4">
          <PublicLink clip={c} copied={copied} onCopied={() => setCopied(true)} onChange={(v) => void act(async () => (await request(UpdateClip, { id, input: { public: v } })).updateClip)} />
          <div>
            <p className="mb-2 text-xs text-ink-3">Sent to</p>
            {c.recipients.length > 0 && (
              <div className="mb-3 flex flex-wrap gap-1.5">
                {c.recipients.map(({ user: p, hidden }) => (
                  <Squircle key={p.id} radius={10} className="flex h-8 items-center gap-2 bg-panel pr-1 pl-1.5 text-[13px]">
                    <Avatar user={p} size={20} />
                    {p.username}
                    {hidden && <span className="text-2xs text-ink-3">removed it</span>}
                    <button
                      aria-label={`Take it back from ${p.username}`}
                      onClick={() => void act(async () => (await request(UnshareClip, { id, userId: p.id })).unshareClip)}
                      className="grid size-6 place-items-center rounded-md text-ink-3 hover:bg-hover hover:text-ink"
                    >
                      <X className="size-3" />
                    </button>
                  </Squircle>
                ))}
              </div>
            )}
            <div className="flex items-center gap-2">
              <SendTo value={sendTo} onChange={setSendTo} />
              {sendTo.length > 0 && (
                <Button
                  variant="primary"
                  size="sm"
                  onClick={() =>
                    void act(async () => {
                      const r = (await request(ShareClip, { id, users: sendTo.map((p) => p.id) })).shareClip
                      toast({ title: `Sent to ${sendTo.length === 1 ? sendTo[0].username : `${sendTo.length} people`}`, tone: 'ok' })
                      setSendTo([])
                      return r
                    })
                  }
                >
                  Send
                </Button>
              )}
            </div>
          </div>
        </div>
      )}
    </Dialog>
  )
}

function Title({ clip: c, onSaved }: { clip: Clip; onSaved: (c: Clip) => void }) {
  const [text, setText] = useState(c.name)
  useEffect(() => setText(c.name), [c.name])
  const save = () => {
    if (text.trim() === c.name) return
    request(UpdateClip, { id: c.id, input: { name: text } })
      .then((r) => onSaved(r.updateClip))
      .catch(toastError)
  }
  return (
    <Input
      value={text}
      maxLength={120}
      placeholder={c.quote ? 'Add a note' : clipName({ ...c, name: '' })}
      onChange={(e) => setText(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
      className="!h-10 !px-2.5 text-[17px] font-semibold tracking-tight"
    />
  )
}

function PublicLink({ clip: c, copied, onCopied, onChange }: { clip: Clip; copied: boolean; onCopied: () => void; onChange: (v: boolean) => void }) {
  const { data: you } = useQuery(allowanceQuery)
  const can = !!you?.canLink || c.public
  if (!can) return null
  return (
    <div className="flex flex-wrap items-center gap-3">
      <Toggle label="Anyone with the link" checked={c.public} onChange={onChange} />
      <div className="min-w-0 flex-1">
        <p className="text-sm">Anyone with the link</p>
        {c.public && !c.linkLive && <p className="text-xs text-ink-3">Links are off for the owner</p>}
      </div>
      {c.public && c.link && c.linkLive && (
        <Button
          size="sm"
          onClick={() =>
            void copyLink(c.link!).then(() => {
              onCopied()
              toast({ title: 'Link copied', tone: 'ok' })
            })
          }
        >
          {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />} Copy link
        </Button>
      )}
      {c.public && !c.linkLive && <Badge tone="warn">Off</Badge>}
    </div>
  )
}
