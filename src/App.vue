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
import { resolvePlaybackHotkey } from "@/lib/hotkeys";
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
const keyboardShortcutsOpen = ref(false);
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
let unlistenPlaybackUpdated: (() => void) | undefined;
let unlistenKeyboardShortcuts: (() => void) | undefined;

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
  if (keyboardShortcutsOpen.value && event.key === "Escape") {
    event.preventDefault();
    keyboardShortcutsOpen.value = false;
    return;
  }

  const target = event.target;
  const isTextEditing =
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable);
  const action = resolvePlaybackHotkey(
    event.key,
    isTextEditing,
    event.metaKey || event.ctrlKey || event.altKey,
  );

  if (action === "toggle") {
    event.preventDefault();
    void playback.toggle();
  } else if (action === "previous") {
    event.preventDefault();
    void playback.previous();
  } else if (action === "next") {
    event.preventDefault();
    void playback.next();
  } else if (action === "toggleMute") {
    event.preventDefault();
    void playback.toggleMute();
  } else if (
    view === "mini" &&
    !isTextEditing &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    event.key.toLowerCase() === "q"
  ) {
    event.preventDefault();
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

async function trackPlaybackUpdates(): Promise<void> {
  unlistenPlaybackUpdated = await listen<PlaybackSnapshot>(
    "playback-updated",
    ({ payload }) => {
      playback.applySnapshot(payload);
    },
  );
}

async function trackKeyboardShortcuts(): Promise<void> {
  unlistenKeyboardShortcuts = await listen("show-keyboard-shortcuts", () => {
    keyboardShortcutsOpen.value = true;
  });
}

onMounted(() => {
  isMounted = true;
  window.addEventListener("keydown", handleKeyboard);
  void trackLibraryUpdates().finally(() => {
    void playback.refresh();
  });
  void trackPlaybackUpdates();
  void trackKeyboardShortcuts();
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
  unlistenPlaybackUpdated?.();
  unlistenKeyboardShortcuts?.();
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
      :playlists="playback.library.value?.playlists"
      :tracks="playback.library.value?.tracks"
      :transport="playback.transport.value ?? undefined"
      :is-starting="playback.isStarting.value"
      :is-updating="playback.isUpdating.value"
      :error-message="windowError || playback.errorMessage.value"
      :metadata-refreshes="playback.metadataRefreshes.value"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @toggle-shuffle="playback.toggleShuffle"
      @cycle-repeat-mode="playback.cycleRepeatMode"
      @play-track="(queueIds, id) => playback.playTrack(id, queueIds)"
      @play-next="playback.playNext"
      @add-to-queue="playback.addToQueue"
      @seek="playback.seek"
      @set-volume="playback.setVolume"
      @toggle-mute="playback.toggleMute"
      @open-import="openImportWindow"
      @upsert-playlist="playback.upsertPlaylist"
      @reorder-playlists="playback.reorderPlaylists"
      @delete-playlist="playback.deletePlaylist"
      @remove-tracks="playback.removeTracks"
      @toggle-favorite="playback.toggleFavorite"
      @update-tracks-metadata="playback.updateTracksMetadata"
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
      @toggle-favorite="playback.toggleFavorite"
    />

    <QueueWindow
      v-else-if="view === 'queue'"
      :queue="playbackSnapshot.queue"
      :playback-order="playbackSnapshot.playbackOrder"
      :current-item-id="playbackSnapshot.currentItem?.id"
      :status="playbackSnapshot.status"
      :shuffle-enabled="playbackSnapshot.shuffleEnabled"
      :position-ms="playbackSnapshot.positionMs"
      :is-starting="playback.isStarting.value"
      :is-updating="playback.isUpdating.value"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @play-track="
        (id) =>
          playback.playTrack(
            id,
            playbackSnapshot.queue.map((item) => item.id),
          )
      "
      @move="playback.moveQueueItem"
      @remove="playback.removeQueueItem"
    />

    <MiniWindow
      v-else
      :snapshot="playbackSnapshot"
      :is-updating="playback.isUpdating.value"
      :is-starting="playback.isStarting.value"
      :favorite-track-ids="
        playback.library.value?.playlists?.find(
          (playlist) => playlist.id === 'favorites',
        )?.trackIds ?? []
      "
      :queue-expanded="queueExpanded"
      :theme="theme"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @seek="playback.seek"
      @set-volume="playback.setVolume"
      @toggle-mute="playback.toggleMute"
      @toggle-shuffle="playback.toggleShuffle"
      @cycle-repeat-mode="playback.cycleRepeatMode"
      @toggle-favorite="playback.toggleFavorite"
      @move="playback.moveQueueItem"
      @remove="playback.removeQueueItem"
      @toggle-queue="toggleQueue"
      @toggle-theme="cycleTheme"
      @close="closeMiniPlayer"
    />

    <section
      v-if="keyboardShortcutsOpen"
      class="absolute inset-0 z-50 grid place-items-center bg-black/60 p-5 backdrop-blur-sm"
      role="dialog"
      aria-labelledby="keyboard-shortcuts-title"
      aria-modal="true"
      @click.self="keyboardShortcutsOpen = false"
    >
      <div
        class="w-full max-w-92 rounded-2xl border border-(--line-strong) bg-[oklch(0.11_0.014_260/0.98)] p-5 shadow-2xl"
      >
        <header class="flex items-center justify-between gap-4">
          <h2
            id="keyboard-shortcuts-title"
            class="text-lg font-semibold text-(--text)"
          >
            Keyboard Shortcuts
          </h2>
          <button
            class="rounded-md px-2 py-1 text-sm text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text)"
            type="button"
            aria-label="Close keyboard shortcuts"
            @click="keyboardShortcutsOpen = false"
          >
            Esc
          </button>
        </header>
        <dl class="mt-5 grid grid-cols-[1fr_auto] gap-x-6 gap-y-3 text-sm">
          <dt class="text-(--muted-text)">Play or pause</dt>
          <dd class="font-mono text-(--text)">Space</dd>
          <dt class="text-(--muted-text)">Previous track</dt>
          <dd class="font-mono text-(--text)">⌘ ← / J</dd>
          <dt class="text-(--muted-text)">Next track</dt>
          <dd class="font-mono text-(--text)">⌘ → / K</dd>
          <dt class="text-(--muted-text)">Mute</dt>
          <dd class="font-mono text-(--text)">M</dd>
          <dt class="text-(--muted-text)">Import music</dt>
          <dd class="font-mono text-(--text)">⌘ I</dd>
          <dt class="text-(--muted-text)">Library windows</dt>
          <dd class="font-mono text-(--text)">⌘ 1–4</dd>
        </dl>
      </div>
    </section>
  </div>
</template>
