import { toast } from 'vue-sonner'

/**
 * The palette closes before the AirPlay picker opens, so it cannot show a picker failure.
 * Show the failure as a toast instead. Returns true when an error was shown.
 */
export function reportAirPlayPickerError(message: string): boolean {
  if (!message) return false
  toast.error(message)
  return true
}
