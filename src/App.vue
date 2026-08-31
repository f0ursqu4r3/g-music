<script setup lang="ts">
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";

import {
  type ImportProgress,
  type MetadataRefreshSnapshot,
  type PlaybackSnapshot,
  windowApi,
} from "@/api";
import type { ThemeName } from "@/lib/theme";
import { readTheme, themes } from "@/lib/theme";
import { resolveMockWindowView } from "@/lib/window-view";
import { playerDimensions } from "@/presentation";

import ArtworkWindow from "@/components/ArtworkWindow.vue";
import ImportWindow from "@/components/ImportWindow.vue";
import LibraryWindow from "@/components/LibraryWindow.vue";
import MiniWindow from "@/components/MiniWindow.vue";
import QueueWindow from "@/components/QueueWindow.vue";
import SettingsWindow from "@/components/SettingsWindow.vue";
import { usePlayback } from "@/composables/usePlayback";

const playback = usePlayback();
const view = resolveMockWindowView(window.location.search);
const queueExpanded = ref(false);
const theme = ref<ThemeName>(
  readTheme(window.localStorage.getItem("gmusic-theme")),
);
const windowError = ref("");
const isWindowFocused = ref(true);
let isMounted = false;
let unlistenWindowFocus: (() => void) | undefined;
let playbackSyncInterval: number | undefined;
let unlistenImportProgress: (() => void) | undefined;
let unlistenLibraryUpdated: (() => void) | undefined;
let unlistenMetadataRefreshProgress: (() => void) | undefined;

const statusMessage = computed(
  () =>
    windowError.value ||
    playback.errorMessage.value ||
    "Local YouTube via yt-dlp + mpv",
);
const isViewLoaded = computed(() =>
  view === "library"
    ? Boolean(playback.library.value && playback.transport.value)
    : Boolean(playback.snapshot.value),
);
const playbackSnapshot = computed<PlaybackSnapshot>(
  () =>
    playback.snapshot.value ?? {
      currentItem: null,
      positionMs: 0,
      queue: [],
      status: "paused",
      volumePercent: 0,
    },
);

watch(
  theme,
  (nextTheme) => {
    document.documentElement.dataset.theme = nextTheme;
    window.localStorage.setItem("gmusic-theme", nextTheme);
  },
  { immediate: true },
);

function cycleTheme(): void {
  const currentIndex = themes.indexOf(theme.value);
  theme.value = themes[(currentIndex + 1) % themes.length];
}

async function toggleQueue(): Promise<void> {
  queueExpanded.value = !queueExpanded.value;
  windowError.value = "";

  const size = playerDimensions(queueExpanded.value);
  const currentWindow = getCurrentWindow();
  let unlocked = false;

  try {
    await currentWindow.setResizable(true);
    unlocked = true;
    await currentWindow.setSize(new LogicalSize(size.width, size.height));
  } catch (error) {
    windowError.value =
      error instanceof Error
        ? error.message
        : "Could not resize the mini player.";
  } finally {
    if (unlocked) {
      try {
        await currentWindow.setResizable(false);
      } catch (error) {
        windowError.value =
          error instanceof Error
            ? error.message
            : "Could not lock the mini player size.";
      }
    }
  }
}

function closeMiniPlayer(): void {
  void getCurrentWindow().close();
}

async function openImportWindow(): Promise<void> {
  windowError.value = "";

  try {
    await windowApi.showImport();
  } catch (error) {
    windowError.value =
      error instanceof Error
        ? error.message
        : "Could not open the Import Music window.";
  }
}

function handleKeyboard(event: KeyboardEvent): void {
  if (
    view !== "mini" ||
    event.target instanceof HTMLInputElement ||
    event.metaKey ||
    event.ctrlKey ||
    event.altKey
  ) {
    return;
  }

  if (event.key === " ") {
    event.preventDefault();
    void playback.toggle();
  } else if (event.key.toLowerCase() === "j") {
    void playback.previous();
  } else if (event.key.toLowerCase() === "k") {
    void playback.next();
  } else if (event.key.toLowerCase() === "q") {
    void toggleQueue();
  }
}

async function trackArtworkWindowFocus(): Promise<void> {
  const currentWindow = getCurrentWindow();

  try {
    isWindowFocused.value = await currentWindow.isFocused();
    const unlisten = await currentWindow.onFocusChanged(({ payload }) => {
      isWindowFocused.value = payload;
    });

    if (isMounted) {
      unlistenWindowFocus = unlisten;
    } else {
      unlisten();
    }
  } catch {
    isWindowFocused.value = true;
  }
}

