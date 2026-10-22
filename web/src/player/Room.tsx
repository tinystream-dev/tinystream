// SPDX-License-Identifier: AGPL-3.0-or-later
// The watch-together parts of the player: who's here, the link, invites,
// the room's settings, and quiet notices when someone else does something.

import { Check, Copy, Crown, Link2, Play } from 'lucide-react'
import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { clock } from '@tinystream/shared/format'
import { Avatar } from '../components/Avatar'
import { Squircle } from '../components/Squircle'
import { Button, Panel, Popover, Segmented, Toggle } from '../components/ui'
import { graphql } from '../gql'
import type { RoomQuery } from '../gql/graphql'
import { usePeople } from '../lib/hooks'
import { type Member, type RoomState, type Status, type SyncSnapshot, type SyncClient, canControl } from './sync'

/** What a room is, and what you may do in it. */
export const RoomInfoQuery = graphql(`
  query Room($code: String!) {
    room(code: $code) {
      code
      signedIn
      isHost
      canShare
      canInvite
      title {
        id
        name
        backdrop
      }
    }
  }
`)

/** A room, as the person arriving sees it. */
export type RoomInfo = RoomQuery['room']
export type RoomContext = { client: SyncClient; info: RoomInfo }

export function useSync(client: SyncClient) {
  return useSyncExternalStore(client.subscribe, client.getSnapshot, client.getSnapshot)
}

const noop = () => () => {}
const nothing = () => null

/** The room as it is now, or null outside of one. */
export function useRoomSnapshot(ctx: RoomContext | undefined): SyncSnapshot | null {
  const c = ctx?.client
  return useSyncExternalStore(c?.subscribe ?? noop, c?.getSnapshot ?? nothing, c?.getSnapshot ?? nothing)
}

/** Rooms whose panel has opened by itself already (once is enough). */
const opened = new Set<string>()

const GUEST_NAME = 'tinystream.guestName'
export const guestName = () => (typeof localStorage === 'undefined' ? null : localStorage.getItem(GUEST_NAME))

const statusText: Record<Status, string | null> = {
  ready: null,
  joining: 'Joining',
  loading: 'Loading',
  buffering: 'Catching up',
  blocked: 'Needs a tap',
}

const avatarSrc = (code: string, m: Member) => (m.userId && m.avatar ? `/api/together/${code}/avatars/${m.userId}?v=${m.avatar}` : null)

function MemberAvatar({ room, m, size }: { room: RoomState; m: Member; size: number }) {
  return <Avatar user={{ id: m.userId ?? -m.id, username: m.name, avatar: m.avatar }} src={avatarSrc(room.code, m)} size={size} />
}

/** Everyone's faces in the corner; opens the room panel. */
export function RoomButton({ ctx }: { ctx: RoomContext }) {
  const snap = useSync(ctx.client)
  // Whoever started the room sees the link straight away.
  const [autoOpen] = useState(() => ctx.info.isHost && !opened.has(ctx.info.code))
  useEffect(() => void opened.add(ctx.info.code), [ctx.info.code])
  const room = snap.room
  if (!room) return null
  const shown = room.members.slice(0, 3)
  const waiting = room.members.some((m) => m.status === 'buffering' || m.status === 'loading')
  return (
    <Popover
      side="bottom"
      trigger={({ toggle, open }) => (
        <OpenOnce when={autoOpen} open={toggle}>
          <Squircle
            as="button"
            radius={12}
            onClick={toggle}
            aria-label="Watching together"
            className={`flex h-10 items-center gap-2 pr-3 pl-1.5 text-media-ink/85 transition-colors hover:bg-media-ink/10 hover:text-media-ink ${open ? 'bg-media-ink/10 text-media-ink' : ''}`}
          >
            <span className="flex -space-x-2">
              {shown.map((m) => (
                <span key={m.id} className="rounded-full ring-2 ring-media-shade/60">
                  <MemberAvatar room={room} m={m} size={26} />
                </span>
              ))}
            </span>
            <span className="text-[13px] tabular">{room.members.length}</span>
            {(waiting || !snap.connected) && <span className="size-1.5 animate-pulse rounded-full bg-warn" />}
          </Squircle>
        </OpenOnce>
      )}
    >
      {() => <RoomPanel ctx={ctx} />}
    </Popover>
  )
}

/** Opens the panel once on arrival, so whoever started the room sees the link. */
function OpenOnce({ when, open, children }: { when: boolean; open: () => void; children: React.ReactNode }) {
  const toggle = useRef(open)
  toggle.current = open
  useEffect(() => {
    if (!when) return
    const t = setTimeout(() => toggle.current(), 300)
    return () => clearTimeout(t)
  }, [when])
  return <>{children}</>
}

