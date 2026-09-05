<script setup lang="ts">
import {
  Disc3,
  Heart,
  LoaderCircle,
  PanelRightClose,
  PanelRightOpen,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Repeat2,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-vue-next";
import { computed } from "vue";

import type { MediaItem, PlaybackTransport } from "@/api";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/time";
import YouTubeArtwork from "../YouTubeArtwork.vue";

const props = defineProps<{
  playback: PlaybackTransport;
  currentItem: MediaItem | null;
  isPlaying: boolean;
  isStarting?: boolean;
  isUpdating: boolean;
  detailsOpen: boolean;
  favoriteTrackIds?: string[];
}>();

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [percent: number];
  toggleMute: [];
  toggleShuffle: [];
  cycleRepeatMode: [];
  toggleDetails: [];
  toggleFavorite: [id: string];
}>();

const isFavorite = computed(() =>
  props.currentItem
    ? (props.favoriteTrackIds ?? []).includes(props.currentItem.id)
    : false,
);
const volumeIcon = computed(() => {
  if (props.playback.volumePercent === 0) {
    return VolumeX;
  }
  if (props.playback.volumePercent <= 33) {
    return Volume;
  }
  if (props.playback.volumePercent <= 66) {
    return Volume1;
  }
  return Volume2;
});
const repeatMode = computed(() => props.playback.repeatMode ?? "off");
const repeatIcon = computed(() => {
  if (repeatMode.value === "one") {
    return Repeat1;
  }
  if (repeatMode.value === "all") {
    return Repeat2;
  }
  return Repeat;
});
const repeatLabel = computed(() => {
  if (repeatMode.value === "one") {
    return "Disable repeat";
  }
  if (repeatMode.value === "all") {
    return "Enable repeat one";
  }
  return "Enable repeat all";
});

