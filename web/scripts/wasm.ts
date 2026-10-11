// SPDX-License-Identifier: AGPL-3.0-or-later
// Builds the web player's Rust (web/decoder, the music player's decoder, and web/subtitles, the
// video player's subtitles) into target/wasm/pkg: cargo for wasm32 with SIMD, then wasm-bindgen,
// then wasm-opt when it's installed.

import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync } from 'node:fs'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dir, '../..')
const target = join(root, 'target/wasm')
const out = join(target, 'pkg')
const crates = ['tinystream_decoder', 'tinystream_subtitles']

// Whatever cargo set for an outer build (rustflags for the host, a linker) mustn't reach this one.
const env = { ...process.env }
for (const key of Object.keys(env)) if (key.startsWith('CARGO_') || key === 'RUSTFLAGS' || key === 'RUSTC_WRAPPER') delete env[key]
env.CARGO_TARGET_DIR = target
// rustyass is written for SIMD; every browser that can run the player has simd128.
env.RUSTFLAGS = '-C target-feature=+simd128'

function run(cmd: string, args: string[], hint: string) {
  const r = spawnSync(cmd, args, { cwd: root, env, stdio: 'inherit' })
  if (r.error || r.status !== 0) {
    console.error(`\n${cmd} failed. ${hint}`)
    process.exit(1)
  }
}

run(
  'cargo',
  ['build', ...crates.flatMap((c) => ['-p', c.replaceAll('_', '-')]), '--target', 'wasm32-unknown-unknown', '--release'],
  'The web player needs the wasm32 target: `rustup target add wasm32-unknown-unknown`.',
)
mkdirSync(out, { recursive: true })
const opt = spawnSync('wasm-opt', ['--version'])
for (const crate of crates) {
  run(
    'wasm-bindgen',
    ['--target', 'web', '--out-dir', out, join(target, `wasm32-unknown-unknown/release/${crate}.wasm`)],
    'Install the wasm-bindgen CLI at the version web/decoder and web/subtitles pin: `cargo install wasm-bindgen-cli --version 0.2.126`.',
  )
  const wasm = join(out, `${crate}_bg.wasm`)
  if (!opt.error && opt.status === 0 && existsSync(wasm)) run('wasm-opt', ['-O3', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-simd', wasm, '-o', wasm], '')
}
