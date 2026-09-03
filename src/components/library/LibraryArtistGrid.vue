<script setup lang="ts">
import { Mic2 } from "lucide-vue-next";

import { ScrollArea } from "@/components/ui/scroll-area";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { formatDuration } from "@/lib/time";
import type {
  ArtistGroup,
  LibraryArtist,
  LibraryDisplayMode,
  LibrarySortOption,
} from "./types";
import LibraryVirtualGrid from "./LibraryVirtualGrid.vue";
import YouTubeArtwork from "../YouTubeArtwork.vue";

type ArtistSortColumn = "album-count" | "duration" | "title" | "track-count";

const props = defineProps<{
  groups: ArtistGroup[];
  displayMode: LibraryDisplayMode;
  selectedArtistName: string | undefined;
  gridItemSize: number;
  sortBy: LibrarySortOption;
}>();

const emit = defineEmits<{
  editArtist: [artist: LibraryArtist];
  playArtist: [artist: LibraryArtist];
  selectArtist: [artist: LibraryArtist];
  openArtist: [artist: LibraryArtist];
  setSort: [option: LibrarySortOption];
}>();

function sortDirection(
  column: ArtistSortColumn,
): "ascending" | "descending" | "none" {
  if (!props.sortBy.startsWith(`${column}-`)) {
    return "none";
  }

  return props.sortBy.endsWith("-desc") ? "descending" : "ascending";
}

function sortIndicator(column: ArtistSortColumn): string {
  const direction = sortDirection(column);

  return direction === "ascending"
    ? "↑"
    : direction === "descending"
      ? "↓"
      : "";
}

function sortButtonLabel(column: ArtistSortColumn, label: string): string {
  const direction = sortDirection(column);
  const nextDirection = direction === "ascending" ? "descending" : "ascending";

  return `Sort by ${label}, ${nextDirection}`;
}

function toggleSort(column: ArtistSortColumn): void {
  const direction = sortDirection(column);
  const nextDirection = direction === "ascending" ? "desc" : "asc";

  emit("setSort", `${column}-${nextDirection}` as LibrarySortOption);
}
</script>

