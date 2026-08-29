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
  ListFilter,
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
  X,
} from "lucide-vue-next";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";

import type {
  MediaItem,
  MetadataRefreshSnapshot,
  PlaybackSnapshot,
} from "@/api";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { formatDuration } from "@/lib/time";
import MetadataRefreshDrawer from "./MetadataRefreshDrawer.vue";
import YouTubeArtwork from "./YouTubeArtwork.vue";

type LibraryCollection = "tracks" | "albums" | "artists";
type LibraryDisplayMode = "grid" | "list";
type LibrarySortOption =
  | "album-asc"
  | "artist-asc"
  | "duration-asc"
  | "duration-desc"
  | "title-asc"
  | "title-desc";
type LibraryGroupOption = "album" | "artist" | "none";

interface LibraryAlbum {
  key: string;
  artist: string;
  durationMs: number;
  trackCount: number;
  title: string;
  videoId: string;
}

interface LibraryArtist {
  detail: string;
  durationMs: number;
  name: string;
  trackCount: number;
  albumCount: number;
  videoId: string;
}

interface TrackFilter {
  label: string;
  type: "album" | "artist";
  value: string;
}

type SelectedLibraryItem =
  | { id: string; kind: "track" }
  | { key: string; kind: "album" }
  | { kind: "artist"; name: string };

interface TrackGroup {
  items: MediaItem[];
  label: string;
}

interface AlbumGroup {
  items: LibraryAlbum[];
  label: string;
}

interface ArtistGroup {
  items: LibraryArtist[];
  label: string;
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
  { href: "#favorites", icon: Heart, label: "Favorites" },
  { href: "#chill-vibes", icon: Sparkles, label: "Chill Vibes" },
  { href: "#focus", icon: CircleDot, label: "Focus" },
  { href: "#road-trip", icon: CarFront, label: "Road Trip" },
  { href: "#90s-mix", icon: CassetteTape, label: "90s Mix" },
] as const;

