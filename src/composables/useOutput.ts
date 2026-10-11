import { isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { computed, getCurrentScope, onScopeDispose, ref } from 'vue'

import { outputApi, type OutputState } from '@/api'

export type OutputClient = typeof outputApi
export type OutputSubscriber = (handler: (state: OutputState) => void) => Promise<() => void>

const empty: OutputState = {
  devices: [],
  selectedDeviceId: null,
  airplayAvailable: false,
  airplayReceivers: null,
}

function readErrorMessage(error: unknown): string {
  if (typeof error === 'string' && error.trim()) return error
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message: unknown }).message
    if (typeof message === 'string' && message.trim()) return message
  }
  return 'The audio output command failed.'
}

const defaultSubscriber: OutputSubscriber = (handler) =>
  listen<OutputState>('output-changed', (event) => handler(event.payload))

/**
 * Output state for one window. The backend owns the mode. Each window loads state when its
 * menu opens and follows `output-changed` events. Outside Tauri (tests, previews) it stays empty.
 */
export function useOutput(
  client: OutputClient = outputApi,
  subscribe: OutputSubscriber | null = isTauri() ? defaultSubscriber : null,
) {
  const state = ref<OutputState>({ ...empty })
  const isLoading = ref(false)
  const errorMessage = ref('')

  const isAirPlay = computed(() => state.value.airplayReceivers !== null)
  const activeLabel = computed(() => {
    if (state.value.airplayReceivers) return state.value.airplayReceivers.join(', ')
    const device = state.value.devices.find((item) => item.id === state.value.selectedDeviceId)
    return device?.name ?? 'System default'
  })

  async function run(operation: () => Promise<OutputState | void>): Promise<void> {
    errorMessage.value = ''
    try {
      const next = await operation()
      if (next) state.value = next
    } catch (error) {
      errorMessage.value = readErrorMessage(error)
    }
  }

  async function refresh(): Promise<void> {
    isLoading.value = true
    await run(() => client.getState())
    isLoading.value = false
  }

  function selectDevice(id: string): Promise<void> {
    return run(() => client.selectDevice(id))
  }

  function openAirPlayPicker(element: HTMLElement): Promise<void> {
    const rect = element.getBoundingClientRect()
    return run(() =>
      client.openAirPlayPicker({
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
      }),
    )
  }

  let stop: (() => void) | undefined
  const ready = subscribe
    ? subscribe((next) => {
        state.value = next
      }).then(
        (unlisten) => {
          stop = unlisten
        },
        () => undefined,
      )
    : Promise.resolve()
  if (getCurrentScope()) onScopeDispose(() => stop?.())

  return {
    state,
    isLoading,
    errorMessage,
    isAirPlay,
    activeLabel,
    refresh,
    selectDevice,
    openAirPlayPicker,
    ready,
  }
}
