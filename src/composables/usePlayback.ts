import { computed, ref, shallowRef } from "vue";

import {
  playbackApi,
  type TrackMetadataUpdate,
  type ImportProgress,
  type LibrarySnapshot,
  type MetadataRefreshSnapshot,
  type PlaybackSnapshot,
  type PlaybackTransport,
  type Playlist,
} from "@/api";

export type PlaybackClient = Omit<
  typeof playbackApi,
  | "inspectLibrary"
  | "inspectMetadataRefreshes"
  | "inspectTransport"
  | "updateTracksMetadata"
  | "toggleFavorite"
  | "upsertPlaylist"
  | "reorderPlaylists"
  | "deletePlaylist"
  | "playNext"
  | "addToQueue"
  | "removeTracks"
  | "removeQueueItem"
  | "toggleShuffle"
  | "cycleRepeatMode"
> &
  Partial<
    Pick<
      typeof playbackApi,
      | "inspectLibrary"
      | "inspectMetadataRefreshes"
      | "inspectTransport"
      | "updateTracksMetadata"
      | "toggleFavorite"
      | "upsertPlaylist"
      | "reorderPlaylists"
      | "deletePlaylist"
      | "playNext"
      | "addToQueue"
      | "removeTracks"
      | "removeQueueItem"
      | "toggleShuffle"
      | "cycleRepeatMode"
    >
  >;

function readErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return "The playback command failed.";
}

