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
  <section
    class="mini-player-shell relative grid min-h-43.5 grid-cols-[164px_minmax(0,1fr)] max-[390px]:grid-cols-[138px_minmax(0,1fr)]"
    aria-label="Now playing"
  >
    <div
      class="mini-drag-handle absolute top-1.75 right-26 left-47.5 z-3 grid min-h-3.25 cursor-grab auto-rows-0.75 grid-cols-[repeat(3,3px)] place-content-center gap-0.75 active:cursor-grabbing [&>span]:size-0.75 [&>span]:rounded-full [&>span]:bg-(--subtle-text) [&>span]:opacity-70"
      data-tauri-drag-region
      title="Drag to move mini player"
      aria-hidden="true"
    >
      <span v-for="dot in 9" :key="dot" />
    </div>

    <div
      class="album-art relative grid place-items-center overflow-hidden"
      aria-hidden="true"
    >
      <span
        class="absolute top-6.75 left-5.25 aspect-square w-24 rounded-full border-17 border-[oklch(0.98_0.01_90/0.17)]"
      />
      <span
        class="absolute top-10.75 -right-6 h-14.5 w-29.5 rotate-[-26deg] bg-[oklch(0.96_0.012_90/0.18)]"
      />
      <span
        class="absolute top-19.5 left-6.5 h-13 w-31.5 rotate-[-27deg] rounded-full border border-[oklch(0.98_0.01_90/0.38)]"
      />
      <div
        class="absolute right-4.5 bottom-4 grid gap-px text-right text-[oklch(0.98_0.01_90/0.84)]"
      >
        <span class="text-[0.52rem] font-bold tracking-[0.14em]">G MUSIC</span>
        <strong class="text-[1.15rem] font-[650] tracking-[-0.08em]">01</strong>
      </div>
    </div>

    <div
      class="grid min-w-0 grid-rows-[auto_1fr_auto] pt-5 pr-11.5 pb-3.5 pl-5 max-[390px]:pl-4"
    >
      <header class="min-w-0">
        <div class="min-w-0">
          <p
            class="mb-1.5 flex items-center gap-1.5 text-[0.59rem] font-[720] tracking-widest text-(--muted-text) uppercase"
          >
            <span
              class="size-1.5 rounded-full bg-(--subtle-text) data-[playing=true]:bg-accent data-[playing=true]:shadow-[0_0_0_4px_var(--accent-soft)]"
              :data-playing="isPlaying"
            />
            {{ isPlaying ? "Playing" : "Paused" }}
            <span
              class="track-source ml-auto inline-flex items-center gap-1.5 text-[0.51rem] tracking-[0.08em] text-(--subtle-text) max-[390px]:hidden"
              >Local session</span
            >
          </p>
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

        <label
          class="grid w-27.5 grid-cols-[14px_minmax(0,1fr)_20px] items-center gap-1.75 text-(--muted-text) max-[390px]:w-21.5"
        >
          <Volume2 class="size-3.5" aria-hidden="true" />
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
          <span
            class="text-right text-[0.6rem] text-(--subtle-text) tabular-nums max-[390px]:hidden"
            aria-hidden="true"
            >{{ snapshot.volumePercent }}</span
          >
        </label>
      </div>

      <div
        class="grid grid-cols-[30px_minmax(0,1fr)_34px] items-center gap-2 text-[0.62rem] text-(--subtle-text) tabular-nums [&>span:last-child]:text-right"
      >
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

<style scoped>
.album-art {
  background: linear-gradient(
    136deg,
    var(--artwork-a),
    var(--artwork-b) 55%,
    var(--artwork-c)
  );
}

.album-art::before,
.album-art::after {
  position: absolute;
  content: "";
  pointer-events: none;
}

.album-art::before {
  inset: 12px;
  border: 1px solid oklch(0.98 0.01 90 / 0.22);
  border-radius: 14px;
}

.album-art::after {
  right: -48px;
  bottom: -56px;
  width: 150px;
  height: 150px;
  border: 1px solid oklch(0.98 0.01 90 / 0.3);
  border-radius: 50%;
}

.track-source::before {
  width: 14px;
  height: 1px;
  content: "";
  background: var(--line-strong);
}
</style>