function RoomPanel({ ctx }: { ctx: RoomContext }) {
  const { client, info } = ctx
  const snap = useSync(client)
  const room = snap.room!
  const isHost = info.isHost
  const link = `${location.origin}/together/${room.code}`
  const [copied, setCopied] = useState(false)
  const [ending, setEnding] = useState(false)
  const me = room.members.find((m) => m.id === snap.you)

  const copy = async () => {
    try {
      if (navigator.share && matchMedia('(pointer: coarse)').matches) await navigator.share({ url: link, title: info.title.name })
      else await navigator.clipboard.writeText(link)
      setCopied(true)
      setTimeout(() => setCopied(false), 1600)
    } catch {}
  }

  return (
    <Panel className="max-h-[min(36rem,calc(100vh-6rem))] w-[22rem] max-w-[calc(100vw-2.5rem)] overflow-y-auto p-3">
      <div className="flex items-center justify-between px-1">
        <p className="text-[15px] font-medium">Watching together</p>
        {!snap.connected && <span className="text-xs text-warn">Reconnecting…</span>}
      </div>

      <div className="mt-3 flex items-center gap-2">
        <Squircle radius={10} edge className="flex h-9 min-w-0 flex-1 items-center gap-2 bg-raised px-2.5 text-[13px] text-ink-2">
          <Link2 className="size-3.5 shrink-0 text-ink-3" />
          <span className="truncate select-all">{link.replace(/^https?:\/\//, '')}</span>
        </Squircle>
        <Button variant="primary" onClick={copy} className="h-9 w-20">
          {copied ? <Check className="size-4" /> : <Copy className="size-4" />}
          {copied ? 'Copied' : 'Copy'}
        </Button>
      </div>

      <p className="mt-4 mb-1 px-1 text-2xs font-medium tracking-wide text-ink-3 uppercase">Here now</p>
      <div className="space-y-0.5">
        {room.members.map((m) => (
          <div key={m.id} className="flex items-center gap-2.5 rounded-lg px-1 py-1.5">
            <MemberAvatar room={room} m={m} size={28} />
            <p className="min-w-0 flex-1 truncate text-sm">
              {m.name}
              {m.id === snap.you && <span className="ml-1.5 text-ink-3">you</span>}
            </p>
            {m.userId === room.host.id && <Crown className="size-3.5 text-ink-3" aria-label="Started the room" />}
            {statusText[m.status] && (
              <span className={`text-xs ${m.status === 'blocked' ? 'text-ink-3' : 'text-warn'}`}>{statusText[m.status]}</span>
            )}
          </div>
        ))}
      </div>
      {me && me.userId === null && <GuestName client={client} name={me.name} />}

      {info.canInvite && <Invite ctx={ctx} room={room} />}

      {isHost && (
        <>
          <p className="mt-4 mb-1 px-1 text-2xs font-medium tracking-wide text-ink-3 uppercase">Room</p>
          <div className="divide-y divide-line">
            {info.canShare && (
              <Row label="Anyone with the link">
                <Toggle label="Anyone with the link" checked={room.public} onChange={(v) => client.settings({ public: v })} />
              </Row>
            )}
            <Row label="Who controls playback">
              <Segmented
                size="sm"
                value={room.settings.control}
                onChange={(v) => client.settings({ control: v })}
                options={[
                  { value: 'everyone', label: 'Everyone' },
                  { value: 'host', label: 'Only me' },
                ]}
              />
            </Row>
            <Row label="Wait for everyone">
              <Toggle
                label="Wait for everyone"
                checked={room.settings.waitForAll}
                onChange={(v) => client.settings({ waitForAll: v })}
              />
            </Row>
          </div>
          <Button
            variant="danger"
            className="mt-2 w-full"
            onClick={() => (ending ? client.end() : setEnding(true))}
            onBlur={() => setEnding(false)}
          >
            {ending ? 'Sure? This ends it for everyone' : 'End for everyone'}
          </Button>
        </>
      )}
    </Panel>
  )
}

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3 px-1 py-2.5">
      <div className="min-w-0 flex-1">
        <p className="text-sm">{label}</p>
        {hint && <p className="mt-0.5 text-xs text-ink-3">{hint}</p>}
      </div>
      {children}
    </div>
  )
}

