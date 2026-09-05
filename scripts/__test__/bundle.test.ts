// @vitest-environment node
import { build } from 'vite'
import { afterEach, expect, it, vi } from 'vitest'

afterEach(() => vi.unstubAllEnvs())

it('keeps production chunks within budget and loads windows on demand', async () => {
  vi.stubEnv('NODE_ENV', 'production')
  const result = await build({ logLevel: 'silent', build: { write: false } })
  if (Array.isArray(result) || !('output' in result)) {
    throw new Error('Expected one production build output')
  }

  const chunks = result.output.filter((output) => output.type === 'chunk')
  for (const chunk of chunks) {
    expect(Buffer.byteLength(chunk.code), chunk.fileName).toBeLessThanOrEqual(500_000)
  }

  for (const name of [
    'ArtworkWindow',
    'ImportWindow',
    'LibraryWindow',
    'MiniWindow',
    'QueueWindow',
    'SettingsWindow',
  ]) {
    expect(
      chunks.some(
        (chunk) =>
          chunk.isDynamicEntry && chunk.facadeModuleId?.endsWith(`/src/components/${name}.vue`),
      ),
      `${name} should have an on-demand entry`,
    ).toBe(true)
  }
}, 30_000)
