import { ref } from "vue";

import { playbackApi, type PlaybackSnapshot } from "@/api";

export type PlaybackClient = typeof playbackApi;

export function usePlayback(client: PlaybackClient = playbackApi) {
  const snapshot = ref<PlaybackSnapshot | null>(null);
  const errorMessage = ref("");
  const isUpdating = ref(false);

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
      errorMessage.value =
        error instanceof Error ? error.message : "The playback command failed.";
    } finally {
      isUpdating.value = false;
    }
  }

  async function refresh(): Promise<void> {
    await execute(client.inspect);
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
    moveQueueItem,
    next,
    previous,
    refresh,
    seek,
    setVolume,
    snapshot,
    toggle,
  };
}
