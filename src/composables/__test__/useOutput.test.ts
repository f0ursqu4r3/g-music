import { describe, expect, it, vi } from 'vitest'

import type { OutputState } from '@/api'
import { useOutput } from '../useOutput'

const local: OutputState = {
  devices: [
    { id: 'coreaudio/Speakers', name: 'MacBook Pro Speakers' },
    { id: 'coreaudio/DAC', name: 'USB DAC' },
  ],
  selectedDeviceId: 'coreaudio/DAC',
  airplayAvailable: true,
  airplayReceivers: null,
}

function client(overrides = {}) {
  return {
    getState: vi.fn().mockResolvedValue(local),
    selectDevice: vi.fn().mockResolvedValue({ ...local, selectedDeviceId: 'coreaudio/Speakers' }),
    openAirPlayPicker: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  }
}

describe('useOutput', () => {
  it('loads state and labels the selected local device', async () => {
    const output = useOutput(client(), null)
    await output.refresh()
    expect(output.state.value.devices).toHaveLength(2)
    expect(output.activeLabel.value).toBe('USB DAC')
    expect(output.isAirPlay.value).toBe(false)
  })

  it('labels the system default when no device is saved', async () => {
    const output = useOutput(
      client({ getState: vi.fn().mockResolvedValue({ ...local, selectedDeviceId: null }) }),
      null,
    )
    await output.refresh()
    expect(output.activeLabel.value).toBe('System default')
  })

  it('labels AirPlay receivers', async () => {
    const airplay = { ...local, airplayReceivers: ['Master Bedroom', 'Family Room'] }
    const output = useOutput(client({ getState: vi.fn().mockResolvedValue(airplay) }), null)
    await output.refresh()
    expect(output.isAirPlay.value).toBe(true)
    expect(output.activeLabel.value).toBe('Master Bedroom, Family Room')
  })

  it('selects a device and applies the returned state', async () => {
    const api = client()
    const output = useOutput(api, null)
    await output.selectDevice('coreaudio/Speakers')
    expect(api.selectDevice).toHaveBeenCalledWith('coreaudio/Speakers')
    expect(output.state.value.selectedDeviceId).toBe('coreaudio/Speakers')
  })

  it('reports command errors without throwing', async () => {
    const api = client({
      selectDevice: vi.fn().mockRejectedValue({
        code: 'output_device_unavailable',
        message: 'The audio output is unavailable.',
      }),
    })
    const output = useOutput(api, null)
    await output.selectDevice('coreaudio/Gone')
    expect(output.errorMessage.value).toBe('The audio output is unavailable.')
  })

  it('opens the picker at the element rectangle', async () => {
    const api = client()
    const output = useOutput(api, null)
    const element = {
      getBoundingClientRect: () => ({ left: 10, top: 20, width: 28, height: 30 }),
    } as unknown as HTMLElement
    await output.openAirPlayPicker(element)
    expect(api.openAirPlayPicker).toHaveBeenCalledWith({ x: 10, y: 20, width: 28, height: 30 })
  })

  it('applies output-changed events', async () => {
    let handler: ((state: OutputState) => void) | undefined
    const subscribe = vi.fn(async (next: (state: OutputState) => void) => {
      handler = next
      return () => {}
    })
    const output = useOutput(client(), subscribe)
    await output.ready
    handler?.({ ...local, airplayReceivers: ['Master Bedroom'] })
    expect(output.isAirPlay.value).toBe(true)
  })
})
