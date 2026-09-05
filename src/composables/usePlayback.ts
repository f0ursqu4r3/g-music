import { computed, getCurrentScope, onScopeDispose, ref, shallowRef } from 'vue'

import {
  playbackApi,
  type TrackMetadataUpdate,
  type ImportProgress,
  type LibrarySnapshot,
  type MetadataRefreshSnapshot,
  type PlaybackSnapshot,
  type PlaybackTransport,
  type Playlist,
} from '@/api'

type RequiredClientMethods =
  | 'inspect'
  | 'play'
  | 'pause'
  | 'previous'
  | 'next'
  | 'seek'
  | 'setVolume'
  | 'moveQueueItem'
  | 'playTrack'
  | 'importYouTubeUrls'
export type PlaybackClient = Pick<typeof playbackApi, RequiredClientMethods> &
  Partial<Omit<typeof playbackApi, RequiredClientMethods>>

function readErrorMessage(error: unknown): string {
  if (typeof error === 'string' && error.trim()) {
    return error
  }

  if (error instanceof Error) {
    return error.message
  }

  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  ) {
    return error.message
  }

  return 'The playback command failed.'
}

export function usePlayback(client: PlaybackClient = playbackApi) {
  const library = shallowRef<LibrarySnapshot | null>(null)
  const queue = shallowRef<PlaybackSnapshot['queue']>([])
  const playbackOrder = shallowRef<string[]>([])
  const transport = ref<PlaybackTransport | null>(null)
  const snapshot = computed<PlaybackSnapshot | null>(() => {
    if (!transport.value) {
      return null
    }

    return {
      ...transport.value,
      ...(playbackOrder.value.length > 0 ? { playbackOrder: playbackOrder.value } : {}),
      queue: queue.value,
    }
  })
  const errorMessage = ref('')
  const errorCode = ref('')
  const importProgress = ref<ImportProgress | null>(null)
  const metadataRefreshes = ref<MetadataRefreshSnapshot>({
    completedTracks: 0,
    jobs: [],
    totalTracks: 0,
  })
  const isUpdating = ref(false)
  const isStarting = ref(false)
  let isSyncing = false
  let isUpdatingVolume = false
  let lastUnmutedVolume = 50
  let pendingVolume: number | undefined
  let alive = true
  let generation = 0
  let metadataGeneration = 0
  let importGeneration = 0
  let importInspection = 0
  let pendingRefresh = false
  let volumeIntent: number | undefined
  let retryAction: (() => Promise<unknown>) | undefined
  if (getCurrentScope())
    onScopeDispose(() => {
      alive = false
      generation++
    })

  function reportError(error: unknown, retry?: () => Promise<unknown>): void {
    if (!alive) return
    errorMessage.value = readErrorMessage(error)
    errorCode.value =
      typeof error === 'object' &&
      error !== null &&
      'code' in error &&
      typeof error.code === 'string'
        ? error.code
        : ''
    retryAction = retry
  }

  async function retry(): Promise<void> {
    const action = retryAction
    errorMessage.value = ''
    if (action) await action()
    else await refresh()
  }

  function finishUpdate(): void {
    isUpdating.value = false
    if (alive && pendingRefresh && !isUpdatingVolume) {
      pendingRefresh = false
      void refresh()
    }
  }
  const isImporting = computed(() => {
    const phase = importProgress.value?.phase
    return phase === 'started' || phase === 'resolving' || phase === 'merging'
  })

  function applyTransport(nextTransport: PlaybackTransport): void {
    if (!alive) return
    transport.value = {
      currentItem: nextTransport.currentItem,
      positionMs: nextTransport.positionMs,
      ...(nextTransport.repeatMode ? { repeatMode: nextTransport.repeatMode } : {}),
      status: nextTransport.status,
      ...(typeof nextTransport.shuffleEnabled === 'boolean'
        ? { shuffleEnabled: nextTransport.shuffleEnabled }
        : {}),
      volumePercent: isUpdatingVolume
        ? (volumeIntent ?? nextTransport.volumePercent)
        : nextTransport.volumePercent,
    }
    if (!isUpdatingVolume && nextTransport.volumePercent > 0) {
      lastUnmutedVolume = nextTransport.volumePercent
    }
  }

  function applySnapshot(nextSnapshot: PlaybackSnapshot): void {
    if (!alive) return
    generation++
    playbackOrder.value = nextSnapshot.playbackOrder ?? []
    queue.value = nextSnapshot.queue
    applyTransport(nextSnapshot)
  }

  function applyTransportEvent(nextTransport: PlaybackTransport): void {
    if (!alive) return
    generation++
    applyTransport(nextTransport)
  }

  function updateVolumeLocally(volumePercent: number): void {
    if (!transport.value) {
      return
    }

    transport.value = { ...transport.value, volumePercent }
  }

  async function execute(action: () => Promise<PlaybackSnapshot>): Promise<boolean> {
    if (!alive || isUpdating.value) {
      return false
    }

    isUpdating.value = true
    const operation = ++generation
    errorMessage.value = ''

    try {
      const result = await action()
      if (alive && operation === generation) applySnapshot(result)
      return true
    } catch (error) {
      reportError(error, () => execute(action))
      return false
    } finally {
      finishUpdate()
    }
  }

  /** Run a sequential batch of single-id commands inside one isUpdating guard. */
  async function executeBatch(
    ids: string[],
    action: (id: string) => Promise<PlaybackSnapshot>,
  ): Promise<void> {
    if (!alive || isUpdating.value || ids.length === 0) {
      return
    }

    isUpdating.value = true
    errorMessage.value = ''

    try {
      for (const id of ids) {
        if (!alive) break
        const operation = ++generation
        const result = await action(id)
        if (alive && operation === generation) applySnapshot(result)
      }
    } catch (error) {
      reportError(error)
    } finally {
      finishUpdate()
    }
  }

  async function startPlayback(action: () => Promise<PlaybackSnapshot>): Promise<void> {
    if (isUpdating.value) {
      return
    }

    isStarting.value = true
    try {
      if (!(await execute(action)) && alive) retryAction = () => startPlayback(action)
    } finally {
      isStarting.value = false
    }
  }

  async function refresh(): Promise<void> {
    if (!alive) return
    if (isUpdating.value || isUpdatingVolume) {
      pendingRefresh = true
      return
    }

    isUpdating.value = true
    const operation = ++generation
    try {
      if (client.inspectLibrary) {
        const [nextLibrary, nextSnapshot] = await Promise.all([
          client.inspectLibrary(),
          client.inspect(),
        ])
        if (alive && !pendingRefresh) library.value = nextLibrary
        if (alive && operation === generation) applySnapshot(nextSnapshot)
      } else {
        const result = await client.inspect()
        if (alive && operation === generation) applySnapshot(result)
      }
    } catch (error) {
      reportError(error, refresh)
    } finally {
      finishUpdate()
    }
    await refreshMetadataRefreshes()
  }

  // App must subscribe before initialization. Any event or local import intent
  // makes the initial snapshot obsolete, even if it arrived before this read.
  async function initializeImportProgress(): Promise<void> {
    if (!alive || !client.inspectImportProgress || importGeneration > 0) return
    const operation = ++importInspection
    try {
      const result = await client.inspectImportProgress()
      if (alive && importGeneration === 0 && operation === importInspection)
        importProgress.value = result
    } catch (error) {
      if (alive && importGeneration === 0 && operation === importInspection)
        reportError(error, initializeImportProgress)
    }
  }

  async function initialize(): Promise<void> {
    await Promise.all([refresh(), initializeImportProgress()])
  }

  async function refreshMetadataRefreshes(): Promise<void> {
    if (!alive || !client.inspectMetadataRefreshes) {
      return
    }

    const operation = ++metadataGeneration
    try {
      const result = await client.inspectMetadataRefreshes()
      if (alive && operation === metadataGeneration) metadataRefreshes.value = result
    } catch (error) {
      if (operation === metadataGeneration) reportError(error, refreshMetadataRefreshes)
    }
  }

  async function sync(): Promise<void> {
    if (!alive || isSyncing || isUpdating.value || isUpdatingVolume) {
      return
    }

    isSyncing = true
    const operation = generation
    try {
      if (client.inspectTransport) {
        const result = await client.inspectTransport()
        if (alive && operation === generation) applyTransport(result)
      } else {
        const result = await client.inspect()
        if (alive && operation === generation) applySnapshot(result)
      }
    } catch (error) {
      if (operation === generation) reportError(error, sync)
    } finally {
      isSyncing = false
    }
  }

  async function importYouTubeUrls(urls: string[]): Promise<void> {
    if (!alive || isImporting.value) {
      return
    }

    importGeneration++
    errorMessage.value = ''
    importProgress.value = {
      completedSources: 0,
      importedTracks: 0,
      message: 'Queueing import…',
      phase: 'started',
      runId: 0,
      skippedMemberOnly: 0,
      totalSources: urls.length,
    }

    try {
      await client.importYouTubeUrls(urls)
    } catch (error) {
      if (!alive) return
      const message = readErrorMessage(error)
      reportError(error, () => importYouTubeUrls(urls))
      importProgress.value = {
        completedSources: importProgress.value?.completedSources ?? 0,
        importedTracks: importProgress.value?.importedTracks ?? 0,
        message,
        phase: 'failed',
        runId: importProgress.value?.runId ?? 0,
        skippedMemberOnly: importProgress.value?.skippedMemberOnly ?? 0,
        totalSources: importProgress.value?.totalSources ?? urls.length,
      }
    }
  }

  function updateImportProgress(progress: ImportProgress): void {
    if (!alive) return
    importGeneration++
    if (importProgress.value && progress.runId < importProgress.value.runId) return
    importProgress.value = progress
    if (progress.phase === 'failed') {
      reportError({ message: progress.message })
    }
    if (
      progress.phase === 'merging' ||
      progress.phase === 'completed' ||
      progress.phase === 'cancelled'
    ) {
      void refresh()
    }
  }

  function updateMetadataRefreshes(progress: MetadataRefreshSnapshot): void {
    if (!alive) return
    metadataGeneration++
    metadataRefreshes.value = progress
    if (progress.jobs.some((job) => job.state === 'completed')) {
      void refresh()
    }
  }

  async function toggle(): Promise<void> {
    if (snapshot.value?.status === 'playing') {
      await execute(client.pause)
      return
    }

    await startPlayback(client.play)
  }

  async function retryPlayback(): Promise<void> {
    await startPlayback(client.play)
  }

  async function previous(): Promise<void> {
    await execute(client.previous)
  }

  async function next(): Promise<void> {
    await execute(client.next)
  }

  async function toggleShuffle(): Promise<void> {
    if (client.toggleShuffle) {
      await execute(client.toggleShuffle)
    }
  }

  async function cycleRepeatMode(): Promise<void> {
    if (client.cycleRepeatMode) {
      await execute(client.cycleRepeatMode)
    }
  }

  async function playTrack(id: string, queueIds?: string[]): Promise<void> {
    await startPlayback(() => (queueIds ? client.playTrack(id, queueIds) : client.playTrack(id)))
  }

  async function playNext(id: string | string[]): Promise<void> {
    if (!client.playNext) return
    const ids = Array.isArray(id) ? id : [id]
    await executeBatch(ids, (singleId) => client.playNext!(singleId))
  }

  async function addToQueue(id: string | string[]): Promise<void> {
    if (!client.addToQueue) return
    const ids = Array.isArray(id) ? id : [id]
    await executeBatch(ids, (singleId) => client.addToQueue!(singleId))
  }

  async function mutateLibrary(action: () => Promise<LibrarySnapshot>): Promise<void> {
    if (!alive) return
    if (isUpdating.value) throw new Error('Another update is in progress. Try again.')
    isUpdating.value = true
    errorMessage.value = ''
    try {
      const result = await action()
      if (alive) library.value = result
    } catch (error) {
      reportError(error)
      throw new Error(readErrorMessage(error))
    } finally {
      finishUpdate()
    }
  }

  async function updateTracksMetadata(updates: TrackMetadataUpdate[]): Promise<void> {
    if (!client.updateTracksMetadata) throw new Error('Metadata editing is unavailable.')
    if (updates.length) await mutateLibrary(() => client.updateTracksMetadata!(updates))
  }

  async function seek(positionMs: number): Promise<void> {
    await execute(() => client.seek(positionMs))
  }

  async function toggleFavorite(id: string | string[]): Promise<void> {
    if (!client.toggleFavorite) return
    const ids = Array.isArray(id) ? [...new Set(id)] : [id]
    if (!ids.length) return
    try {
      await mutateLibrary(async () => {
        let result!: LibrarySnapshot
        for (const singleId of ids) {
          if (!alive) break
          result = await client.toggleFavorite!(singleId)
          if (alive) library.value = result
        }
        return result
      })
    } catch {
      /* The error is visible. A partial toggle batch cannot safely be retried. */
    }
  }

  async function upsertPlaylist(playlist: Playlist): Promise<void> {
    if (!client.upsertPlaylist) throw new Error('Playlist editing is unavailable.')
    await mutateLibrary(() => client.upsertPlaylist!(playlist))
  }

  async function reorderPlaylists(playlistIds: string[]): Promise<void> {
    if (!client.reorderPlaylists) return
    try {
      await mutateLibrary(() => client.reorderPlaylists!(playlistIds))
    } catch {
      /* Visible error. */
    }
  }

  async function deletePlaylist(id: string): Promise<void> {
    if (!client.deletePlaylist) throw new Error('Playlist editing is unavailable.')
    await mutateLibrary(() => client.deletePlaylist!(id))
  }

  async function removeTracks(ids: string[]): Promise<void> {
    if (!client.removeTracks) return
    try {
      await mutateLibrary(() => client.removeTracks!(ids))
    } catch {
      /* Visible error. */
    }
  }

  async function resetTrackMetadata(ids: string[]): Promise<void> {
    if (!client.resetTrackMetadata) throw new Error('Metadata reset is unavailable.')
    await mutateLibrary(() => client.resetTrackMetadata!(ids))
  }

  async function clearQueue(): Promise<void> {
    if (!client.clearQueue) throw new Error('Queue clearing is unavailable.')
    if (!(await execute(client.clearQueue)))
      throw new Error(errorMessage.value || 'Another update is in progress. Try again.')
  }

  const isCancelling = ref(false)
  async function cancelYouTubeImport(runId: number): Promise<void> {
    if (!alive || !client.cancelYouTubeImport || isCancelling.value) return
    isCancelling.value = true
    try {
      await client.cancelYouTubeImport(runId)
    } catch (error) {
      reportError(error, () => cancelYouTubeImport(runId))
    } finally {
      isCancelling.value = false
    }
  }

  const isRetryingMetadata = ref(false)
  async function retryMetadataRefreshes(): Promise<void> {
    if (!alive || !client.retryMetadataRefreshes || isRetryingMetadata.value) return
    isRetryingMetadata.value = true
    const operation = ++metadataGeneration
    try {
      const result = await client.retryMetadataRefreshes()
      if (alive && operation === metadataGeneration) metadataRefreshes.value = result
    } catch (error) {
      reportError(error, retryMetadataRefreshes)
    } finally {
      isRetryingMetadata.value = false
    }
  }

  async function setVolume(volumePercent: number): Promise<void> {
    if (!alive || (isUpdating.value && !isUpdatingVolume)) {
      return
    }

    const nextVolume = Math.max(0, Math.min(100, Math.round(volumePercent)))
    if (nextVolume > 0) {
      lastUnmutedVolume = nextVolume
    }
    pendingVolume = nextVolume
    volumeIntent = nextVolume
    generation++
    updateVolumeLocally(nextVolume)

    if (isUpdatingVolume) {
      return
    }

    isUpdatingVolume = true
    errorMessage.value = ''
    try {
      while (alive && pendingVolume !== undefined) {
        const volume = pendingVolume
        pendingVolume = undefined
        await client.setVolume(volume)
      }
    } catch (error) {
      const failedVolume = volumeIntent ?? nextVolume
      pendingVolume = undefined
      reportError(error, () => setVolume(failedVolume))
    } finally {
      isUpdatingVolume = false
      volumeIntent = undefined
      if (pendingRefresh && !isUpdating.value) {
        pendingRefresh = false
        void refresh()
      }
    }
  }

  async function toggleMute(): Promise<void> {
    const currentVolume = snapshot.value?.volumePercent ?? 0
    if (currentVolume > 0) {
      lastUnmutedVolume = currentVolume
      await setVolume(0)
      return
    }

    await setVolume(lastUnmutedVolume)
  }

  async function moveQueueItem(from: number, to: number): Promise<void> {
    await execute(() => client.moveQueueItem(from, to))
  }

  async function removeQueueItem(index: number): Promise<void> {
    if (client.removeQueueItem) {
      await execute(() => client.removeQueueItem!(index))
    }
  }

  return {
    initialize,
    addToQueue,
    cancelYouTubeImport,
    clearQueue,
    resetTrackMetadata,
    retryMetadataRefreshes,
    isRetryingMetadata,
    isCancelling,
    applySnapshot,
    applyTransportEvent,
    retryPlayback,
    cycleRepeatMode,
    deletePlaylist,
    errorMessage,
    errorCode,
    importProgress,
    isImporting,
    isStarting,
    isUpdating,
    library,
    metadataRefreshes,
    importYouTubeUrls,
    moveQueueItem,
    next,
    playNext,
    playTrack,
    previous,
    refresh,
    reportError,
    retry,
    refreshMetadataRefreshes,
    reorderPlaylists,
    removeQueueItem,
    removeTracks,
    seek,
    setVolume,
    snapshot,
    sync,
    toggle,
    toggleFavorite,
    toggleShuffle,
    toggleMute,
    transport,
    updateTracksMetadata,
    upsertPlaylist,
    updateImportProgress,
    updateMetadataRefreshes,
  }
}
