import { invoke } from '@tauri-apps/api/core'
import { emitTo } from '@tauri-apps/api/event'
import { DOMWrapper, mount, type VueWrapper } from '@vue/test-utils'
import { Storage } from 'happy-dom'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import App from '@/App.vue'
import { flushApp as flush } from '@/__test__/flushApp'
import type { LibrarySnapshot, Playlist, SmartPlaylistDefinition } from '@/api'
const focusEvents = vi.hoisted(() => ({
  changed: undefined as ((event: { payload: boolean }) => void) | undefined,
}))
const events = vi.hoisted(() => new Map<string, (e: { payload: unknown }) => void>())
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), convertFileSrc: (p: string) => p }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name, handler) => {
    events.set(name, handler)
    return () => events.delete(name)
  }),
  emitTo: vi.fn().mockResolvedValue(undefined),
}))
vi.mock('@tauri-apps/api/window', () => ({
  LogicalSize: vi.fn(),
  getCurrentWindow: () => ({
    isFocused: async () => true,
    label: 'main',
    onFocusChanged: async (callback: (event: { payload: boolean }) => void) => {
      focusEvents.changed = callback
      return () => {
        focusEvents.changed = undefined
      }
    },
  }),
}))
let wrapper: VueWrapper
const body = () => new DOMWrapper(document.body)
const smart: SmartPlaylistDefinition = {
  match: 'all',
  rules: [{ field: 'playCount', operator: 'equals', value: 0 }],
  sort: { field: 'libraryOrder', direction: 'asc' },
  limit: null,
}
let library: LibrarySnapshot
const overrides = new Map<string, () => Promise<unknown>>()
const state = { currentItem: null, status: 'paused', queue: [], positionMs: 0, volumePercent: 50 }
beforeEach(async () => {
  vi.stubGlobal('localStorage', new Storage())
  events.clear()
  overrides.clear()
  vi.mocked(emitTo).mockClear()
  library = {
    tracks: [
      { id: 'a', title: 'Alpha', artist: 'Artist', album: 'Album', durationMs: 1 },
      { id: 'b', title: 'Beta', artist: 'Artist', album: 'Album', durationMs: 2 },
    ],
    playlists: [
      { id: 's', name: 'Smart', smart, trackIds: ['b', 'a'] },
      { id: 'p', name: 'Manual', trackIds: ['b'] },
    ],
  }
  vi.mocked(invoke)
    .mockReset()
    .mockImplementation(async (cmd, args) => {
      if (overrides.has(cmd)) return overrides.get(cmd)!()
      if (cmd === 'inspect_library') return library
      if (cmd === 'inspect_import_progress') return null
      if (cmd === 'inspect_diagnostics')
        return { dependencies: [], appVersion: 'test', platform: 'test', audioOutputPolicy: 'test' }
      if (cmd === 'inspect_youtube_auth') return { connected: false }
      if (cmd === 'inspect_metadata_refreshes')
        return { jobs: [], totalTracks: 0, completedTracks: 0 }
      if (cmd === 'preview_smart_playlist')
        return { totalMatches: 2, matches: [{ trackId: 'b', matchedRuleIndexes: [0] }] }
      if (cmd === 'upsert_playlist') {
        const p = (args as { playlist: Playlist }).playlist
        library = { ...library, playlists: [...library.playlists.filter((x) => x.id !== p.id), p] }
        return library
      }
      if (cmd === 'freeze_smart_playlist') {
        library = {
          ...library,
          playlists: library.playlists.map((p) =>
            p.id === 's' ? { id: p.id, name: p.name, trackIds: p.trackIds } : p,
          ),
        }
        return library
      }
      if (cmd === 'delete_playlist') {
        library = { ...library, playlists: library.playlists.filter((p) => p.id !== 's') }
        return library
      }
      return state
    })
  window.history.replaceState({}, '', '/?view=library')
  wrapper = mount(App, { attachTo: document.body })
  await flush()
})
afterEach(() => {
  wrapper?.unmount()
  document.body.innerHTML = ''
  vi.unstubAllGlobals()
})
const calls = (name: string) => vi.mocked(invoke).mock.calls.filter((c) => c[0] === name)
it('creates only on Save, explains previews, and retains failed drafts through the real owner', async () => {
  await wrapper.get('[aria-label="New smart playlist"]').trigger('click')
  await flush()
  await body().get('[data-smart-preset="never-played"]').trigger('click')
  expect(calls('upsert_playlist')).toHaveLength(0)
  await body().get('[data-smart-preview]').trigger('click')
  await flush()
  expect(body().text()).toContain('2 matches')
  expect(body().text()).toContain('Play count equals 0')
  overrides.set('upsert_playlist', async () => {
    throw new Error('Disk full')
  })
  await body().get('[data-smart-name]').setValue('My draft')
  await body().get('[data-smart-form]').trigger('submit')
  await flush()
  expect(body().get('[data-smart-error]').text()).toContain('Disk full')
  expect((body().get('[data-smart-name]').element as HTMLInputElement).value).toBe('My draft')
  overrides.delete('upsert_playlist')
  await body().get('[data-smart-form]').trigger('submit')
  await flush()
  expect(body().find('[data-smart-form]').exists()).toBe(false)
  expect(wrapper.text()).toContain('My draft')
})
it('keeps derived order, blocks rendered drops/removal, and confirms freeze', async () => {
  await wrapper.get('[data-playlist-id="s"]').trigger('click')
  expect(wrapper.findAll('[data-track-id]').map((r) => r.attributes('data-track-id'))).toEqual([
    'b',
    'a',
  ])
  await wrapper
    .get('[data-playlist-reorder-item="s"]')
    .trigger('drop', { dataTransfer: { getData: () => '["a"]' } })
  await wrapper.get('[data-track-id="a"]').trigger('contextmenu')
  await flush()
  expect(body().find('[data-track-context-action="remove-from-playlist"]').exists()).toBe(false)
  expect(calls('upsert_playlist')).toHaveLength(0)
  await body().get('[role="menu"]').trigger('keydown', { key: 'Escape' })
  await flush()
  await wrapper.get('[aria-label="Edit Smart"]').trigger('click')
  await flush()
  await body().get('[data-smart-freeze]').trigger('click')
  expect(calls('freeze_smart_playlist')).toHaveLength(0)
  await body().get('[data-smart-confirm]').trigger('click')
  await flush()
  expect(calls('freeze_smart_playlist')).toEqual([['freeze_smart_playlist', { id: 's' }]])
  await wrapper.get('[aria-label="Edit Smart"]').trigger('click')
  await flush()
  expect(body().find('[data-playlist-track="a"]').exists()).toBe(true)
})
it('opens from the header and keyboard, waits for entire queue batches and recovers errors', async () => {
  await wrapper.get('[data-command-palette-trigger]').trigger('click')
  await flush()
  await body().get('[data-command-search]').setValue('Album')
  let resolve!: (value: unknown) => void
  overrides.set(
    'add_to_queue',
    () =>
      new Promise((r) => {
        resolve = r
      }),
  )
  await body().get('[data-command-mode]').setValue('queue')
  await body().get('[data-command-search]').trigger('keydown', { key: 'Enter' })
  await flush()
  expect(calls('add_to_queue')).toHaveLength(1)
  expect(body().find('[data-command-search]').exists()).toBe(true)
  resolve(state)
  await flush()
  expect(calls('add_to_queue')).toHaveLength(2)
  resolve(state)
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(false)
  window.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, bubbles: true, cancelable: true }),
  )
  await flush()
  overrides.set('play_track', async () => {
    throw new Error('Playback unavailable')
  })
  await body().get('[data-command-search]').setValue('Alpha')
  await body().get('[data-command-search]').trigger('keydown', { key: 'Enter' })
  await flush()
  expect(body().get('[data-command-error]').text()).toContain('Playback unavailable')
  overrides.delete('play_track')
  await body().get('[data-command-search]').trigger('keydown', { key: 'Enter' })
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(false)
})
it('keeps failed conversion and deletion open, blocks dismissal while pending, and retries', async () => {
  await wrapper.get('[aria-label="Edit Smart"]').trigger('click')
  await flush()
  await body().get('[data-smart-freeze]').trigger('click')
  overrides.set('freeze_smart_playlist', async () => {
    throw new Error('Convert failed')
  })
  await body().get('[data-smart-confirm]').trigger('click')
  await flush()
  expect(body().get('[data-smart-error]').text()).toContain('Convert failed')
  await body()
    .findAll('button')
    .find((b) => b.text() === 'Cancel')!
    .trigger('click')
  await body().get('[data-smart-delete]').trigger('click')
  let reject!: (error: Error) => void
  overrides.set(
    'delete_playlist',
    () =>
      new Promise((_, r) => {
        reject = r
      }),
  )
  await body().get('[data-smart-confirm]').trigger('click')
  await flush()
  await body().get('[role="dialog"]').trigger('keydown', { key: 'Escape' })
  await flush()
  expect(body().find('[data-smart-form]').exists()).toBe(true)
  expect(body().get('[data-smart-confirm]').attributes('disabled')).toBeDefined()
  reject(new Error('Delete failed'))
  await flush()
  expect(body().get('[data-smart-error]').text()).toContain('Delete failed')
  overrides.delete('delete_playlist')
  await body().get('[data-smart-confirm]').trigger('click')
  await flush()
  expect(wrapper.find('[data-playlist-id="s"]').exists()).toBe(false)
  expect(wrapper.findAll('[data-track-id]')).toHaveLength(2)
})
it('adds the rendered selection only to ordinary playlists and preserves existing order', async () => {
  await wrapper.get('[data-track-id="a"]').trigger('click')
  await wrapper.get('[data-command-palette-trigger]').trigger('click')
  await flush()
  await body().get('[data-command-search]').setValue('Add selection')
  expect(body().findAll('[data-command-result]')).toHaveLength(1)
  expect(body().get('[data-command-result]').text()).toContain('Manual')
  await body().get('[data-command-result]').trigger('click')
  await flush()
  expect(calls('upsert_playlist')[0]?.[1]).toEqual({
    playlist: { id: 'p', name: 'Manual', trackIds: ['b', 'a'] },
  })
  expect(wrapper.get('[data-track-id="a"]').attributes('data-selected')).toBe('true')
})
it('uses focus, arrows, Enter, Escape, and ignores composition/repeated/handled shortcuts', async () => {
  for (const options of [{ repeat: true }, { isComposing: true }]) {
    window.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'k', metaKey: true, bubbles: true, ...options }),
    )
    await flush()
    expect(body().find('[data-command-search]').exists()).toBe(false)
  }
  const handled = new KeyboardEvent('keydown', { key: 'k', ctrlKey: true, cancelable: true })
  handled.preventDefault()
  window.dispatchEvent(handled)
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(false)
  const trigger = wrapper.get('[data-command-palette-trigger]')
  ;(trigger.element as HTMLElement).focus()
  await trigger.trigger('click')
  await flush()
  expect(document.activeElement).toBe(body().get('[data-command-search]').element)
  await body().get('[data-command-search]').trigger('keydown', { key: 'ArrowDown' })
  expect(body().get('[data-command-search]').attributes('aria-activedescendant')).toBe(
    'command-option-1',
  )
  await body().get('[data-command-search]').trigger('keydown', { key: 'Enter', isComposing: true })
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(true)
  await body().get('[data-command-search]').trigger('keydown', { key: 'Escape' })
  await flush()
  expect(document.activeElement).toBe(trigger.element)
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, cancelable: true }))
  await flush()
  expect(wrapper.find('[aria-label="Search library"]').exists()).toBe(true)
})
it('updates an open smart collection from library events without losing selection', async () => {
  await wrapper.get('[data-playlist-id="s"]').trigger('click')
  await wrapper.get('[data-track-id="b"]').trigger('click')
  library = {
    ...library,
    playlists: library.playlists.map((p) => (p.id === 's' ? { ...p, trackIds: ['b'] } : p)),
  }
  events.get('library-updated')?.({ payload: null })
  await flush()
  expect(wrapper.findAll('[data-track-id]').map((t) => t.attributes('data-track-id'))).toEqual([
    'b',
  ])
  expect(wrapper.get('[data-track-id="b"]').attributes('data-selected')).toBe('true')
  await wrapper.get('[aria-label="Grid view"]').trigger('click')
  await wrapper.get('[data-track-id="b"]').trigger('contextmenu')
  await flush()
  expect(body().find('[data-track-context-action="remove-from-playlist"]').exists()).toBe(false)
})
it('preserves collection order through native play-next insertion and awaits all commands', async () => {
  const current = { id: 'current', title: 'Current', artist: 'Other', durationMs: 10 }
  let transport = {
    ...state,
    status: 'playing',
    currentItem: current,
    queue: [current] as typeof library.tracks,
  }
  overrides.set('queue_track_next', async () => {
    const id = (calls('queue_track_next').slice(-1)[0]![1] as { id: string }).id
    transport.queue = transport.queue.filter((track) => track.id !== id)
    transport.queue.splice(
      transport.queue.findIndex((track) => track.id === transport.currentItem.id) + 1,
      0,
      library.tracks.find((track) => track.id === id)!,
    )
    return { ...transport, queue: [...transport.queue] }
  })
  events.get('playback-updated')?.({ payload: transport })
  await flush()
  await wrapper.get('[data-command-palette-trigger]').trigger('click')
  await flush()
  await body().get('[data-command-search]').setValue('Album')
  await body().get('[data-command-mode]').setValue('next')
  await body().get('[data-command-result]').trigger('click')
  await flush()
  expect(transport.queue.map((track) => track.id)).toEqual(['current', 'a', 'b'])
  expect(calls('queue_track_next').map((c) => c[1])).toEqual([{ id: 'b' }, { id: 'a' }])
})
it('plays a smart playlist in resolved order and shows no-results without changing selection', async () => {
  await wrapper.get('[data-track-id="a"]').trigger('click')
  await wrapper.get('[data-command-palette-trigger]').trigger('click')
  await flush()
  await body().get('[data-command-search]').setValue('Does not exist')
  expect(body().text()).toContain('No results')
  expect(body().findAll('[data-command-result]')).toHaveLength(0)
  await body().get('[data-command-search]').trigger('keydown', { key: 'Enter' })
  expect(calls('play_track')).toHaveLength(0)
  await body().get('[data-command-search]').setValue('Smart')
  await body().get('[data-command-result="playlist:s"]').trigger('click')
  await flush()
  expect(calls('play_track')).toEqual([['play_track', { id: 'b', queueIds: ['b', 'a'] }]])
  expect(wrapper.get('[data-track-id="a"]').attributes('data-selected')).toBe('true')
})
it.each([
  ['Open Queue', 'show_app_window', { surface: 'queue' }],
  ['Open Settings', 'show_app_window', { surface: 'settings' }],
  ['Import music', 'show_import_window', undefined],
] as const)('runs %s through the real API and retries failures', async (query, command, args) => {
  await wrapper.get('[data-command-palette-trigger]').trigger('click')
  await flush()
  await body().get('[data-command-search]').setValue(query)
  overrides.set(command, async () => {
    throw new Error('Window failed')
  })
  await body().get('[data-command-result]').trigger('click')
  await flush()
  expect(body().get('[data-command-error]').text()).toContain('Window failed')
  overrides.delete(command)
  await body().get('[data-command-result]').trigger('click')
  await flush()
  expect(calls(command).slice(-1)[0]).toEqual(args ? [command, args] : [command])
  expect(body().find('[data-command-search]').exists()).toBe(false)
})
it.each(['New playlist', 'New smart playlist'])(
  'routes %s to a draft without saving',
  async (query) => {
    await wrapper.get('[data-command-palette-trigger]').trigger('click')
    await flush()
    await body().get('[data-command-search]').setValue(query)
    await body().get('[data-command-search]').trigger('keydown', { key: 'Enter' })
    await flush()
    expect(
      body()
        .find(query === 'New playlist' ? '[data-new-playlist-editor]' : '[data-smart-form]')
        .exists(),
    ).toBe(true)
    expect(calls('upsert_playlist')).toHaveLength(0)
    expect(body().find('[data-command-search]').exists()).toBe(false)
    expect(document.activeElement).toBe(
      body().get(
        query === 'New playlist' ? '[aria-label="New playlist name"]' : '[data-smart-name]',
      ).element,
    )
  },
)
it('handles a Library handoff, blocks requests during a dialog, and removes listeners on unmount', async () => {
  const request = { payload: { requestId: 'request', sourceLabel: 'mini-player' } }
  events.get('open-command-palette')?.(request)
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(true)
  events.get('open-command-palette')?.(request)
  await flush()
  expect(
    vi.mocked(emitTo).mock.calls.filter((call) => call[1] === 'command-palette-ready'),
  ).toHaveLength(2)
  await body().get('[data-command-search]').trigger('keydown', { key: 'Escape' })
  await flush()
  await wrapper.get('[aria-label="Edit Smart"]').trigger('click')
  await flush()
  events.get('open-command-palette')?.(request)
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(false)
  wrapper.unmount()
  await flush()
  expect(events.size).toBe(0)
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }))
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(false)
})
it('refreshes time rules each minute only while Library is focused and stops after unmount', async () => {
  wrapper.unmount()
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
  try {
    library.playlists[0]!.smart!.rules = [
      { field: 'lastPlayedDays', operator: 'notWithin', value: 30 },
    ]
    wrapper = mount(App, { attachTo: document.body })
    await flush()
    const count = calls('inspect_library').length
    focusEvents.changed?.({ payload: false })
    await vi.advanceTimersByTimeAsync(60000)
    await flush()
    expect(calls('inspect_library')).toHaveLength(count)
    focusEvents.changed?.({ payload: true })
    await flush()
    library = {
      ...library,
      playlists: library.playlists.map((p) => (p.id === 's' ? { ...p, trackIds: ['b'] } : p)),
    }
    await vi.advanceTimersByTimeAsync(60000)
    await flush()
    expect(calls('inspect_library').length).toBeGreaterThan(count)
    await wrapper.get('[data-playlist-id="s"]').trigger('click')
    expect(wrapper.findAll('[data-track-id]').map((t) => t.attributes('data-track-id'))).toEqual([
      'b',
    ])
    wrapper.unmount()
    const after = calls('inspect_library').length
    await vi.advanceTimersByTimeAsync(60000)
    expect(calls('inspect_library')).toHaveLength(after)
  } finally {
    vi.useRealTimers()
  }
})
it.each(['mini', 'queue', 'artwork', 'settings', 'import'])(
  'hands Command-K from %s to Library with no selected-track payload',
  async (view) => {
    wrapper.unmount()
    await flush()
    window.history.replaceState({}, '', `/?view=${view}`)
    wrapper = mount(App, { attachTo: document.body })
    await flush()
    vi.mocked(emitTo).mockImplementation(async (_target, event, payload) => {
      if (event === 'open-command-palette') events.get('command-palette-ready')?.({ payload })
    })
    const shortcut = new KeyboardEvent('keydown', {
      key: 'k',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    })
    window.dispatchEvent(shortcut)
    await flush()
    expect(shortcut.defaultPrevented).toBe(true)
    expect(calls('show_app_window').slice(-1)[0]).toEqual([
      'show_app_window',
      { surface: 'library' },
    ])
    const request = vi.mocked(emitTo).mock.calls.find((call) => call[1] === 'open-command-palette')!
    expect(request[0]).toBe('main')
    expect(Object.keys(request[2] as object).sort()).toEqual(['requestId', 'sourceLabel'])
    expect(events.has('command-palette-ready')).toBe(false)
    expect(body().find('[data-command-search]').exists()).toBe(false)
  },
)
it('keeps selection and drafts during a pending owner mutation and drains library events afterwards', async () => {
  await wrapper.get('[data-playlist-id="s"]').trigger('click')
  await wrapper.get('[data-track-id="b"]').trigger('click')
  await wrapper.get('[aria-label="Edit Smart"]').trigger('click')
  await flush()
  let resolve!: (value: LibrarySnapshot) => void
  overrides.set(
    'upsert_playlist',
    () =>
      new Promise((r) => {
        resolve = r
      }),
  )
  await body().get('[data-smart-name]').setValue('Saved smart')
  await body().get('[data-smart-form]').trigger('submit')
  await flush()
  const count = calls('inspect_library').length
  library = {
    ...library,
    playlists: library.playlists.map((p) =>
      p.id === 's' ? { ...p, name: 'Saved smart', trackIds: ['b'] } : p,
    ),
  }
  events.get('library-updated')?.({ payload: null })
  await flush()
  expect(calls('inspect_library')).toHaveLength(count)
  expect(body().find('[data-smart-form]').exists()).toBe(true)
  resolve(library)
  await flush()
  expect(calls('inspect_library').length).toBeGreaterThan(count)
  expect(body().find('[data-smart-form]').exists()).toBe(false)
  expect(wrapper.get('[data-track-id="b"]').attributes('data-selected')).toBe('true')
  expect(wrapper.find('[data-track-id="a"]').exists()).toBe(false)
})
it('honors a handoff received before the async Library view mounts', async () => {
  wrapper.unmount()
  await flush()
  const { default: LibraryWindow } = await import('@/components/LibraryWindow.vue')
  wrapper = mount(LibraryWindow, {
    attachTo: document.body,
    props: {
      tracks: library.tracks,
      playlists: library.playlists,
      isUpdating: false,
      paletteRequest: 1,
    },
  })
  await flush()
  expect(body().find('[data-command-search]').exists()).toBe(true)
  expect(document.activeElement).toBe(body().get('[data-command-search]').element)
})
