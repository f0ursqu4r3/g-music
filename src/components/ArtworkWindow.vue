<script setup lang="ts">
import {
  Disc3,
  Heart,
  Library,
  ListMusic,
  LoaderCircle,
  Pause,
  Play,
  Repeat2,
  Repeat1,
  Repeat,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
} from 'lucide-vue-next'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import type { PlaybackSnapshot } from '@/api'
import { Button } from '@/components/ui/button'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { Slider } from '@/components/ui/slider'
import { formatDuration } from '@/lib/time'
import AutoScrollText from './AutoScrollText.vue'
import YouTubeArtwork from './YouTubeArtwork.vue'

interface Props {
  snapshot: PlaybackSnapshot
  isUpdating: boolean
  isWindowFocused?: boolean
  isCursorWithinWindow?: boolean | null
  isStarting?: boolean
  favoriteTrackIds?: string[]
}

const props = withDefaults(defineProps<Props>(), {
  isWindowFocused: true,
  isCursorWithinWindow: null,
  isStarting: false,
  favoriteTrackIds: () => [],
})
const emit = defineEmits<{
  toggle: []
  previous: []
  next: []
  seek: [positionMs: number]
  setVolume: [volumePercent: number]
  toggleMute: []
  openQueue: []
  openLibrary: []
  toggleFavorite: [id: string]
  toggleShuffle: []
  cycleRepeatMode: []
}>()

const isPlaying = computed(() => props.snapshot.status === 'playing')
const actionMotionClass =
  'transition-[color,background-color,box-shadow,scale] duration-150 ease-out motion-safe:hover:scale-105 motion-safe:active:scale-95 motion-reduce:transition-none'
const sliderMotionClass =
  '[&_[data-slot=slider-thumb]]:duration-600 [&:is(:hover,:focus-within)_[data-slot=slider-thumb]]:duration-300 [&_[data-slot=slider-thumb]]:motion-reduce:transition-none'
const actionClass = `${actionMotionClass} text-(--muted-text) hover:bg-(--text)/10 hover:text-(--text) aria-pressed:text-accent aria-pressed:hover:text-accent`
const currentItem = computed(() => props.snapshot.currentItem)
const stateLabel = computed(() =>
  props.isStarting
    ? 'Starting playback'
    : !currentItem.value
      ? 'Nothing playing'
      : isPlaying.value
        ? 'Playing'
        : 'Paused',
)
const playLabel = computed(() =>
  props.isStarting ? 'Starting playback' : isPlaying.value ? 'Pause' : 'Play',
)
const isFavorite = computed(() =>
  currentItem.value ? props.favoriteTrackIds.includes(currentItem.value.id) : false,
)
const favoriteLabel = computed(() =>
  isFavorite.value ? 'Remove from Favorites' : 'Add to Favorites',
)
const shuffleLabel = computed(() =>
  props.snapshot.shuffleEnabled ? 'Disable shuffle' : 'Enable shuffle',
)
const muteLabel = computed(() =>
  props.snapshot.volumePercent === 0 ? 'Unmute volume' : 'Mute volume',
)
const repeatMode = computed(() => props.snapshot.repeatMode ?? 'off')
const repeatLabel = computed(() =>
  repeatMode.value === 'one'
    ? 'Disable repeat'
    : repeatMode.value === 'all'
      ? 'Enable repeat one'
      : 'Enable repeat all',
)
const repeatIcon = computed(() =>
  repeatMode.value === 'one' ? Repeat1 : repeatMode.value === 'all' ? Repeat2 : Repeat,
)

function clamp(value: number, max: number): number {
  return Number.isFinite(value) ? Math.min(Math.max(value, 0), max) : 0
}
const durationMs = computed(() => {
  const value = currentItem.value?.durationMs ?? 0
  return Number.isFinite(value) ? Math.max(0, value) : 0
})
const compactProgressMs = computed(() => clamp(props.snapshot.positionMs, durationMs.value))
const volumePercent = computed(() => clamp(props.snapshot.volumePercent, 100))
const seekDisabled = computed(() => props.isUpdating || props.isStarting || durationMs.value === 0)
const seekPreview = ref<number | null>(null)
const seekGeneration = ref(0)
const displayedPosition = computed(() => seekPreview.value ?? compactProgressMs.value)
let seekKeyboardActive = false

