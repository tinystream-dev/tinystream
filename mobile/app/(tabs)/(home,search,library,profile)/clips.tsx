// SPDX-License-Identifier: AGPL-3.0-or-later
// Clips' place, until they're built (#49).

import { Scissors } from 'lucide-react-native'
import { Page } from '../../../src/components/Page'
import { Soon } from '../../../src/components/Soon'

export default function Clips() {
  return (
    <Page title="Clips">
      <Soon icon={Scissors} title="Clips come soon" body="Making, watching and sharing clips isn't in the app yet. Open Clips in tinystream in a browser for now." />
    </Page>
  )
}
