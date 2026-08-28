<script setup lang="ts">
import {
  AudioLines,
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
  Mic2,
  Pause,
  Play,
  Plus,
  Repeat2,
  Search,
  Shuffle,
  SkipBack,
  SkipForward,
  Sparkles,
  Volume2,
} from "lucide-vue-next";
import { computed, ref } from "vue";

import type { PlaybackSnapshot } from "@/api";
import { Button } from "@/components/ui/button";
import { mockAlbums, mockArtists, mockTracks } from "@/lib/mock-library";
import { formatDuration } from "@/lib/time";

type LibraryCollection = "tracks" | "albums" | "artists";

interface Props {
  snapshot: PlaybackSnapshot;
  isUpdating: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  toggle: [];
  previous: [];
  next: [];
  setVolume: [percent: number];
}>();

const playlists = [
  { href: "#favorites", icon: Heart, label: "Favorites" },
  { href: "#chill-vibes", icon: Sparkles, label: "Chill Vibes" },
  { href: "#focus", icon: CircleDot, label: "Focus" },
  { href: "#road-trip", icon: CarFront, label: "Road Trip" },
  { href: "#90s-mix", icon: CassetteTape, label: "90s Mix" },
] as const;

const activeCollection = ref<LibraryCollection>("tracks");
const favoriteTrackIds = ref(
  new Set(["night-drive", "first-light", "granite"]),
);
const isPlaying = computed(() => props.snapshot.status === "playing");
const currentItem = computed(() => props.snapshot.currentItem);
const currentCover = computed(
  () =>
    mockTracks.find((track) => track.id === currentItem.value?.id)?.cover ??
    "violet",
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
    albums: `${mockAlbums.length} albums`,
    artists: `${mockArtists.length} artists`,
    tracks: `${mockTracks.length} songs · 30 min`,
  };

  return summaries[activeCollection.value];
});

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

function inputValue(event: Event): number {
  return Number((event.target as HTMLInputElement).value);
}
</script>