function seekKeydown(event: KeyboardEvent): void {
  if (
    seekDisabled.value ||
    ![
      'ArrowLeft',
      'ArrowRight',
      'ArrowUp',
      'ArrowDown',
      'Home',
      'End',
      'PageUp',
      'PageDown',
    ].includes(event.key)
  )
    return
  // Reka commits keyboard changes before it emits update:modelValue.
  seekKeyboardActive = true
}

// Only the preview is local. The parent snapshot remains the playback authority.
function previewSeek(values: number[] | undefined): void {
  if (seekKeyboardActive) {
    seekKeyboardActive = false
    return
  }
  const value = values?.[0]
  if (!seekDisabled.value && typeof value === 'number' && Number.isFinite(value)) {
    seekPreview.value = clamp(value, durationMs.value)
  }
}
function commitSeek(values: number[]): void {
  const value = values[0]
  if (
    (seekPreview.value === null && !seekKeyboardActive) ||
    seekDisabled.value ||
    !Number.isFinite(value)
  )
    return
  seekPreview.value = null
  emit('seek', clamp(value, durationMs.value))
}
function cancelSeek(): void {
  seekKeyboardActive = false
  seekPreview.value = null

  // Remount the primitive to discard pointer capture and its internal drag value.
  seekGeneration.value += 1
}
function emitVolume(values: number[] | undefined): void {
  const value = values?.[0]
  if (!props.isUpdating && typeof value === 'number' && Number.isFinite(value)) {
    emit('setVolume', clamp(value, 100))
  }
}

const controls = ref<HTMLElement>()
const windowHovered = ref(false)
const frameHovered = computed(() => props.isCursorWithinWindow ?? windowHovered.value)
const playbackControlsVisible = computed(() => props.isWindowFocused)
const menuOpen = ref(false)
const controlsVisible = computed(
  () => playbackControlsVisible.value || frameHovered.value || menuOpen.value,
)
function pointerEnter(): void {
  windowHovered.value = true
}
function pointerMove(): void {
  windowHovered.value = true
}
function pointerLeave(): void {
  windowHovered.value = false
}
function pointerCancel(): void {
  cancelSeek()
}
function keydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') cancelSeek()
  if (playbackControlsVisible.value) {
    // Tab's default action runs before Vue patches the DOM. Remove inert now so
    // the first Tab can reach a control. Do not consume or replace global hotkeys.
    controls.value?.removeAttribute('inert')
    controls.value?.removeAttribute('aria-hidden')
  }
}
function blur(): void {
  cancelSeek()
}
watch(
  () => props.isWindowFocused,
  (focused) => {
    if (!focused) cancelSeek()
  },
)
watch(() => currentItem.value?.id, cancelSeek)
watch(seekDisabled, (disabled) => {
  // A completed keyboard seek has no draft to cancel. Keep its thumb mounted
  // through the pending command so subsequent keys retain their focus target.
  if (disabled && seekPreview.value !== null) cancelSeek()
})

onMounted(() => {
  window.addEventListener('keydown', keydown, true)

  window.addEventListener('pointercancel', pointerCancel)
  window.addEventListener('blur', blur)
})
onUnmounted(() => {
  window.removeEventListener('keydown', keydown, true)

  window.removeEventListener('pointercancel', pointerCancel)
  window.removeEventListener('blur', blur)
})
</script>

