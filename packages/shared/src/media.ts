// SPDX-License-Identifier: AGPL-3.0-or-later

import type { MediaCategory } from './schema'

export const mediaLabels: Record<MediaCategory, string> = {
  EPISODES: 'Episodes / Seasons',
  SPECIALS: 'Specials',
  MOVIES: 'Movies',
  OTHER: 'Other / Mixed',
}

export const mediaOptions = [
  { value: '', label: 'All types' },
  ...Object.entries(mediaLabels).map(([value, label]) => ({ value, label })),
]

export function ofMediaType<T extends { category: MediaCategory }>(items: T[], category: string): T[] {
  return category ? items.filter((item) => item.category === category) : items
}
