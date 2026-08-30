<script setup lang="ts">
import { Disc3 } from "lucide-vue-next";

import type { LibraryAlbum, AlbumGroup, LibraryDisplayMode } from "./types";
import YouTubeArtwork from "../YouTubeArtwork.vue";

const props = defineProps<{
  groups: AlbumGroup[];
  displayMode: LibraryDisplayMode;
  selectedAlbumKey: string | undefined;
  gridItemSize: number;
}>();

const emit = defineEmits<{
  selectAlbum: [album: LibraryAlbum];
  openAlbum: [album: LibraryAlbum];
}>();
</script>

<template>
  <section
    class="min-h-0 overflow-auto px-7 py-6"
    aria-label="Albums"
    :style="{ '--grid-item-min-size': `${props.gridItemSize}px` }"
  >
    <section
      v-for="group in props.groups"
      :key="group.label || 'all-albums'"
      class="library-group"
    >
      <h2
        v-if="group.label"
        class="mb-3 text-xs font-semibold tracking-wide text-(--muted-text) uppercase"
      >
        {{ group.label }}
      </h2>
      <div
        :class="props.displayMode === 'grid' ? 'library-grid' : 'grid gap-2'"
      >
        <article
          v-for="album in group.items"
          :key="album.key"
          class="album-tile min-w-0 cursor-pointer rounded-lg p-2 outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
          :class="props.displayMode === 'list' ? 'flex items-center gap-3' : ''"
          :data-selected="props.selectedAlbumKey === album.key"
          tabindex="0"
          @click="emit('selectAlbum', album)"
          @dblclick="emit('openAlbum', album)"
          @keydown.enter.prevent="emit('openAlbum', album)"
        >
          <div
            class="cover-art grid place-items-center rounded-lg [&>svg]:size-[34%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
            :class="
              props.displayMode === 'grid'
                ? 'aspect-square w-full'
                : 'size-14 shrink-0'
            "
          >
            <Disc3 aria-hidden="true" />
            <YouTubeArtwork
              class="absolute inset-0 size-full object-cover"
              :video-id="album.videoId"
            />
          </div>
          <div class="min-w-0">
            <h2
              class="mt-2 overflow-hidden text-[0.78rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
              :class="props.displayMode === 'list' ? 'mt-0' : ''"
            >
              {{ album.title }}
            </h2>
            <p class="mt-0.5 text-[0.69rem] text-(--muted-text)">
              {{ album.artist }} · {{ album.trackCount }}
              {{ album.trackCount === 1 ? "song" : "songs" }}
            </p>
          </div>
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
