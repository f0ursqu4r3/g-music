import { beforeEach, expect, it, vi } from 'vitest'

import { reportAirPlayPickerError } from '../airplay-picker'

const toastError = vi.hoisted(() => vi.fn())
vi.mock('vue-sonner', () => ({ toast: { error: toastError } }))

beforeEach(() => toastError.mockClear())

it('shows a toast and reports true when the picker failed', () => {
  expect(reportAirPlayPickerError('AirPlay is not available.')).toBe(true)
  expect(toastError).toHaveBeenCalledWith('AirPlay is not available.')
})

it('stays silent when the picker opened without an error', () => {
  expect(reportAirPlayPickerError('')).toBe(false)
  expect(toastError).not.toHaveBeenCalled()
})
