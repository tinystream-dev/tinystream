// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Check, Globe, Headphones, Link2, LogOut, Lock, Power, UserPlus } from 'lucide-react'
import { type CSSProperties, useEffect, useState } from 'react'
import { Avatar } from '../components/Avatar'
import { Login } from '../components/Login'
import { toast } from '../components/feedback'
import { Button, Panel, Popover, Spinner } from '../components/ui'
import { graphql } from '../gql'
import { ApiError, request } from '../lib/api'
import { useMe, usePeople } from '../lib/hooks'
import { useTitle } from '../lib/title'
import { guestName } from '../player/Room'
import { type MusicTrack, cover } from '../music/api'
import { Artists, Cover, Playing, QualityBadge } from '../music/components'
import { Scrubber, Transport, Volume } from '../music/controls'
import { ListenClient, type ListenSnapshot, useListen } from '../music/listen'
import { current, music, usePlayer } from '../music/player'

export const Route = createFileRoute('/listen/$code')({ component: Listen })

const ListenRoomQuery = graphql(`
  query ListenRoom($code: String!) {
    listenRoom(code: $code) {
      code
      hostName
      signedIn
      isHost
      canShare
      canInvite
      tracks {
        ...MusicTrack
      }
    }
  }
`)

async function roomTracks(code: string, ids: number[]): Promise<MusicTrack[]> {
  const { listenRoom } = await request(ListenRoomQuery, { code })
  const byId = new Map(listenRoom.tracks.map((t) => [t.id, t]))
  return ids.map((id) => byId.get(id)).filter((t): t is MusicTrack => !!t)
}

function Listen() {
  const { code } = Route.useParams()
  const me = useMe()
  const { data: info, error } = useQuery({
    queryKey: ['listen', code, me?.id ?? null],
    queryFn: async () => (await request(ListenRoomQuery, { code })).listenRoom,
    retry: false,
    staleTime: Infinity,
  })
  const [client, setClient] = useState<ListenClient | null>(null)
  useTitle(info ? `Listening with ${info.hostName}` : 'Listening together')
  useEffect(() => () => client?.leave(), [client])

  if (error) {
    const kind = error instanceof ApiError ? error.code : ''
    if (kind === 'UNAUTHENTICATED') return <Login setup={false} />
    return <Gone message={kind === 'NOT_FOUND' ? 'This room has ended, or the link isn’t right.' : error.message} />
  }
  if (!info) return <Center><Spinner className="size-9" /></Center>
  if (!client)
    return (
      <Center>
        <div className="flex max-w-sm flex-col items-center text-center">
          <div className="grid size-16 place-items-center rounded-full bg-panel text-ink-2">
            <Headphones className="size-7" />
          </div>
          <p className="mt-5 text-[22px] font-semibold tracking-tight">{info.isHost ? 'Your listening room' : `${info.hostName} is listening`}</p>
          <p className="mt-2 text-[15px] leading-relaxed text-ink-2">
            Everyone here hears the same thing at the same moment. {info.isHost ? 'Send the link to whoever you want here.' : ''}
          </p>
          <Button
            variant="primary"
            size="lg"
            className="mt-6"
            onClick={async () => {
              const c = new ListenClient(code, (ids) => roomTracks(code, ids), info.signedIn ? undefined : (guestName() ?? undefined))
              await c.join()
              setClient(c)
            }}
          >
            <Headphones className="size-4.5" /> {info.isHost ? 'Start listening' : 'Join'}
          </Button>
        </div>
      </Center>
    )
  return <Room client={client} info={info} />
}

function Center({ children }: { children: React.ReactNode }) {
  return <div className="fixed inset-0 grid place-items-center bg-canvas p-6">{children}</div>
}

function Gone({ message }: { message: string }) {
  const navigate = useNavigate()
  const me = useMe()
  return (
    <Center>
      <div className="max-w-sm text-center">
        <p className="text-[22px] font-semibold tracking-tight">Nothing playing here</p>
        <p className="mt-2 text-[15px] leading-relaxed text-ink-2">{message}</p>
        {me && (
          <Button size="lg" className="mt-6" onClick={() => void navigate({ to: '/' })}>
            Go home
          </Button>
        )}
      </div>
    </Center>
  )
}

