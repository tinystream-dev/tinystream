// SPDX-License-Identifier: AGPL-3.0-or-later
// Built in #46.

import { Music as MusicIcon } from 'lucide-react-native'
import { Page } from '../../../src/components/Page'
import { Soon } from '../../../src/components/Soon'

export default function Music() {
  return (
    <Page title="Music">
      <Soon icon={MusicIcon} title="Music comes soon" body="Playing your music, with the lock screen and offline albums, is the next part of the app." />
    </Page>
  )
}
