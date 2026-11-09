// SPDX-License-Identifier: AGPL-3.0-or-later
import { graphql } from '../gql'
export const Albums = graphql(`
  query MusicAlbums($library: String, $sort: AlbumSort!, $offset: Int!) {
    albums(library: $library, sort: $sort, offset: $offset, limit: 60) {
      ...AlbumCard
    }
  }
`)
export const Artists = graphql(`
  query MusicArtists($library: String) {
    artists(library: $library) {
      ...ArtistCard
    }
  }
`)
export const Songs = graphql(`
  query MusicSongs($library: String, $query: String!, $offset: Int!) {
    songs(library: $library, query: $query, offset: $offset, limit: 60) {
      ...MusicTrack
    }
  }
`)
export const Playlists = graphql(`
  query MusicPlaylists {
    playlists {
      ...PlaylistCard
    }
  }
`)
export const Album = graphql(`
  query MusicAlbum($id: Int!) {
    album(id: $id) {
      ...AlbumCard
      coverTint
      genres
      tracks {
        ...MusicTrack
      }
    }
  }
`)
export const Artist = graphql(`
  query MusicArtist($id: Int!) {
    artist(id: $id) {
      ...ArtistCard
      coverTint
      albums {
        ...AlbumCard
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
export const Playlist = graphql(`
  query MusicPlaylist($id: Int!) {
    playlist(id: $id) {
      ...PlaylistCard
      tracks {
        ...MusicTrack
      }
    }
  }
`)
export const CreatePlaylist = graphql(`
  mutation MusicCreatePlaylist($name: String!, $tracks: [Int!]!) {
    createPlaylist(name: $name, tracks: $tracks) {
      id
    }
  }
`)
export const UpdatePlaylist = graphql(`
  mutation MusicUpdatePlaylist($id: Int!, $input: PlaylistInput!) {
    updatePlaylist(id: $id, input: $input) {
      id
    }
  }
`)
export const DeletePlaylist = graphql(`
  mutation MusicDeletePlaylist($id: Int!) {
    deletePlaylist(id: $id)
  }
`)
