// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Link, useNavigate, useRouterState } from '@tanstack/react-router'
import {
  ArrowDownToLine,
  Bell,
  BookOpen,
  CalendarDays,
  Clapperboard,
  CornerDownLeft,
  Ellipsis,
  Film,
  House,
  Inbox,
  Keyboard,
  Music,
  ListTodo,
  LogOut,
  Pause,
  Play,
  Plus,
  Scissors,
  Search,
  Settings,
  Tv,
} from 'lucide-react'
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'
import { shortcut } from '@tinystream/shared/format'
import { graphql } from '../gql'
import { type Library, type User, librariesQuery, request } from '../lib/api'
import { disconnect } from '../lib/graphql'
import { speed, useFeatures } from '../lib/downloads'
import { useClipsOn } from '../lib/hooks'
import { prune, recents } from '../lib/recents'
import { Avatar } from './Avatar'
import { Feedback, toast, toastError } from './feedback'
import { Logo } from './Logo'
import { NotificationsMenu, PriorityPill, setInboxOpen } from './Notifications'
import { Squircle } from './Squircle'
import { Ticker } from './Ticker'
import { MusicPill } from '../music/Pill'
import { useFocus } from '../books/focus'
import { albumTracks } from '../music/components'
import { music, usePlayer } from '../music/player'
import type { MusicTrack } from '../music/api'
import { setIconProgress } from '../lib/logo'
import { Dialog, Panel, Popover } from './ui'

function NavLink({ to, params, icon, children, count }: {
  to: string
  params?: Record<string, string>
  icon: ReactNode
  children: ReactNode
  count?: ReactNode
}) {
  return (
    <Link
      to={to}
      params={params}
      activeOptions={{ exact: to === '/' }}
      className="group block outline-none"
    >
      {({ isActive }) => (
        <Squircle
          radius={8}
          className={`flex h-8 items-center gap-2.5 px-2.5 text-sm transition-colors ${isActive ? 'bg-press text-ink' : 'text-ink-2 hover:bg-hover hover:text-ink'}`}
        >
          <span className="relative grid size-4 place-items-center [&>svg]:size-4">{icon}</span>
          <span className="min-w-0 flex-1 truncate">{children}</span>
          {count !== undefined && count !== null && <span className="text-2xs text-ink-3 tabular">{count}</span>}
        </Squircle>
      )}
    </Link>
  )
}

function libraryIcon(l: Library) {
  if (l.kind === 'MUSIC') return <Music />
  if (l.kind === 'BOOKS') return <BookOpen />
  return l.showCount >= l.movieCount ? <Tv /> : <Film />
}

function libraryCount(l: Library) {
  if (l.kind === 'MUSIC') return l.albumCount
  if (l.kind === 'BOOKS') return l.bookCount
  return l.showCount + l.movieCount
}

export function Ring({ value, size = 14, className = '' }: { value: number; size?: number; className?: string }) {
  const r = size / 2 - 1.5
  const c = 2 * Math.PI * r
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className={`-rotate-90 ${className}`} aria-hidden>
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="currentColor" strokeOpacity=".18" strokeWidth="2" />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - Math.max(0, Math.min(1, value)))}
        className="transition-[stroke-dashoffset] duration-700 ease-out"
      />
    </svg>
  )
}

/** One soft ring off the downloads icon each time a download finishes. */
function FinishedPulse({ n }: { n: number }) {
  if (n === 0) return null
  return <span key={n} aria-hidden className="pointer-events-none absolute -inset-1 animate-[ring-done_1.4s_ease-out_both] rounded-full" />
}

const TransfersQuery = graphql(`
  query Transfers {
    downloadEngine {
      ...EngineFields
    }
    downloads {
      ...DownloadFields
    }
  }
`)

const SignOut = graphql(`
  mutation SignOut {
    signOut
  }
`)

const SearchQuery = graphql(`
  query Search($query: String!) {
    musicSearch(query: $query, limit: 5) {
      artists {
        id
        name
        cover
        albumCount
      }
      albums {
        id
        name
        artist
        cover
        year
      }
      tracks {
        ...MusicTrack
      }
    }
    search(query: $query) {
      titles {
        ...Card
      }
      videos {
        id
        label
        name
        title {
          name
        }
      }
    }
  }
`)

