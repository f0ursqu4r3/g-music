import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'

import AutoScrollText from '../AutoScrollText.vue'
// @ts-expect-error Vite resolves raw SFC imports during tests.
import autoScrollTextSource from '../AutoScrollText.vue?raw'

describe('AutoScrollText', () => {
  it('duplicates and animates text only when it overflows', async () => {
    const wrapper = mount(AutoScrollText, {
      props: {
        as: 'h1',
        speed: 'slow',
        text: 'An exceptionally long track title that needs room',
      },
    })

    Object.defineProperty(wrapper.element, 'clientWidth', {
      configurable: true,
      value: 120,
    })
    Object.defineProperty(wrapper.get('.auto-scroll-content').element, 'scrollWidth', {
      configurable: true,
      value: 360,
    })
    window.dispatchEvent(new Event('resize'))
    await nextTick()
    await nextTick()

    expect(wrapper.element.tagName).toBe('H1')
    expect(wrapper.attributes('data-overflowing')).toBe('true')
    expect(wrapper.attributes('data-speed')).toBe('slow')
    expect(wrapper.findAll('.auto-scroll-segment')).toHaveLength(2)
    expect(wrapper.findAll('.auto-scroll-segment')[1].attributes('aria-hidden')).toBe('true')
  })

  it('keeps short text static and renders it once', async () => {
    const wrapper = mount(AutoScrollText, {
      props: { text: 'Night Drive' },
    })

    Object.defineProperty(wrapper.element, 'clientWidth', {
      configurable: true,
      value: 240,
    })
    Object.defineProperty(wrapper.get('.auto-scroll-content').element, 'scrollWidth', {
      configurable: true,
      value: 80,
    })
    window.dispatchEvent(new Event('resize'))
    await nextTick()
    await nextTick()

    expect(wrapper.attributes('data-overflowing')).toBe('false')
    expect(wrapper.findAll('.auto-scroll-segment')).toHaveLength(1)
  })

  it('fades only the edge where moving text is clipped', () => {
    expect(autoScrollTextSource).toContain('animation-name: auto-scroll-edge-fade;')
    expect(autoScrollTextSource).toMatch(/0%,\s*14%,\s*86%,\s*100%\s*\{[\s\S]*?black 0/)
    expect(autoScrollTextSource).toMatch(/15%,\s*85%\s*\{[\s\S]*?transparent 0/)
    expect(autoScrollTextSource).toContain(".auto-scroll-text[data-overflowing='true']:hover")
  })
})
