<script setup lang="ts">
import { Pause, Play, SkipBack, SkipForward, Volume2 } from "lucide-vue-next";
import { computed } from "vue";

import type { PlaybackSnapshot } from "@/api";
import { Button } from "@/components/ui/button";
import { formatDuration } from "@/lib/time";

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

function inputValue(event: Event): number | null {
  const value = Number((event.target as HTMLInputElement).value);

  return Number.isFinite(value) ? value : null;
}

function emitSeek(event: Event): void {
  const value = inputValue(event);
  if (value !== null) {
    emit("seek", value);
  }
}

function emitVolume(event: Event): void {
  const value = inputValue(event);
  if (value !== null) {
    emit("setVolume", value);
  }
}
</script>

<template>
  <section class="mini-player-shell" aria-label="Now playing">
    <div class="window-grab" data-tauri-drag-region aria-hidden="true" />

    <div class="album-art" aria-hidden="true">
      <div class="album-disc" />
      <span>GM</span>
    </div>

    <div class="player-main">
      <header class="track-header">
        <div class="min-w-0">
          <p class="track-kicker">
            <span class="playback-dot" :data-playing="isPlaying" />
            {{ isPlaying ? "Playing" : "Paused" }}
          </p>
          <h1 class="track-title">{{ trackTitle }}</h1>
          <p class="track-artist">{{ trackArtist }}</p>
        </div>
      </header>

      <div class="transport-row">
        <div class="transport-group" aria-label="Playback controls">
          <Button
            aria-label="Previous track"
            class="transport-button"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating"
            @click="emit('previous')"
          >
            <SkipBack aria-hidden="true" />
          </Button>
          <Button
            :aria-label="isPlaying ? 'Pause' : 'Play'"
            class="play-button"
            size="icon"
            :disabled="isUpdating"
            @click="emit('toggle')"
          >
            <Pause v-if="isPlaying" aria-hidden="true" />
            <Play v-else aria-hidden="true" fill="currentColor" />
          </Button>
          <Button
            aria-label="Next track"
            class="transport-button"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating"
            @click="emit('next')"
          >
            <SkipForward aria-hidden="true" />
          </Button>
        </div>

        <label class="volume-control">
          <Volume2 aria-hidden="true" />
          <span class="sr-only">Volume</span>
          <input
            aria-label="Volume"
            type="range"
            min="0"
            max="100"
            :value="snapshot.volumePercent"
            :disabled="isUpdating"
            @change="emitVolume"
          />
        </label>
      </div>

      <div class="progress-row">
        <span>{{ formatDuration(snapshot.positionMs) }}</span>
        <input
          aria-label="Track progress"
          type="range"
          min="0"
          :max="durationMs"
          :value="snapshot.positionMs"
          :disabled="isUpdating || durationMs === 0"
          @change="emitSeek"
        />
        <span>-{{ formatDuration(remainingMs) }}</span>
      </div>
    </div>
  </section>
</template>