const activeCollection = ref<LibraryCollection>("tracks");
const displayMode = ref<LibraryDisplayMode>("list");
const favoriteTrackIds = ref(new Set<string>());
const gridItemSize = ref(176);
const groupBy = ref<LibraryGroupOption>("none");
const libraryOptionsOpen = ref(false);
const metadataRefreshDrawerOpen = ref(false);
const columnWidths = ref([35, 25, 25, 9, 6]);
const minimumColumnWidths = [18, 12, 12, 7, 5] as const;
const selectedLibraryItem = ref<SelectedLibraryItem | null>(
  props.snapshot.currentItem
    ? { id: props.snapshot.currentItem.id, kind: "track" }
    : null,
);
const sortBy = ref<LibrarySortOption>("title-asc");
const trackFilter = ref<TrackFilter | null>(null);
const trackList = ref<HTMLElement>();
const trackListHeight = ref(600);
const trackListScrollTop = ref(0);
const trackRowHeight = 44;
const trackOverscan = 6;
let stopColumnResize: (() => void) | undefined;
let trackListResizeObserver: ResizeObserver | undefined;
const isPlaying = computed(() => props.snapshot.status === "playing");
const currentItem = computed(() => props.snapshot.currentItem);
const allTracks = computed(() => props.snapshot.queue);
const selectedTrack = computed(() => {
  const selection = selectedLibraryItem.value;
  if (!selection || selection.kind !== "track") {
    return null;
  }

  return allTracks.value.find((track) => track.id === selection.id) ?? null;
});
const libraryTracks = computed(() => {
  const filteredTracks = trackFilter.value
    ? allTracks.value.filter((track) => {
        if (trackFilter.value?.type === "artist") {
          return track.artist === trackFilter.value.value;
        }

        return (
          `${track.artist}\u0000${track.album ?? ""}` ===
          trackFilter.value?.value
        );
      })
    : allTracks.value;

  return sortMediaItems(filteredTracks, sortBy.value);
});
const virtualTrackRange = computed(() => {
  const start = Math.max(
    Math.floor(trackListScrollTop.value / trackRowHeight) - trackOverscan,
    0,
  );
  const visibleCount = Math.ceil(trackListHeight.value / trackRowHeight);
  const end = Math.min(
    start + visibleCount + trackOverscan * 2,
    libraryTracks.value.length,
  );

  return { end, start };
});
const visibleTracks = computed(() =>
  libraryTracks.value.slice(
    virtualTrackRange.value.start,
    virtualTrackRange.value.end,
  ),
);
const trackSpacerBefore = computed(
  () => virtualTrackRange.value.start * trackRowHeight,
);
const trackSpacerAfter = computed(
  () =>
    (libraryTracks.value.length - virtualTrackRange.value.end) * trackRowHeight,
);
const libraryAlbums = computed<LibraryAlbum[]>(() => {
  const albums = new Map<string, LibraryAlbum>();

  for (const track of allTracks.value) {
    const title = track.album?.trim();
    if (!title) {
      continue;
    }

    const key = `${track.artist}\u0000${title}`;
    const album = albums.get(key) ?? {
      artist: track.artist,
      durationMs: 0,
      key,
      title,
      trackCount: 0,
      videoId: track.id,
    };
    album.durationMs += track.durationMs;
    album.trackCount += 1;
    albums.set(key, album);
  }

  return sortCollection(albums.values(), sortBy.value, "album");
});
const libraryArtists = computed<LibraryArtist[]>(() => {
  const artists = new Map<string, { count: number; track: MediaItem }>();

  for (const track of allTracks.value) {
    const existing = artists.get(track.artist);
    artists.set(track.artist, {
      count: (existing?.count ?? 0) + 1,
      track: existing?.track ?? track,
    });
  }

  return sortCollection(
    [...artists.entries()].map(([name, value]) => ({
      albumCount: new Set(
        allTracks.value
          .filter((track) => track.artist === name)
          .map((track) => track.album)
          .filter(Boolean),
      ).size,
      detail: `${value.count} ${value.count === 1 ? "song" : "songs"}`,
      durationMs: allTracks.value
        .filter((track) => track.artist === name)
        .reduce((total, track) => total + track.durationMs, 0),
      name,
      trackCount: value.count,
      videoId: value.track.id,
    })),
    sortBy.value,
    "artist",
  );
});
const selectedAlbum = computed(() => {
  const selection = selectedLibraryItem.value;
  if (!selection || selection.kind !== "album") {
    return null;
  }

  return (
    libraryAlbums.value.find((album) => album.key === selection.key) ?? null
  );
});
const selectedArtist = computed(() => {
  const selection = selectedLibraryItem.value;
  if (!selection || selection.kind !== "artist") {
    return null;
  }

  return (
    libraryArtists.value.find((artist) => artist.name === selection.name) ??
    null
  );
});
const selectedInfoKind = computed(
  () => selectedLibraryItem.value?.kind ?? "empty",
);
const groupedTracks = computed<TrackGroup[]>(() => {
  if (groupBy.value === "none") {
    return [{ items: libraryTracks.value, label: "" }];
  }

  const groups = new Map<string, MediaItem[]>();
  for (const track of libraryTracks.value) {
    const label =
      groupBy.value === "artist"
        ? track.artist
        : track.album?.trim() || "Unknown album";
    groups.set(label, [...(groups.get(label) ?? []), track]);
  }

  return [...groups.entries()].map(([label, items]) => ({ items, label }));
});
const groupedAlbums = computed<AlbumGroup[]>(() =>
  groupCollection(libraryAlbums.value, (album) =>
    groupBy.value === "artist" ? album.artist : album.title,
  ),
);
const groupedArtists = computed<ArtistGroup[]>(() =>
  groupCollection(libraryArtists.value, (artist) =>
    groupBy.value === "album" ? "Artists" : artist.name,
  ),
);
const collectionTitle = computed(() => {
  const titles: Record<LibraryCollection, string> = {
    albums: "Albums",
    artists: "Artists",
    tracks: "Tracks",
  };

  return titles[activeCollection.value];
});
const collectionSummary = computed(() => {
  const summaries: Record<LibraryCollection, string> = {
    albums: `${libraryAlbums.value.length} ${libraryAlbums.value.length === 1 ? "album" : "albums"}`,
    artists: `${libraryArtists.value.length} ${libraryArtists.value.length === 1 ? "artist" : "artists"}`,
    tracks: trackCollectionSummary(libraryTracks.value),
  };

  return summaries[activeCollection.value];
});
const metadataRefreshRemaining = computed(() =>
  Math.max(
    (props.metadataRefreshes?.totalTracks ?? 0) -
      (props.metadataRefreshes?.completedTracks ?? 0),
    0,
  ),
);
const hasActiveMetadataRefresh = computed(
  () =>
    metadataRefreshRemaining.value > 0 &&
    (props.metadataRefreshes?.jobs.some(
      (job) => job.state === "queued" || job.state === "refreshing",
    ) ??
      false),
);
function trackCollectionSummary(tracks: MediaItem[]): string {
  const count = tracks.length;
  const totalMinutes = Math.round(
    tracks.reduce((total, track) => total + track.durationMs, 0) / 60_000,
  );

  return `${count} ${count === 1 ? "song" : "songs"} · ${totalMinutes} min`;
}

