<script setup lang="ts">
import {
  Disc3,
  Heart,
  LoaderCircle,
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
} from 'lucide-vue-next'
import { computed } from 'vue'

import type { PlaybackSnapshot } from '@/api'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { formatDuration } from '@/lib/time'
import YouTubeArtwork from './YouTubeArtwork.vue'

interface Props {
  snapshot: PlaybackSnapshot
  isUpdating: boolean
  isStarting?: boolean
  favoriteTrackIds?: string[]
}

const props = withDefaults(defineProps<Props>(), {
  favoriteTrackIds: () => [],
  isStarting: false,
})

const emit = defineEmits<{
  toggle: []
  previous: []
  next: []
  seek: [positionMs: number]
  setVolume: [volumePercent: number]
  toggleMute: []
  toggleShuffle: []
  cycleRepeatMode: []
  toggleFavorite: [id: string]
}>()

const currentItem = computed(() => props.snapshot.currentItem)
const durationMs = computed(() => currentItem.value?.durationMs ?? 0)
const isPlaying = computed(() => props.snapshot.status === 'playing')
const trackTitle = computed(() => currentItem.value?.title ?? 'Nothing playing')
const trackArtist = computed(() => currentItem.value?.artist ?? 'Choose a track to begin')
const remainingMs = computed(() => Math.max(durationMs.value - props.snapshot.positionMs, 0))
const isFavorite = computed(() =>
  currentItem.value ? props.favoriteTrackIds.includes(currentItem.value.id) : false,
)
const repeatMode = computed(() => props.snapshot.repeatMode ?? 'off')
const repeatIcon = computed(() => {
  if (repeatMode.value === 'one') {
    return Repeat1
  }
  if (repeatMode.value === 'all') {
    return Repeat2
  }
  return Repeat
})
const repeatLabel = computed(() => {
  if (repeatMode.value === 'one') {
    return 'Disable repeat'
  }
  if (repeatMode.value === 'all') {
    return 'Enable repeat one'
  }
  return 'Enable repeat all'
})
const volumeIcon = computed(() => {
  if (props.snapshot.volumePercent === 0) {
    return VolumeX
  }
  if (props.snapshot.volumePercent <= 33) {
    return Volume
  }
  if (props.snapshot.volumePercent <= 66) {
    return Volume1
  }
  return Volume2
})

function emitSeek(values: number[]): void {
  const value = values[0]
  if (Number.isFinite(value)) {
    emit('seek', value)
  }
}

function emitVolume(values: number[] | undefined): void {
  const value = values?.[0]
  if (typeof value === 'number' && Number.isFinite(value)) {
    emit('setVolume', value)
  }
}
</script>

