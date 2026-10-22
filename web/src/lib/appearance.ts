// SPDX-License-Identifier: AGPL-3.0-or-later

import { useQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import { type Tokens, toRecord } from '@tinystream/shared/theme'
import { graphql } from '../gql'
import type { SchemeFieldsFragment } from '../gql/graphql'
import { request } from './api'
import { apply } from './theme'

export type Scheme = SchemeFieldsFragment

graphql(`
  fragment SchemeFields on ColorScheme {
    id
    name
    builtIn
    published
    editable
    code
    shareCode
    forkedFrom {
      id
      name
    }
    palette {
      seeds {
        name
        value
      }
      overrides {
        name
        value
      }
      tokens {
        name
        value
      }
      warnings {
        foreground
        background
        ratio
        minimum
      }
    }
  }
`)

const AppearanceQuery = graphql(`
  query Appearance {
    appearance {
      mode
      style
      mediaTint
      light {
        id
        palette {
          tokens {
            name
            value
          }
        }
      }
      dark {
        id
        palette {
          tokens {
            name
            value
          }
        }
      }
    }
  }
`)

/**
 * Keeps the page in the theme the server picks for whoever's signed in (or
 * the server's own, signed out). The last one is remembered for next time.
 */
export function useAppearance(viewer: number | null, ready: boolean) {
  const { data } = useQuery({
    queryKey: ['appearance', viewer],
    queryFn: async () => (await request(AppearanceQuery)).appearance,
    enabled: ready,
  })
  useEffect(() => {
    if (!data) return
    apply({
      mode: data.mode,
      light: toRecord(data.light.palette.tokens) as Tokens,
      dark: toRecord(data.dark.palette.tokens) as Tokens,
      style: data.style,
      mediaTint: data.mediaTint,
    })
  }, [data])
}
