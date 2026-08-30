import { computed, ref, shallowRef } from "vue";

import {
  playbackApi,
  type ImportProgress,
  type LibrarySnapshot,
  type MetadataRefreshSnapshot,
  type PlaybackSnapshot,
  type PlaybackTransport,
} from "@/api";

export type PlaybackClient = Omit<
  typeof playbackApi,
  "inspectLibrary" | "inspectMetadataRefreshes" | "inspectTransport"
> &
  Partial<
    Pick<
      typeof playbackApi,
      "inspectLibrary" | "inspectMetadataRefreshes" | "inspectTransport"
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
  const transport = ref<PlaybackTransport | null>(null);
  const snapshot = computed<PlaybackSnapshot | null>(() => {
    if (!library.value || !transport.value) {
      return null;
    }

    return { ...transport.value, queue: library.value.tracks };
  });
  const errorMessage = ref("");
  const importProgress = ref<ImportProgress | null>(null);
  const metadataRefreshes = ref<MetadataRefreshSnapshot>({
    completedTracks: 0,
    jobs: [],
    totalTracks: 0,
  });
  const isUpdating = ref(false);
  let isSyncing = false;
  const isImporting = computed(() => {
    const phase = importProgress.value?.phase;
    return phase === "started" || phase === "resolving" || phase === "merging";
  });

  function applySnapshot(nextSnapshot: PlaybackSnapshot): void {
    library.value = { tracks: nextSnapshot.queue };
    transport.value = {
      currentItem: nextSnapshot.currentItem,
      positionMs: nextSnapshot.positionMs,
      status: nextSnapshot.status,
      volumePercent: nextSnapshot.volumePercent,
    };
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
    if (isSyncing || isUpdating.value) {
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
    await execute(() =>
      snapshot.value?.status === "playing" ? client.pause() : client.play(),
    );
  }

  async function previous(): Promise<void> {
    await execute(client.previous);
  }

  async function next(): Promise<void> {
    await execute(client.next);
  }

  async function playTrack(id: string): Promise<void> {
    await execute(() => client.playTrack(id));
  }

  async function seek(positionMs: number): Promise<void> {
    await execute(() => client.seek(positionMs));
  }

  async function setVolume(volumePercent: number): Promise<void> {
    await execute(() => client.setVolume(volumePercent));
  }

  async function moveQueueItem(from: number, to: number): Promise<void> {
    await execute(() => client.moveQueueItem(from, to));
  }

  return {
    errorMessage,
    importProgress,
    isImporting,
    isUpdating,
    library,
    metadataRefreshes,
    importYouTubeUrls,
    moveQueueItem,
    next,
    playTrack,
    previous,
    refresh,
    refreshMetadataRefreshes,
    seek,
    setVolume,
    snapshot,
    sync,
    toggle,
    transport,
    updateImportProgress,
    updateMetadataRefreshes,
  };
}
