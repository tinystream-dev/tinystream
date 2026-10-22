// SPDX-License-Identifier: AGPL-3.0-or-later
// Settings' sections, as web's settings page has them (and who gets which),
// plus the app's own "This device".

import {
  ArrowDownToLine,
  CircleUser,
  FileCode,
  FileX,
  FolderPen,
  Heart,
  Library,
  type LucideIcon,
  Music,
  Palette,
  Rss,
  Scissors,
  Server,
  SlidersHorizontal,
  Smartphone,
  Users,
  Zap,
} from 'lucide-react-native'
import { useMe, useSettings } from '../queries'

export type SectionId =
  | 'account'
  | 'appearance'
  | 'libraries'
  | 'music'
  | 'clips'
  | 'skipped'
  | 'downloads'
  | 'sources'
  | 'profiles'
  | 'automation'
  | 'renames'
  | 'server'
  | 'users'
  | 'file'
  | 'device'
  | 'credits'

export type SectionInfo = { id: SectionId; label: string; icon: LucideIcon }
export type SectionGroup = { title: string; sections: SectionInfo[] }

/** The sections this person gets, grouped as web groups them. */
export function useSections(): SectionGroup[] {
  const me = useMe()
  const { data: settings } = useSettings()
  const admin = !!me?.isAdmin
  const downloads = admin && !!settings?.downloads
  const groups: (SectionGroup | false)[] = [
    {
      title: 'You',
      sections: [
        { id: 'account', label: 'Account', icon: CircleUser },
        { id: 'appearance', label: 'Appearance', icon: Palette },
      ],
    },
    admin && {
      title: 'Library',
      sections: [
        { id: 'libraries', label: 'Libraries', icon: Library },
        { id: 'music', label: 'Music', icon: Music },
        { id: 'clips', label: 'Clips', icon: Scissors },
        { id: 'skipped', label: 'Skipped files', icon: FileX },
      ],
    },
    downloads && {
      title: 'Downloads',
      sections: [
        { id: 'downloads', label: 'Torrents', icon: ArrowDownToLine },
        { id: 'sources', label: 'Sources', icon: Rss },
        { id: 'profiles', label: 'Quality profiles', icon: SlidersHorizontal },
        { id: 'automation', label: 'Automation', icon: Zap },
        { id: 'renames', label: 'Renames', icon: FolderPen },
      ],
    },
    admin && {
      title: 'Server',
      sections: [
        { id: 'server', label: 'General', icon: Server },
        { id: 'users', label: 'Users', icon: Users },
        { id: 'file', label: 'Config file', icon: FileCode },
      ],
    },
    { title: 'This device', sections: [{ id: 'device', label: 'This device', icon: Smartphone }] },
    { title: 'About', sections: [{ id: 'credits', label: 'Credits', icon: Heart }] },
  ]
  return groups.filter((g): g is SectionGroup => !!g)
}
