// SPDX-License-Identifier: AGPL-3.0-or-later

const path = require('node:path')
const { getDefaultConfig } = require('expo/metro-config')
const { withNativeWind } = require('nativewind/metro')

const root = path.resolve(__dirname, '..')
const config = getDefaultConfig(__dirname)

// packages/shared (and the workspace's node_modules) live above the app, so
// Metro has to watch them for Fast Refresh to pick their changes up.
config.watchFolders = [root]
config.resolver.nodeModulesPaths = [path.resolve(__dirname, 'node_modules'), path.resolve(root, 'node_modules')]

// The workspace root has web's React; everything in the app, React Native
// included, has to use the one React Native was built for, in mobile/.
const own = /^react(-dom)?(\/|$)/
const app = path.join(__dirname, 'package.json')
config.resolver.resolveRequest = (context, name, platform) =>
  context.resolveRequest(own.test(name) ? { ...context, originModulePath: app } : context, name, platform)

module.exports = withNativeWind(config, { input: './global.css' })