<template>
  <section
    class="min-h-0"
    aria-label="Artists"
    :style="{ '--grid-item-min-size': `${props.gridItemSize}px` }"
  >
    <ScrollArea v-if="props.displayMode === 'list'" class="size-full">
      <table
        class="w-full table-fixed border-separate border-spacing-0 px-5 text-left"
        data-library-artist-list
      >
        <colgroup>
          <col style="width: 8%" />
          <col style="width: 42%" />
          <col style="width: 15%" />
          <col style="width: 15%" />
          <col style="width: 20%" />
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
                :aria-label="sortButtonLabel('title', 'Artist')"
                class="inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-inherit hover:text-(--text) focus-visible:text-(--text) focus-visible:outline-none"
                type="button"
                @click="toggleSort('title')"
              >
                Artist
                <span v-if="sortIndicator('title')" aria-hidden="true">
                  {{ sortIndicator("title") }}
                </span>
              </button>
            </th>
            <th
              :aria-sort="sortDirection('album-count')"
              class="px-3 py-2 text-center"
              data-sort-column="album-count"
              scope="col"
            >
              <button
                :aria-label="sortButtonLabel('album-count', 'Albums')"
                class="inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-inherit hover:text-(--text) focus-visible:text-(--text) focus-visible:outline-none"
                type="button"
                @click="toggleSort('album-count')"
              >
                Albums
                <span v-if="sortIndicator('album-count')" aria-hidden="true">
                  {{ sortIndicator("album-count") }}
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
            :key="group.label || 'all-artists'"
          >
            <ContextMenu
              v-for="artist in group.items"
              :key="artist.name"
              @update:open="(isOpen) => isOpen && emit('selectArtist', artist)"
            >
              <ContextMenuTrigger as-child>
                <tr
                  class="artist-tile cursor-pointer outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:bg-[oklch(0.72_0.03_268/0.13)] data-[selected=true]:bg-[oklch(0.72_0.03_268/0.13)]"
                  :data-selected="props.selectedArtistName === artist.name"
                  tabindex="0"
                  @click="emit('selectArtist', artist)"
                  @dblclick="emit('openArtist', artist)"
                  @keydown.enter.prevent="emit('openArtist', artist)"
                >
                  <td class="px-3 py-1.5">
                    <div
                      class="cover-art grid size-8 place-items-center rounded-full [&>svg]:size-4 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
                    >
                      <YouTubeArtwork
                        class="absolute inset-0 size-full object-cover [&_svg]:size-4 [&_svg]:text-[oklch(0.98_0.01_90/0.76)]"
                        :video-id="artist.videoId"
                        :missing-icon="Mic2"
                      />
                    </div>
                  </td>
                  <td
                    class="overflow-hidden px-3 py-1.5 text-[0.82rem] font-medium text-(--text)"
                  >
                    <span
                      class="block overflow-hidden text-ellipsis whitespace-nowrap"
                    >
                      {{ artist.name }}
                    </span>
                  </td>
                  <td
                    class="px-3 py-1.5 text-center text-[0.78rem] text-(--muted-text) tabular-nums"
                  >
                    {{ artist.albumCount }}
                  </td>
                  <td
                    class="px-3 py-1.5 text-center text-[0.78rem] text-(--muted-text) tabular-nums"
                  >
                    {{ artist.trackCount }}
                  </td>
                  <td
                    class="px-3 py-1.5 text-right text-[0.78rem] text-(--muted-text) tabular-nums"
                  >
                    {{ formatDuration(artist.durationMs) }}
                  </td>
                </tr>
              </ContextMenuTrigger>
              <ContextMenuContent data-artist-context-menu>
                <ContextMenuItem @select="emit('playArtist', artist)">
                  Play artist
                </ContextMenuItem>
                <ContextMenuItem @select="emit('openArtist', artist)">
                  Open tracks
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem @select="emit('editArtist', artist)">
                  Edit metadata
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          </template>
        </tbody>
      </table>
    </ScrollArea>
    <LibraryVirtualGrid
      v-else
      :get-item-key="(artist) => artist.name"
      :grid-item-size="props.gridItemSize"
      :groups="props.groups"
      :item-height-padding="40"
    >
      <template #item="{ item: artist }">
        <ContextMenu
          @update:open="(isOpen) => isOpen && emit('selectArtist', artist)"
        >
          <ContextMenuTrigger as-child>
            <article
              class="artist-tile min-w-0 cursor-pointer rounded-lg p-2 outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
              :data-selected="props.selectedArtistName === artist.name"
              tabindex="0"
              @click="emit('selectArtist', artist)"
              @dblclick="emit('openArtist', artist)"
              @keydown.enter.prevent="emit('openArtist', artist)"
            >
              <div
                class="cover-art grid place-items-center rounded-full [&>svg]:size-6 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
                :class="'aspect-square w-full'"
              >
                <YouTubeArtwork
                  class="absolute inset-0 size-full object-cover [&_svg]:size-6 [&_svg]:text-[oklch(0.98_0.01_90/0.76)]"
                  :video-id="artist.videoId"
                  :missing-icon="Mic2"
                />
              </div>
              <div class="min-w-0">
                <h2
                  class="overflow-hidden text-[0.78rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
                >
                  {{ artist.name }}
                </h2>
                <p class="mt-0.5 text-[0.69rem] text-(--muted-text)">
                  {{ artist.detail }}
                </p>
              </div>
            </article>
          </ContextMenuTrigger>
          <ContextMenuContent data-artist-context-menu>
            <ContextMenuItem @select="emit('playArtist', artist)">
              Play artist
            </ContextMenuItem>
            <ContextMenuItem @select="emit('openArtist', artist)">
              Open tracks
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem @select="emit('editArtist', artist)">
              Edit metadata
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
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
