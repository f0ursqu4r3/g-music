import { describe, expect, it } from 'vitest'
import { validateSettings } from '../settings'

describe('visualizer preset preferences', () => {
  it.each([
    'spectrum',
    'flow',
    'aurora',
    'plasma',
    'solar',
    'eclipse',
    'tunnel',
    'starfield',
    'nova',
    'prism',
    'kaleidoscope',
    'mandala',
    'grid',
    'helix',
    'rings',
    'silk',
    'vortex',
    'sparks',
    'terrain',
    'chrome',
  ])('restores the saved %s preset without losing calibration', (preset) => {
    expect(
      validateSettings({ preset, intensity: 1.5, offsetMs: 120, quality: '4k', fps: 30 }),
    ).toEqual({
      preset,
      intensity: 1.5,
      offsetMs: 120,
      quality: '4k',
      fps: 30,
      video: false,
      videoOverlay: 0.5,
    })
  })

  it('keeps the video choice and bounds its overlay strength', () => {
    expect(validateSettings({ video: true, videoOverlay: 0.2 })).toMatchObject({
      video: true,
      videoOverlay: 0.2,
    })
    expect(validateSettings({ video: 'yes', videoOverlay: 4 })).toMatchObject({
      video: false,
      videoOverlay: 1,
    })
  })

  it('falls back safely for a removed or invalid preset', () => {
    for (const preset of ['missing', '__proto__', null, 12]) {
      expect(validateSettings({ preset }).preset).toBe('spectrum')
    }
  })
})
