import { isVisualizerPreset, type VisualizerPresetId } from './presets'
import type { RenderQuality } from './render-size'

export interface VisualizerSettings {
  preset: VisualizerPresetId
  intensity: number
  offsetMs: number
  fps: 30 | 60
  quality: RenderQuality
  /** Show the track's YouTube video under the visualization. */
  video: boolean
  /** Strength of the preset drawn over the video. */
  videoOverlay: number
}
const key = 'gmusic.visualizer.v1'
export function validateSettings(value: unknown, _reducedMotion = false): VisualizerSettings {
  const raw = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
  const finite = (name: string, fallback: number, min: number, max: number) =>
    typeof raw[name] === 'number' && Number.isFinite(raw[name])
      ? Math.min(max, Math.max(min, raw[name]))
      : fallback
  return {
    preset: isVisualizerPreset(raw.preset) ? raw.preset : 'spectrum',
    intensity: finite('intensity', 1, 0.25, 2),
    offsetMs: finite('offsetMs', 0, -1000, 1000),
    fps: raw.fps === 30 ? 30 : 60,
    quality: raw.quality === 'display' ? 'display' : '4k',
    video: raw.video === true,
    videoOverlay: finite('videoOverlay', 0.5, 0, 1),
  }
}
export function loadSettings(reducedMotion: boolean): VisualizerSettings {
  try {
    return validateSettings(JSON.parse(window.localStorage.getItem(key) ?? 'null'), reducedMotion)
  } catch {
    return validateSettings(null, reducedMotion)
  }
}
export function saveSettings(settings: VisualizerSettings): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(validateSettings(settings)))
  } catch {
    /* Storage can be disabled. */
  }
}
