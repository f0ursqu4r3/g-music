import { emitTo, listen } from '@tauri-apps/api/event'
import { windowApi } from '@/api'

export const PALETTE_REQUEST_EVENT = 'open-command-palette'
export const PALETTE_ACK_EVENT = 'command-palette-ready'

// This finite native command delegates to the same window specs as the menu.
// See src-tauri/src/windows.rs: show_app_window and show_surface.
export async function showAppWindow(
  view: 'library' | 'queue' | 'settings' | 'import',
): Promise<void> {
  if (view === 'import') return windowApi.showImport()
  return windowApi.showApp(view)
}

export async function handoffCommandPalette(
  sourceLabel: string,
  signal?: AbortSignal,
): Promise<void> {
  const requestId = crypto.randomUUID()
  let acknowledge!: () => void
  let rejectReady!: (reason: Error) => void
  const ready = new Promise<void>((resolve, reject) => {
    acknowledge = resolve
    rejectReady = reject
  })
  // Observe cancellation even if it arrives while a native call is pending.
  void ready.catch(() => {})
  const abort = () => rejectReady(new Error('This window is closed.'))
  signal?.addEventListener('abort', abort, { once: true })
  let unlisten: (() => void) | undefined
  let timer: ReturnType<typeof setTimeout> | undefined
  let resend: ReturnType<typeof setInterval> | undefined
  try {
    if (signal?.aborted) throw new Error('This window is closed.')
    unlisten = await listen<{ requestId: string }>(PALETTE_ACK_EVENT, ({ payload }) => {
      if (payload.requestId === requestId) acknowledge()
    })
    if (signal?.aborted) throw new Error('This window is closed.')
    await showAppWindow('library')
    if (signal?.aborted) throw new Error('This window is closed.')
    const request = () =>
      emitTo('main', PALETTE_REQUEST_EVENT, { requestId, sourceLabel }).catch((cause) =>
        rejectReady(new Error(String(cause))),
      )
    // A newly created Library may still be loading its Vue module and snapshots.
    // Requests carry no selection or playback data and are acknowledged once.
    resend = setInterval(() => {
      void request()
    }, 150)
    timer = setTimeout(
      () => rejectReady(new Error('Library is busy or still loading. Try Command-K in Library.')),
      5000,
    )
    await request()
    await ready
  } finally {
    if (timer) clearTimeout(timer)
    if (resend) clearInterval(resend)
    unlisten?.()
    signal?.removeEventListener('abort', abort)
  }
}
