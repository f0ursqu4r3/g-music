<script setup lang="ts">
import {
  Heart,
  PanelRightClose,
  PanelRightOpen,
  Pause,
  Play,
  Repeat2,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
} from 'lucide-vue-next';
import { ref } from 'vue';

import type { MediaItem, PlaybackTransport } from '@/api';
import { Button } from '@/components/ui/button';
import { Slider } from '@/components/ui/slider';
import { formatDuration } from '@/lib/time';
import YouTubeArtwork from '../YouTubeArtwork.vue';

const props = defineProps<{
  playback: PlaybackTransport;
  currentItem: MediaItem | null;
  isPlaying: boolean;
  isUpdating: boolean;
  queueOpen: boolean;
}>();

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [percent: number];
  toggleQueue: [];
}>();

const isFavorite = ref(false);

function emitVolume(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit('setVolume', value);
  }
}

function emitSeek(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit('seek', value);
  }
}
</script>

<template>
  <footer
    class="col-span-3 row-start-2 flex min-w-0 items-center gap-4 overflow-y-hidden overflow-x-auto border-t border-(--line) pr-4 py-0 max-[760px]:col-span-1"
    data-library-playback-footer
  >
    <div
      class="flex min-w-48 flex-1 items-center gap-3"
      data-playback-control="now-playing"
    >
      <div class="cover-art size-16 shrink-0">
        <YouTubeArtwork
          class="absolute inset-0 size-full object-cover"
          :video-id="props.currentItem?.id"
        />
      </div>
      <div class="min-w-0">
        <p
          class="overflow-hidden text-[0.82rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
        >
          {{ props.currentItem?.title ?? 'Nothing selected' }}
        </p>
        <span
          class="mt-0.5 block overflow-hidden text-[0.74rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
        >
          {{ props.currentItem?.artist ?? 'Choose a track' }}
        </span>
      </div>
    </div>

    <nav
      class="flex shrink-0 items-center gap-1.5"
      aria-label="Playback controls"
      data-playback-control="transport"
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
        <Pause v-if="props.isPlaying" aria-hidden="true" fill="currentColor" />
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

    <Button
      aria-label="Favorite track"
      :aria-pressed="isFavorite"
      size="icon-sm"
      variant="ghost"
      data-playback-control="favorite"
      @click="isFavorite = !isFavorite"
    >
      <Heart :fill="isFavorite ? 'currentColor' : 'none'" aria-hidden="true" />
    </Button>

    <div
      class="grid min-w-52 flex-2 grid-cols-[30px_minmax(8rem,1fr)_30px] items-center gap-2 text-[0.65rem] text-(--muted-text) tabular-nums"
      aria-label="Track progress"
      data-playback-control="progress"
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

    <label
      class="flex w-36 shrink-0 items-center gap-2 text-(--muted-text) [&>svg]:size-4"
      data-playback-control="volume"
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

    <Button
      :aria-label="props.queueOpen ? 'Hide queue' : 'Show queue'"
      :aria-pressed="props.queueOpen"
      size="icon-sm"
      variant="ghost"
      data-playback-control="queue"
      @click="emit('toggleQueue')"
    >
      <PanelRightClose v-if="props.queueOpen" aria-hidden="true" />
      <PanelRightOpen v-else aria-hidden="true" />
    </Button>
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
