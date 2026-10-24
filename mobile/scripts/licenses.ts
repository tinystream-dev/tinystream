// SPDX-License-Identifier: AGPL-3.0-or-later
// Writes the THIRD_PARTY_LICENSES the APK ships (into the native project's
// assets, so run it after `expo prebuild`): the libraries and fonts in
// licenses/, the Rust crates the app links (through cargo-about) and every
// package in the JS bundle.

import { spawnSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const app = resolve(import.meta.dir, '..')
const root = resolve(app, '..')
const out = join(app, 'android/app/src/main/assets/THIRD_PARTY_LICENSES')
const rule = '='.repeat(80)

function run(cmd: string, args: string[], cwd: string) {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], maxBuffer: 1 << 28 })
  if (r.error || r.status !== 0) {
    console.error(`${cmd} ${args.join(' ')} failed`)
    process.exit(1)
  }
  return r.stdout
}

const part = (title: string) => `\n\n${'#'.repeat(80)}\n# ${title}\n${'#'.repeat(80)}\n\n`

/** Every package with a module in the release bundle, from its source map. */
function bundled() {
  const dir = mkdtempSync(join(tmpdir(), 'tinystream-bundle-'))
  try {
    run('bunx', ['expo', 'export', '--platform', 'android', '--source-maps', '--output-dir', dir], app)
    const js = join(dir, '_expo/static/js/android')
    const packages = new Set<string>()
    for (const file of readdirSync(js).filter((f) => f.endsWith('.map'))) {
      // Sources are relative to the workspace, which Metro serves from.
      for (const source of JSON.parse(readFileSync(join(js, file), 'utf8')).sources as string[]) {
        const m = join(root, source).match(/^(.*\/node_modules\/(?:@[^/]+\/)?[^/]+)\//)
        if (m) packages.add(m[1])
      }
    }
    return packages
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

function packageLicenses() {
  const sections = [...bundled()].map((dir) => {
    const pkg = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'))
    const file = readdirSync(dir).find((f) => /^(licen[cs]e|copying)/i.test(f))
    const text = file ? readFileSync(join(dir, file), 'utf8').trimEnd() : `Licensed under ${pkg.license ?? 'an unknown license'}.`
    return { name: pkg.name as string, body: `${rule}\n${pkg.name} ${pkg.version}\n${pkg.license ?? ''}\n${rule}\n\n${text}\n` }
  })
  const seen = new Set<string>()
  return sections
    .sort((a, b) => a.name.localeCompare(b.name))
    .filter((s) => !seen.has(s.body) && seen.add(s.body))
    .map((s) => s.body)
    .join('\n')
}

/** The crates in crates/ that end up in the app (the bindings generator only runs at build time). */
function crateLicenses() {
  const crates = readdirSync(join(app, 'crates')).filter((c) => c !== 'uniffi-bindgen')
  const licenses = join(root, 'licenses')
  return crates
    .map((c) =>
      run('cargo', ['about', 'generate', '--locked', '--fail', '-m', join(app, 'crates', c, 'Cargo.toml'), '-c', join(licenses, 'about.toml'), join(licenses, 'about.hbs')], root),
    )
    .join('\n')
}

const header = `tinystream is free software, released under the GNU Affero General Public License
version 3 or later (see LICENSE). It stands on the shoulders of the projects below;
this file carries their copyright notices and licenses. The source of tinystream
itself is at https://github.com/tinystream-dev/tinystream.
`

const libraries = readdirSync(join(app, 'licenses'))
  .filter((f) => f.endsWith('.txt'))
  .sort()
  .map((f) => readFileSync(join(app, 'licenses', f), 'utf8').trimEnd())
  .join('\n\n')

const text = header + part('Libraries and fonts') + libraries + part('Rust crates') + crateLicenses() + part('JavaScript packages') + packageLicenses()
mkdirSync(join(out, '..'), { recursive: true })
writeFileSync(out, text)
console.log(`wrote ${out}`)
