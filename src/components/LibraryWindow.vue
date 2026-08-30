<script setup lang="ts">
import { computed, ref } from "vue";

import type {
  MediaItem,
  MetadataRefreshSnapshot,
  PlaybackSnapshot,
} from "@/api";
import MetadataRefreshDrawer from "./MetadataRefreshDrawer.vue";
import LibraryAlbumGrid from "./library/LibraryAlbumGrid.vue";
import LibraryArtistGrid from "./library/LibraryArtistGrid.vue";
import LibraryHeader from "./library/LibraryHeader.vue";
import LibraryInfoPanel from "./library/LibraryInfoPanel.vue";
import LibraryPlaybackFooter from "./library/LibraryPlaybackFooter.vue";
import LibrarySidebar from "./library/LibrarySidebar.vue";
import LibraryTrackGrid from "./library/LibraryTrackGrid.vue";
import LibraryTrackList from "./library/LibraryTrackList.vue";
import type {
  AlbumGroup,
  ArtistGroup,
  LibraryAlbum,
  LibraryArtist,
  LibraryCollection,
  LibraryDisplayMode,
  LibraryGroupOption,
  LibrarySortOption,
  TrackFilter,
  TrackGroup,
} from "./library/types";

type SelectedLibraryItem =
  | { id: string; kind: "track" }
  | { key: string; kind: "album" }
  | { kind: "artist"; name: string };

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

const activeCollection = ref<LibraryCollection>("tracks");
const displayMode = ref<LibraryDisplayMode>("list");
const groupBy = ref<LibraryGroupOption>("none");
const gridItemSize = ref(176);
const libraryOptionsOpen = ref(false);
const metadataRefreshDrawerOpen = ref(false);
const selectedLibraryItem = ref<SelectedLibraryItem | null>(
  props.snapshot.currentItem
    ? { id: props.snapshot.currentItem.id, kind: "track" }
    : null,
);
const sortBy = ref<LibrarySortOption>("title-asc");
const trackFilter = ref<TrackFilter | null>(null);

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

  return sortCollection(filteredTracks, sortBy.value, "track");
});
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
  if (!props.isUpdating) {
    emit("playTrack", track.id);
  }
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

function setSort(option: LibrarySortOption): void {
  sortBy.value = option;
  libraryOptionsOpen.value = false;
}

function setGroup(option: LibraryGroupOption): void {
  groupBy.value = option;
  libraryOptionsOpen.value = false;
}

function setGridItemSize(size: number): void {
  gridItemSize.value = size;
}

function toggleOptions(): void {
  libraryOptionsOpen.value = !libraryOptionsOpen.value;
}

function toggleMetadataRefresh(): void {
  metadataRefreshDrawerOpen.value = !metadataRefreshDrawerOpen.value;
}
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

    <LibrarySidebar
      :active-collection="activeCollection"
      @open-import="emit('openImport')"
      @select-collection="selectCollection"
    />

    <section
      class="library-content col-start-2 row-start-1 grid min-h-0 min-w-0 grid-rows-[104px_minmax(0,1fr)] border-l border-(--line) max-[760px]:col-start-1"
    >
      <LibraryHeader
        :collection-title="collectionTitle"
        :collection-summary="collectionSummary"
        :display-mode="displayMode"
        :error-message="errorMessage"
        :grid-item-size="gridItemSize"
        :group-by="groupBy"
        :has-active-metadata-refresh="hasActiveMetadataRefresh"
        :library-options-open="libraryOptionsOpen"
        :metadata-refresh-drawer-open="metadataRefreshDrawerOpen"
        :metadata-refresh-remaining="metadataRefreshRemaining"
        :sort-by="sortBy"
        @set-display-mode="setDisplayMode"
        @set-grid-item-size="setGridItemSize"
        @set-group="setGroup"
        @set-sort="setSort"
        @toggle-metadata-refresh="toggleMetadataRefresh"
        @toggle-options="toggleOptions"
      />

      <LibraryTrackList
        v-if="activeCollection === 'tracks' && displayMode === 'list'"
        :current-item-id="currentItem?.id"
        :track-filter="trackFilter"
        :tracks="libraryTracks"
        @clear-track-filter="trackFilter = null"
        @select-track="selectTrack"
      />
      <LibraryTrackGrid
        v-else-if="activeCollection === 'tracks'"
        :current-item-id="currentItem?.id"
        :grid-item-size="gridItemSize"
        :groups="groupedTracks"
        @select-track="selectTrack"
      />
      <LibraryAlbumGrid
        v-else-if="activeCollection === 'albums'"
        :display-mode="displayMode"
        :grid-item-size="gridItemSize"
        :groups="groupedAlbums"
        :selected-album-key="selectedAlbum?.key"
        @open-album="openAlbum"
        @select-album="selectAlbum"
      />
      <LibraryArtistGrid
        v-else
        :display-mode="displayMode"
        :grid-item-size="gridItemSize"
        :groups="groupedArtists"
        :selected-artist-name="selectedArtist?.name"
        @open-artist="openArtist"
        @select-artist="selectArtist"
      />
    </section>

    <LibraryInfoPanel
      :selected-album="selectedAlbum"
      :selected-artist="selectedArtist"
      :selected-track="selectedTrack"
    />

    <LibraryPlaybackFooter
      :current-item="currentItem"
      :is-playing="isPlaying"
      :is-updating="isUpdating"
      :snapshot="snapshot"
      @next="emit('next')"
      @previous="emit('previous')"
      @seek="emit('seek', $event)"
      @set-volume="emit('setVolume', $event)"
      @toggle="emit('toggle')"
    />

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
/* Library-specific presentation lives with the extracted view components. */
</style>
