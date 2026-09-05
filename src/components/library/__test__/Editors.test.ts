import { DOMWrapper, flushPromises, mount } from '@vue/test-utils'
const body = () => new DOMWrapper(document.body)
import { afterEach, describe, expect, it } from 'vitest'
import type { MediaItem, Playlist, TrackMetadataUpdate } from '@/api'
import LibraryMetadataEditor from '../LibraryMetadataEditor.vue'
import PlaylistEditor from '../PlaylistEditor.vue'

const tracks: MediaItem[] = [
  {
    id: 'a',
    title: 'Alpha',
    artist: 'Artist',
    album: 'One',
    label: 'First',
    genres: ['Jazz'],
    durationMs: 10,
  },
  {
    id: 'b',
    title: 'Beta',
    artist: 'Artist',
    album: 'Two',
    label: 'Second',
    genres: ['Rock'],
    durationMs: 20,
  },
  { id: 'c', title: 'Gamma', artist: 'Artist', album: 'Three', durationMs: 30 },
]
afterEach(() => {
  document.body.innerHTML = ''
})

describe('metadata drafts', () => {
  it('preserves newer provider fields that arrive while the editor stays open', async () => {
    let stored = { ...tracks[0]! }
    const wrapper = mount(LibraryMetadataEditor, {
      attachTo: document.body,
      props: {
        target: { kind: 'track', name: 'Alpha', tracks: [{ ...stored }] },
        saveAction: async (updates) => {
          stored = { ...stored, ...updates[0]!.metadata }
        },
      },
    })
    await flushPromises()
    stored.album = 'New provider album'
    stored.label = 'New provider label'
    await body().get('[data-metadata-field="title"]').setValue('User title')
    await body().get('form').trigger('submit')
    await flushPromises()
    expect(stored.title).toBe('User title')
    expect(stored.album).toBe('New provider album')
    expect(stored.label).toBe('New provider label')
    wrapper.unmount()
  })
  it('renders the modal outside the window stacking layer', async () => {
    const surface = document.createElement('main')
    document.body.append(surface)
    const wrapper = mount(LibraryMetadataEditor, {
      attachTo: surface,
      props: { target: { kind: 'track', name: 'Alpha', tracks: [tracks[0]!] } },
    })
    await flushPromises()
    const dialog = document.querySelector('[role="dialog"]')
    expect(dialog).not.toBeNull()
    expect(surface.contains(dialog)).toBe(false)
    wrapper.unmount()
  })
  it('shows mixed fields and saves only dirty values while preserving each hidden value', async () => {
    let library = structuredClone(tracks.slice(0, 2))
    const saveAction = async (updates: TrackMetadataUpdate[]) => {
      library = library.map((track) => ({
        ...track,
        ...updates.find((update) => update.id === track.id)?.metadata,
      }))
    }
    const wrapper = mount(LibraryMetadataEditor, {
      attachTo: document.body,
      props: {
        target: { kind: 'artist', name: 'Artist', tracks: tracks.slice(0, 2) },
        saveAction,
      },
    })
    await flushPromises()
    expect(body().get('[data-metadata-field="label"]').attributes('placeholder')).toBe(
      'Mixed values',
    )
    await body().get('[data-metadata-field="artist"]').setValue('Renamed')
    await body().get('form').trigger('submit')
    await flushPromises()
    expect(library.map((track) => [track.artist, track.album, track.label, track.genres])).toEqual([
      ['Renamed', 'One', 'First', ['Jazz']],
      ['Renamed', 'Two', 'Second', ['Rock']],
    ])
    wrapper.unmount()
  })
  it('does not submit unchanged metadata and supports explicit optional field clears', async () => {
    const writes: TrackMetadataUpdate[][] = []
    const wrapper = mount(LibraryMetadataEditor, {
      attachTo: document.body,
      props: {
        target: { kind: 'album', name: 'Album', tracks: tracks.slice(0, 2) },
        saveAction: async (updates: TrackMetadataUpdate[]) => {
          writes.push(updates)
        },
      },
    })
    await flushPromises()
    await body().get('form').trigger('submit')
    expect(writes).toEqual([])
    await body().get('[data-metadata-clear="label"]').trigger('click')
    await body().get('form').trigger('submit')
    await flushPromises()
    expect(writes[0]?.map((update) => update.metadata.label)).toEqual([null, null])
    expect(writes[0]?.map((update) => update.metadata)).toEqual([{ label: null }, { label: null }])
    wrapper.unmount()
  })
  it('keeps a failed draft, rejects required clears, and resets only after explicit discard', async () => {
    let resetIds: string[] = []
    const wrapper = mount(LibraryMetadataEditor, {
      attachTo: document.body,
      props: {
        target: { kind: 'track', name: 'Alpha', tracks: [tracks[0]!] },
        saveAction: async () => {
          throw new Error('Disk is full')
        },
        resetAction: async (ids: string[]) => {
          resetIds = ids
        },
      },
    })
    await flushPromises()
    await body().get('[data-metadata-field="title"]').setValue('  ')
    await body().get('form').trigger('submit')
    expect(body().get('[role="alert"]').text()).toContain('Track title is required')
    await body().get('[data-metadata-field="title"]').setValue('Draft')
    await body().get('form').trigger('submit')
    await flushPromises()
    expect(body().get('[role="alert"]').text()).toContain('Disk is full')
    expect((body().get('[data-metadata-field="title"]').element as HTMLInputElement).value).toBe(
      'Draft',
    )
    await body().get('[data-metadata-reset]').trigger('click')
    expect(resetIds).toEqual([])
    expect(body().get('[data-confirm-metadata-reset]').text()).toContain('Discard draft and reset')
    await body().get('[data-cancel-metadata-reset]').trigger('click')
    expect((body().get('[data-metadata-field="title"]').element as HTMLInputElement).value).toBe(
      'Draft',
    )
    await body().get('[data-metadata-reset]').trigger('click')
    await body().get('[data-confirm-metadata-reset]').trigger('click')
    await flushPromises()
    expect(resetIds).toEqual(['a'])
    wrapper.unmount()
  })
})

