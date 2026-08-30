<script setup lang="ts">
import {
  Clock3,
  Heart,
  ListFilter,
  LoaderCircle,
  Play,
  Volume2,
  X,
} from "lucide-vue-next";
import {
  observeElementRect,
  type Rect,
  useVirtualizer,
  type Virtualizer,
} from "@tanstack/vue-virtual";
import {
  type ComponentPublicInstance,
  computed,
  onBeforeUnmount,
  ref,
} from "vue";

import type { MediaItem } from "@/api";
import { formatDuration } from "@/lib/time";
import type { TrackFilter } from "./types";

const props = defineProps<{
  tracks: MediaItem[];
  currentItemId: string | undefined;
  trackFilter: TrackFilter | null;
}>();

const emit = defineEmits<{
  selectTrack: [track: MediaItem];
  playTrack: [track: MediaItem];
  clearTrackFilter: [];
}>();

const favoriteTrackIds = ref(new Set<string>());
const columnWidths = ref([6, 29, 25, 25, 9, 6]);
const minimumColumnWidths = [5, 18, 12, 12, 7, 5] as const;
const trackList = ref<HTMLElement | null>(null);
const trackRowHeight = 44;
let stopColumnResize: (() => void) | undefined;

function viewportHeight(): number {
  return typeof window === "undefined" ? 600 : window.innerHeight || 600;
}

function setTrackList(element: Element | ComponentPublicInstance | null): void {
  trackList.value = element instanceof HTMLElement ? element : null;
}

function observeTrackListRect(
  instance: Virtualizer<HTMLElement, Element>,
  callback: (rect: Rect) => void,
): (() => void) | undefined {
  return observeElementRect(instance, (rect) => {
    callback(rect.height > 0 ? rect : { ...rect, height: viewportHeight() });
  });
}

const virtualizerOptions = computed(() => {
  const scrollElement = trackList.value;

  return {
    count: props.tracks.length,
    estimateSize: () => trackRowHeight,
    getScrollElement: () => scrollElement,
    initialRect: {
      height: viewportHeight(),
      width: 0,
    },
    observeElementRect: observeTrackListRect,
    overscan: 8,
  };
});
const trackVirtualizer = useVirtualizer(virtualizerOptions);
const virtualTracks = computed(() =>
  trackVirtualizer.value.getVirtualItems().flatMap((virtualItem) => {
    const track = props.tracks[virtualItem.index];

    return track ? [{ track, virtualItem }] : [];
  }),
);
const trackGridTemplateColumns = computed(() =>
  columnWidths.value.map((width) => `${width}%`).join(" "),
);
const virtualTrackHeight = computed(
  () => `${trackVirtualizer.value.getTotalSize()}px`,
);

function isFavorite(trackId: string): boolean {
  return favoriteTrackIds.value.has(trackId);
}

function toggleFavorite(trackId: string): void {
  const nextFavorites = new Set(favoriteTrackIds.value);

  if (nextFavorites.has(trackId)) {
    nextFavorites.delete(trackId);
  } else {
    nextFavorites.add(trackId);
  }

  favoriteTrackIds.value = nextFavorites;
}

function resizeColumnBoundary(
  boundaryIndex: number,
  requestedDelta: number,
  initialWidths = columnWidths.value,
): void {
  const leftWidth = initialWidths[boundaryIndex];
  const rightWidth = initialWidths[boundaryIndex + 1];
  const minimumLeftWidth = minimumColumnWidths[boundaryIndex];
  const minimumRightWidth = minimumColumnWidths[boundaryIndex + 1];
  if (
    leftWidth === undefined ||
    rightWidth === undefined ||
    minimumLeftWidth === undefined ||
    minimumRightWidth === undefined
  ) {
    return;
  }

  const delta = Math.min(
    Math.max(requestedDelta, minimumLeftWidth - leftWidth),
    rightWidth - minimumRightWidth,
  );
  const nextWidths = [...initialWidths];
  nextWidths[boundaryIndex] = leftWidth + delta;
  nextWidths[boundaryIndex + 1] = rightWidth - delta;
  columnWidths.value = nextWidths;
}

