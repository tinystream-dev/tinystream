// SPDX-License-Identifier: AGPL-3.0-or-later

import { useEffect, useState } from 'react'
import { useStatus } from './hooks'

export {
  airs,
  bytes,
  clockTime,
  countdown,
  duration,
  episodeCode,
  monitorLabels,
  relative,
  shortDate,
  speed,
  startOfDay,
  stateLabels,
} from '@tinystream/shared/downloads'

export type Features = {
  canRequest: boolean
  autoApproved: boolean
  /** Sources that are turned on. */
  sources: number
}

/** What downloads can do in this build for this person; null when the build has none. */
export function useFeatures(): Features | null {
  const { data } = useStatus()
  if (!data?.server.downloads || !data.viewer) return null
  const p = data.viewer.permissions
  return { canRequest: p.request, autoApproved: p.autoApprove, sources: data.server.sources }
}

/** Re-renders every `ms`, for countdowns. */
export function useNow(ms = 1000) {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms)
    return () => clearInterval(t)
  }, [ms])
  return now
}
