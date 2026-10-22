// SPDX-License-Identifier: AGPL-3.0-or-later
// The stacks of Home, Search, Library and Profile, which share every screen
// past their first.

import { TabStack } from '../../../src/components/TabStack'

export const unstable_settings = {
  initialRouteName: 'index',
  search: { initialRouteName: 'search' },
  library: { initialRouteName: 'library' },
  profile: { initialRouteName: 'profile' },
}

export default TabStack
