import { expect, it } from 'vitest'
import { renderSize } from '../render-size'

it('renders a 16:9 window at native 4K even on a lower-resolution display', () => {
  expect(renderSize(1280, 720, 1, '4k')).toEqual({ width: 3840, height: 2160 })
  expect(renderSize(1920, 1080, 2, '4k')).toEqual({ width: 3840, height: 2160 })
})
it('preserves window aspect ratio within the 4K pixel bounds', () => {
  expect(renderSize(960, 640, 2, '4k')).toEqual({ width: 3240, height: 2160 })
  expect(renderSize(2560, 1080, 2, '4k')).toEqual({ width: 3840, height: 1620 })
})
it('offers display resolution without exceeding the 4K budget', () => {
  expect(renderSize(960, 640, 1, 'display')).toEqual({ width: 960, height: 640 })
  expect(renderSize(3840, 2160, 2, 'display')).toEqual({ width: 3840, height: 2160 })
})
