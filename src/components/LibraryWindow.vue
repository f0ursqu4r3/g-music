<script setup lang="ts">
import {
  Disc3,
  Heart,
  ListMusic,
  Mic2,
  Music2,
  Pause,
  Play,
  Search,
  SkipBack,
  SkipForward,
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
}>();

const activeCollection = ref<LibraryCollection>("tracks");
const isPlaying = computed(() => props.snapshot.status === "playing");
const currentItem = computed(() => props.snapshot.currentItem);
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
    albums: `${mockAlbums.length} releases in your library`,
    artists: `${mockArtists.length} artists in rotation`,
    tracks: `${mockTracks.length} selections · 25 min`,
  };

  return summaries[activeCollection.value];
});

function selectCollection(collection: LibraryCollection): void {
  activeCollection.value = collection;
}
</script>

<template>
  <main
    class="library-window relative grid min-h-screen grid-cols-[208px_minmax(0,1fr)] bg-[linear-gradient(140deg,oklch(0.31_0.045_246/0.12),transparent_38%),var(--glass-window)] text-(--text) backdrop-saturate-[1.16] max-[760px]:grid-cols-1"
    aria-label="Music library"
  >
    <div
      class="application-drag-region absolute inset-x-0 top-0 z-10 h-13"
      data-tauri-drag-region
      aria-hidden="true"
    />

    <aside
      class="flex flex-col gap-7.5 border-r border-(--line) px-4 pt-18 pb-5 max-[760px]:hidden"
    >
      <label
        class="flex min-h-8.5 items-center gap-2 rounded-lg border border-[oklch(0.7_0.02_248/0.06)] bg-(--glass-control) px-2.5 text-(--subtle-text) focus-within:border-(--focus-ring) [&>svg]:size-3.5"
      >
        <Search aria-hidden="true" />
        <span class="sr-only">Search library</span>
        <input
          class="min-w-0 flex-1 border-0 bg-transparent text-xs text-(--text) outline-0 placeholder:text-(--subtle-text)"
          placeholder="Search"
          type="search"
        />
      </label>

      <nav class="grid gap-0.75" aria-label="Library navigation">
        <p
          class="mb-1.25 px-2.5 text-[0.57rem] font-[720] tracking-widest text-(--subtle-text) uppercase"
        >
          Library
        </p>
        <button
          :aria-current="activeCollection === 'tracks' ? 'page' : undefined"
          class="flex min-h-8 w-full cursor-pointer items-center gap-2.25 rounded-[7px] border-0 bg-transparent px-2.5 text-left text-[0.77rem] text-(--muted-text) hover:bg-[oklch(0.72_0.02_248/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.54_0.09_215/0.2)] aria-[current=page]:text-(--text) aria-[current=page]:shadow-[inset_0_1px_oklch(0.92_0.04_205/0.06)] aria-[current=page]:[&>svg]:text-accent [&>svg]:size-3.75"
          :data-collection="'tracks'"
          type="button"
          @click="selectCollection('tracks')"
        >
          <ListMusic aria-hidden="true" />Tracks
        </button>
        <button
          :aria-current="activeCollection === 'albums' ? 'page' : undefined"
          class="flex min-h-8 w-full cursor-pointer items-center gap-2.25 rounded-[7px] border-0 bg-transparent px-2.5 text-left text-[0.77rem] text-(--muted-text) hover:bg-[oklch(0.72_0.02_248/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.54_0.09_215/0.2)] aria-[current=page]:text-(--text) aria-[current=page]:shadow-[inset_0_1px_oklch(0.92_0.04_205/0.06)] aria-[current=page]:[&>svg]:text-accent [&>svg]:size-3.75"
          :data-collection="'albums'"
          type="button"
          @click="selectCollection('albums')"
        >
          <Disc3 aria-hidden="true" />Albums
        </button>
        <button
          :aria-current="activeCollection === 'artists' ? 'page' : undefined"
          class="flex min-h-8 w-full cursor-pointer items-center gap-2.25 rounded-[7px] border-0 bg-transparent px-2.5 text-left text-[0.77rem] text-(--muted-text) hover:bg-[oklch(0.72_0.02_248/0.1)] hover:text-(--text) aria-[current=page]:bg-[oklch(0.54_0.09_215/0.2)] aria-[current=page]:text-(--text) aria-[current=page]:shadow-[inset_0_1px_oklch(0.92_0.04_205/0.06)] aria-[current=page]:[&>svg]:text-accent [&>svg]:size-3.75"
          :data-collection="'artists'"
          type="button"
          @click="selectCollection('artists')"
        >
          <Mic2 aria-hidden="true" />Artists
        </button>
        <span
          class="flex min-h-8 items-center gap-2.25 rounded-[7px] px-2.5 text-[0.77rem] text-(--muted-text) [&>svg]:size-3.75"
          ><Music2 aria-hidden="true" />Play Queue</span
        >
      </nav>

      <nav class="mt-auto grid gap-0.75" aria-label="Playlists">
        <p
          class="mb-1.25 px-2.5 text-[0.57rem] font-[720] tracking-widest text-(--subtle-text) uppercase"
        >
          Playlists
        </p>
        <a
          class="flex min-h-8 items-center gap-2.25 rounded-[7px] px-2.5 text-[0.77rem] text-(--muted-text) no-underline hover:bg-[oklch(0.72_0.02_248/0.1)] hover:text-(--text) [&>svg]:size-3.75"
          href="#focus"
          ><Heart aria-hidden="true" />Focused work</a
        >
        <a
          class="flex min-h-8 items-center gap-2.25 rounded-[7px] px-2.5 text-[0.77rem] text-(--muted-text) no-underline hover:bg-[oklch(0.72_0.02_248/0.1)] hover:text-(--text) [&>svg]:size-3.75"
          href="#after-hours"
          ><Heart aria-hidden="true" />After hours</a
        >
        <a
          class="flex min-h-8 items-center gap-2.25 rounded-[7px] px-2.5 text-[0.77rem] text-(--muted-text) no-underline hover:bg-[oklch(0.72_0.02_248/0.1)] hover:text-(--text) [&>svg]:size-3.75"
          href="#instrumental"
          ><Heart aria-hidden="true" />Instrumental</a
        >
      </nav>
    </aside>

    <section
      class="library-content grid min-w-0 grid-rows-[auto_minmax(0,1fr)_auto]"
    >
      <header
        class="flex items-end justify-between gap-5 border-b border-(--line) px-9.5 pt-21.5 pb-6"
      >
        <div>
          <p
            class="text-[0.58rem] font-[720] tracking-widest text-(--subtle-text) uppercase"
          >
            Your library
          </p>
          <h1
            class="mt-0.75 text-2xl font-bold tracking-[-0.045em] text-(--text)"
          >
            {{ collectionTitle }}
          </h1>
          <span class="mt-1.5 block text-[0.73rem] text-(--muted-text)">{{
            collectionSummary
          }}</span>
        </div>
        <span class="text-[0.69rem] text-(--subtle-text)"
          >Artwork and queue are in the Window menu</span
        >
      </header>

      <div
        v-if="activeCollection === 'tracks'"
        class="min-h-0 overflow-auto px-6.5 py-3.5"
      >
        <table class="w-full border-separate border-spacing-y-0.75 text-left">
          <thead>
            <tr>
              <th
                class="w-[34%] px-3 pb-2 text-[0.59rem] font-[650] tracking-[0.07em] text-(--subtle-text) uppercase"
              >
                Title
              </th>
              <th
                class="w-[27%] px-3 pb-2 text-[0.59rem] font-[650] tracking-[0.07em] text-(--subtle-text) uppercase"
              >
                Artist
              </th>
              <th
                class="w-[27%] px-3 pb-2 text-[0.59rem] font-[650] tracking-[0.07em] text-(--subtle-text) uppercase"
              >
                Album
              </th>
              <th
                class="w-[12%] px-3 pb-2 text-right text-[0.59rem] font-[650] tracking-[0.07em] text-(--subtle-text) uppercase"
              >
                Time
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
                class="rounded-l-lg px-3 py-2.25 text-[0.77rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.02_248/0.09)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.52_0.085_215/0.2)] group-data-[current=true]:text-(--text) group-data-[current=true]:shadow-[inset_0_1px_oklch(0.9_0.04_205/0.045)]"
              >
                <span
                  class="flex items-center gap-2.5 font-[620] text-(--text)"
                >
                  <span
                    class="cover-art size-6.5 shrink-0 rounded-[5px]"
                    :data-cover="track.cover"
                  />
                  <span>{{ track.title }}</span>
                </span>
              </td>
              <td
                class="px-3 py-2.25 text-[0.77rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.02_248/0.09)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.52_0.085_215/0.2)] group-data-[current=true]:text-(--text) group-data-[current=true]:shadow-[inset_0_1px_oklch(0.9_0.04_205/0.045)]"
              >
                {{ track.artist }}
              </td>
              <td
                class="px-3 py-2.25 text-[0.77rem] text-(--muted-text) group-hover:bg-[oklch(0.72_0.02_248/0.09)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.52_0.085_215/0.2)] group-data-[current=true]:text-(--text) group-data-[current=true]:shadow-[inset_0_1px_oklch(0.9_0.04_205/0.045)]"
              >
                {{ track.album }}
              </td>
              <td
                class="rounded-r-lg px-3 py-2.25 text-right text-[0.77rem] text-(--muted-text) tabular-nums group-hover:bg-[oklch(0.72_0.02_248/0.09)] group-hover:text-(--text) group-data-[current=true]:bg-[oklch(0.52_0.085_215/0.2)] group-data-[current=true]:text-(--text) group-data-[current=true]:shadow-[inset_0_1px_oklch(0.9_0.04_205/0.045)]"
              >
                {{ track.duration }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <section
        v-else-if="activeCollection === 'albums'"
        class="grid grid-cols-[repeat(auto-fit,minmax(118px,1fr))] gap-x-4.25 gap-y-6 px-8 py-7"
        aria-label="Albums"
      >
        <article
          v-for="album in mockAlbums"
          :key="album.title"
          class="album-tile min-w-0"
        >
          <div
            class="cover-art grid aspect-square w-full place-items-center rounded-[9px] [&>svg]:size-[34%] [&>svg]:text-[oklch(0.98_0.01_90/0.74)]"
            :data-cover="album.cover"
          >
            <Disc3 aria-hidden="true" />
          </div>
          <h2
            class="mt-2.25 overflow-hidden text-[0.76rem] font-[650] tracking-[-0.01em] text-ellipsis whitespace-nowrap text-(--text)"
          >
            {{ album.title }}
          </h2>
          <p class="mt-1 text-[0.68rem] text-(--muted-text)">
            {{ album.artist }}
          </p>
        </article>
      </section>

      <section
        v-else
        class="grid content-start grid-cols-[repeat(auto-fit,minmax(200px,1fr))] gap-4.5 px-8 py-7"
        aria-label="Artists"
      >
        <article
          v-for="artist in mockArtists"
          :key="artist.name"
          class="artist-tile flex min-w-0 items-center gap-3 rounded-[11px] p-2 hover:bg-(--surface-muted)"
        >
          <div
            class="cover-art grid size-15 shrink-0 place-items-center rounded-full [&>svg]:size-6.5 [&>svg]:text-[oklch(0.98_0.01_90/0.76)]"
            :data-cover="artist.cover"
          >
            <Mic2 aria-hidden="true" />
          </div>
          <div class="min-w-0">
            <h2
              class="mt-2.25 overflow-hidden text-[0.76rem] font-[650] tracking-[-0.01em] text-ellipsis whitespace-nowrap text-(--text)"
            >
              {{ artist.name }}
            </h2>
            <p class="mt-1 text-[0.68rem] text-(--muted-text)">
              {{ artist.detail }}
            </p>
          </div>
        </article>
      </section>

      <footer
        class="grid min-h-20.5 grid-cols-[minmax(160px,1fr)_auto_minmax(180px,1fr)_auto] items-center gap-5.5 border-t border-(--line) px-7 py-3.25 shadow-[inset_0_1px_oklch(0.95_0.01_248/0.025)] max-[760px]:grid-cols-[minmax(0,1fr)_auto]"
      >
        <div class="flex min-w-0 items-center gap-2.5">
          <span
            class="cover-art size-12 shrink-0 rounded-[7px]"
            data-cover="violet"
          />
          <div class="min-w-0">
            <p
              class="overflow-hidden text-xs font-[650] text-ellipsis whitespace-nowrap text-(--text)"
            >
              {{ currentItem?.title ?? "Nothing selected" }}
            </p>
            <span
              class="mt-0.75 block overflow-hidden text-[0.69rem] text-ellipsis whitespace-nowrap text-(--muted-text)"
              >{{ currentItem?.artist ?? "Choose a track to begin" }}</span
            >
          </div>
        </div>
        <nav class="flex items-center gap-0.75" aria-label="Playback controls">
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
            class="rounded-full bg-accent text-(--accent-ink)"
            size="icon"
            :disabled="isUpdating"
            @click="emit('toggle')"
          >
            <Pause v-if="isPlaying" aria-hidden="true" />
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
        </nav>
        <div
          class="grid grid-cols-[30px_minmax(0,1fr)_30px] items-center gap-1.75 text-[0.61rem] text-(--subtle-text) tabular-nums max-[760px]:hidden"
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
          <span>{{ formatDuration(currentItem?.durationMs ?? 0) }}</span>
        </div>
        <span
          class="flex items-center gap-1.75 text-[0.67rem] text-(--muted-text) tabular-nums max-[760px]:hidden [&>svg]:size-3.75"
          ><Volume2 aria-hidden="true" />{{ snapshot.volumePercent }}</span
        >
      </footer>
    </section>
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
