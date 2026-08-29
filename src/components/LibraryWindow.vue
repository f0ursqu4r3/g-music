<script setup lang="ts">
import {
  CarFront,
  CassetteTape,
  CircleDot,
  Clock3,
  Disc3,
  Ellipsis,
  Grid2X2,
  Heart,
  List,
  ListMusic,
  LoaderCircle,
  Mic2,
  Pause,
  Play,
  Plus,
  Repeat2,
  Shuffle,
  SkipBack,
  SkipForward,
  Sparkles,
  Volume2,
} from 'lucide-vue-next';
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue';

import type {
  MediaItem,
  MetadataRefreshSnapshot,
  PlaybackSnapshot,
} from '@/api';
import { Button } from '@/components/ui/button';
import { Slider } from '@/components/ui/slider';
import { formatDuration } from '@/lib/time';
import MetadataRefreshDrawer from './MetadataRefreshDrawer.vue';
import YouTubeArtwork from './YouTubeArtwork.vue';

type LibraryCollection = 'tracks' | 'albums' | 'artists';

interface LibraryAlbum {
  artist: string;
  title: string;
  videoId: string;
}

interface LibraryArtist {
  detail: string;
  name: string;
  videoId: string;
}

interface Props {
  snapshot: PlaybackSnapshot;
  isUpdating: boolean;
  errorMessage?: string;
  metadataRefreshes?: MetadataRefreshSnapshot;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [percent: number];
  openImport: [];
  playTrack: [id: string];
}>();

const playlists = [
  { href: '#favorites', icon: Heart, label: 'Favorites' },
  { href: '#chill-vibes', icon: Sparkles, label: 'Chill Vibes' },
  { href: '#focus', icon: CircleDot, label: 'Focus' },
  { href: '#road-trip', icon: CarFront, label: 'Road Trip' },
  { href: '#90s-mix', icon: CassetteTape, label: '90s Mix' },
] as const;

const activeCollection = ref<LibraryCollection>('tracks');
const favoriteTrackIds = ref(new Set<string>());
const metadataRefreshDrawerOpen = ref(false);
const columnWidths = ref([35, 25, 25, 9, 6]);
const minimumColumnWidths = [18, 12, 12, 7, 5] as const;
const trackList = ref<HTMLElement>();
const trackListHeight = ref(600);
const trackListScrollTop = ref(0);
const trackRowHeight = 44;
const trackOverscan = 6;
let stopColumnResize: (() => void) | undefined;
let trackListResizeObserver: ResizeObserver | undefined;
const isPlaying = computed(() => props.snapshot.status === 'playing');
const currentItem = computed(() => props.snapshot.currentItem);
const libraryTracks = computed(() => props.snapshot.queue);
const virtualTrackRange = computed(() => {
  const start = Math.max(
    Math.floor(trackListScrollTop.value / trackRowHeight) - trackOverscan,
    0
  );
  const visibleCount = Math.ceil(trackListHeight.value / trackRowHeight);
  const end = Math.min(
    start + visibleCount + trackOverscan * 2,
    libraryTracks.value.length
  );

  return { end, start };
});
const visibleTracks = computed(() =>
  libraryTracks.value.slice(
    virtualTrackRange.value.start,
    virtualTrackRange.value.end
  )
);
const trackSpacerBefore = computed(
  () => virtualTrackRange.value.start * trackRowHeight
);
const trackSpacerAfter = computed(
  () =>
    (libraryTracks.value.length - virtualTrackRange.value.end) * trackRowHeight
);
const libraryAlbums = computed<LibraryAlbum[]>(() => {
  const albums = new Map<string, LibraryAlbum>();

  for (const track of libraryTracks.value) {
    const title = track.album?.trim();
    if (!title) {
      continue;
    }

    const key = `${track.artist}\u0000${title}`;
    if (!albums.has(key)) {
      albums.set(key, { artist: track.artist, title, videoId: track.id });
    }
  }

  return [...albums.values()];
});
const libraryArtists = computed<LibraryArtist[]>(() => {
  const artists = new Map<string, { count: number; track: MediaItem }>();

  for (const track of libraryTracks.value) {
    const existing = artists.get(track.artist);
    artists.set(track.artist, {
      count: (existing?.count ?? 0) + 1,
      track: existing?.track ?? track,
    });
  }

  return [...artists.entries()].map(([name, value]) => ({
    detail: `${value.count} ${value.count === 1 ? 'song' : 'songs'}`,
    name,
    videoId: value.track.id,
  }));
});
const collectionTitle = computed(() => {
  const titles: Record<LibraryCollection, string> = {
    albums: 'Albums',
    artists: 'Artists',
    tracks: 'Tracks',
  };

  return titles[activeCollection.value];
});
const collectionSummary = computed(() => {
  const summaries: Record<LibraryCollection, string> = {
    albums: `${libraryAlbums.value.length} ${libraryAlbums.value.length === 1 ? 'album' : 'albums'}`,
    artists: `${libraryArtists.value.length} ${libraryArtists.value.length === 1 ? 'artist' : 'artists'}`,
    tracks: trackCollectionSummary(libraryTracks.value),
  };

  return summaries[activeCollection.value];
});
const metadataRefreshRemaining = computed(() =>
  Math.max(
    (props.metadataRefreshes?.totalTracks ?? 0) -
      (props.metadataRefreshes?.completedTracks ?? 0),
    0
  )
);
const hasActiveMetadataRefresh = computed(
  () =>
    metadataRefreshRemaining.value > 0 &&
    (props.metadataRefreshes?.jobs.some(
      (job) => job.state === 'queued' || job.state === 'refreshing'
    ) ??
      false)
);
function trackCollectionSummary(tracks: MediaItem[]): string {
  const count = tracks.length;
  const totalMinutes = Math.round(
    tracks.reduce((total, track) => total + track.durationMs, 0) / 60_000
  );

  return `${count} ${count === 1 ? 'song' : 'songs'} · ${totalMinutes} min`;
}

