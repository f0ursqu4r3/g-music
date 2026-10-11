import { describe, expect, it } from 'vitest'
import { themePalette } from '../palette'
import { fallbackPalette } from '../renderer'
describe('visualizer theme colors', () => {
  it('uses a safe palette when canvas color conversion is unavailable', () => {
    expect(themePalette()).toEqual(fallbackPalette)
  })
})