<template>
  <section
    class="mini-player-shell relative grid min-h-36 grid-cols-[112px_minmax(0,1fr)] max-[390px]:grid-cols-[96px_minmax(0,1fr)]"
    aria-label="Now playing"
  >
    <div
      class="absolute top-0 right-0 left-0 z-3 grid h-7"
      data-tauri-drag-region
      title="Drag to move mini player"
      aria-hidden="true"
    ></div>

    <div
      class="album-art relative grid place-items-center overflow-hidden border-r border-(--line)"
      aria-hidden="true"
    >
      <YouTubeArtwork
        class="absolute inset-0 size-full object-cover"
        :video-id="currentItem?.id"
        :missing-icon="Disc3"
      />
    </div>

    <div class="grid min-w-0 grid-rows-[auto_1fr_auto] px-3.5 py-3 max-[390px]:px-3">
      <header class="min-w-0 pr-14">
        <div class="min-w-0">
          <h1
            class="overflow-hidden text-[1rem] leading-[1.1] font-[680] tracking-[-0.04em] text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ trackTitle }}
          </h1>
          <p
            class="mt-0.75 overflow-hidden text-[0.7rem] font-[510] tracking-[-0.005em] text-ellipsis whitespace-nowrap text-(--muted-text)"
          >
            {{ trackArtist }}
          </p>
        </div>
      </header>

      <div class="flex items-end justify-between gap-3 pt-2">
        <nav class="flex items-center gap-0.5" aria-label="Playback controls">
          <Button
            :aria-label="snapshot.shuffleEnabled ? 'Disable shuffle' : 'Enable shuffle'"
            :aria-pressed="snapshot.shuffleEnabled ?? false"
            class="size-6 text-(--muted-text) aria-pressed:text-accent hover:bg-(--surface-muted) hover:text-(--text) [&_svg]:size-3.5"
            size="icon-xs"
            variant="ghost"
            :disabled="isUpdating || isStarting"
            @click="emit('toggleShuffle')"
          >
            <Shuffle aria-hidden="true" />
          </Button>
          <Button
            aria-label="Previous track"
            class="size-7 text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) [&_svg]:size-4"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating || isStarting || !currentItem"
            @click="emit('previous')"
          >
            <SkipBack aria-hidden="true" />
          </Button>
          <Button
            :aria-label="isStarting ? 'Starting playback' : isPlaying ? 'Pause' : 'Play'"
            :aria-busy="isStarting ? 'true' : undefined"
            class="size-10 rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text) disabled:bg-(--text)/50 disabled:opacity-100 [&_svg]:size-4"
            size="icon"
            :disabled="isUpdating || isStarting || (!currentItem && snapshot.queue.length === 0)"
            @click="emit('toggle')"
          >
            <LoaderCircle
              v-if="isStarting"
              class="animate-spin"
              data-playback-starting
              aria-hidden="true"
            />
            <Pause v-else-if="isPlaying" aria-hidden="true" fill="currentColor" />
            <Play v-else aria-hidden="true" fill="currentColor" />
          </Button>
          <Button
            aria-label="Next track"
            class="size-7 text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) [&_svg]:size-4"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating || isStarting || !currentItem"
            @click="emit('next')"
          >
            <SkipForward aria-hidden="true" />
          </Button>
          <Button
            :aria-label="repeatLabel"
            :aria-pressed="repeatMode !== 'off'"
            :data-repeat-mode="repeatMode"
            class="size-6 text-(--muted-text) aria-pressed:text-accent hover:bg-(--surface-muted) hover:text-(--text) [&_svg]:size-3.5"
            size="icon-xs"
            variant="ghost"
            :disabled="isUpdating || isStarting"
            @click="emit('cycleRepeatMode')"
          >
            <component :is="repeatIcon" aria-hidden="true" />
          </Button>
          <Button
            aria-label="Favorite track"
            :title="isFavorite ? 'Remove from Favorites' : 'Add to Favorites'"
            :aria-pressed="isFavorite"
            class="size-6 text-(--muted-text) aria-pressed:text-accent hover:bg-(--surface-muted) hover:text-(--text) [&_svg]:size-3.5"
            size="icon-xs"
            variant="ghost"
            :disabled="!currentItem || isUpdating"
            @click="currentItem && emit('toggleFavorite', currentItem.id)"
          >
            <Heart :fill="isFavorite ? 'currentColor' : 'none'" aria-hidden="true" />
          </Button>
        </nav>

        <div
          class="grid w-24 grid-cols-[14px_minmax(0,1fr)_18px] items-center gap-1.5 text-(--muted-text) max-[390px]:hidden"
        >
          <Button
            :aria-label="snapshot.volumePercent === 0 ? 'Unmute volume' : 'Mute volume'"
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
        class="grid grid-cols-[28px_minmax(0,1fr)_32px] items-center gap-1.5 pt-2 text-[0.58rem] text-(--subtle-text) tabular-nums [&>span:last-child]:text-right"
      >
        <span>{{ formatDuration(snapshot.positionMs) }}</span>
        <Slider
          aria-label="Track progress"
          :min="0"
          :max="durationMs"
          :step="1000"
          :model-value="[snapshot.positionMs]"
          :disabled="isUpdating || isStarting || durationMs === 0"
          @value-commit="emitSeek"
        />
        <span>-{{ formatDuration(remainingMs) }}</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.album-art {
  background: linear-gradient(136deg, var(--artwork-a), var(--artwork-b) 55%, var(--artwork-c));
}
</style>