function emitVolume(values: number[] | undefined): void {
  const value = values?.[0];
  if (typeof value === "number" && Number.isFinite(value)) {
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
  <div class="col-span-3 row-start-2 min-w-0 max-[760px]:col-span-1">
    <footer
      class="flex h-full min-w-0 items-center gap-4 overflow-hidden border-t border-(--line) py-0 pr-4 max-[920px]:grid max-[920px]:grid-cols-[minmax(10rem,1fr)_auto_auto] max-[920px]:grid-rows-2 max-[920px]:gap-x-2 max-[920px]:gap-y-1 max-[920px]:px-3 max-[920px]:py-1.5"
      data-library-playback-footer
    >
      <div
        class="flex min-w-48 flex-1 items-center gap-3 max-[920px]:min-w-0 max-[920px]:gap-2"
        data-playback-control="now-playing"
      >
        <div class="cover-art size-16 shrink-0 max-[920px]:size-10">
          <YouTubeArtwork
            class="absolute inset-0 size-full object-cover"
            :video-id="props.currentItem?.id"
            :missing-icon="Disc3"
          />
        </div>
        <div class="min-w-0">
          <p
            class="overflow-hidden text-[0.82rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ props.currentItem?.title ?? "Nothing playing" }}
          </p>
          <span
            class="mt-0.5 block overflow-hidden text-[0.74rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
          >
            {{ props.currentItem?.artist ?? "Choose a track" }}
          </span>
        </div>
      </div>

      <nav
        class="flex shrink-0 items-center gap-1.5 max-[920px]:gap-1"
        aria-label="Playback controls"
        data-playback-control="transport"
      >
        <Button
          :aria-label="
            props.playback.shuffleEnabled ? 'Disable shuffle' : 'Enable shuffle'
          "
          :aria-pressed="props.playback.shuffleEnabled ?? false"
          class="aria-pressed:text-accent"
          size="icon-sm"
          variant="ghost"
          :disabled="props.isUpdating || props.isStarting"
          @click="emit('toggleShuffle')"
        >
          <Shuffle aria-hidden="true" />
        </Button>
        <Button
          aria-label="Previous track"
          size="icon-sm"
          variant="ghost"
          :disabled="props.isUpdating || props.isStarting || !props.currentItem"
          @click="emit('previous')"
        >
          <SkipBack aria-hidden="true" />
        </Button>
        <Button
          :aria-label="
            props.isStarting
              ? 'Starting playback'
              : props.isPlaying
                ? 'Pause'
                : 'Play'
          "
          :aria-busy="props.isStarting ? 'true' : undefined"
          class="size-10 rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text) disabled:bg-(--text)/50 disabled:opacity-100"
          size="icon"
          :disabled="props.isUpdating || props.isStarting"
          @click="emit('toggle')"
        >
          <LoaderCircle
            v-if="props.isStarting"
            class="animate-spin"
            data-playback-starting
            aria-hidden="true"
          />
          <Pause
            v-else-if="props.isPlaying"
            aria-hidden="true"
            fill="currentColor"
          />
          <Play v-else aria-hidden="true" fill="currentColor" />
        </Button>
        <Button
          aria-label="Next track"
          size="icon-sm"
          variant="ghost"
          :disabled="props.isUpdating || props.isStarting || !props.currentItem"
          @click="emit('next')"
        >
          <SkipForward aria-hidden="true" />
        </Button>
        <Button
          :aria-label="repeatLabel"
          :aria-pressed="repeatMode !== 'off'"
          class="aria-pressed:text-accent"
          :data-repeat-mode="repeatMode"
          size="icon-sm"
          variant="ghost"
          :disabled="props.isUpdating || props.isStarting"
          @click="emit('cycleRepeatMode')"
        >
          <component :is="repeatIcon" aria-hidden="true" />
        </Button>
      </nav>

      <Button
        aria-label="Favorite track"
        :aria-pressed="isFavorite"
        size="icon-sm"
        variant="ghost"
        data-playback-control="favorite"
        :title="isFavorite ? 'Remove from Favorites' : 'Add to Favorites'"
        :disabled="!props.currentItem || props.isUpdating"
        @click="
          props.currentItem && emit('toggleFavorite', props.currentItem.id)
        "
      >
        <Heart
          :fill="isFavorite ? 'currentColor' : 'none'"
          aria-hidden="true"
        />
      </Button>

      <div
        class="grid min-w-52 flex-2 grid-cols-[30px_minmax(8rem,1fr)_30px] items-center gap-2 text-[0.65rem] text-(--muted-text) tabular-nums max-[920px]:min-w-0"
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
          :disabled="props.isUpdating || props.isStarting || !props.currentItem"
          @value-commit="emitSeek"
        />
        <span class="text-right">
          {{ formatDuration(props.currentItem?.durationMs ?? 0) }}
        </span>
      </div>

      <div
        class="flex w-36 shrink-0 items-center gap-2 text-(--muted-text) [&>svg]:size-4"
        data-playback-control="volume"
      >
        <Button
          :aria-label="
            props.playback.volumePercent === 0 ? 'Unmute volume' : 'Mute volume'
          "
          size="icon-sm"
          variant="ghost"
          :disabled="props.isUpdating"
          @click="emit('toggleMute')"
        >
          <component :is="volumeIcon" aria-hidden="true" />
        </Button>
        <Slider
          aria-label="Volume"
          :min="0"
          :max="100"
          :step="1"
          :model-value="[props.playback.volumePercent]"
          :disabled="props.isUpdating"
          @update:model-value="emitVolume"
        />
      </div>

      <Button
        :aria-label="
          props.detailsOpen
            ? 'Hide selection details'
            : 'Show selection details'
        "
        :aria-pressed="props.detailsOpen"
        size="icon-sm"
        variant="ghost"
        data-playback-control="details"
        @click="emit('toggleDetails')"
      >
        <PanelRightClose v-if="props.detailsOpen" aria-hidden="true" />
        <PanelRightOpen v-else aria-hidden="true" />
      </Button>
    </footer>
  </div>
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
