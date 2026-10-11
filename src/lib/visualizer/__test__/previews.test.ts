import { afterEach, expect, it, vi } from 'vitest'

vi.mock('../renderer', () => ({
  fallbackPalette: [
    [0, 0, 0],
    [1, 1, 1],
  ],
  VisualizerRenderer: class {
    draw() {}
    dispose() {}
  },
}))
afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
})

it('does not retain blank thumbnails after graphics loss and retries on the next open', async () => {
  vi.useFakeTimers()
  let lost = true
  const context = { isContextLost: () => lost, getExtension: () => null }
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context as never)
  vi.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockImplementation(() =>
    lost ? 'blank' : 'rendered',
  )
  const { createPresetPreviews } = await import('../previews')
  const first = new Map<string, string>()
  createPresetPreviews((id, url) => first.set(id, url))
  await vi.runAllTimersAsync()
  expect(first.size).toBe(0)
  lost = false
  const recovered = new Map<string, string>()
  createPresetPreviews((id, url) => recovered.set(id, url))
  await vi.runAllTimersAsync()
  expect(recovered.size).toBe(20)
  expect([...recovered.values()].every((value) => value === 'rendered')).toBe(true)
  expect(vi.getTimerCount()).toBe(0)
})