async function trackImportProgress(): Promise<void> {
  if (view !== "import") {
    return;
  }

  unlistenImportProgress = await listen<ImportProgress>(
    "import-progress",
    ({ payload }) => {
      playback.updateImportProgress(payload);
    },
  );
}

async function trackMetadataRefreshProgress(): Promise<void> {
  unlistenMetadataRefreshProgress = await listen<MetadataRefreshSnapshot>(
    "metadata-refresh-progress",
    ({ payload }) => {
      playback.updateMetadataRefreshes(payload);
    },
  );
}

async function trackLibraryUpdates(): Promise<void> {
  unlistenLibraryUpdated = await listen("library-updated", () => {
    void playback.refresh();
  });
}

onMounted(() => {
  isMounted = true;
  window.addEventListener("keydown", handleKeyboard);
  void trackLibraryUpdates().finally(() => {
    void playback.refresh();
  });
  if (view !== "settings") {
    playbackSyncInterval = window.setInterval(() => {
      void playback.sync();
    }, 500);
  }

  if (view === "artwork") {
    void trackArtworkWindowFocus();
  }
  void trackImportProgress();
  void trackMetadataRefreshProgress();
});

onUnmounted(() => {
  isMounted = false;
  unlistenWindowFocus?.();
  unlistenImportProgress?.();
  unlistenLibraryUpdated?.();
  unlistenMetadataRefreshProgress?.();
  if (playbackSyncInterval !== undefined) {
    window.clearInterval(playbackSyncInterval);
  }
  window.removeEventListener("keydown", handleKeyboard);
});
</script>

<template>
  <div
    class="relative min-h-screen w-full"
    :data-view="view"
    :data-queue-expanded="queueExpanded"
  >
    <SettingsWindow v-if="view === 'settings'" />

    <section
      v-else-if="!isViewLoaded"
      class="grid min-h-screen content-center gap-3.5 bg-(--glass-window) p-12"
      aria-label="Loading music window"
    >
      <span class="block h-3 w-[18%] rounded-full bg-(--surface-muted)" />
      <span class="block h-3 w-[48%] rounded-full bg-(--surface-muted)" />
      <span class="block h-3 w-[32%] rounded-full bg-(--surface-muted)" />
    </section>

    <LibraryWindow
      v-else-if="view === 'library'"
      :tracks="playback.library.value?.tracks"
      :transport="playback.transport.value ?? undefined"
      :is-starting="playback.isStarting.value"
      :is-updating="playback.isUpdating.value"
      :error-message="windowError || playback.errorMessage.value"
      :metadata-refreshes="playback.metadataRefreshes.value"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @play-track="playback.playTrack"
      @seek="playback.seek"
      @set-volume="playback.setVolume"
      @toggle-mute="playback.toggleMute"
      @open-import="openImportWindow"
    />

    <ImportWindow
      v-else-if="view === 'import'"
      :is-importing="playback.isImporting.value"
      :error-message="playback.errorMessage.value"
      :progress="playback.importProgress.value"
      @import-youtube-urls="playback.importYouTubeUrls"
    />

    <ArtworkWindow
      v-else-if="view === 'artwork'"
      :snapshot="playbackSnapshot"
      :is-updating="playback.isUpdating.value"
      :is-window-focused="isWindowFocused"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @seek="playback.seek"
    />

    <QueueWindow
      v-else-if="view === 'queue'"
      :queue="playbackSnapshot.queue"
      :current-item-id="playbackSnapshot.currentItem?.id"
    />

    <MiniWindow
      v-else
      :snapshot="playbackSnapshot"
      :is-updating="playback.isUpdating.value"
      :queue-expanded="queueExpanded"
      :theme="theme"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @seek="playback.seek"
      @set-volume="playback.setVolume"
      @toggle-mute="playback.toggleMute"
      @move="playback.moveQueueItem"
      @toggle-queue="toggleQueue"
      @toggle-theme="cycleTheme"
      @close="closeMiniPlayer"
    />

    <p
      v-if="view === 'mini'"
      class="absolute right-11.5 bottom-0.5 m-0 max-w-43 overflow-hidden text-right text-[0.53rem] font-semibold tracking-[0.06em] text-ellipsis whitespace-nowrap text-(--subtle-text) uppercase"
    >
      {{ statusMessage }}
    </p>
  </div>
</template>