function sortMediaItems(
  tracks: MediaItem[],
  option: LibrarySortOption,
): MediaItem[] {
  return sortCollection(tracks, option, "track");
}

function sortCollection<T extends MediaItem | LibraryAlbum | LibraryArtist>(
  items: Iterable<T>,
  option: LibrarySortOption,
  kind: "album" | "artist" | "track",
): T[] {
  const sorted = [...items];
  const collator = new Intl.Collator(undefined, {
    numeric: true,
    sensitivity: "base",
  });
  const direction = option.endsWith("desc") ? -1 : 1;

  sorted.sort((left, right) => {
    const leftValue = sortValue(left, option, kind);
    const rightValue = sortValue(right, option, kind);
    if (typeof leftValue === "number" && typeof rightValue === "number") {
      return (leftValue - rightValue) * direction;
    }

    return collator.compare(String(leftValue), String(rightValue)) * direction;
  });

  return sorted;
}

function sortValue(
  item: MediaItem | LibraryAlbum | LibraryArtist,
  option: LibrarySortOption,
  kind: "album" | "artist" | "track",
): number | string {
  if (kind === "artist") {
    return option.startsWith("duration")
      ? (item as LibraryArtist).durationMs
      : (item as LibraryArtist).name;
  }
  if (kind === "album") {
    const album = item as LibraryAlbum;
    return option.startsWith("artist")
      ? album.artist
      : option.startsWith("duration")
        ? album.durationMs
        : album.title;
  }

  const track = item as MediaItem;
  if (option.startsWith("artist")) return track.artist;
  if (option.startsWith("album")) return track.album ?? "";
  if (option.startsWith("duration")) return track.durationMs;
  return track.title;
}

function groupCollection<T>(
  items: T[],
  getLabel: (item: T) => string,
): Array<{ items: T[]; label: string }> {
  if (groupBy.value === "none") {
    return [{ items, label: "" }];
  }

  const groups = new Map<string, T[]>();
  for (const item of items) {
    const label = getLabel(item);
    groups.set(label, [...(groups.get(label) ?? []), item]);
  }

  return [...groups.entries()].map(([label, groupedItems]) => ({
    items: groupedItems,
    label,
  }));
}

function selectCollection(collection: LibraryCollection): void {
  activeCollection.value = collection;
}

function setDisplayMode(mode: LibraryDisplayMode): void {
  displayMode.value = mode;
}

function selectTrack(track: MediaItem): void {
  selectedLibraryItem.value = { id: track.id, kind: "track" };
  playTrack(track.id);
}

function selectAlbum(album: LibraryAlbum): void {
  selectedLibraryItem.value = { key: album.key, kind: "album" };
}

function selectArtist(artist: LibraryArtist): void {
  selectedLibraryItem.value = { kind: "artist", name: artist.name };
}

function openAlbum(album: LibraryAlbum): void {
  selectAlbum(album);
  trackFilter.value = { label: album.title, type: "album", value: album.key };
  activeCollection.value = "tracks";
  displayMode.value = "list";
}

function openArtist(artist: LibraryArtist): void {
  selectArtist(artist);
  trackFilter.value = {
    label: artist.name,
    type: "artist",
    value: artist.name,
  };
  activeCollection.value = "tracks";
  displayMode.value = "list";
}

function clearTrackFilter(): void {
  trackFilter.value = null;
}

