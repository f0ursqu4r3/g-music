import { DOMWrapper, enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
const body = () => new DOMWrapper(document.body)
import { afterEach, describe, expect, it } from 'vitest'
import type { MediaItem, TrackMetadataUpdate } from '@/api'
import LibraryWindow from '../LibraryWindow.vue'
enableAutoUnmount(afterEach)
const tracks: MediaItem[] = [
  {
    id: 'a',
    title: 'Alpha',
    artist: 'Shared',
    album: 'First',
    label: 'Blue',
    genres: ['Jazz'],
    durationMs: 10,
  },
  {
    id: 'b',
    title: 'Beta',
    artist: 'Shared',
    album: 'Second',
    label: 'Red',
    genres: ['Jazz'],
    durationMs: 20,
  },
  { id: 'c', title: 'Gamma', artist: 'Third', durationMs: 30 },
]
describe('library hardening', () => {
  it('retains a failed new playlist name and saves it on retry', async () => {
    let failed = true
    let storedName = ''
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: {
        isUpdating: false,
        tracks,
        savePlaylist: async (playlist) => {
          if (failed) throw new Error('Disk full')
          storedName = playlist.name
        },
      },
    })
    await wrapper.get('button[aria-label="New playlist"]').trigger('click')
    const name = wrapper.get('input[aria-label="New playlist name"]')
    await name.setValue('Draft mix')
    await wrapper.get('[data-new-playlist-editor]').trigger('submit')
    await flushPromises()
    expect((name.element as HTMLInputElement).value).toBe('Draft mix')
    expect(body().get('[role="alert"]').text()).toContain('Disk full')
    failed = false
    await wrapper.get('button[aria-label="Save new playlist"]').trigger('click')
    await flushPromises()
    expect(storedName).toBe('Draft mix')
    expect(wrapper.find('[data-new-playlist-editor]').exists()).toBe(false)
  })
  it('searches all metadata without losing playlist filters and plays visible order', async () => {
    const played: string[][] = []
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: {
        isUpdating: false,
        tracks,
        playlists: [{ id: 'p', name: 'Mix', trackIds: ['b', 'a'] }],
        onPlayTrack: (ids: string[]) => {
          played.push(ids)
        },
      },
    })
    await wrapper.get('[data-library-search-toggle]').trigger('click')
    const search = wrapper.get('input[aria-label="Search library"]')
    for (const [query, ids] of [
      ['blue', ['a']],
      ['Second', ['b']],
      ['Shared', ['a', 'b']],
      ['gamma', ['c']],
    ] as const) {
      await search.setValue(query)
      expect(
        wrapper.findAll('[data-track-id]').map((row) => row.attributes('data-track-id')),
      ).toEqual(ids)
    }
    await wrapper.get('[data-playlist-id="p"]').trigger('click')
    await search.setValue('jazz')
    expect(wrapper.get('[data-search-count]').text()).toContain('2')
    await wrapper.get('button[aria-label="Play Alpha"]').trigger('click')
    expect(played).toEqual([['a', 'b']])
    await search.setValue('not found')
    expect(wrapper.get('[data-search-empty]').text()).toContain('No matches')
    await wrapper.get('button[aria-label="Clear library search"]').trigger('click')
    expect(wrapper.findAll('[data-track-id]')).toHaveLength(2)
  })
  it('keeps the actual parent metadata dialog on failure and closes after successful retry', async () => {
    let failed = true
    let saved: TrackMetadataUpdate[] = []
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: {
        isUpdating: false,
        tracks,
        saveMetadata: async (updates: TrackMetadataUpdate[]) => {
          if (failed) throw new Error('Disk full')
          saved = updates
        },
      },
    })
    await wrapper.get('[data-track-id="a"]').trigger('contextmenu')
    ;(document.querySelector('[data-track-context-action="edit"]') as HTMLElement).click()
    await flushPromises()
    await body().get('[data-metadata-field="title"]').setValue('Draft')
    await body().get('[role="dialog"] form').trigger('submit')
    await flushPromises()
    expect(body().get('[role="alert"]').text()).toContain('Disk full')
    failed = false
    await body().get('[role="dialog"] form').trigger('submit')
    await flushPromises()
    expect(saved[0]?.metadata.title).toBe('Draft')
    expect(body().find('[role="dialog"]').exists()).toBe(false)
  })
  it('plays a clicked playlist track in visible order even without a text query', async () => {
    const played: string[][] = []
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: {
        isUpdating: false,
        tracks,
        playlists: [{ id: 'p', name: 'Mix', trackIds: ['b', 'a'] }],
        onPlayTrack: (ids: string[]) => {
          played.push(ids)
        },
      },
    })
    await wrapper.get('[data-playlist-id="p"]').trigger('click')
    await wrapper.get('button[aria-label="Play Beta"]').trigger('click')
    expect(played).toEqual([['a', 'b']])
  })
  it('keeps large search result sets virtualized and narrows to a visible match', async () => {
    const large = Array.from({ length: 5000 }, (_, index) => ({
      ...tracks[0]!,
      id: `track-${index}`,
      title: `Track ${index}`,
    }))
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: { isUpdating: false, tracks: large },
    })
    await wrapper.get('[data-library-search-toggle]').trigger('click')
    const search = wrapper.get('input[aria-label="Search library"]')
    await search.setValue('shared')
    expect(wrapper.get('[data-search-count]').text()).toContain('5000')
    expect(wrapper.findAll('[data-track-id]').length).toBeLessThan(100)
    await search.setValue('Track 4999')
    expect(
      wrapper.findAll('[data-track-id]').map((row) => row.attributes('data-track-id')),
    ).toEqual(['track-4999'])
  })
  it('uses keyboard radio menu selection and returns focus on Escape', async () => {
    const wrapper = mount(LibraryWindow, {
      attachTo: document.body,
      props: { isUpdating: false, tracks },
    })
    const trigger = wrapper.get('button[aria-label="More library options"]')
    ;(trigger.element as HTMLElement).focus()
    await trigger.trigger('keydown', { key: 'ArrowDown' })
    await flushPromises()
    const selected = document.querySelector('[data-sort="title-asc"]')
    expect(selected?.getAttribute('aria-checked')).toBe('true')
    ;(selected as HTMLElement).focus()
    ;(selected as HTMLElement).dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }),
    )
    await flushPromises()
    expect(document.activeElement?.getAttribute('data-sort')).toBe('title-desc')
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    )
    await flushPromises()
    await expect.poll(() => document.activeElement).toBe(trigger.element)
    await trigger.trigger('keydown', { key: 'ArrowDown' })
    await flushPromises()
    ;(document.querySelector('[data-sort="title-desc"]') as HTMLElement).click()
    await flushPromises()
    expect(document.querySelector('.library-options-menu')).toBeNull()
  })
})
