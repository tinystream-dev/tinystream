// SPDX-License-Identifier: AGPL-3.0-or-later

import { execFileSync } from 'node:child_process'
import { readdirSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig, type Plugin } from 'vite'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// In development, API calls go to a running tinystream (default port 3000).
const api = process.env.TINYSTREAM_API ?? 'http://127.0.0.1:3000'

const BOT = /\[bot\]|^(claude|copilot|blacksmith)\b|@anthropic\.com$/i

// Everyone who authored or co-authored a commit reachable from HEAD, most commits first.
function contributors(): Contributor[] {
  let log: string
  try {
    log = execFileSync('git', ['log', '--format=%aN <%aE>%x1f%(trailers:key=Co-authored-by,valueonly,separator=%x1f)%x1e'], { encoding: 'utf8' })
  } catch {
    return []
  }
  const people = new Map<string, Contributor>()
  for (const commit of log.split('\x1e')) {
    for (const entry of commit.split('\x1f')) {
      const m = entry.trim().match(/^(.+?)\s*<(.*)>$/)
      if (!m || BOT.test(m[1]) || BOT.test(m[2])) continue
      const key = m[1].toLowerCase()
      const person = people.get(key) ?? { name: m[1], github: null, commits: 0 }
      person.github ??= m[2].match(/^(?:\d+\+)?([^@]+)@users\.noreply\.github\.com$/)?.[1] ?? null
      person.commits++
      people.set(key, person)
    }
  }
  return [...people.values()].sort((a, b) => b.commits - a.commits || a.name.localeCompare(b.name))
}

// The licenses of the Rust in the player's wasm (web/decoder and web/subtitles), through cargo-about.
function crateLicenses() {
  const root = fileURLToPath(new URL('..', import.meta.url))
  const crates = ['web/decoder', 'web/subtitles'].map((c) =>
    execFileSync(
      'cargo',
      ['about', 'generate', '--locked', '--fail', '-m', join(root, c, 'Cargo.toml'), '-c', join(root, 'licenses/about.toml'), join(root, 'licenses/about.hbs')],
      { cwd: root, encoding: 'utf8', maxBuffer: 1 << 28 },
    ),
  )
  return crates.join('\n')
}

// Writes the license of every package that ends up in the client bundle, and of the Rust in its wasm, to
// third-party-licenses.txt.
function licenses(): Plugin {
  return {
    name: 'third-party-licenses',
    apply: 'build',
    generateBundle(_, bundle) {
      if (this.environment.name !== 'client') return
      // Assets count too: fonts pulled in through CSS @import never show up as modules.
      const assets = Object.values(bundle).flatMap((f) => (f.type === 'asset' ? f.originalFileNames.map((n) => resolve(n)) : []))
      const packages = new Set<string>()
      for (const id of [...this.getModuleIds(), ...assets]) {
        const m = id.match(/^(.*\/node_modules\/(?:@[^/]+\/)?[^/]+)\//)
        if (m) packages.add(m[1])
      }
      const rule = '='.repeat(80)
      const sections = [...packages].map((dir) => {
        const pkg = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'))
        const file = readdirSync(dir).find((f) => /^(licen[cs]e|copying)/i.test(f))
        const text = file ? readFileSync(join(dir, file), 'utf8').trimEnd() : `Licensed under ${pkg.license ?? 'an unknown license'}.`
        return { name: pkg.name as string, body: `${rule}\n${pkg.name} ${pkg.version}\n${pkg.license ?? ''}\n${rule}\n\n${text}\n` }
      })
      sections.sort((a, b) => a.name.localeCompare(b.name))
      const source = [...sections.map((s) => s.body), crateLicenses()].join('\n')
      this.emitFile({ type: 'asset', fileName: 'third-party-licenses.txt', source })
    },
  }
}

export default defineConfig({
  define: { __CONTRIBUTORS__: JSON.stringify(contributors()) },
  resolve: {
    // The web player's Rust (web/decoder and web/subtitles), built by `bun run wasm`.
    alias: { '#wasm': fileURLToPath(new URL('../target/wasm/pkg', import.meta.url)) },
  },
  server: {
    fs: { allow: ['.', '../target/wasm/pkg', '../assets/fonts'] },
    proxy: { '/api': { target: api, changeOrigin: false, ws: true } },
    headers: {
      // Lets the music player share its ring with the decoder (SharedArrayBuffer).
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'credentialless',
    },
  },
  plugins: [
    tailwindcss(),
    tanstackStart({
      spa: {
        enabled: true,
        prerender: { outputPath: '/index.html' },
      },
    }),
    react(),
    licenses(),
  ],
})
