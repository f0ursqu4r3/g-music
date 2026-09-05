import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import type { ImportProgress, PlaybackSnapshot } from '@/api'
import { usePlayback, type PlaybackClient } from '../usePlayback'

const initial: PlaybackSnapshot = {
  status: 'paused',
  currentItem: null,
  positionMs: 1000,
  volumePercent: 70,
  queue: [],
}
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((r) => {
    resolve = r
  })
  return { promise, resolve }
}
function setup(extra: Partial<PlaybackClient> = {}) {
  const client = {
    inspect: vi.fn().mockResolvedValue(initial),
    inspectLibrary: vi.fn().mockResolvedValue({ tracks: [], playlists: [] }),
    inspectImportProgress: vi.fn().mockResolvedValue(null),
    play: vi.fn().mockResolvedValue(initial),
    pause: vi.fn(),
    previous: vi.fn(),
    next: vi.fn(),
    seek: vi.fn().mockResolvedValue({ ...initial, positionMs: 99000 }),
    setVolume: vi.fn().mockResolvedValue(undefined),
    moveQueueItem: vi.fn(),
    playTrack: vi.fn(),
    importYouTubeUrls: vi.fn(),
    ...extra,
  }
  let state!: ReturnType<typeof usePlayback>
  const wrapper = mount(
    defineComponent({
      setup() {
        state = usePlayback(client)
        return () =>
          h(
            'output',
            `${state.snapshot.value?.positionMs}/${state.snapshot.value?.volumePercent}/${state.library.value?.tracks.length}`,
          )
      },
    }),
  )
  return { client, state, wrapper }
}
describe('mounted playback race protection', () => {
  const activeImport: ImportProgress = {
    completedSources: 0,
    importedTracks: 2,
    message: 'Reading source',
    phase: 'resolving',
    runId: 84,
    skippedMemberOnly: 0,
    totalSources: 1,
  }
  it('keeps an event received before initialization over the initial inspection', async () => {
    const { state, wrapper } = setup()
    state.updateImportProgress(activeImport)
    await state.initialize()
    expect(state.importProgress.value).toEqual(activeImport)
    expect(state.isImporting.value).toBe(true)
    wrapper.unmount()
  })
  it('does not restore an active run after its completion event', async () => {
    const read = deferred<ImportProgress | null>()
    const { state, wrapper } = setup({
      inspectImportProgress: vi.fn().mockReturnValue(read.promise),
    })
    const pending = state.initialize()
    state.updateImportProgress({ ...activeImport, phase: 'completed' })
    read.resolve(activeImport)
    await pending
    expect(state.importProgress.value?.phase).toBe('completed')
    expect(state.isImporting.value).toBe(false)
    wrapper.unmount()
  })
  it('discards an import inspection after unmount', async () => {
    const read = deferred<ImportProgress | null>()
    const { state, wrapper } = setup({
      inspectImportProgress: vi.fn().mockReturnValue(read.promise),
    })
    const pending = state.initialize()
    wrapper.unmount()
    read.resolve(activeImport)
    await pending
    expect(state.importProgress.value).toBeNull()
  })
  it('retries a failed initial import inspection', async () => {
    const { state, wrapper, client } = setup({
      inspectImportProgress: vi
        .fn()
        .mockRejectedValueOnce(new Error('Import unavailable'))
        .mockResolvedValueOnce(activeImport),
    })
    await state.initialize()
    expect(state.errorMessage.value).toBe('Import unavailable')
    await state.retry()
    expect(client.inspectImportProgress).toHaveBeenCalledTimes(2)
    expect(state.importProgress.value).toEqual(activeImport)
    expect(state.errorMessage.value).toBe('')
    wrapper.unmount()
  })
  it('does not clear a local import when a pending initial inspection returns null', async () => {
    const read = deferred<ImportProgress | null>()
    const { state, wrapper } = setup({
      inspectImportProgress: vi.fn().mockReturnValue(read.promise),
    })
    const pending = state.initialize()
    await state.importYouTubeUrls(['https://youtu.be/fixture'])
    read.resolve(null)
    await pending
    expect(state.isImporting.value).toBe(true)
    expect(state.importProgress.value?.message).toBe('Queueing import…')
    wrapper.unmount()
  })
  it('keeps starting state visible while retrying a failed play', async () => {
    const play = deferred<PlaybackSnapshot>()
    const { state, wrapper } = setup({
      play: vi
        .fn()
        .mockRejectedValueOnce(new Error('Load failed'))
        .mockReturnValueOnce(play.promise),
    })
    await state.refresh()
    await state.toggle()
    const retry = state.retry()
    expect(state.isStarting.value).toBe(true)
    play.resolve(initial)
    await retry
    wrapper.unmount()
  })
  it('stops issuing the remaining batch after unmount', async () => {
    const first = deferred<PlaybackSnapshot>()
    const { state, wrapper } = setup({
      addToQueue: vi.fn().mockReturnValueOnce(first.promise),
    })
    const pending = state.addToQueue(['a', 'b'])
    wrapper.unmount()
    first.resolve(initial)
    await pending
    expect(state.snapshot.value).toBeNull()
  })
  it('retries the latest volume intent after a failed drag', async () => {
    let reject!: (error: Error) => void
    const failed = new Promise<void>((_, fail) => {
      reject = fail
    })
    const { state, client, wrapper } = setup({
      setVolume: vi.fn().mockReturnValueOnce(failed).mockResolvedValue(undefined),
    })
    await state.refresh()
    const pending = state.setVolume(20)
    await state.setVolume(80)
    reject(new Error('Volume failed'))
    await pending
    await state.retry()
    expect(client.setVolume).toHaveBeenLastCalledWith(80)
    wrapper.unmount()
  })
  it('keeps the unmuted volume when an old event arrives during mute', async () => {
    const volume = deferred<void>()
    const { state, client, wrapper } = setup({
      setVolume: vi.fn().mockReturnValueOnce(volume.promise).mockResolvedValue(undefined),
    })
    await state.refresh()
    const pending = state.toggleMute()
    state.applySnapshot({ ...initial, volumePercent: 30 })
    volume.resolve()
    await pending
    await state.toggleMute()
    expect(client.setVolume).toHaveBeenLastCalledWith(70)
    wrapper.unmount()
  })
  it('keeps newer metadata progress over an old inspect response', async () => {
    const read = deferred<{
      completedTracks: number
      totalTracks: number
      jobs: []
    }>()
    const { state, wrapper } = setup({
      inspectMetadataRefreshes: vi.fn().mockReturnValue(read.promise),
    })
    const pending = state.refreshMetadataRefreshes()
    state.updateMetadataRefreshes({
      completedTracks: 0,
      totalTracks: 1,
      jobs: [{ state: 'refreshing', trackId: 'a', title: 'A', message: 'Loading' }],
    })
    read.resolve({ completedTracks: 0, totalTracks: 0, jobs: [] })
    await pending
    expect(state.metadataRefreshes.value.jobs).toHaveLength(1)
    wrapper.unmount()
  })
  it('discards a poll started before a seek', async () => {
    const poll = deferred<PlaybackSnapshot>()
    const { state, wrapper } = setup({
      inspectTransport: vi.fn().mockReturnValue(poll.promise),
    })
    await state.refresh()
    const pending = state.sync()
    await state.seek(99000)
    poll.resolve(initial)
    await pending
    await flushPromises()
    expect(wrapper.text()).toMatch(/^99000\//)
    wrapper.unmount()
  })
  it('keeps a newer semantic event over an older command response', async () => {
    const command = deferred<PlaybackSnapshot>()
    const { state, wrapper } = setup({
      seek: vi.fn().mockReturnValue(command.promise),
    })
    await state.refresh()
    const pending = state.seek(99000)
    state.applySnapshot({ ...initial, positionMs: 80000 })
    command.resolve({ ...initial, positionMs: 99000 })
    await pending
    await flushPromises()
    expect(wrapper.text()).toMatch(/^80000\//)
    wrapper.unmount()
  })
  it('retains the latest drag volume when a semantic event arrives', async () => {
    const volume = deferred<void>()
    const { state, wrapper, client } = setup({
      setVolume: vi.fn().mockReturnValueOnce(volume.promise).mockResolvedValue(undefined),
    })
    await state.refresh()
    const pending = state.setVolume(20)
    await state.setVolume(45)
    state.applySnapshot({ ...initial, positionMs: 80000 })
    await flushPromises()
    expect(wrapper.text()).toMatch(/^80000\/45/)
    volume.resolve()
    await pending
    expect(client.setVolume).toHaveBeenLastCalledWith(45)
    wrapper.unmount()
  })
  it('coalesces library invalidations while busy and replays them', async () => {
    const command = deferred<PlaybackSnapshot>()
    const { state, client, wrapper } = setup({
      seek: vi.fn().mockReturnValue(command.promise),
    })
    await state.refresh()
    const pending = state.seek(99000)
    void state.refresh()
    void state.refresh()
    command.resolve(initial)
    await pending
    await flushPromises()
    expect(client.inspectLibrary).toHaveBeenCalledTimes(2)
    wrapper.unmount()
  })
  it('does not apply an in-flight read after unmount', async () => {
    const poll = deferred<PlaybackSnapshot>()
    const { state, wrapper } = setup({
      inspectTransport: vi.fn().mockReturnValue(poll.promise),
    })
    await state.refresh()
    const pending = state.sync()
    wrapper.unmount()
    poll.resolve({ ...initial, positionMs: 999 })
    await pending
    expect(state.snapshot.value?.positionMs).toBe(1000)
  })
})
