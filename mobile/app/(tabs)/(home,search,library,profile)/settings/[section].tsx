// SPDX-License-Identifier: AGPL-3.0-or-later
// One section of Settings.

import { useLocalSearchParams } from 'expo-router'
import { Scissors } from 'lucide-react-native'
import type { ReactNode } from 'react'
import { Page } from '../../../../src/components/Page'
import { Soon } from '../../../../src/components/Soon'
import { Empty } from '../../../../src/components/ui'
import { MusicSettings } from '../../../../src/settings/Music'
import { Account } from '../../../../src/settings/Account'
import { Appearance } from '../../../../src/settings/Appearance'
import { Credits } from '../../../../src/settings/Credits'
import { Device } from '../../../../src/settings/Device'
import { Automation, Profiles, Renames, Sources, Torrents } from '../../../../src/settings/Downloads'
import { Libraries } from '../../../../src/settings/Libraries'
import { ConfigFile, General, Skipped } from '../../../../src/settings/Server'
import { Users } from '../../../../src/settings/Users'
import { ConfigError } from '../../../../src/settings/kit'
import { type SectionId, useSections } from '../../../../src/settings/sections'

/** Each section's screen: its content, and whatever floats above the tab bar (a Save button). */
const SCREENS: Record<SectionId, () => { body: ReactNode; footer?: ReactNode }> = {
  account: () => ({ body: <Account /> }),
  appearance: () => ({ body: <Appearance /> }),
  libraries: () => ({ body: <Libraries /> }),
  music: () => ({ body: <MusicSettings /> }),
  clips: () => ({ body: <Soon icon={Scissors} title="Clip settings come with clips" body="Set them in tinystream in a browser for now." /> }),
  skipped: () => ({ body: <Skipped /> }),
  downloads: Torrents,
  sources: () => ({ body: <Sources /> }),
  profiles: () => ({ body: <Profiles /> }),
  automation: Automation,
  renames: () => ({ body: <Renames /> }),
  server: General,
  users: () => ({ body: <Users /> }),
  file: ConfigFile,
  device: () => ({ body: <Device /> }),
  credits: () => ({ body: <Credits /> }),
}

export default function Section() {
  const { section } = useLocalSearchParams<{ section: SectionId }>()
  const info = useSections()
    .flatMap((g) => g.sections)
    .find((s) => s.id === section)
  if (!info)
    return (
      <Page title="Settings">
        <Empty title="This isn't something you can change here" />
      </Page>
    )
  return <Screen id={info.id} title={info.label} />
}

/** A component of its own, so each section's hooks stay put. */
function Screen({ id, title }: { id: SectionId; title: string }) {
  const { body, footer } = SCREENS[id]()
  return (
    <Page title={title} footer={footer}>
      <ConfigError />
      {body}
    </Page>
  )
}
