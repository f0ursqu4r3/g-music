import { afterEach, expect, it, vi } from 'vitest'
import { trackWindowCursor } from '../window-cursor'

const native = vi.hoisted(() => ({
  cursorPosition: vi.fn(),
  outerPosition: vi.fn(),
  outerSize: vi.fn(),
  isVisible: vi.fn(),
  isMinimized: vi.fn(),
}))
vi.mock('@tauri-apps/api/window', () => ({
  cursorPosition: native.cursorPosition,
  getCurrentWindow: () => native,
}))
afterEach(() => {
  vi.useRealTimers()
  vi.resetAllMocks()
})

it('tracks physical frame bounds while inactive, moved, and minimized', async () => {
  vi.useFakeTimers()
  native.cursorPosition.mockResolvedValue({ x: -300, y: 110 })
  native.outerPosition.mockResolvedValue({ x: -400, y: 100 })
  native.outerSize.mockResolvedValue({ width: 200, height: 200 })
  native.isVisible.mockResolvedValue(true)
  native.isMinimized.mockResolvedValue(false)
  const update = vi.fn()
  const stop = trackWindowCursor(update)
  await vi.advanceTimersByTimeAsync(0)
  expect(update).toHaveBeenLastCalledWith(true)
  native.outerPosition.mockResolvedValue({ x: 0, y: 0 })
  await vi.advanceTimersByTimeAsync(200)
  expect(update).toHaveBeenLastCalledWith(false)
  native.outerPosition.mockResolvedValue({ x: -400, y: 100 })
  native.isMinimized.mockResolvedValue(true)
  await vi.advanceTimersByTimeAsync(200)
  expect(update).toHaveBeenLastCalledWith(false)
  stop()
  expect(vi.getTimerCount()).toBe(0)
})

it('falls back to DOM hover when native lookup fails without retry flooding', async () => {
  vi.useFakeTimers()
  native.cursorPosition.mockRejectedValue(new Error('unavailable'))
  const update = vi.fn()
  const stop = trackWindowCursor(update)
  await vi.advanceTimersByTimeAsync(0)
  expect(update).toHaveBeenLastCalledWith(null)
  expect(vi.getTimerCount()).toBe(0)
  stop()
})

it('ignores an in-flight sample after disposal', async () => {
  vi.useFakeTimers()
  let resolve!: (value: { x: number; y: number }) => void
  native.cursorPosition.mockReturnValue(
    new Promise((done) => {
      resolve = done
    }),
  )
  native.outerPosition.mockResolvedValue({ x: 0, y: 0 })
  native.outerSize.mockResolvedValue({ width: 200, height: 200 })
  native.isVisible.mockResolvedValue(true)
  native.isMinimized.mockResolvedValue(false)
  const update = vi.fn()
  const stop = trackWindowCursor(update)
  stop()
  resolve({ x: 100, y: 100 })
  await vi.advanceTimersByTimeAsync(1000)
  expect(update).not.toHaveBeenCalled()
  expect(vi.getTimerCount()).toBe(0)
})
