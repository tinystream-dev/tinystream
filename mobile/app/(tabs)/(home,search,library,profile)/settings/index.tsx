// SPDX-License-Identifier: AGPL-3.0-or-later
// Settings (web/src/routes/settings.tsx): its sections as a grouped list,
// each a screen of its own.

import { Page } from '../../../../src/components/Page'
import { Divider, Group, ListRow } from '../../../../src/components/ui'
import { useGo } from '../../../../src/nav'
import { ConfigError } from '../../../../src/settings/kit'
import { useSections } from '../../../../src/settings/sections'
import { useTheme } from '../../../../src/theme/ThemeProvider'

export default function Settings() {
  const go = useGo()
  const groups = useSections()
  const { tokens } = useTheme()
  return (
    <Page title="Settings">
      <ConfigError />
      {groups.map((g) => (
        <Group key={g.title} title={g.sections.length > 1 ? g.title : undefined}>
          {g.sections.map((s, i) => (
            <Divided key={s.id} first={i === 0}>
              <ListRow icon={<s.icon size={20} color={tokens['ink-2']} />} label={s.label} onPress={() => go(`settings/${s.id}`)} />
            </Divided>
          ))}
        </Group>
      ))}
    </Page>
  )
}

function Divided({ first, children }: { first: boolean; children: React.ReactNode }) {
  return (
    <>
      {!first && <Divider inset={52} />}
      {children}
    </>
  )
}
