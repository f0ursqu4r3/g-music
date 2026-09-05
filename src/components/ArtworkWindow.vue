<script setup lang="ts">
import {
  Disc3,
  Heart,
  LoaderCircle,
  Pause,
  Play,
  Repeat2,
  Repeat1,
  Repeat,
  Shuffle,
  SkipBack,
  SkipForward,
} from "lucide-vue-next";
import { computed } from "vue";

import type { PlaybackSnapshot } from "@/api";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/time";
import AutoScrollText from "./AutoScrollText.vue";
import YouTubeArtwork from "./YouTubeArtwork.vue";

interface Props {
  snapshot: PlaybackSnapshot;
  isUpdating: boolean;
  isWindowFocused?: boolean;
  isStarting?: boolean;
  favoriteTrackIds?: string[];
}

const props = withDefaults(defineProps<Props>(), {
  isWindowFocused: true,
  isStarting: false,
  favoriteTrackIds: () => [],
});
const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  toggleFavorite: [id: string];
  toggleShuffle: [];
  cycleRepeatMode: [];
}>();

const isPlaying = computed(() => props.snapshot.status === "playing");
const currentItem = computed(() => props.snapshot.currentItem);
const isFavorite = computed(() =>
  currentItem.value
    ? props.favoriteTrackIds.includes(currentItem.value.id)
    : false,
);
const repeatMode = computed(() => props.snapshot.repeatMode ?? "off");
const repeatLabel = computed(() =>
  repeatMode.value === "one"
    ? "Disable repeat"
    : repeatMode.value === "all"
      ? "Enable repeat one"
      : "Enable repeat all",
);
const repeatIcon = computed(() =>
  repeatMode.value === "one"
    ? Repeat1
    : repeatMode.value === "all"
      ? Repeat2
      : Repeat,
);
const compactProgressMs = computed(() => {
  const durationMs = currentItem.value?.durationMs ?? 0;
  return Math.min(Math.max(props.snapshot.positionMs, 0), durationMs);
});

function emitSeek(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit("seek", value);
  }
}
</script>