export function usePlayback(client: PlaybackClient = playbackApi) {
  const library = shallowRef<LibrarySnapshot | null>(null);
  const queue = shallowRef<PlaybackSnapshot["queue"]>([]);
  const transport = ref<PlaybackTransport | null>(null);
  const snapshot = computed<PlaybackSnapshot | null>(() => {
    if (!transport.value) {
      return null;
    }

    return { ...transport.value, queue: queue.value };
  });
  const errorMessage = ref("");
  const importProgress = ref<ImportProgress | null>(null);
  const metadataRefreshes = ref<MetadataRefreshSnapshot>({
    completedTracks: 0,
    jobs: [],
    totalTracks: 0,
  });
  const isUpdating = ref(false);
  const isStarting = ref(false);
  let isSyncing = false;
  let isUpdatingVolume = false;
  let lastUnmutedVolume = 50;
  let pendingVolume: number | undefined;
  const isImporting = computed(() => {
    const phase = importProgress.value?.phase;
    return phase === "started" || phase === "resolving" || phase === "merging";
  });

  function applyTransport(nextTransport: PlaybackTransport): void {
    transport.value = {
      currentItem: nextTransport.currentItem,
      positionMs: nextTransport.positionMs,
      ...(nextTransport.repeatMode
        ? { repeatMode: nextTransport.repeatMode }
        : {}),
      status: nextTransport.status,
      ...(typeof nextTransport.shuffleEnabled === "boolean"
        ? { shuffleEnabled: nextTransport.shuffleEnabled }
        : {}),
      volumePercent: nextTransport.volumePercent,
    };
    if (nextTransport.volumePercent > 0) {
      lastUnmutedVolume = nextTransport.volumePercent;
    }
  }

  function applySnapshot(nextSnapshot: PlaybackSnapshot): void {
    queue.value = nextSnapshot.queue;
    applyTransport(nextSnapshot);
  }

  function updateVolumeLocally(volumePercent: number): void {
    if (!transport.value) {
      return;
    }

    transport.value = { ...transport.value, volumePercent };
  }

  async function execute(
    action: () => Promise<PlaybackSnapshot>,
  ): Promise<void> {
    if (isUpdating.value) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";

    try {
      applySnapshot(await action());
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function startPlayback(
    action: () => Promise<PlaybackSnapshot>,
  ): Promise<void> {
    if (isUpdating.value) {
      return;
    }

    isStarting.value = true;
    try {
      await execute(action);
    } finally {
      isStarting.value = false;
    }
  }

  async function refresh(): Promise<void> {
    if (isUpdating.value) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";
    try {
      if (client.inspectLibrary && client.inspectTransport) {
        const [nextLibrary, nextTransport] = await Promise.all([
          client.inspectLibrary(),
          client.inspectTransport(),
        ]);
        library.value = nextLibrary;
        transport.value = nextTransport;
      } else {
        applySnapshot(await client.inspect());
      }
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
    await refreshMetadataRefreshes();
  }

  async function refreshMetadataRefreshes(): Promise<void> {
    if (!client.inspectMetadataRefreshes) {
      return;
    }

    try {
      metadataRefreshes.value = await client.inspectMetadataRefreshes();
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    }
  }

  async function sync(): Promise<void> {
    if (isSyncing || isUpdating.value || isUpdatingVolume) {
      return;
    }

    isSyncing = true;
    try {
      if (client.inspectTransport) {
        transport.value = await client.inspectTransport();
      } else {
        applySnapshot(await client.inspect());
      }
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isSyncing = false;
    }
  }

  async function importYouTubeUrls(urls: string[]): Promise<void> {
    if (isImporting.value) {
      return;
    }

    errorMessage.value = "";
    importProgress.value = {
      completedSources: 0,
      importedTracks: 0,
      message: "Queueing import…",
      phase: "started",
      runId: 0,
      skippedMemberOnly: 0,
      totalSources: urls.length,
    };

    try {
      await client.importYouTubeUrls(urls);
    } catch (error) {
      const message = readErrorMessage(error);
      errorMessage.value = message;
      importProgress.value = {
        completedSources: importProgress.value?.completedSources ?? 0,
        importedTracks: importProgress.value?.importedTracks ?? 0,
        message,
        phase: "failed",
        runId: importProgress.value?.runId ?? 0,
        skippedMemberOnly: importProgress.value?.skippedMemberOnly ?? 0,
        totalSources: importProgress.value?.totalSources ?? urls.length,
      };
    }
  }

  function updateImportProgress(progress: ImportProgress): void {
    importProgress.value = progress;
    if (progress.phase === "failed") {
      errorMessage.value = progress.message;
    }
    if (progress.phase === "merging" || progress.phase === "completed") {
      void refresh();
    }
  }

  function updateMetadataRefreshes(progress: MetadataRefreshSnapshot): void {
    metadataRefreshes.value = progress;
    if (progress.jobs.some((job) => job.state === "completed")) {
      void refresh();
    }
  }

  async function toggle(): Promise<void> {
    if (snapshot.value?.status === "playing") {
      await execute(client.pause);
      return;
    }

    await startPlayback(client.play);
  }

  async function previous(): Promise<void> {
    await execute(client.previous);
  }

  async function next(): Promise<void> {
    await execute(client.next);
  }

  async function toggleShuffle(): Promise<void> {
    if (client.toggleShuffle) {
      await execute(client.toggleShuffle);
    }
  }

  async function cycleRepeatMode(): Promise<void> {
    if (client.cycleRepeatMode) {
      await execute(client.cycleRepeatMode);
    }
  }

  async function playTrack(id: string, queueIds?: string[]): Promise<void> {
    await startPlayback(() =>
      queueIds ? client.playTrack(id, queueIds) : client.playTrack(id),
    );
  }

  async function playNext(id: string): Promise<void> {
    if (client.playNext) {
      await execute(() => client.playNext!(id));
    }
  }

  async function addToQueue(id: string): Promise<void> {
    if (client.addToQueue) {
      await execute(() => client.addToQueue!(id));
    }
  }

  async function updateTracksMetadata(
    updates: TrackMetadataUpdate[],
  ): Promise<void> {
    if (isUpdating.value || !client.updateTracksMetadata) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";
    try {
      library.value = await client.updateTracksMetadata(updates);
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function seek(positionMs: number): Promise<void> {
    await execute(() => client.seek(positionMs));
  }

  async function toggleFavorite(id: string): Promise<void> {
    if (isUpdating.value || !client.toggleFavorite) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";
    try {
      library.value = await client.toggleFavorite(id);
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function upsertPlaylist(playlist: Playlist): Promise<void> {
    if (isUpdating.value || !client.upsertPlaylist) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";
    try {
      library.value = await client.upsertPlaylist(playlist);
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function reorderPlaylists(playlistIds: string[]): Promise<void> {
    if (isUpdating.value || !client.reorderPlaylists) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";
    try {
      library.value = await client.reorderPlaylists(playlistIds);
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function deletePlaylist(id: string): Promise<void> {
    if (isUpdating.value || !client.deletePlaylist) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";
    try {
      library.value = await client.deletePlaylist(id);
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function removeTracks(ids: string[]): Promise<void> {
    if (isUpdating.value || !client.removeTracks) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";
    try {
      library.value = await client.removeTracks(ids);
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function setVolume(volumePercent: number): Promise<void> {
    if (isUpdating.value && !isUpdatingVolume) {
      return;
    }

    const nextVolume = Math.max(0, Math.min(100, Math.round(volumePercent)));
    if (nextVolume > 0) {
      lastUnmutedVolume = nextVolume;
    }
    pendingVolume = nextVolume;
    updateVolumeLocally(nextVolume);

    if (isUpdatingVolume) {
      return;
    }

    isUpdatingVolume = true;
    errorMessage.value = "";
    try {
      while (pendingVolume !== undefined) {
        const volume = pendingVolume;
        pendingVolume = undefined;
        await client.setVolume(volume);
      }
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdatingVolume = false;
    }
  }

  async function toggleMute(): Promise<void> {
    const currentVolume = snapshot.value?.volumePercent ?? 0;
    if (currentVolume > 0) {
      lastUnmutedVolume = currentVolume;
      await setVolume(0);
      return;
    }

    await setVolume(lastUnmutedVolume);
  }

  async function moveQueueItem(from: number, to: number): Promise<void> {
    await execute(() => client.moveQueueItem(from, to));
  }

  async function removeQueueItem(index: number): Promise<void> {
    if (client.removeQueueItem) {
      await execute(() => client.removeQueueItem!(index));
    }
  }

  return {
    addToQueue,
    applySnapshot,
    cycleRepeatMode,
    deletePlaylist,
    errorMessage,
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
  };
}
