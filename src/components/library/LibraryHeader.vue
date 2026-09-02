<script setup lang="ts">
import { Ellipsis, Grid2X2, List, LoaderCircle } from "lucide-vue-next";

import { Slider } from "@/components/ui/slider";
import type {
  LibraryDisplayMode,
  LibraryGroupOption,
  LibrarySortOption,
} from "./types";

withDefaults(
  defineProps<{
    collectionTitle: string;
    collectionSummary: string;
    errorMessage?: string;
    displayMode: LibraryDisplayMode;
    gridItemSize: number;
    libraryOptionsOpen: boolean;
    sortBy: LibrarySortOption;
    groupBy: LibraryGroupOption;
    metadataRefreshRemaining: number;
    hasActiveMetadataRefresh: boolean;
    metadataRefreshDrawerOpen: boolean;
  }>(),
  { errorMessage: undefined },
);

const emit = defineEmits<{
  setDisplayMode: [mode: LibraryDisplayMode];
  toggleOptions: [];
  setSort: [option: LibrarySortOption];
  setGroup: [option: LibraryGroupOption];
  setGridItemSize: [size: number];
  toggleMetadataRefresh: [];
}>();

const sortOptions: ReadonlyArray<readonly [LibrarySortOption, string]> = [
  ["title-asc", "Title"],
  ["title-desc", "Title (Z–A)"],
  ["artist-asc", "Artist"],
  ["artist-desc", "Artist (Z–A)"],
  ["album-asc", "Album"],
  ["album-desc", "Album (Z–A)"],
  ["duration-desc", "Longest first"],
  ["duration-asc", "Shortest first"],
];
const groupOptions: ReadonlyArray<readonly [LibraryGroupOption, string]> = [
  ["none", "No grouping"],
  ["artist", "Artist"],
  ["album", "Album"],
];

function emitGridItemSize(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit("setGridItemSize", value);
  }
}
</script>

<template>
  <header class="window-header gap-6">
    <div>
      <h1 class="window-title">
        {{ collectionTitle }}
      </h1>
      <div
        class="mt-1 flex items-center gap-2 text-[0.77rem] text-(--muted-text)"
      >
        <span data-library-summary>{{ collectionSummary }}</span>
        <button
          v-if="hasActiveMetadataRefresh"
          :aria-expanded="metadataRefreshDrawerOpen"
          :aria-label="`${metadataRefreshRemaining} metadata refreshes remaining`"
          class="inline-flex cursor-pointer items-center gap-1 border-0 bg-transparent p-0 text-inherit hover:text-(--text)"
          data-metadata-refresh-remaining
          type="button"
          @click="emit('toggleMetadataRefresh')"
        >
          <LoaderCircle class="size-3 animate-spin" aria-hidden="true" />
          {{ metadataRefreshRemaining }}
        </button>
      </div>
    </div>

    <p
      v-if="errorMessage"
      class="m-0 max-w-xl flex-1 rounded-lg border border-red-500/25 bg-red-500/8 px-3 py-2 text-sm text-red-300"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <nav class="flex items-center gap-1" aria-label="Library view options">
      <button
        aria-label="List view"
        :aria-pressed="displayMode === 'list'"
        class="grid size-8.5 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:bg-[oklch(0.72_0.025_258/0.13)] aria-pressed:text-(--text) [&>svg]:size-4.5"
        type="button"
        @click="emit('setDisplayMode', 'list')"
      >
        <List aria-hidden="true" />
      </button>
      <button
        aria-label="Grid view"
        :aria-pressed="displayMode === 'grid'"
        class="grid size-8.5 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:bg-[oklch(0.72_0.025_258/0.13)] aria-pressed:text-(--text) [&>svg]:size-4.5"
        type="button"
        @click="emit('setDisplayMode', 'grid')"
      >
        <Grid2X2 aria-hidden="true" />
      </button>

      <div class="relative">
        <button
          aria-label="More library options"
          :aria-expanded="libraryOptionsOpen"
          aria-haspopup="menu"
          class="grid size-8.5 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4.5"
          type="button"
          @click="emit('toggleOptions')"
        >
          <Ellipsis aria-hidden="true" />
        </button>
        <div
          v-if="libraryOptionsOpen"
          class="library-options-menu absolute top-10 right-0 z-20 grid w-52 gap-1 rounded-lg border border-(--line) bg-(--glass-window) p-2 shadow-xl backdrop-blur-lg"
          role="menu"
        >
          <p
            class="px-2 py-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
          >
            Sort by
          </p>
          <button
            v-for="option in sortOptions"
            :key="option[0]"
            :data-sort="option[0]"
            :aria-pressed="sortBy === option[0]"
            class="flex cursor-pointer items-center justify-between rounded-md border-0 bg-transparent px-2 py-1.5 text-left text-xs text-(--muted-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:text-(--text)"
            role="menuitemradio"
            type="button"
            @click="emit('setSort', option[0])"
          >
            {{ option[1] }}
            <span v-if="sortBy === option[0]" aria-hidden="true">✓</span>
          </button>
          <p
            class="mt-1 px-2 py-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
          >
            Group in grid
          </p>
          <button
            v-for="option in groupOptions"
            :key="option[0]"
            :data-group="option[0]"
            :aria-pressed="groupBy === option[0]"
            class="flex cursor-pointer items-center justify-between rounded-md border-0 bg-transparent px-2 py-1.5 text-left text-xs text-(--muted-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:text-(--text)"
            role="menuitemradio"
            type="button"
            @click="emit('setGroup', option[0])"
          >
            {{ option[1] }}
            <span v-if="groupBy === option[0]" aria-hidden="true">✓</span>
          </button>
        </div>
      </div>
      <label
        v-if="displayMode === 'grid'"
        class="ml-2 flex w-28 items-center gap-2 text-(--muted-text) [&>svg]:size-3.5"
      >
        <span class="sr-only">Grid item size</span>
        <Slider
          aria-label="Grid item size"
          :min="120"
          :max="280"
          :step="4"
          :model-value="[gridItemSize]"
          @value-commit="emitGridItemSize"
        />
      </label>
    </nav>
  </header>
</template>

<style scoped>
/* The header uses utility classes for its layout and visual treatment. */
</style>