const RecentTitles = graphql(`
  query RecentTitles($ids: [Int!]!) {
    titles(ids: $ids) {
      id
    }
  }
`)

const DownloadStates = graphql(`
  query DownloadStates {
    downloads {
      id
      state
    }
  }
`)

const PauseDownloads = graphql(`
  mutation PauseDownloads($ids: [Int!]!) {
    pauseDownloads(ids: $ids) {
      id
    }
  }
`)

const ResumeDownloads = graphql(`
  mutation ResumeDownloads($ids: [Int!]!) {
    resumeDownloads(ids: $ids) {
      id
    }
  }
`)

/** Everything downloading right now, as one number. */
function useTransfers(admin: boolean) {
  const features = useFeatures()
  const enabled = admin && !!features
  const { data } = useQuery({
    queryKey: ['downloads', 'transfers'],
    queryFn: () => request(TransfersQuery),
    enabled,
    refetchInterval: 3000,
  })
  const engine = data?.downloadEngine
  const downloads = data?.downloads
  const active = (downloads ?? []).filter((d) => d.state === 'DOWNLOADING' && d.live)
  const wanted = active.reduce((n, d) => n + (d.size ?? 0), 0)
  const done = active.reduce((n, d) => n + (d.live?.done ?? 0), 0)
  // Counts downloads that finished while we watched, so the icon can pulse for each.
  const [finished, setFinished] = useState(0)
  const downloading = useRef<Set<number> | null>(null)
  useEffect(() => {
    if (!downloads) return
    const now = new Set(downloads.filter((d) => d.state === 'DOWNLOADING').map((d) => d.id))
    const before = downloading.current
    downloading.current = now
    if (before && downloads.some((d) => before.has(d.id) && (d.state === 'SEEDING' || d.state === 'DONE'))) setFinished((n) => n + 1)
  }, [downloads])
  return { engine, active: active.length, progress: wanted > 0 ? done / wanted : null, finished }
}

/** Puts download progress on the tab's icon, and the speed into its title. */
function useTabProgress(progress: number | null, rate: number) {
  const pct = progress === null ? null : Math.round(progress * 50) / 50
  useEffect(() => {
    setIconProgress(pct)
    return () => setIconProgress(null)
  }, [pct])
  useEffect(() => {
    if (!rate) return
    const base = document.title.replace(/^↓ [^·]+ · /, '')
    document.title = `↓ ${speed(rate)} · ${base}`
    return () => {
      document.title = document.title.replace(/^↓ [^·]+ · /, '')
    }
  }, [rate])
}

/** Whether the sidebar is showing (rather than the small-screen bars). */
function useWide() {
  return useSyncExternalStore(
    (l) => {
      const mq = matchMedia('(min-width: 48rem)')
      mq.addEventListener('change', l)
      return () => mq.removeEventListener('change', l)
    },
    () => matchMedia('(min-width: 48rem)').matches,
    () => true,
  )
}