function selectCollection(collection: LibraryCollection): void {
  activeCollection.value = collection;
}

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

function emitVolume(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit('setVolume', value);
  }
}

function emitSeek(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit('seek', value);
  }
}

function resizeColumnBoundary(
  boundaryIndex: number,
  requestedDelta: number,
  initialWidths = columnWidths.value
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
    rightWidth - minimumRightWidth
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
  const table = (event.currentTarget as HTMLElement).closest('table');

  const handleMouseMove = (moveEvent: MouseEvent): void => {
    const tableWidth = table?.getBoundingClientRect().width ?? 0;
    if (tableWidth <= 0) {
      return;
    }

    const delta = ((moveEvent.clientX - startX) / tableWidth) * 100;
    resizeColumnBoundary(boundaryIndex, delta, initialWidths);
  };
  const handleMouseUp = (): void => {
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', handleMouseUp);
    stopColumnResize = undefined;
  };

  stopColumnResize = handleMouseUp;
  window.addEventListener('mousemove', handleMouseMove);
  window.addEventListener('mouseup', handleMouseUp);
}

function resizeColumnWithKeyboard(
  boundaryIndex: number,
  event: KeyboardEvent
): void {
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') {
    return;
  }

  event.preventDefault();
  resizeColumnBoundary(boundaryIndex, event.key === 'ArrowRight' ? 1 : -1);
}

function playTrack(id: string): void {
  if (!props.isUpdating) {
    emit('playTrack', id);
  }
}

function updateTrackListViewport(): void {
  trackListHeight.value = trackList.value?.clientHeight || 600;
}

function observeTrackList(): void {
  trackListResizeObserver?.disconnect();
  updateTrackListViewport();

  if (!trackList.value || typeof ResizeObserver === 'undefined') {
    return;
  }

  trackListResizeObserver = new ResizeObserver(updateTrackListViewport);
  trackListResizeObserver.observe(trackList.value);
}

function handleTrackListScroll(event: Event): void {
  trackListScrollTop.value = (event.currentTarget as HTMLElement).scrollTop;
}

onMounted(observeTrackList);

watch(activeCollection, () => void nextTick(observeTrackList));

