<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";

import type {
  MediaItem,
  MetadataRefreshSnapshot,
  PlaybackSnapshot,
  PlaybackTransport,
  Playlist,
  TrackMetadataUpdate,
} from "@/api";
import MetadataRefreshDrawer from "./MetadataRefreshDrawer.vue";
import LibraryAlbumGrid from "./library/LibraryAlbumGrid.vue";
import LibraryArtistGrid from "./library/LibraryArtistGrid.vue";
import LibraryHeader from "./library/LibraryHeader.vue";
import LibraryInfoPanel from "./library/LibraryInfoPanel.vue";
import LibraryMetadataEditor, {
  type MetadataEditTarget,
} from "./library/LibraryMetadataEditor.vue";
import LibraryPlaybackFooter from "./library/LibraryPlaybackFooter.vue";
import LibrarySidebar from "./library/LibrarySidebar.vue";
import LibraryTrackGrid from "./library/LibraryTrackGrid.vue";
import LibraryTrackList from "./library/LibraryTrackList.vue";
import PlaylistEditor from "./library/PlaylistEditor.vue";
import { buildLibraryArtists, groupItems } from "./library/collections";
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
  TrackSelectionModifiers,
} from "./library/types";
import {
  LIBRARY_TRACK_IDS_MIME_TYPE,
  LIBRARY_TRACK_IDS_TEXT_PREFIX,
} from "./library/types";

type SelectedLibraryItem =
  | { id: string; kind: "track" }
  | { key: string; kind: "album" }
  | { kind: "artist"; name: string };

interface Props {
  snapshot?: PlaybackSnapshot;
  playlists?: Playlist[];
  tracks?: MediaItem[];
  transport?: PlaybackTransport;
  isStarting?: boolean;
  isUpdating: boolean;
  errorMessage?: string;
  metadataRefreshes?: MetadataRefreshSnapshot;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  removeTracks: [ids: string[]];
  toggle: [];
  previous: [];
  next: [];
  seek: [positionMs: number];
  setVolume: [percent: number];
  toggleMute: [];
  toggleShuffle: [];
  cycleRepeatMode: [];
  openImport: [];
  playTrack: [queueIds: string[], id: string];
  playNext: [ids: string[]];
  addToQueue: [ids: string[]];
  toggleFavorite: [ids: string[]];
  upsertPlaylist: [playlist: Playlist];
  reorderPlaylists: [playlistIds: string[]];
  deletePlaylist: [id: string];
  updateTracksMetadata: [updates: TrackMetadataUpdate[]];
}>();

const activeCollection = ref<LibraryCollection>("tracks");
const displayMode = ref<LibraryDisplayMode>("list");
const groupBy = ref<LibraryGroupOption>("none");
const gridItemSize = ref(176);
const libraryOptionsOpen = ref(false);
const metadataRefreshDrawerOpen = ref(false);
const metadataEditorTarget = ref<MetadataEditTarget | null>(null);
const playlistEditorTarget = ref<Playlist | null>(null);
const trackRemovalTarget = ref<MediaItem[]>([]);
const isCreatingPlaylist = ref(false);
const detailsSidebarOpen = ref(false);
const activePlaylistId = ref<string>();
const sidebarWidth = ref(244);
const playback = computed<PlaybackTransport>(
  () =>
    props.transport ??
    props.snapshot ?? {
      currentItem: null,
      positionMs: 0,
      status: "paused",
      volumePercent: 0,
    },
);
const selectedLibraryItem = ref<SelectedLibraryItem | null>(
  playback.value.currentItem
    ? { id: playback.value.currentItem.id, kind: "track" }
    : null,
);
const selectedTrackIds = ref<Set<string>>(
  playback.value.currentItem
    ? new Set([playback.value.currentItem.id])
    : new Set(),
);
const trackSelectionAnchorId = ref<string | null>(
  playback.value.currentItem?.id ?? null,
);
let trackDragImage: HTMLElement | null = null;
const sortBy = ref<LibrarySortOption>("title-asc");
const trackFilter = ref<TrackFilter | null>(null);

