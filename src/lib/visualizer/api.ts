import { invoke, isTauri } from '@tauri-apps/api/core'

export interface VisualizerFrame {
  positionMs: number
  bands: number[]
}
export interface VisualizerSnapshot {
  generation: number
  status: 'idle' | 'warming' | 'playing' | 'paused' | 'buffering' | 'error'
  error: string | null
  audioPositionMs: number | null
  frames: VisualizerFrame[]
}
export const visualizerApi = {
  available: () => isTauri(),
  setActive: (active: boolean): Promise<void> => invoke('set_visualizer_active', { active }),
  inspect: (): Promise<VisualizerSnapshot> => invoke('inspect_visualizer'),
}
