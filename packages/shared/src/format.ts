// SPDX-License-Identifier: AGPL-3.0-or-later

export function clock(seconds: number | null | undefined) {
  if (seconds == null || !Number.isFinite(seconds)) return '0:00'
  const s = Math.max(0, Math.floor(seconds))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = String(s % 60).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
}

/** "24 min", "1 h 52 min" */
export function runtime(seconds: number | null | undefined) {
  if (!seconds) return null
  const m = Math.round(seconds / 60)
  if (m < 60) return `${m} min`
  return `${Math.floor(m / 60)} h ${m % 60} min`
}

export function remaining(position: number | null, duration: number | null) {
  if (!position || !duration) return null
  const m = Math.max(1, Math.round((duration - position) / 60))
  return `${m} min left`
}

const LANGUAGES: Record<string, string> = {
  eng: 'English', en: 'English', jpn: 'Japanese', ja: 'Japanese', ger: 'German', deu: 'German', de: 'German',
  fre: 'French', fra: 'French', fr: 'French', spa: 'Spanish', es: 'Spanish', ita: 'Italian', it: 'Italian',
  por: 'Portuguese', pt: 'Portuguese', rus: 'Russian', ru: 'Russian', chi: 'Chinese', zho: 'Chinese', zh: 'Chinese',
  kor: 'Korean', ko: 'Korean', ara: 'Arabic', ar: 'Arabic', hun: 'Hungarian', hu: 'Hungarian', pol: 'Polish',
  pl: 'Polish', dut: 'Dutch', nld: 'Dutch', nl: 'Dutch', swe: 'Swedish', sv: 'Swedish', tur: 'Turkish', tr: 'Turkish',
}

export function language(code: string | null | undefined) {
  if (!code) return null
  return LANGUAGES[code.toLowerCase()] ?? code
}

const MAC = typeof navigator !== 'undefined' && /mac|iphone|ipad/i.test(navigator.platform)
const MAC_MODIFIERS: Record<string, string> = { Ctrl: '⌘', Alt: '⌥', Shift: '⇧' }

/** "Ctrl+K" → "⌘K" on macOS, "Ctrl + K" elsewhere */
export function shortcut(combo: string) {
  const keys = combo.split('+')
  return MAC ? keys.map((k) => MAC_MODIFIERS[k] ?? k).join('') : keys.join(' + ')
}
