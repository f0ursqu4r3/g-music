import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'

import type { PlaybackSnapshot } from '@/api'
import { Slider } from '@/components/ui/slider'

import AutoScrollText from '../AutoScrollText.vue'
import ArtworkWindow from '../ArtworkWindow.vue'
import YouTubeArtwork from '../YouTubeArtwork.vue'
import { dragSlider } from './slider-interaction'

enableAutoUnmount(afterEach)
afterEach(() => {
  vi.useRealTimers()
})

const snapshot: PlaybackSnapshot = {
  status: 'paused',
  currentItem: {
    id: 'M7lc1UVf-VE',
    title: 'Night Drive',
    artist: 'Chromatic Skies',
    durationMs: 238_000,
  },
  positionMs: 57_000,
  volumePercent: 72,
  queue: [],
}

describe('ArtworkWindow', () => {
  it('uses a slower exit than entrance for the overlay and playback controls', () => {
    const wrapper = mount(ArtworkWindow, { props: { snapshot, isUpdating: false } })
    for (const selector of ['.artwork-controls', '.artwork-playback-controls']) {
      const classes = wrapper.get(selector).classes()
      expect(classes).toContain('transition-[opacity,translate]')
      expect(classes).toContain('motion-reduce:transition-none')
    }
    expect(wrapper.get('.artwork-controls').classes()).toContain(
      'duration-(--artwork-transition-durations)',
    )
    expect(wrapper.get('.artwork-playback-controls').classes()).toContain('duration-300')
    expect(wrapper.get('.artwork-playback-controls').classes()).toContain(
      'data-[visible=false]:duration-1200',
    )
  })

  it('gives artwork actions restrained hover and press motion', () => {
    const wrapper = mount(ArtworkWindow, { props: { snapshot, isUpdating: false } })
    for (const button of wrapper.findAll('.artwork-playback-controls button')) {
      expect(button.classes()).toContain('motion-safe:active:scale-95')
      expect(button.classes()).toContain('motion-reduce:transition-none')
    }
  })

  it.each([
    [false, false, false],
    [false, true, false],
    [true, false, true],
    [true, true, true],
  ])(
    'uses active=%s independently of hover=%s for playback visibility=%s',
    async (active, hovered, visible) => {
      const wrapper = mount(ArtworkWindow, {
        props: {
          snapshot,
          isUpdating: false,
          isWindowFocused: active,
          isCursorWithinWindow: hovered,
        },
      })
      const controls = wrapper.get('.artwork-playback-controls')
      expect(controls.attributes('data-visible')).toBe(String(visible))
      expect(controls.attributes('aria-hidden')).toBe(visible ? undefined : 'true')
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }))
      expect(controls.attributes('inert')).toBe(visible ? undefined : '')
      await nextTick()
      expect(controls.attributes('data-visible')).toBe(String(visible))
    },
  )

  it('slides the full control height below the frame while inactive', async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false, isWindowFocused: false, isCursorWithinWindow: true },
    })
    const overlay = wrapper.get('.artwork-controls')
    expect(overlay.attributes('data-playback-visible')).toBe('false')
    expect(overlay.classes()).toContain('data-[playback-visible=false]:translate-y-34')
    expect(wrapper.get('.artwork-playback-controls').classes()).toContain('h-34')
    await wrapper.setProps({ isWindowFocused: true, isCursorWithinWindow: false })
    expect(overlay.attributes('data-visible')).toBe('true')
    expect(overlay.attributes('data-playback-visible')).toBe('true')
  })

  it.each([true, false])(
    'keeps active overlays visible and fades inactive overlays on cursor exit: %s',
    async (isWindowFocused) => {
      const wrapper = mount(ArtworkWindow, {
        props: { snapshot, isUpdating: false, isWindowFocused },
      })
      const overlay = wrapper.get('.artwork-controls')
      expect(overlay.attributes('data-visible')).toBe(String(isWindowFocused))
      await wrapper.get('main').trigger('pointerenter')
      expect(overlay.attributes('data-visible')).toBe('true')
      await wrapper.get('main').trigger('pointerleave')
      expect(overlay.attributes('data-visible')).toBe(String(isWindowFocused))
      expect(overlay.classes()).toContain('data-[visible=false]:opacity-0')
      expect(wrapper.get('.artwork-metadata').attributes('aria-hidden')).toBe(
        isWindowFocused ? undefined : 'true',
      )
    },
  )

  it('keeps an inactive overlay visible under a stationary cursor', async () => {
    const wrapper = playing()
    await wrapper.setProps({ isWindowFocused: false })
    await wrapper.get('main').trigger('pointerenter')
    await vi.advanceTimersByTimeAsync(10000)
    expect(wrapper.get('.artwork-controls').attributes('data-visible')).toBe('true')
  })

  it('uses native cursor presence without requiring WebView pointer events', async () => {
    const wrapper = playing()
    await wrapper.setProps({ isWindowFocused: false, isCursorWithinWindow: true })
    expect(wrapper.get('.artwork-controls').attributes('data-visible')).toBe('true')
    await wrapper.setProps({ isCursorWithinWindow: false })
    expect(wrapper.get('.artwork-controls').attributes('data-visible')).toBe('false')
  })

  it('fills the frame with artwork behind bottom-overlay controls', () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false },
    })

    expect(wrapper.get('main').classes()).toEqual(
      expect.arrayContaining(['relative', 'h-screen', 'overflow-hidden']),
    )
    expect(wrapper.get('.artwork-cover').classes()).toEqual(
      expect.arrayContaining(['absolute', 'inset-0']),
    )
    expect(wrapper.getComponent(YouTubeArtwork).props('videoId')).toBe('M7lc1UVf-VE')
    expect(wrapper.get('.artwork-drag-region').attributes('data-tauri-drag-region')).toBe('')
    expect(wrapper.get('main').classes()).toEqual(expect.arrayContaining(['flex', 'flex-col']))
    expect(wrapper.get('.artwork-drag-region').classes()).toEqual(
      expect.arrayContaining(['absolute', 'inset-0']),
    )

    const overlay = wrapper.get('.artwork-controls')
    expect(overlay.classes()).toEqual(
      expect.arrayContaining(['artwork-information-gradient', 'relative', 'shrink-0']),
    )
    expect(wrapper.find('.artwork-scrim').exists()).toBe(false)
    expect(wrapper.findAll('input[type="range"]')).toHaveLength(0)
    const progress = wrapper.getComponent(Slider)
    expect(progress.attributes('aria-label')).toBe('Track progress')
    expect(progress.props('disabled')).toBe(false)
    expect(wrapper.text()).not.toContain('G MUSIC')
  })

  it('emits transport actions from the overlaid controls', async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false },
    })

    await wrapper.get('[aria-label="Previous track"]').trigger('click')
    await wrapper.get('[aria-label="Play"]').trigger('click')
    await wrapper.get('[aria-label="Next track"]').trigger('click')

    expect(wrapper.emitted('previous')).toHaveLength(1)
    expect(wrapper.emitted('toggle')).toHaveLength(1)
    expect(wrapper.emitted('next')).toHaveLength(1)
  })

  it('exposes current-track favorite actions from the artwork surface', async () => {
    const wrapper = mount(ArtworkWindow, {
      attachTo: document.body,
      props: { snapshot, isUpdating: false },
    })

    await wrapper.get('[aria-label="Favorite track"]').trigger('click')
    expect(wrapper.emitted('toggleFavorite')).toEqual([['M7lc1UVf-VE']])

    await wrapper.get('.artwork-drag-region').trigger('contextmenu')
    expect(document.body.querySelector('[data-artwork-context-menu]')?.textContent).toContain(
      'Add to Favorites',
    )
    wrapper.unmount()
  })

  it('commits pointer drags from the progress scrubber', async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false },
    })
    const progress = wrapper.get('[data-slot="slider"][aria-label="Track progress"]')

    await dragSlider(progress, 50)

    expect(wrapper.emitted('seek')).toEqual([[119_000]])
  })

  it('slides controls out of view when the native window loses focus', async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false, isWindowFocused: true },
    })
    const metadata = wrapper.get('.artwork-metadata')
    const controls = wrapper.get('.artwork-playback-controls')

    expect(controls.attributes('data-window-focused')).toBe('true')

    await wrapper.setProps({ isWindowFocused: false })

    expect(controls.attributes('data-window-focused')).toBe('false')
    expect(controls.attributes('aria-hidden')).toBe('true')
    expect(controls.attributes()).toHaveProperty('inert')
    expect(metadata.text()).toContain('Night Drive')
    expect(metadata.text()).toContain('Chromatic Skies')
    expect(wrapper.findAllComponents(AutoScrollText)).toHaveLength(2)
    expect(wrapper.findAllComponents(AutoScrollText)[0].props()).toMatchObject({
      as: 'h1',
      speed: 'slow',
      text: 'Night Drive',
    })
    expect(wrapper.findAllComponents(AutoScrollText)[1].props()).toMatchObject({
      as: 'p',
      speed: 'medium',
      text: 'Chromatic Skies',
    })
    expect(metadata.attributes('aria-hidden')).toBe('true')
    expect(metadata.attributes()).not.toHaveProperty('inert')
    expect(controls.classes()).toEqual(
      expect.arrayContaining([
        'transition-[opacity,translate]',
        'data-[visible=false]:translate-y-2',
        'data-[visible=false]:opacity-0',
        'motion-reduce:transition-none',
      ]),
    )
    expect(controls.classes()).not.toContain('h-0.5')
  })

  function playing() {
    vi.useFakeTimers()
    return mount(ArtworkWindow, {
      attachTo: document.body,
      props: { snapshot: { ...snapshot, status: 'playing' }, isUpdating: false },
    })
  }

  it('keeps active controls visible after idle time and cursor exit', async () => {
    const wrapper = playing()
    const controls = wrapper.get('.artwork-playback-controls')
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }))
    await vi.advanceTimersByTimeAsync(2999)
    expect(controls.attributes('inert')).toBeUndefined()
    await vi.advanceTimersByTimeAsync(1)
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    expect(controls.attributes()).not.toHaveProperty('inert')
    expect(wrapper.get('.artwork-metadata').text()).toContain('Night Drive')
    expect(wrapper.get('progress').attributes('aria-hidden')).toBe('true')
    expect(wrapper.get('progress').attributes('data-visible')).toBe('false')
    await wrapper.get('main').trigger('pointerenter')
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await vi.advanceTimersByTimeAsync(3000)
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.get('main').trigger('pointerleave')
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.get('main').trigger('pointermove')
    expect(controls.attributes('aria-hidden')).toBeUndefined()
  })

  it('preserves shortcuts and hides focused controls only on native blur', async () => {
    const wrapper = playing()
    const controls = wrapper.get('.artwork-playback-controls')
    await vi.advanceTimersByTimeAsync(3000)
    const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true })
    await wrapper.get('main').trigger('pointerenter')
    window.dispatchEvent(tab)
    expect(controls.attributes('inert')).toBeUndefined()
    expect(tab.defaultPrevented).toBe(false)
    await nextTick()
    const button = wrapper.get<HTMLButtonElement>('[aria-label="Pause"]')
    button.element.focus()
    await vi.advanceTimersByTimeAsync(4000)
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.get('main').trigger('pointerleave')
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.setProps({ isWindowFocused: false })
    expect(controls.attributes('aria-hidden')).toBe('true')
    button.element.blur()
    await vi.advanceTimersByTimeAsync(3000)
    expect(controls.attributes('aria-hidden')).toBe('true')
    const key = new KeyboardEvent('keydown', { key: 'm', cancelable: true })
    window.dispatchEvent(key)
    await nextTick()
    expect(key.defaultPrevented).toBe(false)
    expect(controls.attributes('aria-hidden')).toBe('true')
  })

  it('keeps controls during a pointer press outside the frame until native blur', async () => {
    const wrapper = playing()
    const controls = wrapper.get('.artwork-playback-controls')
    await controls.trigger('pointerenter')
    await wrapper.get('main').trigger('pointerenter')
    await vi.advanceTimersByTimeAsync(4000)
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.get('[aria-label="Pause"]').trigger('pointerdown', { pointerId: 1 })
    await controls.trigger('pointerleave')
    await wrapper.get('main').trigger('pointerleave')
    await vi.advanceTimersByTimeAsync(4000)
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 1 }))
    await vi.advanceTimersByTimeAsync(3000)
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.setProps({ isWindowFocused: false })
    expect(controls.attributes('aria-hidden')).toBe('true')
  })

  it('respects native blur, active hover, and focused paused, starting and empty states', async () => {
    const wrapper = playing()
    const controls = wrapper.get('.artwork-playback-controls')
    await wrapper.get('main').trigger('pointerenter')
    await wrapper.setProps({ isWindowFocused: false })
    expect(controls.attributes('aria-hidden')).toBe('true')
    await wrapper.get('main').trigger('pointerleave')
    expect(controls.attributes('aria-hidden')).toBe('true')
    await wrapper.setProps({ isWindowFocused: true, snapshot })
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.get('main').trigger('pointerenter')
    await vi.advanceTimersByTimeAsync(4000)
    expect(wrapper.get('[data-playback-state]').text()).toBe('Paused')
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.setProps({ isStarting: true })
    await vi.advanceTimersByTimeAsync(4000)
    expect(wrapper.get('[data-playback-state]').text()).toBe('Starting playback')
    await wrapper.setProps({ isStarting: false, snapshot: { ...snapshot, currentItem: null } })
    await vi.advanceTimersByTimeAsync(4000)
    expect(wrapper.text()).toContain('Choose a track to begin')
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    expect(wrapper.get('[aria-label="Play"]').attributes('disabled')).toBeDefined()
    expect(wrapper.getComponent(Slider).props('disabled')).toBe(true)
  })

  it('keeps menu actions reachable without revealing inactive playback controls', async () => {
    const wrapper = playing()
    const surface = wrapper.get('.artwork-drag-region')
    expect(surface.attributes('data-slot')).toBe('context-menu-trigger')
    await surface.trigger('contextmenu')
    await wrapper.get('main').trigger('pointerleave')
    await wrapper.setProps({ isWindowFocused: false })
    await vi.advanceTimersByTimeAsync(4000)
    expect(wrapper.get('.artwork-playback-controls').attributes('aria-hidden')).toBe('true')
    const menu = document.querySelector('[data-artwork-context-menu]')!
    for (const label of ['Open queue', 'Open library']) {
      expect(menu.textContent).toContain(label)
      expect(wrapper.get(`[aria-label="${label}"]`).attributes('title')).toBe(label)
    }
    const queue = [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (item) => item.textContent?.trim() === 'Open queue',
    )!
    queue.click()
    await nextTick()
    expect(wrapper.emitted('openQueue')).toHaveLength(1)
    await vi.advanceTimersByTimeAsync(3000)
    expect(wrapper.get('.artwork-playback-controls').attributes('aria-hidden')).toBe('true')
  })

  it('routes volume, mute and visible window actions without changing the snapshot', async () => {
    const wrapper = mount(ArtworkWindow, { props: { snapshot, isUpdating: false } })
    await dragSlider(wrapper.get('[data-slot="slider"][aria-label="Volume"]'), 25)
    expect(wrapper.emitted('setVolume')).toEqual([[25]])
    expect(snapshot.volumePercent).toBe(72)
    await wrapper.get('[aria-label="Mute volume"]').trigger('click')
    expect(wrapper.emitted('toggleMute')).toHaveLength(1)
    await wrapper.setProps({ snapshot: { ...snapshot, volumePercent: 0 } })
    expect(wrapper.get('[aria-label="Unmute volume"]').attributes('title')).toBe('Unmute volume')
    await wrapper.get('[aria-label="Open queue"]').trigger('click')
    await wrapper.get('[aria-label="Open library"]').trigger('click')
    expect(wrapper.emitted('openQueue')).toHaveLength(1)
    expect(wrapper.emitted('openLibrary')).toHaveLength(1)
    await wrapper.setProps({ isUpdating: true })
    expect(wrapper.get('[aria-label="Unmute volume"]').attributes('disabled')).toBeDefined()
    expect(
      wrapper.get('[data-slot="slider"][aria-label="Volume"]').attributes('data-disabled'),
    ).toBeDefined()
  })

  async function beginSeek(wrapper: ReturnType<typeof playing>) {
    const slider = wrapper.get('[data-slot="slider"][aria-label="Track progress"]')
    // Install the same geometry and pointer capture used by the shared drag helper.
    await dragSlider({ element: slider.element, trigger: async () => {} }, 50)
    await slider.trigger('pointerdown', { clientX: 50, pointerId: 1 })
    return slider
  }

  it('previews a rendered seek drag, ignores snapshots during drag, and commits once', async () => {
    const wrapper = playing()
    const slider = await beginSeek(wrapper)
    expect(wrapper.get('[data-seek-preview]').text()).toBe('1:59')
    expect(wrapper.emitted('seek')).toBeUndefined()
    await slider.trigger('pointermove', { clientX: 60, pointerId: 1 })
    expect(wrapper.get('[data-seek-preview]').text()).toBe('2:23')
    await slider.trigger('pointermove', { clientX: 50, pointerId: 1 })
    await wrapper.setProps({ snapshot: { ...snapshot, status: 'playing', positionMs: 60_000 } })
    await wrapper.get('main').trigger('pointerleave')
    await vi.advanceTimersByTimeAsync(4000)
    expect(wrapper.get('.artwork-playback-controls').attributes('aria-hidden')).toBeUndefined()
    expect(wrapper.get('[data-seek-preview]').text()).toBe('1:59')
    await slider.trigger('pointerup', { clientX: 50, pointerId: 1 })
    await slider.trigger('pointerup', { clientX: 50, pointerId: 1 })
    expect(wrapper.emitted('seek')).toEqual([[119_000]])
    expect(wrapper.find('[data-seek-preview]').exists()).toBe(false)
  })

  it.each(['cancel', 'blur', 'native blur', 'track change', 'Escape'])(
    'resets seek preview on %s and rejects a late commit',
    async (reason) => {
      const wrapper = playing()
      const slider = await beginSeek(wrapper)
      if (reason === 'cancel') await slider.trigger('pointercancel', { pointerId: 1 })
      if (reason === 'blur') window.dispatchEvent(new Event('blur'))
      if (reason === 'native blur') await wrapper.setProps({ isWindowFocused: false })
      if (reason === 'track change')
        await wrapper.setProps({
          snapshot: {
            ...snapshot,
            currentItem: { ...snapshot.currentItem!, id: 'next' },
          },
        })
      if (reason === 'Escape') window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
      await nextTick()
      expect(wrapper.find('[data-seek-preview]').exists()).toBe(false)
      await slider.trigger('pointerup', { clientX: 50, pointerId: 1 })
      expect(wrapper.emitted('seek')).toBeUndefined()
      expect(wrapper.getComponent(Slider).props('modelValue')).toEqual([57000])
    },
  )

  it('supports keyboard seek and clamps finite values at the component boundary', async () => {
    const wrapper = playing()
    await wrapper
      .get('[role="slider"][aria-label="Track progress"]')
      .trigger('keydown', { key: 'ArrowRight' })
    expect(wrapper.emitted('seek')).toEqual([[58000]])
    expect(wrapper.find('[data-seek-preview]').exists()).toBe(false)
    const slider = wrapper.getComponent(Slider)
    slider.vm.$emit('update:modelValue', [999999])
    slider.vm.$emit('valueCommit', [999999])
    slider.vm.$emit('valueCommit', [999999])
    slider.vm.$emit('update:modelValue', [NaN])
    slider.vm.$emit('valueCommit', [Infinity])
    slider.vm.$emit('update:modelValue', [-1000])
    slider.vm.$emit('valueCommit', [-1000])
    expect(wrapper.emitted('seek')).toEqual([[58000], [238000], [0]])
  })

  it.each([0, -1, NaN, Infinity])(
    'disables seeking for invalid duration %s',
    async (durationMs) => {
      const wrapper = mount(ArtworkWindow, {
        props: {
          snapshot: {
            ...snapshot,
            currentItem: { ...snapshot.currentItem!, durationMs },
            positionMs: NaN,
          },
          isUpdating: false,
        },
      })
      expect(wrapper.getComponent(Slider).props('disabled')).toBe(true)
      await dragSlider(wrapper.get('[data-slot="slider"][aria-label="Track progress"]'), 50)
      expect(wrapper.emitted('seek')).toBeUndefined()
      expect(wrapper.text()).not.toMatch(/NaN|Infinity/)
    },
  )

  it('places full-width progress above auto-width hour labels and marks active modes', () => {
    const wrapper = mount(ArtworkWindow, {
      props: {
        snapshot: {
          ...snapshot,
          positionMs: 360_001_000,
          shuffleEnabled: true,
          repeatMode: 'one',
          currentItem: { ...snapshot.currentItem!, durationMs: 720_002_000 },
        },
        isUpdating: false,
      },
    })
    const times = wrapper.get('[data-progress-times]')
    expect(times.text()).toContain('100:00:01')
    expect(times.text()).toContain('200:00:02')
    expect(times.classes()).toContain('justify-between')
    expect(times.element.previousElementSibling?.getAttribute('data-slot')).toBe('slider')
    expect(wrapper.get('[aria-label="Disable shuffle"]').classes()).toContain(
      'aria-pressed:text-accent',
    )
    expect(wrapper.get('[aria-label="Disable repeat"]').classes()).toContain(
      'aria-pressed:text-accent',
    )
  })

  it('cleans up input listeners without leaving timers on unmount', async () => {
    const remove = vi.spyOn(window, 'removeEventListener')
    const wrapper = playing()
    wrapper.unmount()
    await flushPromises()
    for (const event of ['keydown', 'pointercancel', 'blur']) {
      expect(remove.mock.calls.some(([name]) => name === event)).toBe(true)
    }
    expect(vi.getTimerCount()).toBe(0)
    remove.mockRestore()
  })

  it('keeps hidden controls inert if a key arrives before native focus returns', async () => {
    const wrapper = playing()
    await wrapper.setProps({ isWindowFocused: false })
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }))
    await nextTick()
    const controls = wrapper.get('.artwork-playback-controls')
    expect(controls.attributes('aria-hidden')).toBe('true')
    expect(controls.attributes()).toHaveProperty('inert')
    await wrapper.setProps({ isWindowFocused: true })
    expect(controls.attributes('aria-hidden')).toBeUndefined()
    await wrapper.get('main').trigger('pointerenter')
    expect(controls.attributes('aria-hidden')).toBeUndefined()
  })

  it('keeps active controls visible when a track change replaces the seek thumb', async () => {
    const wrapper = playing()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }))
    wrapper.get<HTMLElement>('[role="slider"][aria-label="Track progress"]').element.focus()
    await nextTick()
    await wrapper.setProps({
      snapshot: {
        ...snapshot,
        status: 'playing',
        currentItem: { ...snapshot.currentItem!, id: 'next' },
      },
    })
    await vi.advanceTimersByTimeAsync(3000)
    expect(wrapper.get('.artwork-playback-controls').attributes('aria-hidden')).toBeUndefined()
  })

  it('exposes a formatted seek value on the keyboard slider', async () => {
    const wrapper = playing()
    const thumb = wrapper.get('[role="slider"][aria-label="Track progress"]')
    expect(thumb.attributes('aria-valuetext')).toBe('0:57')
    await wrapper.setProps({ snapshot: { ...snapshot, positionMs: 62000 } })
    expect(thumb.attributes('aria-valuetext')).toBe('1:02')
  })

  it('does not hold playback controls open for native artwork drags', async () => {
    const wrapper = playing()
    await wrapper.get('.artwork-drag-region').trigger('pointerdown', {
      pointerId: 1,
      pointerType: 'mouse',
      button: 0,
    })
    await wrapper.get('main').trigger('pointerleave')
    // Native dragging can finish outside the WebView without a DOM pointerup.
    await wrapper.setProps({ isWindowFocused: false })
    await vi.advanceTimersByTimeAsync(3000)
    expect(wrapper.get('.artwork-playback-controls').attributes('aria-hidden')).toBe('true')
  })

  it('releases a cancelled pointer interaction on native blur', async () => {
    const wrapper = playing()
    await beginSeek(wrapper)
    await wrapper.get('main').trigger('pointerleave')
    await wrapper.setProps({ isWindowFocused: false })
    expect(wrapper.get('.artwork-playback-controls').attributes('aria-hidden')).toBe('true')
  })

  it('preserves keyboard seek focus across pending playback updates', async () => {
    const wrapper = playing()
    await wrapper.get('main').trigger('pointerenter')
    await flushPromises()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }))
    const thumb = wrapper.get<HTMLElement>('[role="slider"][aria-label="Track progress"]')
    thumb.element.focus()
    await nextTick()
    await thumb.trigger('keydown', { key: 'ArrowRight' })
    await wrapper.setProps({ isUpdating: true })
    await wrapper.setProps({
      isUpdating: false,
      snapshot: { ...snapshot, status: 'playing', positionMs: 58000 },
    })
    expect(document.activeElement).toBe(thumb.element)
    await thumb.trigger('keydown', { key: 'ArrowRight' })
    expect(wrapper.emitted('seek')).toEqual([[58000], [59000]])
    await vi.advanceTimersByTimeAsync(4000)
    expect(wrapper.get('.artwork-playback-controls').attributes('aria-hidden')).toBeUndefined()
  })

  it('shows track progress along the panel bottom while controls are hidden', async () => {
    const wrapper = mount(ArtworkWindow, {
      props: { snapshot, isUpdating: false, isWindowFocused: true },
    })

    await wrapper.get('main').trigger('pointerenter')
    expect(wrapper.get('.artwork-unfocused-progress').attributes('data-visible')).toBe('false')

    await wrapper.get('main').trigger('pointerleave')
    await wrapper.setProps({ isWindowFocused: false })

    const progress = wrapper.get('.artwork-unfocused-progress')
    expect(progress.element.tagName).toBe('PROGRESS')
    expect(progress.attributes('aria-label')).toBe('Track progress')
    expect(progress.attributes('value')).toBe('57000')
    expect(progress.attributes('max')).toBe('238000')
    expect(progress.classes()).toEqual(
      expect.arrayContaining(['absolute', 'right-0', 'bottom-0', 'left-0']),
    )
  })
})
