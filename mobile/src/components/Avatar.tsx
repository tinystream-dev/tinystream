// SPDX-License-Identifier: AGPL-3.0-or-later

import { type ReactNode, useEffect, useState } from 'react'
import { Text, View } from 'react-native'
import { Img } from './Img'

type Who = { username: string; avatar?: string | null }

/** A stable, quiet hue per name, so pictureless people still look distinct (web's Avatar). */
function hue(name: string) {
  let h = 0
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return h % 360
}

/** `oklch(l c h)` as sRGB, for the same colours web's CSS gets. */
function oklch(l: number, c: number, h: number) {
  const a = c * Math.cos((h * Math.PI) / 180)
  const b = c * Math.sin((h * Math.PI) / 180)
  const L = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const M = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const S = (l - 0.0894841775 * a - 1.291485548 * b) ** 3
  const linear = [
    4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S,
    -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S,
    -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S,
  ]
  const [r, g, bl] = linear.map((x) => {
    const v = x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055
    return Math.round(Math.max(0, Math.min(1, v)) * 255)
  })
  return `rgb(${r}, ${g}, ${bl})`
}

export function Avatar({ user, size = 32, fallback }: { user: Who; size?: number; fallback?: ReactNode }) {
  const [broken, setBroken] = useState(false)
  const url = user.avatar ?? null
  useEffect(() => setBroken(false), [url])
  const h = hue(user.username)
  return (
    <View
      style={{
        width: size,
        height: size,
        borderRadius: size / 2,
        overflow: 'hidden',
        alignItems: 'center',
        justifyContent: 'center',
        experimental_backgroundImage: `linear-gradient(145deg, ${oklch(0.42, 0.06, h)}, ${oklch(0.3, 0.045, (h + 40) % 360)})`,
      }}
    >
      {url && !broken ? (
        <Img src={url} style={{ width: size, height: size }} onError={() => setBroken(true)} />
      ) : (
        (fallback ?? (
          <Text style={{ fontFamily: 'Geist', fontWeight: '600', fontSize: Math.round(size * 0.42), color: oklch(0.9, 0.05, h) }}>
            {user.username.slice(0, 1).toUpperCase()}
          </Text>
        ))
      )}
    </View>
  )
}
