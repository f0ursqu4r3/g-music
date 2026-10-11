import { describe, expect, it, vi } from 'vitest'
import { VisualizerTimeline } from '../timeline'
import { loadSettings, saveSettings, validateSettings } from '../settings'

const frame = (positionMs: number, value: number) => ({ positionMs, bands: Array(16).fill(value) })
const snapshot = (generation = 1, position = 100, status: 'playing' | 'paused' = 'playing') => ({
  generation,
  status,
  error: null,
  audioPositionMs: position,
  frames: [frame(80, 0.2), frame(100, 0.6), frame(120, 0.9)],
})
describe('visualizer timestamp selection', () => {
  it('anchors at the request midpoint and advances only through fresh observations', () => {
    const timeline = new VisualizerTimeline()
    timeline.observe(snapshot(), 0, 20)
    expect(timeline.sample(10, 0).bands[0]).toBeCloseTo(0.6)
    expect(timeline.sample(30, 0).bands[0]).toBeCloseTo(0.9)
    expect(timeline.sample(1000, 0).bands[0]).toBe(0)
  })
  it('retains delayed frames across polls and positive offsets delay the visual', () => {
    const timeline = new VisualizerTimeline()
    timeline.observe(snapshot(), 0, 0)
    timeline.observe({ ...snapshot(), audioPositionMs: 120, frames: [frame(120, 0.9)] }, 20, 20)
    expect(timeline.sample(20, 40).bands[0]).toBeCloseTo(0.2)
  })
  it('clears old generations and ignores a late prior generation', () => {
    const timeline = new VisualizerTimeline()
    timeline.observe(snapshot(), 0, 0)
    timeline.observe({ ...snapshot(2), frames: [] }, 10, 10)
    timeline.observe(snapshot(1), 20, 20)
    expect(timeline.sample(20, 0).bands[0]).toBe(0)
  })
  it('estimates a playhead for video and stops it when polls go stale or pause', () => {
    const timeline = new VisualizerTimeline()
    expect(timeline.playhead(0)).toBeNull()
    timeline.observe(snapshot(), 0, 20)
    expect(timeline.playhead(60)).toEqual({ positionMs: 150, playing: true })
    expect(timeline.playhead(2000)).toEqual({ positionMs: 100, playing: false })
    timeline.observe(snapshot(1, 400, 'paused'), 2000, 2000)
    expect(timeline.playhead(3000)).toEqual({ positionMs: 400, playing: false })
  })
  it('freezes bands and shader time during pause', () => {
    const timeline = new VisualizerTimeline()
    timeline.observe(snapshot(1, 100, 'paused'), 0, 0)
    expect(timeline.sample(3000, 0)).toEqual(timeline.sample(0, 0))
  })
  it('fades buffering and missing input to neutral', () => {
    const timeline = new VisualizerTimeline()
    timeline.observe(snapshot(), 0, 0)
    timeline.sample(0, 0)
    timeline.observe({ ...snapshot(), status: 'buffering', frames: [] }, 10, 10)
    expect(timeline.sample(1000, 0).bands[0]).toBe(0)
  })
  it('stops extrapolation if the observed playback clock stops', () => {
    const timeline = new VisualizerTimeline()
    timeline.observe(snapshot(), 0, 0)
    timeline.observe(snapshot(), 100, 100)
    expect(timeline.sample(120, 0).timeSeconds).toBe(0.1)
  })
  it('rejects malformed bands and bounds retained frames', () => {
    const timeline = new VisualizerTimeline()
    timeline.observe(
      { ...snapshot(), frames: Array.from({ length: 10000 }, (_, i) => frame(i * 20, 0.5)) },
      0,
      0,
    )
    expect(timeline.frameCount).toBeLessThanOrEqual(256)
    timeline.observe({ ...snapshot(2), frames: [frame(100, NaN)] }, 0, 0)
    expect(timeline.sample(0, 0).bands[0]).toBe(0)
  })
})
describe('visualizer preferences', () => {
  it('round-trips only valid saved preferences', () => {
    const saved = new Map<string, string>()
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => saved.get(key) ?? null,
      setItem: (key: string, value: string) => saved.set(key, value),
    })
    saveSettings({
      preset: 'flow',
      intensity: 20,
      offsetMs: -9000,
      quality: '4k',
      fps: 30,
      video: true,
      videoOverlay: 3,
    })
    expect(loadSettings(false)).toEqual({
      preset: 'flow',
      intensity: 2,
      offsetMs: -1000,
      quality: '4k',
      fps: 30,
      video: true,
      videoOverlay: 1,
    })
    vi.unstubAllGlobals()
  })
  it('validates persisted preferences and honors reduced motion', () => {
    expect(
      validateSettings({ preset: 'invalid', intensity: NaN, offsetMs: 9000, fps: 100 }),
    ).toEqual({
      preset: 'spectrum',
      intensity: 1,
      offsetMs: 1000,
      quality: '4k',
      fps: 60,
      video: false,
      videoOverlay: 0.5,
    })
    expect(validateSettings(null, true).preset).toBe('spectrum')
    expect(
      validateSettings({ preset: 'flow', intensity: 2, offsetMs: -100, quality: '4k', fps: 30 })
        .preset,
    ).toBe('flow')
  })
})
