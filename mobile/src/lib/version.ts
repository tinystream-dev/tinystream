// SPDX-License-Identifier: AGPL-3.0-or-later
// The app and the server share a version number (0.28.3, or v0.28.3 as a tag).

type Version = [number, number, number]

export function parse(version: string): Version | null {
  const m = /^\s*v?(\d+)\.(\d+)(?:\.(\d+))?/.exec(version)
  return m ? [Number(m[1]), Number(m[2]), Number(m[3] ?? 0)] : null
}

/** "0.28", the part that has to match between the app and the server. */
export function minor(version: string) {
  const v = parse(version)
  return v ? `${v[0]}.${v[1]}` : version
}

/** Whether the app and the server differ in a way that could break things. */
export const mismatched = (app: string, server: string) => minor(app) !== minor(server)

/** Whether `a` comes after `b`. */
export function newer(a: string, b: string) {
  const x = parse(a)
  const y = parse(b)
  if (!x || !y) return false
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i]
  return false
}
