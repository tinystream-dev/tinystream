// SPDX-License-Identifier: AGPL-3.0-or-later

import { describe, expect, test } from 'bun:test'
import { log, logText, redact } from './log'

describe('redact', () => {
  test('drops query strings from URLs', () => {
    expect(redact('GET https://media.example.com/api/stream/4?token=abc&start=10 failed')).toBe(
      'GET https://media.example.com/api/stream/4?… failed',
    )
    expect(redact('ws://10.0.0.2:3000/api/graphql?t=1')).toBe('ws://10.0.0.2:3000/api/graphql?…')
    expect(redact('https://example.com/path stays')).toBe('https://example.com/path stays')
  })

  test('drops bearer tokens and secrets', () => {
    expect(redact('Authorization: Bearer abc.def-ghi==')).toBe('Authorization: Bearer …')
    expect(redact('{"token":"s3cr3t","name":"x"}')).toBe('{"token":"…","name":"x"}')
    expect(redact('password=hunter2 user=me')).toBe('password=… user=me')
  })
})

describe('log', () => {
  test('keeps the last 2000 lines', () => {
    for (let i = 0; i < 2100; i++) log('info', 'line', i)
    const lines = logText().split('\n')
    expect(lines).toHaveLength(2000)
    expect(lines[0]).toEndWith('line 100')
    expect(lines.at(-1)).toEndWith('line 2099')
  })

  test('redacts what it keeps', () => {
    log('warn', new Error('fetch https://x.test/a?token=1'))
    expect(logText()).toContain('https://x.test/a?…')
    expect(logText()).not.toContain('token=1')
  })
})