export function Shell({ user, children }: { user: User; children: ReactNode }) {
  const { data: libraries } = useQuery(librariesQuery)
  const [searching, setSearching] = useState(false)
  const [shortcuts, setShortcuts] = useState(false)
  const [more, setMore] = useState(false)
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const transfers = useTransfers(user.permissions.downloads)
  const wide = useWide()
  const clipsOn = useClipsOn()
  // Room at the bottom for the music player, so it never covers the end of a page.
  const focus = useFocus()
  const pill = usePlayer((p) => p.queue.length > 0 && !p.dismissed && !p.suspended) && !focus
  useTabProgress(transfers.progress, transfers.engine?.downloadRate ?? 0)

  const signOut = useCallback(async () => {
    await request(SignOut)
    disconnect()
    queryClient.clear()
    location.href = '/'
  }, [queryClient])

  useEffect(() => {
    let leader = 0
    const onKey = (e: KeyboardEvent) => {
      const typing =
        e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setSearching((s) => !s)
        return
      }
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return
      if (e.key === '/') {
        e.preventDefault()
        setSearching(true)
      } else if (e.key === '?') {
        e.preventDefault()
        setShortcuts((s) => !s)
      } else if (e.key === 'n' && Date.now() - leader >= 1200) {
        e.preventDefault()
        setInboxOpen(true)
      } else if (e.key === 'g') {
        leader = Date.now()
      } else if (Date.now() - leader < 1200) {
        leader = 0
        const to = { h: '/', c: '/calendar', d: '/downloads', w: '/wanted', s: '/settings', a: '/discover', r: '/requests', x: '/clips' }[e.key]
        if (to) {
          e.preventDefault()
          void navigate({ to })
        }
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [navigate])

  return (
    <div className="flex min-h-dvh">
      {!focus && (
        <aside className="sticky top-0 hidden h-dvh w-60 shrink-0 flex-col border-r border-line bg-raised/60 px-2.5 py-3 [view-transition-name:chrome] md:flex">
          <div className="flex items-center gap-2.5 pb-3 pl-2.5">
            <Logo size={20} />
            <span className="flex-1 text-[15px] font-semibold tracking-tight">tinystream</span>
            {wide && <NotificationsMenu align="start" />}
          </div>
          <button onClick={() => setSearching(true)} className="mb-3 block w-full outline-none">
            <Squircle
              radius={9}
              edge
              className="flex h-8 items-center gap-2.5 bg-panel px-2.5 text-sm text-ink-3 transition-colors hover:text-ink-2"
            >
              <Search className="size-4" />
              <span className="flex-1 text-left">Search</span>
              <kbd className="font-sans text-2xs text-ink-3">{shortcut('Ctrl+K')}</kbd>
            </Squircle>
          </button>
          <nav className="space-y-0.5">
            <NavLink to="/" icon={<House />}>
              Home
            </NavLink>
            {clipsOn && (
              <NavLink to="/clips" icon={<Scissors />}>
                Clips
              </NavLink>
            )}
          </nav>
          <DownloadsNav user={user} transfers={transfers} />
          {libraries && libraries.length > 0 && (
            <>
              <p className="mt-5 mb-1 px-2.5 text-xs text-ink-3">Libraries</p>
              <nav className="space-y-0.5">
                {libraries.map((l) => (
                  <NavLink
                    key={l.name}
                    to="/library/$name"
                    params={{ name: l.name }}
                    icon={libraryIcon(l)}
                    count={libraryCount(l)}
                  >
                    {l.name}
                  </NavLink>
                ))}
              </nav>
            </>
          )}
          <div className="flex-1" />
          <AccountMenu user={user} onShortcuts={() => setShortcuts(true)} onSignOut={signOut} />
        </aside>
      )}

      <div className={`min-w-0 flex-1 ${focus ? '' : pill ? 'pb-[calc(8.5rem+env(safe-area-inset-bottom))] md:pb-24' : 'pb-[calc(4.25rem+env(safe-area-inset-bottom))] md:pb-0'}`}>
        {/* Small screens: a slim top bar, and tabs along the bottom. */}
        {!focus && (
          <header className="sticky top-0 z-30 flex h-12 items-center gap-1 material-bar border-b border-line bg-canvas/85 px-3 backdrop-blur-xl [view-transition-name:chrome-top] md:hidden">
            <Link to="/" className="flex items-center gap-2 px-1.5 text-sm font-semibold">
              <Logo size={20} /> tinystream
            </Link>
            <div className="flex-1" />
            {!wide && <NotificationsMenu />}
            <button aria-label="Search" onClick={() => setSearching(true)} className="grid size-9 place-items-center text-ink-2">
              <Search className="size-4.5" />
            </button>
          </header>
        )}
        {children}
      </div>

      {!focus && <TabBar user={user} transfers={transfers} onMore={() => setMore(true)} />}
      {more && (
        <MoreSheet
          user={user}
          libraries={libraries ?? []}
          onClose={() => setMore(false)}
          onSignOut={signOut}
        />
      )}
      {searching && <CommandPalette user={user} onClose={() => setSearching(false)} onShortcuts={() => setShortcuts(true)} />}
      {shortcuts && <Shortcuts onClose={() => setShortcuts(false)} />}
      {!focus && <PriorityPill />}
      {!focus && <MusicPill />}
      <Feedback />
    </div>
  )
}

type Transfers = ReturnType<typeof useTransfers>

function TransferCount({ transfers }: { transfers: Transfers }) {
  const { engine, progress } = transfers
  if (engine?.killSwitch) return <span className="text-danger">VPN down</span>
  if (!engine || engine.downloadRate <= 0) return null
  return (
    <span className="flex items-center gap-1.5 text-info">
      <Ticker value={speed(engine.downloadRate)} />
      {progress !== null && <Ring value={progress} size={13} />}
    </span>
  )
}

/** Calendar, downloads and requests, when this build has them. */
function DownloadsNav({ user, transfers }: { user: User; transfers: Transfers }) {
  const features = useFeatures()
  if (!features) return null
  const can = user.permissions
  return (
    <>
      <p className="mt-5 mb-1 px-2.5 text-xs text-ink-3">Shows</p>
      <nav className="space-y-0.5">
        <NavLink to="/calendar" icon={<CalendarDays />}>
          Calendar
        </NavLink>
        {(can.request || can.manageShows) && (
          <NavLink to="/discover" icon={<Plus />}>
            {can.manageShows ? 'Add shows' : 'Request shows'}
          </NavLink>
        )}
        {(can.request || can.manageRequests) && (
          <NavLink to="/requests" icon={<Inbox />}>
            Requests
          </NavLink>
        )}
        {can.downloads && (
          <NavLink
            to="/downloads"
            icon={
              <>
                <ArrowDownToLine />
                <FinishedPulse n={transfers.finished} />
              </>
            }
            count={<TransferCount transfers={transfers} />}
          >
            Downloads
          </NavLink>
        )}
        {can.manageShows && (
          <NavLink to="/wanted" icon={<ListTodo />}>
            Wanted
          </NavLink>
        )}
      </nav>
    </>
  )
}

function AccountMenu({ user, onShortcuts, onSignOut }: { user: User; onShortcuts: () => void; onSignOut: () => void }) {
  const navigate = useNavigate()
  return (
    <Popover
      side="top"
      align="start"
      trigger={({ toggle, open }) => (
        <button onClick={toggle} className="block w-full outline-none">
          <Squircle
            radius={10}
            className={`flex h-10 items-center gap-2.5 px-2 text-sm transition-colors hover:bg-hover ${open ? 'bg-hover' : ''}`}
          >
            <Avatar user={user} size={24} />
            <span className="min-w-0 flex-1 truncate text-left text-ink-2">{user.username}</span>
            <Ellipsis className="size-4 text-ink-3" />
          </Squircle>
        </button>
      )}
    >
      {(close) => (
        <Panel className="w-56 p-1.5">
          <MenuRow icon={<Settings />} onClick={() => (close(), void navigate({ to: '/settings' }))} hint="G S">
            Settings
          </MenuRow>
          <MenuRow icon={<Keyboard />} onClick={() => (close(), onShortcuts())} hint="?">
            Keyboard shortcuts
          </MenuRow>
          <div className="my-1 h-px bg-line" />
          <MenuRow icon={<LogOut />} onClick={() => (close(), onSignOut())}>
            Sign out
          </MenuRow>
        </Panel>
      )}
    </Popover>
  )
}

function MenuRow({ icon, children, hint, onClick }: { icon: ReactNode; children: ReactNode; hint?: string; onClick: () => void }) {
  return (
    <Squircle
      as="button"
      radius={8}
      onClick={onClick}
      className="flex w-full items-center gap-2.5 px-2.5 py-1.5 text-left text-[13px] text-ink-2 transition-colors hover:bg-hover hover:text-ink"
    >
      <span className="[&>svg]:size-3.5">{icon}</span>
      <span className="flex-1">{children}</span>
      {hint && <kbd className="font-sans text-2xs text-ink-3">{hint}</kbd>}
    </Squircle>
  )
}
function Tab({ to, icon, label, badge }: { to: string; icon: ReactNode; label: string; badge?: ReactNode }) {
  return (
    <Link to={to} activeOptions={{ exact: to === '/' }} className="flex-1 outline-none">
      {({ isActive }) => (
        <span className={`relative flex flex-col items-center gap-1 py-2 text-[10px] font-medium ${isActive ? 'text-ink' : 'text-ink-3'}`}>
          <span className="relative [&>svg]:size-5">
            {icon}
            {badge}
          </span>
          {label}
        </span>
      )}
    </Link>
  )
}

function TabBar({ user, transfers, onMore }: { user: User; transfers: Transfers; onMore: () => void }) {
  const features = useFeatures()
  return (
    <nav className="fixed inset-x-0 bottom-0 z-40 flex material-bar border-t border-line bg-canvas/90 pb-[env(safe-area-inset-bottom)] backdrop-blur-xl [view-transition-name:chrome-bottom] md:hidden">
      <Tab to="/" icon={<House />} label="Home" />
      {features && <Tab to="/calendar" icon={<CalendarDays />} label="Calendar" />}
      {features && (user.permissions.request || user.permissions.manageShows) && (
        <Tab to="/discover" icon={<Plus />} label={user.permissions.manageShows ? 'Add' : 'Request'} />
      )}
      {features && user.permissions.downloads && (
        <Tab
          to="/downloads"
          icon={<ArrowDownToLine />}
          label="Downloads"
          badge={
            <>
              <FinishedPulse n={transfers.finished} />
              {transfers.progress !== null && (
                <span className="absolute -top-1 -right-2 text-info">
                  <Ring value={transfers.progress} size={12} />
                </span>
              )}
            </>
          }
        />
      )}
      <button onClick={onMore} className="flex flex-1 flex-col items-center gap-1 py-2 text-[10px] font-medium text-ink-3">
        <Ellipsis className="size-5" />
        More
      </button>
    </nav>
  )
}

function MoreSheet({
  user,
  libraries,
  onClose,
  onSignOut,
}: {
  user: User
  libraries: Library[]
  onClose: () => void
  onSignOut: () => void
}) {
  const features = useFeatures()
  const clipsOn = useClipsOn()
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const first = useRef(pathname)
  useEffect(() => {
    if (pathname !== first.current) onClose()
  }, [pathname, onClose])
  const item = 'flex h-11 items-center gap-3 px-3 text-[15px] text-ink-2 active:bg-press [&>svg]:size-4.5 [&>svg]:text-ink-3'
  return createPortal(
    <div className="fixed inset-0 z-50 bg-shade/50 animate-[fade_120ms_ease-out] md:hidden" onPointerDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="absolute inset-x-0 bottom-0 max-h-[80dvh] animate-[sheet_220ms_cubic-bezier(.2,.8,.2,1)] overflow-y-auto material rounded-t-[22px] border-t border-line-strong bg-float px-2 pt-2 pb-[calc(0.75rem+env(safe-area-inset-bottom))]">
        <div className="mx-auto mb-2 h-1 w-9 rounded-full bg-line-strong" />
        {libraries.length > 0 && <p className="px-3 pt-2 pb-1 text-xs text-ink-3">Libraries</p>}
        {libraries.map((l) => (
          <Link key={l.name} to="/library/$name" params={{ name: l.name }} className={item}>
            {libraryIcon(l)}
            <span className="flex-1">{l.name}</span>
            <span className="text-xs text-ink-3 tabular">{libraryCount(l)}</span>
          </Link>
        ))}
        {clipsOn && (
          <Link to="/clips" className={item}>
            <Scissors /> Clips
          </Link>
        )}
        {features && <p className="px-3 pt-3 pb-1 text-xs text-ink-3">Shows</p>}
        {features && (user.permissions.request || user.permissions.manageRequests) && (
          <Link to="/requests" className={item}>
            <Inbox /> Requests
          </Link>
        )}
        {features && user.permissions.manageShows && (
          <Link to="/wanted" className={item}>
            <ListTodo /> Wanted
          </Link>
        )}
        <div className="my-2 h-px bg-line" />
        <Link to="/settings" className={item}>
          <Settings /> Settings
        </Link>
        <button onClick={onSignOut} className={`${item} w-full`}>
          <LogOut /> Sign out {user.username}
        </button>
      </div>
    </div>,
    document.body,
  )
}
/** Plays a song from the search with the rest of its album after it. */
async function playTrack(t: MusicTrack) {
  if (t.albumId == null) return music.play([t])
  const tracks = await albumTracks(t.albumId)
  const i = tracks.findIndex((x) => x.id === t.id)
  return i < 0 ? music.play([t]) : music.play(tracks, i)
}

type Command = {
  key: string
  group: string
  title: string
  meta?: string
  icon?: ReactNode
  image?: string | null
  /** Album art is square; posters are tall. */
  square?: boolean
  go: () => void
}

function CommandPalette({ user, onClose, onShortcuts }: { user: User; onClose: () => void; onShortcuts: () => void }) {
  const [q, setQ] = useState('')
  const [debounced, setDebounced] = useState('')
  const [index, setIndex] = useState(0)
  const navigate = useNavigate()
  const qc = useQueryClient()
  const features = useFeatures()
  const clipsOn = useClipsOn()
  const can = user.permissions
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const first = useRef(pathname)
  const list = useRef<HTMLDivElement>(null)
  const { data: libraries } = useQuery(librariesQuery)

  useEffect(() => {
    const t = setTimeout(() => setDebounced(q), 120)
    return () => clearTimeout(t)
  }, [q])
  useEffect(() => {
    if (pathname !== first.current) onClose()
  }, [pathname, onClose])

  const { data } = useQuery({
    queryKey: ['search', debounced],
    queryFn: async () => {
      const r = await request(SearchQuery, { query: debounced })
      return { ...r.search, music: r.musicSearch }
    },
    enabled: debounced.trim().length > 0,
    placeholderData: (prev) => prev,
  })
  const [stored] = useState(recents)
  const { data: recent } = useQuery({
    queryKey: ['recents', stored.map((r) => r.id)],
    queryFn: async () => prune(new Set((await request(RecentTitles, { ids: stored.map((r) => r.id) })).titles.map((t) => t.id))),
    enabled: stored.length > 0,
    gcTime: 0,
  })

  const commands = useMemo(() => {
    const go = (to: string, params?: Record<string, string>) => () => void navigate({ to, params })
    const pages: Command[] = [
      { key: 'p/', group: 'Go to', title: 'Home', icon: <House />, go: go('/'), meta: 'G H' },
      ...(clipsOn ? [{ key: 'p/clips', group: 'Go to', title: 'Clips', icon: <Scissors />, go: go('/clips'), meta: 'G X' }] : []),
      ...(features ? [{ key: 'p/calendar', group: 'Go to', title: 'Calendar', icon: <CalendarDays />, go: go('/calendar'), meta: 'G C' }] : []),
      ...(features && (can.request || can.manageShows)
        ? [{ key: 'p/discover', group: 'Go to', title: can.manageShows ? 'Add shows' : 'Request shows', icon: <Plus />, go: go('/discover'), meta: 'G A' }]
        : []),
      ...(features && can.downloads
        ? [{ key: 'p/downloads', group: 'Go to', title: 'Downloads', icon: <ArrowDownToLine />, go: go('/downloads'), meta: 'G D' }]
        : []),
      ...(features && can.manageShows
        ? [{ key: 'p/wanted', group: 'Go to', title: 'Wanted', icon: <ListTodo />, go: go('/wanted'), meta: 'G W' }]
        : []),
      ...(features && (can.request || can.manageRequests)
        ? [{ key: 'p/requests', group: 'Go to', title: 'Requests', icon: <Inbox />, go: go('/requests'), meta: 'G R' }]
        : []),
      ...(libraries ?? []).map((l) => ({
        key: `l${l.name}`,
        group: 'Go to',
        title: l.name,
        meta: 'Library',
        icon: l.showCount >= l.movieCount ? <Tv /> : <Film />,
        go: go('/library/$name', { name: l.name }),
      })),
      { key: 'p/settings', group: 'Go to', title: 'Settings', icon: <Settings />, go: go('/settings'), meta: 'G S' },
    ]
    const bulk = (action: 'pause' | 'resume') => async () => {
      onClose()
      try {
        const all = (await request(DownloadStates)).downloads
        const targets = all.filter((d) => (action === 'pause' ? d.state === 'DOWNLOADING' || d.state === 'SEEDING' : d.state === 'PAUSED'))
        const ids = targets.map((d) => d.id)
        if (action === 'pause') await request(PauseDownloads, { ids })
        else await request(ResumeDownloads, { ids })
        void qc.invalidateQueries({ queryKey: ['downloads'] })
        toast({ title: `${action === 'pause' ? 'Paused' : 'Resumed'} ${targets.length} download${targets.length === 1 ? '' : 's'}`, tone: 'ok' })
      } catch (e) {
        toastError(e)
      }
    }
    const actions: Command[] = [
      ...(features && can.downloads
        ? [
            { key: 'a/pause', group: 'Actions', title: 'Pause all downloads', icon: <Pause />, go: bulk('pause') },
            { key: 'a/resume', group: 'Actions', title: 'Resume all downloads', icon: <Play />, go: bulk('resume') },
          ]
        : []),
      { key: 'a/inbox', group: 'Actions', title: 'Notifications', icon: <Bell />, meta: 'N', go: () => (onClose(), setInboxOpen(true)) },
      { key: 'a/keys', group: 'Actions', title: 'Keyboard shortcuts', icon: <Keyboard />, meta: '?', go: () => (onClose(), onShortcuts()) },
    ]
    return { pages, actions }
  }, [features, clipsOn, can, libraries, navigate, onClose, onShortcuts, qc])

  const needle = q.trim().toLowerCase()
  const matches = (c: Command) => c.title.toLowerCase().includes(needle)
  const results: Command[] = needle
    ? [
        ...(data?.titles ?? []).map((c) => ({
          key: `i${c.id}`,
          group: 'Titles',
          title: c.name,
          meta: c.kind === 'SHOW' ? `Show · ${c.library}` : `Movie${c.year ? ` · ${c.year}` : ''}`,
          image: c.poster,
          icon: c.kind === 'SHOW' ? <Tv /> : <Film />,
          go: () => void navigate({ to: '/title/$id', params: { id: String(c.id) } }),
        })),
        ...(data?.videos ?? []).map((e) => ({
          key: `e${e.id}`,
          group: 'Episodes',
          title: e.name ?? e.label ?? '',
          meta: `${e.title.name} · ${e.label ?? ''}`,
          icon: <Clapperboard />,
          go: () => void navigate({ to: '/watch/$id', params: { id: String(e.id) } }),
        })),
        ...(data?.music.artists ?? []).map((a) => ({
          key: `ar${a.id}`,
          group: 'Artists',
          title: a.name,
          meta: `${a.albumCount} album${a.albumCount === 1 ? '' : 's'}`,
          image: a.cover ? `${a.cover}?size=48` : null,
          go: () => void navigate({ to: '/artist/$id', params: { id: String(a.id) } }),
        })),
        ...(data?.music.albums ?? []).map((a) => ({
          key: `al${a.id}`,
          group: 'Albums',
          title: a.name,
          meta: `${a.artist}${a.year ? ` · ${a.year}` : ''}`,
          image: a.cover ? `${a.cover}?size=48` : null,
          square: true,
          go: () => void navigate({ to: '/album/$id', params: { id: String(a.id) } }),
        })),
        ...(data?.music.tracks ?? []).map((t) => ({
          key: `tr${t.id}`,
          group: 'Songs',
          title: t.title,
          meta: `${t.artist} · ${t.album}`,
          image: t.cover ? `${t.cover}?size=48` : null,
          square: true,
          go: () => {
            onClose()
            void playTrack(t)
          },
        })),
        ...commands.pages.filter(matches),
        ...commands.actions.filter(matches),
        ...(features && (can.request || can.manageShows)
          ? [
              {
                key: 'discover',
                group: 'Elsewhere',
                title: `${can.manageShows ? 'Add' : 'Request'} “${q.trim()}”`,
                meta: 'Search AniList or TMDB',
                icon: <Plus />,
                go: () => void navigate({ to: '/discover', search: { q: q.trim() } }),
              },
            ]
          : []),
      ]
    : [
        ...(recent ?? []).map((r) => ({
          key: `r${r.id}`,
          group: 'Recent',
          title: r.title,
          meta: r.kind === 'show' ? 'Show' : 'Movie',
          image: r.poster,
          icon: r.kind === 'show' ? <Tv /> : <Film />,
          go: () => void navigate({ to: '/title/$id', params: { id: String(r.id) } }),
        })),
        ...commands.pages,
        ...commands.actions,
      ]

  useEffect(() => setIndex(0), [debounced])
  useEffect(() => {
    list.current?.querySelector(`[data-index="${index}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [index])

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-shade/50 px-4 pt-[12vh] animate-[fade_120ms_ease-out]"
      onPointerDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="lift w-full max-w-xl animate-[pop_140ms_ease-out]">
        <Squircle radius={18} edge className="material bg-float">
          <div className="flex items-center gap-3 border-b border-line px-4">
            <Search className="size-4.5 text-ink-3" />
            <input
              autoFocus
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search, or jump anywhere"
              className="h-13 flex-1 bg-transparent text-[15px] outline-none placeholder:text-ink-3"
              onKeyDown={(e) => {
                if (e.key === 'Escape') onClose()
                else if (e.key === 'ArrowDown') (e.preventDefault(), setIndex((i) => Math.min(results.length - 1, i + 1)))
                else if (e.key === 'ArrowUp') (e.preventDefault(), setIndex((i) => Math.max(0, i - 1)))
                else if (e.key === 'Enter' && results[index]) results[index].go()
              }}
            />
            <kbd className="font-sans text-2xs text-ink-3">Esc</kbd>
          </div>
          <div ref={list} className="max-h-[56vh] overflow-y-auto p-1.5">
            {needle && results.length === 0 && data && (
              <p className="px-3 py-6 text-center text-sm text-ink-3">Nothing called “{debounced}”.</p>
            )}
            {results.map((r, i) => (
              <div key={r.key}>
                {(i === 0 || results[i - 1].group !== r.group) && (
                  <p className="px-3 pt-2.5 pb-1 text-2xs font-medium tracking-wide text-ink-3 uppercase">{r.group}</p>
                )}
                <button data-index={i} className="block w-full text-left" onMouseMove={() => setIndex(i)} onClick={r.go}>
                  <Squircle radius={10} className={`flex items-center gap-3 px-3 py-2 ${i === index ? 'bg-hover' : ''}`}>
                    {r.image !== undefined ? (
                      <Squircle radius={5} className={`${r.square ? 'aspect-square' : 'aspect-[2/3]'} w-6 shrink-0 bg-panel`}>
                        {r.image && <img src={r.image} alt="" loading="lazy" className="size-full object-cover" />}
                      </Squircle>
                    ) : (
                      <span className="grid w-6 shrink-0 place-items-center text-ink-3 [&>svg]:size-4">{r.icon}</span>
                    )}
                    <span className="min-w-0 flex-1 truncate text-sm">{r.title}</span>
                    {r.meta && <span className="shrink-0 truncate text-xs text-ink-3">{r.meta}</span>}
                    {i === index && <CornerDownLeft className="size-3.5 shrink-0 text-ink-3" />}
                  </Squircle>
                </button>
              </div>
            ))}
          </div>
        </Squircle>
      </div>
    </div>
  )
}
const SHORTCUTS: { group: string; keys: [string, string][] }[] = [
  {
    group: 'Anywhere',
    keys: [
      ['Ctrl+K', 'Search and commands'],
      ['/', 'Search'],
      ['?', 'This list'],
      ['N', 'Notifications'],
      ['G then H', 'Home'],
      ['G then C', 'Calendar'],
      ['G then A', 'Add shows'],
      ['G then D', 'Downloads'],
      ['G then W', 'Wanted'],
      ['G then X', 'Clips'],
      ['G then S', 'Settings'],
    ],
  },
  {
    group: 'Watching',
    keys: [
      ['Space  K', 'Play or pause'],
      ['←  →', 'Back or forward 5 s'],
      ['J  L', 'Back or forward 10 s'],
      ['↑  ↓', 'Volume'],
      ['M', 'Mute'],
      ['F', 'Full screen'],
      ['C', 'Next subtitle track'],
      ['S', 'Skip opening or credits'],
      ['N', 'Next episode'],
      ['<  >', 'Slower or faster'],
      ['P', 'Picture in picture'],
      ['X', 'Make a clip'],
      ['Shift+S', 'Take a screenshot'],
      ['0–9', 'Jump to 0–90%'],
    ],
  },
]

export function Shortcuts({ onClose }: { onClose: () => void }) {
  return (
    <Dialog onClose={onClose} width="max-w-2xl">
      <p className="text-[15px] font-medium">Keyboard shortcuts</p>
      <div className="mt-5 grid gap-x-8 gap-y-6 sm:grid-cols-2">
        {SHORTCUTS.map((g) => (
          <div key={g.group}>
            <p className="mb-2 text-xs text-ink-3">{g.group}</p>
            <div className="space-y-1.5">
              {g.keys.map(([k, what]) => (
                <div key={k} className="flex items-center gap-3 text-sm">
                  <span className="flex-1 text-ink-2">{what}</span>
                  <span className="flex gap-1">
                    {k.split(/\s+/).map((part, i) =>
                      part === 'then' ? (
                        <span key={i} className="text-2xs text-ink-3">
                          then
                        </span>
                      ) : (
                        <kbd key={i} className="min-w-6 rounded-md border border-line-strong bg-panel px-1.5 py-0.5 text-center font-sans text-2xs text-ink">
                          {shortcut(part)}
                        </kbd>
                      ),
                    )}
                  </span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </Dialog>
  )
}
