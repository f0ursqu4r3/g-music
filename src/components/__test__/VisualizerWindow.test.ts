import { mount, flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import VisualizerWindow from '../VisualizerWindow.vue'

const graphics = vi.hoisted(() => ({ dispose: vi.fn() }))
vi.mock('@/lib/visualizer/renderer', () => ({
  fallbackPalette: [
    [0.4, 0.7, 0.8],
    [0.8, 0.5, 0.3],
  ],
  VisualizerRenderer: class {
    draw() {}
    dispose() {
      graphics.dispose()
    }
  },
}))
const api = vi.hoisted(() => ({
  available: vi.fn(() => false),
  setActive: vi.fn(async () => {}),
  inspect: vi.fn(async () => ({
    generation: 1,
    status: 'warming',
    audioPositionMs: null,
    frames: [],
    error: null,
  })),
}))
vi.mock('@/lib/visualizer/api', () => ({ visualizerApi: api }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ isMinimized: async () => false, onResized: async () => () => {} }),
}))
const props = {
  snapshot: {
    status: 'paused' as const,
    currentItem: null,
    positionMs: 0,
    volumePercent: 100,
    queue: [],
  },
}
beforeEach(() => {
  const saved = new Map<string, string>()
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => saved.get(key) ?? null,
    setItem: (key: string, value: string) => saved.set(key, value),
  })
})
afterEach(() => {
  vi.useRealTimers()
  vi.clearAllMocks()
  api.available.mockReturnValue(false)
  vi.unstubAllGlobals()
})
describe('Visualizer window', () => {
  it('switches presets in both directions and keeps 4K calibration preferences', async () => {
    const wrapper = mount(VisualizerWindow, { props })
    await wrapper.get('[aria-label="Next visualizer"]').trigger('click')
    expect(wrapper.get('[aria-label="Browse visualizers"]').text()).toContain('Flow')
    await wrapper.get('[aria-label="Previous visualizer"]').trigger('click')
    expect(wrapper.get('[aria-label="Browse visualizers"]').text()).toContain('Spectrum')
    await wrapper.get('[aria-label="Previous visualizer"]').trigger('click')
    expect(wrapper.get('[aria-label="Browse visualizers"]').text()).toContain('Liquid chrome')
    expect(JSON.parse(localStorage.getItem('gmusic.visualizer.v1')!)).toMatchObject({
      preset: 'chrome',
      quality: '4k',
      offsetMs: 0,
    })
    wrapper.unmount()
  })

  it('selects a visualizer from the gallery and returns to the display', async () => {
    const wrapper = mount(VisualizerWindow, { props, attachTo: document.body })
    try {
      await wrapper.get('[aria-label="Browse visualizers"]').trigger('click')
      await flushPromises()
      const button = document.querySelector<HTMLButtonElement>('[aria-label="Use Aurora"]')
      expect(button).not.toBeNull()
      button!.click()
      await flushPromises()
      expect(wrapper.get('[aria-label="Browse visualizers"]').text()).toContain('Aurora')
      expect(document.querySelector('[role="dialog"]')).toBeNull()
      expect(JSON.parse(localStorage.getItem('gmusic.visualizer.v1')!).preset).toBe('aurora')
    } finally {
      wrapper.unmount()
    }
  })

  it('retries audio analysis after an inspection failure', async () => {
    api.available.mockReturnValue(true)
    api.inspect.mockRejectedValueOnce(new Error('Audio probe failed'))
    const wrapper = mount(VisualizerWindow, { props })
    await flushPromises()
    expect(wrapper.text()).toContain('Audio probe failed')
    await wrapper.get('[aria-label="Retry audio analysis"]').trigger('click')
    await flushPromises()
    expect(api.setActive).toHaveBeenLastCalledWith(true)
    expect(api.inspect).toHaveBeenCalledTimes(2)
    expect(wrapper.text()).toContain('Preparing audio')
    wrapper.unmount()
  })
  it('explains desktop audio and labels the optional demo', async () => {
    const wrapper = mount(VisualizerWindow, { props })
    expect(wrapper.text()).toContain('Live audio requires the desktop app')
    expect(wrapper.find('[aria-label="Start demo"]').exists()).toBe(true)
    await wrapper.get('[aria-label="Start demo"]').trigger('click')
    expect(wrapper.text()).toContain('Demo · simulated audio')
    wrapper.unmount()
  })
  it('emits transport actions through the parent', async () => {
    api.available.mockReturnValue(true)
    const wrapper = mount(VisualizerWindow, { props })
    for (const [label, event] of [
      ['Play', 'togglePlay'],
      ['Previous track', 'previous'],
      ['Next track', 'next'],
    ]) {
      await wrapper.get(`[aria-label="${label}"]`).trigger('click')
      expect(wrapper.emitted(event!)).toEqual([[]])
    }
    wrapper.unmount()
  })
  it('disables native transport in browser preview', async () => {
    const wrapper = mount(VisualizerWindow, { props })
    for (const label of ['Play', 'Previous track', 'Next track']) {
      const button = wrapper.get(`[aria-label="${label}"]`)
      expect(button.attributes()).toHaveProperty('disabled')
      await button.trigger('click')
    }
    expect(wrapper.emitted('togglePlay')).toBeUndefined()
    expect(wrapper.emitted('previous')).toBeUndefined()
    expect(wrapper.emitted('next')).toBeUndefined()
    wrapper.unmount()
  })
  it('releases a native lease after an activation resolves during teardown', async () => {
    api.available.mockReturnValue(true)
    let resolve!: () => void
    api.setActive.mockImplementationOnce(
      () =>
        new Promise<void>((r) => {
          resolve = r
        }),
    )
    const wrapper = mount(VisualizerWindow, { props })
    await flushPromises()
    wrapper.unmount()
    resolve()
    await flushPromises()
    expect(api.setActive).toHaveBeenLastCalledWith(false)
    expect(api.inspect).not.toHaveBeenCalled()
  })
  it('releases the lease when hidden and resumes when visible', async () => {
    api.available.mockReturnValue(true)
    vi.useFakeTimers()
    const wrapper = mount(VisualizerWindow, { props })
    await flushPromises()
    expect(api.setActive).toHaveBeenLastCalledWith(true)
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
    document.dispatchEvent(new Event('visibilitychange'))
    await flushPromises()
    expect(api.setActive).toHaveBeenLastCalledWith(false)
    const inspections = api.inspect.mock.calls.length
    await vi.advanceTimersByTimeAsync(400)
    expect(api.inspect.mock.calls.length).toBe(inspections)
    hidden.mockReturnValue(false)
    document.dispatchEvent(new Event('visibilitychange'))
    await flushPromises()
    expect(api.setActive).toHaveBeenLastCalledWith(true)
    wrapper.unmount()
    hidden.mockRestore()
    await flushPromises()
    expect(vi.getTimerCount()).toBe(0)
  })
  it('releases the audio lease on context loss and restores it after recovery', async () => {
    api.available.mockReturnValue(true)
    const wrapper = mount(VisualizerWindow, { props })
    await flushPromises()
    await wrapper.get('canvas').trigger('webglcontextlost')
    await flushPromises()
    expect(api.setActive).toHaveBeenLastCalledWith(false)
    expect(wrapper.text()).toContain('Waiting for recovery')
    await wrapper.get('canvas').trigger('webglcontextrestored')
    await flushPromises()
    expect(api.setActive).toHaveBeenLastCalledWith(true)
    // The old context already deleted its program. Do not delete its invalid handle after restore.
    expect(graphics.dispose).not.toHaveBeenCalled()
    wrapper.unmount()
    expect(graphics.dispose).toHaveBeenCalledTimes(1)
  })
})
