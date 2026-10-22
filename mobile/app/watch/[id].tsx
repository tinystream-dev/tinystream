// SPDX-License-Identifier: AGPL-3.0-or-later
// The video player, full screen: no system bars, landscape (following the
// sensor) unless the rotation's locked. Changing episodes keeps the screen
// and starts a new player in it.

import { useLocalSearchParams } from 'expo-router'
import { useEffect, useState } from 'react'
import { setBrightness, setImmersive, setOrientation } from '../../modules/player'
import { Player, type Rotation } from '../../src/player/Player'

const ORIENTATION = { auto: 'landscape', locked: 'locked', portrait: 'portrait' } as const

export default function Watch() {
  const { id, t } = useLocalSearchParams<{ id: string; t?: string }>()
  const [rotation, setRotation] = useState<Rotation>('auto')

  useEffect(() => {
    void setImmersive(true)
    return () => {
      void setImmersive(false)
      void setOrientation('default')
      void setBrightness(null)
    }
  }, [])
  useEffect(() => void setOrientation(ORIENTATION[rotation]), [rotation])

  return <Player key={id} mediaId={Number(id)} startAt={t ? Number(t) : undefined} rotation={rotation} onRotation={setRotation} />
}