describe('playlist drafts', () => {
  it('preserves original order and unknown IDs, removes in place, and appends new selections', async () => {
    let stored: Playlist = {
      id: 'p',
      name: 'Mix',
      trackIds: ['b', 'missing', 'a'],
    }
    const wrapper = mount(PlaylistEditor, {
      attachTo: document.body,
      props: {
        playlist: stored,
        tracks,
        saveAction: async (playlist: Playlist) => {
          stored = playlist
        },
      },
    })
    await flushPromises()
    await body().get('[data-playlist-field="name"]').setValue('Renamed')
    await body().get('form').trigger('submit')
    await flushPromises()
    expect(stored.trackIds).toEqual(['b', 'missing', 'a'])
    await body().get('[data-playlist-track="a"] input').setValue(false)
    await body().get('[data-playlist-track="c"] input').setValue(true)
    await body().get('form').trigger('submit')
    await flushPromises()
    expect(stored.trackIds).toEqual(['b', 'missing', 'c'])
    wrapper.unmount()
  })
  it('keeps failed playlist edits and focus inside the dialog', async () => {
    const wrapper = mount(PlaylistEditor, {
      attachTo: document.body,
      props: {
        playlist: { id: 'p', name: 'Mix', trackIds: ['b'] },
        tracks,
        saveAction: async () => {
          throw new Error('Retry after freeing disk space')
        },
      },
    })
    await flushPromises()
    await body().get('[data-playlist-field="name"]').setValue('Draft')
    await body().get('form').trigger('submit')
    await flushPromises()
    expect(body().get('[role="alert"]').text()).toContain('Retry after freeing disk space')
    expect((body().get('[data-playlist-field="name"]').element as HTMLInputElement).value).toBe(
      'Draft',
    )
    const save = body().get('[data-playlist-editor-save]')
    ;(save.element as HTMLElement).focus()
    await save.trigger('keydown', { key: 'Tab' })
    expect(body().get('[role="dialog"]').element.contains(document.activeElement)).toBe(true)
    await body().get('[role="dialog"]').trigger('keydown', { key: 'Escape' })
    expect(wrapper.emitted('cancel')).toBeDefined()
    wrapper.unmount()
  })
})