function setSort(option: LibrarySortOption): void {
  sortBy.value = option;
  libraryOptionsOpen.value = false;
}

function setGroup(option: LibraryGroupOption): void {
  groupBy.value = option;
  libraryOptionsOpen.value = false;
}

function emitGridItemSize(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    gridItemSize.value = value;
  }
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
    emit("setVolume", value);
  }
}

function emitSeek(values: number[]): void {
  const value = values[0];
  if (Number.isFinite(value)) {
    emit("seek", value);
  }
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

function playTrack(id: string): void {
  if (!props.isUpdating) {
    emit("playTrack", id);
  }
}

function updateTrackListViewport(): void {
  trackListHeight.value = trackList.value?.clientHeight || 600;
}

function observeTrackList(): void {
  trackListResizeObserver?.disconnect();
  updateTrackListViewport();

  if (!trackList.value || typeof ResizeObserver === "undefined") {
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
    0,
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
    class="library-window relative grid h-screen min-h-0 grid-cols-[244px_minmax(0,1fr)_272px] grid-rows-[minmax(0,1fr)_104px] overflow-hidden bg-(--glass-window) text-(--text) backdrop-saturate-[1.2] max-[1040px]:grid-cols-[244px_minmax(0,1fr)] max-[760px]:grid-cols-1"
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
            :aria-pressed="displayMode === 'list'"
            class="grid size-8.5 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:bg-[oklch(0.72_0.025_258/0.13)] aria-pressed:text-(--text) [&>svg]:size-4.5"
            type="button"
            @click="setDisplayMode('list')"
          >
            <List aria-hidden="true" />
          </button>
          <button
            aria-label="Grid view"
            :aria-pressed="displayMode === 'grid'"
            class="grid size-8.5 cursor-pointer place-items-center rounded-md border-0 bg-transparent text-(--muted-text) transition-colors hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:bg-[oklch(0.72_0.025_258/0.13)] aria-pressed:text-(--text) [&>svg]:size-4.5"
            type="button"
            @click="setDisplayMode('grid')"
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
              @click="libraryOptionsOpen = !libraryOptionsOpen"
            >
              <Ellipsis aria-hidden="true" />
            </button>
            <div
              v-if="libraryOptionsOpen"
              class="library-options-menu absolute top-10 right-0 z-20 grid w-52 gap-1 rounded-lg border border-(--line) bg-(--glass-window) p-2 shadow-xl"
              role="menu"
            >
              <p
                class="px-2 py-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
              >
                Sort by
              </p>
              <button
                v-for="option in [
                  ['title-asc', 'Title'],
                  ['title-desc', 'Title (Z–A)'],
                  ['artist-asc', 'Artist'],
                  ['album-asc', 'Album'],
                  ['duration-desc', 'Longest first'],
                  ['duration-asc', 'Shortest first'],
                ]"
                :key="option[0]"
                :data-sort="option[0]"
                :aria-pressed="sortBy === option[0]"
                class="flex cursor-pointer items-center justify-between rounded-md border-0 bg-transparent px-2 py-1.5 text-left text-xs text-(--muted-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:text-(--text)"
                role="menuitemradio"
                type="button"
                @click="setSort(option[0] as LibrarySortOption)"
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
                v-for="option in [
                  ['none', 'No grouping'],
                  ['artist', 'Artist'],
                  ['album', 'Album'],
                ]"
                :key="option[0]"
                :data-group="option[0]"
                :aria-pressed="groupBy === option[0]"
                class="flex cursor-pointer items-center justify-between rounded-md border-0 bg-transparent px-2 py-1.5 text-left text-xs text-(--muted-text) hover:bg-[oklch(0.72_0.025_258/0.1)] hover:text-(--text) aria-pressed:text-(--text)"
                role="menuitemradio"
                type="button"
                @click="setGroup(option[0] as LibraryGroupOption)"
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

      <div
        v-if="activeCollection === 'tracks' && displayMode === 'list'"
        class="flex min-h-0 flex-col py-2.5 pl-2"
      >
        <div
          v-if="trackFilter"
          class="mx-5 mb-2 flex items-center gap-2 rounded-md bg-[oklch(0.72_0.03_268/0.13)] px-3 py-1.5 text-xs text-(--text)"
          data-library-filter
        >
          <ListFilter class="size-3.5 text-accent" aria-hidden="true" />
          Showing {{ trackFilter.type }}: {{ trackFilter.label }}
          <button
            aria-label="Clear library filter"
            class="ml-auto grid size-5 cursor-pointer place-items-center rounded border-0 bg-transparent text-(--muted-text) hover:text-(--text)"
            type="button"
            @click="clearTrackFilter"
          >
            <X class="size-3.5" aria-hidden="true" />
          </button>
        </div>
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
                @click="selectTrack(track)"
                @keydown.enter.prevent="selectTrack(track)"
                @keydown.space.prevent="selectTrack(track)"
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
                    {{ track.album || "—" }}
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
        v-else-if="activeCollection === 'tracks'"
        class="min-h-0 overflow-auto px-7 py-6"
        aria-label="Tracks grid"
        :style="{ '--grid-item-min-size': `${gridItemSize}px` }"
      >
        <section
          v-for="group in groupedTracks"
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
              :data-current="track.id === currentItem?.id"
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

      <section
        v-else-if="activeCollection === 'albums'"
        class="min-h-0 overflow-auto px-7 py-6"
        aria-label="Albums"
        :style="{ '--grid-item-min-size': `${gridItemSize}px` }"
      >
        <section
          v-for="group in groupedAlbums"
          :key="group.label || 'all-albums'"
          class="library-group"
        >
          <h2
            v-if="group.label"
            class="mb-3 text-xs font-semibold tracking-wide text-(--muted-text) uppercase"
          >
            {{ group.label }}
          </h2>
          <div :class="displayMode === 'grid' ? 'library-grid' : 'grid gap-2'">
            <article
              v-for="album in group.items"
              :key="album.key"
              class="album-tile min-w-0 cursor-pointer rounded-lg p-2 outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
              :class="displayMode === 'list' ? 'flex items-center gap-3' : ''"
              :data-selected="selectedAlbum?.key === album.key"
              tabindex="0"
              @click="selectAlbum(album)"
              @dblclick="openAlbum(album)"
              @keydown.enter.prevent="openAlbum(album)"
            >
              <div
                class="cover-art grid place-items-center rounded-lg [&>svg]:size-[34%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
                :class="
                  displayMode === 'grid'
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
                  :class="displayMode === 'list' ? 'mt-0' : ''"
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

      <section
        v-else
        class="min-h-0 overflow-auto px-7 py-6"
        aria-label="Artists"
        :style="{ '--grid-item-min-size': `${gridItemSize}px` }"
      >
        <section
          v-for="group in groupedArtists"
          :key="group.label || 'all-artists'"
          class="library-group"
        >
          <h2
            v-if="group.label"
            class="mb-3 text-xs font-semibold tracking-wide text-(--muted-text) uppercase"
          >
            {{ group.label }}
          </h2>
          <div :class="displayMode === 'grid' ? 'library-grid' : 'grid gap-2'">
            <article
              v-for="artist in group.items"
              :key="artist.name"
              class="artist-tile min-w-0 cursor-pointer rounded-lg p-2 outline-none hover:bg-[oklch(0.72_0.025_258/0.08)] focus-visible:ring-2 focus-visible:ring-(--focus-ring)"
              :class="displayMode === 'list' ? 'flex items-center gap-3' : ''"
              :data-selected="selectedArtist?.name === artist.name"
              tabindex="0"
              @click="selectArtist(artist)"
              @dblclick="openArtist(artist)"
              @keydown.enter.prevent="openArtist(artist)"
            >
              <div
                class="cover-art grid place-items-center rounded-full [&>svg]:size-6 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
                :class="
                  displayMode === 'grid'
                    ? 'aspect-square w-full'
                    : 'size-14 shrink-0'
                "
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
          </div>
        </section>
      </section>
    </section>

    <aside
      class="library-info-panel col-start-3 row-start-1 min-h-0 overflow-y-auto border-l border-(--line) p-5 max-[1040px]:hidden"
      :data-library-info="selectedInfoKind"
      aria-label="Selected library item details"
    >
      <div v-if="selectedTrack" data-library-info="track">
        <div class="cover-art mb-5 aspect-square w-full rounded-xl">
          <YouTubeArtwork
            class="absolute inset-0 size-full object-cover"
            :video-id="selectedTrack.id"
          />
        </div>
        <p
          class="mb-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
        >
          Track
        </p>
        <h2 class="m-0 text-lg font-semibold text-(--text)">
          {{ selectedTrack.title }}
        </h2>
        <p class="mt-1 text-sm text-(--muted-text)">
          {{ selectedTrack.artist }}
        </p>
        <dl class="mt-6 grid gap-3 border-t border-(--line) pt-4 text-xs">
          <div class="flex items-start justify-between gap-3">
            <dt class="text-(--muted-text)">Album</dt>
            <dd class="m-0 text-right text-(--text)">
              {{ selectedTrack.album || "—" }}
            </dd>
          </div>
          <div class="flex items-start justify-between gap-3">
            <dt class="text-(--muted-text)">Duration</dt>
            <dd class="m-0 text-right text-(--text)">
              {{ formatDuration(selectedTrack.durationMs) }}
            </dd>
          </div>
        </dl>
      </div>

      <div v-else-if="selectedAlbum" data-library-info="album">
        <div class="cover-art mb-5 aspect-square w-full rounded-xl">
          <YouTubeArtwork
            class="absolute inset-0 size-full object-cover"
            :video-id="selectedAlbum.videoId"
          />
        </div>
        <p
          class="mb-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
        >
          Album
        </p>
        <h2 class="m-0 text-lg font-semibold text-(--text)">
          {{ selectedAlbum.title }}
        </h2>
        <p class="mt-1 text-sm text-(--muted-text)">
          {{ selectedAlbum.artist }}
        </p>
        <dl class="mt-6 grid gap-3 border-t border-(--line) pt-4 text-xs">
          <div class="flex items-start justify-between gap-3">
            <dt class="text-(--muted-text)">Tracks</dt>
            <dd class="m-0 text-right text-(--text)">
              {{ selectedAlbum.trackCount }}
            </dd>
          </div>
          <div class="flex items-start justify-between gap-3">
            <dt class="text-(--muted-text)">Duration</dt>
            <dd class="m-0 text-right text-(--text)">
              {{ formatDuration(selectedAlbum.durationMs) }}
            </dd>
          </div>
        </dl>
      </div>

      <div v-else-if="selectedArtist" data-library-info="artist">
        <div
          class="cover-art mb-5 grid aspect-square w-full place-items-center rounded-full [&>svg]:size-20 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
        >
          <Mic2 aria-hidden="true" />
          <YouTubeArtwork
            class="absolute inset-0 size-full object-cover"
            :video-id="selectedArtist.videoId"
          />
        </div>
        <p
          class="mb-1 text-[0.65rem] font-semibold tracking-wide text-(--subtle-text) uppercase"
        >
          Artist
        </p>
        <h2 class="m-0 text-lg font-semibold text-(--text)">
          {{ selectedArtist.name }}
        </h2>
        <dl class="mt-6 grid gap-3 border-t border-(--line) pt-4 text-xs">
          <div class="flex items-start justify-between gap-3">
            <dt class="text-(--muted-text)">Albums</dt>
            <dd class="m-0 text-right text-(--text)">
              {{ selectedArtist.albumCount }}
            </dd>
          </div>
          <div class="flex items-start justify-between gap-3">
            <dt class="text-(--muted-text)">Tracks</dt>
            <dd class="m-0 text-right text-(--text)">
              {{ selectedArtist.trackCount }}
            </dd>
          </div>
          <div class="flex items-start justify-between gap-3">
            <dt class="text-(--muted-text)">Duration</dt>
            <dd class="m-0 text-right text-(--text)">
              {{ formatDuration(selectedArtist.durationMs) }}
            </dd>
          </div>
        </dl>
      </div>

      <div
        v-else
        class="grid min-h-full place-items-center text-center text-sm text-(--muted-text)"
      >
        Select a track, album, or artist to see its metadata.
      </div>
    </aside>

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
            {{ currentItem?.title ?? "Nothing selected" }}
          </p>
          <span
            class="mt-0.5 block overflow-hidden text-[0.74rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
          >
            {{ currentItem?.artist ?? "Choose a track" }}
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
