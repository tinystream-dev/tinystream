// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { graphql } from '../gql'
import { type User, request } from './api'

const StatusQuery = graphql(`
  query Status {
    server {
      setupRequired
      clips
      downloads
      sources
      smtp
      email
      publicUrl
    }
    viewer {
      ...Viewer
    }
  }
`)

/** Who's signed in, and what this server can do. */
export const useStatus = () => useQuery({ queryKey: ['auth'], queryFn: () => request(StatusQuery) })

export function useMe(): User | null {
  return useStatus().data?.viewer ?? null
}

/** Whether clipping is on here, and this person may make clips. */
export function useCanClip(): boolean {
  const { data } = useStatus()
  return !!data?.server.clips && !!data.viewer?.permissions.clip
}

/** Whether clipping is on here at all (clips sent to you still show if you can't make them). */
export function useClipsOn(): boolean {
  return !!useStatus().data?.server.clips
}

/** Whether this server can send email: invite links, and password resets for people who gave an address. */
export function useEmailOn(): boolean {
  return !!useStatus().data?.server.email
}

/** Where links to this server should point: its public address, or else wherever you opened it. */
export function useOrigin(): string {
  return useStatus().data?.server.publicUrl ?? location.origin
}

const PeopleQuery = graphql(`
  query People {
    users {
      ...Person
    }
  }
`)

/** Everyone else on this server, by name: people to invite or send things to. */
export function usePeople() {
  const me = useMe()
  return useQuery({
    queryKey: ['people'],
    queryFn: async () => (await request(PeopleQuery)).users,
    select: (users) =>
      users.filter((u) => u.id !== me?.id).sort((a, b) => a.username.localeCompare(b.username, undefined, { sensitivity: 'base' })),
  })
}