function startColumnResize(boundaryIndex: number, event: MouseEvent): void {
  if (event.button !== 0) {
    return;
  }

  event.preventDefault();
  stopColumnResize?.();
  const startX = event.clientX;
  const initialWidths = [...columnWidths.value];
  const table = (event.currentTarget as HTMLElement).closest("table");

  const handleMouseMove = (moveEvent: MouseEvent): void => {
    const tableWidth = table?.getBoundingClientRect().width ?? 0;
    if (tableWidth <= 0) {
      return;
    }

    const delta = ((moveEvent.clientX - startX) / tableWidth) * 100;
    resizeColumnBoundary(boundaryIndex, delta, initialWidths);
  };
  const handleMouseUp = (): void => {
    window.removeEventListener("mousemove", handleMouseMove);
    window.removeEventListener("mouseup", handleMouseUp);
    stopColumnResize = undefined;
  };

  stopColumnResize = handleMouseUp;
  window.addEventListener("mousemove", handleMouseMove);
  window.addEventListener("mouseup", handleMouseUp);
}

function resizeColumnWithKeyboard(
  boundaryIndex: number,
  event: KeyboardEvent,
): void {
  if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
    return;
  }

  event.preventDefault();
  resizeColumnBoundary(boundaryIndex, event.key === "ArrowRight" ? 1 : -1);
}

onBeforeUnmount(() => {
  stopColumnResize?.();
});
</script>

