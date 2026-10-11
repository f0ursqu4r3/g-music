import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import type { OutputState } from '@/api'
import OutputMenu from '../OutputMenu.vue'

const local: OutputState = {
  devices: [
    { id: 'coreaudio/Speakers', name: 'MacBook Pro Speakers' },
    { id: 'coreaudio/DAC', name: 'USB DAC' },
  ],
  selectedDeviceId: 'coreaudio/DAC',
  airplayAvailable: true,
  airplayReceivers: null,
}

function client(state: OutputState = local) {
  return {
    getState: vi.fn().mockResolvedValue(state),
    selectDevice: vi.fn().mockResolvedValue({ ...state, selectedDeviceId: 'coreaudio/Speakers' }),
    openAirPlayPicker: vi.fn().mockResolvedValue(undefined),
  }
}

async function open(wrapper: ReturnType<typeof mount>) {
  // This Reka version opens the dropdown on click, not on pointerdown.
  await wrapper.get('[data-output-menu] button').trigger('click', { button: 0, ctrlKey: false })
  await flushPromises()
}

describe('OutputMenu', () => {
  it('loads devices when opened and marks the selected one', async () => {
    const api = client()
    const wrapper = mount(OutputMenu, {
      props: { client: api, subscribe: null },
      attachTo: document.body,
    })
    await open(wrapper)
    expect(api.getState).toHaveBeenCalled()
    const items = document.querySelectorAll('[data-output-device]')
    expect([...items].map((item) => item.textContent?.trim())).toEqual([
      'MacBook Pro Speakers',
      'USB DAC',
    ])
    expect(
      document.querySelector('[data-output-device="coreaudio/DAC"]')?.getAttribute('aria-checked'),
    ).toBe('true')
    wrapper.unmount()
  })

  it('selects a device', async () => {
    const api = client()
    const wrapper = mount(OutputMenu, {
      props: { client: api, subscribe: null },
      attachTo: document.body,
    })
    await open(wrapper)
    ;(document.querySelector('[data-output-device="coreaudio/Speakers"]') as HTMLElement).click()
    await flushPromises()
    expect(api.selectDevice).toHaveBeenCalledWith('coreaudio/Speakers')
    wrapper.unmount()
  })

  it('shows AirPlay only when available and opens the picker', async () => {
    const api = client()
    const wrapper = mount(OutputMenu, {
      props: { client: api, subscribe: null },
      attachTo: document.body,
    })
    await open(wrapper)
    const airplay = document.querySelector('[data-output-airplay]') as HTMLElement
    expect(airplay.textContent).toContain('AirPlay…')
    airplay.click()
    await flushPromises()
    expect(api.openAirPlayPicker).toHaveBeenCalled()
    wrapper.unmount()
  })

  it('hides AirPlay when unavailable', async () => {
    const api = client({ ...local, airplayAvailable: false })
    const wrapper = mount(OutputMenu, {
      props: { client: api, subscribe: null },
      attachTo: document.body,
    })
    await open(wrapper)
    expect(document.querySelector('[data-output-airplay]')).toBeNull()
    wrapper.unmount()
  })

  it('names the active output in the button label', async () => {
    const api = client({ ...local, airplayReceivers: ['Master Bedroom'] })
    const wrapper = mount(OutputMenu, {
      props: { client: api, subscribe: null },
      attachTo: document.body,
    })
    await open(wrapper)
    expect(wrapper.get('[data-output-menu] button').attributes('aria-label')).toBe(
      'Audio output: Master Bedroom',
    )
    expect(document.querySelector('[data-output-airplay]')?.textContent).toContain('Master Bedroom')
    wrapper.unmount()
  })

  it('loads the active output on mount when a subscriber is active', async () => {
    const api = client({ ...local, airplayReceivers: ['Master Bedroom'] })
    const wrapper = mount(OutputMenu, {
      props: { client: api, subscribe: async () => () => {} },
      attachTo: document.body,
    })
    await flushPromises()
    expect(api.getState).toHaveBeenCalledTimes(1)
    expect(wrapper.get('[data-output-menu] button').attributes('aria-label')).toBe(
      'Audio output: Master Bedroom',
    )
    wrapper.unmount()
  })

  it('stays inert on mount without a subscriber', async () => {
    const api = client()
    const wrapper = mount(OutputMenu, {
      props: { client: api, subscribe: null },
      attachTo: document.body,
    })
    await flushPromises()
    expect(api.getState).not.toHaveBeenCalled()
    wrapper.unmount()
  })
})
