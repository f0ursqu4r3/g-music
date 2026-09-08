import { cursorPosition, getCurrentWindow } from '@tauri-apps/api/window'

/** Track the physical native frame without activating the window. */
export function trackWindowCursor(update: (inside: boolean | null) => void): () => void {
  const current = getCurrentWindow()
  let stopped = false
  let timer: ReturnType<typeof setTimeout> | undefined

  async function sample(): Promise<void> {
    try {
      // All coordinates are physical pixels, including on mixed-DPI displays.
      const [cursor, origin, size, visible, minimized] = await Promise.all([
        cursorPosition(),
        current.outerPosition(),
        current.outerSize(),
        current.isVisible(),
        current.isMinimized(),
      ])
      if (stopped) return
      update(
        visible &&
          !minimized &&
          cursor.x >= origin.x &&
          cursor.y >= origin.y &&
          cursor.x < origin.x + size.width &&
          cursor.y < origin.y + size.height,
      )
      // Schedule after completion so slow IPC cannot overlap or race samples.
      timer = setTimeout(() => {
        void sample()
      }, 200)
    } catch {
      // Browser previews and unavailable native APIs retain DOM hover behavior.
      if (!stopped) update(null)
    }
  }

  void sample()
  return () => {
    stopped = true
    clearTimeout(timer)
  }
}