watch(libraryTracks, () => {
  const maximumScrollTop = Math.max(
    libraryTracks.value.length * trackRowHeight - trackListHeight.value,
    0
  );
  const nextScrollTop = Math.min(trackListScrollTop.value, maximumScrollTop);

  if (trackList.value && trackList.value.scrollTop !== nextScrollTop) {
    trackList.value.scrollTop = nextScrollTop;
  }
  trackListScrollTop.value = nextScrollTop;
});

onBeforeUnmount(() => {
  stopColumnResize?.();
  trackListResizeObserver?.disconnect();
});
</script>

<template>
  <main
    class="library-window relative grid h-screen min-h-0 grid-cols-[244px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_104px] overflow-hidden bg-(--glass-window) text-(--text) backdrop-saturate-[1.2] max-[760px]:grid-cols-1"
    aria-label="Music library"
  >
    <div
      class="application-drag-region absolute top-0 right-0 left-0 z-10 h-13"
      data-tauri-drag-region
      aria-hidden="true"
    />

    <aside
      class="col-start-1 row-start-1 min-h-0 overflow-y-auto p-4 mt-8 max-[760px]:hidden"
      data-library-sidebar
    >
      <nav class="grid gap-0.5" aria-label="Library navigation">
        <div
          class="mb-1 flex items-center justify-between px-2.5"
          data-library-heading="library"
        >
          <p
            class="text-[0.61rem] font-semibold tracking-[0.06em] text-(--subtle-text)"
          >
            Library
          </p>
          <button
            aria-label="Import music"
            class="grid size-6 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4"
            type="button"
            @click="emit('openImport')"
          >
            <Plus aria-hidden="true" />
          </button>
        </div>
        <button
          :aria-current="activeCollection === 'tracks' ? 'page' : undefined"
          class="flex min-h-8.5 w-full cursor-pointer items-center gap-2.5 rounded-md border-0 bg-transparent px-2.5 text-left text-[0.82rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.7_0.03_262/0.15)] aria-[current=page]:text-(--text) aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
          :data-collection="'tracks'"
          type="button"
          @click="selectCollection('tracks')"
        >
          <ListMusic aria-hidden="true" />Tracks
        </button>
        <button
          :aria-current="activeCollection === 'albums' ? 'page' : undefined"
          class="flex min-h-8.5 w-full cursor-pointer items-center gap-2.5 rounded-md border-0 bg-transparent px-2.5 text-left text-[0.82rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.7_0.03_262/0.15)] aria-[current=page]:text-(--text) aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
          :data-collection="'albums'"
          type="button"
          @click="selectCollection('albums')"
        >
          <Disc3 aria-hidden="true" />Albums
        </button>
        <button
          :aria-current="activeCollection === 'artists' ? 'page' : undefined"
          class="flex min-h-8.5 w-full cursor-pointer items-center gap-2.5 rounded-md border-0 bg-transparent px-2.5 text-left text-[0.82rem] text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.7_0.03_262/0.15)] aria-[current=page]:text-(--text) aria-[current=page]:[&>svg]:text-accent [&>svg]:size-4"
          :data-collection="'artists'"
          type="button"
          @click="selectCollection('artists')"
        >
          <Mic2 aria-hidden="true" />Artists
        </button>
        <span
          class="flex min-h-8.5 items-center gap-2.5 rounded-md px-2.5 text-[0.82rem] text-(--muted-text) [&>svg]:size-4"
          data-library-destination="playlists"
        >
          <ListMusic aria-hidden="true" />Playlists
        </span>
      </nav>

      <nav
        class="mt-5 grid gap-0.5"
        aria-label="Playlists"
        data-library-playlists
      >
        <div class="mb-1 flex items-center justify-between px-2.5">
          <p
            class="text-[0.61rem] font-semibold tracking-[0.06em] text-(--subtle-text)"
          >
            Playlists
          </p>
          <button
            aria-label="New playlist"
            class="grid size-6 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--subtle-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4"
            type="button"
          >
            <Plus aria-hidden="true" />
          </button>
        </div>
        <a
          v-for="playlist in playlists"
          :key="playlist.href"
          class="flex min-h-8 items-center gap-2.5 rounded-md px-2.5 text-[0.79rem] text-(--muted-text) no-underline transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4"
          :href="playlist.href"
        >
          <component :is="playlist.icon" aria-hidden="true" />
          {{ playlist.label }}
        </a>
      </nav>
    </aside>

    <section
      class="library-content col-start-2 row-start-1 grid min-h-0 min-w-0 grid-rows-[104px_minmax(0,1fr)] max-[760px]:col-start-1 border-l border-(--line)"
    >
      <header
        class="flex items-center justify-between gap-6 border-b border-(--line) px-8 pt-3"
      >
        <div>
          <h1 class="text-2xl font-semibold tracking-[-0.035em] text-(--text)">
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
              @click="metadataRefreshDrawerOpen = !metadataRefreshDrawerOpen"
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
            :aria-pressed="activeCollection === 'tracks'"
            class="grid size-8.5 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:bg-[oklch(0.72_0.025_258/0.13)] aria-pressed:text-(--text) [&>svg]:size-4.5"
            type="button"
            @click="selectCollection('tracks')"
          >
            <List aria-hidden="true" />
          </button>
          <button
            aria-label="Grid view"
            :aria-pressed="activeCollection === 'albums'"
            class="grid size-8.5 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:bg-[oklch(0.72_0.025_258/0.13)] aria-pressed:text-(--text) [&>svg]:size-4.5"
            type="button"
            @click="selectCollection('albums')"
          >
            <Grid2X2 aria-hidden="true" />
          </button>

          <button
            aria-label="More library options"
            aria-haspopup="menu"
            class="grid size-8.5 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) [&>svg]:size-4.5"
            type="button"
          >
            <Ellipsis aria-hidden="true" />
          </button>
        </nav>
      </header>

      <div
        v-if="activeCollection === 'tracks'"
        class="flex min-h-0 flex-col py-2.5 pl-2"
      >
        <table
          class="w-full shrink-0 table-fixed text-left px-5"
          data-library-track-header
        >
          <colgroup>
            <col
              v-for="(width, index) in columnWidths"
              :key="index"
              :data-column="
                ['title', 'artist', 'album', 'duration', 'favorite'][index]
              "
              :style="{ width: `${width}%` }"
            />
          </colgroup>
          <thead>
            <tr>
              <th
                class="relative px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
              >
                Title
                <button
                  aria-label="Resize Title column"
                  aria-orientation="vertical"
                  :aria-valuenow="columnWidths[0]"
                  class="column-resize-handle absolute top-0 -right-1 z-10 h-full w-2 cursor-col-resize border-0 bg-transparent p-0"
                  role="separator"
                  type="button"
                  @mousedown.stop="startColumnResize(0, $event)"
                  @keydown="resizeColumnWithKeyboard(0, $event)"
                />
              </th>
              <th
                class="relative px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
              >
                Artist
                <button
                  aria-label="Resize Artist column"
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
                Album
                <button
                  aria-label="Resize Album column"
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
                class="relative px-2 pb-1.5 text-center text-(--subtle-text) [&>svg]:mx-auto [&>svg]:size-3.75"
              >
                <span class="sr-only">Duration</span>
                <Clock3 aria-hidden="true" />
                <button
                  aria-label="Resize Duration column"
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
          ref="trackList"
          @scroll="handleTrackListScroll"
        >
          <table
            class="w-full table-fixed border-separate border-spacing-y-0.5 text-left"
          >
            <colgroup>
              <col
                v-for="(width, index) in columnWidths"
                :key="index"
                :data-column="
                  ['title', 'artist', 'album', 'duration', 'favorite'][index]
                "
                :style="{ width: `${width}%` }"
              />
            </colgroup>
            <tbody>
              <tr v-if="trackSpacerBefore" aria-hidden="true">
                <td
                  :colspan="columnWidths.length"
                  :style="{ height: `${trackSpacerBefore}px` }"
                />
              </tr>
              <tr
                v-for="track in visibleTracks"
                :key="track.id"
                :aria-label="`Play ${track.title}`"
                class="group cursor-pointer outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
                :data-current="track.id === currentItem?.id"
                :data-track-id="track.id"
                tabindex="0"
                @click="playTrack(track.id)"
                @keydown.enter.prevent="playTrack(track.id)"
                @keydown.space.prevent="playTrack(track.id)"
              >
                <td
                  class="h-10.5 overflow-hidden rounded-l-md px-4 text-[0.82rem] text-(--text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
                >
                  <span class="flex min-w-0 items-center gap-2.5 font-medium">
                    <Volume2
                      v-if="track.id === currentItem?.id"
                      class="track-playing-indicator size-3.75 shrink-0 text-(--text)"
                      aria-label="Currently playing"
                    />
                    <span
                      v-else
                      class="size-3.75 shrink-0"
                      aria-hidden="true"
                    />
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
                </td>
                <td
                  class="h-10.5 overflow-hidden px-4 text-[0.8rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
                >
                  <span
                    class="track-artist block overflow-hidden text-ellipsis whitespace-nowrap"
                  >
                    {{ track.artist }}
                  </span>
                </td>
                <td
                  class="h-10.5 overflow-hidden px-4 text-[0.8rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
                >
                  <span
                    class="track-album block overflow-hidden text-ellipsis whitespace-nowrap"
                  >
                    {{ track.album || '—' }}
                  </span>
                </td>
                <td
                  class="h-10.5 px-2 text-center text-[0.78rem] text-(--muted-text) tabular-nums group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
                >
                  {{ formatDuration(track.durationMs) }}
                </td>
                <td
                  class="h-10.5 rounded-r-md px-2 text-center group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
                >
                  <button
                    :aria-label="`Favorite ${track.title}`"
                    :aria-pressed="isFavorite(track.id)"
                    class="mx-auto grid size-7 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-(--subtle-text) transition-colors hover:bg-[oklch(0.74_0.05_300/0.1)] hover:text-(--text) aria-pressed:text-accent [&>svg]:size-4"
                    type="button"
                    @click.stop="toggleFavorite(track.id)"
                  >
                    <Heart
                      aria-hidden="true"
                      :fill="isFavorite(track.id) ? 'currentColor' : 'none'"
                    />
                  </button>
                </td>
              </tr>
              <tr v-if="trackSpacerAfter" aria-hidden="true">
                <td
                  :colspan="columnWidths.length"
                  :style="{ height: `${trackSpacerAfter}px` }"
                />
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <section
        v-else-if="activeCollection === 'albums'"
        class="grid min-h-0 grid-cols-[repeat(auto-fit,minmax(118px,1fr))] content-start gap-x-4.25 gap-y-6 overflow-auto px-7 py-6"
        aria-label="Albums"
      >
        <article
          v-for="album in libraryAlbums"
          :key="`${album.artist}:${album.title}`"
          class="album-tile min-w-0"
        >
          <div
            class="cover-art grid aspect-square w-full place-items-center rounded-lg [&>svg]:size-[34%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
          >
            <Disc3 aria-hidden="true" />
            <YouTubeArtwork
              class="absolute inset-0 size-full object-cover"
              :video-id="album.videoId"
            />
          </div>
          <h2
            class="mt-2 overflow-hidden text-[0.78rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ album.title }}
          </h2>
          <p class="mt-0.5 text-[0.69rem] text-(--muted-text)">
            {{ album.artist }}
          </p>
        </article>
      </section>

      <section
        v-else
        class="grid min-h-0 grid-cols-[repeat(auto-fit,minmax(200px,1fr))] content-start gap-4 overflow-auto px-7 py-6"
        aria-label="Artists"
      >
        <article
          v-for="artist in libraryArtists"
          :key="artist.name"
          class="artist-tile flex min-w-0 items-center gap-3 rounded-lg p-2 hover:bg-[oklch(0.72_0.025_258/0.08)]"
        >
          <div
            class="cover-art grid size-14 shrink-0 place-items-center rounded-full [&>svg]:size-6 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
          >
            <Mic2 aria-hidden="true" />
            <YouTubeArtwork
              class="absolute inset-0 size-full object-cover"
              :video-id="artist.videoId"
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
      </section>
    </section>

    <footer class="contents">
      <div
        class="relative col-start-1 row-start-2 flex min-w-0 items-center gap-3 border-t border-(--line) pr-4 max-[760px]:hidden"
      >
        <div class="cover-art h-full aspect-square shrink-0">
          <YouTubeArtwork
            class="absolute inset-0 size-full object-cover"
            :video-id="currentItem?.id"
          />
        </div>
        <div class="min-w-0">
          <p
            class="overflow-hidden text-[0.82rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ currentItem?.title ?? 'Nothing selected' }}
          </p>
          <span
            class="mt-0.5 block overflow-hidden text-[0.74rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
          >
            {{ currentItem?.artist ?? 'Choose a track' }}
          </span>
        </div>
      </div>

      <div
        class="col-start-2 row-start-2 grid min-w-0 grid-rows-[1fr_auto] border-t border-l border-(--line) px-6 pt-2 pb-2 max-[760px]:col-start-1"
      >
        <div class="grid grid-cols-[1fr_auto_1fr] items-center gap-4">
          <span aria-hidden="true" />
          <nav
            class="flex items-center justify-center gap-1.5"
            aria-label="Playback controls"
          >
            <Button aria-label="Shuffle" size="icon-sm" variant="ghost">
              <Shuffle aria-hidden="true" />
            </Button>
            <Button
              aria-label="Previous track"
              size="icon-sm"
              variant="ghost"
              :disabled="isUpdating"
              @click="emit('previous')"
            >
              <SkipBack aria-hidden="true" />
            </Button>
            <Button
              :aria-label="isPlaying ? 'Pause' : 'Play'"
              class="size-10 rounded-full bg-(--text) text-(--accent-ink) hover:bg-(--text)"
              size="icon"
              :disabled="isUpdating"
              @click="emit('toggle')"
            >
              <Pause v-if="isPlaying" aria-hidden="true" fill="currentColor" />
              <Play v-else aria-hidden="true" fill="currentColor" />
            </Button>
            <Button
              aria-label="Next track"
              size="icon-sm"
              variant="ghost"
              :disabled="isUpdating"
              @click="emit('next')"
            >
              <SkipForward aria-hidden="true" />
            </Button>
            <Button aria-label="Repeat" size="icon-sm" variant="ghost">
              <Repeat2 aria-hidden="true" />
            </Button>
          </nav>

          <label
            class="ml-auto flex w-40 items-center gap-2 text-(--muted-text) [&>svg]:size-4"
          >
            <Volume2 aria-hidden="true" />
            <span class="sr-only">Volume</span>
            <Slider
              aria-label="Volume"
              :min="0"
              :max="100"
              :step="1"
              :model-value="[snapshot.volumePercent]"
              :disabled="isUpdating"
              @value-commit="emitVolume"
            />
          </label>
        </div>

        <div
          class="grid grid-cols-[30px_minmax(0,1fr)_30px] items-center gap-2 text-[0.65rem] text-(--muted-text) tabular-nums"
          aria-label="Track progress"
        >
          <span>{{ formatDuration(snapshot.positionMs) }}</span>
          <Slider
            aria-label="Track progress"
            :min="0"
            :max="currentItem?.durationMs ?? 0"
            :step="1000"
            :model-value="[snapshot.positionMs]"
            :disabled="isUpdating || !currentItem"
            @value-commit="emitSeek"
          />
          <span class="text-right">
            {{ formatDuration(currentItem?.durationMs ?? 0) }}
          </span>
        </div>
      </div>
    </footer>

    <MetadataRefreshDrawer
      v-if="
        metadataRefreshDrawerOpen &&
        hasActiveMetadataRefresh &&
        metadataRefreshes
      "
      :refreshes="metadataRefreshes"
    />
  </main>
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

.library-track-scroll {
  -webkit-mask-image: linear-gradient(to bottom, transparent, black 24px);
  -webkit-mask-origin: content-box;
  -webkit-mask-clip: content-box;
  mask-image: linear-gradient(to bottom, transparent, black 24px);
  mask-origin: content-box;
  mask-clip: content-box;
}
</style>
