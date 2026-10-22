// SPDX-License-Identifier: AGPL-3.0-or-later
// What the player remembers on the device, as web keeps it in localStorage
// (`tinystream.<key>`): the tracks picked last and the speed.

import { read, write } from '../storage'

type Pref = 'subtitles' | 'subtitleLanguage' | 'audioLanguage' | 'rate' | 'rotation'

export const pref = {
  get: (k: Pref) => read<string>(`player.${k}`),
  set: (k: Pref, v: string) => write(`player.${k}`, v),
}