<template>
  <main
    class="library-window relative grid h-screen min-h-0 grid-cols-[244px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_104px] overflow-hidden bg-[radial-gradient(circle_at_14%_4%,oklch(0.48_0.09_274/0.22),transparent_34%),radial-gradient(circle_at_82%_12%,oklch(0.34_0.045_252/0.15),transparent_42%),linear-gradient(135deg,oklch(0.28_0.055_248/0.2),transparent_58%),var(--glass-window)] text-(--text) backdrop-saturate-[1.2] max-[760px]:grid-cols-1"
    aria-label="Music library"
  >
    <div
      class="application-drag-region absolute top-0 right-44 left-0 z-10 h-13"
      data-tauri-drag-region
      aria-hidden="true"
    />

    <aside
      class="col-start-1 row-start-1 flex min-h-0 flex-col border-r border-(--line) px-6 pt-14 pb-4 max-[760px]:hidden"
    >
      <label
        class="flex min-h-9.5 items-center gap-2.5 rounded-lg bg-(--glass-control) px-3 text-(--subtle-text) ring-1 ring-[oklch(0.75_0.025_260/0.07)] transition-colors focus-within:ring-(--focus-ring) [&>svg]:size-4"
      >
        <Search aria-hidden="true" />
        <span class="sr-only">Search library</span>
        <input
          class="min-w-0 flex-1 border-0 bg-transparent text-[0.8rem] text-(--text) outline-0 placeholder:text-(--subtle-text)"
          placeholder="Search"
          type="search"
        />
      </label>

      <nav class="mt-6 grid gap-0.5" aria-label="Library navigation">
        <p
          class="mb-1 px-2.5 text-[0.61rem] font-semibold tracking-[0.06em] text-(--subtle-text)"
        >
          Library
        </p>
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
          ><ListMusic aria-hidden="true" />Playlists</span
        >
      </nav>

      <nav class="mt-5 grid gap-0.5" aria-label="Playlists">
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
      class="library-content col-start-2 row-start-1 grid min-h-0 min-w-0 grid-rows-[104px_minmax(0,1fr)] max-[760px]:col-start-1"
    >
      <header
        class="flex items-center justify-between gap-6 border-b border-(--line) px-8 pt-3"
      >
        <div>
          <h1 class="text-2xl font-semibold tracking-[-0.035em] text-(--text)">
            {{ collectionTitle }}
          </h1>
          <span class="mt-1 block text-[0.77rem] text-(--muted-text)">{{
            collectionSummary
          }}</span>
        </div>

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
        class="min-h-0 overflow-auto px-5 py-2.5"
      >
        <table class="w-full border-separate border-spacing-y-0.5 text-left">
          <thead>
            <tr>
              <th
                class="w-[35%] px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
              >
                Title
              </th>
              <th
                class="w-[25%] px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
              >
                Artist
              </th>
              <th
                class="w-[25%] px-4 pb-1.5 text-[0.66rem] font-medium text-(--subtle-text)"
              >
                Album
              </th>
              <th
                class="w-[9%] px-2 pb-1.5 text-center text-(--subtle-text) [&>svg]:mx-auto [&>svg]:size-3.75"
              >
                <span class="sr-only">Duration</span>
                <Clock3 aria-hidden="true" />
              </th>
              <th
                class="w-[6%] px-2 pb-1.5 text-center text-(--subtle-text) [&>svg]:mx-auto [&>svg]:size-3.75"
              >
                <span class="sr-only">Favorite</span>
                <Heart aria-hidden="true" />
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="track in mockTracks"
              :key="track.id"
              class="group"
              :data-current="track.id === currentItem?.id"
            >
              <td
                class="h-10.5 rounded-l-md px-4 text-[0.82rem] text-(--text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
              >
                <span class="flex items-center gap-2.5 font-medium">
                  <Volume2
                    v-if="track.id === currentItem?.id"
                    class="track-playing-indicator size-3.75 shrink-0 text-(--text)"
                    aria-label="Currently playing"
                  />
                  <span v-else class="size-3.75 shrink-0" aria-hidden="true" />
                  <span>{{ track.title }}</span>
                </span>
              </td>
              <td
                class="h-10.5 px-4 text-[0.8rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
              >
                {{ track.artist }}
              </td>
              <td
                class="h-10.5 px-4 text-[0.8rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
              >
                {{ track.album }}
              </td>
              <td
                class="h-10.5 px-2 text-center text-[0.78rem] text-(--muted-text) tabular-nums group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)] group-data-[current=true]:text-(--text)"
              >
                {{ track.duration }}
              </td>
              <td
                class="h-10.5 rounded-r-md px-2 text-center group-hover:bg-[oklch(0.72_0.025_258/0.08)] group-data-[current=true]:bg-[oklch(0.72_0.03_268/0.13)]"
              >
                <button
                  :aria-label="`Favorite ${track.title}`"
                  :aria-pressed="isFavorite(track.id)"
                  class="mx-auto grid size-7 cursor-pointer place-items-center rounded-full border-0 bg-transparent text-(--subtle-text) transition-colors hover:bg-[oklch(0.74_0.05_300/0.1)] hover:text-(--text) aria-pressed:text-accent [&>svg]:size-4"
                  type="button"
                  @click="toggleFavorite(track.id)"
                >
                  <Heart
                    aria-hidden="true"
                    :fill="isFavorite(track.id) ? 'currentColor' : 'none'"
                  />
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <section
        v-else-if="activeCollection === 'albums'"
        class="grid min-h-0 grid-cols-[repeat(auto-fit,minmax(118px,1fr))] content-start gap-x-4.25 gap-y-6 overflow-auto px-7 py-6"
        aria-label="Albums"
      >
        <article
          v-for="album in mockAlbums"
          :key="album.title"
          class="album-tile min-w-0"
        >
          <div
            class="cover-art grid aspect-square w-full place-items-center rounded-lg [&>svg]:size-[34%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
            :data-cover="album.cover"
          >
            <Disc3 aria-hidden="true" />
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
          v-for="artist in mockArtists"
          :key="artist.name"
          class="artist-tile flex min-w-0 items-center gap-3 rounded-lg p-2 hover:bg-[oklch(0.72_0.025_258/0.08)]"
        >
          <div
            class="cover-art grid size-14 shrink-0 place-items-center rounded-full [&>svg]:size-6 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
            :data-cover="artist.cover"
          >
            <Mic2 aria-hidden="true" />
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
        class="col-start-1 row-start-2 flex min-w-0 items-center gap-3 border-t border-r border-(--line) px-6 max-[760px]:hidden"
      >
        <span
          class="cover-art size-15 shrink-0 rounded-md"
          :data-cover="currentCover"
        />
        <div class="min-w-0">
          <p
            class="overflow-hidden text-[0.82rem] font-semibold text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ currentItem?.title ?? "Nothing selected" }}
          </p>
          <span
            class="mt-0.5 block overflow-hidden text-[0.74rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
            >{{ currentItem?.artist ?? "Choose a track" }}</span
          >
        </div>
        <AudioLines
          class="ml-auto size-5 shrink-0 text-accent"
          aria-label="Playback activity"
        />
      </div>

      <div
        class="col-start-2 row-start-2 grid min-w-0 grid-rows-[1fr_auto] border-t border-(--line) px-6 pt-2 pb-2 max-[760px]:col-start-1"
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
            <input
              aria-label="Volume"
              type="range"
              min="0"
              max="100"
              :value="snapshot.volumePercent"
              @input="emit('setVolume', inputValue($event))"
            />
          </label>
        </div>

        <div
          class="grid grid-cols-[30px_minmax(0,1fr)_30px] items-center gap-2 text-[0.65rem] text-(--muted-text) tabular-nums"
          aria-label="Track progress"
        >
          <span>{{ formatDuration(snapshot.positionMs) }}</span>
          <input
            class="pointer-events-none"
            aria-label="Track progress"
            type="range"
            min="0"
            :max="currentItem?.durationMs ?? 0"
            :value="snapshot.positionMs"
            disabled
          />
          <span class="text-right">{{
            formatDuration(currentItem?.durationMs ?? 0)
          }}</span>
        </div>
      </div>
    </footer>
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

.cover-art::after {
  position: absolute;
  inset: 15%;
  content: "";
  border: 1px solid oklch(0.98 0.01 90 / 0.35);
  border-radius: inherit;
  transform: rotate(-18deg);
}
</style>
