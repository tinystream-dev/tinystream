// SPDX-License-Identifier: AGPL-3.0-or-later
// Every picture from a server: images, artwork and covers need the token,
// so they're fetched with it, and kept on disk.

import { Image, type ImageProps } from 'expo-image'
import { resolve } from '../lib/address'
import { authHeaders } from '../lib/graphql'
import { useConnection } from '../session'

export type ImgProps = Omit<ImageProps, 'source'> & {
  /** A path on the server (`/api/images/…`) or a full URL. */
  src: string | null | undefined
}

export function Img({ src, transition = 200, ...rest }: ImgProps) {
  const connection = useConnection()
  const source =
    src && connection ? { uri: resolve(connection.origin, src), headers: authHeaders(connection.token) } : null
  return <Image source={source} cachePolicy="disk" transition={transition} {...rest} />
}
