// SPDX-License-Identifier: AGPL-3.0-or-later
// The schema's own types (enums, inputs) for the shared code, from
// schema.graphql. Each app types its queries and fragments itself.

import type { CodegenConfig } from '@graphql-codegen/cli'

const config: CodegenConfig = {
  schema: 'schema.graphql',
  generates: {
    'src/schema.ts': {
      plugins: ['typescript'],
      config: {
        enumsAsTypes: true,
        useTypeImports: true,
        skipTypename: true,
        scalars: { Duration: 'string', JSON: 'unknown', Upload: 'Blob' },
      },
    },
  },
}

export default config