function Members({ snap }: { snap: ListenSnapshot }) {
  const room = snap.room!
  return (
    <div className="flex items-center -space-x-1.5">
      {room.members.slice(0, 8).map((m) => (
        <span
          key={m.id}
          title={`${m.name}${m.status === 'ready' ? '' : ` (${m.status})`}`}
          className={`relative rounded-full ring-2 ring-canvas transition-opacity ${m.status === 'ready' ? '' : 'opacity-60'}`}
        >
          {m.userId ? (
            <Avatar user={{ id: m.userId, username: m.name, avatar: m.avatar }} size={28} src={`/api/listen/${room.code}/avatars/${m.userId}`} />
          ) : (
            <span className="grid size-7 place-items-center rounded-full bg-panel text-2xs font-medium text-ink-2">{m.name.replace('Guest ', '').slice(0, 2)}</span>
          )}
          {m.status !== 'ready' && <span className="absolute -right-0.5 -bottom-0.5 size-2.5 animate-[pulse-dot_1.6s_ease-in-out_infinite] rounded-full bg-warn ring-2 ring-canvas" />}
        </span>
      ))}
      {room.members.length > 8 && <span className="pl-3 text-xs text-ink-3">+{room.members.length - 8}</span>}
    </div>
  )
}

function RoomMenu({ client, snap, info }: { client: ListenClient; snap: ListenSnapshot; info: { isHost: boolean; canShare: boolean; canInvite: boolean } }) {
  const room = snap.room!
  const { data: people } = usePeople()
  const [copied, setCopied] = useState(false)
  const navigate = useNavigate()
  const here = new Set(room.members.map((m) => m.userId))
  const row = 'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13px] text-ink-2 hover:bg-hover hover:text-ink [&>svg]:size-4'
  return (
    <Popover
      trigger={({ toggle }) => (
        <Button onClick={toggle}>
          <UserPlus className="size-4" /> Invite
        </Button>
      )}
    >
      {() => (
        <Panel className="w-64 p-1.5">
          <button
            className={row}
            onClick={async () => {
              await navigator.clipboard.writeText(location.href)
              setCopied(true)
              setTimeout(() => setCopied(false), 1500)
            }}
          >
            {copied ? <Check /> : <Link2 />} {copied ? 'Copied' : 'Copy link'}
          </button>
          {info.canShare && (
            <button className={row} onClick={() => client.command({ type: 'settings', public: !room.public })}>
              {room.public ? <Globe /> : <Lock />} {room.public ? 'Anyone with the link' : 'Only people here can join'}
            </button>
          )}
          {info.isHost && (
            <button className={row} onClick={() => client.command({ type: 'settings', control: room.settings.control === 'everyone' ? 'host' : 'everyone' })}>
              <Headphones /> {room.settings.control === 'everyone' ? 'Everyone controls it' : 'Only you control it'}
            </button>
          )}
          {info.canInvite && people && people.filter((p) => !here.has(p.id)).length > 0 && (
            <>
              <p className="mt-2 mb-1 px-2.5 text-2xs font-medium tracking-wide text-ink-3 uppercase">Invite</p>
              {people
                .filter((p) => !here.has(p.id))
                .map((p) => (
                  <button
                    key={p.id}
                    className={row}
                    onClick={() => {
                      client.command({ type: 'invite', userId: p.id })
                      toast({ title: `Invited ${p.username}`, tone: 'ok' })
                    }}
                  >
                    <Avatar user={p} size={20} /> {p.username}
                  </button>
                ))}
            </>
          )}
          <div className="my-1 border-t border-line" />
          {info.isHost ? (
            <button className={`${row} !text-danger`} onClick={() => (client.command({ type: 'end' }), void navigate({ to: '/' }))}>
              <Power /> End the room
            </button>
          ) : (
            <button className={row} onClick={() => void navigate({ to: '/' })}>
              <LogOut /> Leave
            </button>
          )}
        </Panel>
      )}
    </Popover>
  )
}

