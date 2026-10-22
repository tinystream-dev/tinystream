// SPDX-License-Identifier: AGPL-3.0-or-later
// Pins the NDK that builds everything native: React Native's C++, the
// modules' and our Rust crates (mobile/rust/rust.gradle uses the same one).

import { type ConfigPlugin, withProjectBuildGradle } from 'expo/config-plugins'

const plugin: ConfigPlugin<{ version: string }> = (config, { version }) =>
  withProjectBuildGradle(config, (config) => {
    const anchor = 'buildscript {\n'
    if (!config.modResults.contents.includes(anchor)) throw new Error("ndk: android/build.gradle has no buildscript block")
    config.modResults.contents = config.modResults.contents.replace(anchor, `${anchor}  ext.ndkVersion = '${version}'\n`)
    return config
  })

export default plugin
