<script setup lang="ts">
import { Disc3 } from "lucide-vue-next";

import type { MediaItem } from "@/api";
import YouTubeArtwork from "../YouTubeArtwork.vue";
import type { TrackGroup } from "./types";

const props = defineProps<{
  groups: TrackGroup[];
  currentItemId: string | undefined;
  gridItemSize: number;
}>();

const emit = defineEmits<{
  selectTrack: [track: MediaItem];
}>();

function selectTrack(track: MediaItem): void {
  emit("selectTrack", track);
}
</script>

<template>
  <section
    class="min-h-0 overflow-auto px-7 py-6"
    aria-label="Tracks grid"
    :style="{ '--grid-item-min-size': `${props.gridItemSize}px` }"
  >
    <section
      v-for="group in props.groups"
      :key="group.label || 'all-tracks'"
      class="library-group"
    >
      <h2
        v-if="group.label"
        class="mb-3 text-xs font-semibold tracking-wide text-(--muted-text) uppercase"
      >
        {{ group.label }}
      </h2>
      <div class="library-grid track-grid">
        <article
          v-for="track in group.items"
          :key="track.id"
          :aria-label="`Play ${track.title}`"
          class="track-tile min-w-0 cursor-pointer rounded-lg p-2 outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
          :data-current="track.id === props.currentItemId"
          :data-track-id="track.id"
          tabindex="0"
          @click="selectTrack(track)"
          @keydown.enter.prevent="selectTrack(track)"
          @keydown.space.prevent="selectTrack(track)"
        >
          <div
            class="cover-art grid aspect-square w-full place-items-center rounded-lg [&>svg]:size-[28%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
          >
            <Disc3 aria-hidden="true" />
            <YouTubeArtwork
              class="absolute inset-0 size-full object-cover"
              :video-id="track.id"
            />
          </div>
          <h2
            class="mt-2 overflow-hidden text-xs font-semibold text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ track.title }}
          </h2>
          <p
            class="mt-0.5 overflow-hidden text-[0.69rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
          >
            {{ track.artist }}
          </p>
        </article>
      </div>
    </section>
  </section>
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

.library-grid {
  display: grid;
  grid-template-columns: repeat(
    auto-fill,
    minmax(min(var(--grid-item-min-size, 176px), 100%), 1fr)
  );
  align-content: start;
  gap: 1rem;
}

.library-group + .library-group {
  margin-top: 1.5rem;
}
</style>
