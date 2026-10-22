// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Headphones, Play, Shuffle } from 'lucide-react'
import type { CSSProperties } from 'react'
import { Empty, Page, Section } from '../components/Page'
import { TitleSkeleton } from '../components/Skeleton'
import { Button } from '../components/ui'
import { graphql } from '../gql'
import { request } from '../lib/api'
import { useTitle } from '../lib/title'
import { hiRes, length, quality } from '../music/api'
import { AlbumGrid, AlbumTile, Cover, QualityBadge, StarButton, TrackList, TrackMenu } from '../music/components'
import { useStartListening } from '../music/listen'
import { current, music, usePlayer } from '../music/player'

export const Route = createFileRoute('/album/$id')({ component: AlbumPage })

const AlbumQuery = graphql(`
  query Album($id: Int!) {
    album(id: $id) {
      ...AlbumCard
      coverTint
      library
      releaseDate
      originalDate
      genres
      releaseTypes
      labels
      discTitles {
        disc
        title
      }
      tracks {
        ...MusicTrack
      }
    }
  }
`)

const MoreQuery = graphql(`
  query MoreByArtist($id: Int!) {
    artist(id: $id) {
      albums {
        ...AlbumCard
      }
    }
  }
`)

function kindOf(types: string[], compilation: boolean): string {
  const t = types.map((x) => x.toLowerCase())
  if (compilation || t.includes('compilation')) return 'Compilation'
  if (t.includes('ep')) return 'EP'
  if (t.includes('single')) return 'Single'
  if (t.includes('live')) return 'Live album'
  return 'Album'
}

function AlbumPage() {
  const id = Number(Route.useParams().id)
  const { data: album, isPending } = useQuery({ queryKey: ['music', 'album', id], queryFn: async () => (await request(AlbumQuery, { id })).album })
  useTitle(album?.name)
  const tint = album?.coverTint ?? null
  const firstArtist = album?.artists[0]?.id
  const { data: more } = useQuery({
    queryKey: ['music', 'more', firstArtist],
    queryFn: async () => (await request(MoreQuery, { id: firstArtist! })).artist?.albums ?? [],
    enabled: firstArtist != null && !album?.compilation,
  })
  const here = usePlayer((s) => current(s)?.track.albumId === id)
  const startListening = useStartListening()
  const playing = usePlayer((s) => s.playing)

  if (isPending) return <TitleSkeleton />
  if (!album) return <Page><Empty title="This album isn't here anymore" /></Page>

  const tracks = album.tracks
  // The best of what it's in, since a few albums mix formats.
  const best = tracks.reduce((a, t) => ((t.bitDepth ?? 0) * 1e6 + (t.sampleRate ?? 0) > (a.bitDepth ?? 0) * 1e6 + (a.sampleRate ?? 0) ? t : a), tracks[0])
  const formats = new Set(tracks.map((t) => quality(t)))
  const year = (album.originalDate ?? album.releaseDate)?.slice(0, 4) ?? album.year
  const others = (more ?? []).filter((a) => a.id !== album.id)

  return (
    <div className="relative">
      <div
        className="pointer-events-none absolute inset-x-0 top-0 h-[40rem] transition-[background] duration-700"
        style={{ background: tint ? `radial-gradient(70% 60% at 15% 0%, rgb(${tint} / 0.24), transparent 70%)` : undefined } as CSSProperties}
      />
      <Page>
        <div className="relative flex flex-col gap-8 pt-2 sm:flex-row sm:items-end md:pt-10">
          <div className="lift w-52 shrink-0 sm:w-60" style={{ '--lift-radius': '20px' } as CSSProperties}>
            <Cover src={album.cover} size={480} className="aspect-square w-full [view-transition-name:cover]" />
          </div>
          <div className="min-w-0 flex-1 pb-1">
            <p className="text-xs font-medium tracking-wide text-ink-3 uppercase">{kindOf(album.releaseTypes, album.compilation)}</p>
            <h1 className="mt-1.5 text-[34px] leading-[1.1] font-semibold tracking-[-0.025em] text-balance md:text-[44px]">{album.name}</h1>
            <p className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-ink-2">
              <span className="font-medium text-ink">
                {album.artists.length && !album.compilation
                  ? album.artists.map((a, i) => (
                      <span key={a.id}>
                        {i > 0 && ', '}
                        <Link to="/artist/$id" params={{ id: String(a.id) }} className="hover:underline">
                          {a.name}
                        </Link>
                      </span>
                    ))
                  : album.artist}
              </span>
              {year && <span>{year}</span>}
              <span className="tabular">
                {album.trackCount} song{album.trackCount === 1 ? '' : 's'}, {length(album.duration)}
              </span>
              {best && <QualityBadge track={best} />}
              {album.genres.length > 0 && <span className="text-ink-3">{album.genres.slice(0, 3).join(', ')}</span>}
            </p>
            <div className="mt-6 flex flex-wrap items-center gap-2">
              <Button variant="primary" size="lg" onClick={() => (here ? music.toggle() : music.play(tracks))}>
                <Play className="size-4.5 fill-current" />
                {here && playing ? 'Pause' : here ? 'Resume' : 'Play'}
              </Button>
              <Button size="lg" onClick={() => music.play(tracks, 0, true)}>
                <Shuffle className="size-4.5" /> Shuffle
              </Button>
              <Button size="lg" variant="plain" onClick={() => startListening(tracks)}>
                <Headphones className="size-4.5" /> Listen together
              </Button>
              <StarButton kind="ALBUM" id={album.id} starred={album.starred} />
              <TrackMenu tracks={tracks} />
            </div>
          </div>
        </div>

        <div className="mt-10">
          <TrackList tracks={tracks} numbers discs discTitles={album.discTitles} />
        </div>

        <p className="mt-8 text-xs leading-relaxed text-ink-3">
          {album.releaseDate && <>Released {album.releaseDate}</>}
          {album.labels.length > 0 && <> · {album.labels.join(', ')}</>}
          {formats.size > 1 && <> · {[...formats].join(', ')}</>}
          {hiRes(best) && <> · Hi-res</>}
        </p>

        {others.length > 0 && (
          <div className="mt-12">
            <Section title={`More by ${album.artists[0]?.name ?? album.artist}`}>
              <AlbumGrid>
                {others.slice(0, 12).map((a) => (
                  <AlbumTile key={a.id} album={a} caption={a.year ?? undefined} />
                ))}
              </AlbumGrid>
            </Section>
          </div>
        )}
      </Page>
    </div>
  )
}
