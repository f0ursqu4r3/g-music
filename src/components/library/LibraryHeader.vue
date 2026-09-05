<script setup lang="ts">
import {
  ChevronRight,
  CircleAlert,
  Ellipsis,
  Grid2X2,
  List,
  Loader,
  Play,
  Search,
} from "lucide-vue-next";
import {
  DropdownMenuRoot,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  PopoverRoot,
  PopoverTrigger,
  PopoverPortal,
  PopoverContent,
} from "reka-ui";
import { ref } from "vue";

import { Slider } from "@/components/ui/slider";
import type {
  LibraryDisplayMode,
  LibraryGroupOption,
  LibrarySortOption,
} from "./types";

const props = withDefaults(
  defineProps<{
    collectionTitle: string;
    collectionSummary: string;
    searchOpen?: boolean;
    playlistName?: string;
    canPlayPlaylist?: boolean;
    isUpdating?: boolean;
    errorMessage?: string;
    displayMode: LibraryDisplayMode;
    gridItemSize: number;
    libraryOptionsOpen: boolean;
    sortBy: LibrarySortOption;
    groupBy: LibraryGroupOption;
    metadataRefreshRemaining: number;
    metadataRefreshFailed?: number;
    metadataRefreshSkipped?: number;
    hasActiveMetadataRefresh: boolean;
    metadataRefreshDrawerOpen: boolean;
  }>(),
  {
    errorMessage: undefined,
    metadataRefreshFailed: 0,
    metadataRefreshSkipped: 0,
  },
);

const emit = defineEmits<{
  setDisplayMode: [mode: LibraryDisplayMode];
  toggleOptions: [open: boolean];
  setSort: [option: LibrarySortOption];
  setGroup: [option: LibraryGroupOption];
  setGridItemSize: [size: number];
  toggleMetadataRefresh: [open: boolean];
  playPlaylist: [];
  toggleSearch: [];
}>();

