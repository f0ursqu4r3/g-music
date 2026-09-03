<script setup lang="ts">
import {
  Disc3,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Volume,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-vue-next";
import { computed } from "vue";

import type { PlaybackSnapshot } from "@/api";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/time";
import YouTubeArtwork from "./YouTubeArtwork.vue";

interface Props {
  snapshot: PlaybackSnapshot;
  isUpdating: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [volumePercent: number];
  toggleMute: [];
}>();

const currentItem = computed(() => props.snapshot.currentItem);
const durationMs = computed(() => currentItem.value?.durationMs ?? 0);
const isPlaying = computed(() => props.snapshot.status === "playing");
const trackTitle = computed(
  () => currentItem.value?.title ?? "Nothing selected",
);
const trackArtist = computed(
  () => currentItem.value?.artist ?? "Choose a track to begin",
);
const remainingMs = computed(() =>
  Math.max(durationMs.value - props.snapshot.positionMs, 0),
);
const volumeIcon = computed(() => {
  if (props.snapshot.volumePercent === 0) {
    return VolumeX;
  }
  if (props.snapshot.volumePercent <= 33) {
    return Volume;
  }
  if (props.snapshot.volumePercent <= 66) {
    return Volume1;
  }
  return Volume2;
});

function emitSeek(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit("seek", value);
  }
}

function emitVolume(values: number[] | undefined): void {
  const value = values?.[0];
  if (typeof value === "number" && Number.isFinite(value)) {
    emit("setVolume", value);
  }
}
</script>

<template>
  <section
    class="mini-player-shell relative grid min-h-43.5 grid-cols-[164px_minmax(0,1fr)] max-[390px]:grid-cols-[138px_minmax(0,1fr)]"
    aria-label="Now playing"
  >
    <div
      class="absolute top-0 right-0 left-0 z-3 grid min-h-8"
      data-tauri-drag-region
      title="Drag to move mini player"
      aria-hidden="true"
    ></div>

    <div
      class="album-art relative grid place-items-center overflow-hidden"
      aria-hidden="true"
    >
      <YouTubeArtwork
        class="absolute inset-0 size-full object-cover"
        :video-id="currentItem?.id"
        :missing-icon="Disc3"
      />
    </div>

    <div
      class="grid min-w-0 grid-rows-[auto_1fr_auto] pt-5 pr-11.5 pb-3.5 pl-5 max-[390px]:pl-4"
    >
      <header class="min-w-0 mt-4">
        <div class="min-w-0">
          <h1
            class="overflow-hidden text-[1.2rem] leading-[1.08] font-[680] tracking-[-0.045em] text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ trackTitle }}
          </h1>
          <p
            class="mt-1.25 overflow-hidden text-[0.77rem] font-[510] tracking-[-0.005em] text-ellipsis whitespace-nowrap text-(--muted-text)"
          >
            {{ trackArtist }}
          </p>
        </div>
      </header>

      <div class="flex items-center justify-between gap-4">
        <nav class="flex items-center gap-0.75" aria-label="Playback controls">
          <Button
            aria-label="Previous track"
            class="text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text)"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating"
            @click="emit('previous')"
          >
            <SkipBack aria-hidden="true" />
          </Button>
          <Button
            :aria-label="isPlaying ? 'Pause' : 'Play'"
            class="rounded-[11px] bg-accent text-(--accent-ink) hover:bg-[color-mix(in_oklch,var(--accent)_88%,oklch(0.98_0.01_90))] hover:text-(--accent-ink) active:translate-y-px"
            size="icon"
            :disabled="isUpdating"
            @click="emit('toggle')"
          >
            <Pause v-if="isPlaying" aria-hidden="true" />
            <Play v-else aria-hidden="true" fill="currentColor" />
          </Button>
          <Button
            aria-label="Next track"
            class="text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text)"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating"
            @click="emit('next')"
          >
            <SkipForward aria-hidden="true" />
          </Button>
        </nav>

        <div
          class="grid w-27.5 grid-cols-[14px_minmax(0,1fr)_20px] items-center gap-1.75 text-(--muted-text) max-[390px]:w-21.5"
        >
          <Button
            :aria-label="
              snapshot.volumePercent === 0 ? 'Unmute volume' : 'Mute volume'
            "
            class="size-5 text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) [&_svg]:size-3.5"
            size="icon-xs"
            variant="ghost"
            :disabled="isUpdating"
            @click="emit('toggleMute')"
          >
            <component :is="volumeIcon" aria-hidden="true" />
          </Button>
          <Slider
            aria-label="Volume"
            :min="0"
            :max="100"
            :step="1"
            :model-value="[snapshot.volumePercent]"
            :disabled="isUpdating"
            @update:model-value="emitVolume"
          />
          <span
            class="text-right text-[0.6rem] text-(--subtle-text) tabular-nums max-[390px]:hidden"
            aria-hidden="true"
          >
            {{ snapshot.volumePercent }}
          </span>
        </div>
      </div>

      <div
        class="grid grid-cols-[30px_minmax(0,1fr)_34px] items-center gap-2 text-[0.62rem] text-(--subtle-text) tabular-nums [&>span:last-child]:text-right"
      >
        <span>{{ formatDuration(snapshot.positionMs) }}</span>
        <Slider
          aria-label="Track progress"
          :min="0"
          :max="durationMs"
          :step="1000"
          :model-value="[snapshot.positionMs]"
          :disabled="isUpdating || durationMs === 0"
          @value-commit="emitSeek"
        />
        <span>-{{ formatDuration(remainingMs) }}</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.album-art {
  background: linear-gradient(
    136deg,
    var(--artwork-a),
    var(--artwork-b) 55%,
    var(--artwork-c)
  );
}
</style>
