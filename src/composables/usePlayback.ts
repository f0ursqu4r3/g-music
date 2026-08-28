import { ref } from "vue";

import { playbackApi, type PlaybackSnapshot } from "@/api";

export type PlaybackClient = typeof playbackApi;

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
  const snapshot = ref<PlaybackSnapshot | null>(null);
  const errorMessage = ref("");
  const isUpdating = ref(false);
  let isSyncing = false;

  async function execute(
    action: () => Promise<PlaybackSnapshot>,
  ): Promise<void> {
    if (isUpdating.value) {
      return;
    }

    isUpdating.value = true;
    errorMessage.value = "";

    try {
      snapshot.value = await action();
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isUpdating.value = false;
    }
  }

  async function refresh(): Promise<void> {
    await execute(client.inspect);
  }

  async function sync(): Promise<void> {
    if (isSyncing || isUpdating.value) {
      return;
    }

    isSyncing = true;
    try {
      snapshot.value = await client.inspect();
    } catch (error) {
      errorMessage.value = readErrorMessage(error);
    } finally {
      isSyncing = false;
    }
  }

  async function importYouTubeUrl(url: string): Promise<void> {
    await execute(() => client.importYouTubeUrl(url));
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
    isUpdating,
    importYouTubeUrl,
    moveQueueItem,
    next,
    playTrack,
    previous,
    refresh,
    seek,
    setVolume,
    snapshot,
    sync,
    toggle,
  };
}
