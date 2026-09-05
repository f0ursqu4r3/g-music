import { flushPromises } from '@vue/test-utils'
import { vi } from 'vitest'

export async function flushApp() {
  await vi.dynamicImportSettled()
  await flushPromises()
}
