<script setup lang="ts">
import {
  Disc3,
  Heart,
  MoreHorizontal,
  Pause,
  Play,
  Repeat2,
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
}

const props = withDefaults(defineProps<Props>(), {
  isWindowFocused: true,
});
const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  toggleFavorite: [id: string];
}>();

const isPlaying = computed(() => props.snapshot.status === "playing");
const currentItem = computed(() => props.snapshot.currentItem);

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
          Add to Favorites
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
              :text="currentItem?.title ?? 'Nothing selected'"
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
              size="icon-sm"
              variant="ghost"
              :disabled="!currentItem || isUpdating"
              @click="currentItem && emit('toggleFavorite', currentItem.id)"
            >
              <Heart aria-hidden="true" />
            </Button>
            <Button
              class="text-[oklch(0.94_0.012_270/0.74)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
              aria-label="More actions"
              size="icon-sm"
              variant="ghost"
            >
              <MoreHorizontal aria-hidden="true" />
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
            aria-label="Shuffle"
            size="icon-sm"
            variant="ghost"
          >
            <Shuffle aria-hidden="true" />
          </Button>
          <Button
            class="text-[oklch(0.94_0.012_270/0.76)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
            aria-label="Previous track"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating"
            @click="emit('previous')"
          >
            <SkipBack aria-hidden="true" />
          </Button>
          <Button
            :aria-label="isPlaying ? 'Pause' : 'Play'"
            class="size-10 rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text)"
            size="icon"
            :disabled="isUpdating"
            @click="emit('toggle')"
          >
            <Pause v-if="isPlaying" aria-hidden="true" fill="currentColor" />
            <Play v-else aria-hidden="true" fill="currentColor" />
          </Button>
          <Button
            class="text-[oklch(0.94_0.012_270/0.76)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
            aria-label="Next track"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating"
            @click="emit('next')"
          >
            <SkipForward aria-hidden="true" />
          </Button>
          <Button
            class="text-[oklch(0.94_0.012_270/0.76)] hover:bg-[oklch(0.94_0.012_270/0.1)] hover:text-(--text)"
            aria-label="Repeat"
            size="icon-sm"
            variant="ghost"
          >
            <Repeat2 aria-hidden="true" />
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
            :disabled="isUpdating || !currentItem"
            @value-commit="emitSeek"
          />
          <span class="text-right">{{
            formatDuration(currentItem?.durationMs ?? 0)
          }}</span>
        </div>
      </div>
    </section>
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
  text-shadow: 0 1px 12px oklch(0.05 0.02 260 / 0.45);
}
</style>
