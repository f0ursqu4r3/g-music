<script setup lang="ts">
import { Disc3 } from "lucide-vue-next";

import type { MediaItem } from "@/api";
import { ScrollArea } from "@/components/ui/scroll-area";
import YouTubeArtwork from "../YouTubeArtwork.vue";
import LibraryTrackContextMenu from "./LibraryTrackContextMenu.vue";
import type { TrackGroup } from "./types";

const props = defineProps<{
  groups: TrackGroup[];
  currentItemId: string | undefined;
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
    <ScrollArea class="size-full">
      <div class="px-7 py-6">
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
            <LibraryTrackContextMenu
              v-for="track in group.items"
              :key="track.id"
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
                  <YouTubeArtwork
                    class="absolute inset-0 size-full object-cover"
                    :video-id="track.id"
                    :missing-icon="Disc3"
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
            </LibraryTrackContextMenu>
          </div>
        </section>
      </div>
    </ScrollArea>
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
