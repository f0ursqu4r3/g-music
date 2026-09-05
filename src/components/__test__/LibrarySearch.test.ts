import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import LibraryWindow from '../LibraryWindow.vue'

enableAutoUnmount(afterEach)
const props = {
  isUpdating: false,
  tracks: [
    { id: 'a', title: 'Alpha', artist: 'Artist', durationMs: 10 },
    { id: 'b', title: 'Beta', artist: 'Artist', durationMs: 20 },
  ],
}
const inputSelector = 'input[aria-label="Search library"]'
const toggleSelector = '[data-library-search-toggle]'

function key(key: string, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
    ...options,
  })
  ;(document.activeElement ?? document.body).dispatchEvent(event)
  return event
}

describe('collapsible library search', () => {
  it('starts collapsed, focuses on opening, and clears filtering when closed', async () => {
    const wrapper = mount(LibraryWindow, { attachTo: document.body, props })
    const toggle = wrapper.get(toggleSelector)
    expect(toggle.attributes('aria-expanded')).toBe('false')
    expect(wrapper.find(inputSelector).exists()).toBe(false)
    await toggle.trigger('click')
    expect(toggle.attributes('aria-expanded')).toBe('true')
    const input = wrapper.get(inputSelector)
    expect(document.activeElement).toBe(input.element)
    await input.setValue('Alpha')
    expect(wrapper.findAll('[data-track-id]')).toHaveLength(1)
    await toggle.trigger('click')
    expect(wrapper.find(inputSelector).exists()).toBe(false)
    expect(wrapper.findAll('[data-track-id]')).toHaveLength(2)
    expect(document.activeElement).toBe(toggle.element)
    await toggle.trigger('click')
    expect((wrapper.get(inputSelector).element as HTMLInputElement).value).toBe('')
  })

  it.each(['metaKey', 'ctrlKey'])('toggles with %s+F and closes with Escape', async (modifier) => {
    const wrapper = mount(LibraryWindow, { attachTo: document.body, props })
    expect(key('f', { [modifier]: true }).defaultPrevented).toBe(true)
    await flushPromises()
    expect(document.activeElement).toBe(wrapper.get(inputSelector).element)
    key('f', { [modifier]: true })
    await flushPromises()
    expect(wrapper.find(inputSelector).exists()).toBe(false)
    key('f', { [modifier]: true })
    await flushPromises()
    await wrapper.get(inputSelector).setValue('Alpha')
    expect(key('Escape').defaultPrevented).toBe(true)
    await flushPromises()
    expect(wrapper.find(inputSelector).exists()).toBe(false)
    expect(wrapper.findAll('[data-track-id]')).toHaveLength(2)
    expect(document.activeElement).toBe(wrapper.get(toggleSelector).element)
  })

  it('does not intercept typing, repeat events, other editors, or shortcuts after unmount', async () => {
    const wrapper = mount(LibraryWindow, { attachTo: document.body, props })
    expect(key('f').defaultPrevented).toBe(false)
    expect(key('f', { metaKey: true, repeat: true }).defaultPrevented).toBe(false)
    await wrapper.get('button[aria-label="New playlist"]').trigger('click')
    const name = wrapper.get('input[aria-label="New playlist name"]')
    ;(name.element as HTMLInputElement).focus()
    expect(key('f', { metaKey: true }).defaultPrevented).toBe(false)
    await flushPromises()
    expect(wrapper.find(inputSelector).exists()).toBe(false)
    wrapper.unmount()
    expect(key('f', { metaKey: true }).defaultPrevented).toBe(false)
  })
})
