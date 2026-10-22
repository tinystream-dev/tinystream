// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Play, Shuffle } from 'lucide-react'
import { type CSSProperties, useState } from 'react'
import { Empty, Page, Section } from '../components/Page'
import { TitleSkeleton } from '../components/Skeleton'
import { Button } from '../components/ui'
import { graphql } from '../gql'
import { request } from '../lib/api'
import { useTitle } from '../lib/title'
import { AlbumGrid, AlbumTile, Cover, StarButton, TrackList } from '../music/components'
import { music } from '../music/player'

export const Route = createFileRoute('/artist/$id')({ component: ArtistPage })

const ArtistQuery = graphql(`
  query Artist($id: Int!) {
    artist(id: $id) {
      ...ArtistCard
      coverTint
      albums {
        ...AlbumCard
        releaseTypes
      }
      appearsOn {
        ...AlbumCard
      }
      topTracks(count: 200) {
        ...MusicTrack
      }
    }
  }
`)

function ArtistPage() {
  const id = Number(Route.useParams().id)
  const { data: artist, isPending } = useQuery({ queryKey: ['music', 'artist', id], queryFn: async () => (await request(ArtistQuery, { id })).artist })
  useTitle(artist?.name)
  const tint = artist?.coverTint ?? null
  const [all, setAll] = useState(false)

  if (isPending) return <TitleSkeleton />
  if (!artist) return <Page><Empty title="This artist isn't here anymore" /></Page>

  const top = artist.topTracks
  const isSingle = (types: string[]) => types.some((t) => ['single', 'ep'].includes(t.toLowerCase()))
  const albums = artist.albums.filter((a) => !isSingle(a.releaseTypes))
  const singles = artist.albums.filter((a) => isSingle(a.releaseTypes))

  return (
    <div className="relative">
      <div
        className="pointer-events-none absolute inset-x-0 top-0 h-[36rem]"
        style={{ background: tint ? `radial-gradient(60% 60% at 50% 0%, rgb(${tint} / 0.24), transparent 70%)` : undefined } as CSSProperties}
      />
      <Page>
        <div className="relative flex flex-col items-center pt-4 text-center md:pt-12">
          <Cover src={artist.cover} size={260} round className="size-44 shadow-[0_30px_60px_-28px_var(--color-shade)] md:size-52" />
          <h1 className="mt-6 text-[36px] leading-tight font-semibold tracking-[-0.025em] md:text-[48px]">{artist.name}</h1>
          <p className="mt-1 text-sm text-ink-2 tabular">
            {artist.albumCount} album{artist.albumCount === 1 ? '' : 's'} · {artist.trackCount} song{artist.trackCount === 1 ? '' : 's'}
          </p>
          <div className="mt-6 flex items-center gap-2">
            <Button variant="primary" size="lg" disabled={!top.length} onClick={() => music.play(top)}>
              <Play className="size-4.5 fill-current" /> Play
            </Button>
            <Button size="lg" disabled={!top.length} onClick={() => music.play(top, 0, true)}>
              <Shuffle className="size-4.5" /> Shuffle
            </Button>
            <StarButton kind="ARTIST" id={artist.id} starred={artist.starred} />
          </div>
        </div>

        {top.length > 0 && (
          <div className="mx-auto mt-12 max-w-4xl">
            <Section
              title="Popular"
              aside={
                top.length > 5 && (
                  <button onClick={() => setAll(!all)} className="text-xs text-ink-3 hover:text-ink">
                    {all ? 'Fewer' : 'More'}
                  </button>
                )
              }
            >
              <TrackList tracks={top.slice(0, all ? 20 : 5)} showAlbum />
            </Section>
          </div>
        )}

        {albums.length > 0 && (
          <Section title="Albums">
            <AlbumGrid>
              {albums.map((a) => (
                <AlbumTile key={a.id} album={a} caption={a.year ?? undefined} />
              ))}
            </AlbumGrid>
          </Section>
        )}
        {singles.length > 0 && (
          <Section title="Singles and EPs">
            <AlbumGrid>
              {singles.map((a) => (
                <AlbumTile key={a.id} album={a} caption={a.year ?? undefined} />
              ))}
            </AlbumGrid>
          </Section>
        )}
        {artist.appearsOn.length > 0 && (
          <Section title="Appears on">
            <AlbumGrid>
              {artist.appearsOn.map((a) => (
                <AlbumTile key={a.id} album={a} />
              ))}
            </AlbumGrid>
          </Section>
        )}
      </Page>
    </div>
  )
}
