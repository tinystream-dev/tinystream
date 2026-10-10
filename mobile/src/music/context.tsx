// SPDX-License-Identifier: AGPL-3.0-or-later
import { createMusicPlayer, type MusicPlayer } from '@tinystream/shared/music-player'
import { createContext, useContext, useEffect, useMemo, useSyncExternalStore, type ReactNode } from 'react'
import { AppState } from 'react-native'
import { Audio } from '../../modules/audio'
import { useConnection, useSession, type Connection } from '../session'
import type { Api } from '../lib/graphql'
import { read, write } from '../storage'
import { Engine } from './engine'
import { MeasureLoudness, NowPlaying, Played, PlayQueueQuery, SavePlayQueue, Star, TrackQuery } from './api'

const Context = createContext<MusicPlayer | null>(null)
let active: { id: string; token: string | null; player: MusicPlayer; release(): void } | null = null
export function stopMusic() {
  active?.release()
  active = null
}
export function MusicProvider({ children }: { children: ReactNode }) {
  const connection = useConnection()
  useEffect(() => {
    if (!connection?.token) {
      active?.release()
      active = null
    }
  }, [connection?.token])
  return connection?.token ? <SignedInMusic>{children}</SignedInMusic> : <>{children}</>
}
function SignedInMusic({ children }: { children: ReactNode }) {
  const { api, server } = useSession()
  const connection = useConnection()!
  const player = useMemo(() => controller(server.id, api, connection), [server.id, connection.token, api])
  return <Context.Provider value={player}>{children}</Context.Provider>
}
function controller(id: string, api: Api, connection: Connection): MusicPlayer {
  if (active?.id === id && active.token === connection.token) return active.player
  active?.release()
  let engine: Engine | null = null
  const prefix = `music.${id}.`
  const player = createMusicPlayer({
    extraCodecs: ['opus'],
    storage: {
      getItem: (key) => read<string>(prefix + key),
      setItem: (key, value) => write(prefix + key, value),
      removeItem: (key) => write(prefix + key, null),
    },
    animate: (fn) => requestAnimationFrame(fn),
    cancelAnimation: (id) => cancelAnimationFrame(id),
    engine: () => (engine = new Engine(connection)),
    supported: () => true,
    queue: () => api.request(PlayQueueQuery),
    track: (id) => api.request(TrackQuery, { id }),
    measure: (trackId) => api.request(MeasureLoudness, { trackId }),
    save: (input) => api.request(SavePlayQueue, { input }),
    played: (trackId) => api.request(Played, { trackId }),
    nowPlaying: (trackId, position, paused) => api.request(NowPlaying, { trackId, position, paused }),
    metadata(t) {
      const s = player.getState()
      engine?.metadata({
        queue: s.queue.map((e) => ({ key: e.uid, title: e.track.title, artist: e.track.artist, album: e.track.album, duration: e.track.duration })),
        index: s.index,
        key: player.current()?.uid ?? '',
        title: t.title,
        artist: t.artist,
        album: t.album,
        artwork: t.cover,
        duration: t.duration,
        starred: t.starred,
        shuffled: s.shuffled,
        repeat: s.repeat === 'ALL' ? 2 : s.repeat === 'ONE' ? 1 : 0,
      })
    },
  })
  void player.restore()
  const commands = Audio.addListener('command', ({ action, time }) => {
    const m = player.music
    switch (action) {
      case 'play':
        void m.resume()
        break
      case 'pause':
        m.pause()
        break
      case 'focusPause':
        m.pause(true)
        break
      case 'next':
        m.next()
        break
      case 'previous':
        m.previous()
        break
      case 'jump':
        m.jumpTo(time)
        break
      case 'seek':
        m.seek(time)
        break
      case 'star': {
        const t = player.current()?.track
        if (t)
          void api
            .request(Star, { kind: 'TRACK', id: t.id, starred: !t.starred })
            .then(() => player.updateTrack({ ...t, starred: !t.starred }))
            .catch(() => {})
        break
      }
      case 'stop':
        m.clear()
        break
      case 'shuffle':
        m.shuffle()
        break
      case 'repeat':
        m.repeat(time === 1 ? 'ONE' : time === 2 ? 'ALL' : player.getState().repeat === 'OFF' ? 'ALL' : player.getState().repeat === 'ALL' ? 'ONE' : 'OFF')
        break
    }
  })
  let savedAt = Date.now()
  const progress = Audio.addListener('position', () => {
    player.tick()
    if (Date.now() - savedAt >= 30000) {
      savedAt = Date.now()
      void player.flush()
    }
  })
  const state = AppState.addEventListener('change', (s) => {
    if (s !== 'active') void player.flush()
    else void player.refreshFromServer().catch(() => {})
  })
  active = {
    id,
    token: connection.token,
    player,
    release() {
      commands.remove()
      progress.remove()
      state.remove()
      player.dispose()
      engine?.dispose()
    },
  }
  return player
}

export function useMusic() {
  const player = useContext(Context)
  if (!player) throw new Error('Music needs a signed-in server')
  return player
}

export function useMusicSpace() {
  const player = useContext(Context)
  const state = useSyncExternalStore(player?.subscribe ?? (() => () => {}), () => player?.getState())
  return state && state.queue.length && !state.dismissed && !state.suspended ? 78 : 0
}