<template>
  <div class="flex min-h-0 flex-col py-2.5 pl-2">
    <div
      v-if="props.trackFilter"
      class="mx-5 mb-2 flex items-center gap-2 rounded-md bg-[oklch(0.72_0.03_268/0.13)] px-3 py-1.5 text-xs text-(--text)"
      data-library-filter
    >
      <ListFilter class="size-3.5 text-accent" aria-hidden="true" />
      Showing {{ props.trackFilter.type }}: {{ props.trackFilter.label }}
      <button
        aria-label="Clear library filter"
        class="ml-auto grid size-5 cursor-pointer place-items-center rounded border-0 bg-transparent text-(--muted-text) hover:text-(--text)"
        type="button"
        @click="emit('clearTrackFilter')"
      >
        <X class="size-3.5" aria-hidden="true" />
      </button>
    </div>
    <table
      class="w-full shrink-0 table-fixed px-5 text-left"
      data-library-track-header
    >
      <colgroup>
        <col
          v-for="(width, index) in columnWidths"
          :key="index"
          :data-column="
            ['action', 'title', 'artist', 'album', 'duration', 'favorite'][
              index
            ]
          "
          :style="{ width: `${width}%` }"
        />
      </colgroup>
      <thead>
        <tr>
          <th class="px-2 pb-1.5 text-center text-(--subtle-text)">
            <span class="sr-only">Play</span>
          </th>
          <th
            class="relative px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
          >
            Title
            <button
              aria-label="Resize Title column"
              aria-orientation="vertical"
              :aria-valuenow="columnWidths[1]"
              class="column-resize-handle absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize border-0 bg-transparent p-0"
              role="separator"
              type="button"
              @mousedown.stop="startColumnResize(1, $event)"
              @keydown="resizeColumnWithKeyboard(1, $event)"
            />
          </th>
          <th
            class="relative px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
          >
            Artist
            <button
              aria-label="Resize Artist column"
              aria-orientation="vertical"
              :aria-valuenow="columnWidths[2]"
              class="column-resize-handle absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize border-0 bg-transparent p-0"
              role="separator"
              type="button"
              @mousedown.stop="startColumnResize(2, $event)"
              @keydown="resizeColumnWithKeyboard(2, $event)"
            />
          </th>
          <th
            class="relative px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
          >
            Album
            <button
              aria-label="Resize Album column"
              aria-orientation="vertical"
              :aria-valuenow="columnWidths[3]"
              class="column-resize-handle absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize border-0 bg-transparent p-0"
              role="separator"
              type="button"
              @mousedown.stop="startColumnResize(3, $event)"
              @keydown="resizeColumnWithKeyboard(3, $event)"
            />
          </th>
          <th
            class="relative px-2 pb-1.5 text-center text-(--subtle-text) [&>svg]:mx-auto [&>svg]:size-3.75"
          >
            <span class="sr-only">Duration</span>
            <Clock3 aria-hidden="true" />
            <button
              aria-label="Resize Duration column"
              aria-orientation="vertical"
              :aria-valuenow="columnWidths[4]"
              class="column-resize-handle absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize border-0 bg-transparent p-0"
              role="separator"
              type="button"
              @mousedown.stop="startColumnResize(4, $event)"
              @keydown="resizeColumnWithKeyboard(4, $event)"
            />
          </th>
          <th
            class="px-2 pb-1.5 text-center text-(--subtle-text) [&>svg]:mx-auto [&>svg]:size-3.75"
          >
            <span class="sr-only">Favorite</span>
            <Heart aria-hidden="true" />
          </th>
        </tr>
      </thead>
    </table>

    <div
      class="library-track-scroll min-h-0 flex-1 overflow-auto"
      data-library-track-list
      :ref="setTrackList"
    >
      <div
        class="relative min-w-0"
        data-library-track-virtualizer
        role="rowgroup"
        :style="{ height: virtualTrackHeight }"
      >
        <div
          v-for="{ track, virtualItem } in virtualTracks"
          :key="String(virtualItem.key)"
          :aria-label="`${track.title} by ${track.artist}`"
          class="group absolute left-0 grid h-10.5 w-full cursor-pointer outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
          :data-current="track.id === props.currentItemId"
          :data-index="virtualItem.index"
          :data-track-id="track.id"
          role="row"
          tabindex="0"
          :style="{
            gridTemplateColumns: trackGridTemplateColumns,
            transform: `translateY(${virtualItem.start}px)`,
          }"
          @click="emit('selectTrack', track)"
          @dblclick="emit('playTrack', track)"
          @keydown.enter.prevent="emit('selectTrack', track)"
          @keydown.space.prevent="emit('selectTrack', track)"
        >
          <div
            class="relative grid place-items-center rounded-l-md px-2 group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
            role="gridcell"
          >
            <Volume2
              v-if="track.id === props.currentItemId"
              class="track-playing-indicator size-3.75 text-(--text)"
              aria-label="Currently playing"
            />
            <button
              :aria-label="`Play ${track.title}`"
              class="absolute grid size-7 cursor-pointer place-items-center border-0 bg-transparent p-0 text-(--muted-text) opacity-0 transition-[color,opacity] group-hover:opacity-100 group-focus-within:opacity-100 hover:text-(--text) focus-visible:opacity-100 [&>svg]:size-3.5"
              data-track-action="play"
              type="button"
              @click.stop="emit('playTrack', track)"
              @dblclick.stop
            >
              <Play aria-hidden="true" />
            </button>
          </div>
          <div
            class="overflow-hidden px-4 text-[0.82rem] text-(--text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
            role="gridcell"
          >
            <span class="flex h-full min-w-0 items-center gap-2.5 font-medium">
              <span
                class="track-title min-w-0 overflow-hidden text-ellipsis whitespace-nowrap"
              >
                {{ track.title }}
              </span>
              <LoaderCircle
                v-if="track.metadataDirty"
                class="size-3.5 shrink-0 animate-spin text-amber-200"
                data-metadata-dirty
                aria-label="Metadata refresh pending"
                role="status"
              />
            </span>
          </div>
          <div
            class="overflow-hidden px-4 text-[0.8rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
            role="gridcell"
          >
            <span
              class="track-artist block overflow-hidden text-ellipsis whitespace-nowrap leading-10.5"
            >
              {{ track.artist }}
            </span>
          </div>
          <div
            class="overflow-hidden px-4 text-[0.8rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
            role="gridcell"
          >
            <span
              class="track-album block overflow-hidden text-ellipsis whitespace-nowrap leading-10.5"
            >
              {{ track.album || "—" }}
            </span>
          </div>
          <div
            class="px-2 text-center text-[0.78rem] leading-10.5 text-(--muted-text) tabular-nums group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
            role="gridcell"
          >
            {{ formatDuration(track.durationMs) }}
          </div>
          <div
            class="grid place-items-center rounded-r-md px-2 group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
            role="gridcell"
          >
            <button
              :aria-label="`Favorite ${track.title}`"
              :aria-pressed="isFavorite(track.id)"
              class="grid size-7 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-(--subtle-text) transition-colors hover:bg-[oklch(0.74_0.05_300/0.1)] hover:text-(--text) aria-pressed:text-accent [&>svg]:size-4"
              type="button"
              @click.stop="toggleFavorite(track.id)"
            >
              <Heart
                aria-hidden="true"
                :fill="isFavorite(track.id) ? 'currentColor' : 'none'"
              />
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.library-track-scroll {
  -webkit-mask-image: linear-gradient(to bottom, transparent, black 24px);
  -webkit-mask-origin: content-box;
  -webkit-mask-clip: content-box;
  mask-image: linear-gradient(to bottom, transparent, black 24px);
  mask-origin: content-box;
  mask-clip: content-box;
}
</style>
