// SPDX-License-Identifier: AGPL-3.0-or-later
// Posting local notifications (NotifyModule.kt). There's no push: the
// background check (src/background.ts) asks each server and posts what's new.

import { requireNativeModule } from 'expo'

export type LocalNotice = { id: number; title: string; body?: string | null; link?: string | null }

const Notify = requireNativeModule<{
  enabled(): boolean
  post(server: string, serverName: string, notices: LocalNotice[]): void
  openSettings(): void
}>('TinystreamNotify')

/** Whether Android lets the app post notifications at all. */
export const notificationsEnabled = () => Notify.enabled()

/** Posts notices from a server, grouped with its others; tapping one opens it there. */
export const postNotices = (server: string, serverName: string, notices: LocalNotice[]) => Notify.post(server, serverName, notices)

/** Android's notification settings for the app. */
export const openNotificationSettings = () => Notify.openSettings()
