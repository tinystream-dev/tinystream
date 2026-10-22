// SPDX-License-Identifier: AGPL-3.0-or-later
// The background check: every 15 minutes or so (as Android sees fit), each
// server signed in to is asked for its notifications, and unread ones newer
// than the last posted are posted as local notifications. Nothing is pushed:
// the app asks, and only the user's servers are asked.

import * as BackgroundTask from 'expo-background-task'
import * as TaskManager from 'expo-task-manager'
import { AppState, PermissionsAndroid } from 'react-native'
import { postNotices } from '../modules/notify'
import { connect } from './lib/graphql'
import { InboxQuery } from './notifications'
import { servers, tokenOf } from './servers'
import { stored } from './storage'

const TASK = 'tinystream.notifications'

/** Whether to check in the background; on unless turned off. */
export const backgroundChecks = stored('notify.background', true)
/** Whether the permission was asked for after the first sign-in. */
const asked = stored('notify.asked', false)
/** The newest notification posted, per server. */
const lastPosted = stored<Record<string, number>>('notify.last', {})

/** Asks each server for what's new, and posts it (`post` off only notes how far it got). */
export async function check(post = true) {
  const { servers: list, signedIn } = servers()
  const last = { ...lastPosted.get() }
  await Promise.all(
    list
      .filter((s) => signedIn.has(s.id))
      .map(async (s) => {
        try {
          const inbox = (await connect(s.url, tokenOf(s.id)).request(InboxQuery)).notifications
          const newest = Math.max(0, ...inbox.items.map((n) => n.id))
          const since = last[s.id]
          last[s.id] = Math.max(since ?? 0, newest)
          // The first look only notes where things are: what came before isn't news.
          if (since === undefined) return
          const fresh = inbox.items.filter((n) => n.readAt == null && n.id > since).sort((a, b) => a.id - b.id)
          if (post && fresh.length) postNotices(s.id, s.name, fresh.map((n) => ({ id: n.id, title: n.title, body: n.body, link: n.link })))
        } catch (e) {
          console.warn(`background check of ${s.name}:`, e)
        }
      }),
  )
  lastPosted.set(last)
}

TaskManager.defineTask(TASK, async () => {
  try {
    // While the app is open, new ones show as toasts already.
    await check(AppState.currentState !== 'active')
    return BackgroundTask.BackgroundTaskResult.Success
  } catch {
    return BackgroundTask.BackgroundTaskResult.Failed
  }
})

/** Asks for permission to post (Android 13 and later); true when allowed. */
export async function allowNotifications() {
  const r = await PermissionsAndroid.request(PermissionsAndroid.PERMISSIONS.POST_NOTIFICATIONS)
  return r === PermissionsAndroid.RESULTS.GRANTED
}

/** Turns the background check on or off. */
export async function setBackgroundChecks(on: boolean) {
  backgroundChecks.set(on)
  if (on) {
    await allowNotifications()
    // Start from what's there now, so turning it on doesn't post a backlog.
    await check(false).catch(() => {})
    await BackgroundTask.registerTaskAsync(TASK, { minimumInterval: 15 })
  } else if (await TaskManager.isTaskRegisteredAsync(TASK)) {
    await BackgroundTask.unregisterTaskAsync(TASK)
  }
}

/** After a sign-in: the first time, asks for the permission; and keeps the check registered while it's on. */
export async function startBackgroundChecks() {
  if (!backgroundChecks.get()) return
  if (!asked.get()) {
    asked.set(true)
    await allowNotifications()
  }
  if (!(await TaskManager.isTaskRegisteredAsync(TASK))) {
    await check(false).catch(() => {})
    await BackgroundTask.registerTaskAsync(TASK, { minimumInterval: 15 })
  }
}
