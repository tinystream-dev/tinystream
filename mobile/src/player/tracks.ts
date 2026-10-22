// SPDX-License-Identifier: AGPL-3.0-or-later
// Picking and naming tracks, and the chapters worth skipping, as web's
// Player.tsx does.

import { language } from '@tinystream/shared/format'
import type { Playback } from './plan'

export const SKIPPABLE = /^(op|opening|intro|ed|ending|credits|preview|next episode|recap)\b/i

export function skipLabel(title: string) {
  const t = title.toLowerCase()
  if (/^(op|opening|intro)/.test(t)) return 'Skip opening'
  if (/^(ed|ending|credits)/.test(t)) return 'Skip credits'
  if (/^recap/.test(t)) return 'Skip recap'
  return 'Skip preview'
}

/** The audio track to start with: the language picked last, else the file's default. */
export function defaultAudio(pb: Playback, lang: string | null): number | null {
  const a = (lang && pb.media.audio.find((t) => t.language === lang)) || pb.media.audio.find((t) => t.default)
  return a?.index ?? null
}

/** The subtitles to start with (in the language picked last), unless they were turned `off` last time. */
export function defaultSubtitle(pb: Playback, off: boolean, lang: string | null): string | null {
  const usable = pb.media.subtitles.filter((s) => s.supported)
  if (!usable.length || off) return null
  return (
    (lang && usable.find((s) => s.language === lang && !s.forced)?.id) ||
    usable.find((s) => s.default)?.id ||
    usable.find((s) => !s.forced)?.id ||
    usable[0].id
  )
}

export function trackName(t: { title: string | null; language: string | null }, fallback: string) {
  const lang = language(t.language)
  if (t.title && lang && !t.title.toLowerCase().includes(lang.toLowerCase())) return `${lang} · ${t.title}`
  return t.title || lang || fallback
}
