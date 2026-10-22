// SPDX-License-Identifier: AGPL-3.0-or-later
// No scrollbars anywhere, as on the web (web/src/styles.css): the app's
// theme gives every scrolling view an invisible thumb.

import { AndroidConfig, type ConfigPlugin, withAndroidStyles } from 'expo/config-plugins'

const plugin: ConfigPlugin = (config) =>
  withAndroidStyles(config, (config) => {
    for (const name of ['android:scrollbarThumbVertical', 'android:scrollbarThumbHorizontal']) {
      config.modResults = AndroidConfig.Styles.assignStylesValue(config.modResults, {
        add: true,
        parent: AndroidConfig.Styles.getAppThemeGroup(),
        name,
        value: '@android:color/transparent',
      })
    }
    return config
  })

export default plugin