function GuestName({ client, name }: { client: SyncClient; name: string }) {
  const [value, setValue] = useState(name)
  useEffect(() => setValue(name), [name])
  const save = () => {
    const v = value.trim()
    if (!v || v === name) return
    localStorage.setItem(GUEST_NAME, v)
    client.rename(v)
  }
  return (
    <form
      className="mt-2 flex items-center gap-2 px-1"
      onSubmit={(e) => {
        e.preventDefault()
        save()
      }}
    >
      <label className="text-xs text-ink-3" htmlFor="guest-name">
        Your name
      </label>
      <input
        id="guest-name"
        value={value}
        maxLength={40}
        onChange={(e) => setValue(e.target.value)}
        onBlur={save}
        className="h-7 min-w-0 flex-1 rounded-lg bg-raised px-2 text-[13px] text-ink outline-none focus:ring-2 focus:ring-ink/30"
      />
    </form>
  )
}


function Invite({ ctx, room }: { ctx: RoomContext; room: RoomState }) {
  const { data: people } = usePeople()
  if (!people?.length) return null
  const here = new Set(room.members.map((m) => m.userId))
  return (
    <>
      <p className="mt-4 mb-1 px-1 text-2xs font-medium tracking-wide text-ink-3 uppercase">Invite</p>
      <div className="space-y-0.5">
        {people.map((p) => {
          const invited = room.invited.includes(p.id)
          return (
            <div key={p.id} className="flex items-center gap-2.5 px-1 py-1">
              <Avatar user={p} size={28} />
              <p className="min-w-0 flex-1 truncate text-sm">{p.username}</p>
              {here.has(p.id) ? (
                <span className="text-xs text-ink-3">Here</span>
              ) : (
                <Button size="sm" variant={invited ? 'plain' : 'quiet'} onClick={() => ctx.client.invite(p.id)}>
                  {invited ? 'Invite again' : 'Invite'}
                </Button>
              )}
            </div>
          )
        })}
      </div>
    </>
  )
}

function describe(a: NonNullable<RoomState['action']>): string | null {
  switch (a.kind) {
    case 'play':
      return `${a.by} pressed play`
    case 'pause':
      return `${a.by} paused`
    case 'seek':
      return `${a.by} jumped to ${clock(a.value ?? 0)}`
    case 'rate':
      return `${a.by} set the speed to ${a.value}×`
    case 'media':
      return `${a.by} changed the episode`
    case 'tracks':
      return `${a.by} changed the audio or subtitles`
    case 'join':
      return `${a.by} joined`
    case 'leave':
      return `${a.by} left`
    default:
      return null
  }
}

/** A quiet line at the top when someone else does something, or the room says no. */
export function RoomNotices({ client }: { client: SyncClient }) {
  const snap = useSync(client)
  const [notice, setNotice] = useState<{ text: string; key: number } | null>(null)
  const seen = useRef<number | null>(null)
  const action = snap.room?.action
  useEffect(() => {
    if (!action) return
    // Whatever happened before we arrived isn't news.
    if (seen.current === null || action.seq <= seen.current) {
      seen.current = Math.max(seen.current ?? 0, action.seq)
      return
    }
    seen.current = action.seq
    if (action.member === snap.you) return
    const text = describe(action)
    if (text) setNotice({ text, key: action.seq })
  }, [action, snap.you])
  useEffect(() => {
    if (snap.error) setNotice({ text: snap.error.message, key: -snap.error.at })
  }, [snap.error])
  useEffect(() => {
    if (!notice) return
    const t = setTimeout(() => setNotice(null), 2600)
    return () => clearTimeout(t)
  }, [notice])
  if (!notice) return null
  return (
    <div className="pointer-events-none absolute inset-x-0 top-20 z-10 flex justify-center">
      <span
        key={notice.key}
        className="animate-[pop_160ms_ease-out] rounded-full bg-media-shade/60 px-4 py-2 text-[13px] text-media-ink backdrop-blur-md"
      >
        {notice.text}
      </span>
    </div>
  )
}

/** When the browser won't play until it's tapped. */
export function TapToJoin({ client, title }: { client: SyncClient; title: string }) {
  return (
    <button
      onClick={() => client.unblock()}
      className="absolute inset-0 z-20 grid place-items-center bg-media-shade/55 backdrop-blur-sm animate-[fade_200ms_ease-out]"
    >
      <span className="flex flex-col items-center gap-4 text-media-ink">
        <span className="grid size-20 place-items-center rounded-full bg-media-ink text-media-shade shadow-2xl">
          <Play className="size-8 translate-x-0.5 fill-current" />
        </span>
        <span className="text-center">
          <span className="block text-[17px] font-medium">Join the others</span>
          <span className="mt-1 block text-sm text-media-ink/60">{title}</span>
        </span>
      </span>
    </button>
  )
}

/** Shown when you can't control playback here. */
export function controlledBy(room: RoomState | null, you: number | null): string | null {
  if (!room || canControl(room, you)) return null
  return room.host.name
}
