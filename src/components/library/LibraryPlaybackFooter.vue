<script setup lang="ts">
import {
  Pause,
  Play,
  Repeat2,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
} from "lucide-vue-next";

import type { MediaItem, PlaybackTransport } from "@/api";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/time";
import YouTubeArtwork from "../YouTubeArtwork.vue";

const props = defineProps<{
  playback: PlaybackTransport;
  currentItem: MediaItem | null;
  isPlaying: boolean;
  isUpdating: boolean;
}>();

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [percent: number];
}>();

function emitVolume(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit("setVolume", value);
  }
}

function emitSeek(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit("seek", value);
  }
}
</script>

<template>
  <footer class="contents">
    <div
      class="relative col-start-1 row-start-2 flex min-w-0 items-center gap-3 border-t border-(--line) pr-4 max-[760px]:hidden"
    >
      <div class="cover-art h-full aspect-square shrink-0">
        <YouTubeArtwork
          class="absolute inset-0 size-full object-cover"
          :video-id="props.currentItem?.id"
        />
      </div>
      <div class="min-w-0">
        <p
          class="overflow-hidden text-[0.82rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
        >
          {{ props.currentItem?.title ?? "Nothing selected" }}
        </p>
        <span
          class="mt-0.5 block overflow-hidden text-[0.74rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
        >
          {{ props.currentItem?.artist ?? "Choose a track" }}
        </span>
      </div>
    </div>

    <div
      class="col-start-2 row-start-2 grid min-w-0 grid-rows-[1fr_auto] border-t border-l border-(--line) px-6 pt-2 pb-2 max-[760px]:col-start-1"
    >
      <div class="grid grid-cols-[1fr_auto_1fr] items-center gap-4">
        <span aria-hidden="true" />
        <nav
          class="flex items-center justify-center gap-1.5"
          aria-label="Playback controls"
        >
          <Button aria-label="Shuffle" size="icon-sm" variant="ghost">
            <Shuffle aria-hidden="true" />
          </Button>
          <Button
            aria-label="Previous track"
            size="icon-sm"
            variant="ghost"
            :disabled="props.isUpdating"
            @click="emit('previous')"
          >
            <SkipBack aria-hidden="true" />
          </Button>
          <Button
            :aria-label="props.isPlaying ? 'Pause' : 'Play'"
            class="size-10 rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text)"
            size="icon"
            :disabled="props.isUpdating"
            @click="emit('toggle')"
          >
            <Pause
              v-if="props.isPlaying"
              aria-hidden="true"
              fill="currentColor"
            />
            <Play v-else aria-hidden="true" fill="currentColor" />
          </Button>
          <Button
            aria-label="Next track"
            size="icon-sm"
            variant="ghost"
            :disabled="props.isUpdating"
            @click="emit('next')"
          >
            <SkipForward aria-hidden="true" />
          </Button>
          <Button aria-label="Repeat" size="icon-sm" variant="ghost">
            <Repeat2 aria-hidden="true" />
          </Button>
        </nav>

        <label
          class="ml-auto flex w-40 items-center gap-2 text-(--muted-text) [&>svg]:size-4"
        >
          <Volume2 aria-hidden="true" />
          <span class="sr-only">Volume</span>
          <Slider
            aria-label="Volume"
            :min="0"
            :max="100"
            :step="1"
            :model-value="[props.playback.volumePercent]"
            :disabled="props.isUpdating"
            @value-commit="emitVolume"
          />
        </label>
      </div>

      <div
        class="grid grid-cols-[30px_minmax(0,1fr)_30px] items-center gap-2 text-[0.65rem] text-(--muted-text) tabular-nums"
        aria-label="Track progress"
      >
        <span>{{ formatDuration(props.playback.positionMs) }}</span>
        <Slider
          aria-label="Track progress"
          :min="0"
          :max="props.currentItem?.durationMs ?? 0"
          :step="1000"
          :model-value="[props.playback.positionMs]"
          :disabled="props.isUpdating || !props.currentItem"
          @value-commit="emitSeek"
        />
        <span class="text-right">
          {{ formatDuration(props.currentItem?.durationMs ?? 0) }}
        </span>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.cover-art {
  position: relative;
  overflow: hidden;
  background: linear-gradient(
    138deg,
    var(--artwork-a),
    var(--artwork-b) 58%,
    var(--artwork-c)
  );
}
</style>