<template>
  <main
    class="artwork-window window-shell relative flex h-screen min-h-0 flex-col justify-end overflow-hidden bg-(--canvas) text-(--text)"
    aria-label="Current artwork"
    @pointerenter="pointerEnter"
    @pointermove="pointerMove"
    @pointerleave="pointerLeave"
    @pointercancel="pointerCancel"
  >
    <div
      class="artwork-cover pointer-events-none absolute inset-0 overflow-hidden"
      data-cover="violet"
      aria-hidden="true"
    >
      <YouTubeArtwork
        class="absolute inset-0 size-full object-cover"
        :video-id="currentItem?.id"
        :missing-icon="Disc3"
      />
    </div>

    <ContextMenu v-model:open="menuOpen" :modal="false">
      <ContextMenuTrigger as-child>
        <div
          class="artwork-drag-region absolute inset-0 z-10 cursor-grab active:cursor-grabbing"
          data-tauri-drag-region
          title="Drag to move Artwork. Right-click for track and window actions."
          aria-hidden="true"
        />
      </ContextMenuTrigger>
      <ContextMenuContent
        class="duration-300 data-closed:duration-600 motion-reduce:animate-none"
        data-artwork-context-menu
        @close-auto-focus.prevent
      >
        <ContextMenuItem
          :disabled="!currentItem || isUpdating"
          @select="currentItem && emit('toggleFavorite', currentItem.id)"
        >
          {{ favoriteLabel }}
        </ContextMenuItem>
        <ContextMenuItem @select="emit('openQueue')">Open queue</ContextMenuItem>
        <ContextMenuItem @select="emit('openLibrary')">Open library</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>

    <section
      class="artwork-controls artwork-information-gradient pointer-events-none relative z-10 shrink-0 px-5 pt-8 pb-4 transition-[opacity,translate] duration-(--artwork-transition-durations) ease-(--artwork-ease) motion-reduce:transition-none data-[playback-visible=false]:translate-y-34 data-[visible=false]:opacity-0"
      aria-label="Artwork playback controls"
      :data-visible="controlsVisible"
      :data-playback-visible="playbackControlsVisible"
    >
      <header class="artwork-metadata min-w-0" :aria-hidden="controlsVisible ? undefined : 'true'">
        <p class="mb-1 text-xs text-(--muted-text)" data-playback-state>{{ stateLabel }}</p>
        <AutoScrollText
          as="h1"
          class="text-2xl font-semibold tracking-tight"
          speed="slow"
          :text="currentItem?.title ?? 'Nothing playing'"
        />
        <AutoScrollText
          as="p"
          class="mt-0.5 text-sm text-(--muted-text)"
          speed="medium"
          :text="currentItem?.artist ?? 'Choose a track to begin'"
        />
      </header>

      <div
        ref="controls"
        class="artwork-playback-controls pointer-events-auto h-34 pt-3 transition-[opacity,translate] duration-300 ease-(--artwork-ease) motion-reduce:transition-none data-[visible=false]:pointer-events-none data-[visible=false]:translate-y-2 data-[visible=false]:opacity-0 data-[visible=false]:duration-1200"
        :data-visible="playbackControlsVisible"
        :data-window-focused="isWindowFocused"
        :aria-hidden="playbackControlsVisible ? undefined : 'true'"
        :inert="!playbackControlsVisible"
      >
        <div aria-label="Track progress">
          <Slider
            :key="seekGeneration"
            class="h-4"
            :class="sliderMotionClass"
            aria-label="Track progress"
            thumb-alignment="overflow"
            :aria-valuetext="formatDuration(displayedPosition)"
            :min="0"
            :max="durationMs || 1"
            :step="1000"
            :model-value="[displayedPosition]"
            :disabled="seekDisabled"
            @keydown.capture="seekKeydown"
            @keyup="seekKeyboardActive = false"
            @pointerdown.capture="seekKeyboardActive = false"
            @update:model-value="previewSeek"
            @value-commit="commitSeek"
          />
          <div
            class="flex justify-between gap-3 text-xs whitespace-nowrap text-(--muted-text) tabular-nums"
            data-progress-times
          >
            <span
              :data-seek-preview="seekPreview !== null ? '' : undefined"
              :class="{ 'text-accent': seekPreview !== null }"
            >
              {{ formatDuration(displayedPosition) }}
            </span>
            <span>{{ formatDuration(durationMs) }}</span>
          </div>
        </div>

        <nav class="mt-2 flex items-center justify-center gap-2" aria-label="Playback controls">
          <Button
            :aria-label="shuffleLabel"
            :title="shuffleLabel"
            :aria-pressed="snapshot.shuffleEnabled ?? false"
            :class="actionClass"
            :disabled="isUpdating || isStarting"
            size="icon-sm"
            variant="ghost"
            @click="emit('toggleShuffle')"
          >
            <Shuffle aria-hidden="true" />
          </Button>
          <Button
            :class="actionClass"
            aria-label="Previous track"
            title="Previous track"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating || isStarting || !currentItem"
            @click="emit('previous')"
          >
            <SkipBack aria-hidden="true" />
          </Button>
          <Button
            :aria-label="playLabel"
            :title="playLabel"
            :aria-busy="isStarting ? 'true' : undefined"
            :class="actionMotionClass"
            class="size-10 rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text) disabled:bg-(--text)/50 disabled:opacity-100"
            size="icon"
            :disabled="isUpdating || isStarting || (!currentItem && snapshot.queue.length === 0)"
            @click="emit('toggle')"
          >
            <LoaderCircle
              v-if="isStarting"
              class="animate-spin motion-reduce:animate-none"
              data-playback-starting
              aria-hidden="true"
            />
            <Pause v-else-if="isPlaying" aria-hidden="true" fill="currentColor" />
            <Play v-else aria-hidden="true" fill="currentColor" />
          </Button>
          <Button
            :class="actionClass"
            aria-label="Next track"
            title="Next track"
            size="icon-sm"
            variant="ghost"
            :disabled="isUpdating || isStarting || !currentItem"
            @click="emit('next')"
          >
            <SkipForward aria-hidden="true" />
          </Button>
          <Button
            :aria-label="repeatLabel"
            :title="repeatLabel"
            :aria-pressed="repeatMode !== 'off'"
            :data-repeat-mode="repeatMode"
            :class="actionClass"
            :disabled="isUpdating || isStarting"
            size="icon-sm"
            variant="ghost"
            @click="emit('cycleRepeatMode')"
          >
            <component :is="repeatIcon" aria-hidden="true" />
          </Button>
        </nav>

        <div class="mt-2 flex items-center justify-between gap-3">
          <div class="flex w-28 min-w-0 items-center gap-2">
            <Button
              :aria-label="muteLabel"
              :title="muteLabel"
              :class="actionClass"
              size="icon-sm"
              variant="ghost"
              :disabled="isUpdating"
              @click="emit('toggleMute')"
            >
              <component :is="volumePercent === 0 ? VolumeX : Volume2" aria-hidden="true" />
            </Button>
            <Slider
              class="h-7"
              :class="sliderMotionClass"
              aria-label="Volume"
              thumb-alignment="overflow"
              :min="0"
              :max="100"
              :step="1"
              :model-value="[volumePercent]"
              :disabled="isUpdating"
              @update:model-value="emitVolume"
            />
          </div>
          <div class="flex items-center gap-1">
            <Button
              :class="actionClass"
              aria-label="Favorite track"
              :aria-pressed="isFavorite"
              :title="favoriteLabel"
              size="icon-sm"
              variant="ghost"
              :disabled="!currentItem || isUpdating"
              @click="currentItem && emit('toggleFavorite', currentItem.id)"
            >
              <Heart :fill="isFavorite ? 'currentColor' : 'none'" aria-hidden="true" />
            </Button>
            <Button
              :class="actionClass"
              aria-label="Open queue"
              title="Open queue"
              size="icon-sm"
              variant="ghost"
              @click="emit('openQueue')"
            >
              <ListMusic aria-hidden="true" />
            </Button>
            <Button
              :class="actionClass"
              aria-label="Open library"
              title="Open library"
              size="icon-sm"
              variant="ghost"
              @click="emit('openLibrary')"
            >
              <Library aria-hidden="true" />
            </Button>
          </div>
        </div>
      </div>
    </section>

    <progress
      v-if="currentItem && durationMs > 0"
      class="artwork-unfocused-progress pointer-events-none absolute right-0 bottom-0 left-0 z-20 block h-0.5 w-full overflow-hidden border-0 transition-opacity duration-300 ease-out motion-reduce:transition-none data-[visible=false]:opacity-0 data-[visible=false]:duration-1200"
      :data-visible="!playbackControlsVisible"
      :aria-hidden="playbackControlsVisible ? 'true' : undefined"
      aria-label="Track progress"
      :value="compactProgressMs"
      :max="durationMs"
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
    linear-gradient(148deg, var(--artwork-a), var(--artwork-b) 54%, var(--artwork-c));
}

.artwork-information-gradient::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: -1;

  background: linear-gradient(
    180deg,
    transparent 0%,
    oklch(0.09 0.025 260 / 0.8) 28%,
    oklch(0.075 0.025 260 / 0.96) 100%
  );
}

.artwork-controls {
  --artwork-ease: cubic-bezier(0.22, 1, 0.36, 1);
  --artwork-fade-duration: 300ms;
  --artwork-slide-duration: 300ms;
  --artwork-transition-durations: var(--artwork-fade-duration), var(--artwork-slide-duration);
  --text: oklch(0.98 0.005 270);
  --muted-text: oklch(0.85 0.012 270);
  --accent-ink: oklch(0.12 0.015 270);
  text-shadow: 0 1px 12px oklch(0.05 0.02 260 / 0.45);
}

.artwork-controls[data-visible='false'] {
  --artwork-fade-duration: 1200ms;
}

.artwork-controls[data-playback-visible='false'] {
  --artwork-slide-duration: 1200ms;
}

.artwork-unfocused-progress {
  appearance: none;
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