const isPlaying = computed(() => playback.value.status === "playing");
const currentItem = computed(() => playback.value.currentItem);
const playingItemId = computed(() =>
  isPlaying.value ? currentItem.value?.id : undefined,
);
const allTracks = computed(() => props.tracks ?? props.snapshot?.queue ?? []);
const playlists = computed(() => props.playlists ?? []);
const activePlaylist = computed(
  () =>
    playlists.value.find(
      (playlist) => playlist.id === activePlaylistId.value,
    ) ?? null,
);
const canRemoveFromPlaylist = computed(
  () =>
    activePlaylist.value !== null &&
    activePlaylist.value.id !== "favorites" &&
    activePlaylist.value.id !== "most-played",
);
const playlistTracks = computed(() => {
  const playlist = activePlaylist.value;
  if (!playlist) {
    return allTracks.value;
  }

  const tracksById = new Map(allTracks.value.map((track) => [track.id, track]));
  return playlist.trackIds.flatMap((id) => {
    const track = tracksById.get(id);
    return track ? [track] : [];
  });
});
const selectedTrack = computed(() => {
  if (selectedTrackIds.value.size !== 1) {
    return null;
  }

  const [id] = selectedTrackIds.value;
  return allTracks.value.find((track) => track.id === id) ?? null;
});
const libraryTracks = computed(() => {
  const filteredTracks = trackFilter.value
    ? playlistTracks.value.filter((track) => {
        if (trackFilter.value?.type === "artist") {
          return track.artist === trackFilter.value.value;
        }

        return (
          `${track.artist}\u0000${track.album ?? ""}` ===
          trackFilter.value?.value
        );
      })
    : playlistTracks.value;

  return sortCollection(filteredTracks, sortBy.value, "track");
});
const selectedTracks = computed(() =>
  libraryTracks.value.filter((track) => selectedTrackIds.value.has(track.id)),
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
  return sortCollection(
    buildLibraryArtists(allTracks.value),
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
    return groupSortSections(libraryTracks.value, sortBy.value, "track");
  }

  return groupItems(libraryTracks.value, (track) =>
    groupBy.value === "artist"
      ? track.artist
      : track.album?.trim() || "Unknown album",
  );
});
const groupedAlbums = computed<AlbumGroup[]>(() =>
  groupGridCollection(libraryAlbums.value, "album", (album) =>
    groupBy.value === "artist" ? album.artist : album.title,
  ),
);
const groupedArtists = computed<ArtistGroup[]>(() =>
  groupGridCollection(libraryArtists.value, "artist", (artist) =>
    groupBy.value === "album" ? "Artists" : artist.name,
  ),
);
const collectionTitle = computed(() => {
  if (activePlaylist.value) {
    return activePlaylist.value.name;
  }
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
    const artist = item as LibraryArtist;
    if (option.startsWith("album-count")) return artist.albumCount;
    if (option.startsWith("track-count")) return artist.trackCount;
    if (option.startsWith("duration")) return artist.durationMs;
    return artist.name;
  }
  if (kind === "album") {
    const album = item as LibraryAlbum;
    return option.startsWith("artist")
      ? album.artist
      : option.startsWith("track-count")
        ? album.trackCount
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

function alphabeticalSection(value: string): string {
  const firstCharacter = Array.from(
    value
      .trim()
      .normalize("NFD")
      .replace(/\p{Diacritic}/gu, ""),
  )[0]?.toLocaleUpperCase();

  return firstCharacter && /[A-Z]/.test(firstCharacter) ? firstCharacter : "#";
}

function durationSection(durationMs: number): string {
  if (durationMs < 60_000) return "Under 1 min";
  if (durationMs < 300_000) return "1–4 min";
  if (durationMs < 600_000) return "5–9 min";
  if (durationMs < 1_800_000) return "10–29 min";
  if (durationMs < 3_600_000) return "30–59 min";
  return "1 hr or more";
}

function sortSectionLabel(
  item: MediaItem | LibraryAlbum | LibraryArtist,
  option: LibrarySortOption,
  kind: "album" | "artist" | "track",
): string {
  const value = sortValue(item, option, kind);

  if (option.startsWith("duration")) {
    return durationSection(Number(value));
  }
  if (kind === "album" && option.startsWith("track-count")) {
    return `${value} ${value === 1 ? "track" : "tracks"}`;
  }
  if (kind === "artist" && option.startsWith("album-count")) {
    return `${value} ${value === 1 ? "album" : "albums"}`;
  }
  if (kind === "artist" && option.startsWith("track-count")) {
    return `${value} ${value === 1 ? "track" : "tracks"}`;
  }

  return alphabeticalSection(String(value));
}

function groupSortSections<T extends MediaItem | LibraryAlbum | LibraryArtist>(
  items: T[],
  option: LibrarySortOption,
  kind: "album" | "artist" | "track",
): Array<{ items: T[]; label: string }> {
  return groupItems(items, (item) => sortSectionLabel(item, option, kind));
}

function groupGridCollection<T extends LibraryAlbum | LibraryArtist>(
  items: T[],
  kind: "album" | "artist",
  getLabel: (item: T) => string,
): Array<{ items: T[]; label: string }> {
  if (groupBy.value === "none") {
    return groupSortSections(items, sortBy.value, kind);
  }

  return groupItems(items, getLabel);
}

function selectCollection(collection: LibraryCollection): void {
  activePlaylistId.value = undefined;
  activeCollection.value = collection;
  if (collection !== "tracks") {
    clearTrackSelection();
  }
}

function selectPlaylist(id: string): void {
  activePlaylistId.value = id;
  activeCollection.value = "tracks";
  displayMode.value = "list";
  trackFilter.value = null;
  clearTrackSelection();
}

function openPlaylistEditor(playlist: Playlist): void {
  playlistEditorTarget.value = playlist;
}

function beginPlaylistCreation(): void {
  isCreatingPlaylist.value = true;
}

function createPlaylist(name: string): void {
  const playlist: Playlist = {
    id: `playlist-${crypto.randomUUID()}`,
    name,
    trackIds: [],
  };
  activePlaylistId.value = playlist.id;
  isCreatingPlaylist.value = false;
  emit("upsertPlaylist", playlist);
}

function cancelPlaylistCreation(): void {
  isCreatingPlaylist.value = false;
}

function savePlaylist(playlist: Playlist): void {
  playlistEditorTarget.value = null;
  emit("upsertPlaylist", playlist);
}

function addTracksToPlaylist(playlist: Playlist, trackIds: string[]): void {
  if (
    props.isUpdating ||
    playlist.id === "favorites" ||
    playlist.id === "most-played"
  ) {
    return;
  }

  const ids = new Set(playlist.trackIds);
  for (const id of trackIds) {
    ids.add(id);
  }
  emit("upsertPlaylist", {
    ...playlist,
    trackIds: [...ids],
  });
}

function removeTracksFromPlaylist(tracks: MediaItem[]): void {
  const playlist = activePlaylist.value;
  if (props.isUpdating || !playlist || !canRemoveFromPlaylist.value) {
    return;
  }

  const removedIds = new Set(tracks.map((track) => track.id));
  const trackIds = playlist.trackIds.filter((id) => !removedIds.has(id));
  if (trackIds.length === playlist.trackIds.length) {
    return;
  }

  emit("upsertPlaylist", { ...playlist, trackIds });
  clearTrackSelection();
}

function deletePlaylist(id: string): void {
  if (activePlaylistId.value === id) {
    activePlaylistId.value = undefined;
  }
  playlistEditorTarget.value = null;
  emit("deletePlaylist", id);
}

function setDisplayMode(mode: LibraryDisplayMode): void {
  displayMode.value = mode;
}

function clearTrackSelection(): void {
  selectedTrackIds.value = new Set();
  trackSelectionAnchorId.value = null;
}

function selectTrack(
  track: MediaItem,
  modifiers: TrackSelectionModifiers = { additive: false, range: false },
): void {
  const next = new Set(selectedTrackIds.value);
  const anchorId = trackSelectionAnchorId.value;
  const anchorIndex = anchorId
    ? libraryTracks.value.findIndex((item) => item.id === anchorId)
    : -1;
  const trackIndex = libraryTracks.value.findIndex(
    (item) => item.id === track.id,
  );

  if (modifiers.range && anchorIndex !== -1 && trackIndex !== -1) {
    const start = Math.min(anchorIndex, trackIndex);
    const end = Math.max(anchorIndex, trackIndex);
    const range = libraryTracks.value
      .slice(start, end + 1)
      .map((item) => item.id);
    selectedTrackIds.value = modifiers.additive
      ? new Set([...next, ...range])
      : new Set(range);
  } else if (modifiers.additive) {
    if (next.has(track.id)) {
      next.delete(track.id);
    } else {
      next.add(track.id);
    }
    selectedTrackIds.value = next;
    trackSelectionAnchorId.value = track.id;
  } else {
    selectedTrackIds.value = new Set([track.id]);
    trackSelectionAnchorId.value = track.id;
  }

  selectedLibraryItem.value = selectedTrackIds.value.size
    ? { id: track.id, kind: "track" }
    : null;
}

function selectContextMenuTarget(track: MediaItem): void {
  if (!selectedTrackIds.value.has(track.id)) {
    selectTrack(track);
  }
}

function startTrackDrag(track: MediaItem, event: DragEvent): void {
  if (!selectedTrackIds.value.has(track.id)) {
    selectTrack(track);
  }

  const trackIds = selectedTracks.value.map((item) => item.id);
  if (!event.dataTransfer || trackIds.length === 0) {
    return;
  }

  event.dataTransfer.effectAllowed = "copy";
  event.dataTransfer.setData(
    LIBRARY_TRACK_IDS_MIME_TYPE,
    JSON.stringify(trackIds),
  );
  event.dataTransfer.setData(
    "text/plain",
    [LIBRARY_TRACK_IDS_TEXT_PREFIX, ...trackIds].join("\n"),
  );

  const source = event.target;
  if (
    !(source instanceof HTMLElement) ||
    typeof event.dataTransfer.setDragImage !== "function"
  ) {
    return;
  }

  finishTrackDrag();
  const sourceBounds = source.getBoundingClientRect();
  const dragImage = source.cloneNode(true) as HTMLElement;
  dragImage.removeAttribute("data-track-id");
  dragImage.setAttribute("aria-hidden", "true");
  Object.assign(dragImage.style, {
    height: `${sourceBounds.height}px`,
    left: `${sourceBounds.left}px`,
    margin: "0",
    pointerEvents: "none",
    position: "fixed",
    top: `${sourceBounds.top}px`,
    transform: "none",
    width: `${sourceBounds.width}px`,
    zIndex: "2147483647",
  });
  document.body.append(dragImage);
  event.dataTransfer.setDragImage(
    dragImage,
    Math.max(0, event.clientX - sourceBounds.left),
    Math.max(0, event.clientY - sourceBounds.top),
  );
  trackDragImage = dragImage;
}

function finishTrackDrag(): void {
  trackDragImage?.remove();
  trackDragImage = null;
}

function playTracks(tracks: MediaItem[]): void {
  const track = tracks[0];
  if (!track || props.isUpdating) {
    return;
  }

  if (!selectedTrackIds.value.has(track.id)) {
    selectTrack(track);
  }
  emit(
    "playTrack",
    tracks.map((item) => item.id),
    track.id,
  );
}

function playTracksNext(tracks: MediaItem[]): void {
  if (props.isUpdating) {
    return;
  }

  // Emit all IDs in a single batch event (reversed so first resolves first in queue).
  const ids = [...tracks].reverse().map((t) => t.id);
  if (ids.length > 0) {
    emit("playNext", ids);
  }
}

function addTracksToQueue(tracks: MediaItem[]): void {
  if (props.isUpdating) {
    return;
  }

  const ids = tracks.map((t) => t.id);
  if (ids.length > 0) {
    emit("addToQueue", ids);
  }
}

function toggleFavorites(ids: string[]): void {
  const deduplicated = [...new Set(ids)];
  if (deduplicated.length > 0) {
    emit("toggleFavorite", deduplicated);
  }
}

function playAlbum(album: LibraryAlbum): void {
  playTracks(
    allTracks.value.filter(
      (track) =>
        track.artist === album.artist && track.album?.trim() === album.title,
    ),
  );
}

function playArtist(artist: LibraryArtist): void {
  playTracks(allTracks.value.filter((track) => track.artist === artist.name));
}

function playPlaylist(playlist: Playlist): void {
  if (playlist.id === activePlaylist.value?.id) {
    playTracks(libraryTracks.value);
  }
}

function playActivePlaylist(): void {
  if (activePlaylist.value) {
    playPlaylist(activePlaylist.value);
  }
}

function selectAlbum(album: LibraryAlbum): void {
  clearTrackSelection();
  selectedLibraryItem.value = { key: album.key, kind: "album" };
}

function selectArtist(artist: LibraryArtist): void {
  clearTrackSelection();
  selectedLibraryItem.value = { kind: "artist", name: artist.name };
}

function openTrackMetadataEditor(track: MediaItem): void {
  metadataEditorTarget.value = {
    kind: "track",
    name: track.title,
    tracks: [track],
  };
}

function openTrackAlbum(track: MediaItem): void {
  const album = track.album
    ? libraryAlbums.value.find(
        (candidate) => candidate.key === `${track.artist}\u0000${track.album}`,
      )
    : undefined;
  if (album) {
    openAlbum(album);
  }
}

function openTrackArtist(track: MediaItem): void {
  const artist = libraryArtists.value.find(
    (candidate) => candidate.name === track.artist,
  );
  if (artist) {
    openArtist(artist);
  }
}

function requestTrackRemoval(tracks: MediaItem[]): void {
  trackRemovalTarget.value = tracks;
}

function confirmTrackRemoval(): void {
  if (trackRemovalTarget.value.length === 0) {
    return;
  }

  const ids = trackRemovalTarget.value.map((track) => track.id);
  trackRemovalTarget.value = [];
  emit("removeTracks", ids);
}

function openAlbumMetadataEditor(album: LibraryAlbum): void {
  metadataEditorTarget.value = {
    kind: "album",
    name: album.title,
    tracks: allTracks.value.filter(
      (track) =>
        track.artist === album.artist && track.album?.trim() === album.title,
    ),
  };
}

function openArtistMetadataEditor(artist: LibraryArtist): void {
  metadataEditorTarget.value = {
    kind: "artist",
    name: artist.name,
    tracks: allTracks.value.filter((track) => track.artist === artist.name),
  };
}

function saveMetadata(updates: TrackMetadataUpdate[]): void {
  metadataEditorTarget.value = null;
  emit("updateTracksMetadata", updates);
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

function resizeSidebar(width: number): void {
  sidebarWidth.value = Math.min(360, Math.max(180, Math.round(width)));
}

onBeforeUnmount(finishTrackDrag);
</script>

<template>
  <main
    class="library-window window-shell window-surface grid h-screen min-h-0 grid-rows-[minmax(0,1fr)_64px] transition-[grid-template-columns] duration-200 ease-out motion-reduce:transition-none max-[1040px]:grid-cols-[var(--library-sidebar-width)_minmax(0,1fr)] max-[920px]:grid-rows-[minmax(0,1fr)_104px] max-[760px]:grid-cols-1"
    :class="
      detailsSidebarOpen
        ? 'grid-cols-[var(--library-sidebar-width)_minmax(0,1fr)_272px]'
        : 'grid-cols-[var(--library-sidebar-width)_minmax(0,1fr)_0px]'
    "
    :style="{ '--library-sidebar-width': `${sidebarWidth}px` }"
    aria-label="Music library"
  >
    <div
      class="application-drag-region window-drag-region"
      data-tauri-drag-region
      aria-hidden="true"
    />

    <LibrarySidebar
      :active-collection="activeCollection"
      :active-playlist-id="activePlaylistId"
      :is-creating-playlist="isCreatingPlaylist"
      :is-updating="props.isUpdating"
      :playlists="playlists"
      :sidebar-width="sidebarWidth"
      @cancel-playlist-creation="cancelPlaylistCreation"
      @create-playlist="createPlaylist"
      @delete-playlist="openPlaylistEditor"
      @drop-tracks="addTracksToPlaylist"
      @edit-playlist="openPlaylistEditor"
      @new-playlist="beginPlaylistCreation"
      @open-import="emit('openImport')"
      @play-playlist="playPlaylist"
      @reorder-playlists="emit('reorderPlaylists', $event)"
      @select-collection="selectCollection"
      @select-playlist="selectPlaylist"
      @resize-sidebar="resizeSidebar"
    />

    <section
      class="library-content col-start-2 row-start-1 grid min-h-0 min-w-0 grid-rows-[88px_minmax(0,1fr)] border-l border-(--line) max-[760px]:col-start-1"
    >
      <LibraryHeader
        :collection-title="collectionTitle"
        :collection-summary="collectionSummary"
        :playlist-name="activePlaylist?.name"
        :can-play-playlist="playlistTracks.length > 0"
        :is-updating="props.isUpdating"
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
        @play-playlist="playActivePlaylist"
      />

      <LibraryTrackList
        v-if="activeCollection === 'tracks' && displayMode === 'list'"
        :playing-item-id="playingItemId"
        :selected-track-ids="[...selectedTrackIds]"
        :sort-by="sortBy"
        :track-filter="trackFilter"
        :tracks="libraryTracks"
        :favorite-track-ids="
          playlists.find((playlist) => playlist.id === 'favorites')?.trackIds ??
          []
        "
        @clear-track-filter="trackFilter = null"
        @add-to-queue="addTracksToQueue"
        @drag-tracks="startTrackDrag"
        @drag-tracks-end="finishTrackDrag"
        @edit-track="openTrackMetadataEditor"
        @open-album="openTrackAlbum"
        @open-artist="openTrackArtist"
        @open-track-context="selectContextMenuTarget"
        @play-track="playTracks"
        @play-next="playTracksNext"
        :can-remove-from-playlist="canRemoveFromPlaylist"
        :is-updating="props.isUpdating"
        @remove-from-playlist="removeTracksFromPlaylist"
        @remove-track="requestTrackRemoval"
        @select-track="selectTrack"
        @set-sort="setSort"
        @toggle-favorite="toggleFavorites"
      />
      <LibraryTrackGrid
        v-else-if="activeCollection === 'tracks'"
        :playing-item-id="playingItemId"
        :selected-track-ids="[...selectedTrackIds]"
        :favorite-track-ids="
          playlists.find((playlist) => playlist.id === 'favorites')?.trackIds ??
          []
        "
        :grid-item-size="gridItemSize"
        :groups="groupedTracks"
        @add-to-queue="addTracksToQueue"
        @drag-tracks="startTrackDrag"
        @drag-tracks-end="finishTrackDrag"
        @edit-track="openTrackMetadataEditor"
        @open-album="openTrackAlbum"
        @open-artist="openTrackArtist"
        @open-track-context="selectContextMenuTarget"
        @play-next="playTracksNext"
        @play-track="playTracks"
        :can-remove-from-playlist="canRemoveFromPlaylist"
        :is-updating="props.isUpdating"
        @remove-from-playlist="removeTracksFromPlaylist"
        @remove-track="requestTrackRemoval"
        @select-track="selectTrack"
        @toggle-favorite="toggleFavorites"
      />
      <LibraryAlbumGrid
        v-else-if="activeCollection === 'albums'"
        :display-mode="displayMode"
        :grid-item-size="gridItemSize"
        :groups="groupedAlbums"
        :selected-album-key="selectedAlbum?.key"
        @edit-album="openAlbumMetadataEditor"
        @open-album="openAlbum"
        @play-album="playAlbum"
        @select-album="selectAlbum"
        @set-sort="setSort"
        :sort-by="sortBy"
      />
      <LibraryArtistGrid
        v-else
        :display-mode="displayMode"
        :grid-item-size="gridItemSize"
        :groups="groupedArtists"
        :selected-artist-name="selectedArtist?.name"
        @edit-artist="openArtistMetadataEditor"
        @open-artist="openArtist"
        @play-artist="playArtist"
        @select-artist="selectArtist"
        @set-sort="setSort"
        :sort-by="sortBy"
      />
    </section>

    <LibraryInfoPanel
      :is-open="detailsSidebarOpen"
      :selected-album="selectedAlbum"
      :selected-artist="selectedArtist"
      :selected-track="selectedTrack"
      :selected-tracks="selectedTracks"
      @add-to-queue="addTracksToQueue"
      @edit-album="openAlbumMetadataEditor"
      @edit-artist="openArtistMetadataEditor"
      @edit-track="openTrackMetadataEditor"
      @play-next="playTracksNext"
    />

    <LibraryPlaybackFooter
      :current-item="currentItem"
      :is-playing="isPlaying"
      :is-starting="isStarting ?? false"
      :is-updating="isUpdating"
      :playback="playback"
      :details-open="detailsSidebarOpen"
      @next="emit('next')"
      @previous="emit('previous')"
      @seek="emit('seek', $event)"
      @set-volume="emit('setVolume', $event)"
      @toggle-shuffle="emit('toggleShuffle')"
      @toggle-mute="emit('toggleMute')"
      @toggle="emit('toggle')"
      @cycle-repeat-mode="emit('cycleRepeatMode')"
      @toggle-details="detailsSidebarOpen = !detailsSidebarOpen"
    />

    <MetadataRefreshDrawer
      v-if="
        metadataRefreshDrawerOpen &&
        hasActiveMetadataRefresh &&
        metadataRefreshes
      "
      :refreshes="metadataRefreshes"
    />

    <LibraryMetadataEditor
      v-if="metadataEditorTarget"
      :target="metadataEditorTarget"
      @cancel="metadataEditorTarget = null"
      @save="saveMetadata"
    />
    <PlaylistEditor
      v-if="playlistEditorTarget"
      :playlist="playlistEditorTarget"
      :tracks="allTracks"
      @cancel="playlistEditorTarget = null"
      @delete="deletePlaylist"
      @save="savePlaylist"
    />
    <section
      v-if="trackRemovalTarget.length"
      aria-labelledby="track-removal-title"
      aria-modal="true"
      class="absolute inset-0 z-50 grid place-items-center bg-black/60 p-5 backdrop-blur-sm"
      role="dialog"
      @click.self="trackRemovalTarget = []"
      @keydown.esc="trackRemovalTarget = []"
    >
      <div
        class="w-full max-w-100 rounded-2xl border border-(--line-strong) bg-[oklch(0.11_0.014_260/0.98)] p-6 shadow-2xl"
      >
        <h2
          id="track-removal-title"
          class="text-lg font-semibold text-(--text)"
        >
          Remove from library?
        </h2>
        <p class="mt-2 text-sm text-(--muted-text)">
          <template v-if="trackRemovalTarget.length === 1">
            Remove {{ trackRemovalTarget[0]?.title }} from your library and
            every playlist?
          </template>
          <template v-else>
            Remove {{ trackRemovalTarget.length }} tracks from your library and
            every playlist?
          </template>
        </p>
        <div class="mt-6 flex justify-end gap-3">
          <button
            class="rounded-md px-3 py-2 text-sm font-medium text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
            type="button"
            @click="trackRemovalTarget = []"
          >
            Cancel
          </button>
          <button
            class="rounded-md bg-red-500/15 px-3 py-2 text-sm font-medium text-red-300 hover:bg-red-500/25 focus-visible:ring-2 focus-visible:ring-(--focus-ring) focus-visible:outline-none"
            data-confirm-track-removal
            type="button"
            @click="confirmTrackRemoval"
          >
            Remove {{ trackRemovalTarget.length === 1 ? "track" : "tracks" }}
          </button>
        </div>
      </div>
    </section>
  </main>
</template>

<style scoped>
/* Library-specific presentation lives with the extracted view components. */
</style>
