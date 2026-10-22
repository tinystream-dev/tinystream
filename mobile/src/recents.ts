// SPDX-License-Identifier: AGPL-3.0-or-later
// Titles opened recently on each server, for the search's empty state (web/src/lib/recents.ts).

import { read, write } from './storage'

export type Recent = { id: number; title: string; poster: string | null; kind: 'show' | 'movie' }

const key = (server: string) => `recents.${server}`

export function recents(server: string): Recent[] {
  return read<Recent[]>(key(server)) ?? []
}

/** Keeps only the recents whose ids are in `ids`, e.g. after titles were deleted. */
export function prune(server: string, ids: Set<number>) {
  const list = recents(server)
  const kept = list.filter((r) => ids.has(r.id))
  if (kept.length !== list.length) write(key(server), kept)
  return kept
}

export function remember(server: string, r: Recent) {
  write(key(server), [r, ...recents(server).filter((x) => x.id !== r.id)].slice(0, 6))
}