<template>
  <main
    class="artwork-window window-shell relative flex h-screen min-h-0 flex-col overflow-hidden bg-(--canvas) text-(--text)"
    aria-label="Current artwork"
  >
    <ContextMenu>
      <ContextMenuTrigger as-child>
        <div
          class="artwork-cover absolute inset-0 overflow-hidden"
          data-cover="violet"
          aria-hidden="true"
        >
          <YouTubeArtwork
            class="absolute inset-0 size-full object-cover"
            :video-id="currentItem?.id"
            :missing-icon="Disc3"
          />
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent data-artwork-context-menu>
        <ContextMenuItem
          :disabled="!currentItem || isUpdating"
          @select="currentItem && emit('toggleFavorite', currentItem.id)"
        >
          {{ isFavorite ? "Remove from Favorites" : "Add to Favorites" }}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>

    <div
      class="artwork-drag-region relative z-20 min-h-0 flex-1 cursor-grab active:cursor-grabbing"
      data-tauri-drag-region
      aria-hidden="true"
    />

    <section
      class="artwork-controls artwork-information-gradient relative z-10 shrink-0 p-6"
      aria-label="Artwork playback controls"
    >
      <header class="artwork-metadata">
        <p
          class="text-[0.58rem] font-bold tracking-[0.16em] text-[oklch(0.94_0.012_270/0.64)] uppercase"
        >
          Now playing
        </p>
        <div class="mt-1 flex items-end justify-between gap-4">
          <div class="min-w-0">
            <AutoScrollText
              as="h1"
              class="text-2xl font-semibold tracking-[-0.04em]"
              speed="slow"
              :text="currentItem?.title ?? 'Nothing playing'"
            />
            <AutoScrollText
              as="p"
              class="mt-1 text-[0.8rem] text-[oklch(0.94_0.012_270/0.72)]"
              speed="medium"
              :text="currentItem?.artist ?? 'Choose a track to begin'"
            />
          </div>
          <div class="flex shrink-0 items-center gap-0.5">
            <Button
              class="text-[oklch(0.94_0.012_270/0.74)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
              aria-label="Favorite track"
              :aria-pressed="isFavorite"
              :title="isFavorite ? 'Remove from Favorites' : 'Add to Favorites'"
              size="icon-sm"
              variant="ghost"
              :disabled="!currentItem || isUpdating"
              @click="currentItem && emit('toggleFavorite', currentItem.id)"
            >
              <Heart
                :fill="isFavorite ? 'currentColor' : 'none'"
                aria-hidden="true"
              />
            </Button>
          </div>
        </div>
      </header>

      <div
        class="artwork-playback-controls mt-6 max-h-32 overflow-hidden transition-[max-height,margin,opacity,transform] duration-250 ease-out motion-reduce:transition-none data-[window-focused=false]:mt-0 data-[window-focused=false]:max-h-0 data-[window-focused=false]:pointer-events-none data-[window-focused=false]:translate-y-full data-[window-focused=false]:opacity-0"
        :data-window-focused="isWindowFocused"
        :aria-hidden="isWindowFocused ? undefined : 'true'"
        :inert="!isWindowFocused"
      >
        <div
          class="h-0.5 w-full rounded-full bg-[oklch(0.94_0.012_270/0.1)]"
          aria-hidden="true"
        />
        <nav
          class="flex items-center justify-center gap-1.5 pt-5 pb-4"
          aria-label="Playback controls"
        >
          <Button
            class="text-[oklch(0.94_0.012_270/0.76)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
            :aria-label="
              snapshot.shuffleEnabled ? 'Disable shuffle' : 'Enable shuffle'
            "
            :aria-pressed="snapshot.shuffleEnabled ?? false"
            :disabled="isUpdating || isStarting"
            size="icon-sm"
            variant="ghost"
            @click="emit('toggleShuffle')"
          >
            <Shuffle aria-hidden="true" />
          </Button>
          <Button
            class="text-[oklch(0.94_0.012_270/0.76)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
            aria-label="Previous track"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating || isStarting || !currentItem"
            @click="emit('previous')"
          >
            <SkipBack aria-hidden="true" />
          </Button>
          <Button
            :aria-label="
              isStarting ? 'Starting playback' : isPlaying ? 'Pause' : 'Play'
            "
            :aria-busy="isStarting ? 'true' : undefined"
            class="size-10 rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text) disabled:bg-(--text)/50 disabled:opacity-100"
            size="icon"
            :disabled="
              isUpdating ||
              isStarting ||
              (!currentItem && snapshot.queue.length === 0)
            "
            @click="emit('toggle')"
          >
            <LoaderCircle
              v-if="isStarting"
              class="animate-spin"
              data-playback-starting
              aria-hidden="true"
            />
            <Pause
              v-else-if="isPlaying"
              aria-hidden="true"
              fill="currentColor"
            />
            <Play v-else aria-hidden="true" fill="currentColor" />
          </Button>
          <Button
            class="text-[oklch(0.94_0.012_270/0.76)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
            aria-label="Next track"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating || isStarting || !currentItem"
            @click="emit('next')"
          >
            <SkipForward aria-hidden="true" />
          </Button>
          <Button
            class="text-[oklch(0.94_0.012_270/0.76)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
            :aria-label="repeatLabel"
            :aria-pressed="repeatMode !== 'off'"
            :data-repeat-mode="repeatMode"
            :disabled="isUpdating || isStarting"
            size="icon-sm"
            variant="ghost"
            @click="emit('cycleRepeatMode')"
          >
            <component :is="repeatIcon" aria-hidden="true" />
          </Button>
        </nav>

        <div
          class="grid grid-cols-[34px_minmax(0,1fr)_34px] items-center gap-2 text-[0.65rem] text-[oklch(0.94_0.012_270/0.62)] tabular-nums"
          aria-label="Track progress"
        >
          <span>{{ formatDuration(snapshot.positionMs) }}</span>
          <Slider
            aria-label="Track progress"
            :min="0"
            :max="currentItem?.durationMs ?? 0"
            :step="1000"
            :model-value="[snapshot.positionMs]"
            :disabled="isUpdating || isStarting || !currentItem"
            @value-commit="emitSeek"
          />
          <span class="text-right">{{
            formatDuration(currentItem?.durationMs ?? 0)
          }}</span>
        </div>
      </div>
    </section>

    <progress
      v-if="!isWindowFocused && currentItem && currentItem.durationMs > 0"
      class="artwork-unfocused-progress pointer-events-none absolute right-0 bottom-0 left-0 z-30 block h-0.5 w-full overflow-hidden border-0"
      aria-label="Track progress"
      :value="compactProgressMs"
      :max="currentItem.durationMs"
    />
  </main>
</template>

<style scoped>
.artwork-cover {
  background:
    radial-gradient(
      circle at 18% 8%,
      color-mix(in oklch, var(--artwork-a), transparent 8%),
      transparent 43%
    ),
    linear-gradient(
      148deg,
      var(--artwork-a),
      var(--artwork-b) 54%,
      var(--artwork-c)
    );
}

.artwork-information-gradient {
  background: linear-gradient(
    180deg,
    transparent 0%,
    oklch(0.09 0.025 260 / 0.5) 20%,
    oklch(0.075 0.025 260 / 0.8) 100%
  );
}

.artwork-controls {
  --text: oklch(0.98 0.005 270);
  --accent-ink: oklch(0.12 0.015 270);
  text-shadow: 0 1px 12px oklch(0.05 0.02 260 / 0.45);
}

.artwork-unfocused-progress {
  appearance: none;
  border-radius: 0 0 12px 12px;
  background: transparent;
  color: var(--accent);
}

.artwork-unfocused-progress::-webkit-progress-bar {
  background: transparent;
}

.artwork-unfocused-progress::-webkit-progress-value {
  background: var(--accent);
}

.artwork-unfocused-progress::-moz-progress-bar {
  background: var(--accent);
}
</style>
