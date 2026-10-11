export type RenderQuality = '4k' | 'display'

/** Preserve the window aspect ratio. 16:9 uses exactly 3840 × 2160 in 4K mode. */
export function renderSize(width: number, height: number, dpr: number, quality: RenderQuality) {
  const w = Math.max(1, width),
    h = Math.max(1, height)
  const limit = Math.min(3840 / w, 2160 / h)
  const scale = quality === '4k' ? limit : Math.min(Math.max(1, dpr), limit)
  return { width: Math.max(1, Math.round(w * scale)), height: Math.max(1, Math.round(h * scale)) }
}
