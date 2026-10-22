// SPDX-License-Identifier: AGPL-3.0-or-later
// The last lines the app logged, kept in memory for "Copy debug log". Nothing
// is sent anywhere. Tokens and query strings never make it in.

const MAX = 2000

export type Level = 'debug' | 'info' | 'warn' | 'error'

const lines: string[] = []

/** Takes out what shouldn't be in a log someone pastes into an issue. */
export function redact(text: string) {
  return text
    .replace(/\b((?:https?|wss?|tinystream):\/\/[^\s?#"'<>]*)\?[^\s#"'<>]*/gi, '$1?…')
    .replace(/\b(bearer\s+)[\w\-.~+/]+=*/gi, '$1…')
    .replace(/("?(?:token|password|secret)"?\s*[:=]\s*"?)[^"\s,}]+/gi, '$1…')
}

function show(part: unknown): string {
  if (part instanceof Error) return part.stack ?? `${part.name}: ${part.message}`
  if (typeof part === 'string') return part
  try {
    return JSON.stringify(part)
  } catch {
    return String(part)
  }
}

export function log(level: Level, ...parts: unknown[]) {
  const line = `${new Date().toISOString()} ${level.toUpperCase().padEnd(5)} ${redact(parts.map(show).join(' '))}`
  lines.push(line)
  if (lines.length > MAX) lines.splice(0, lines.length - MAX)
}

/** Everything still kept, oldest first. */
export const logText = () => lines.join('\n')

let installed = false

/** Copies console output and uncaught errors into the log. */
export function installLogging() {
  if (installed) return
  installed = true
  for (const level of ['debug', 'info', 'warn', 'error'] as const) {
    const original = console[level].bind(console)
    console[level] = (...parts: unknown[]) => {
      log(level, ...parts)
      original(...parts)
    }
  }
  const original = console.log.bind(console)
  console.log = (...parts: unknown[]) => {
    log('info', ...parts)
    original(...parts)
  }
  const handler = ErrorUtils.getGlobalHandler()
  ErrorUtils.setGlobalHandler((error, fatal) => {
    log('error', fatal ? 'fatal:' : 'uncaught:', error)
    handler(error, fatal)
  })
}
