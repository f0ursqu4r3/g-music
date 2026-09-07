import { invoke } from '@tauri-apps/api/core'
import { emitTo, listen } from '@tauri-apps/api/event'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import {
  handoffCommandPalette,
  PALETTE_ACK_EVENT,
  PALETTE_REQUEST_EVENT,
  showAppWindow,
} from '../app-windows'
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ emitTo: vi.fn(), listen: vi.fn() }))
let handler: (event: { payload: { requestId: string } }) => void
const unlisten = vi.fn()
beforeEach(() => {
  vi.useFakeTimers()
  vi.mocked(invoke).mockReset().mockResolvedValue(undefined)
  vi.mocked(emitTo).mockReset().mockResolvedValue(undefined)
  vi.mocked(listen)
    .mockReset()
    .mockImplementation(async (_event, callback) => {
      handler = (event) => callback({ ...event, event: PALETTE_ACK_EVENT, id: 1 })
      return unlisten
    })
  unlisten.mockReset()
})
afterEach(() => vi.useRealTimers())
it('uses the finite native window command and the existing Import command', async () => {
  await showAppWindow('library')
  await showAppWindow('queue')
  await showAppWindow('settings')
  await showAppWindow('import')
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ['show_app_window', { surface: 'library' }],
    ['show_app_window', { surface: 'queue' }],
    ['show_app_window', { surface: 'settings' }],
    ['show_import_window'],
  ])
})
it('subscribes before showing Library and retries a cold window until acknowledgement', async () => {
  const pending = handoffCommandPalette('mini-player')
  await vi.advanceTimersByTimeAsync(0)
  expect(listen).toHaveBeenCalledWith(PALETTE_ACK_EVENT, expect.any(Function))
  expect(vi.mocked(listen).mock.invocationCallOrder[0]).toBeLessThan(
    vi.mocked(invoke).mock.invocationCallOrder[0]!,
  )
  expect(emitTo).toHaveBeenCalledWith('main', PALETTE_REQUEST_EVENT, {
    sourceLabel: 'mini-player',
    requestId: expect.any(String),
  })
  handler({ payload: { requestId: 'wrong' } })
  await vi.advanceTimersByTimeAsync(150)
  expect(emitTo).toHaveBeenCalledTimes(2)
  const requestId = (vi.mocked(emitTo).mock.calls[0]![2] as { requestId: string }).requestId
  handler({ payload: { requestId } })
  await pending
  expect(unlisten).toHaveBeenCalledOnce()
  await vi.advanceTimersByTimeAsync(1000)
  expect(emitTo).toHaveBeenCalledTimes(2)
})
it('cleans up on timeout, native failure and caller disposal', async () => {
  const pending = handoffCommandPalette('queue')
  const assertion = expect(pending).rejects.toThrow('Library is busy')
  await vi.advanceTimersByTimeAsync(5000)
  await assertion
  expect(unlisten).toHaveBeenCalledOnce()
  vi.mocked(invoke).mockRejectedValueOnce(new Error('Cannot show window'))
  await expect(handoffCommandPalette('queue')).rejects.toThrow('Cannot show window')
  expect(unlisten).toHaveBeenCalledTimes(2)
  const controller = new AbortController()
  const cancelled = handoffCommandPalette('queue', controller.signal)
  const cancelledAssertion = expect(cancelled).rejects.toThrow('closed')
  await vi.advanceTimersByTimeAsync(0)
  controller.abort()
  await cancelledAssertion
  expect(unlisten).toHaveBeenCalledTimes(3)
  expect(vi.getTimerCount()).toBe(0)
})
