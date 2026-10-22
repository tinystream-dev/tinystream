// SPDX-License-Identifier: AGPL-3.0-or-later
// Signs release builds with the keystore in ANDROID_KEYSTORE_FILE (and
// ANDROID_KEYSTORE_PASSWORD, ANDROID_KEY_ALIAS, ANDROID_KEY_PASSWORD), as CI
// sets them. Without it, release builds are signed with the debug key, which
// is only good for trying them out locally.

import { type ConfigPlugin, withAppBuildGradle } from 'expo/config-plugins'

const RELEASE = `
        release {
            if (System.getenv('ANDROID_KEYSTORE_FILE')) {
                storeFile file(System.getenv('ANDROID_KEYSTORE_FILE'))
                storePassword System.getenv('ANDROID_KEYSTORE_PASSWORD')
                keyAlias System.getenv('ANDROID_KEY_ALIAS')
                keyPassword System.getenv('ANDROID_KEY_PASSWORD')
            }
        }`

const plugin: ConfigPlugin = (config) =>
  withAppBuildGradle(config, (config) => {
    let gradle = config.modResults.contents
    const configs = /signingConfigs \{\n/
    const release = /(release \{[^}]*?)signingConfig signingConfigs\.debug/
    if (!configs.test(gradle) || !release.test(gradle)) throw new Error('signing: android/app/build.gradle has changed shape')
    gradle = gradle.replace(configs, (m) => m + RELEASE.slice(1) + '\n')
    gradle = gradle.replace(
      release,
      "$1signingConfig System.getenv('ANDROID_KEYSTORE_FILE') ? signingConfigs.release : signingConfigs.debug",
    )
    config.modResults.contents = gradle
    return config
  })

export default plugin
