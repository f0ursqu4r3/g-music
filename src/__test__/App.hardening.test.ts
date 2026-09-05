import { DOMWrapper, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Storage } from 'happy-dom'
import { flushApp as flushPromises } from './flushApp'
import App from '../App.vue'
import type { PlaybackSnapshot, Playlist } from '@/api'

const events = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>())
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: (path: string) => path,
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name, handler) => {
    events.set(name, handler)
    return () => events.delete(name)
  }),
}))
vi.mock('@tauri-apps/api/window', () => ({
  LogicalSize: vi.fn(),
  getCurrentWindow: () => ({
    isFocused: async () => true,
    onFocusChanged: async () => () => {},
    close: async () => {},
  }),
}))
const track = {
  id: 'a',
  title: 'A session',
  artist: 'Artist',
  durationMs: 100000,
}
let state: PlaybackSnapshot
let favorites: string[]
let customPlaylists: Playlist[]
let wrapper: VueWrapper | undefined
async function open(view = 'library') {
  window.history.replaceState({}, '', `/?view=${view}`)
  wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  return wrapper
}
beforeEach(() => {
  vi.stubGlobal('localStorage', new Storage())
  events.clear()
  vi.mocked(listen)
    .mockReset()
    .mockImplementation(async (name, handler) => {
      events.set(name, handler as (event: { payload: unknown }) => void)
      return () => events.delete(name)
    })
  window.localStorage.clear()
  favorites = []
  customPlaylists = []
  state = {
    status: 'paused',
    currentItem: track,
    queue: [track],
    positionMs: 0,
    volumePercent: 70,
    shuffleEnabled: false,
    repeatMode: 'off',
  }
  vi.mocked(invoke)
    .mockReset()
    .mockImplementation(async (command, args) => {
      if (command === 'inspect_library')
        return {
          tracks: [track],
          playlists: [
            { id: 'favorites', name: 'Favorites', trackIds: favorites },
            ...customPlaylists,
          ],
        }
      if (command === 'inspect_metadata_refreshes')
        return { jobs: [], totalTracks: 0, completedTracks: 0 }
      if (command === 'inspect_import_progress') return null
      if (command === 'inspect_diagnostics')
        return {
          appVersion: '0.1',
          platform: 'macos',
          dependencies: [],
          audioOutputPolicy: 'System default',
        }
      if (command === 'toggle_favorite') {
        favorites = favorites.length ? [] : [String((args as { id: string }).id)]
        return {
          tracks: [track],
          playlists: [{ id: 'favorites', name: 'Favorites', trackIds: favorites }],
        }
      }
      if (command === 'toggle_shuffle') state = { ...state, shuffleEnabled: !state.shuffleEnabled }
      if (command === 'cycle_repeat_mode') state = { ...state, repeatMode: 'all' }
      if (command === 'play') state = { ...state, status: 'playing' }
      return state
    })
})
afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
  document.body.innerHTML = ''
})
describe('real App command routing', () => {
  it('applies lightweight volume events without reloading the library', async () => {
    const app = await open('mini')
    vi.mocked(invoke).mockClear()
    events.get('playback-transport-updated')?.({
      payload: { ...state, volumePercent: 23 },
    })
    await flushPromises()
    expect(app.get('[role="slider"][aria-label="Volume"]').attributes('aria-valuenow')).toBe('23')
    expect(invoke).not.toHaveBeenCalledWith('inspect_library')
  })
  it('retries native playback failure without toggling a stale playing status to pause', async () => {
    state.status = 'playing'
    const app = await open('mini')
    events.get('playback-error')?.({
      payload: { code: 'load_failed', message: 'Could not play' },
    })
    await flushPromises()
    await app.get('[data-sonner-toast] button[data-action]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('play')
    expect(invoke).not.toHaveBeenCalledWith('pause')
  })
  it('routes playlist membership removal through the actual App', async () => {
    customPlaylists = [{ id: 'mix', name: 'Mix', trackIds: ['a'] }]
    const app = await open()
    await app.get('[data-playlist-id="mix"]').trigger('click')
    await app.get('[data-track-id="a"]').trigger('contextmenu')
    vi.mocked(invoke).mockResolvedValueOnce({
      tracks: [track],
      playlists: [{ ...customPlaylists[0], trackIds: [] }],
    })
    ;(
      document.querySelector('[data-track-context-action="remove-from-playlist"]') as HTMLElement
    ).click()
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('upsert_playlist', {
      playlist: { id: 'mix', name: 'Mix', trackIds: [] },
    })
    expect(app.find('[data-track-id="a"]').exists()).toBe(false)
  })
  it('retries the failed native Import window action', async () => {
    const app = await open()
    vi.mocked(invoke).mockRejectedValueOnce(new Error('Cannot open window'))
    await app.get('button[aria-label="Import music"]').trigger('click')
    await flushPromises()
    await app.get('[data-sonner-toast] button[data-action]').trigger('click')
    await flushPromises()
    expect(
      vi.mocked(invoke).mock.calls.filter(([command]) => command === 'show_import_window'),
    ).toHaveLength(2)
  })
  it.each(['playback-updated', 'import-progress'])(
    'retains Retry for a failed %s subscription',
    async (failedEvent) => {
      vi.mocked(listen).mockImplementation(async (name, handler) => {
        if (name === failedEvent) throw new Error('Event connection failed')
        events.set(name, handler as (event: { payload: unknown }) => void)
        return () => events.delete(name)
      })
      const app = await open()
      if (failedEvent === 'import-progress')
        expect(invoke).not.toHaveBeenCalledWith('inspect_import_progress')
      const dismiss = app.find('button[aria-label="Dismiss error"]')
      if (dismiss.exists()) await dismiss.trigger('click')
      expect(app.find('[data-sonner-toast] button[data-action]').exists()).toBe(true)
      vi.mocked(listen).mockImplementation(async (name, handler) => {
        events.set(name, handler as (event: { payload: unknown }) => void)
        return () => events.delete(name)
      })
      await app.get('[data-sonner-toast] button[data-action]').trigger('click')
      await flushPromises()
      expect(events.has(failedEvent)).toBe(true)
      expect(invoke).toHaveBeenCalledWith('inspect_import_progress')
      await expect
        .poll(() => app.find('[data-sonner-toast][data-removed="false"]').exists())
        .toBe(false)
    },
  )
  it('applies a native queue-clearing event to the real Queue window', async () => {
    const app = await open('queue')
    expect(app.find('button[aria-label="Play A session"]').exists()).toBe(true)
    events.get('playback-updated')?.({
      payload: { ...state, currentItem: null, queue: [], playbackOrder: [] },
    })
    await flushPromises()
    expect(app.find('button[aria-label="Play A session"]').exists()).toBe(false)
    expect(app.get('button[aria-label="Clear queue"]').attributes('disabled')).toBeDefined()
  })
  it('keeps a metadata draft when the actual IPC save fails and retries that draft', async () => {
    const app = await open()
    await app.get('[data-track-id="a"]').trigger('contextmenu')
    ;(document.querySelector('[data-track-context-action="edit"]') as HTMLElement).click()
    await flushPromises()
    await new DOMWrapper(document.body).get('[data-metadata-field="title"]').setValue('My draft')
    vi.mocked(invoke).mockRejectedValueOnce({
      code: 'disk_full',
      message: 'Disk full',
    })
    await new DOMWrapper(document.body).get('[role="dialog"] form').trigger('submit')
    await flushPromises()
    expect(
      (
        new DOMWrapper(document.body).get('[data-metadata-field="title"]')
          .element as HTMLInputElement
      ).value,
    ).toBe('My draft')
    expect(new DOMWrapper(document.body).get('[role="dialog"]').text()).toContain('Disk full')
    vi.mocked(invoke).mockResolvedValueOnce({
      tracks: [{ ...track, title: 'My draft' }],
      playlists: [],
    })
    await new DOMWrapper(document.body).get('[role="dialog"] form').trigger('submit')
    await flushPromises()
    expect(new DOMWrapper(document.body).find('[role="dialog"]').exists()).toBe(false)
    expect(invoke).toHaveBeenCalledWith('update_tracks_metadata', {
      updates: [{ id: 'a', metadata: { title: 'My draft' } }],
    })
  })
  it('waits for listeners before reads and cleans up listeners that finish after unmount', async () => {
    const releases: (() => void)[] = []
    const unlisten = vi.fn()
    vi.mocked(listen).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          releases.push(() => resolve(unlisten))
        }),
    )
    wrapper = mount(App)
    await flushPromises()
    expect(invoke).not.toHaveBeenCalled()
    wrapper.unmount()
    wrapper = undefined
    releases.forEach((release) => release())
    await flushPromises()
    expect(unlisten).toHaveBeenCalledOnce()
    expect(invoke).not.toHaveBeenCalled()
  })
  it('traps shortcut dialog keys and returns focus after Escape', async () => {
    const app = await open()
    const trigger = app.get('button[aria-label="Import music"]')
    ;(trigger.element as HTMLElement).focus()
    events.get('show-keyboard-shortcuts')?.({ payload: null })
    await flushPromises()
    expect(document.activeElement?.closest('[role="dialog"]')).not.toBeNull()
    vi.mocked(invoke).mockClear()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'j' }))
    expect(invoke).not.toHaveBeenCalledWith('previous')
    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    )
    await flushPromises()
    expect(new DOMWrapper(document.body).find('[role="dialog"]').exists()).toBe(false)
    await expect.poll(() => document.activeElement).toBe(trigger.element)
  })
  it('retries native playback failures with playback', async () => {
    const app = await open('mini')
    events.get('playback-error')?.({
      payload: { code: 'load_failed', message: 'Could not load track' },
    })
    await flushPromises()
    await app.get('[data-sonner-toast] button[data-action]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('play')
  })
  it.each(['inspect_library', 'inspect_playback'])(
    'retains Retry after repeated startup %s failures',
    async (failedCommand) => {
      const normal = vi.mocked(invoke).getMockImplementation()!
      let failures = 2
      vi.mocked(invoke).mockImplementation(async (command, args) => {
        if (command === failedCommand && failures-- > 0) throw new Error('Library is unavailable')
        return normal(command, args)
      })
      const app = await open()
      expect(app.get('[data-sonner-toast]').text()).toContain('Library is unavailable')
      const dismiss = app.find('button[aria-label="Dismiss error"]')
      if (dismiss.exists()) await dismiss.trigger('click')
      expect(app.find('[data-sonner-toast] button[data-action]').exists()).toBe(true)
      expect(app.find('[aria-label="Loading music window"]').exists()).toBe(false)
      await app.get('[data-sonner-toast] button[data-action]').trigger('click')
      await flushPromises()
      expect(app.find('button[aria-label="Dismiss error"]').exists()).toBe(false)
      expect(
        app.find('[data-sonner-toast][data-removed="false"] button[data-action]').exists(),
      ).toBe(true)
      await app.get('[data-sonner-toast] button[data-action]').trigger('click')
      await flushPromises()
      expect(app.get('main').attributes('aria-label')).toBe('Music library')
      await expect
        .poll(() => app.find('[data-sonner-toast][data-removed="false"]').exists())
        .toBe(false)
    },
  )
  it('persists Library footer favorite through the actual composable and IPC', async () => {
    const app = await open()
    await app.get('button[aria-label="Favorite track"]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('toggle_favorite', { id: 'a' })
    expect(app.get('button[aria-label="Favorite track"]').attributes('aria-pressed')).toBe('true')
  })
  it('routes Artwork shuffle, repeat and favorite using authoritative snapshots', async () => {
    const app = await open('artwork')
    await app.get('button[aria-label="Enable shuffle"]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('toggle_shuffle')
    expect(app.get('button[aria-label="Disable shuffle"]').attributes('aria-pressed')).toBe('true')
    await app.get('button[aria-label="Enable repeat all"]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('cycle_repeat_mode')
  })
  it('does not hijack Space on a button or a prevented keyboard event', async () => {
    const app = await open()
    vi.mocked(invoke).mockClear()
    await app.get('button[aria-label="Import music"]').trigger('keydown', { key: ' ' })
    const event = new KeyboardEvent('keydown', { key: ' ', cancelable: true })
    event.preventDefault()
    window.dispatchEvent(event)
    await flushPromises()
    expect(invoke).not.toHaveBeenCalledWith('play')
  })
  it.each(['library', 'mini', 'artwork', 'queue', 'import', 'settings'])(
    'shows native playback errors in %s',
    async (view) => {
      const app = await open(view)
      events.get('playback-error')?.({
        payload: {
          code: 'load_failed',
          message: 'Audio output failed. Retry playback.',
        },
      })
      await flushPromises()
      expect(app.text().split('Audio output failed. Retry playback.')).toHaveLength(2)
      expect(app.find('[data-sonner-toast] button[data-action]').exists()).toBe(true)
    },
  )
  it('receives a theme change from another window and removes the listener on unmount', async () => {
    await open()
    window.dispatchEvent(new StorageEvent('storage', { key: 'gmusic-theme', newValue: 'ember' }))
    await flushPromises()
    expect(document.documentElement.dataset.theme).toBe('ember')
    wrapper?.unmount()
    wrapper = undefined
    window.dispatchEvent(new StorageEvent('storage', { key: 'gmusic-theme', newValue: 'plum' }))
    await flushPromises()
    expect(document.documentElement.dataset.theme).toBe('ember')
  })
})
