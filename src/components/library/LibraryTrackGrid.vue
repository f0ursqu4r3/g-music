<script setup lang="ts">
import { Disc3, Volume2 } from "lucide-vue-next";

import type { MediaItem } from "@/api";
import YouTubeArtwork from "../YouTubeArtwork.vue";
import LibraryTrackContextMenu from "./LibraryTrackContextMenu.vue";
import LibraryVirtualGrid from "./LibraryVirtualGrid.vue";
import type { TrackGroup } from "./types";

const props = defineProps<{
  groups: TrackGroup[];
  playingItemId: string | undefined;
  selectedTrackId: string | undefined;
  favoriteTrackIds: string[];
  gridItemSize: number;
}>();

const emit = defineEmits<{
  addToQueue: [id: string];
  editTrack: [track: MediaItem];
  openAlbum: [track: MediaItem];
  openArtist: [track: MediaItem];
  playNext: [id: string];
  playTrack: [track: MediaItem];
  removeTrack: [track: MediaItem];
  selectTrack: [track: MediaItem];
  toggleFavorite: [id: string];
}>();

function selectTrack(track: MediaItem): void {
  emit("selectTrack", track);
}
</script>

<template>
  <section
    class="min-h-0"
    aria-label="Tracks grid"
    :style="{ '--grid-item-min-size': `${props.gridItemSize}px` }"
  >
    <LibraryVirtualGrid
      :get-item-key="(track) => track.id"
      grid-class="track-grid"
      :grid-item-size="props.gridItemSize"
      :groups="props.groups"
    >
      <template #item="{ item: track }">
        <LibraryTrackContextMenu
          :is-favorite="props.favoriteTrackIds.includes(track.id)"
          :track="track"
          @add-to-queue="emit('addToQueue', $event)"
          @edit="emit('editTrack', $event)"
          @open-album="emit('openAlbum', $event)"
          @open-artist="emit('openArtist', $event)"
          @play="emit('playTrack', $event)"
          @play-next="emit('playNext', $event)"
          @remove="emit('removeTrack', $event)"
          @select="selectTrack"
          @toggle-favorite="emit('toggleFavorite', $event)"
        >
          <article
            :aria-label="`Play ${track.title}`"
            :aria-selected="track.id === props.selectedTrackId"
            class="track-tile min-w-0 cursor-pointer rounded-lg p-2 outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:ring-2 focus-visible:ring-(--focus-ring) data-[selected=true]:bg-[oklch(0.72_0.03_268/0.13)]"
            :data-playing="track.id === props.playingItemId"
            :data-selected="track.id === props.selectedTrackId"
            :data-track-id="track.id"
            tabindex="0"
            @click="selectTrack(track)"
            @keydown.enter.prevent="selectTrack(track)"
            @keydown.space.prevent="selectTrack(track)"
          >
            <div
              class="cover-art grid aspect-square w-full place-items-center rounded-lg [&>svg]:size-[28%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
            >
              <YouTubeArtwork
                class="absolute inset-0 size-full object-cover [&_svg]:size-[28%] [&_svg]:text-[oklch(0.98_0.01_90/0.74)]"
                :video-id="track.id"
                :missing-icon="Disc3"
              />
            </div>
            <h2
              class="mt-2 flex min-w-0 items-center gap-1.5 text-xs font-semibold text-(--text)"
            >
              <Volume2
                v-if="track.id === props.playingItemId"
                class="track-playing-indicator size-3.5 shrink-0"
                aria-label="Currently playing"
              />
              <span
                class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap"
              >
                {{ track.title }}
              </span>
            </h2>
            <p
              class="mt-0.5 overflow-hidden text-[0.69rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
            >
              {{ track.artist }}
            </p>
          </article>
        </LibraryTrackContextMenu>
      </template>
    </LibraryVirtualGrid>
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
</style>