function Room({ client, info }: { client: ListenClient; info: { isHost: boolean; canShare: boolean; canInvite: boolean; hostName: string } }) {
  const snap = useListen(client)!
  const entry = usePlayer(current)
  const queue = usePlayer((s) => s.queue)
  const index = usePlayer((s) => s.index)
  const paused = usePlayer((s) => !s.playing)
  const tint = entry?.track.coverTint ?? null
  const navigate = useNavigate()
  useEffect(() => {
    if (snap.ended) void navigate({ to: '/' })
  }, [snap.ended, navigate])
  if (!snap.room || !entry) return <Center><Spinner className="size-9" /></Center>
  const t = entry.track
  return (
    <div className="fixed inset-0 overflow-hidden bg-canvas" style={{ '--tint': tint ?? '128 128 128' } as CSSProperties}>
      <div aria-hidden className="absolute inset-0 scale-125 opacity-40 blur-[90px] saturate-150">
        {t.cover && <img src={cover(t.cover, 64)} alt="" className="size-full object-cover" />}
      </div>
      <div aria-hidden className="absolute inset-0 bg-canvas/55" />
      <div className="relative mx-auto flex h-full max-w-[1300px] flex-col px-5 pt-[max(1rem,env(safe-area-inset-top))] pb-6 md:px-12">
        <div className="flex h-14 items-center gap-3">
          <Headphones className="size-4.5 text-ink-2" />
          <p className="text-sm font-medium">Listening with {info.isHost ? 'you' : info.hostName}</p>
          {!snap.connected && <span className="text-xs text-warn">Reconnecting…</span>}
          {snap.error && <span className="text-xs text-danger">{snap.error}</span>}
          <div className="flex-1" />
          <Members snap={snap} />
          <RoomMenu client={client} snap={snap} info={info} />
        </div>
        <div className="grid min-h-0 flex-1 gap-10 pt-6 md:grid-cols-[minmax(0,28rem)_minmax(0,1fr)] md:gap-16">
          <div className="flex min-h-0 flex-col justify-center">
            <Cover src={t.cover} size={480} className="aspect-square w-full max-w-[min(28rem,48vh)] self-center shadow-[0_40px_80px_-30px_var(--color-shade)]" />
            <div className="mx-auto mt-6 w-full max-w-[min(28rem,48vh)]">
              <div className="flex items-start gap-3">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-2xl font-semibold tracking-tight">{t.title}</p>
                  <p className="truncate text-[15px] text-ink-2">
                    <Artists track={t} /> · {t.album}
                  </p>
                </div>
                <QualityBadge track={t} />
              </div>
              <div className="mt-4">
                <Scrubber tint={tint} />
              </div>
              <div className="mt-2">
                <Transport size="lg" />
              </div>
              <div className="mx-auto mt-4 max-w-52">
                <Volume />
              </div>
            </div>
          </div>
          <div className="min-h-0 overflow-y-auto pr-1 max-md:hidden">
            <p className="mb-2 px-2 text-xs text-ink-3">Up next</p>
            {queue.map((e, i) => (
              <div
                key={e.uid}
                onClick={() => i !== index && music.jumpTo(i)}
                className={`group flex items-center gap-3 rounded-xl px-2 py-1.5 transition-colors hover:bg-hover ${i < index ? 'opacity-45' : ''} ${i === index ? 'bg-press' : ''}`}
              >
                <span className="relative">
                  <Cover src={e.track.cover} size={40} className="size-10" />
                  {i === index && (
                    <span className="absolute inset-0 grid place-items-center rounded-md bg-media-shade/55 text-media-ink">
                      <Playing paused={paused} />
                    </span>
                  )}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm">{e.track.title}</span>
                  <span className="block truncate text-xs text-ink-3">{e.track.artist}</span>
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
