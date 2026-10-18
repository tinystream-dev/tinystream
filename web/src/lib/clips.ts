// SPDX-License-Identifier: AGPL-3.0-or-later
// What a clip can come out as, worked out the same way the server renders it
// (src/media/clip.rs): fitted into a 16:9 box, at the source's own frame rate
// or half of it, under a bitrate ceiling.

import { useQuery } from '@tanstack/react-query'
import { graphql } from '../gql'
import { type Clip, type ClipAllowance, type Playback, type SubtitleTrack, request } from './api'
import { useClipsOn } from './hooks'

/** A list of clips, and what you may do with them. */
export type ClipList = { clips: Clip[]; you: ClipAllowance }

/** Image subtitles can be burned in, though the player can't show them. */
export const BITMAP_SUBTITLES = ['hdmv_pgs_subtitle', 'dvd_subtitle', 'dvb_subtitle', 'xsub']

export const burnable = (s: SubtitleTrack) => s.supported || BITMAP_SUBTITLES.includes(s.codec)

export type Preset = {
  height: 1080 | 720 | 480
  halfRate: boolean
  width: number
  /** What actually comes out, which can be smaller than the box. */
  outHeight: number
  fps: number
  label: string
}

const BOXES = [1080, 720, 480] as const

/** The frame rate a clip gets: the source's, at most 60, halved when asked (and above 30). */
export function frameRate(source: number, half: boolean) {
  let r = source > 0 ? source : 24
  while (r > 61) r /= 2
  return half && r > 31 ? r / 2 : r
}

function fit(width: number, height: number, box: number): [number, number] {
  const scale = Math.min((box * 16) / 9 / width, box / height, 1)
  const even = (v: number) => Math.max(2, Math.round(v / 2) * 2)
  return [even(width * scale), even(height * scale)]
}

export function presets(video: Playback['media']['video']): Preset[] {
  const { width = 1920, height = 1080, fps = 24 } = video ?? {}
  const out: Preset[] = []
  const seen = new Set<number>()
  for (const box of BOXES) {
    const [w, h] = fit(width, height, box)
    if (seen.has(h)) continue
    seen.add(h)
    const full = frameRate(fps, false)
    const rates = full > 31 ? [false, true] : [false]
    for (const half of rates) {
      const f = frameRate(fps, half)
      out.push({ height: box, halfRate: half, width: w, outHeight: h, fps: f, label: `${h}p${Math.round(f)}` })
    }
  }
  return out
}

/** The ceiling a clip's video stays under, in bits per second. */
export function maxBitrate(height: number, fps: number) {
  const fast = fps > 31
  if (height > 720) return fast ? 8_000_000 : 6_000_000
  if (height > 480) return fast ? 5_000_000 : 3_500_000
  return 1_500_000
}

/** The most a clip can weigh, in bytes. */
export function worstSize(p: Preset, seconds: number) {
  return ((maxBitrate(p.outHeight, p.fps) + 160_000) * seconds) / 8
}

/** "0:42.3": a position with a tenth of a second. */
export function stamp(seconds: number) {
  const s = Math.max(0, seconds)
  const m = Math.floor(s / 60)
  const rest = s - m * 60
  return `${m}:${rest.toFixed(1).padStart(4, '0')}`
}

/** "42 s", "1:05" */
export function length(seconds: number) {
  const s = Math.round(seconds)
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

export function clipName(c: Clip) {
  if (c.name) return c.name
  if (c.quote) return c.source.name
  return [c.source.name, c.source.label].filter(Boolean).join(' ')
}

export const clipSource = (c: Clip) =>
  c.source.kind === 'SHOW' || c.quote
    ? [c.source.name, c.source.label].filter(Boolean).join(' · ')
    : `${c.source.name}${c.source.year ? ` (${c.source.year})` : ''}`

/** A cover tint ("r g b") as a colour, `alpha` see-through. */
export const tint = (rgb: string | null | undefined, alpha: number) => (rgb ? `rgb(${rgb} / ${alpha})` : undefined)

/** Copies a public link, made absolute. */
export async function copyLink(path: string) {
  const url = new URL(path, location.origin).toString()
  await navigator.clipboard.writeText(url)
  return url
}

const ClipQuery = graphql(`
  query Clip($id: Int!) {
    clip(id: $id) {
      ...ClipFields
    }
  }
`)

export const fetchClip = async (id: number) => (await request(ClipQuery, { id })).clip

const RenderingQuery = graphql(`
  query RenderingClips {
    clips(scope: RENDERING) {
      ...ClipFields
    }
  }
`)

/** Your clips still on their way out, oldest first. Kept under ['clips'] so progress ticks reach it. */
export function useRendering(): Clip[] {
  const on = useClipsOn()
  const { data } = useQuery({
    queryKey: ['clips', 'rendering'],
    queryFn: async (): Promise<Pick<ClipList, 'clips'>> => ({ clips: (await request(RenderingQuery)).clips }),
    enabled: on,
  })
  return on ? (data?.clips ?? []) : []
}

/** A clip space limit (in MB, as permissions keep it), in the GB people set it in. */
export const space = (mb: number) => `${Math.round((mb / 1024) * 10) / 10} GB`
