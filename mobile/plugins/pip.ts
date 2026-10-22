// SPDX-License-Identifier: AGPL-3.0-or-later
// Lets the video player go into picture-in-picture: the main activity takes
// it (its configChanges already keep it from being recreated on the way).

import { AndroidConfig, type ConfigPlugin, withAndroidManifest } from 'expo/config-plugins'

const plugin: ConfigPlugin = (config) =>
  withAndroidManifest(config, (config) => {
    const activity = AndroidConfig.Manifest.getMainActivityOrThrow(config.modResults)
    activity.$['android:supportsPictureInPicture'] = 'true'
    return config
  })

export default plugin
