import { visualizerPresets, type VisualizerPresetId } from './presets'
import { VisualizerRenderer, fallbackPalette } from './renderer'

const cache = new Map<VisualizerPresetId, string>()

/** Render static thumbnails with one short-lived context, not twenty live canvases. */
export function createPresetPreviews(
  ready: (id: VisualizerPresetId, url: string) => void,
): () => void {
  for (const [id, url] of cache) ready(id, url)
  const pending = visualizerPresets.filter(({ id }) => !cache.has(id))
  if (!pending.length) return () => {}
  const canvas = document.createElement('canvas')
  canvas.width = 320
  canvas.height = 200
  let renderer: VisualizerRenderer | undefined
  let timer: ReturnType<typeof setTimeout> | undefined
  let stopped = false
  const stop = () => {
    stopped = true
    clearTimeout(timer)
    renderer?.dispose()
    // This context belongs only to the thumbnail batch.
    canvas.getContext('webgl2')?.getExtension('WEBGL_lose_context')?.loseContext()
    renderer = undefined
  }
  const next = () => {
    if (stopped) return
    const preset = pending.shift()
    if (!preset) return stop()
    try {
      renderer ??= new VisualizerRenderer(canvas, true)
      renderer.draw(
        {
          bands: Float32Array.from({ length: 16 }, (_, i) => 0.38 + 0.2 * Math.sin(i * 0.7)),
          peaks: Float32Array.from({ length: 16 }, (_, i) => 0.5 + 0.2 * Math.sin(i * 0.7)),
          levels: [0.55, 0.45, 0.4, 0.35],
          hits: [0.35, 0, 0],
          beat: 0.35,
          swell: 0.5,
          timeSeconds: 12,
        },
        {
          preset: preset.id,
          intensity: 1,
          offsetMs: 0,
          fps: 30,
          quality: 'display',
          video: false,
          videoOverlay: 0.5,
        },
        fallbackPalette,
        false,
      )
      const context = canvas.getContext('webgl2')
      if (!context || context.isContextLost()) {
        stop()
        return
      }
      const url = canvas.toDataURL('image/webp')
      cache.set(preset.id, url)
      ready(preset.id, url)
    } catch {
      // A missing preview must not prevent preset selection or hide its name.
      stop()
      return
    }
    timer = setTimeout(next, 16)
  }
  timer = setTimeout(next, 0)
  return stop
}
