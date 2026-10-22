// SPDX-License-Identifier: AGPL-3.0-or-later
// Versions: the app's against the server's, and against the latest release
// on GitHub (at most once a day, and only while "Check for updates" is on).
// The debug screen can pretend any of them is something else.

import Constants from 'expo-constants'
import { useEffect, useState } from 'react'
import { minor, mismatched, newer } from './lib/version'
import { stored } from './storage'

const RELEASES = 'https://api.github.com/repos/tinystream-dev/tinystream/releases/latest'
const DAY = 24 * 3600 * 1000

/** Versions the debug screen pretends to be, to try the banner and the update sheet. */
export const fakeVersions = stored<{ app?: string; server?: string; latest?: string }>('debug.versions', {})

export const checkForUpdates = stored('updates.check', true)
const lastCheck = stored('updates.last', 0)
const dismissedMismatches = stored<string[]>('mismatch.dismissed', [])

export function useAppVersion() {
  return fakeVersions.use().app || Constants.expoConfig?.version || '0.0.0'
}

export type Mismatch = { app: string; server: string; update: 'app' | 'server'; dismiss: () => void }

/** The app's and the server's versions, when they don't go together and that hasn't been waved away. */
export function useMismatch(serverId: string, realServer: string | undefined): Mismatch | null {
  const app = useAppVersion()
  const server = fakeVersions.use().server || realServer
  const dismissed = dismissedMismatches.use()
  if (!server || !mismatched(app, server)) return null
  const key = `${serverId}@${minor(server)}`
  if (dismissed.includes(key)) return null
  return {
    app: minor(app),
    server: minor(server),
    update: newer(app, server) ? 'server' : 'app',
    dismiss: () => dismissedMismatches.set([...dismissedMismatches.get().filter((k) => k !== key), key]),
  }
}

export type Release = { version: string; notes: string; download: string }

type GitHubRelease = { tag_name: string; html_url: string; assets?: { name: string; browser_download_url: string }[] }

async function latest(): Promise<Release | null> {
  const fake = fakeVersions.get().latest
  if (fake)
    return { version: fake.replace(/^v/, ''), notes: 'https://github.com/tinystream-dev/tinystream/releases', download: 'https://github.com/tinystream-dev/tinystream/releases' }
  const res = await fetch(RELEASES, { headers: { accept: 'application/vnd.github+json' }, credentials: 'omit' })
  // No release yet.
  if (res.status === 404) return null
  if (!res.ok) throw new Error(`GitHub said ${res.status}`)
  const release: GitHubRelease = await res.json()
  const apk = release.assets?.find((a) => a.name.endsWith('.apk'))
  return { version: release.tag_name.replace(/^v/, ''), notes: release.html_url, download: apk?.browser_download_url ?? release.html_url }
}

/** A newer release, looked for once when the app starts. */
export function useUpdate(): { release: Release | null; open: boolean; dismiss: () => void } {
  const [release, setRelease] = useState<Release | null>(null)
  const [open, setOpen] = useState(false)
  const app = useAppVersion()
  useEffect(() => {
    const faking = !!fakeVersions.get().latest
    if (!faking && (!checkForUpdates.get() || Date.now() - lastCheck.get() < DAY)) return
    let stale = false
    latest()
      .then((r) => {
        if (!faking) lastCheck.set(Date.now())
        if (!stale && r && newer(r.version, app)) {
          setRelease(r)
          setOpen(true)
        }
      })
      .catch((e) => console.warn('update check:', e))
    return () => {
      stale = true
    }
  }, [app])
  return { release, open, dismiss: () => setOpen(false) }
}
