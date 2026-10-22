// SPDX-License-Identifier: AGPL-3.0-or-later
// Sizes, speeds, times and episode states as both apps write them.

import type { EpisodeState } from './schema'

export function bytes(n: number | null | undefined) {
  if (n == null || !Number.isFinite(n)) return '—'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let i = 0
  let v = n
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000
    i++
  }
  return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`
}

export function speed(n: number | null | undefined) {
  if (!n) return '0 KB/s'
  return `${bytes(n)}/s`
}

/** "3 h 12 min", "45 s" */
export function duration(seconds: number | null | undefined) {
  if (seconds == null || !Number.isFinite(seconds)) return '—'
  const s = Math.max(0, Math.round(seconds))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d) return h ? `${d} d ${h} h` : `${d} d`
  if (h) return m ? `${h} h ${m} min` : `${h} h`
  if (m) return `${m} min`
  return `${s} s`
}

/** "in 2 d 4 h", "5 min ago", "now" */
export function relative(unix: number, now = Date.now()) {
  const diff = unix - now / 1000
  if (Math.abs(diff) < 45) return 'now'
  return diff > 0 ? `in ${duration(diff)}` : `${duration(-diff)} ago`
}

/** A countdown precise to the second for the last hour. */
export function countdown(unix: number, now = Date.now()) {
  const s = Math.round(unix - now / 1000)
  if (s <= 0) return 'airing now'
  if (s < 3600) {
    const m = Math.floor(s / 60)
    return `in ${m}:${String(s % 60).padStart(2, '0')}`
  }
  return `in ${duration(s)}`
}

const dayFormat = new Intl.DateTimeFormat(undefined, { weekday: 'long' })
const timeFormat = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' })
const dateFormat = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' })
const yearFormat = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', year: 'numeric' })

export function startOfDay(d: Date) {
  const x = new Date(d)
  x.setHours(0, 0, 0, 0)
  return x
}

/** "Today at 17:30", "Tomorrow at 9:00", "Wednesday at 17:30", "Oct 14 at 17:30" */
export function airs(unix: number, now = Date.now()) {
  const d = new Date(unix * 1000)
  const days = Math.round((startOfDay(d).getTime() - startOfDay(new Date(now)).getTime()) / 86400000)
  const at = timeFormat.format(d)
  if (days === 0) return `Today at ${at}`
  if (days === 1) return `Tomorrow at ${at}`
  if (days === -1) return `Yesterday at ${at}`
  if (days > 1 && days < 7) return `${dayFormat.format(d)} at ${at}`
  if (Math.abs(days) > 180) return yearFormat.format(d)
  return `${dateFormat.format(d)} at ${at}`
}

export function clockTime(unix: number) {
  return timeFormat.format(new Date(unix * 1000))
}

export function shortDate(unix: number) {
  const d = new Date(unix * 1000)
  return (Math.abs(Date.now() - d.getTime()) > 180 * 86400000 ? yearFormat : dateFormat).format(d)
}

export function episodeCode(season: number, episode: number) {
  if (season === 0) return `Special ${episode}`
  return `S${String(season).padStart(2, '0')}E${String(episode).padStart(2, '0')}`
}

export const stateLabels: Record<EpisodeState, string> = {
  IDLE: 'Not monitored',
  WANTED: 'Searching',
  GRABBED: 'Downloading',
  MISSING: 'Missing',
  DONE: 'In library',
  SKIPPED: 'Skipped',
}

export const monitorLabels = {
  NONE: { label: 'Off' },
  FUTURE: { label: 'New episodes' },
  MISSING: { label: 'Everything' },
} as const
