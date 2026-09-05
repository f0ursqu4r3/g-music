import { DOMWrapper, flushPromises, mount } from '@vue/test-utils'
const body = () => new DOMWrapper(document.body)
import { afterEach, describe, expect, it } from 'vitest'

import type { MediaItem, PlaybackSnapshot, Playlist } from '@/api'
import LibraryWindow from '../LibraryWindow.vue'

const tracks: MediaItem[] = [
  { id: 'track-a', title: 'Alpha', artist: 'Artist', durationMs: 60_000 },
  { id: 'track-b', title: 'Beta', artist: 'Artist', durationMs: 60_000 },
  { id: 'track-c', title: 'Gamma', artist: 'Artist', durationMs: 60_000 },
]
const snapshot: PlaybackSnapshot = {
  currentItem: tracks[0],
  queue: tracks,
  positionMs: 0,
  status: 'playing',
  volumePercent: 70,
}
const playlists: Playlist[] = [
  { id: 'focus', name: 'Focus', trackIds: ['track-c', 'track-a', 'track-b'] },
  { id: 'other', name: 'Other', trackIds: ['track-a', 'track-b'] },
  { id: 'favorites', name: 'Favorites', trackIds: ['track-a'] },
  { id: 'most-played', name: 'Most Played', trackIds: ['track-a'] },
]
const wrappers: ReturnType<typeof mount>[] = []
function mountLibrary() {
  const wrapper = mount(LibraryWindow, {
    attachTo: document.body,
    props: { tracks, playlists, snapshot, isUpdating: false },
  })
  wrappers.push(wrapper)
  return wrapper
}
function removalAction() {
  return document.body.querySelector<HTMLElement>(
    '[data-track-context-action="remove-from-playlist"]',
  )
}
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount()
})

describe('playlist track removal', () => {
  it.each(['List', 'Grid'])('removes only playlist membership from the %s view', async (view) => {
    const wrapper = mountLibrary()
    await wrapper.get('[data-playlist-id="focus"]').trigger('click')
    await wrapper.get(`button[aria-label="${view} view"]`).trigger('click')
    await wrapper.get('[data-track-id="track-a"]').trigger('contextmenu')
    const action = removalAction()
    expect(action?.textContent).toContain('Remove from playlist')
    expect(
      document.body.querySelector('[data-track-context-action="remove"]')?.textContent,
    ).toContain('Remove from library')
    action!.click()
    await flushPromises()

    const updated = { ...playlists[0]!, trackIds: ['track-c', 'track-b'] }
    expect(wrapper.emitted('upsertPlaylist')).toEqual([[updated]])
    expect(wrapper.emitted('removeTracks')).toBeUndefined()
    expect(body().find('[role="dialog"]').exists()).toBe(false)
    expect(wrapper.props('tracks')).toEqual(tracks)
    expect(wrapper.props('snapshot')).toEqual(snapshot)
    expect(wrapper.props('playlists')).toEqual(playlists)

    await wrapper.setProps({ playlists: [updated, ...playlists.slice(1)] })
    expect(wrapper.find('[data-track-id="track-a"]').exists()).toBe(false)
    await wrapper.get('[data-collection="tracks"]').trigger('click')
    expect(wrapper.find('[data-track-id="track-a"]').exists()).toBe(true)
    await wrapper.get('[data-playlist-id="other"]').trigger('click')
    expect(wrapper.find('[data-track-id="track-a"]').exists()).toBe(true)
  })

  it('removes the full selection without deleting an empty playlist', async () => {
    const wrapper = mountLibrary()
    await wrapper.get('[data-playlist-id="focus"]').trigger('click')
    await wrapper.get('[data-track-id="track-a"]').trigger('click')
    await wrapper.get('[data-track-id="track-c"]').trigger('click', { shiftKey: true })
    await wrapper.get('[data-track-id="track-b"]').trigger('contextmenu')
    expect(removalAction()?.textContent).toContain('Remove 3 tracks from playlist')
    removalAction()!.click()
    await flushPromises()
    const updated = { ...playlists[0]!, trackIds: [] }
    expect(wrapper.emitted('upsertPlaylist')).toEqual([[updated]])
    expect(wrapper.emitted('removeTracks')).toBeUndefined()
    expect(wrapper.emitted('deletePlaylist')).toBeUndefined()
    await wrapper.setProps({ playlists: [updated, ...playlists.slice(1)] })
    expect(wrapper.findAll('[data-track-id]')).toHaveLength(0)
    expect(wrapper.find('[data-playlist-id="focus"]').exists()).toBe(true)
  })

  it.each([null, 'favorites', 'most-played'])(
    'does not offer playlist removal in %s',
    async (playlistId) => {
      const wrapper = mountLibrary()
      if (playlistId) await wrapper.get(`[data-playlist-id="${playlistId}"]`).trigger('click')
      await wrapper.get('[data-track-id="track-a"]').trigger('contextmenu')
      expect(document.body.querySelector('[data-track-context-menu]')).not.toBeNull()
      expect(removalAction()).toBeNull()
      expect(wrapper.emitted('upsertPlaylist')).toBeUndefined()
    },
  )

  it('disables playlist removal while a mutation is pending', async () => {
    const wrapper = mountLibrary()
    await wrapper.get('[data-playlist-id="focus"]').trigger('click')
    await wrapper.get('[data-track-id="track-a"]').trigger('contextmenu')
    await wrapper.setProps({ isUpdating: true })
    expect(removalAction()?.getAttribute('aria-disabled')).toBe('true')
    removalAction()!.click()
    await flushPromises()
    expect(wrapper.emitted('upsertPlaylist')).toBeUndefined()
    expect(wrapper.emitted('removeTracks')).toBeUndefined()
  })
})