const summaryElement = ref<HTMLElement>();
function restoreRefreshFocus(event: Event): void {
  if (
    !props.hasActiveMetadataRefresh &&
    !props.metadataRefreshFailed &&
    !props.metadataRefreshSkipped
  ) {
    event.preventDefault();
    summaryElement.value?.focus();
  }
}

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
  ["none", "Sort sections"],
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
  <header class="window-header flex-wrap gap-x-3 gap-y-2 py-3">
    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 items-center gap-3">
        <h1 class="window-title min-w-0 truncate" :title="collectionTitle">
          {{ collectionTitle }}
        </h1>
        <button
          v-if="playlistName"
          :aria-label="`Play ${playlistName}`"
          :disabled="isUpdating || !canPlayPlaylist"
          class="inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-md border border-(--line-strong) bg-transparent px-2.5 text-xs font-medium text-(--muted-text) transition-colors hover:border-accent hover:text-(--text) disabled:cursor-default disabled:opacity-40 [&>svg]:size-3.5"
          data-play-selected-playlist
          type="button"
          @click="emit('playPlaylist')"
        >
          <Play aria-hidden="true" />
          Play
        </button>
      </div>
      <div
        class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[0.77rem] text-(--muted-text)"
      >
        <span ref="summaryElement" data-library-summary tabindex="-1">{{
          collectionSummary
        }}</span>
        <PopoverRoot
          v-if="
            hasActiveMetadataRefresh ||
            metadataRefreshFailed ||
            metadataRefreshSkipped ||
            metadataRefreshDrawerOpen
          "
          :open="metadataRefreshDrawerOpen"
          :modal="false"
          @update:open="emit('toggleMetadataRefresh', $event)"
        >
          <PopoverTrigger as-child>
            <button
              :aria-label="
                hasActiveMetadataRefresh
                  ? `${metadataRefreshRemaining} metadata refreshes remaining${metadataRefreshFailed ? `, ${metadataRefreshFailed} failed` : ''}${metadataRefreshSkipped ? `, ${metadataRefreshSkipped} skipped` : ''}`
                  : metadataRefreshFailed
                    ? `${metadataRefreshFailed} metadata refreshes failed${metadataRefreshSkipped ? `, ${metadataRefreshSkipped} skipped` : ''}`
                    : metadataRefreshSkipped
                      ? `Metadata refresh complete, ${metadataRefreshSkipped} skipped`
                      : 'Metadata refresh complete'
              "
              class="inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-(--line) bg-(--glass-control) px-2 py-1 text-xs text-(--muted-text) tabular-nums outline-none hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
              data-metadata-refresh-remaining
              type="button"
            >
              <Loader
                v-if="hasActiveMetadataRefresh"
                class="size-3 shrink-0 animate-spin text-accent motion-reduce:animate-none"
                aria-hidden="true"
              />
              <CircleAlert
                v-else-if="metadataRefreshFailed"
                class="size-3 shrink-0 text-(--warning)"
                aria-hidden="true"
              />
              <span v-if="hasActiveMetadataRefresh"
                >Refreshing metadata · {{ metadataRefreshRemaining }} left</span
              >
              <span
                v-else-if="!metadataRefreshFailed && !metadataRefreshSkipped"
                >Metadata refreshed</span
              >
              <span v-if="metadataRefreshFailed" class="text-(--warning)"
                >{{ hasActiveMetadataRefresh ? "· " : ""
                }}{{ metadataRefreshFailed }} failed</span
              >
              <span v-if="metadataRefreshSkipped"
                >{{
                  hasActiveMetadataRefresh || metadataRefreshFailed ? "· " : ""
                }}{{ metadataRefreshSkipped }} skipped</span
              >
              <ChevronRight
                class="size-3 shrink-0"
                :class="{ 'rotate-90': metadataRefreshDrawerOpen }"
                aria-hidden="true"
              />
            </button>
          </PopoverTrigger>
          <PopoverPortal>
            <PopoverContent
              data-metadata-refresh-popover
              aria-label="Metadata refresh"
              side="bottom"
              align="start"
              :side-offset="8"
              :collision-padding="12"
              class="z-50 h-[min(480px,var(--reka-popover-content-available-height))] w-96 max-w-[calc(100vw-24px)] overflow-hidden rounded-xl border border-(--line-strong) bg-(--popover) text-(--text) shadow-xl outline-none"
              @close-auto-focus="restoreRefreshFocus"
            >
              <slot name="metadata-refresh" />
            </PopoverContent>
          </PopoverPortal>
        </PopoverRoot>
      </div>
    </div>

    <p
      v-if="errorMessage"
      class="order-last m-0 max-h-20 w-full overflow-y-auto break-words rounded-lg border border-(--line-strong) bg-(--error-surface) px-3 py-2 text-sm text-(--error-text)"
      role="alert"
    >
      {{ errorMessage }}
    </p>

    <nav class="flex items-center gap-1" aria-label="Library view options">
      <button
        :aria-label="
          searchOpen ? 'Close library search' : 'Open library search'
        "
        :aria-expanded="!!searchOpen"
        :aria-controls="searchOpen ? 'library-search' : undefined"
        aria-keyshortcuts="Meta+f Control+f"
        title="Toggle library search (⌘F / Ctrl+F)"
        data-library-search-toggle
        class="grid size-8.5 shrink-0 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-expanded:bg-[oklch(0.72_0.025_258/0.13)] aria-expanded:text-(--text) [&>svg]:size-4.5"
        type="button"
        @click="emit('toggleSearch')"
      >
        <Search aria-hidden="true" />
      </button>
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

      <DropdownMenuRoot
        :open="libraryOptionsOpen"
        @update:open="emit('toggleOptions', $event)"
      >
        <DropdownMenuTrigger
          aria-label="More library options"
          :aria-expanded="libraryOptionsOpen"
          aria-haspopup="menu"
          class="grid size-8.5 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4.5"
          type="button"
        >
          <Ellipsis aria-hidden="true" />
        </DropdownMenuTrigger>
        <DropdownMenuContent
          :side-offset="6"
          :collision-padding="8"
          align="end"
          class="library-options-menu z-50 grid max-h-[var(--reka-dropdown-menu-content-available-height)] w-52 gap-1 overflow-y-auto rounded-lg border border-(--line) bg-(--menu-surface,var(--surface)) p-2"
        >
          <p
            class="px-2 py-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
          >
            Sort by
          </p>
          <DropdownMenuRadioGroup
            :model-value="sortBy"
            @update:model-value="emit('setSort', $event as LibrarySortOption)"
          >
            <DropdownMenuRadioItem
              v-for="option in sortOptions"
              :key="option[0]"
              :data-sort="option[0]"
              :value="option[0]"
              class="flex cursor-pointer items-center justify-between rounded-md border-0 bg-transparent px-2 py-1.5 text-left text-xs text-(--muted-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:text-(--text)"
            >
              {{ option[1] }}
              <span v-if="sortBy === option[0]" aria-hidden="true">✓</span>
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
          <p
            class="mt-1 px-2 py-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
          >
            Group in grid
          </p>
          <DropdownMenuRadioGroup
            :model-value="groupBy"
            @update:model-value="emit('setGroup', $event as LibraryGroupOption)"
          >
            <DropdownMenuRadioItem
              v-for="option in groupOptions"
              :key="option[0]"
              :data-group="option[0]"
              :value="option[0]"
              class="flex cursor-pointer items-center justify-between rounded-md border-0 bg-transparent px-2 py-1.5 text-left text-xs text-(--muted-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:text-(--text)"
            >
              {{ option[1] }}
              <span v-if="groupBy === option[0]" aria-hidden="true">✓</span>
            </DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenuRoot>
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
