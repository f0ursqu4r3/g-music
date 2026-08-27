<script setup lang="ts">
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { ChevronDown, ChevronUp, Palette, X } from "lucide-vue-next";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";

import MiniPlayer from "@/components/MiniPlayer.vue";
import QueueDrawer from "@/components/QueueDrawer.vue";
import { Button } from "@/components/ui/button";

import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { usePlayback } from "@/composables/usePlayback";
import { playerDimensions } from "@/presentation";
import { readTheme, themes, type ThemeName } from "@/lib/theme";

const playback = usePlayback();
const queueExpanded = ref(false);
const theme = ref<ThemeName>(
  readTheme(window.localStorage.getItem("gmusic-theme")),
);
const themeMenuOpen = ref(false);
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

function selectTheme(nextTheme: ThemeName): void {
  theme.value = nextTheme;
  themeMenuOpen.value = false;
}

async function toggleQueue(): Promise<void> {
  queueExpanded.value = !queueExpanded.value;
  themeMenuOpen.value = false;
  windowError.value = "";

  const size = playerDimensions(queueExpanded.value);
  const currentWindow = getCurrentWindow();

  try {
    await currentWindow.setResizable(true);
    await currentWindow.setSize(new LogicalSize(size.width, size.height));
  } catch (error) {
    windowError.value =
      error instanceof Error
        ? error.message
        : "Could not resize the mini player.";
  } finally {
    await currentWindow.setResizable(false);
  }
}

function handleKeyboard(event: KeyboardEvent): void {
  if (
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
  <TooltipProvider :delay-duration="250">
    <main class="app-shell" :data-queue-expanded="queueExpanded">
      <section
        v-if="!playback.snapshot.value"
        class="loading-player"
        aria-label="Loading player"
      >
        <div class="loading-artwork" />
        <div class="loading-copy">
          <span />
          <span />
          <span />
        </div>
      </section>

      <section v-else class="player-frame">
        <MiniPlayer
          :snapshot="playback.snapshot.value"
          :is-updating="playback.isUpdating.value"
          @toggle="playback.toggle"
          @previous="playback.previous"
          @next="playback.next"
          @seek="playback.seek"
          @set-volume="playback.setVolume"
        />

        <QueueDrawer
          v-if="queueExpanded"
          :queue="playback.snapshot.value.queue"
          :current-item-id="playback.snapshot.value.currentItem?.id"
          :is-updating="playback.isUpdating.value"
          @move="playback.moveQueueItem"
        />

        <p class="player-status">{{ statusMessage }}</p>

        <nav class="window-actions" aria-label="Player options">
          <div class="theme-menu">
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  aria-label="Choose theme"
                  :aria-expanded="themeMenuOpen"
                  size="icon-xs"
                  variant="ghost"
                  @click="themeMenuOpen = !themeMenuOpen"
                >
                  <Palette aria-hidden="true" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="left">Choose theme</TooltipContent>
            </Tooltip>
            <div
              v-if="themeMenuOpen"
              class="theme-menu-content"
              role="menu"
              aria-label="Appearance"
            >
              <p class="theme-menu-label">Appearance</p>
              <Button
                v-for="themeOption in themes"
                :key="themeOption"
                :aria-current="themeOption === theme ? 'true' : undefined"
                class="theme-menu-item"
                role="menuitemradio"
                size="sm"
                variant="ghost"
                @click="selectTheme(themeOption)"
              >
                <span class="theme-swatch" :data-theme="themeOption" />
                <span class="capitalize">{{ themeOption }}</span>
                <span
                  v-if="themeOption === theme"
                  class="ml-auto text-[0.65rem]"
                  >Current</span
                >
              </Button>
            </div>
          </div>

          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                :aria-label="queueExpanded ? 'Hide queue' : 'Show queue'"
                size="icon-xs"
                variant="ghost"
                @click="toggleQueue"
              >
                <ChevronUp v-if="queueExpanded" aria-hidden="true" />
                <ChevronDown v-else aria-hidden="true" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="left">{{
              queueExpanded ? "Hide queue" : "Show queue"
            }}</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                aria-label="Close player"
                size="icon-xs"
                variant="ghost"
                @click="getCurrentWindow().close()"
              >
                <X aria-hidden="true" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="left">Close player</TooltipContent>
          </Tooltip>
        </nav>
      </section>
    </main>
  </TooltipProvider>
</template>
