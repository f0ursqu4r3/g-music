import { globalIgnores } from 'eslint/config'
import { defineConfigWithVueTs, vueTsConfigs } from '@vue/eslint-config-typescript'
import pluginVue from 'eslint-plugin-vue'
import pluginVitest from '@vitest/eslint-plugin'
import skipFormatting from 'eslint-config-prettier/flat'
import pluginOxlint from 'eslint-plugin-oxlint'

export default defineConfigWithVueTs(
  {
    name: 'app/files-to-lint',
    files: ['**/*.{vue,ts,mts,tsx}'],
  },

  globalIgnores(['**/dist/**', '**/dist-ssr/**', '**/coverage/**', 'manual-src/data/**']),

  ...pluginVue.configs['flat/essential'],
  vueTsConfigs.recommended,

  {
    name: 'app/rules',
    rules: {
      'vue/multi-word-component-names': 'off',
      // slot="" attribute is needed for web component (rux-*) slot distribution.
      // Vue's v-slot directive does not work with custom elements.
      'vue/no-deprecated-slot-attribute': 'off',
    },
  },

  // SatSimJS engine-boundary rule: only the adapter, composables, perf harness,
  // tools, and ambient declarations may import from `satsim/*`. UI components
  // and other features must consume satsim through `@/shared/services/satsim`.
  {
    name: 'app/satsim-engine-boundary',
    files: ['src/**/*.{ts,vue}'],
    ignores: [
      'src/features/worldView/composables/**',
      'src/features/worldView/perf/**',
      'src/features/worldView/types.ts',
      'src/shared/services/satsim/**',
    ],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['satsim', 'satsim/*'],
              message:
                'Import from @/shared/services/satsim. Direct satsim imports are restricted to composables and the adapter.',
            },
          ],
        },
      ],
    },
  },

  {
    ...pluginVitest.configs.recommended,
    files: ['src/**/*.test.ts'],
  },

  skipFormatting,

  ...pluginOxlint.configs['flat/recommended'],
)
