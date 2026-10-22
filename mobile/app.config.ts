// SPDX-License-Identifier: AGPL-3.0-or-later

import type { ExpoConfig } from 'expo/config'
import { version } from './package.json'

/** The NDK everything native is built with, Rust included; its default is 16 KB pages. */
export const NDK = '30.0.16248370'

const [major, minor, patch] = version.split('.').map(Number)

const config: ExpoConfig = {
  name: 'tinystream',
  slug: 'tinystream',
  scheme: 'tinystream',
  version,
  orientation: 'portrait',
  icon: './assets/icon.png',
  userInterfaceStyle: 'automatic',
  android: {
    package: 'dev.tinystream.app',
    versionCode: major * 1_000_000 + minor * 1_000 + patch,
    adaptiveIcon: {
      backgroundColor: '#191919',
      foregroundImage: './assets/android-icon-foreground.png',
      monochromeImage: './assets/android-icon-monochrome.png',
    },
    predictiveBackGestureEnabled: true,
  },
  plugins: [
    'expo-router',
    ['expo-splash-screen', { image: './assets/splash-icon.png', imageWidth: 160, backgroundColor: '#191919' }],
    'expo-secure-store',
    'expo-sqlite',
    'expo-background-task',
    ['expo-image-picker', { cameraPermission: false, microphonePermission: false }],
    [
      'expo-build-properties',
      {
        android: {
          minSdkVersion: 34,
          targetSdkVersion: 36,
          compileSdkVersion: 36,
          // Debug builds also run on x86_64 emulators; release builds are arm64 only (see `bun run release`).
          buildArchs: ['arm64-v8a', 'x86_64'],
        },
      },
    ],
    [
      'expo-font',
      {
        android: {
          fonts: [
            {
              fontFamily: 'Geist',
              fontDefinitions: [
                { path: './assets/fonts/Geist-Regular.ttf', weight: 400 },
                { path: './assets/fonts/Geist-Medium.ttf', weight: 500 },
                { path: './assets/fonts/Geist-SemiBold.ttf', weight: 600 },
                { path: './assets/fonts/Geist-Bold.ttf', weight: 700 },
              ],
            },
          ],
        },
      },
    ],
    ['./plugins/ndk', { version: NDK }],
    './plugins/signing',
    './plugins/scrollbars',
    './plugins/network',
    './plugins/pip',
  ],
  experiments: {
    typedRoutes: true,
  },
}

export default config
