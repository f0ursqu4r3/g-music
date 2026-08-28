<script setup lang="ts">
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";

import type { ThemeName } from "@/lib/theme";
import { readTheme, themes } from "@/lib/theme";
import { resolveMockWindowView } from "@/lib/window-view";
import { playerDimensions } from "@/presentation";

import ArtworkWindow from "@/components/ArtworkWindow.vue";
import LibraryWindow from "@/components/LibraryWindow.vue";
import MiniWindow from "@/components/MiniWindow.vue";
import QueueWindow from "@/components/QueueWindow.vue";
import { usePlayback } from "@/composables/usePlayback";

const playback = usePlayback();
const view = resolveMockWindowView(window.location.search);
const queueExpanded = ref(false);
const theme = ref<ThemeName>(
  readTheme(window.localStorage.getItem("gmusic-theme")),
);
const windowError = ref("");

const statusMessage = computed(
  () =>
    windowError.value || playback.errorMessage.value || "Local fake provider",
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

onMounted(() => {
  window.addEventListener("keydown", handleKeyboard);
  void playback.refresh();
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyboard);
});
</script>

<template>
  <main
    class="relative min-h-screen w-full"
    :data-view="view"
    :data-queue-expanded="queueExpanded"
  >
    <section
      v-if="!playback.snapshot.value"
      class="grid min-h-screen content-center gap-3.5 bg-(--glass-window) p-12"
      aria-label="Loading music window"
    >
      <span class="block h-3 w-[18%] rounded-full bg-(--surface-muted)" />
      <span class="block h-3 w-[48%] rounded-full bg-(--surface-muted)" />
      <span class="block h-3 w-[32%] rounded-full bg-(--surface-muted)" />
    </section>

    <LibraryWindow
      v-else-if="view === 'library'"
      :snapshot="playback.snapshot.value"
      :is-updating="playback.isUpdating.value"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
    />

    <ArtworkWindow
      v-else-if="view === 'artwork'"
      :snapshot="playback.snapshot.value"
      :is-updating="playback.isUpdating.value"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
    />

    <QueueWindow
      v-else-if="view === 'queue'"
      :queue="playback.snapshot.value.queue"
      :current-item-id="playback.snapshot.value.currentItem?.id"
    />

    <MiniWindow
      v-else
      :snapshot="playback.snapshot.value"
      :is-updating="playback.isUpdating.value"
      :queue-expanded="queueExpanded"
      :theme="theme"
      @toggle="playback.toggle"
      @previous="playback.previous"
      @next="playback.next"
      @seek="playback.seek"
      @set-volume="playback.setVolume"
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
  </main>
</template>
