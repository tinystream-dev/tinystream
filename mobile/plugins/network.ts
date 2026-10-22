// SPDX-License-Identifier: AGPL-3.0-or-later
// Servers at home: plain http:// on the LAN, and https:// with a certificate
// from someone's own CA. Android blocks the first and, for apps targeting
// API 24+, ignores the user's CAs; this network security config allows both.

import fs from 'node:fs'
import path from 'node:path'
import { AndroidConfig, type ConfigPlugin, withAndroidManifest, withDangerousMod } from 'expo/config-plugins'

const CONFIG = `<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <base-config cleartextTrafficPermitted="true">
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />
        </trust-anchors>
    </base-config>
</network-security-config>
`

const plugin: ConfigPlugin = (config) => {
  config = withDangerousMod(config, [
    'android',
    (config) => {
      const dir = path.join(config.modRequest.platformProjectRoot, 'app/src/main/res/xml')
      fs.mkdirSync(dir, { recursive: true })
      fs.writeFileSync(path.join(dir, 'network_security_config.xml'), CONFIG)
      return config
    },
  ])
  return withAndroidManifest(config, (config) => {
    const app = AndroidConfig.Manifest.getMainApplicationOrThrow(config.modResults)
    app.$['android:usesCleartextTraffic'] = 'true'
    app.$['android:networkSecurityConfig'] = '@xml/network_security_config'
    return config
  })
}

export default plugin
