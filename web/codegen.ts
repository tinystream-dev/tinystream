// SPDX-License-Identifier: AGPL-3.0-or-later
// Types for every query in src/ and the shared fragments, from the server's
// schema (packages/shared/schema.graphql, written by
// `TINYSTREAM_WRITE_SCHEMA=1 cargo test schema_file_is_current`).

import type { CodegenConfig } from '@graphql-codegen/cli'

const config: CodegenConfig = {
  schema: '../packages/shared/schema.graphql',
  documents: ['src/**/*.{ts,tsx}', '!src/gql/**/*', '../packages/shared/fragments.graphql'],
  ignoreNoDocuments: true,
  generates: {
    'src/gql/': {
      preset: 'client',
      presetConfig: { fragmentMasking: false },
      config: {
        documentMode: 'string',
        enumsAsTypes: true,
        useTypeImports: true,
        skipTypename: true,
        scalars: { Duration: 'string', JSON: 'unknown', Upload: 'Blob' },
      },
    },
  },
}

export default config
