<script setup lang="ts">
import {
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
import { formatDuration } from "@/lib/time";

interface Props {
  snapshot: PlaybackSnapshot;
  isUpdating: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{ toggle: []; previous: []; next: [] }>();

const isPlaying = computed(() => props.snapshot.status === "playing");
const currentItem = computed(() => props.snapshot.currentItem);
const remainingMs = computed(() =>
  Math.max((currentItem.value?.durationMs ?? 0) - props.snapshot.positionMs, 0),
);
</script>

<template>
  <main
    class="artwork-window grid min-h-screen grid-rows-[minmax(0,1fr)_auto] bg-[linear-gradient(140deg,oklch(0.31_0.045_246/0.12),transparent_38%),var(--glass-window)] text-(--text) backdrop-saturate-[1.16]"
    aria-label="Current artwork"
  >
    <div
      class="artwork-cover relative mx-8 mt-8 min-h-0 overflow-hidden rounded-[15px]"
      data-cover="violet"
    >
      <span
        class="absolute inset-[12%] rounded-[10px] border border-[oklch(0.98_0.01_90/0.3)]"
      />
      <span
        class="absolute right-[-18%] bottom-[6%] h-[30%] w-[72%] rotate-[-26deg] rounded-full border border-[oklch(0.98_0.01_90/0.42)]"
      />
      <span
        class="absolute right-6 bottom-5.5 text-[0.61rem] font-bold tracking-[0.15em] text-[oklch(0.98_0.01_90/0.82)]"
        >G MUSIC</span
      >
    </div>

    <section class="px-8 pt-6 pb-7">
      <header>
        <p
          class="text-[0.58rem] font-[720] tracking-widest text-(--subtle-text) uppercase"
        >
          Now playing
        </p>
        <div class="flex items-center justify-between gap-4">
          <div class="min-w-0">
            <h1
              class="mt-1.25 overflow-hidden text-[1.34rem] font-bold tracking-[-0.045em] text-ellipsis whitespace-nowrap"
            >
              {{ currentItem?.title ?? "Nothing selected" }}
            </h1>
            <p
              class="mt-1 overflow-hidden text-[0.76rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
            >
              {{ currentItem?.artist ?? "Choose a track to begin" }}
            </p>
          </div>
          <div class="flex items-center gap-0.5">
            <Button aria-label="Favorite track" size="icon-sm" variant="ghost"
              ><Heart aria-hidden="true"
            /></Button>
            <Button aria-label="More actions" size="icon-sm" variant="ghost"
              ><MoreHorizontal aria-hidden="true"
            /></Button>
          </div>
        </div>
      </header>

      <nav
        class="flex items-center justify-center gap-3 pt-5.75 pb-4.5"
        aria-label="Playback controls"
      >
        <Button aria-label="Shuffle" size="icon-sm" variant="ghost"
          ><Shuffle aria-hidden="true"
        /></Button>
        <Button
          aria-label="Previous track"
          size="icon-sm"
          variant="ghost"
          :disabled="isUpdating"
          @click="emit('previous')"
          ><SkipBack aria-hidden="true"
        /></Button>
        <Button
          :aria-label="isPlaying ? 'Pause' : 'Play'"
          class="rounded-full bg-accent text-(--accent-ink)"
          size="icon"
          :disabled="isUpdating"
          @click="emit('toggle')"
        >
          <Pause v-if="isPlaying" aria-hidden="true" />
          <Play v-else aria-hidden="true" fill="currentColor" />
        </Button>
        <Button
          aria-label="Next track"
          size="icon-sm"
          variant="ghost"
          :disabled="isUpdating"
          @click="emit('next')"
          ><SkipForward aria-hidden="true"
        /></Button>
        <Button aria-label="Repeat" size="icon-sm" variant="ghost"
          ><Repeat2 aria-hidden="true"
        /></Button>
      </nav>

      <div
        class="grid grid-cols-[32px_minmax(0,1fr)_36px] items-center gap-2 text-[0.61rem] text-(--subtle-text) tabular-nums"
      >
        <span>{{ formatDuration(snapshot.positionMs) }}</span>
        <input
          class="pointer-events-none"
          aria-label="Track progress"
          type="range"
          min="0"
          :max="currentItem?.durationMs ?? 0"
          :value="snapshot.positionMs"
          disabled
        />
        <span>-{{ formatDuration(remainingMs) }}</span>
      </div>
    </section>
  </main>
</template>

<style scoped>
.artwork-cover {
  background: linear-gradient(
    138deg,
    var(--artwork-a),
    var(--artwork-b) 58%,
    var(--artwork-c)
  );
}

.artwork-cover::before {
  position: absolute;
  top: 16%;
  left: 16%;
  width: 62%;
  aspect-ratio: 1;
  content: "";
  border: clamp(18px, 5vw, 46px) solid oklch(0.98 0.01 90 / 0.15);
  border-radius: 50%;
}

.artwork-cover::after {
  position: absolute;
  inset: 15%;
  content: "";
  border: 1px solid oklch(0.98 0.01 90 / 0.35);
  border-radius: inherit;
  transform: rotate(-18deg);
}
</style>
