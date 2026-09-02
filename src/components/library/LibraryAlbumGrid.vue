<script setup lang="ts">
import { Disc3 } from "lucide-vue-next";

import { ScrollArea } from "@/components/ui/scroll-area";
import { formatDuration } from "@/lib/time";
import type {
  AlbumGroup,
  LibraryAlbum,
  LibraryDisplayMode,
  LibrarySortOption,
} from "./types";
import YouTubeArtwork from "../YouTubeArtwork.vue";

type AlbumSortColumn = "artist" | "duration" | "title" | "track-count";

const props = defineProps<{
  groups: AlbumGroup[];
  displayMode: LibraryDisplayMode;
  selectedAlbumKey: string | undefined;
  gridItemSize: number;
  sortBy: LibrarySortOption;
}>();

const emit = defineEmits<{
  selectAlbum: [album: LibraryAlbum];
  openAlbum: [album: LibraryAlbum];
  setSort: [option: LibrarySortOption];
}>();

function sortDirection(
  column: AlbumSortColumn,
): "ascending" | "descending" | "none" {
  if (!props.sortBy.startsWith(`${column}-`)) {
    return "none";
  }

  return props.sortBy.endsWith("-desc") ? "descending" : "ascending";
}

function sortIndicator(column: AlbumSortColumn): string {
  const direction = sortDirection(column);

  return direction === "ascending"
    ? "↑"
    : direction === "descending"
      ? "↓"
      : "";
}

function sortButtonLabel(column: AlbumSortColumn, label: string): string {
  const direction = sortDirection(column);
  const nextDirection = direction === "ascending" ? "descending" : "ascending";

  return `Sort by ${label}, ${nextDirection}`;
}

function toggleSort(column: AlbumSortColumn): void {
  const direction = sortDirection(column);
  const nextDirection = direction === "ascending" ? "desc" : "asc";

  emit("setSort", `${column}-${nextDirection}` as LibrarySortOption);
}
</script>

<template>
  <section
    class="min-h-0"
    aria-label="Albums"
    :style="{ '--grid-item-min-size': `${props.gridItemSize}px` }"
  >
    <ScrollArea class="size-full">
      <table
        v-if="props.displayMode === 'list'"
        class="w-full table-fixed border-separate border-spacing-0 px-5 text-left"
        data-library-album-list
      >
        <colgroup>
          <col style="width: 8%" />
          <col style="width: 32%" />
          <col style="width: 27%" />
          <col style="width: 12%" />
          <col style="width: 21%" />
        </colgroup>
        <thead>
          <tr class="text-[0.66rem] font-medium text-(--subtle-text)">
            <th class="px-3 py-2" scope="col">
              <span class="sr-only">Artwork</span>
            </th>
            <th
              :aria-sort="sortDirection('title')"
              class="px-3 py-2"
              data-sort-column="title"
              scope="col"
            >
              <button
                :aria-label="sortButtonLabel('title', 'Album')"
                class="inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-inherit hover:text-(--text) focus-visible:text-(--text) focus-visible:outline-none"
                type="button"
                @click="toggleSort('title')"
              >
                Album
                <span v-if="sortIndicator('title')" aria-hidden="true">
                  {{ sortIndicator("title") }}
                </span>
              </button>
            </th>
            <th
              :aria-sort="sortDirection('artist')"
              class="px-3 py-2"
              data-sort-column="artist"
              scope="col"
            >
              <button
                :aria-label="sortButtonLabel('artist', 'Artist')"
                class="inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-inherit hover:text-(--text) focus-visible:text-(--text) focus-visible:outline-none"
                type="button"
                @click="toggleSort('artist')"
              >
                Artist
                <span v-if="sortIndicator('artist')" aria-hidden="true">
                  {{ sortIndicator("artist") }}
                </span>
              </button>
            </th>
            <th
              :aria-sort="sortDirection('track-count')"
              class="px-3 py-2 text-center"
              data-sort-column="track-count"
              scope="col"
            >
              <button
                :aria-label="sortButtonLabel('track-count', 'Tracks')"
                class="inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-inherit hover:text-(--text) focus-visible:text-(--text) focus-visible:outline-none"
                type="button"
                @click="toggleSort('track-count')"
              >
                Tracks
                <span v-if="sortIndicator('track-count')" aria-hidden="true">
                  {{ sortIndicator("track-count") }}
                </span>
              </button>
            </th>
            <th
              :aria-sort="sortDirection('duration')"
              class="px-3 py-2 text-right"
              data-sort-column="duration"
              scope="col"
            >
              <button
                :aria-label="sortButtonLabel('duration', 'Duration')"
                class="inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-inherit hover:text-(--text) focus-visible:text-(--text) focus-visible:outline-none"
                type="button"
                @click="toggleSort('duration')"
              >
                Duration
                <span v-if="sortIndicator('duration')" aria-hidden="true">
                  {{ sortIndicator("duration") }}
                </span>
              </button>
            </th>
          </tr>
        </thead>
        <tbody>
          <template
            v-for="group in props.groups"
            :key="group.label || 'all-albums'"
          >
            <tr
              v-for="album in group.items"
              :key="album.key"
              class="album-tile cursor-pointer outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:bg-[oklch(0.72_0.03_268/0.13)] data-[selected=true]:bg-[oklch(0.72_0.03_268/0.13)]"
              :data-selected="props.selectedAlbumKey === album.key"
              tabindex="0"
              @click="emit('selectAlbum', album)"
              @dblclick="emit('openAlbum', album)"
              @keydown.enter.prevent="emit('openAlbum', album)"
            >
              <td class="px-3 py-1.5">
                <div
                  class="cover-art grid size-8 place-items-center rounded-md [&>svg]:size-4 [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
                >
                  <Disc3 aria-hidden="true" />
                  <YouTubeArtwork
                    class="absolute inset-0 size-full object-cover"
                    :video-id="album.videoId"
                  />
                </div>
              </td>
              <td
                class="overflow-hidden px-3 py-1.5 text-[0.82rem] font-medium text-(--text)"
              >
                <span
                  class="block overflow-hidden text-ellipsis whitespace-nowrap"
                >
                  {{ album.title }}
                </span>
              </td>
              <td
                class="overflow-hidden px-3 py-1.5 text-[0.8rem] text-(--muted-text)"
              >
                <span
                  class="block overflow-hidden text-ellipsis whitespace-nowrap"
                >
                  {{ album.artist }}
                </span>
              </td>
              <td
                class="px-3 py-1.5 text-center text-[0.78rem] text-(--muted-text) tabular-nums"
              >
                {{ album.trackCount }}
              </td>
              <td
                class="px-3 py-1.5 text-right text-[0.78rem] text-(--muted-text) tabular-nums"
              >
                {{ formatDuration(album.durationMs) }}
              </td>
            </tr>
          </template>
        </tbody>
      </table>
      <div class="px-7 py-6">
        <section
          v-if="props.displayMode === 'grid'"
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
          <div class="library-grid">
            <article
              v-for="album in group.items"
              :key="album.key"
              class="album-tile min-w-0 cursor-pointer rounded-lg p-2 outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
              :data-selected="props.selectedAlbumKey === album.key"
              tabindex="0"
              @click="emit('selectAlbum', album)"
              @dblclick="emit('openAlbum', album)"
              @keydown.enter.prevent="emit('openAlbum', album)"
            >
              <div
                class="cover-art grid place-items-center rounded-lg [&>svg]:size-[34%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
                :class="'aspect-square w-full'"
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
